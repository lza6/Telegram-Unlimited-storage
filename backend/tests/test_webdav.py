"""WebDAV protocol tests — auth gating, PROPFIND, PUT/DELETE, MKCOL, OPTIONS."""

from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from app.config import Settings
from app.main import create_app

AUTH = {"X-Access-Pwd": "testpwd"}


@pytest.fixture
def client(tmp_path, monkeypatch):
    """App with WebDAV enabled but no Telegram transport configured."""
    monkeypatch.setenv("DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setenv("WEBDAV_ENABLED", "true")
    monkeypatch.setenv("ACCESS_PWD", "testpwd")
    monkeypatch.setenv("API_KEY", "testapikey1234567890123456789012")
    monkeypatch.setenv("DOWNLOAD_SIGNING_SECRET", "x" * 40)
    for key in ("TELEGRAM_API_ID", "TELEGRAM_API_HASH", "TG_BOT_TOKEN", "TG_STORAGE_CHANNEL_ID"):
        monkeypatch.delenv(key, raising=False)
    settings = Settings(_env_file=None)
    app = create_app(settings)
    with TestClient(app) as c:
        yield c
        # 测试间隔离：清空爆破锁定计数器，避免跨用例污染。
        c.app.state.app.authenticator.guard._attempts.clear()


@pytest.fixture
def client_disabled(tmp_path, monkeypatch):
    """App with WebDAV disabled."""
    monkeypatch.setenv("DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setenv("WEBDAV_ENABLED", "false")
    monkeypatch.setenv("ACCESS_PWD", "testpwd")
    for key in ("TELEGRAM_API_ID", "TELEGRAM_API_HASH", "TG_BOT_TOKEN", "TG_STORAGE_CHANNEL_ID"):
        monkeypatch.delenv(key, raising=False)
    settings = Settings(_env_file=None)
    app = create_app(settings)
    with TestClient(app) as c:
        yield c


class TestWebDAVDisabled:
    def test_propfind_returns_404_when_disabled(self, client_disabled):
        resp = client_disabled.request("PROPFIND", "/webdav", headers=AUTH)
        assert resp.status_code == 404

    def test_get_returns_404_when_disabled(self, client_disabled):
        resp = client_disabled.get("/webdav", headers=AUTH)
        assert resp.status_code == 404


class TestWebDAVAuth:
    def test_propfind_requires_auth(self, client):
        resp = client.request("PROPFIND", "/webdav")
        assert resp.status_code == 401
        assert "WWW-Authenticate" in resp.headers

    def test_propfind_with_header_auth(self, client):
        resp = client.request("PROPFIND", "/webdav", headers=AUTH)
        assert resp.status_code == 207

    def test_propfind_with_basic_auth(self, client):
        import base64

        token = base64.b64encode(b"user:testpwd").decode()
        resp = client.request(
            "PROPFIND", "/webdav", headers={"Authorization": f"Basic {token}"}
        )
        assert resp.status_code == 207

    def test_propfind_with_wrong_basic_auth(self, client):
        import base64

        token = base64.b64encode(b"user:wrongpwd").decode()
        resp = client.request(
            "PROPFIND", "/webdav", headers={"Authorization": f"Basic {token}"}
        )
        assert resp.status_code == 401


class TestWebDAVOptions:
    def test_options_advertises_dav(self, client):
        resp = client.options("/webdav", headers=AUTH)
        assert resp.status_code == 200
        assert resp.headers.get("DAV") == "1"
        assert "PROPFIND" in resp.headers.get("Allow", "")

    def test_options_requires_auth(self, client):
        # OPTIONS is deliberately open so WebDAV clients can discover capabilities
        # before authenticating.
        resp = client.options("/webdav")
        assert resp.status_code == 200
        assert resp.headers.get("DAV") == "1"


class TestWebDAVPropfind:
    def test_propfind_root_returns_multistatus(self, client):
        resp = client.request("PROPFIND", "/webdav", headers=AUTH)
        assert resp.status_code == 207
        assert "multistatus" in resp.text
        assert "Telegram Drive" in resp.text

    def test_propfind_depth_0(self, client):
        resp = client.request(
            "PROPFIND", "/webdav", headers={**AUTH, "Depth": "0"}
        )
        assert resp.status_code == 207
        assert "collection" in resp.text

    def test_propfind_missing_file_depth_0(self, client):
        resp = client.request(
            "PROPFIND", "/webdav/nonexistent.txt", headers={**AUTH, "Depth": "0"}
        )
        assert resp.status_code == 207

    def test_propfind_rejects_traversal(self, client):
        # HTTP layer normalizes ".." before routing, so test _safe_segments directly.
        # ".." segments are dropped (not resolved), preventing parent traversal.
        from app.routers.webdav import _safe_segments

        assert _safe_segments("/webdav/../../etc/passwd") == ["etc", "passwd"]
        assert _safe_segments("/webdav/foo/../../../etc") == ["foo", "etc"]
        assert _safe_segments("/webdav/normal/file.txt") == ["normal", "file.txt"]
        assert _safe_segments("/webdav/") == []


class TestWebDAVGet:
    def test_get_root_returns_200(self, client):
        resp = client.get("/webdav", headers=AUTH)
        assert resp.status_code == 200

    def test_get_missing_file_404(self, client):
        resp = client.get("/webdav/missing.txt", headers=AUTH)
        assert resp.status_code == 404


class TestWebDAVMkcol:
    def test_mkcol_root_rejected(self, client):
        resp = client.request("MKCOL", "/webdav", headers=AUTH)
        assert resp.status_code == 405

    def test_mkcol_requires_auth(self, client):
        resp = client.request("MKCOL", "/webdav/newfolder")
        assert resp.status_code == 401


class TestWebDAVPut:
    def test_put_root_rejected(self, client):
        resp = client.put("/webdav", headers=AUTH, content=b"data")
        assert resp.status_code == 405

    def test_put_requires_auth(self, client):
        resp = client.put("/webdav/file.txt", content=b"data")
        assert resp.status_code == 401


class TestWebDAVDelete:
    def test_delete_root_rejected(self, client):
        resp = client.delete("/webdav", headers=AUTH)
        assert resp.status_code == 405

    def test_delete_missing_file_404(self, client):
        resp = client.delete("/webdav/missing.txt", headers=AUTH)
        assert resp.status_code == 404

    def test_delete_requires_auth(self, client):
        resp = client.delete("/webdav/file.txt")
        assert resp.status_code == 401


class TestWebDAVBruteForce:
    def test_brute_force_lockout(self, client):
        # 锁定阈值 ACCESS_LOCKOUT_MAX=8，每个 Basic 错误请求记录 1 次失败，
        # 前 8 个请求返回 401（第 8 次在认证中达到阈值），第 9 个请求触发 429。
        wrong_auth = {"Authorization": "Basic d3Jvbmc6d3Jvbmc="}
        for _ in range(8):
            resp = client.get("/webdav/", headers=wrong_auth)
            assert resp.status_code == 401

        # 9th request — already locked out (429)
        resp = client.get("/webdav/", headers=wrong_auth)
        assert resp.status_code == 429

    def test_brute_force_failure_count(self, client):
        # 每个 Basic 错误请求应累计 1 次失败（verify_access_pwd 内统一记录）。
        wrong_auth = {"Authorization": "Basic d3Jvbmc6d3Jvbmc="}
        attempts = client.app.state.app.authenticator.guard._attempts
        client.get("/webdav/", headers=wrong_auth)
        assert sum(len(v) for v in attempts.values()) == 1

