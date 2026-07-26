"""Tests for structured error types (errors.py)."""

from __future__ import annotations

from app.errors import (
    AuthenticationError,
    NotFoundError,
    PayloadTooLargeError,
    RateLimitError,
    StorageError,
    TelegramDriveError,
    TelegramError,
    ValidationError,
)


def test_base_error_as_dict():
    err = TelegramDriveError("TEST_CODE", "Test message", 418)
    assert err.code == "TEST_CODE"
    assert err.message == "Test message"
    assert err.status_code == 418
    assert err.as_dict() == {"error": {"code": "TEST_CODE", "message": "Test message"}}


def test_base_error_default_status():
    err = TelegramDriveError("X", "msg")
    assert err.status_code == 500


def test_not_found_error():
    err = NotFoundError("File", "123")
    assert err.code == "NOT_FOUND"
    assert err.status_code == 404
    assert "File" in err.message
    assert "123" in err.message


def test_authentication_error():
    err = AuthenticationError()
    assert err.code == "UNAUTHORIZED"
    assert err.status_code == 401
    assert err.message == "Invalid credentials"


def test_authentication_error_custom_message():
    err = AuthenticationError("Wrong password")
    assert err.message == "Wrong password"


def test_rate_limit_error():
    err = RateLimitError()
    assert err.code == "RATE_LIMITED"
    assert err.status_code == 429
    assert err.retry_after == 60


def test_rate_limit_error_custom():
    err = RateLimitError("Slow down", retry_after=120)
    assert err.retry_after == 120


def test_storage_error():
    err = StorageError()
    assert err.code == "STORAGE_ERROR"
    assert err.status_code == 500


def test_telegram_error():
    err = TelegramError()
    assert err.code == "NOT_CONNECTED"
    assert err.status_code == 503


def test_validation_error():
    err = ValidationError("Name is required", field="name")
    assert err.code == "VALIDATION_ERROR"
    assert err.status_code == 400
    assert err.field == "name"


def test_validation_error_no_field():
    err = ValidationError("Bad input")
    assert err.field is None


def test_payload_too_large_error():
    err = PayloadTooLargeError(100)
    assert err.code == "PAYLOAD_TOO_LARGE"
    assert err.status_code == 413
    assert "100MB" in err.message


def test_error_is_exception():
    err = TelegramDriveError("X", "msg")
    assert isinstance(err, Exception)


def test_error_str():
    err = TelegramDriveError("X", "hello world")
    assert str(err) == "hello world"


def test_global_exception_handler_integration(client):
    # Pass authentication to avoid 401
    from .conftest import API_KEY
    response = client.get("/api/v1/files/99999999", headers={"X-API-Key": API_KEY})
    # Since Telegram is not configured in tests, it should raise a TelegramError and return 503
    assert response.status_code == 503
    data = response.json()
    assert data["error"]["code"] == "NOT_CONNECTED"

