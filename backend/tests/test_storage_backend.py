"""TDD tests for StorageBackend abstraction (TASK-P0-01, v8.0).

Verifies:
  1. ``Storage`` (SQLite) satisfies the ``StorageBackend`` Protocol (duck typing).
  2. ``PostgresBackend`` has the required async methods.
  3. ``create_storage_backend`` picks SQLite by default and PG when
     ``database_url`` is a postgres URL.
  4. v8 storage methods: get/set_file_etag, idempotency store round-trip.
"""

from __future__ import annotations

import inspect
import json

import pytest

from app.config import Settings, get_settings
from app.storage import Storage
from app.storage_backend import StorageBackend, create_storage_backend


def _fresh_settings(**env_overrides) -> Settings:
    """Build a Settings instance from current env (must be set via monkeypatch)."""
    get_settings.cache_clear()
    return Settings(_env_file=None, **env_overrides)


# ── Protocol conformance ────────────────────────────────────────────────────
def test_storage_satisfies_storage_backend_protocol(env) -> None:
    """SQLite Storage must structurally satisfy StorageBackend (duck typing)."""
    settings = _fresh_settings()
    storage = Storage(settings.db_path)
    try:
        assert isinstance(storage, StorageBackend), (
            "Storage must satisfy StorageBackend Protocol via duck typing"
        )
    finally:
        storage.close()


def test_storage_core_methods_present(env) -> None:
    """All Protocol methods that Storage claims to implement must exist."""
    settings = _fresh_settings()
    storage = Storage(settings.db_path)
    required = [
        "close", "get_meta", "set_meta",
        "create_share", "get_share", "list_shares",
        "revoke_share", "bulk_revoke_shares", "revoke_shares_by_file",
        "record_share_access", "delete_share", "cleanup_expired_shares",
        "create_upload_session", "get_upload_session",
        "upsert_tenant", "list_tenants", "get_tenant_scopes",
        "get_tenant_quota", "upsert_tenant_quota",
        "get_file_etag", "set_file_etag",
        "get_idempotency", "set_idempotency_processing", "set_idempotency_complete",
    ]
    try:
        for name in required:
            assert hasattr(storage, name), f"Storage missing required method: {name}"
    finally:
        storage.close()


def test_postgres_backend_has_async_counterparts() -> None:
    """PostgresBackend must expose async versions of the Protocol methods."""
    from app.storage_pg import PostgresBackend

    required_async = [
        "get_meta", "set_meta",
        "create_share", "get_share", "list_shares",
        "revoke_share", "upsert_tenant", "list_tenants",
        "get_tenant_scopes", "get_tenant_quota", "upsert_tenant_quota",
    ]
    for name in required_async:
        assert hasattr(PostgresBackend, name), (
            f"PostgresBackend missing required method: {name}"
        )
        method = getattr(PostgresBackend, name)
        assert inspect.iscoroutinefunction(method), (
            f"PostgresBackend.{name} must be async"
        )


# ── Factory ─────────────────────────────────────────────────────────────────
def test_factory_defaults_to_sqlite(env) -> None:
    """No DATABASE_URL → SQLite Storage (default path)."""
    settings = _fresh_settings()
    backend = create_storage_backend(settings)
    try:
        assert isinstance(backend, Storage), (
            "Empty DATABASE_URL must select SQLite Storage"
        )
        # db file lives under the isolated DATA_DIR
        assert "shares.db" in str(backend._db_path)
    finally:
        backend.close()


def test_factory_selects_postgres(env, monkeypatch) -> None:
    """postgresql:// DATABASE_URL → PostgresBackend (no live connection needed)."""
    monkeypatch.setenv("DATABASE_URL", "postgresql://user:pass@localhost:5432/db")
    settings = _fresh_settings()
    backend = create_storage_backend(settings)
    from app.storage_pg import PostgresBackend
    assert isinstance(backend, PostgresBackend), (
        "postgresql:// DATABASE_URL must select PostgresBackend"
    )
    # Do NOT call connect() — no live PG in test env. Just verify construction.


def test_factory_postgres_alias(env, monkeypatch) -> None:
    """postgres:// scheme alias also selects PostgresBackend."""
    monkeypatch.setenv("DATABASE_URL", "postgres://user:pass@localhost:5432/db")
    settings = _fresh_settings()
    from app.storage_pg import PostgresBackend
    backend = create_storage_backend(settings)
    assert isinstance(backend, PostgresBackend)


# ── Behavioral parity: SQLite backend works end-to-end ─────────────────────
def test_sqlite_backend_create_and_get_share(env) -> None:
    """Factory-produced SQLite backend handles a full create→get cycle."""
    settings = _fresh_settings()
    backend = create_storage_backend(settings)
    try:
        created = backend.create_share(
            share_id="share-1",
            folder_id=None,
            message_id=100,
            file_name="test.txt",
            file_size=42,
            password_hash=None,
            password_salt=None,
            expires_at=None,
            owner_id=None,
        )
        assert created["id"] == "share-1"
        assert created["file_name"] == "test.txt"

        fetched = backend.get_share("share-1")
        assert fetched is not None
        assert fetched["file_size"] == 42
    finally:
        backend.close()


# ── v8 ETag cache ────────────────────────────────────────────────────────────
def test_etag_default_returns_none(env) -> None:
    """get_file_etag returns None before any ETag is computed, then round-trips."""
    settings = _fresh_settings()
    backend = create_storage_backend(settings)
    try:
        assert backend.get_file_etag(999) is None
        backend.set_file_etag(999, "W/abc123")
        assert backend.get_file_etag(999) == "W/abc123"
    finally:
        backend.close()


# ── v8 idempotency key store ────────────────────────────────────────────────
def test_idempotency_store_round_trip(env) -> None:
    """Idempotency key store: processing → complete → replay returns cached."""
    settings = _fresh_settings()
    backend = create_storage_backend(settings)
    try:
        assert backend.get_idempotency("k1") is None
        backend.set_idempotency_processing("k1")
        assert backend.get_idempotency("k1") == "__PROCESSING__"
        backend.set_idempotency_complete("k1", {"status": "ok", "id": 42})
        cached = json.loads(backend.get_idempotency("k1"))
        assert cached == {"status": "ok", "id": 42}
    finally:
        backend.close()
