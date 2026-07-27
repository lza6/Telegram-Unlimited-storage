"""Download signing key automatic rotation manager.

Supports time-based automated key rotation + manual administrative rotation.
Persists rotated key rings in data/signing_keys.json with 0600 file permissions.
Retains retired keys (default: 3) so previously issued pre-signed URLs remain valid.
"""

from __future__ import annotations

import json
import logging
import os
import secrets
import time
from pathlib import Path
from typing import TYPE_CHECKING

from .audit import AuditEvent, get_audit_logger

if TYPE_CHECKING:
    from .config import Settings

logger = logging.getLogger("telegram_drive.key_rotation")

DEFAULT_ROTATION_INTERVAL_SECS = 30 * 86400  # 30 days
MAX_RETAINED_KEYS = 3


class KeyRotationManager:
    """Manages rotation, persistence and retrieval of HMAC signing key rings."""

    def __init__(
        self,
        data_dir: Path,
        settings: Settings | None = None,
        rotation_interval_secs: int = DEFAULT_ROTATION_INTERVAL_SECS,
        max_retained_keys: int = MAX_RETAINED_KEYS,
    ) -> None:
        self.data_dir = Path(data_dir)
        self.key_file = self.data_dir / "signing_keys.json"
        self.settings = settings
        self.rotation_interval_secs = rotation_interval_secs
        self.max_retained_keys = max_retained_keys

    def _ensure_permissions(self, path: Path) -> None:
        """Set file permissions to 0600 (read/write for owner only) where OS supports it."""
        try:
            os.chmod(path, 0o600)
        except (AttributeError, OSError):
            pass  # Windows permissions handled by ACLs

    def load_key_ring(self) -> dict:
        """Load key ring from data/signing_keys.json."""
        if not self.key_file.exists():
            return {
                "active_key": "",
                "retired_keys": [],
                "last_rotated_at": 0,
            }
        try:
            data = json.loads(self.key_file.read_text(encoding="utf-8"))
            if isinstance(data, dict):
                return {
                    "active_key": str(data.get("active_key", "")),
                    "retired_keys": [str(k) for k in data.get("retired_keys", []) if k],
                    "last_rotated_at": int(data.get("last_rotated_at", 0)),
                }
        except (OSError, ValueError) as exc:
            logger.warning("failed to load signing_keys.json: %s", exc)
        return {
            "active_key": "",
            "retired_keys": [],
            "last_rotated_at": 0,
        }

    def save_key_ring(self, ring: dict) -> None:
        """Atomically persist key ring and enforce 0600 permissions."""
        self.data_dir.mkdir(parents=True, exist_ok=True)
        tmp = self.key_file.with_suffix(".tmp")
        payload = {
            "active_key": ring.get("active_key", ""),
            "retired_keys": ring.get("retired_keys", []),
            "last_rotated_at": ring.get("last_rotated_at", int(time.time())),
        }
        tmp.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        self._ensure_permissions(tmp)
        tmp.replace(self.key_file)
        self._ensure_permissions(self.key_file)

    def rotate_key(self, actor: str = "system") -> dict:
        """Rotate signing key ring: generate new active secret, retire old active secret."""
        now = int(time.time())
        current = self.load_key_ring()
        new_secret = secrets.token_urlsafe(32)

        old_active = current.get("active_key", "")
        # If no active key exists in JSON, fallback to settings default if present
        if not old_active and self.settings:
            old_active = self.settings.download_signing_secret

        retired = current.get("retired_keys", [])
        if old_active and old_active not in retired:
            retired.insert(0, old_active)

        # Enforce max retained keys limit
        retired = retired[: self.max_retained_keys]

        new_ring = {
            "active_key": new_secret,
            "retired_keys": retired,
            "last_rotated_at": now,
        }
        self.save_key_ring(new_ring)

        # Update in-memory settings if available
        if self.settings:
            all_keys = [new_secret] + retired
            self.settings.download_signing_secrets = ",".join(all_keys)
            self.settings.download_signing_secret = new_secret

        # Audit log
        audit = get_audit_logger()
        if audit:
            audit.log(
                event=AuditEvent.SETTINGS_CHANGE,
                actor=actor,
                target="signing_keys.json",
                success=True,
                action="rotate_keys",
                rotated_at=now,
                retired_count=len(retired),
            )

        logger.info("Signing key rotated successfully by %s at %d", actor, now)
        return new_ring

    def get_all_secrets(self) -> list[str]:
        """Return all valid secrets: active secret first, followed by retired secrets."""
        ring = self.load_key_ring()
        active = ring.get("active_key", "")
        retired = ring.get("retired_keys", [])
        secrets_list = []

        if active:
            secrets_list.append(active)
        for r in retired:
            if r and r not in secrets_list:
                secrets_list.append(r)

        # Fallback to Settings if file is empty
        if not secrets_list and self.settings:
            return self.settings.all_signing_secrets

        return secrets_list

    def rotate_if_due(self, actor: str = "scheduler") -> dict | None:
        """Check if rotation interval has elapsed and rotate if due."""
        ring = self.load_key_ring()
        last = ring.get("last_rotated_at", 0)
        now = int(time.time())
        if last == 0 or (now - last) >= self.rotation_interval_secs:
            return self.rotate_key(actor=actor)
        return None
