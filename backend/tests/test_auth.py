"""Authentication tests — X-Access-Pwd and X-API-Key headers."""

from __future__ import annotations

from .conftest import ACCESS_PWD, API_KEY


def test_no_auth_rejected(client):
    r = client.get("/api/v1/shares")
    assert r.status_code == 401


def test_access_pwd_accepted(client):
    r = client.get("/api/v1/shares", headers={"X-Access-Pwd": ACCESS_PWD})
    assert r.status_code == 200


def test_wrong_pwd_rejected(client):
    r = client.get("/api/v1/shares", headers={"X-Access-Pwd": "wrong"})
    assert r.status_code == 401


def test_api_key_accepted(client):
    r = client.get("/api/v1/settings", headers={"X-API-Key": API_KEY})
    assert r.status_code == 200


def test_wrong_api_key_rejected(client):
    r = client.get("/api/v1/settings", headers={"X-API-Key": "bad"})
    assert r.status_code == 401


def test_lockout_after_repeated_failures(client):
    for _ in range(8):
        client.get("/api/v1/shares", headers={"X-Access-Pwd": "wrong"})
    # 9th attempt should be locked out (429) even with correct password.
    r = client.get("/api/v1/shares", headers={"X-Access-Pwd": ACCESS_PWD})
    assert r.status_code == 429
