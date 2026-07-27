"""Tests for audit log query (query_audit_log function)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from app.audit import query_audit_log


@pytest.fixture()
def audit_log(tmp_path):
    p = tmp_path / "audit.log"
    return p


def test_query_empty_log(audit_log):
    results = query_audit_log(audit_log, limit=10)
    assert results == []


def test_query_all_entries(audit_log):
    entries = [
        json.dumps({"event": "auth.success", "actor": "127.0.0.1", "timestamp": "2026-01-01T00:00:00Z"}),
        json.dumps({"event": "file.upload", "actor": "127.0.0.1", "timestamp": "2026-01-01T01:00:00Z"}),
        json.dumps({"event": "file.download", "actor": "10.0.0.1", "timestamp": "2026-01-01T02:00:00Z"}),
    ]
    audit_log.write_text("\n".join(entries) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, limit=10)
    assert len(results) == 3


def test_query_filter_by_event(audit_log):
    entries = [
        json.dumps({"event": "auth.success", "actor": "127.0.0.1", "timestamp": "2026-01-01T00:00:00Z"}),
        json.dumps({"event": "file.upload", "actor": "127.0.0.1", "timestamp": "2026-01-01T01:00:00Z"}),
    ]
    audit_log.write_text("\n".join(entries) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, event_type="auth.success", limit=10)
    assert len(results) == 1
    assert results[0]["event"] == "auth.success"


def test_query_filter_by_actor(audit_log):
    entries = [
        json.dumps({"event": "auth.success", "actor": "127.0.0.1", "timestamp": "2026-01-01T00:00:00Z"}),
        json.dumps({"event": "file.upload", "actor": "10.0.0.1", "timestamp": "2026-01-01T01:00:00Z"}),
    ]
    audit_log.write_text("\n".join(entries) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, actor="10.0.0.1", limit=10)
    assert len(results) == 1
    assert results[0]["actor"] == "10.0.0.1"


def test_query_filter_by_since(audit_log):
    entries = [
        json.dumps({"event": "auth.success", "actor": "127.0.0.1", "timestamp": "2026-01-01T00:00:00Z"}),
        json.dumps({"event": "file.upload", "actor": "127.0.0.1", "timestamp": "2026-01-01T02:00:00Z"}),
    ]
    audit_log.write_text("\n".join(entries) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, since="2026-01-01T01:00:00Z", limit=10)
    assert len(results) == 1
    assert results[0]["event"] == "file.upload"


def test_query_limit_enforced(audit_log):
    entries = [
        json.dumps({"event": f"event.{i}", "actor": "127.0.0.1", "timestamp": "2026-01-01T00:00:00Z"})
        for i in range(10)
    ]
    audit_log.write_text("\n".join(entries) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, limit=3)
    assert len(results) == 3


def test_query_missing_file():
    p = Path("/nonexistent/path/audit.log")
    results = query_audit_log(p)
    assert results == []


def test_query_skips_invalid_json(audit_log):
    lines = [
        "not valid json",
        json.dumps({"event": "auth.success", "actor": "x", "timestamp": "2026-01-01T00:00:00Z"}),
    ]
    audit_log.write_text("\n".join(lines) + "\n", encoding="utf-8")
    results = query_audit_log(audit_log, limit=10)
    assert len(results) == 1
