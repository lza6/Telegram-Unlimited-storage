"""SQLite storage layer.

Reuses the exact schema of the legacy Rust backend (``data/shares.db``) so
existing deployments keep working with no migration. All queries are
parameterized. The class is synchronous (sqlite3); FastAPI routes that touch
it should be plain ``def`` so they run in the threadpool, or call via
``asyncio.to_thread``.
"""

from __future__ import annotations

import json
import sqlite3
import threading
import time
from pathlib import Path
from typing import Any, Iterable, Optional

_SCHEMA = """
CREATE TABLE IF NOT EXISTS shared_links (
    id TEXT PRIMARY KEY,
    folder_id INTEGER,
    message_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    password_hash TEXT,
    password_salt TEXT,
    expires_at INTEGER,
    revoked INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    owner_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_shares_expires ON shared_links(expires_at);
CREATE INDEX IF NOT EXISTS idx_shares_revoked ON shared_links(revoked, created_at);
CREATE INDEX IF NOT EXISTS idx_shares_owner ON shared_links(owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS upload_sessions (
    session_id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    total_chunks INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    manifest_file_id TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS upload_chunks (
    session_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    file_id TEXT,
    sha256 TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, chunk_index),
    FOREIGN KEY (session_id) REFERENCES upload_sessions(session_id)
);
CREATE INDEX IF NOT EXISTS idx_upload_session ON upload_chunks(session_id);

CREATE TABLE IF NOT EXISTS bot_file_map (
    message_id INTEGER PRIMARY KEY,
    telegram_file_id TEXT NOT NULL,
    file_name TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    caption TEXT,
    created_at INTEGER NOT NULL,
    bot_pool_index INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_bot_file_created ON bot_file_map(created_at DESC);

CREATE TABLE IF NOT EXISTS metadata_cache (
    cache_key TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_metadata_cache_kind ON metadata_cache(kind, updated_at);

CREATE TABLE IF NOT EXISTS tenants (
    tenant_id TEXT PRIMARY KEY,
    api_key_hash TEXT NOT NULL,
    display_name TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS file_assets (
    message_id INTEGER PRIMARY KEY,
    folder_id INTEGER,
    owner_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_file_assets_owner ON file_assets(owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS asset_locators (
    asset_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    transport_mode TEXT NOT NULL CHECK (transport_mode IN ('bot','user')),
    storage_peer_id INTEGER NOT NULL,
    storage_peer_kind TEXT NOT NULL,
    message_id INTEGER NOT NULL,
    legacy_folder_id INTEGER,
    telegram_file_id TEXT,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    bot_pool_index INTEGER,
    uploader_bot_id TEXT,
    locator_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (locator_state IN ('ready','deleted','reconcile')),
    locator_version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(owner_id, transport_mode, storage_peer_id, storage_peer_kind, message_id)
);
CREATE INDEX IF NOT EXISTS idx_asset_locators_message_owner
    ON asset_locators(message_id, owner_id, legacy_folder_id);
CREATE INDEX IF NOT EXISTS idx_asset_locators_peer_message
    ON asset_locators(storage_peer_id, storage_peer_kind, message_id);

CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"""


def _now() -> int:
    return int(time.time())


class Storage:
    """Thread-safe SQLite access over the legacy shares.db schema."""

    def __init__(self, db_path: Path) -> None:
        self._db_path = Path(db_path)
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.RLock()
        self._conn = sqlite3.connect(
            str(self._db_path), check_same_thread=False
        )
        self._conn.row_factory = sqlite3.Row
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA foreign_keys=ON")
        with self._lock:
            self._conn.executescript(_SCHEMA)
            self._conn.commit()

    def close(self) -> None:
        with self._lock:
            self._conn.close()

    # ── low-level helpers ───────────────────────────────────────────────────
    def _query(self, sql: str, params: Iterable[Any] = ()) -> list[dict[str, Any]]:
        with self._lock:
            cur = self._conn.execute(sql, tuple(params))
            return [dict(row) for row in cur.fetchall()]

    def _query_one(self, sql: str, params: Iterable[Any] = ()) -> Optional[dict[str, Any]]:
        rows = self._query(sql, params)
        return rows[0] if rows else None

    def _execute(self, sql: str, params: Iterable[Any] = ()) -> int:
        with self._lock:
            cur = self._conn.execute(sql, tuple(params))
            self._conn.commit()
            return cur.rowcount

    # ── app_meta ────────────────────────────────────────────────────────────
    def get_meta(self, key: str) -> Optional[str]:
        row = self._query_one("SELECT value FROM app_meta WHERE key = ?", (key,))
        return row["value"] if row else None

    def set_meta(self, key: str, value: str) -> None:
        self._execute(
            "INSERT INTO app_meta (key, value) VALUES (?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )

    # ── shared_links ────────────────────────────────────────────────────────
    def create_share(
        self,
        share_id: str,
        folder_id: Optional[int],
        message_id: int,
        file_name: str,
        file_size: int,
        password_hash: Optional[str],
        password_salt: Optional[str],
        expires_at: Optional[int],
        owner_id: Optional[str],
    ) -> dict[str, Any]:
        self._execute(
            "INSERT INTO shared_links (id, folder_id, message_id, file_name, "
            "file_size, password_hash, password_salt, expires_at, revoked, "
            "created_at, owner_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
            (
                share_id,
                folder_id,
                message_id,
                file_name,
                file_size,
                password_hash,
                password_salt,
                expires_at,
                _now(),
                owner_id,
            ),
        )
        return self.get_share(share_id)  # type: ignore[return-value]

    def get_share(self, share_id: str) -> Optional[dict[str, Any]]:
        return self._query_one("SELECT * FROM shared_links WHERE id = ?", (share_id,))

    def list_shares(self, owner_id: Optional[str] = None) -> list[dict[str, Any]]:
        if owner_id is not None:
            return self._query(
                "SELECT * FROM shared_links WHERE owner_id = ? "
                "ORDER BY created_at DESC",
                (owner_id,),
            )
        return self._query("SELECT * FROM shared_links ORDER BY created_at DESC")

    def revoke_share(self, share_id: str) -> int:
        return self._execute(
            "UPDATE shared_links SET revoked = 1 WHERE id = ?", (share_id,)
        )

    def delete_share(self, share_id: str) -> int:
        return self._execute("DELETE FROM shared_links WHERE id = ?", (share_id,))

    def cleanup_expired_shares(self) -> int:
        """Lazy prune: mark expired shares revoked (Rust cleanup_expired)."""
        return self._execute(
            "UPDATE shared_links SET revoked = 1 "
            "WHERE revoked = 0 AND expires_at IS NOT NULL AND expires_at < ?",
            (_now(),),
        )

    # ── upload sessions / chunks ────────────────────────────────────────────
    def create_upload_session(
        self,
        session_id: str,
        filename: str,
        total_chunks: int,
        expires_at: int,
    ) -> None:
        """Idempotent (INSERT OR IGNORE) + pre-create chunk rows (Rust parity).

        Pre-creating the ``pending`` chunk rows is load-bearing: ``merge_chunks``
        relies on every expected chunk being present so a missing upload is
        rejected instead of silently merging an incomplete file.
        """
        now = _now()
        with self._lock:
            self._conn.execute(
                "INSERT OR IGNORE INTO upload_sessions (session_id, filename, "
                "total_chunks, status, created_at, expires_at) "
                "VALUES (?, ?, ?, 'active', ?, ?)",
                (session_id, filename, total_chunks, now, expires_at),
            )
            for i in range(total_chunks):
                self._conn.execute(
                    "INSERT OR IGNORE INTO upload_chunks (session_id, "
                    "chunk_index, status, created_at) VALUES (?, ?, 'pending', ?)",
                    (session_id, i, now),
                )
            self._conn.commit()

    def get_upload_session(self, session_id: str) -> Optional[dict[str, Any]]:
        return self._query_one(
            "SELECT * FROM upload_sessions WHERE session_id = ?", (session_id,)
        )

    def update_upload_session_status(
        self, session_id: str, status: str, manifest_file_id: Optional[str] = None
    ) -> None:
        self._execute(
            "UPDATE upload_sessions SET status = ?, manifest_file_id = "
            "COALESCE(?, manifest_file_id) WHERE session_id = ?",
            (status, manifest_file_id, session_id),
        )

    def record_upload_chunk(
        self,
        session_id: str,
        chunk_index: int,
        file_id: Optional[str],
        sha256: Optional[str],
    ) -> None:
        self._execute(
            "INSERT INTO upload_chunks (session_id, chunk_index, file_id, "
            "sha256, status, created_at) VALUES (?, ?, ?, ?, 'uploaded', ?) "
            "ON CONFLICT(session_id, chunk_index) DO UPDATE SET "
            "file_id = excluded.file_id, sha256 = excluded.sha256, "
            "status = excluded.status",
            (session_id, chunk_index, file_id, sha256, _now()),
        )

    def get_upload_chunk(
        self, session_id: str, chunk_index: int
    ) -> Optional[dict[str, Any]]:
        return self._query_one(
            "SELECT * FROM upload_chunks WHERE session_id = ? AND chunk_index = ?",
            (session_id, chunk_index),
        )

    def list_upload_chunks(self, session_id: str) -> list[dict[str, Any]]:
        return self._query(
            "SELECT * FROM upload_chunks WHERE session_id = ? "
            "ORDER BY chunk_index ASC",
            (session_id,),
        )

    # ── bot_file_map ────────────────────────────────────────────────────────
    def record_bot_file(
        self,
        message_id: int,
        telegram_file_id: str,
        file_name: str,
        file_size: int,
        caption: Optional[str],
        bot_pool_index: int,
    ) -> None:
        self._execute(
            "INSERT INTO bot_file_map (message_id, telegram_file_id, file_name, "
            "file_size, caption, created_at, bot_pool_index) "
            "VALUES (?, ?, ?, ?, ?, ?, ?) "
            "ON CONFLICT(message_id) DO UPDATE SET "
            "telegram_file_id = excluded.telegram_file_id",
            (message_id, telegram_file_id, file_name, file_size, caption, _now(), bot_pool_index),
        )

    def get_bot_file(self, message_id: int) -> Optional[dict[str, Any]]:
        return self._query_one(
            "SELECT * FROM bot_file_map WHERE message_id = ?", (message_id,)
        )

    def list_bot_files(self, limit: int = 1000) -> list[dict[str, Any]]:
        """List all bot files from bot_file_map (bot mode has no folders)."""
        return self._query(
            "SELECT * FROM bot_file_map ORDER BY created_at DESC LIMIT ?",
            (limit,),
        )

    def search_bot_files(self, query: str, limit: int = 50) -> list[dict[str, Any]]:
        """Search bot files by name substring."""
        return self._query(
            "SELECT * FROM bot_file_map WHERE file_name LIKE ? "
            "ORDER BY created_at DESC LIMIT ?",
            (f"%{query}%", limit),
        )

    def delete_bot_file(self, message_id: int) -> bool:
        """Delete bot file from both bot_file_map and file_assets."""
        with self._lock:
            self._conn.execute(
                "DELETE FROM file_assets WHERE message_id = ?", (message_id,)
            )
            cursor = self._conn.execute(
                "DELETE FROM bot_file_map WHERE message_id = ?", (message_id,)
            )
            self._conn.commit()
            return cursor.rowcount > 0

    # ── metadata_cache ──────────────────────────────────────────────────────
    def cache_get(self, cache_key: str, ttl_secs: int) -> Optional[Any]:
        row = self._query_one(
            "SELECT payload, updated_at FROM metadata_cache WHERE cache_key = ?",
            (cache_key,),
        )
        if not row:
            return None
        if ttl_secs > 0 and (_now() - row["updated_at"]) > ttl_secs:
            return None
        try:
            return json.loads(row["payload"])
        except (ValueError, TypeError):
            return None

    def cache_set(self, cache_key: str, kind: str, payload: Any) -> None:
        self._execute(
            "INSERT INTO metadata_cache (cache_key, kind, payload, updated_at) "
            "VALUES (?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET "
            "kind = excluded.kind, payload = excluded.payload, "
            "updated_at = excluded.updated_at",
            (cache_key, kind, json.dumps(payload), _now()),
        )

    # ── tenants ─────────────────────────────────────────────────────────────
    def upsert_tenant(
        self,
        tenant_id: str,
        api_key_hash: str,
        display_name: Optional[str],
    ) -> None:
        self._execute(
            "INSERT INTO tenants (tenant_id, api_key_hash, display_name, "
            "enabled, created_at) VALUES (?, ?, ?, 1, ?) "
            "ON CONFLICT(tenant_id) DO UPDATE SET "
            "api_key_hash = excluded.api_key_hash, "
            "display_name = COALESCE(excluded.display_name, tenants.display_name)",
            (tenant_id, api_key_hash, display_name, _now()),
        )

    def list_tenants(self) -> list[dict[str, Any]]:
        return self._query("SELECT * FROM tenants ORDER BY created_at ASC")

    def get_enabled_tenant_by_hash(
        self, api_key_hash: str
    ) -> Optional[dict[str, Any]]:
        return self._query_one(
            "SELECT * FROM tenants WHERE api_key_hash = ? AND enabled = 1",
            (api_key_hash,),
        )

    # ── file_assets / asset_locators ────────────────────────────────────────
    def upsert_file_asset(
        self,
        message_id: int,
        folder_id: Optional[int],
        owner_id: str,
        file_name: str,
        file_size: int,
    ) -> None:
        self._execute(
            "INSERT INTO file_assets (message_id, folder_id, owner_id, "
            "file_name, file_size, created_at) VALUES (?, ?, ?, ?, ?, ?) "
            "ON CONFLICT(message_id) DO UPDATE SET "
            "folder_id = excluded.folder_id, file_name = excluded.file_name, "
            "file_size = excluded.file_size",
            (message_id, folder_id, owner_id, file_name, file_size, _now()),
        )

    def delete_owner_assets(self, owner_id: str) -> int:
        return self._execute(
            "DELETE FROM file_assets WHERE owner_id = ?", (owner_id,)
        )

    def search_file_assets(
        self, owner_id: str, query: str, limit: int = 50
    ) -> list[dict[str, Any]]:
        like = f"%{query}%"
        return self._query(
            "SELECT * FROM file_assets WHERE owner_id = ? AND file_name LIKE ? "
            "ORDER BY created_at DESC LIMIT ?",
            (owner_id, like, limit),
        )
