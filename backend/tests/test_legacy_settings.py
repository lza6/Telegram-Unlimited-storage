"""Legacy endpoints (/verify, /upload_progress_token, /upload_status) + settings/network."""

from __future__ import annotations

from .conftest import ACCESS_PWD, API_KEY

PWD_AUTH = {"X-Access-Pwd": ACCESS_PWD}
KEY_AUTH = {"X-API-Key": API_KEY}


# ── legacy /verify ──────────────────────────────────────────────────────────
def test_verify_ok(client):
    r = client.post("/verify", data={"pwd": ACCESS_PWD})
    assert r.status_code == 200
    assert r.text == "ok"


def test_verify_wrong_pwd(client):
    r = client.post("/verify", data={"pwd": "wrong"})
    assert r.status_code == 401
    assert r.text == "密码错误"


# ── upload progress token ───────────────────────────────────────────────────
def test_progress_token_requires_access_pwd(client):
    # API key must be rejected (admin/console only).
    r = client.post(
        "/upload_progress_token", data={"session_id": "s1"}, headers=KEY_AUTH
    )
    assert r.status_code == 401


def test_progress_token_issued(client):
    r = client.post(
        "/upload_progress_token", json={"session_id": "s1"}, headers=PWD_AUTH
    )
    assert r.status_code == 200
    body = r.json()
    assert body["session_id"] == "s1"
    assert body["token"]
    assert body["expires_at"] > 0


def test_upload_status_requires_auth(client):
    r = client.get("/upload_status?session_id=s1")
    assert r.status_code == 401


# ── settings ────────────────────────────────────────────────────────────────
def test_get_settings(client):
    r = client.get("/api/v1/settings", headers=PWD_AUTH)
    assert r.status_code == 200
    body = r.json()
    assert "share_domain" in body
    assert body["chunk_size_mb"] > 0


def test_put_settings_share_domain(client):
    r = client.put(
        "/api/v1/settings",
        json={"share_domain": "https://cdn.example.com"},
        headers=PWD_AUTH,
    )
    assert r.status_code == 200
    assert r.json()["share_domain"] == "https://cdn.example.com"


# ── network ─────────────────────────────────────────────────────────────────
def test_get_network_redacts_password(client):
    r = client.get("/api/v1/network", headers=PWD_AUTH)
    assert r.status_code == 200
    body = r.json()
    assert body["proxy"]["password"] == ""


def test_put_network_rejects_empty_proxy_host(client):
    r = client.put(
        "/api/v1/network",
        json={"proxy": {"enabled": True, "host": ""}},
        headers=PWD_AUTH,
    )
    assert r.status_code == 400
    assert r.json()["error"]["code"] == "INVALID_CONFIG"


def test_put_network_merges_and_redacts(client):
    r = client.put(
        "/api/v1/network",
        json={"proxy": {"enabled": True, "host": "1.2.3.4", "port": 1080,
                        "password": "secret"}},
        headers=PWD_AUTH,
    )
    assert r.status_code == 200
    body = r.json()
    assert body["proxy"]["host"] == "1.2.3.4"
    assert body["proxy"]["password"] == ""
    assert body["proxy"]["password_set"] is True
