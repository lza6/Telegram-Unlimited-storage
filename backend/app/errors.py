"""Structured error types — replaces ad-hoc string-based error handling.

All API errors inherit from TelegramDriveError, which carries an error code,
human-readable message, and HTTP status code. FastAPI exception handlers can
map these to the standard ``{"error": {"code": ..., "message": ...}}`` envelope.
"""

from __future__ import annotations


class TelegramDriveError(Exception):
    """Base exception for all Telegram Drive API errors."""

    def __init__(self, code: str, message: str, status_code: int = 500) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.status_code = status_code

    def as_dict(self) -> dict:
        return {"error": {"code": self.code, "message": self.message}}


class NotFoundError(TelegramDriveError):
    """Resource not found (404)."""

    def __init__(self, resource: str, identifier: str) -> None:
        super().__init__(
            "NOT_FOUND",
            f"{resource} '{identifier}' not found",
            status_code=404,
        )


class AuthenticationError(TelegramDriveError):
    """Authentication failed (401)."""

    def __init__(self, message: str = "Invalid credentials") -> None:
        super().__init__("UNAUTHORIZED", message, status_code=401)


class RateLimitError(TelegramDriveError):
    """Too many requests (429)."""

    def __init__(self, message: str = "Too many requests", retry_after: int = 60) -> None:
        super().__init__("RATE_LIMITED", message, status_code=429)
        self.retry_after = retry_after


class StorageError(TelegramDriveError):
    """Database or storage operation failed (500)."""

    def __init__(self, message: str = "Storage operation failed") -> None:
        super().__init__("STORAGE_ERROR", message, status_code=500)


class TelegramError(TelegramDriveError):
    """Telegram transport is not ready (503)."""

    def __init__(self, message: str = "Telegram transport is not ready") -> None:
        super().__init__("NOT_CONNECTED", message, status_code=503)


class ValidationError(TelegramDriveError):
    """Invalid input (400)."""

    def __init__(self, message: str, field: str | None = None) -> None:
        super().__init__("VALIDATION_ERROR", message, status_code=400)
        self.field = field


class PayloadTooLargeError(TelegramDriveError):
    """Request body exceeds limit (413)."""

    def __init__(self, max_mb: int) -> None:
        super().__init__(
            "PAYLOAD_TOO_LARGE",
            f"Request body exceeds {max_mb}MB limit",
            status_code=413,
        )