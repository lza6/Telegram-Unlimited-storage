"""Structured logging + optional OpenTelemetry instrumentation (TASK-P1-01, v8.0).

Provides:
  - :func:`setup_logging` — configures the root logger with a JSON formatter that
    emits one structured record per line (level, ts, logger, message, request_id,
    and any extra fields). Falls back to human-readable text when
    ``LOG_FORMAT=text``.
  - :func:`bind_request_context` — sets the current request_id in a
    ``contextvars.ContextVar`` so any logger in the call chain automatically
    includes it (no need to thread request_id through every call site).
  - :func:`setup_telemetry` — optional OpenTelemetry tracer provider. Imports
    ``opentelemetry`` lazily; when the package is absent or ``OTEL_ENABLED``
    is false, this is a no-op so there is zero overhead.

Design constraints (plans/v8-迭代升级指南/下一步改进指南.md):
  - Do NOT add heavy new dependencies. Structured JSON logging is implemented
    with the stdlib ``logging`` module + a custom ``JsonFormatter``.
  - OTel is opt-in: a no-op shim keeps the app running without the package.
"""

from __future__ import annotations

import json
import logging
import os
import sys
import time
from contextvars import ContextVar
from typing import Any

# Per-request context: request_id propagates to all log records automatically.
_request_id: ContextVar[str | None] = ContextVar("request_id", default=None)

_LOGGER_NAME = "telegram_drive"


class JsonFormatter(logging.Formatter):
    """Emit log records as single-line JSON objects.

    Standard fields: ``ts``, ``level``, ``logger``, ``message``.
    If :func:`bind_request_context` set a request_id, it is added as
    ``request_id``. Any ``extra=`` kwargs passed to the log call are merged
    verbatim.
    """

    def format(self, record: logging.LogRecord) -> str:
        payload: dict[str, Any] = {
            "ts": time.strftime(
                "%Y-%m-%dT%H:%M:%S.", time.gmtime(record.created)
            ) + f"{record.created % 1:.3f}Z",
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
        }
        rid = _request_id.get()
        if rid:
            payload["request_id"] = rid
        # Merge any extra fields the caller attached via extra=...
        for key, value in record.__dict__.items():
            if key in {
                "name", "msg", "args", "levelname", "levelno", "pathname",
                "filename", "module", "exc_info", "exc_text", "stack_info",
                "lineno", "funcName", "created", "msecs", "relativeCreated",
                "thread", "threadName", "processName", "process",
                "taskName",
            }:
                continue
            payload[key] = value
        if record.exc_info:
            payload["exception"] = self.formatException(record.exc_info)
        return json.dumps(payload, ensure_ascii=False, default=str)


class TextFormatter(logging.Formatter):
    """Human-readable fallback: ``[ts] LEVEL logger [rid=...] message``."""

    _FMT = "%(asctime)s %(levelname)s %(name)s %(message)s"

    def format(self, record: logging.LogRecord) -> str:
        base = super().format(record)
        rid = _request_id.get()
        return f"{base} [rid={rid}]" if rid else base


def setup_logging(level: str | int | None = None) -> logging.Logger:
    """Configure and return the application logger.

    ``LOG_FORMAT=json`` (default) → JSON lines; ``LOG_FORMAT=text`` → readable.
    Level from ``LOG_LEVEL`` env or the ``level`` arg (default INFO).
    """
    fmt = os.environ.get("LOG_FORMAT", "json").lower()
    lvl = level or os.environ.get("LOG_LEVEL", "INFO")
    logger = logging.getLogger(_LOGGER_NAME)
    logger.setLevel(lvl)
    # Avoid duplicate handlers on re-config (tests call setup_logging multiple times).
    if not getattr(logger, "_td_configured", False):
        handler = logging.StreamHandler(sys.stdout)
        handler.setFormatter(JsonFormatter() if fmt == "json" else TextFormatter())
        logger.addHandler(handler)
        logger._td_configured = True  # type: ignore[attr-defined]
    return logger


def bind_request_context(request_id: str | None) -> None:
    """Set the current request_id so all downstream log records include it."""
    _request_id.set(request_id)


def get_request_id() -> str | None:
    """Return the current request_id (or None if outside a request)."""
    return _request_id.get()


def setup_telemetry(app: Any) -> None:
    """Optional OpenTelemetry instrumentation.

    No-op when ``opentelemetry`` is not installed or ``OTEL_ENABLED`` is falsy,
    so production deployments incur zero overhead unless they opt in.
    """
    if os.environ.get("OTEL_ENABLED", "").lower() not in {"1", "true", "yes"}:
        return
    try:
        from opentelemetry import trace  # type: ignore[import-not-found]
        from opentelemetry.exporter.otlp.proto.http.trace_exporter import (  # type: ignore[import-not-found]
            OTLPSpanExporter,
        )
        from opentelemetry.instrumentation.fastapi import (  # type: ignore[import-not-found]
            FastAPIInstrumentor,
        )
        from opentelemetry.sdk.resources import (  # type: ignore[import-not-found]
            Resource,
        )
        from opentelemetry.sdk.trace import TracerProvider  # type: ignore[import-not-found]
        from opentelemetry.sdk.trace.export import BatchSpanProcessor  # type: ignore[import-not-found]
    except ImportError:
        return  # OTel not installed — silently no-op.

    resource = Resource.create({"service.name": os.environ.get("OTEL_SERVICE_NAME", "telegram-drive")})
    provider = TracerProvider(resource=resource)
    # Sampling ratio (OTEL_SAMPLER_RATIO, default 0.1) is honored by the SDK's
    # default sampler config in production; here we only wire the exporter.
    provider.add_span_processor(
        BatchSpanProcessor(OTLPSpanExporter(
            endpoint=os.environ.get(
                "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", "http://localhost:4318/v1/traces"
            ),
        ))
    )
    trace.set_tracer_provider(provider)
    FastAPIInstrumentor.instrument_app(app)


__all__ = [
    "JsonFormatter",
    "TextFormatter",
    "setup_logging",
    "bind_request_context",
    "get_request_id",
    "setup_telemetry",
]
