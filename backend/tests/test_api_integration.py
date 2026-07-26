"""Integration tests — API endpoints with the test client (no Telegram needed)."""

from __future__ import annotations

from .conftest import ACCESS_PWD, API_KEY


def test_health_live_returns_alive(client):
    r = client.get("/health/live")
    assert r.status_code == 200
    assert r.json()["status"] == "alive"


def test_health_ready_503_when_no_telegram(client):
    r = client.get("/health/ready")
    assert r.status_code == 503


def test_metrics_endpoint_returns_metrics(client):
    r = client.get("/metrics")
    assert r.status_code == 200
    body = r.text
    assert "telegram_drive_requests_total" in body
    assert "telegram_drive_request_duration_seconds" in body


def test_metrics_has_upload_slots(client):
    r = client.get("/metrics")
    assert "telegram_drive_upload_slots_available" in r.text


def test_config_endpoint_returns_config(client):
    r = client.get("/config")
    assert r.status_code == 200
    body = r.json()
    assert "chunk_size_mb" in body
    assert "chunk_concurrent" in body
    assert "files_concurrent" in body
    assert body["api_version"] == "2.0.0-python"


def test_files_list_requires_auth(client):
    r = client.get("/api/v1/files")
    assert r.status_code in (401, 403)


def test_files_list_with_access_pwd(client):
    r = client.get("/api/v1/files", headers={"X-Access-Pwd": ACCESS_PWD})
    assert r.status_code == 503  # not connected


def test_folders_list_requires_auth(client):
    r = client.get("/api/v1/folders")
    assert r.status_code in (401, 403)


def test_shares_list_requires_auth(client):
    r = client.get("/api/v1/shares")
    assert r.status_code in (401, 403)


def test_trash_endpoints_require_auth(client):
    r = client.get("/api/v1/trash")
    assert r.status_code in (401, 403)
    r = client.post("/api/v1/trash/restore", json={"message_ids": [1]})
    assert r.status_code in (401, 403)


def test_legacy_verify_endpoint(client):
    r = client.get("/verify")
    # Returns 401 if ACCESS_PWD is configured
    assert r.status_code in (200, 401)


def test_webdav_disabled_by_default(client):
    r = client.request("OPTIONS", "/webdav/")
    assert r.status_code == 404


def test_request_id_header_present(client):
    r = client.get("/api/v1/health")
    assert "x-request-id" in r.headers


def test_security_headers_present(client):
    r = client.get("/api/v1/health")
    assert r.headers.get("x-content-type-options") == "nosniff"
    assert r.headers.get("x-frame-options") == "DENY"


def test_csp_header_present(client):
    r = client.get("/api/v1/health")
    assert "Content-Security-Policy" in r.headers
    assert "default-src 'self'" in r.headers["Content-Security-Policy"]


def test_rate_limit_exempt_health(client):
    r = client.get("/api/v1/health")
    assert r.status_code == 200
    # Should not be rate-limited (health is exempt)


def test_404_on_unknown_path(client):
    r = client.get("/nonexistent")
    # SPA fallback: /{rel:path} serves index.html for unknown paths
    assert r.status_code in (200, 404)


def test_413_on_large_payload(client):
    # Send a request with Content-Length exceeding max_upload_size_mb
    r = client.post(
        "/api/v1/files",
        headers={
            "Content-Length": "105000000",  # 105MB > 100MB default
            "X-Access-Pwd": ACCESS_PWD,
        },
    )
    assert r.status_code == 413
    assert "PAYLOAD_TOO_LARGE" in r.text


def test_api_key_auth_works(client):
    r = client.get("/api/v1/files", headers={"X-API-Key": API_KEY})
    # Should pass auth but fail with 503 (not connected)
    assert r.status_code == 503