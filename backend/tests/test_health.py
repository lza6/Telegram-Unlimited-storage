"""Health / readiness / config / metrics endpoint tests."""

from __future__ import annotations


def test_health_live(client):
    r = client.get("/health/live")
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "alive"
    assert "uptime_secs" in body


def test_api_health_always_200(client):
    r = client.get("/api/v1/health")
    assert r.status_code == 200
    body = r.json()
    assert body["version"] == "1.0.0-python"
    assert body["ready"] is False  # Telegram not configured


def test_health_ready_503_when_not_connected(client):
    r = client.get("/health/ready")
    assert r.status_code == 503


def test_config_endpoint(client):
    r = client.get("/config")
    assert r.status_code == 200
    body = r.json()
    assert "chunk_size_mb" in body
    assert body["api_version"] == "1.0.0-python"


def test_metrics_endpoint(client):
    r = client.get("/metrics")
    assert r.status_code == 200
    assert "telegram_drive_uptime_seconds" in r.text


def test_request_id_header(client):
    r = client.get("/api/v1/health")
    assert "x-request-id" in r.headers


def test_security_headers_present(client):
    r = client.get("/api/v1/health")
    assert r.headers.get("x-content-type-options") == "nosniff"
    assert r.headers.get("x-frame-options") == "DENY"
    assert "Content-Security-Policy" in r.headers
