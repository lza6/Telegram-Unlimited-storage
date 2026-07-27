"""TASK-P1-02: 分享链接治理 — 验收测试.

验证：
1. 批量撤销（bulk-revoke）
2. 按文件撤销（revoke-by-file）
3. 访问统计（access_count / unique_visitors）
4. 下载限流（429 + Retry-After）
"""

from __future__ import annotations

import time
from unittest.mock import patch

from .conftest import ACCESS_PWD

AUTH = {"X-Access-Pwd": ACCESS_PWD}


def _create_share(client, **overrides):
    body = {"message_id": 42, "file_name": "demo.txt", "file_size": 100}
    body.update(overrides)
    return client.post("/api/v1/shares", json=body, headers=AUTH)


def test_bulk_revoke_shares(client):
    created = [_create_share(client).json() for _ in range(3)]
    ids = [c["id"] for c in created]

    r = client.post("/api/v1/shares/bulk-revoke", json={"share_ids": ids}, headers=AUTH)
    assert r.status_code == 200
    data = r.json()
    assert data["revoked"] == 3
    assert data["requested"] == 3

    # Verify all are revoked
    listing = client.get("/api/v1/shares", headers=AUTH).json()
    active_ids = [s["id"] for s in listing]
    for sid in ids:
        assert sid not in active_ids


def test_bulk_revoke_validates_input(client):
    r = client.post("/api/v1/shares/bulk-revoke", json={"share_ids": []}, headers=AUTH)
    assert r.status_code == 400

    r = client.post("/api/v1/shares/bulk-revoke", json={}, headers=AUTH)
    assert r.status_code == 400


def test_revoke_by_file(client):
    # Create shares for the same file_id (message_id)
    _create_share(client, message_id=99).json()
    _create_share(client, message_id=99).json()
    _create_share(client, message_id=100).json()

    r = client.post("/api/v1/shares/revoke-by-file", json={"file_id": 99}, headers=AUTH)
    assert r.status_code == 200
    data = r.json()
    assert data["revoked"] == 2
    assert data["file_id"] == 99


def test_revoke_by_file_validates_input(client):
    r = client.post("/api/v1/shares/revoke-by-file", json={}, headers=AUTH)
    assert r.status_code == 400
    r = client.post("/api/v1/shares/revoke-by-file", json={"file_id": "abc"}, headers=AUTH)
    assert r.status_code == 400


def test_share_info_includes_access_stats(client):
    created = _create_share(client).json()
    # access_count should start at 0
    assert created["access_count"] == 0
    assert created["unique_visitors_count"] == 0


def test_share_download_rate_limit(client, settings):
    """Per-share rate limit triggers 429 after SHARE_DOWNLOAD_RPM requests."""
    created = _create_share(client).json()
    token = created["id"]

    # Override to a small limit for testing
    settings.share_download_rpm = 2

    # Make requests up to limit
    responses = []
    for _ in range(3):
        r = client.get(f"/d/{token}")
        responses.append(r.status_code)

    # Should have at least one 429 (rate limited)
    # NOTE: this depends on whether the password form is shown (no password here)
    # For a no-password share, _stream_target is called; in test env Telegram not connected
    # so it returns 500. But rate limit is checked before streaming.
    # 2 allowed, 3rd should be 429
    assert 429 in responses, f"Expected 429 in responses: {responses}"


def test_bulk_revoke_audits(client):
    created = [_create_share(client).json() for _ in range(2)]
    ids = [c["id"] for c in created]

    r = client.post("/api/v1/shares/bulk-revoke", json={"share_ids": ids}, headers=AUTH)
    assert r.status_code == 200

    # Check audit log
    r = client.get("/api/v1/admin/audit?event=share.revoke", headers=AUTH)
    assert r.status_code == 200
    entries = r.json()
    # At least one bulk revoke entry
    assert any(
        e.get("metadata", {}).get("bulk") is True
        for e in entries
    )


def test_share_download_records_access(client):
    """Download attempts should increment access_count (even on failure)."""
    created = _create_share(client).json()
    token = created["id"]

    # Attempt download (will fail with 500 since no Telegram, but access should be recorded)
    client.get(f"/d/{token}")

    # Check that access_count was incremented
    listing = client.get("/api/v1/shares", headers=AUTH).json()
    match = [s for s in listing if s["id"] == token]
    if match:
        assert match[0]["access_count"] >= 1


def test_unique_visitors_capped_at_100(client):
    """Ensure visitor list doesn't grow unboundedly."""
    created = _create_share(client).json()
    token = created["id"]

    # Simulate many unique visitors by patching client IP
    for i in range(150):
        # Use X-Forwarded-For to vary IP
        client.get(f"/d/{token}", headers={"X-Forwarded-For": f"10.0.0.{i}"})

    # Check storage layer directly
    from app.main import create_app
    share = client.get("/api/v1/shares", headers=AUTH).json()
    # Should still be in listing since not revoked
    match = [s for s in share if s["id"] == token]
    if match:
        # unique_visitors_count should be capped at 100
        assert match[0]["unique_visitors_count"] <= 100
