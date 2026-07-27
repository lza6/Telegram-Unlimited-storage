"""Upload concurrency gates and progress event bus.

Mirrors the Rust upload_gate (file/chunk slot semaphores) and the progress
broadcast used by /upload_events (SSE), /upload_ws and /upload_status.
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ProgressState:
    session_id: str
    filename: str = ""
    total_chunks: int = 0
    uploaded_chunks: int = 0
    status: str = "active"  # active | completed | failed
    file_id: str | None = None
    download_url: str | None = None
    updated_at: float = field(default_factory=time.time)

    def snapshot(self) -> dict[str, Any]:
        data: dict[str, Any] = {
            "session_id": self.session_id,
            "filename": self.filename,
            "uploaded_chunks": self.uploaded_chunks,
            "total_chunks": self.total_chunks,
            "status": self.status,
        }
        if self.file_id is not None:
            data["file_id"] = self.file_id
        if self.download_url is not None:
            data["download_url"] = self.download_url
        return data


class ProgressBus:
    """Fan-out of progress snapshots to SSE/WebSocket subscribers."""

    def __init__(self) -> None:
        self._subscribers: set[asyncio.Queue[dict[str, Any] | None]] = set()

    def publish(self, snapshot: dict[str, Any]) -> None:
        for queue in list(self._subscribers):
            try:
                queue.put_nowait(snapshot)
            except asyncio.QueueFull:
                pass  # slow consumer — drop, next snapshot follows shortly

    async def subscribe(self) -> asyncio.Queue[dict[str, Any] | None]:
        queue: asyncio.Queue[dict[str, Any] | None] = asyncio.Queue(maxsize=16)
        self._subscribers.add(queue)
        return queue

    def unsubscribe(self, queue: asyncio.Queue[dict[str, Any] | None]) -> None:
        self._subscribers.discard(queue)

    async def close(self) -> None:
        for queue in list(self._subscribers):
            try:
                queue.put_nowait(None)
            except asyncio.QueueFull:
                pass
        self._subscribers.clear()


class TransferManager:
    """Slot semaphores + per-session progress tracking."""

    def __init__(
        self,
        file_slots: int,
        chunk_slots: int,
        storage: Any | None = None,
    ) -> None:
        self.file_slots_total = max(1, file_slots)
        self.chunk_slots_total = max(1, chunk_slots)
        self._file_sem = asyncio.Semaphore(self.file_slots_total)
        self._chunk_sem = asyncio.Semaphore(self.chunk_slots_total)
        self._file_in_use = 0
        self._chunk_in_use = 0
        self._progress: dict[str, ProgressState] = {}
        self._buses: dict[str, ProgressBus] = {}
        # Presigned-URL download counters keyed by signature.
        # Values are (count, last_seen_timestamp) tuples for TTL-based pruning.
        self._download_counts: dict[str, tuple[int, float]] = {}
        # Idempotency-Key → (status_code, media_type, body_bytes) cache for /upload.
        # Prevents duplicate uploads when the client retries after a timeout.
        self._idempotency_cache: dict[str, tuple[int, str, bytes, float]] = {}
        self._idempotency_locks: dict[str, asyncio.Lock] = {}
        # v8 (TASK-P1-04): optional persistent backend so idempotency survives
        # restarts. When set, idempotency_get/put mirror to storage so a process
        # restart replays the cached response instead of re-executing the upload.
        self._storage = storage

    # ── slot accounting ─────────────────────────────────────────────────────
    def queue_status(self) -> dict[str, int]:
        return {
            "chunk_slots_total": self.chunk_slots_total,
            "chunk_slots_available": self.chunk_slots_total - self._chunk_in_use,
            "file_slots_total": self.file_slots_total,
            "file_slots_available": self.file_slots_total - self._file_in_use,
        }

    def try_acquire_file_slot(self) -> bool:
        if self._file_in_use >= self.file_slots_total:
            return False
        self._file_in_use += 1
        return True

    def release_file_slot(self) -> None:
        self._file_in_use = max(0, self._file_in_use - 1)

    def try_acquire_chunk_slot(self) -> bool:
        if self._chunk_in_use >= self.chunk_slots_total:
            return False
        self._chunk_in_use += 1
        return True

    def release_chunk_slot(self) -> None:
        self._chunk_in_use = max(0, self._chunk_in_use - 1)

    # ── progress ────────────────────────────────────────────────────────────
    def ensure_progress(
        self, session_id: str, filename: str = "", total_chunks: int = 0
    ) -> ProgressState:
        state = self._progress.get(session_id)
        if state is None:
            state = ProgressState(
                session_id=session_id,
                filename=filename,
                total_chunks=total_chunks,
            )
            self._progress[session_id] = state
        else:
            if filename:
                state.filename = filename
            if total_chunks:
                state.total_chunks = total_chunks
        return state

    def get_progress(self, session_id: str) -> ProgressState | None:
        return self._progress.get(session_id)

    def update_progress(self, session_id: str, **changes: Any) -> ProgressState | None:
        state = self._progress.get(session_id)
        if state is None:
            return None
        for key, value in changes.items():
            if hasattr(state, key):
                setattr(state, key, value)
        state.updated_at = time.time()
        bus = self._buses.get(session_id)
        if bus is not None:
            bus.publish(state.snapshot())
        return state

    def bus_for(self, session_id: str) -> ProgressBus:
        bus = self._buses.get(session_id)
        if bus is None:
            bus = ProgressBus()
            self._buses[session_id] = bus
        return bus

    # ── presigned download counting ─────────────────────────────────────────
    def count_download(self, signature: str) -> int:
        prev = self._download_counts.get(signature)
        count = (prev[0] if prev else 0) + 1
        self._download_counts[signature] = (count, time.time())
        return count

    # ── idempotency cache ──────────────────────────────────────────────────
    def idempotency_get(
        self, key: str,
    ) -> tuple[int, str, bytes] | None:
        """Return (status, media_type, body_bytes) if key was already processed.

        v8 (TASK-P1-04): falls back to the persistent store on a memory miss so
        a restart replays the cached response. Returns the sentinel
        ``("__PROCESSING__", "", b"")`` when a request is in-flight so the
        caller can return 409 Conflict.
        """
        entry = self._idempotency_cache.get(key)
        if entry is not None:
            status, media_type, body, stored_at = entry
            if time.time() - stored_at > 3600.0:
                self._idempotency_cache.pop(key, None)
            else:
                return status, media_type, body
        # Memory miss → check persistent store (v8).
        if self._storage is not None:
            raw = self._storage.get_idempotency(key)
            if raw == "__PROCESSING__":
                # In-flight in a previous process; treat as conflict.
                return 409, "", b""
            if raw and raw != "__PROCESSING__":
                try:
                    import base64
                    import json

                    payload = json.loads(raw)
                    body = base64.b64decode(payload.get("body", ""))
                    return int(payload["status"]), str(payload["media_type"]), body
                except (ValueError, KeyError, TypeError):
                    return None
        return None

    def idempotency_put(
        self, key: str, status: int, media_type: str, body: bytes,
    ) -> None:
        self._idempotency_cache[key] = (status, media_type, body, time.time())
        # v8: mirror to persistent store so restart replays.
        if self._storage is not None:
            import base64

            payload = {
                "status": status,
                "media_type": media_type,
                "body": base64.b64encode(body).decode("ascii"),
            }
            self._storage.set_idempotency_complete(key, payload)

    def idempotency_mark_processing(self, key: str) -> None:
        """Mark a key as in-flight in the persistent store (v8)."""
        if self._storage is not None:
            self._storage.set_idempotency_processing(key)

    def idempotency_lock(self, key: str) -> asyncio.Lock:
        """Return (or create) a per-key lock so concurrent retries serialise."""
        lock = self._idempotency_locks.get(key)
        if lock is None:
            lock = asyncio.Lock()
            self._idempotency_locks[key] = lock
        return lock

    def prune_progress(self, max_age_secs: float = 3600.0) -> None:
        now = time.time()
        stale = [
            sid
            for sid, state in self._progress.items()
            if now - state.updated_at > max_age_secs
            and state.status in ("completed", "failed")
        ]
        for sid in stale:
            self._progress.pop(sid, None)
            self._buses.pop(sid, None)
        # Prune download counters older than max_age_secs.
        stale_dl = [
            sig for sig, (_, last_seen) in self._download_counts.items()
            if now - last_seen > max_age_secs
        ]
        for sig in stale_dl:
            self._download_counts.pop(sig, None)
        # Prune idempotency cache entries older than max_age_secs.
        stale_idem = [
            k for k, (_, _, _, stored_at) in self._idempotency_cache.items()
            if now - stored_at > max_age_secs
        ]
        for k in stale_idem:
            self._idempotency_cache.pop(k, None)

    # ── transfer listing (TASK-U-02) ─────────────────────────────────────────
    def list_all_progress(self) -> list[dict[str, Any]]:
        """Return a snapshot of all tracked transfer progress states."""
        return [state.snapshot() for state in self._progress.values()]

    def cancel_transfer(self, session_id: str) -> bool:
        """Mark a transfer as cancelled; clients polling will see status='cancelled'."""
        state = self._progress.get(session_id)
        if state is None:
            return False
        state.status = "cancelled"
        state.updated_at = time.time()
        bus = self._buses.get(session_id)
        if bus is not None:
            bus.publish(state.snapshot())
        return True

    def retry_transfer(self, session_id: str) -> bool:
        """Mark a failed/cancelled transfer for retry by resetting to 'queued'."""
        state = self._progress.get(session_id)
        if state is None:
            return False
        if state.status not in ("failed", "cancelled"):
            return False
        state.status = "queued"
        state.updated_at = time.time()
        bus = self._buses.get(session_id)
        if bus is not None:
            bus.publish(state.snapshot())
        return True

    def pause_transfer(self, session_id: str) -> bool:
        """Mark a running transfer as paused."""
        state = self._progress.get(session_id)
        if state is None:
            return False
        state.status = "paused"
        state.updated_at = time.time()
        bus = self._buses.get(session_id)
        if bus is not None:
            bus.publish(state.snapshot())
        return True
