"""Prometheus-compatible metrics for Telegram Drive.

Exports counters and gauges via the ``get_metrics()`` function that returns
Prometheus text format. The /metrics endpoint in health.py calls this.
"""

from __future__ import annotations

import time

from prometheus_client import (  # type: ignore[import-untyped]
    CollectorRegistry,
    Counter,
    Gauge,
    Histogram,
    generate_latest,
)


class MetricsRegistry:
    """Central metrics collector with a dedicated Prometheus registry."""

    def __init__(self) -> None:
        self._registry: CollectorRegistry = CollectorRegistry()

        self.requests_total = Counter(
            "telegram_drive_requests_total",
            "Total HTTP requests",
            ["method", "path", "status_code"],
            registry=self._registry,
        )

        self.request_duration_seconds = Histogram(
            "telegram_drive_request_duration_seconds",
            "HTTP request duration in seconds",
            ["method", "path"],
            buckets=(0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0),
            registry=self._registry,
        )

        self.upload_bytes_total = Counter(
            "telegram_drive_upload_bytes_total",
            "Total bytes uploaded",
            ["transport_mode"],
            registry=self._registry,
        )

        self.download_bytes_total = Counter(
            "telegram_drive_download_bytes_total",
            "Total bytes downloaded",
            ["transport_mode"],
            registry=self._registry,
        )

        self.active_connections = Gauge(
            "telegram_drive_active_connections",
            "Currently active connections",
            registry=self._registry,
        )

        self.files_total = Gauge(
            "telegram_drive_files_total",
            "Total files tracked",
            ["transport_mode"],
            registry=self._registry,
        )

        self.storage_bytes_total = Gauge(
            "telegram_drive_storage_bytes_total",
            "Total storage bytes used",
            ["transport_mode"],
            registry=self._registry,
        )

        self.shares_total = Gauge(
            "telegram_drive_shares_total",
            "Active share links",
            registry=self._registry,
        )

        self.upload_slots_available = Gauge(
            "telegram_drive_upload_slots_available",
            "Available upload file slots",
            registry=self._registry,
        )

        self.upload_chunk_slots_available = Gauge(
            "telegram_drive_upload_chunk_slots_available",
            "Available upload chunk slots",
            registry=self._registry,
        )

        # v8 (TASK-P1-03): HTTP cache hit/miss counters for ETag conditional requests.
        self.download_304_total = Counter(
            "telegram_drive_download_304_total",
            "Downloads served 304 Not Modified (ETag hit)",
            registry=self._registry,
        )
        self.download_200_total = Counter(
            "telegram_drive_download_200_total",
            "Downloads served 200 with body",
            registry=self._registry,
        )

        self._started_at = time.time()

    def uptime_seconds(self) -> float:
        return time.time() - self._started_at

    def generate(self) -> bytes:
        return generate_latest(self._registry)


# Global singleton.
_metrics_registry: MetricsRegistry | None = None


def get_registry() -> MetricsRegistry:
    """Return the global metrics registry, creating it on first access."""
    global _metrics_registry
    if _metrics_registry is None:
        _metrics_registry = MetricsRegistry()
    return _metrics_registry


def get_metrics() -> bytes:
    """Return Prometheus text-format metrics."""
    return get_registry().generate()
