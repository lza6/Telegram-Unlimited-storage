"""Unified storage backend abstraction (TASK-P0-01, v8.0).

Defines a :class:`StorageBackend` Protocol that both the synchronous SQLite
``Storage`` class and the asynchronous ``PostgresBackend`` (storage_pg.py)
satisfy. This is the single surface every router should depend on so the
storage implementation can be swapped via ``DATABASE_URL`` without touching
call sites.

Design constraints (from plans/v8-迭代升级指南/下一步改进指南.md):
  - Do NOT rewrite storage.py. ``Storage`` already satisfies this Protocol via
    duck typing (structural typing) — no inheritance required.
  - Do NOT change router call sites (they already do ``state.storage.<method>``).
  - Add a :func:`create_storage_backend` factory that picks the implementation
    based on ``Settings.database_url``.

The Protocol deliberately mirrors the *existing* method signatures of
``Storage`` so it is a drop-in contract, not a new API. New v8 methods
(etag, idempotency, transfer_events) are added to ``Storage`` and declared
here as optional members (default-implemented as no-ops in the Protocol).
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Protocol, runtime_checkable

from .config import Settings


@runtime_checkable
class StorageBackend(Protocol):
    """Structural contract for storage backends.

    ``Storage`` (SQLite, sync) and ``PostgresBackend`` (asyncpg, async) both
    satisfy this contract via duck typing. Methods are grouped by domain.

    Backends are NOT required to be async — the synchronous ``Storage`` runs
    plain ``def`` methods and FastAPI routes that touch it use ``def`` (so
    they run in the threadpool) or wrap via ``asyncio.to_thread``. The
    async ``PostgresBackend`` exposes ``async def`` counterparts; call sites
    detect asynchronicity via ``inspect.iscoroutinefunction`` (see
    ``state.py`` dispatcher) so the same call site works for both.
    """

    # ── lifecycle ──────────────────────────────────────────────────────────
    def close(self) -> None: ...

    # ── app_meta ────────────────────────────────────────────────────────────
    def get_meta(self, key: str) -> str | None: ...
    def set_meta(self, key: str, value: str) -> None: ...

    # ── shared_links ───────────────────────────────────────────────────────
    def create_share(
        self,
        share_id: str,
        folder_id: int | None,
        message_id: int,
        file_name: str,
        file_size: int,
        password_hash: str | None,
        password_salt: str | None,
        expires_at: int | None,
        owner_id: str | None,
    ) -> dict[str, Any]: ...
    def get_share(self, share_id: str) -> dict[str, Any] | None: ...
    def list_shares(self, owner_id: str | None = None) -> list[dict[str, Any]]: ...
    def revoke_share(self, share_id: str) -> int: ...
    def bulk_revoke_shares(self, share_ids: list[str]) -> int: ...
    def revoke_shares_by_file(self, message_id: int) -> int: ...
    def record_share_access(self, share_id: str, visitor_ip_hash: str) -> None: ...
    def delete_share(self, share_id: str) -> int: ...
    def cleanup_expired_shares(self) -> int: ...

    # ── upload sessions / chunks ────────────────────────────────────────────
    def create_upload_session(
        self,
        session_id: str,
        filename: str,
        total_chunks: int,
        expires_at: int,
    ) -> None: ...
    def get_upload_session(self, session_id: str) -> dict[str, Any] | None: ...
    def update_upload_session_status(
        self, session_id: str, status: str
    ) -> None: ...
    def record_upload_chunk(
        self, session_id: str, chunk_index: int, sha256: str
    ) -> None: ...
    def get_upload_chunk(
        self, session_id: str, chunk_index: int
    ) -> dict[str, Any] | None: ...
    def list_upload_chunks(self, session_id: str) -> list[dict[str, Any]]: ...

    # ── tenants / quotas ───────────────────────────────────────────────────
    def upsert_tenant(
        self,
        tenant_id: str,
        api_key_hash: str,
        display_name: str | None,
    ) -> None: ...
    def list_tenants(self) -> list[dict[str, Any]]: ...
    def get_tenant_scopes(self, tenant_id: str) -> list[str]: ...
    def get_tenant_quota(self, tenant_id: str) -> dict[str, Any] | None: ...
    def upsert_tenant_quota(
        self,
        tenant_id: str,
        storage_bytes_limit: int,
        files_count_limit: int,
        storage_bytes_used: int = 0,
        files_count_used: int = 0,
    ) -> None: ...

    # ── file_assets (v8 ETag — optional, default no-op for backends without it)
    def get_file_etag(self, message_id: int) -> str | None:
        """Return cached ETag for a file asset, or None if not computed."""
        return None


def create_storage_backend(settings: Settings) -> StorageBackend:
    """Factory: pick the storage backend based on ``Settings.database_url``.

    - ``database_url`` empty / ``sqlite``  → ``Storage`` (SQLite WAL, sync)
    - ``database_url`` starts with ``postgresql://`` → ``PostgresBackend`` (async)

    The SQLite path is the default and never breaks — it is the existing
    behaviour. PostgreSQL is opt-in via ``DATABASE_URL``.
    """
    db_url = (settings.database_url or "").strip()
    if db_url.startswith("postgresql://") or db_url.startswith("postgres://"):
        # Lazy import so asyncpg is only required when PG is actually used.
        from .storage_pg import PostgresBackend  # type: ignore[attr-defined]

        return PostgresBackend(db_url)  # type: ignore[return-value]
    # Default: SQLite. Resolve the db path from settings.
    from .storage import Storage

    return Storage(Path(settings.db_path))


__all__ = ["StorageBackend", "create_storage_backend"]
