"""TASK-P0-02: 断点续传 — 验收测试.

验证：
1. 客户端 manifest + 服务端幂等
2. 重复 init 同 file_hash 返回同 session
3. 分片 sha256 不匹配拒绝
4. 重复上传同分片幂等
5. complete 时校验所有分片齐全
6. 7 天过期清理任务
"""

from __future__ import annotations

import hashlib
import time
from pathlib import Path

import pytest

from app.resume import ResumeManager
from app.storage import Storage


@pytest.fixture
def storage(tmp_path):
    s = Storage(tmp_path / "test.db")
    yield s
    s.close()


def test_init_session_idempotent_by_file_hash(storage):
    rm = ResumeManager(storage)
    s1 = rm.init_session("file.bin", 4, 1000, "hash123", "owner1")
    s2 = rm.init_session("file.bin", 4, 1000, "hash123", "owner1")
    assert s1.session_id == s2.session_id
    assert s1.status == "active"


def test_init_session_different_owner_separate(storage):
    rm = ResumeManager(storage)
    s1 = rm.init_session("file.bin", 4, 1000, "hash123", "owner1")
    s2 = rm.init_session("file.bin", 4, 1000, "hash123", "owner2")
    assert s1.session_id != s2.session_id


def test_record_chunk_and_missing_chunks(storage):
    rm = ResumeManager(storage)
    s = rm.init_session("file.bin", 4, 1000, "hash123", "owner1")

    # Initially all 4 are missing
    missing = rm.get_missing_chunks(s.session_id)
    assert len(missing) == 4
    assert sorted(missing) == [0, 1, 2, 3]

    # Record chunk 1 and 3
    data1 = b"chunk-1-data"
    assert rm.record_chunk(s.session_id, 1, data1) is True
    data3 = b"chunk-3-data"
    assert rm.record_chunk(s.session_id, 3, data3) is True

    missing = rm.get_missing_chunks(s.session_id)
    assert sorted(missing) == [0, 2]


def test_record_chunk_checksum_mismatch(storage):
    rm = ResumeManager(storage)
    s = rm.init_session("file.bin", 2, 100, "hash123", "owner1")
    data = b"data"
    wrong_sha = hashlib.sha256(b"other").hexdigest()
    assert rm.record_chunk(s.session_id, 0, data, expected_sha256=wrong_sha) is False


def test_record_chunk_checksum_match(storage):
    rm = ResumeManager(storage)
    s = rm.init_session("file.bin", 2, 100, "hash123", "owner1")
    data = b"data"
    correct_sha = hashlib.sha256(data).hexdigest()
    assert rm.record_chunk(s.session_id, 0, data, expected_sha256=correct_sha) is True


def test_record_chunk_idempotent(storage):
    rm = ResumeManager(storage)
    s = rm.init_session("file.bin", 1, 100, "hash123", "owner1")
    data = b"data"
    assert rm.record_chunk(s.session_id, 0, data) is True
    assert rm.record_chunk(s.session_id, 0, data) is True
    # Still only 1 chunk recorded
    chunks = storage.list_upload_chunks(s.session_id)
    assert len(chunks) == 1
    assert chunks[0]["status"] == "uploaded"


def test_is_complete_and_mark_completed(storage):
    rm = ResumeManager(storage)
    s = rm.init_session("file.bin", 2, 100, "hash123", "owner1")
    assert rm.is_complete(s.session_id) is False

    rm.record_chunk(s.session_id, 0, b"a")
    assert rm.is_complete(s.session_id) is False

    rm.record_chunk(s.session_id, 1, b"b")
    assert rm.is_complete(s.session_id) is True

    rm.mark_session_completed(s.session_id)
    session = storage.get_upload_session(s.session_id)
    assert session["status"] == "completed"


def test_cleanup_expired_sessions(storage):
    rm = ResumeManager(storage, session_ttl_secs=1)
    s = rm.init_session("file.bin", 1, 100, "hash123", "owner1")

    # Force expiration
    storage._write(
        "UPDATE upload_sessions SET expires_at = ? WHERE session_id = ?",
        (int(time.time()) - 10, s.session_id),
    )

    count = rm.cleanup_expired_sessions()
    assert count == 1
    assert storage.get_upload_session(s.session_id) is None
    assert storage.list_upload_chunks(s.session_id) == []
