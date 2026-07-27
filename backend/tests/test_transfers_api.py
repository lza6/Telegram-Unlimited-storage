"""TASK-U-02: 传输中心 API — 验收测试.

验证：
1. GET /api/v1/transfers 返回当前传输列表
2. POST /api/v1/transfers/{id}/cancel 取消传输
3. POST /api/v1/transfers/{id}/retry 重试失败传输
4. POST /api/v1/transfers/{id}/pause 暂停传输
5. SSE /api/v1/transfers/events 推送状态变更
"""

from __future__ import annotations

from .conftest import ACCESS_PWD

AUTH = {"X-Access-Pwd": ACCESS_PWD}


def test_list_transfers_requires_auth(client):
    r = client.get("/api/v1/transfers")
    assert r.status_code in (401, 403)


def test_list_transfers_empty(client):
    r = client.get("/api/v1/transfers", headers=AUTH)
    assert r.status_code == 200
    data = r.json()
    assert "transfers" in data
    assert isinstance(data["transfers"], list)
    # No active transfers initially
    assert len(data["transfers"]) == 0


def test_cancel_transfer_unknown_404(client):
    r = client.post("/api/v1/transfers/nonexistent-session/cancel", headers=AUTH)
    assert r.status_code == 404


def test_retry_transfer_unknown_404(client):
    r = client.post("/api/v1/transfers/nonexistent-session/retry", headers=AUTH)
    assert r.status_code == 404


def test_pause_transfer_unknown_404(client):
    r = client.post("/api/v1/transfers/nonexistent-session/pause", headers=AUTH)
    assert r.status_code == 404


def test_cancel_existing_transfer(client):
    # Seed a progress state via TransferManager
    from app.main import create_app
    app = client.app
    state = app.state.app
    state.transfers.ensure_progress("sess-test", "test.txt", 4)
    state.transfers.update_progress("sess-test", status="running", uploaded_chunks=1)

    r = client.post("/api/v1/transfers/sess-test/cancel", headers=AUTH)
    assert r.status_code == 200
    assert r.json()["status"] == "cancelled"

    # Verify in list
    r = client.get("/api/v1/transfers", headers=AUTH)
    transfers = r.json()["transfers"]
    match = [t for t in transfers if t["session_id"] == "sess-test"]
    assert match
    assert match[0]["status"] == "cancelled"


def test_retry_failed_transfer(client):
    app = client.app
    state = app.state.app
    state.transfers.ensure_progress("sess-fail", "fail.txt", 2)
    state.transfers.update_progress("sess-fail", status="failed")

    r = client.post("/api/v1/transfers/sess-fail/retry", headers=AUTH)
    assert r.status_code == 200
    assert r.json()["status"] == "queued"


def test_retry_non_retryable_returns_404(client):
    app = client.app
    state = app.state.app
    state.transfers.ensure_progress("sess-running", "run.txt", 2)
    state.transfers.update_progress("sess-running", status="running")

    # Running transfer cannot be retried
    r = client.post("/api/v1/transfers/sess-running/retry", headers=AUTH)
    assert r.status_code == 404
