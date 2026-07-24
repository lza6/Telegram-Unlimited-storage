"""Storage layer tests — schema, shares, upload sessions/chunks, tenants."""

from __future__ import annotations

import pytest

from app.storage import Storage


@pytest.fixture()
def storage(tmp_path):
    s = Storage(tmp_path / "shares.db")
    yield s
    s.close()


def test_schema_created(storage):
    rows = storage._query(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    )
    names = {r["name"] for r in rows}
    assert {"shared_links", "upload_sessions", "upload_chunks", "tenants"} <= names


def test_create_and_get_share(storage):
    share = storage.create_share(
        share_id="abc123",
        folder_id=None,
        message_id=42,
        file_name="f.txt",
        file_size=10,
        password_hash=None,
        password_salt=None,
        expires_at=None,
        owner_id="system:web",
    )
    assert share["id"] == "abc123"
    assert storage.get_share("abc123")["message_id"] == 42


def test_list_shares_owner_scoped(storage):
    storage.create_share("s1", None, 1, "a", 1, None, None, None, "tenant:t1")
    storage.create_share("s2", None, 2, "b", 1, None, None, None, "tenant:t2")
    assert len(storage.list_shares("tenant:t1")) == 1
    assert len(storage.list_shares(None)) == 2


def test_revoke_and_cleanup_expired(storage):
    storage.create_share("s1", None, 1, "a", 1, None, None, None, None)
    storage.create_share("s2", None, 2, "b", 1, None, None, 1, None)  # expired
    assert storage.cleanup_expired_shares() == 1
    assert storage.get_share("s2")["revoked"] == 1
    storage.revoke_share("s1")
    assert storage.get_share("s1")["revoked"] == 1


def test_create_upload_session_idempotent_and_precreates_chunks(storage):
    storage.create_upload_session("sess", "f.bin", 3, 9999999999)
    storage.create_upload_session("sess", "f.bin", 3, 9999999999)  # idempotent
    chunks = storage.list_upload_chunks("sess")
    assert len(chunks) == 3
    assert all(c["status"] == "pending" for c in chunks)
    assert storage.get_upload_session("sess")["status"] == "active"


def test_record_upload_chunk_marks_uploaded(storage):
    storage.create_upload_session("sess", "f.bin", 2, 9999999999)
    storage.record_upload_chunk("sess", 0, "fid0", "sha0")
    chunk = storage.get_upload_chunk("sess", 0)
    assert chunk["status"] == "uploaded"
    assert chunk["file_id"] == "fid0"
    # chunk 1 still pending
    assert storage.get_upload_chunk("sess", 1)["status"] == "pending"


def test_tenant_upsert_and_lookup(storage):
    storage.upsert_tenant("default", "hash1", "Default")
    assert storage.get_enabled_tenant_by_hash("hash1")["tenant_id"] == "default"
    assert storage.get_enabled_tenant_by_hash("nope") is None
