"""TransferManager unit tests — slot accounting, progress lifecycle, pruning, idempotency."""

from __future__ import annotations

import time

from app.transfers import TransferManager


class TestSlotAccounting:
    def test_file_slots_acquire_and_release(self):
        tm = TransferManager(file_slots=2, chunk_slots=4)
        assert tm.try_acquire_file_slot() is True
        assert tm.try_acquire_file_slot() is True
        assert tm.try_acquire_file_slot() is False  # full
        tm.release_file_slot()
        assert tm.try_acquire_file_slot() is True

    def test_chunk_slots_acquire_and_release(self):
        tm = TransferManager(file_slots=1, chunk_slots=2)
        assert tm.try_acquire_chunk_slot() is True
        assert tm.try_acquire_chunk_slot() is True
        assert tm.try_acquire_chunk_slot() is False
        tm.release_chunk_slot()
        assert tm.try_acquire_chunk_slot() is True

    def test_release_never_goes_negative(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        tm.release_file_slot()  # no prior acquire
        tm.release_chunk_slot()
        status = tm.queue_status()
        assert status["file_slots_available"] == 1
        assert status["chunk_slots_available"] == 1

    def test_queue_status_reflects_usage(self):
        tm = TransferManager(file_slots=3, chunk_slots=5)
        tm.try_acquire_file_slot()
        tm.try_acquire_chunk_slot()
        tm.try_acquire_chunk_slot()
        status = tm.queue_status()
        assert status["file_slots_total"] == 3
        assert status["file_slots_available"] == 2
        assert status["chunk_slots_total"] == 5
        assert status["chunk_slots_available"] == 3


class TestProgressLifecycle:
    def test_ensure_creates_state(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        state = tm.ensure_progress("sess-1", "file.zip", 10)
        assert state.session_id == "sess-1"
        assert state.filename == "file.zip"
        assert state.total_chunks == 10
        assert state.status == "active"

    def test_ensure_updates_existing(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        tm.ensure_progress("sess-1", "old.zip", 5)
        state = tm.ensure_progress("sess-1", "new.zip", 10)
        assert state.filename == "new.zip"
        assert state.total_chunks == 10

    def test_update_progress(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        tm.ensure_progress("sess-1", "file.zip", 10)
        result = tm.update_progress("sess-1", uploaded_chunks=5, status="completed")
        assert result is not None
        assert result.uploaded_chunks == 5
        assert result.status == "completed"

    def test_update_nonexistent_returns_none(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        assert tm.update_progress("nonexistent", status="failed") is None

    def test_get_progress(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        assert tm.get_progress("sess-1") is None
        tm.ensure_progress("sess-1")
        assert tm.get_progress("sess-1") is not None


class TestProgressPruning:
    def test_prune_stale_completed(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        state = tm.ensure_progress("sess-1")
        state.status = "completed"
        state.updated_at = time.time() - 7200  # 2h ago
        tm.prune_progress(max_age_secs=3600)
        assert tm.get_progress("sess-1") is None

    def test_prune_stale_failed(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        state = tm.ensure_progress("sess-1")
        state.status = "failed"
        state.updated_at = time.time() - 7200
        tm.prune_progress(max_age_secs=3600)
        assert tm.get_progress("sess-1") is None

    def test_prune_keeps_active(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        state = tm.ensure_progress("sess-1")
        state.status = "active"
        state.updated_at = time.time() - 7200
        tm.prune_progress(max_age_secs=3600)
        assert tm.get_progress("sess-1") is not None

    def test_prune_keeps_recent(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        state = tm.ensure_progress("sess-1")
        state.status = "completed"
        state.updated_at = time.time() - 100  # recent
        tm.prune_progress(max_age_secs=3600)
        assert tm.get_progress("sess-1") is not None


class TestIdempotencyCache:
    def test_get_nonexistent_returns_none(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        assert tm.idempotency_get("key-1") is None

    def test_put_and_get(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        tm.idempotency_put("key-1", 200, "application/json", b'{"ok":true}')
        result = tm.idempotency_get("key-1")
        assert result == (200, "application/json", b'{"ok":true}')

    def test_expired_entry_returns_none(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        tm.idempotency_put("key-1", 200, "text/plain", b"ok")
        # Manually age the entry
        entry = tm._idempotency_cache["key-1"]
        tm._idempotency_cache["key-1"] = (entry[0], entry[1], entry[2], time.time() - 7200)
        assert tm.idempotency_get("key-1") is None

    def test_idempotency_lock_returns_same_lock(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        lock1 = tm.idempotency_lock("key-1")
        lock2 = tm.idempotency_lock("key-1")
        assert lock1 is lock2


class TestDownloadCounts:
    def test_count_increments(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        assert tm.count_download("sig-1") == 1
        assert tm.count_download("sig-1") == 2
        assert tm.count_download("sig-1") == 3

    def test_different_signatures_independent(self):
        tm = TransferManager(file_slots=1, chunk_slots=1)
        assert tm.count_download("sig-1") == 1
        assert tm.count_download("sig-2") == 1
