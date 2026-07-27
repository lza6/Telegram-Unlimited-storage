"""Integration test for ETag conditional requests on /download (TASK-P1-03)."""

from __future__ import annotations

from unittest.mock import AsyncMock, patch

from app.etag import compute_etag

AUTH = {"X-Access-Pwd": "testpwd"}


def test_download_returns_etag_header(client, monkeypatch):
    """First download: 200 + ETag header set + metric incremented."""
    from app.metrics import get_registry
    get_registry()  # ensure singleton
    before = get_registry().download_200_total._value.get()  # type: ignore[attr-defined]

    # Stub resolve_download to return a fake stream target.
    fake_target = type(
        "T",
        (),
        {
            "filename": "doc.txt",
            "size": 100,
            "mime_type": "text/plain",
            "stream": iter([b"hello"]),
        },
    )()
    with patch(
        "app.routers.files.resolve_download",
        new_callable=AsyncMock,
        return_value=fake_target,
    ):
        with patch("app.routers.files._require_connected", new_callable=AsyncMock):
            r = client.get("/api/v1/files/500/download", headers=AUTH)
    assert r.status_code == 200
    assert "etag" in {k.lower() for k in r.headers}
    # ETag persisted to storage; second call with If-None-Match → 304
    etag = r.headers["etag"]
    assert etag.startswith("W/")


def test_download_returns_304_on_match(client):
    """Second download with If-None-Match → 304 Not Modified."""
    fake_target = type(
        "T",
        (),
        {
            "filename": "doc.txt",
            "size": 100,
            "mime_type": "text/plain",
            "stream": iter([b"hello"]),
        },
    )()
    # Pre-seed the ETag so the conditional matches.
    from app.state import AppState
    state: AppState = client.app.state.app  # type: ignore[attr-defined]
    etag = compute_etag(501, 100, "doc.txt")
    state.storage.set_file_etag(501, etag)

    with patch(
        "app.routers.files.resolve_download",
        new_callable=AsyncMock,
        return_value=fake_target,
    ):
        with patch("app.routers.files._require_connected", new_callable=AsyncMock):
            r = client.get(
                "/api/v1/files/501/download",
                headers={**AUTH, "If-None-Match": etag},
            )
    assert r.status_code == 304
    assert r.headers["etag"] == etag
    assert r.content == b""


def test_download_returns_200_on_etag_mismatch(client):
    """Stale If-None-Match → 200 with body."""
    fake_target = type(
        "T",
        (),
        {
            "filename": "doc.txt",
            "size": 100,
            "mime_type": "text/plain",
            "stream": iter([b"data"]),
        },
    )()
    from app.state import AppState
    state: AppState = client.app.state.app  # type: ignore[attr-defined]
    real_etag = compute_etag(502, 100, "doc.txt")
    state.storage.set_file_etag(502, real_etag)

    with patch(
        "app.routers.files.resolve_download",
        new_callable=AsyncMock,
        return_value=fake_target,
    ):
        with patch("app.routers.files._require_connected", new_callable=AsyncMock):
            r = client.get(
                "/api/v1/files/502/download",
                headers={**AUTH, "If-None-Match": "W/stale-etag"},
            )
    assert r.status_code == 200
