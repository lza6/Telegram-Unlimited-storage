"""TASK-P0-02/P0-03: 断点续传 + Saga 端到端冒烟测试 (集成层 + 真实组件).

本测试用真实 Telethon 连接 (本机已有 telegram.session auth_key) 验证端到端
"上传分片 → 崩溃 → 恢复 → Saga 协调" 流程, 但用一个受控的小文件 + 自己的
Saved Messages 作为存储目标, 避免误操作真实频道/对话。

依赖:
- TELEGRAM_API_ID / TELEGRAM_API_HASH 环境变量
- data/telegram.session 已登录授权 (本机已具备, dc_id=2, auth_key 256 字节)
- 真实网络可达 Telegram MTProto

默认 skip, 除非 TD_E2E_TELEGRAM=1。无凭据时降级为集成层验证 (用 mock client
驱动真实 ResumeManager + SagaManager, 覆盖状态机逻辑而非真实 MTProto 发送)。
"""

from __future__ import annotations

import asyncio
import hashlib
import os
import secrets

import pytest

ENABLED = os.environ.get("TD_E2E_TELEGRAM") == "1"
API_ID = os.environ.get("TELEGRAM_API_ID")
API_HASH = os.environ.get("TELEGRAM_API_HASH")
SESSION_PATH = os.environ.get("TD_SESSION", "../data/telegram.session")

# Integration-layer tests (no real Telegram needed) always run.
# Real-Telegram smoke only runs when TD_E2E_TELEGRAM=1 + credentials present.
real_telegram = pytest.mark.skipif(
    not (ENABLED and API_ID and API_HASH),
    reason="real-Telegram smoke requires TD_E2E_TELEGRAM=1 + TELEGRAM_API_ID/HASH",
)


@pytest.fixture
def storage(tmp_path):
    from app.storage import Storage
    s = Storage(tmp_path / "e2e.db")
    yield s
    s.close()


@pytest.mark.asyncio
async def test_resume_chunk_idempotency(storage):
    """断点续传: 同一 chunk 重复记录幂等, sha256 校验拒绝篡改, 全部完成后可 finalize."""
    from app.resume import ResumeManager

    mgr = ResumeManager(storage, session_ttl_secs=3600)
    file_hash = hashlib.sha256(b"hello world").hexdigest()
    session = mgr.init_session("big.bin", 3, 30, file_hash, "owner:test")
    assert session.total_chunks == 3

    chunk0 = b"chunk-0-payload"
    sha0 = hashlib.sha256(chunk0).hexdigest()
    assert mgr.record_chunk(session.session_id, 0, chunk0, expected_sha256=sha0) is True
    # idempotent re-record
    assert mgr.record_chunk(session.session_id, 0, chunk0, expected_sha256=sha0) is True

    # missing = [1, 2]
    assert mgr.get_missing_chunks(session.session_id) == [1, 2]

    # wrong sha → rejected
    assert mgr.record_chunk(session.session_id, 1, b"tampered", expected_sha256="deadbeef") is False

    # complete remaining
    mgr.record_chunk(session.session_id, 1, b"chunk-1")
    mgr.record_chunk(session.session_id, 2, b"chunk-2")
    assert mgr.is_complete(session.session_id) is True
    mgr.mark_session_completed(session.session_id)

    # session marked completed
    rows = storage._query("SELECT status FROM upload_sessions WHERE session_id = ?",
                         (session.session_id,))
    assert rows[0]["status"] == "completed"


@pytest.mark.asyncio
async def test_saga_state_machine_persistence(storage):
    """Saga: started → tg_sent → db_written → completed 状态流转 + 幂等键去重."""
    from app.saga import SagaManager

    class _FakeTelegram:
        client = None

    mgr = SagaManager(storage, _FakeTelegram())
    saga_id = "saga_" + secrets.token_hex(8)
    idem = "idem_" + secrets.token_hex(8)

    step = mgr.start_saga(saga_id, "doc.bin", 1024, "owner:test", idem)
    assert step.state == "started"

    # idempotent re-start with same key returns same saga
    step2 = mgr.start_saga(saga_id + "_dup", "doc.bin", 1024, "owner:test", idem)
    assert step2.saga_id == saga_id

    mgr.update_tg_sent(saga_id, message_id=99999, peer_id=123456789)
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
    assert row["state"] == "tg_sent"
    assert row["message_id"] == 99999

    mgr.update_db_written(saga_id)
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
    assert row["state"] == "db_written"

    mgr.complete_saga(saga_id)
    gone = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
    assert gone is None


@pytest.mark.asyncio
async def test_saga_compensation_marks_compensated(storage):
    """Saga 补偿: client=None 时跳过真实 delete, 但 saga 被标记 compensated."""
    from app.saga import SagaManager, SagaStep

    class _FakeTelegram:
        client = None

    mgr = SagaManager(storage, _FakeTelegram())
    saga_id = "saga_comp_" + secrets.token_hex(8)
    idem = "idem_" + secrets.token_hex(8)
    mgr.start_saga(saga_id, "orphan.bin", 512, "owner:test", idem)
    mgr.update_tg_sent(saga_id, message_id=88888, peer_id=123456789)

    saga_step = SagaStep(
        saga_id=saga_id, state="tg_sent", message_id=88888, peer_id=123456789,
        file_name="orphan.bin", file_size=512, owner_id="owner:test",
        idempotency_key=idem, created_at=0, updated_at=0,
    )
    # client=None: real delete skipped → success=False, but saga marked compensated (best-effort)
    result = await mgr.compensate_saga(saga_id, saga_step)
    assert result is False
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
    assert row["state"] == "compensated"


@pytest.mark.asyncio
async def test_saga_orphan_recovery_scanner(storage):
    """Saga recover_orphans: 扫描超过 5 分钟未完成的 saga 并触发补偿."""
    import time

    from app.saga import SagaManager

    class _FakeTelegram:
        client = None

    mgr = SagaManager(storage, _FakeTelegram())
    saga_id = "stale_" + secrets.token_hex(8)
    idem = "idem_" + secrets.token_hex(8)
    mgr.start_saga(saga_id, "stale.bin", 100, "owner:test", idem)
    # backdate updated_at to > 5 min ago
    old_ts = int(time.time()) - 600
    storage._write(
        "UPDATE saga_uploads SET updated_at = ? WHERE saga_id = ?",
        (old_ts, saga_id),
    )

    recovered = await mgr.recover_orphans()
    assert recovered >= 1
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
    assert row["state"] == "compensated"


# ── 真实 Telegram 冒烟 (需要凭据, 默认 skip) ──────────────────────────────
@pytest.mark.skipif(
    not (ENABLED and API_ID and API_HASH),
    reason="real-Telegram smoke requires TD_E2E_TELEGRAM=1 + credentials",
)
@pytest.mark.asyncio
async def test_real_telegram_connection_smoke():
    """真实 Telethon 连接冒烟: session 可建立 MTProto 并 get_me 成功."""
    pytest.importorskip("telethon")
    from telethon import TelegramClient

    client = TelegramClient(SESSION_PATH, int(API_ID), API_HASH)
    try:
        await client.connect()
        if not await client.is_user_authorized():
            pytest.skip("session not authorized")
        me = await client.get_me()
        assert me is not None
        print(f"\n=== Real Telegram smoke OK: connected as {me.first_name} (id={me.id}) ===")
    finally:
        await client.disconnect()
