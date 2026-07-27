"""TDD tests for observability: structured logging + request context (TASK-P1-01)."""

from __future__ import annotations

import io
import json
import logging

from app.observability import (
    JsonFormatter,
    bind_request_context,
    get_request_id,
    setup_logging,
    setup_telemetry,
)


def _make_record(msg: str = "hello", **extra) -> logging.LogRecord:
    record = logging.LogRecord(
        name="telegram_drive.test", level=logging.INFO, pathname="t.py",
        lineno=1, msg=msg, args=None, exc_info=None,
    )
    for k, v in extra.items():
        setattr(record, k, v)
    return record


def test_json_formatter_emits_valid_json() -> None:
    """JsonFormatter produces a single-line JSON object."""
    out = JsonFormatter().format(_make_record("test message"))
    payload = json.loads(out)
    assert payload["message"] == "test message"
    assert payload["level"] == "INFO"
    assert payload["logger"] == "telegram_drive.test"
    assert "ts" in payload
    assert "\n" not in out  # single line


def test_json_formatter_includes_request_id_when_bound() -> None:
    """request_id from context is injected into the JSON record."""
    bind_request_context("req-abc")
    try:
        out = JsonFormatter().format(_make_record())
        payload = json.loads(out)
        assert payload["request_id"] == "req-abc"
    finally:
        bind_request_context(None)


def test_json_formatter_omits_request_id_when_unset() -> None:
    """No request_id in context → key absent from record."""
    bind_request_context(None)
    out = JsonFormatter().format(_make_record())
    payload = json.loads(out)
    assert "request_id" not in payload


def test_json_formatter_merges_extras() -> None:
    """Extra fields passed to logger.info(..., extra=...) are merged."""
    out = JsonFormatter().format(_make_record("m", user="alice", bytes_in=1024))
    payload = json.loads(out)
    assert payload["user"] == "alice"
    assert payload["bytes_in"] == 1024


def test_json_formatter_serializes_exception() -> None:
    """Exc info is serialized into an 'exception' field."""
    try:
        raise ValueError("boom")
    except ValueError:
        import sys
        record = logging.LogRecord(
            name="x", level=logging.ERROR, pathname="t.py", lineno=1,
            msg="failed", args=None, exc_info=sys.exc_info(),
        )
    out = JsonFormatter().format(record)
    payload = json.loads(out)
    assert "ValueError" in payload["exception"]
    assert "boom" in payload["exception"]


def test_setup_logging_returns_logger_and_is_idempotent() -> None:
    """setup_logging returns the telegram_drive logger and is idempotent."""
    logger = setup_logging("DEBUG")
    assert logger.name == "telegram_drive"
    assert logger.level == logging.DEBUG
    # Second call does not add duplicate handlers.
    n_before = len(logger.handlers)
    setup_logging("DEBUG")
    assert len(logger.handlers) == n_before


def test_get_request_id_round_trip() -> None:
    """bind then get returns the bound id; clear returns None."""
    bind_request_context("r1")
    assert get_request_id() == "r1"
    bind_request_context(None)
    assert get_request_id() is None


def test_setup_telemetry_noop_when_disabled(monkeypatch) -> None:
    """OTEL_ENABLED unset → setup_telemetry is a no-op (no exception)."""
    monkeypatch.delenv("OTEL_ENABLED", raising=False)
    # Should not raise even though opentelemetry may be absent.
    setup_telemetry(app=None)


def test_setup_telemetry_noop_without_package(monkeypatch) -> None:
    """OTEL_ENABLED=1 but no opentelemetry installed → silent no-op."""
    monkeypatch.setenv("OTEL_ENABLED", "1")
    # Simulate absent package by injecting ImportError in the import path.
    import builtins
    real_import = builtins.__import__

    def _fail(name, *a, **kw):
        if name.startswith("opentelemetry"):
            raise ImportError(name)
        return real_import(name, *a, **kw)
    builtins.__import__ = _fail
    try:
        setup_telemetry(app=None)  # must not raise
    finally:
        builtins.__import__ = real_import
