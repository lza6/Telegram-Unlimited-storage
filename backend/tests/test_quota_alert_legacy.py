"""Tests for quota alerts + legacy Deprecation headers (TASK-P2-02/P2-04, v8.0)."""

from __future__ import annotations

from pathlib import Path

from app.quota import _maybe_alert_quota, check_upload_quota
from app.storage import Storage


def _storage(tmp_path: Path) -> Storage:
    return Storage(tmp_path / "q.db")


def test_quota_alert_below_threshold_is_silent(tmp_path: Path, monkeypatch) -> None:
    """Usage < 80% → no audit alert emitted."""
    calls: list[tuple] = []
    # Capture audit.log calls by monkeypatching the audit logger.
    import app.quota as quota_mod
    class _FakeAudit:
        def log(self, *a, **kw): calls.append((a, kw))
    monkeypatch.setattr(quota_mod, "get_audit_logger", lambda: _FakeAudit())
    s = _storage(tmp_path)
    try:
        state = type("S", (), {"storage": s})()
        _maybe_alert_quota(state, "t1", 40, 100, 4, 10)
        assert calls == []
    finally:
        s.close()


def test_quota_alert_at_threshold_emits_event(tmp_path: Path, monkeypatch) -> None:
    """Usage >= 80% → audit.alert event emitted with ratio."""
    calls: list[dict] = []
    import app.quota as quota_mod
    class _FakeAudit:
        def log(self, *a, **kw):
            calls.append({"args": a, "kw": kw})
    monkeypatch.setattr(quota_mod, "get_audit_logger", lambda: _FakeAudit())
    s = _storage(tmp_path)
    try:
        state = type("S", (), {"storage": s})()
        _maybe_alert_quota(state, "t1", 80, 100, 8, 10)
        assert len(calls) == 1
        assert calls[0]["kw"]["action"] == "quota.alert"
        assert calls[0]["kw"]["ratio"] == 0.8
    finally:
        s.close()


def test_quota_alert_zero_limit_is_silent(tmp_path: Path, monkeypatch) -> None:
    """limit=0 (unlimited) → never alerts."""
    calls: list = []
    import app.quota as quota_mod
    class _FakeAudit:
        def log(self, *a, **kw): calls.append(kw)
    monkeypatch.setattr(quota_mod, "get_audit_logger", lambda: _FakeAudit())
    s = _storage(tmp_path)
    try:
        state = type("S", (), {"storage": s})()
        _maybe_alert_quota(state, "t1", 999, 0, 1, 0)
        assert calls == []
    finally:
        s.close()


def test_legacy_endpoints_carry_deprecation_header(client):
    """Legacy tg-disk endpoints return Deprecation + Sunset headers."""
    # /upload_status is a legacy endpoint; without a session it returns an
    # error but the deprecation header is still applied by the middleware.
    r = client.get("/upload_status", headers={"X-Access-Pwd": "testpwd"})
    assert "deprecation" in {k.lower() for k in r.headers}
    assert "sunset" in {k.lower() for k in r.headers}


def test_non_legacy_endpoints_have_no_deprecation_header(client):
    """Modern /api/v1/* endpoints must NOT carry the Deprecation header."""
    r = client.get("/api/v1/health")
    assert "deprecation" not in {k.lower() for k in r.headers}
