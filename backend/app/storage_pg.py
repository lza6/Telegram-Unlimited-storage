"""PostgreSQL backend for the control-plane mode (TASK-P1-04 step 2).

A Protocol-compatible Storage backend backed by asyncpg. Mirrors the
minimum schema needed for multi-tenant control-plane deployments.

Used behind a feature flag (DATABASE_URL) so SQLite stays the default.
"""

from __future__ import annotations

import logging
import time
from collections.abc import Iterable
from typing import Any

logger = logging.getLogger("telegram_drive.storage_pg")

# Schema mirroring the SQLite _SCHEMA (subset needed for control-plane).
_PG_SCHEMA = """
CREATE TABLE IF NOT EXISTS shared_links (
    id TEXT PRIMARY KEY,
    folder_id INTEGER,
    message_id BIGINT NOT NULL,
    file_name TEXT NOT NULL,
    file_size BIGINT NOT NULL DEFAULT 0,
    password_hash TEXT,
    password_salt TEXT,
    expires_at BIGINT,
    revoked INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    owner_id TEXT,
    access_count BIGINT NOT NULL DEFAULT 0,
    last_accessed_at BIGINT,
    unique_visitors TEXT
);
CREATE TABLE IF NOT EXISTS tenants (
    tenant_id TEXT PRIMARY KEY,
    api_key_hash TEXT NOT NULL,
    display_name TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    scopes TEXT,
    created_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS tenant_quotas (
    tenant_id TEXT PRIMARY KEY,
    storage_bytes_limit BIGINT NOT NULL DEFAULT 0,
    storage_bytes_used BIGINT NOT NULL DEFAULT 0,
    files_count_limit BIGINT NOT NULL DEFAULT 0,
    files_count_used BIGINT NOT NULL DEFAULT 0,
    updated_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS file_assets (
    message_id BIGINT PRIMARY KEY,
    folder_id BIGINT,
    owner_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    deleted_at BIGINT
);
CREATE TABLE IF NOT EXISTS saga_uploads (
    saga_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    message_id BIGINT,
    peer_id BIGINT,
    file_name TEXT,
    file_size BIGINT,
    owner_id TEXT,
    idempotency_key TEXT UNIQUE,
    created_at BIGINT,
    updated_at BIGINT
);
CREATE TABLE IF NOT EXISTS upload_sessions (
    session_id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    total_chunks INTEGER NOT NULL,
    total_size BIGINT NOT NULL DEFAULT 0,
    file_hash TEXT NOT NULL DEFAULT '',
    owner_id TEXT NOT NULL DEFAULT 'default',
    status TEXT NOT NULL DEFAULT 'active',
    manifest_file_id TEXT,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"""


def _now() -> int:
    return int(time.time())


class PostgresBackend:
    """Async PostgreSQL storage backend (asyncpg).

    Note: callers use async/await; the synchronous Storage._write/_query
    interface is NOT used. PG backend exposes async methods.
    """

    def __init__(self, dsn: str) -> None:
        try:
            import asyncpg  # noqa: F401
        except ImportError as exc:
            raise RuntimeError(
                "asyncpg not installed — run `pip install asyncpg` for PG mode"
            ) from exc
        self._dsn = dsn
        self._pool = None

    async def connect(self) -> None:
        import asyncpg
        self._pool = await asyncpg.create_pool(dsn=self._dsn, min_size=1, max_size=8)
        async with self._pool.acquire() as conn:
            await conn.execute(_PG_SCHEMA)
        logger.info("PostgreSQL backend connected: %s", self._dsn.split("@")[-1] if "@" in self._dsn else "pg")

    async def close(self) -> None:
        if self._pool:
            await self._pool.close()
            self._pool = None

    # ── async query helpers ───────────────────────────────────────────────────
    async def fetch(self, sql: str, params: Iterable[Any] = ()) -> list[dict[str, Any]]:
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(sql, *params)
            return [dict(r) for r in rows]

    async def fetchrow(self, sql: str, params: Iterable[Any] = ()) -> dict[str, Any] | None:
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(sql, *params)
            return dict(row) if row else None

    async def execute(self, sql: str, params: Iterable[Any] = ()) -> str:
        async with self._pool.acquire() as conn:
            return await conn.execute(sql, *params)

    # ── shared_links ──────────────────────────────────────────────────────────
    async def create_share(
        self, share_id: str, message_id: int, file_name: str, file_size: int,
        owner_id: str | None = None, password_hash: str | None = None,
        expires_at: int | None = None, folder_id: int | None = None,
    ) -> dict[str, Any]:
        now = _now()
        await self.execute(
            "INSERT INTO shared_links (id, folder_id, message_id, file_name, file_size, "
            "password_hash, expires_at, revoked, created_at, owner_id) "
            "VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8, $9)",
            (share_id, folder_id, message_id, file_name, file_size, password_hash,
             expires_at, now, owner_id),
        )
        return await self.get_share(share_id)

    async def get_share(self, share_id: str) -> dict[str, Any] | None:
        return await self.fetchrow("SELECT * FROM shared_links WHERE id = $1", (share_id,))

    async def list_shares(self, owner_id: str | None = None) -> list[dict[str, Any]]:
        if owner_id:
            return await self.fetch(
                "SELECT * FROM shared_links WHERE owner_id = $1 ORDER BY created_at DESC",
                (owner_id,),
            )
        return await self.fetch("SELECT * FROM shared_links ORDER BY created_at DESC")

    async def revoke_share(self, share_id: str) -> str:
        return await self.execute(
            "UPDATE shared_links SET revoked = 1 WHERE id = $1", (share_id,)
        )

    # ── tenants ───────────────────────────────────────────────────────────────
    async def upsert_tenant(
        self, tenant_id: str, api_key_hash: str, display_name: str | None
    ) -> None:
        await self.execute(
            "INSERT INTO tenants (tenant_id, api_key_hash, display_name, enabled, created_at) "
            "VALUES ($1, $2, $3, 1, $4) "
            "ON CONFLICT (tenant_id) DO UPDATE SET "
            "api_key_hash = EXCLUDED.api_key_hash, "
            "display_name = COALESCE(EXCLUDED.display_name, tenants.display_name)",
            (tenant_id, api_key_hash, display_name, _now()),
        )

    async def list_tenants(self) -> list[dict[str, Any]]:
        return await self.fetch("SELECT * FROM tenants ORDER BY created_at ASC")

    async def get_tenant_scopes(self, tenant_id: str) -> list[str]:
        row = await self.fetchrow(
            "SELECT scopes FROM tenants WHERE tenant_id = $1 AND enabled = 1",
            (tenant_id,),
        )
        if not row or not row.get("scopes"):
            return []
        import json
        try:
            return json.loads(row["scopes"]) or []
        except Exception:
            return []

    # ── tenant_quotas ─────────────────────────────────────────────────────────
    async def get_tenant_quota(self, tenant_id: str) -> dict[str, Any] | None:
        return await self.fetchrow(
            "SELECT * FROM tenant_quotas WHERE tenant_id = $1", (tenant_id,)
        )

    async def upsert_tenant_quota(
        self, tenant_id: str, storage_bytes_limit: int, files_count_limit: int,
        storage_bytes_used: int = 0, files_count_used: int = 0,
    ) -> None:
        await self.execute(
            "INSERT INTO tenant_quotas (tenant_id, storage_bytes_limit, storage_bytes_used, "
            "files_count_limit, files_count_used, updated_at) "
            "VALUES ($1, $2, $3, $4, $5, $6) "
            "ON CONFLICT (tenant_id) DO UPDATE SET "
            "storage_bytes_limit = EXCLUDED.storage_bytes_limit, "
            "files_count_limit = EXCLUDED.files_count_limit, "
            "updated_at = EXCLUDED.updated_at",
            (tenant_id, storage_bytes_limit, storage_bytes_used,
             files_count_limit, files_count_used, _now()),
        )

    async def recompute_tenant_quota(self, tenant_id: str) -> dict[str, int]:
        rows = await self.fetch(
            "SELECT COALESCE(SUM(file_size), 0) AS total_bytes, COUNT(*) AS total_files "
            "FROM file_assets WHERE owner_id = $1 AND deleted_at IS NULL",
            (tenant_id,),
        )
        r = rows[0] if rows else {"total_bytes": 0, "total_files": 0}
        usage = {
            "storage_bytes_used": int(r.get("total_bytes") or 0),
            "files_count_used": int(r.get("total_files") or 0),
        }
        await self.execute(
            "UPDATE tenant_quotas SET storage_bytes_used = $1, files_count_used = $2, updated_at = $3 "
            "WHERE tenant_id = $4",
            (usage["storage_bytes_used"], usage["files_count_used"], _now(), tenant_id),
        )
        return usage

    # ── saga_uploads ──────────────────────────────────────────────────────────
    async def start_saga(
        self, saga_id: str, file_name: str, file_size: int, owner_id: str,
        idempotency_key: str,
    ) -> dict[str, Any]:
        existing = await self.fetchrow(
            "SELECT * FROM saga_uploads WHERE idempotency_key = $1", (idempotency_key,)
        )
        if existing:
            return existing
        now = _now()
        await self.execute(
            "INSERT INTO saga_uploads (saga_id, state, message_id, peer_id, file_name, "
            "file_size, owner_id, idempotency_key, created_at, updated_at) "
            "VALUES ($1, 'started', NULL, NULL, $2, $3, $4, $5, $6, $6)",
            (saga_id, file_name, file_size, owner_id, idempotency_key, now),
        )
        return await self.fetchrow("SELECT * FROM saga_uploads WHERE saga_id = $1", (saga_id,))

    async def update_saga_tg_sent(self, saga_id: str, message_id: int, peer_id: int) -> None:
        await self.execute(
            "UPDATE saga_uploads SET state = 'tg_sent', message_id = $1, peer_id = $2, "
            "updated_at = $3 WHERE saga_id = $4",
            (message_id, peer_id, _now(), saga_id),
        )

    async def complete_saga(self, saga_id: str) -> None:
        await self.execute(
            "UPDATE saga_uploads SET state = 'completed', updated_at = $1 WHERE saga_id = $2",
            (_now(), saga_id),
        )
        await self.execute("DELETE FROM saga_uploads WHERE saga_id = $1", (saga_id,))

    async def list_stale_sagas(self, threshold: int) -> list[dict[str, Any]]:
        return await self.fetch(
            "SELECT * FROM saga_uploads WHERE state IN ('started', 'tg_sent', 'compensating') "
            "AND updated_at < $1",
            (threshold,),
        )

    async def mark_saga_compensated(self, saga_id: str) -> None:
        await self.execute(
            "UPDATE saga_uploads SET state = 'compensated', updated_at = $1 WHERE saga_id = $2",
            (_now(), saga_id),
        )

    # ── app_meta ──────────────────────────────────────────────────────────────
    async def get_meta(self, key: str) -> str | None:
        row = await self.fetchrow("SELECT value FROM app_meta WHERE key = $1", (key,))
        return row["value"] if row else None

    async def set_meta(self, key: str, value: str) -> None:
        await self.execute(
            "INSERT INTO app_meta (key, value) VALUES ($1, $2) "
            "ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            (key, value),
        )
