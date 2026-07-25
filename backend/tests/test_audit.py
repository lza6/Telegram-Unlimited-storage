"""AuditLogger unit tests — event logging, JSON serialization, file output."""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path

import pytest

from app.audit import AuditEntry, AuditEvent, AuditLogger


class TestAuditEntry:
    def test_to_json_contains_required_fields(self):
        entry = AuditEntry(
            event=AuditEvent.AUTH_SUCCESS,
            actor="192.168.1.1",
            target="user-123",
            success=True,
            metadata={"method": "password"},
        )
        data = json.loads(entry.to_json())
        assert data["event"] == "auth.success"
        assert data["actor"] == "192.168.1.1"
        assert data["target"] == "user-123"
        assert data["success"] is True
        assert data["metadata"] == {"method": "password"}
        assert "timestamp" in data

    def test_to_json_with_minimal_data(self):
        entry = AuditEntry(
            event=AuditEvent.FILE_DELETE,
            actor="admin",
            target=None,
            success=True,
        )
        data = json.loads(entry.to_json())
        assert data["event"] == "file.delete"
        assert data["actor"] == "admin"
        assert data["target"] is None
        assert data["success"] is True
        assert data["metadata"] == {}

    def test_to_json_timestamp_is_iso_format(self):
        entry = AuditEntry(
            event=AuditEvent.TELEGRAM_LOGIN,
            actor="127.0.0.1",
            target=None,
            success=True,
        )
        data = json.loads(entry.to_json())
        # ISO format check: contains 'T' separator
        assert "T" in data["timestamp"]
        # Should parse back to a valid datetime
        from datetime import datetime

        dt = datetime.fromisoformat(data["timestamp"])
        assert dt.year > 2020


class TestAuditLogger:
    def test_log_writes_to_file(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log(AuditEvent.AUTH_SUCCESS, "10.0.0.1", success=True)

            with open(log_path, "r", encoding="utf-8") as f2:
                lines = f2.readlines()

            assert len(lines) == 1
            data = json.loads(lines[0])
            assert data["event"] == "auth.success"
            assert data["actor"] == "10.0.0.1"
        finally:
            os.unlink(log_path)

    def test_log_does_not_write_when_disabled(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=False)
            logger.log(AuditEvent.AUTH_FAILURE, "10.0.0.1", success=False)

            with open(log_path, "r", encoding="utf-8") as f2:
                content = f2.read()

            assert content == ""
        finally:
            os.unlink(log_path)

    def test_log_creates_parent_directories(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "subdir" / "audit.log"
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log(AuditEvent.SHARE_CREATE, "localhost", success=True)
            assert log_path.exists()
            with open(log_path, "r", encoding="utf-8") as f:
                lines = f.readlines()
            assert len(lines) == 1

    def test_log_auth_success(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_auth_success("192.168.1.100", method="password")

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "auth.success"
            assert data["actor"] == "192.168.1.100"
            assert data["metadata"]["method"] == "password"
        finally:
            os.unlink(log_path)

    def test_log_auth_failure(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_auth_failure("192.168.1.100", reason="invalid password")

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "auth.failure"
            assert data["success"] is False
            assert data["metadata"]["reason"] == "invalid password"
        finally:
            os.unlink(log_path)

    def test_log_file_upload(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_file_upload("10.0.0.1", file_id=12345, filename="test.pdf", size=1024)

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "file.upload"
            assert data["target"] == "12345"
            assert data["metadata"]["filename"] == "test.pdf"
            assert data["metadata"]["size"] == 1024
        finally:
            os.unlink(log_path)

    def test_log_file_delete(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_file_delete("admin", file_ids=[1, 2, 3], count=3)

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "file.delete"
            assert data["metadata"]["file_ids"] == [1, 2, 3]
            assert data["metadata"]["count"] == 3
        finally:
            os.unlink(log_path)

    def test_log_share_create(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_share_create(
                "localhost", share_id="abc123", filename="secret.pdf", password_protected=True
            )

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "share.create"
            assert data["target"] == "abc123"
            assert data["metadata"]["filename"] == "secret.pdf"
            assert data["metadata"]["password_protected"] is True
        finally:
            os.unlink(log_path)

    def test_log_share_download(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_share_download("192.168.1.1", share_id="xyz789", file_id=42)

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "share.download"
            assert data["target"] == "xyz789"
            assert data["metadata"]["file_id"] == 42
        finally:
            os.unlink(log_path)

    def test_log_share_password_fail(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_share_password_fail("10.0.0.5", share_id="locked-share")

            with open(log_path, "r", encoding="utf-8") as f2:
                data = json.loads(f2.read())
            assert data["event"] == "share.password_failed"
            assert data["success"] is False
        finally:
            os.unlink(log_path)

    def test_console_output_enabled(self, capsys):
        """Test that console output works when enabled."""
        logger = AuditLogger(log_path=None, enabled=True, console_output=True)
        logger.log(AuditEvent.AUTH_LOCKOUT, "192.168.1.1", success=False)
        # Just verify it doesn't raise

    def test_multiple_events_in_sequence(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".log", delete=False) as f:
            log_path = Path(f.name)
        try:
            logger = AuditLogger(log_path=log_path, enabled=True)
            logger.log_auth_success("10.0.0.1")
            logger.log_file_upload("10.0.0.1", file_id=1, filename="a.txt", size=100)
            logger.log_share_create("10.0.0.1", share_id="s1", filename="b.txt", password_protected=False)

            with open(log_path, "r", encoding="utf-8") as f2:
                lines = f2.readlines()

            assert len(lines) == 3
            events = [json.loads(line)["event"] for line in lines]
            assert events == ["auth.success", "file.upload", "share.create"]
        finally:
            os.unlink(log_path)


class TestAuditEvent:
    def test_all_events_have_string_values(self):
        """All AuditEvent members should have string values."""
        for event in AuditEvent:
            assert isinstance(event.value, str)
            assert len(event.value) > 0

    def test_event_categories(self):
        """Verify events are categorized correctly."""
        # Auth events
        assert AuditEvent.AUTH_SUCCESS.value == "auth.success"
        assert AuditEvent.AUTH_FAILURE.value == "auth.failure"
        assert AuditEvent.AUTH_LOCKOUT.value == "auth.lockout"

        # File events
        assert AuditEvent.FILE_UPLOAD.value == "file.upload"
        assert AuditEvent.FILE_DELETE.value == "file.delete"

        # Share events
        assert AuditEvent.SHARE_CREATE.value == "share.create"
        assert AuditEvent.SHARE_DOWNLOAD.value == "share.download"
        assert AuditEvent.SHARE_PASSWORD_FAIL.value == "share.password_failed"
