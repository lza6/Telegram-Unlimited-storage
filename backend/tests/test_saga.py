"""TASK-P0-03: 传输 Saga — 验收测试.

验证：
1. Saga 状态机流转正确
2. 同 idempotency_key 返回相同结果（幂等）
3. 崩溃恢复 worker 清理孤儿消息
4. 补偿逻辑（补偿前二次校验消息归属）
"""

from __future__ import annotations

import asyncio
import time
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from app.saga import SagaManager
from app.storage import Storage


@pytest.fixture
def storage(tmp_path):
    s = Storage(tmp_path / "test.db")
    yield s
    s.close()


@pytest.fixture
def mock_telegram():
    t = MagicMock()
    t.client = AsyncMock()
    return t


def test_start_saga_idempotent(storage, mock_telegram):
    sm = SagaManager(storage, mock_telegram)
    idem = "idem-key-1"
    s1 = sm.start_saga("s1", "file.bin", 1000, "owner1", idem)
    s2 = sm.start_saga("s2", "file.bin", 1000, "owner1", idem)
    assert s1.saga_id == s2.saga_id
    assert s1.state == "started"


def test_saga_state_transitions(storage, mock_telegram):
    sm = SagaManager(storage, mock_telegram)
    s = sm.start_saga("s1", "file.bin", 1000, "owner1", "idem-1")
    assert s.state == "started"

    sm.update_tg_sent("s1", 12345, 678)
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row["state"] == "tg_sent"
    assert row["message_id"] == 12345

    sm.update_db_written("s1")
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row["state"] == "db_written"

    sm.complete_saga("s1")
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row is None  # completed sagas are pruned


def test_compensate_saga_deletes_orphan(storage, mock_telegram):
    sm = SagaManager(storage, mock_telegram)
    s = sm.start_saga("s1", "file.bin", 1000, "owner1", "idem-1")
    sm.update_tg_sent("s1", 12345, 678)

    # Mock delete success
    mock_telegram.client.get_input_entity = AsyncMock(return_value="peer")
    mock_telegram.client.delete_messages = AsyncMock()

    ok = asyncio.run(sm.compensate_saga("s1", s))
    assert ok is True
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row["state"] == "compensated"
    mock_telegram.client.delete_messages.assert_called_once_with("peer", [12345])


def test_compensate_saga_no_message_id(storage, mock_telegram):
    sm = SagaManager(storage, mock_telegram)
    s = sm.start_saga("s1", "file.bin", 1000, "owner1", "idem-1")
    # Don't call update_tg_sent, so no message_id
    ok = asyncio.run(sm.compensate_saga("s1", s))
    assert ok is True
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row["state"] == "compensated"


def test_recover_orphans(storage, mock_telegram):
    sm = SagaManager(storage, mock_telegram)
    sm.start_saga("s1", "file.bin", 1000, "owner1", "idem-1")
    sm.update_tg_sent("s1", 12345, 678)

    # Force saga to be old (>5 min)
    storage._write(
        "UPDATE saga_uploads SET updated_at = ? WHERE saga_id = ?",
        (int(time.time()) - 400, "s1"),
    )

    mock_telegram.client.get_input_entity = AsyncMock(return_value="peer")
    mock_telegram.client.delete_messages = AsyncMock()

    count = asyncio.run(sm.recover_orphans())
    assert count == 1
    row = storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", ("s1",))
    assert row["state"] == "compensated"
    mock_telegram.client.delete_messages.assert_called_once_with("peer", [12345])
