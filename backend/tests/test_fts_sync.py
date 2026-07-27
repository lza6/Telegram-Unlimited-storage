"""Tests for FTS5 auto-sync triggers + quota alerts (TASK-P2-01, v8.0)."""

from __future__ import annotations

from pathlib import Path

from app.storage import Storage


def _storage(tmp_path: Path) -> Storage:
    return Storage(tmp_path / "fts.db")


def test_fts_auto_syncs_on_file_asset_insert(tmp_path: Path) -> None:
    """Inserting a file_asset row auto-populates file_fts (no manual fts_insert)."""
    s = _storage(tmp_path)
    try:
        s.upsert_file_asset(
            message_id=2001,
            folder_id=None,
            owner_id="owner-1",
            file_name="vacation_photo.jpg",
            file_size=1024,
        )
        results = s.fts_search("vacation")
        assert any(r["message_id"] == 2001 for r in results)
    finally:
        s.close()


def test_fts_auto_syncs_on_rename(tmp_path: Path) -> None:
    """Updating file_name in file_assets updates the FTS index (trigger)."""
    s = _storage(tmp_path)
    try:
        s.upsert_file_asset(2002, None, "owner", "old_name.txt", 10)
        # Rename: update file_assets directly (UPDATE triggers FTS sync).
        s._execute(  # type: ignore[attr-defined]
            "UPDATE file_assets SET file_name = ? WHERE message_id = ?",
            ("renamed_doc.txt", 2002),
        )
        # Old name no longer matches.
        assert s.fts_search("old_name") == []
        # New name matches.
        results = s.fts_search("renamed_doc")
        assert any(r["message_id"] == 2002 for r in results)
    finally:
        s.close()


def test_fts_auto_syncs_on_delete(tmp_path: Path) -> None:
    """Deleting a file_asset row removes it from the FTS index."""
    s = _storage(tmp_path)
    try:
        s.upsert_file_asset(2003, None, "owner", "to_delete.pdf", 50)
        assert s.fts_search("to_delete")  # present before delete
        s._execute("DELETE FROM file_assets WHERE message_id = ?", (2003,))  # type: ignore[attr-defined]
        assert s.fts_search("to_delete") == []
    finally:
        s.close()
