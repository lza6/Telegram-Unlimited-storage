"""TASK-P1-04 step 2: PostgreSQL 控制面模式 — 验收测试.

使用本机已运行的 PostgreSQL 16 服务（td_test_pg 数据库）验证
PG backend 的 CRUD、配额、Saga 等核心流程。

环境依赖：本机 PostgreSQL 16 已启动，td_test_pg 数据库已创建。
"""

from __future__ import annotations

import asyncio
import os
import secrets

import pytest

pg_dsn = os.environ.get(
    "PG_TEST_DSN", "postgresql://postgres:postgres@localhost:5432/td_test_pg"
)

pytestmark = pytest.mark.asyncio


@pytest.fixture
async def pg():
    """Spin up a fresh PostgresBackend for each test."""
    pytest.importorskip("asyncpg")
    from app.storage_pg import PostgresBackend
    backend = PostgresBackend(pg_dsn)
    try:
        await backend.connect()
    except Exception as exc:
        pytest.skip(f"PostgreSQL not reachable: {exc}")
    # Clean tables before each test
    async with backend._pool.acquire() as conn:
        await conn.execute("TRUNCATE shared_links, tenants, tenant_quotas, file_assets, saga_uploads, app_meta")
    yield backend
    await backend.close()


async def test_pg_create_and_get_share(pg):
    share_id = "sh_" + secrets.token_hex(4)
    created = await pg.create_share(
        share_id=share_id, message_id=1001, file_name="test.bin", file_size=2048,
        owner_id="tenant:t1",
    )
    assert created["id"] == share_id
    assert created["file_name"] == "test.bin"
    assert created["file_size"] == 2048
    assert created["owner_id"] == "tenant:t1"

    fetched = await pg.get_share(share_id)
    assert fetched is not None
    assert fetched["file_name"] == "test.bin"


async def test_pg_revoke_share(pg):
    share_id = "sh_" + secrets.token_hex(4)
    await pg.create_share(share_id=share_id, message_id=1002, file_name="x.txt", file_size=10)
    await pg.revoke_share(share_id)
    fetched = await pg.get_share(share_id)
    assert fetched["revoked"] == 1


async def test_pg_tenant_crud(pg):
    tid = "tenant_" + secrets.token_hex(4)
    await pg.upsert_tenant(tid, "hash_placeholder", "Display Name")
    scopes = await pg.get_tenant_scopes(tid)
    assert scopes == []  # no scopes = full access

    # Update scopes via raw execute
    import json
    await pg.execute(
        "UPDATE tenants SET scopes = $1 WHERE tenant_id = $2",
        (json.dumps(["read", "write"]), tid),
    )
    scopes2 = await pg.get_tenant_scopes(tid)
    assert scopes2 == ["read", "write"]


async def test_pg_quota_lifecycle(pg):
    tid = "q_" + secrets.token_hex(4)
    await pg.upsert_tenant(tid, "hash", "Q Tenant")
    await pg.upsert_tenant_quota(tid, storage_bytes_limit=10000, files_count_limit=5)

    quota = await pg.get_tenant_quota(tid)
    assert quota["storage_bytes_limit"] == 10000
    assert quota["files_count_limit"] == 5

    # Recompute from empty file_assets
    usage = await pg.recompute_tenant_quota(tid)
    assert usage["storage_bytes_used"] == 0
    assert usage["files_count_used"] == 0


async def test_pg_saga_state_machine(pg):
    saga_id = "sg_" + secrets.token_hex(4)
    idem = "idem_" + secrets.token_hex(4)

    step = await pg.start_saga(saga_id, "big.bin", 9999, "owner:t1", idem)
    assert step["state"] == "started"

    # Idempotent re-call returns same saga
    step2 = await pg.start_saga(saga_id, "big.bin", 9999, "owner:t1", idem)
    assert step2["saga_id"] == saga_id

    # tg_sent transition
    await pg.update_saga_tg_sent(saga_id, message_id=5555, peer_id=1234)
    step3 = await pg.fetchrow("SELECT * FROM saga_uploads WHERE saga_id = $1", (saga_id,))
    assert step3["state"] == "tg_sent"
    assert step3["message_id"] == 5555

    # complete → row deleted
    await pg.complete_saga(saga_id)
    gone = await pg.fetchrow("SELECT * FROM saga_uploads WHERE saga_id = $1", (saga_id,))
    assert gone is None


async def test_pg_saga_stale_detection(pg):
    saga_id = "sg_" + secrets.token_hex(4)
    idem = "idem_" + secrets.token_hex(4)
    await pg.start_saga(saga_id, "orphan.bin", 100, "owner:x", idem)

    # Mark it stale by backdating updated_at
    import time
    old_ts = int(time.time()) - 600  # 10 min ago
    await pg.execute(
        "UPDATE saga_uploads SET updated_at = $1 WHERE saga_id = $2",
        (old_ts, saga_id),
    )

    stale = await pg.list_stale_sagas(int(time.time()) - 300)
    assert any(s["saga_id"] == saga_id for s in stale)

    # Compensate
    await pg.mark_saga_compensated(saga_id)
    stale2 = await pg.list_stale_sagas(int(time.time()) - 300)
    assert not any(s["saga_id"] == saga_id for s in stale2)


async def test_pg_app_meta_kv(pg):
    await pg.set_meta("schema_version", "7.0.0-python")
    val = await pg.get_meta("schema_version")
    assert val == "7.0.0-python"

    # Update existing
    await pg.set_meta("schema_version", "7.0.1-python")
    val2 = await pg.get_meta("schema_version")
    assert val2 == "7.0.1-python"
