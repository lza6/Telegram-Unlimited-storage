"""Tests for the /files/{id}/thumb SVG placeholder endpoint (TASK-P1-02)."""

from __future__ import annotations

from unittest.mock import AsyncMock, patch

AUTH = {"X-Access-Pwd": "testpwd"}


def _fake_target(name: str = "report.pdf", mime: str = "application/pdf"):
    return type("T", (), {"filename": name, "size": 100, "mime_type": mime, "stream": iter([b"x"])})()


def test_thumb_returns_svg(client):
    """Thumbnail endpoint returns an SVG image."""
    with patch(
        "app.routers.files.resolve_download",
        new_callable=AsyncMock,
        return_value=_fake_target(),
    ):
        with patch("app.routers.files._require_connected", new_callable=AsyncMock):
            r = client.get("/api/v1/files/100/thumb", headers=AUTH)
    assert r.status_code == 200
    assert r.headers["content-type"].startswith("image/svg+xml")
    assert "<svg" in r.text
    assert "R" in r.text  # first letter of "report.pdf"


def test_thumb_colours_by_category(client):
    """Different file categories yield different SVG background colours."""
    with patch("app.routers.files._require_connected", new_callable=AsyncMock):
        with patch(
            "app.routers.files.resolve_download",
            new_callable=AsyncMock,
            side_effect=lambda s, fid, mid, fn, **kw: _fake_target("photo.jpg", "image/jpeg"),
        ):
            r_img = client.get("/api/v1/files/1/thumb", headers=AUTH)
        with patch(
            "app.routers.files.resolve_download",
            new_callable=AsyncMock,
            side_effect=lambda s, fid, mid, fn, **kw: _fake_target("song.mp3", "audio/mpeg"),
        ):
            r_aud = client.get("/api/v1/files/2/thumb", headers=AUTH)
    assert r_img.status_code == 200
    assert r_aud.status_code == 200
    assert r_img.text != r_aud.text  # different colours


def test_thumb_requires_auth(client):
    """No auth header → 401."""
    r = client.get("/api/v1/files/100/thumb")
    assert r.status_code == 401
