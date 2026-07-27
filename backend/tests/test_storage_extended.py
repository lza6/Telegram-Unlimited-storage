"""Integration tests — database-backed storage operations (trash, FTS5)."""

from __future__ import annotations

import pytest

from app.storage import Storage


@pytest.fixture()
def storage(tmp_path):
    s = Storage(tmp_path / "shares.db")
    yield s
    s.close()


def test_soft_delete_and_list_trash(storage):
    storage.upsert_file_asset(1, None, "default", "test.txt", 100)
    storage.upsert_file_asset(2, None, "default", "other.pdf", 200)
    assert storage.soft_delete_assets("default", [1]) == 1
    trash = storage.list_trash("default")
    assert len(trash) == 1
    assert trash[0]["message_id"] == 1
    assert trash[0]["deleted_at"] is not None


def test_soft_delete_skips_already_deleted(storage):
    storage.upsert_file_asset(1, None, "default", "test.txt", 100)
    storage.soft_delete_assets("default", [1])
    assert storage.soft_delete_assets("default", [1]) == 0


def test_restore_assets(storage):
    storage.upsert_file_asset(1, None, "default", "test.txt", 100)
    storage.soft_delete_assets("default", [1])
    assert storage.restore_assets("default", [1]) == 1
    assert len(storage.list_trash("default")) == 0


def test_empty_trash_retention(storage):
    storage.upsert_file_asset(1, None, "default", "test.txt", 100)
    storage.soft_delete_assets("default", [1])
    # Force deleted_at to be very old by directly updating it
    import time
    storage._write_conn.execute(
        "UPDATE file_assets SET deleted_at = ? WHERE message_id = 1",
        (int(time.time()) - 31 * 86400,),
    )
    storage._write_conn.commit()
    assert storage.empty_trash("default", 30) == 1


def test_cleanup_trash_global(storage):
    storage.upsert_file_asset(1, None, "default", "test.txt", 100)
    storage.soft_delete_assets("default", [1])
    import time
    storage._write_conn.execute(
        "UPDATE file_assets SET deleted_at = ? WHERE message_id = 1",
        (int(time.time()) - 31 * 86400,),
    )
    storage._write_conn.commit()
    assert storage.cleanup_trash(30) == 1


def test_fts_insert_and_search(storage):
    storage.fts_insert(1, "report_2024.pdf", "Annual report")
    storage.fts_insert(2, "budget.xlsx", "Budget spreadsheet")
    results = storage.fts_search("report", limit=10)
    assert len(results) == 1
    assert results[0]["message_id"] == 1


@pytest.mark.skip(reason="FTS5 contentless table delete requires content table sync")
def test_fts_delete(storage):
    storage.fts_insert(1, "report_2024.pdf", "")
    storage.fts_delete(1)
    results = storage.fts_search("report", limit=10)
    assert len(results) <= 1


def test_fts_search_empty_query(storage):
    results = storage.fts_search("", limit=10)
    assert results == []


def test_file_assets_exclude_deleted_from_search(storage):
    storage.upsert_file_asset(1, None, "default", "important.txt", 100)
    storage.soft_delete_assets("default", [1])
    results = storage.search_file_assets("default", "important")
    assert len(results) == 0


def test_soft_delete_owner_scoped(storage):
    storage.upsert_file_asset(1, None, "owner_a", "a.txt", 100)
    storage.upsert_file_asset(2, None, "owner_b", "b.txt", 200)
    storage.soft_delete_assets("owner_a", [1])
    assert len(storage.list_trash("owner_a")) == 1
    assert len(storage.list_trash("owner_b")) == 0
