"""Resumable upload session manager (TASK-P0-02).

Provides chunk-level manifest tracking and idempotency verification.
Enables interrupted uploads (network drops, browser restarts) to resume
without re-uploading completed chunks.
"""

from __future__ import annotations

import hashlib
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from .storage import Storage


@dataclass
class ResumableSession:
    session_id: str
    filename: str
    total_chunks: int
    total_size: int
    file_hash: str
    status: str
    created_at: int
    expires_at: int


class ResumeManager:
    """Manages upload sessions and chunk manifest verification in Storage."""

    def __init__(self, storage: Storage, session_ttl_secs: int = 7 * 86400) -> None:
        self.storage = storage
        self.session_ttl_secs = session_ttl_secs

    def init_session(
        self,
        filename: str,
        total_chunks: int,
        total_size: int,
        file_hash: str,
        owner_id: str = "default",
    ) -> ResumableSession:
        """Initialize or retrieve an existing upload session by file hash.

        If an active session matching file_hash and owner_id exists and has not
        expired, it is reused so the client can resume chunk uploads.
        """
        now = int(time.time())
        expires_at = now + self.session_ttl_secs

        # Query existing session by file_hash
        rows = self.storage._query(
            "SELECT session_id, filename, total_chunks, status, created_at, expires_at "
            "FROM upload_sessions WHERE file_hash = ? AND owner_id = ? AND status = 'active' "
            "AND expires_at > ?",
            (file_hash, owner_id, now),
        )
        if rows:
            r = rows[0]
            return ResumableSession(
                session_id=r["session_id"],
                filename=r["filename"],
                total_chunks=r["total_chunks"],
                total_size=total_size,
                file_hash=file_hash,
                status=r["status"],
                created_at=r["created_at"],
                expires_at=r["expires_at"],
            )

        # Create new session ID
        raw = f"{owner_id}|{file_hash}|{now}|{total_size}".encode("utf-8")
        session_id = f"res_{hashlib.sha256(raw).hexdigest()[:24]}"

        self.storage._write(
            "INSERT INTO upload_sessions (session_id, filename, total_chunks, total_size, file_hash, owner_id, status, created_at, expires_at) "
            "VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)",
            (session_id, filename, total_chunks, total_size, file_hash, owner_id, now, expires_at),
        )

        return ResumableSession(
            session_id=session_id,
            filename=filename,
            total_chunks=total_chunks,
            total_size=total_size,
            file_hash=file_hash,
            status="active",
            created_at=now,
            expires_at=expires_at,
        )

    def get_missing_chunks(self, session_id: str) -> list[int]:
        """Return list of chunk indices (0-based) that have not been completed."""
        rows = self.storage._query(
            "SELECT total_chunks FROM upload_sessions WHERE session_id = ?",
            (session_id,),
        )
        if not rows:
            raise KeyError(f"session_id {session_id} not found")

        total_chunks = rows[0]["total_chunks"]

        completed_rows = self.storage._query(
            "SELECT chunk_index FROM upload_chunks WHERE session_id = ? AND status = 'uploaded'",
            (session_id,),
        )
        completed_set = {r["chunk_index"] for r in completed_rows}

        return [i for i in range(total_chunks) if i not in completed_set]

    def record_chunk(
        self,
        session_id: str,
        chunk_index: int,
        chunk_bytes: bytes,
        file_id: Optional[str] = None,
        expected_sha256: Optional[str] = None,
    ) -> bool:
        """Record chunk verification and mark it complete in storage.

        Validates payload sha256 checksum if expected_sha256 is provided.
        Returns True if chunk recorded (or already completed), False if checksum mismatch.
        """
        actual_sha256 = hashlib.sha256(chunk_bytes).hexdigest()
        if expected_sha256 and expected_sha256.lower() != actual_sha256:
            return False

        now = int(time.time())
        # Idempotent insert/update (using 'uploaded' to maintain parity with legacy schema)
        self.storage._write(
            "INSERT INTO upload_chunks (session_id, chunk_index, file_id, sha256, status, created_at) "
            "VALUES (?, ?, ?, ?, 'uploaded', ?) "
            "ON CONFLICT(session_id, chunk_index) DO UPDATE SET status='uploaded', file_id=excluded.file_id, sha256=excluded.sha256",
            (session_id, chunk_index, file_id or "", actual_sha256, now),
        )
        return True

    def is_complete(self, session_id: str) -> bool:
        """Return True if all chunks for session_id are completed."""
        missing = self.get_missing_chunks(session_id)
        return len(missing) == 0

    def mark_session_completed(self, session_id: str) -> None:
        """Mark session status as completed."""
        self.storage._write(
            "UPDATE upload_sessions SET status = 'completed' WHERE session_id = ?",
            (session_id,),
        )

    def cleanup_expired_sessions(self) -> int:
        """Delete sessions and chunk records older than expires_at. Returns count removed."""
        now = int(time.time())
        expired = self.storage._query(
            "SELECT session_id FROM upload_sessions WHERE expires_at <= ?",
            (now,),
        )
        if not expired:
            return 0

        session_ids = [r["session_id"] for r in expired]
        for sid in session_ids:
            self.storage._write("DELETE FROM upload_chunks WHERE session_id = ?", (sid,))
            self.storage._write("DELETE FROM upload_sessions WHERE session_id = ?", (sid,))

        return len(session_ids)
