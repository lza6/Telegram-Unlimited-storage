"""Tests for the Prometheus metrics registry."""

from __future__ import annotations

from app.metrics import get_metrics, get_registry


def test_metrics_registry_is_singleton():
    r1 = get_registry()
    r2 = get_registry()
    assert r1 is r2


def test_metrics_requests_total_exists():
    registry = get_registry()
    # Counter should be created and registered
    registry.requests_total.labels(
        method="GET", path="/test", status_code="200"
    ).inc()
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_requests_total" in output


def test_metrics_upload_bytes_counter():
    registry = get_registry()
    registry.upload_bytes_total.labels(transport_mode="bot").inc(1024)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_upload_bytes_total" in output
    assert "bot" in output


def test_metrics_download_bytes_counter():
    registry = get_registry()
    registry.download_bytes_total.labels(transport_mode="user").inc(2048)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_download_bytes_total" in output


def test_metrics_active_connections_gauge():
    registry = get_registry()
    registry.active_connections.set(5)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_active_connections" in output


def test_metrics_files_total_gauge():
    registry = get_registry()
    registry.files_total.labels(transport_mode="bot").set(42)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_files_total" in output


def test_metrics_storage_bytes_total():
    registry = get_registry()
    registry.storage_bytes_total.labels(transport_mode="user").set(1024 * 1024)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_storage_bytes_total" in output


def test_metrics_shares_total():
    registry = get_registry()
    registry.shares_total.set(10)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_shares_total" in output


def test_metrics_upload_slots():
    registry = get_registry()
    registry.upload_slots_available.set(3)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_upload_slots_available" in output


def test_metrics_histogram_buckets():
    registry = get_registry()
    registry.request_duration_seconds.labels(
        method="POST", path="/upload"
    ).observe(0.15)
    output = get_metrics().decode("utf-8")
    assert "telegram_drive_request_duration_seconds" in output
    assert 'le="0.25"' in output


def test_metrics_uptime():
    registry = get_registry()
    uptime = registry.uptime_seconds()
    assert uptime >= 0
    assert uptime < 120  # uptime grows across test suite; bound generously
