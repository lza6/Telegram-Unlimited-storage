"""TDD tests for idempotency key persistence (TASK-P1-04, v8.0)."""

from __future__ import annotations

from pathlib import Path

from app.storage import Storage
from app.transfers import TransferManager


def _storage(tmp_path: Path) -> Storage:
    return Storage(tmp_path / "idem.db")


def test_idempotency_put_then_get_round_trip(tmp_path: Path) -> None:
    """In-memory put then get returns the cached response."""
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=_storage(tmp_path))
    tm.idempotency_put("k1", 200, "application/json", b'{"ok":true}')
    got = tm.idempotency_get("k1")
    assert got is not None
    status, media_type, body = got
    assert status == 200
    assert media_type == "application/json"
    assert body == b'{"ok":true}'


def test_idempotency_survives_memory_eviction(tmp_path: Path) -> None:
    """After memory entry is popped, the persistent store replays it."""
    store = _storage(tmp_path)
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=store)
    tm.idempotency_put("k2", 201, "text/plain", b"created")
    # Simulate restart: fresh TransferManager reuses the same storage.
    tm2 = TransferManager(file_slots=2, chunk_slots=4, storage=store)
    got = tm2.idempotency_get("k2")
    assert got is not None
    status, media_type, body = got
    assert status == 201
    assert body == b"created"


def test_idempotency_returns_none_for_unknown_key(tmp_path: Path) -> None:
    """Unknown key → None (no cached response)."""
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=_storage(tmp_path))
    assert tm.idempotency_get("never") is None


def test_idempotency_mark_processing_returns_conflict(tmp_path: Path) -> None:
    """A key marked processing → 409 conflict on subsequent get (in-flight)."""
    store = _storage(tmp_path)
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=store)
    tm.idempotency_mark_processing("k3")
    got = tm.idempotency_get("k3")
    assert got is not None
    assert got[0] == 409


def test_idempotency_without_storage_works(tmp_path: Path) -> None:
    """No storage backend → pure in-memory behaviour (backward compat)."""
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=None)
    tm.idempotency_put("k4", 200, "text/plain", b"ok")
    assert tm.idempotency_get("k4") == (200, "text/plain", b"ok")
    # Memory-only: a fresh manager loses the entry.
    tm2 = TransferManager(file_slots=2, chunk_slots=4, storage=None)
    assert tm2.idempotency_get("k4") is None


def test_idempotency_lock_returns_per_key_lock(tmp_path: Path) -> None:
    """idempotency_lock returns the same lock for the same key."""
    tm = TransferManager(file_slots=2, chunk_slots=4, storage=_storage(tmp_path))
    lock1 = tm.idempotency_lock("k5")
    lock2 = tm.idempotency_lock("k5")
    assert lock1 is lock2
