"""TASK-P0-01: 版本号统一 + 构建元数据 — 验收测试.

验证：
1. 单一版本源 `app.__version__` 存在且为 8.0.0-python
2. state.version / FastAPI version 引用同一常量
3. health payload 含 build_date / python_version
4. config.py 中 download_signing_secrets 只定义一次（无重复字段）
"""

from __future__ import annotations

import inspect
import re


def test_single_version_source():
    import app

    assert app.__version__ == "8.0.0-python"
    meta = app.build_metadata()
    assert meta["version"] == app.__version__
    assert "build_date" in meta and meta["build_date"]
    assert re.match(r"\d+\.\d+\.\d+", meta["python_version"])


def test_state_version_uses_single_source():
    import app
    from app.state import AppState

    # state.version property must return the single-source constant
    prop = inspect.getattr_static(AppState, "version")
    assert isinstance(prop, property)
    src = inspect.getsource(prop.fget)
    assert "2.0.0-python" not in src
    # construct a minimal AppState to read the property
    import secrets

    class _S:
        version = None
        started_at = 0
        settings = None

    st = AppState.__new__(AppState)
    st.__dict__["started_at"] = 0
    assert AppState.version.fget(st) == app.__version__


def test_health_payload_contains_build_metadata():
    from app.routers.health import _health_payload

    class _Settings:
        data_dir = "."
        metadata_cache_enabled = False
        metadata_cache_ttl_secs = 0
        public_file_id_download = False
        upload_share_ttl_hours = 0
        download_signing_secret = ""
        multi_tenant_enabled = False

    class _Storage:
        def _query(self, sql, *a):
            return [(1,)]

    class _Transfers:
        def queue_status(self):
            return {}

    class _State:
        settings = _Settings()
        storage = _Storage()
        transfers = _Transfers()

        @property
        def version(self):
            return "8.0.0-python"

        @property
        def uptime_secs(self):
            return 1

        def effective_transport_mode(self):
            return "user"

        @property
        def bot_configured(self):
            return False

        @property
        def user_configured(self):
            return False

    payload = _health_payload(_State(), telegram_connected=False, ready=True)
    assert payload["version"] == "8.0.0-python"
    assert "build_date" in payload
    assert "python_version" in payload


def test_no_duplicate_signing_secrets_field():
    """config.py must define download_signing_secrets exactly once as a Field."""
    import app.config as cfg

    src = inspect.getsource(cfg)
    occurrences = src.count("download_signing_secrets: str = Field(")
    assert occurrences == 1, f"expected exactly 1 definition, found {occurrences}"
    # and the property still works
    s = cfg.Settings()
    assert isinstance(s.signing_keys, list)


def test_no_hardcoded_legacy_version_in_source():
    """No .py source file may contain the stale 2.0.0-python literal."""
    import pathlib

    root = pathlib.Path(app_root())
    offenders = []
    for f in root.rglob("*.py"):
        if "__pycache__" in f.parts or "test_" in f.name:
            continue
        if "2.0.0-python" in f.read_text(encoding="utf-8"):
            offenders.append(str(f))
    assert not offenders, f"stale version literal found in: {offenders}"


def app_root() -> str:
    import pathlib

    import app

    return str(pathlib.Path(app.__file__).resolve().parent)
