"""Shared pytest fixtures.

Each test gets an isolated temporary ``DATA_DIR`` and fixed credentials. The
``settings`` fixture constructs ``Settings(_env_file=None)`` so the repo's real
``.env`` (which holds live Telegram credentials) is never loaded during tests.
"""

from __future__ import annotations

import pytest

ACCESS_PWD = "testpwd"
API_KEY = "test-api-key-123"


@pytest.fixture()
def env(tmp_path, monkeypatch):
    """Isolated DATA_DIR + credentials; no Telegram configured."""
    monkeypatch.setenv("DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setenv("ACCESS_PWD", ACCESS_PWD)
    monkeypatch.setenv("API_KEY", API_KEY)
    monkeypatch.setenv("BASE_URL", "http://localhost:1334")
    monkeypatch.setenv("DOWNLOAD_SIGNING_SECRET", "s" * 40)
    for key in (
        "TELEGRAM_API_ID",
        "TELEGRAM_API_HASH",
        "TG_BOT_TOKEN",
        "TG_STORAGE_CHANNEL_ID",
        "PROXY_SOCKS5",
    ):
        monkeypatch.delenv(key, raising=False)
    return tmp_path


@pytest.fixture()
def settings(env):
    from app.config import Settings

    return Settings(_env_file=None)


@pytest.fixture()
def client(settings):
    from fastapi.testclient import TestClient

    from app.main import create_app

    app = create_app(settings)
    with TestClient(app) as c:
        yield c
