"""Share CRUD + public download gating tests."""

from __future__ import annotations

from .conftest import ACCESS_PWD

AUTH = {"X-Access-Pwd": ACCESS_PWD}


def _create_share(client, **overrides):
    body = {"message_id": 42, "file_name": "demo.txt", "file_size": 100}
    body.update(overrides)
    return client.post("/api/v1/shares", json=body, headers=AUTH)


def test_create_share(client):
    r = _create_share(client)
    assert r.status_code == 200
    data = r.json()
    assert data["file_name"] == "demo.txt"
    assert data["link"].endswith(f"/d/{data['id']}")
    assert data["has_password"] is False


def test_create_share_validates_message_id(client):
    r = client.post(
        "/api/v1/shares", json={"message_id": 0, "file_name": "x"}, headers=AUTH
    )
    assert r.status_code == 400


def test_list_shares_excludes_revoked(client):
    created = _create_share(client).json()
    client.delete(f"/api/v1/shares/{created['id']}", headers=AUTH)
    listing = client.get("/api/v1/shares", headers=AUTH).json()
    assert all(s["id"] != created["id"] for s in listing)


def test_delete_share_revokes(client):
    created = _create_share(client).json()
    r = client.delete(f"/api/v1/shares/{created['id']}", headers=AUTH)
    assert r.status_code == 200
    assert r.json()["revoked"] is True


def test_download_unknown_token_404(client):
    r = client.get("/d/doesnotexist")
    assert r.status_code == 404
    assert r.text == "Shared link not found"


def test_download_revoked_share_404(client):
    created = _create_share(client).json()
    client.delete(f"/api/v1/shares/{created['id']}", headers=AUTH)
    r = client.get(f"/d/{created['id']}")
    assert r.status_code == 404
    assert "revoked" in r.text


def test_password_share_shows_form(client):
    created = _create_share(client, password="s3cret").json()
    assert created["has_password"] is True
    r = client.get(f"/d/{created['id']}")
    assert r.status_code == 200
    assert "Password Protected File" in r.text


def test_password_verify_wrong_then_correct(client):
    created = _create_share(client, password="s3cret").json()
    token = created["id"]
    wrong = client.post(f"/d/{token}/verify", data={"password": "nope"})
    assert "Incorrect password" in wrong.text
    ok = client.post(
        f"/d/{token}/verify", data={"password": "s3cret"}, follow_redirects=False
    )
    assert ok.status_code == 302
    assert ok.headers["location"] == f"/d/{token}"
    assert f"share_auth_{token}" in ok.headers.get("set-cookie", "")


def test_verify_no_password_share_400(client):
    created = _create_share(client).json()
    r = client.post(f"/d/{created['id']}/verify", data={"password": "x"})
    assert r.status_code == 400


def test_signed_download_disabled_without_secret(client, settings):
    # DOWNLOAD_SIGNING_SECRET is 40 chars in the fixture, so presign is enabled;
    # an invalid signature must be rejected.
    r = client.get("/d/signed?file_id=1&sig=bad")
    assert r.status_code == 403


def test_stream_requires_token(client):
    r = client.get("/stream/me/1")
    assert r.status_code == 403
