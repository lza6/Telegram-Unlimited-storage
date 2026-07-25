"""Audit logging — records all security-sensitive operations.

Records in JSON Lines format for easy parsing by log aggregators.
"""

from __future__ import annotations

import json
import logging
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Optional

logger = logging.getLogger("telegram_drive.audit")


class AuditEvent(str, Enum):
    """All auditable event types."""

    # Authentication
    AUTH_SUCCESS = "auth.success"
    AUTH_FAILURE = "auth.failure"
    AUTH_LOCKOUT = "auth.lockout"

    # File operations
    FILE_UPLOAD = "file.upload"
    FILE_DOWNLOAD = "file.download"
    FILE_DELETE = "file.delete"
    FILE_MOVE = "file.move"
    FILE_LIST = "file.list"
    FILE_SEARCH = "file.search"

    # Share operations
    SHARE_CREATE = "share.create"
    SHARE_ACCESS = "share.access"
    SHARE_REVOKE = "share.revoke"
    SHARE_DOWNLOAD = "share.download"
    SHARE_PASSWORD_FAIL = "share.password_failed"

    # Settings
    SETTINGS_CHANGE = "settings.change"

    # System
    API_KEY_CREATE = "api.key_create"
    API_KEY_REGENERATE = "api.key_regenerate"
    TELEGRAM_LOGIN = "telegram.login"
    TELEGRAM_LOGOUT = "telegram.logout"


@dataclass
class AuditEntry:
    """Single audit log entry."""

    event: AuditEvent
    actor: str  # IP address or tenant_id
    target: Optional[str]  # Resource identifier
    success: bool
    metadata: dict[str, Any] = field(default_factory=dict)
    timestamp: str = field(
        default_factory=lambda: datetime.now(timezone.utc).isoformat()
    )

    def to_json(self) -> str:
        """Serialize to JSON string."""
        return json.dumps(
            {
                "event": self.event.value,
                "actor": self.actor,
                "target": self.target,
                "success": self.success,
                "metadata": self.metadata,
                "timestamp": self.timestamp,
            },
            ensure_ascii=False,
        )


class AuditLogger:
    """Structured audit logger with file sink and optional console output."""

    def __init__(
        self,
        log_path: Optional[Path] = None,
        enabled: bool = True,
        console_output: bool = False,
    ) -> None:
        self._enabled = enabled
        self._console = console_output
        self._path = log_path
        if log_path and enabled:
            log_path.parent.mkdir(parents=True, exist_ok=True)

    def log(
        self,
        event: AuditEvent,
        actor: str,
        target: Optional[str] = None,
        success: bool = True,
        **metadata: Any,
    ) -> None:
        """Record an audit event."""
        if not self._enabled:
            return

        entry = AuditEntry(
            event=event,
            actor=actor,
            target=target,
            success=success,
            metadata=metadata,
        )
        line = entry.to_json()

        if self._console:
            logger.info("[AUDIT] %s", line)

        if self._path:
            try:
                with open(self._path, "a", encoding="utf-8") as f:
                    f.write(line + "\n")
            except OSError as exc:
                logger.warning("audit log write failed: %s", exc)

    def log_auth_success(self, actor: str, method: str = "password") -> None:
        """Log successful authentication."""
        self.log(AuditEvent.AUTH_SUCCESS, actor, success=True, method=method)

    def log_auth_failure(self, actor: str, reason: str) -> None:
        """Log failed authentication attempt."""
        self.log(AuditEvent.AUTH_FAILURE, actor, success=False, reason=reason)

    def log_auth_lockout(self, actor: str) -> None:
        """Log account lockout."""
        self.log(AuditEvent.AUTH_LOCKOUT, actor, success=False)

    def log_file_upload(
        self, actor: str, file_id: int, filename: str, size: int
    ) -> None:
        """Log file upload."""
        self.log(
            AuditEvent.FILE_UPLOAD,
            actor,
            target=str(file_id),
            success=True,
            filename=filename,
            size=size,
        )

    def log_file_download(
        self, actor: str, file_id: int, filename: str
    ) -> None:
        """Log file download."""
        self.log(
            AuditEvent.FILE_DOWNLOAD,
            actor,
            target=str(file_id),
            success=True,
            filename=filename,
        )

    def log_file_delete(
        self, actor: str, file_ids: list[int], count: int
    ) -> None:
        """Log file deletion."""
        self.log(
            AuditEvent.FILE_DELETE,
            actor,
            target=f"count:{count}",
            success=True,
            file_ids=file_ids,
            count=count,
        )

    def log_share_create(
        self, actor: str, share_id: str, filename: str, password_protected: bool
    ) -> None:
        """Log share creation."""
        self.log(
            AuditEvent.SHARE_CREATE,
            actor,
            target=share_id,
            success=True,
            filename=filename,
            password_protected=password_protected,
        )

    def log_share_access(
        self, actor: str, share_id: str, success: bool
    ) -> None:
        """Log share access (public download)."""
        self.log(
            AuditEvent.SHARE_ACCESS,
            actor,
            target=share_id,
            success=success,
        )

    def log_share_download(
        self, actor: str, share_id: str, file_id: int
    ) -> None:
        """Log share-based download."""
        self.log(
            AuditEvent.SHARE_DOWNLOAD,
            actor,
            target=share_id,
            success=True,
            file_id=file_id,
        )

    def log_share_password_fail(self, actor: str, share_id: str) -> None:
        """Log failed share password attempt."""
        self.log(
            AuditEvent.SHARE_PASSWORD_FAIL,
            actor,
            target=share_id,
            success=False,
        )


# Global audit logger instance (initialized in main.py)
_audit_logger: Optional[AuditLogger] = None


def get_audit_logger() -> Optional[AuditLogger]:
    """Get the global audit logger instance."""
    return _audit_logger


def init_audit_logger(log_path: Path, enabled: bool = True) -> AuditLogger:
    """Initialize the global audit logger."""
    global _audit_logger
    _audit_logger = AuditLogger(log_path=log_path, enabled=enabled)
    return _audit_logger
