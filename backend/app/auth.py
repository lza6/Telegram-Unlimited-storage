"""HTTP authentication — X-Access-Pwd (console) and X-API-Key (integration).

Mirrors the Rust auth middleware:
- ``X-Access-Pwd`` is compared against ``ACCESS_PWD`` (env) or
  ``local_access_pwd`` (data/api_settings.json) in constant time.
- ``X-API-Key`` is verified against the stored hash (Argon2id or legacy
  SHA-256 hex); a successful legacy verification transparently upgrades the
  hash in ``api_settings.json``.
- Multi-tenant mode additionally checks the ``tenants`` table.
- Failed password attempts are rate-limited (lockout window).
"""

from __future__ import annotations

import hmac
import json
import logging
import time
from dataclasses import dataclass
from pathlib import Path

from fastapi import HTTPException, Request, status

from . import security
from .config import Settings
from .storage import Storage

logger = logging.getLogger("telegram_drive.auth")

ACCESS_PWD_HEADER = "X-Access-Pwd"
API_KEY_HEADER = "X-API-Key"


@dataclass(frozen=True)
class CallerIdentity:
    kind: str  # "console" | "api" | "tenant"
    tenant_id: str | None = None

    @property
    def owner_id(self) -> str:
        return self.tenant_id or "default"


class ApiSettingsFile:
    """Read/write data/api_settings.json (legacy format)."""

    def __init__(self, path: Path) -> None:
        self.path = path

    def load(self) -> dict:
        try:
            return json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return {}

    def save(self, data: dict) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(data, indent=2), encoding="utf-8")
        tmp.replace(self.path)

    def upgrade_key_hash(self, new_hash: str) -> None:
        data = self.load()
        existing = data.get("key_hash")
        if isinstance(existing, str) and existing.startswith(security.ARGON2_MARKER):
            return  # already Argon2 — never overwrite
        data["key_hash"] = new_hash
        self.save(data)


class AccessGuard:
    """Password lockout tracking (per-client-IP failed attempt window)."""

    def __init__(self, max_attempts: int, window_secs: int) -> None:
        self.max_attempts = max_attempts
        self.window_secs = window_secs
        self._attempts: dict[str, list[float]] = {}

    def check(self, client_ip: str) -> None:
        now = time.time()
        recent = [t for t in self._attempts.get(client_ip, []) if now - t < self.window_secs]
        self._attempts[client_ip] = recent
        if len(recent) >= self.max_attempts:
            raise HTTPException(
                status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                detail={
                    "code": "LOCKED_OUT",
                    "message": f"Too many failed attempts — retry in {self.window_secs}s",
                },
            )

    def record_failure(self, client_ip: str) -> None:
        self._attempts.setdefault(client_ip, []).append(time.time())

    def clear(self, client_ip: str) -> None:
        self._attempts.pop(client_ip, None)


class Authenticator:
    def __init__(self, settings: Settings, storage: Storage) -> None:
        self.settings = settings
        self.storage = storage
        self.api_settings_file = ApiSettingsFile(settings.api_settings_path)
        self.guard = AccessGuard(
            settings.access_lockout_max, settings.access_lockout_secs
        )

    # ── credential sources ──────────────────────────────────────────────────
    def _effective_access_pwd(self) -> str | None:
        if self.settings.access_pwd:
            return self.settings.access_pwd
        data = self.api_settings_file.load()
        pwd = data.get("local_access_pwd")
        return pwd if isinstance(pwd, str) and pwd else None

    def _effective_key_hash(self) -> str | None:
        data = self.api_settings_file.load()
        h = data.get("key_hash")
        if isinstance(h, str) and h:
            return h
        # Fall back to hashing the env API key so a fresh .env works without
        # a pre-seeded api_settings.json.
        if self.settings.api_key:
            return security.hash_api_key(self.settings.api_key)
        return None

    # ── verification ────────────────────────────────────────────────────────
    def verify_access_pwd(self, provided: str, client_ip: str) -> bool:
        expected = self._effective_access_pwd()
        if not expected:
            return False
        ok = hmac.compare_digest(provided, expected)
        if ok:
            self.guard.clear(client_ip)
        else:
            self.guard.record_failure(client_ip)
        return ok

    def verify_api_key(self, provided: str, required_scope: str | None = None) -> CallerIdentity | None:
        # Single-tenant key hash (api_settings.json / env API_KEY).
        stored = self._effective_key_hash()
        if stored:
            valid, should_upgrade = security.verify_api_key(provided, stored)
            if valid:
                if should_upgrade:
                    try:
                        self.api_settings_file.upgrade_key_hash(
                            security.hash_api_key(provided)
                        )
                    except OSError as exc:
                        logger.warning("key hash upgrade failed: %s", exc)
                # Scope check for single-tenant (full access by default)
                if required_scope and required_scope not in []:
                    # Single tenant has full access unless restricted via tenants table
                    pass
                return CallerIdentity(kind="api", tenant_id="default")
        # Multi-tenant table.
        if self.settings.multi_tenant_enabled:
            for tenant in self.storage.list_tenants():
                if not tenant.get("enabled"):
                    continue
                valid, should_upgrade = security.verify_api_key(
                    provided, tenant.get("api_key_hash", "")
                )
                if valid:
                    if should_upgrade:
                        self.storage.upsert_tenant(
                            tenant["tenant_id"],
                            security.hash_api_key(provided),
                            tenant.get("display_name"),
                        )
                    scopes = self.storage.get_tenant_scopes(tenant["tenant_id"])
                    if required_scope and scopes and required_scope not in scopes:
                        return None  # scope denied
                    return CallerIdentity(
                        kind="tenant", tenant_id=tenant["tenant_id"]
                    )
        return None

    # ── FastAPI dependency ──────────────────────────────────────────────────
    def require_auth(self, request: Request) -> CallerIdentity:
        client_ip = request.client.host if request.client else "unknown"
        pwd = request.headers.get(ACCESS_PWD_HEADER)
        if pwd:
            self.guard.check(client_ip)
            if self.verify_access_pwd(pwd, client_ip):
                return CallerIdentity(kind="console", tenant_id="default")
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "UNAUTHORIZED", "message": "Invalid access password"},
            )
        api_key = request.headers.get(API_KEY_HEADER)
        if api_key:
            # Infer required scope from route path
            path = request.url.path
            scope_map = {
                "/api/v1/files": "read",
                "/api/v1/shares": "read",
                "/api/v1/folders": "read",
            }
            required = None
            for prefix, sc in scope_map.items():
                if path.startswith(prefix):
                    required = sc
                    break
            identity = self.verify_api_key(api_key, required_scope=required)
            if identity is not None:
                return identity
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "UNAUTHORIZED", "message": "Invalid API key"},
            )
        # Neither header supplied.
        if self._effective_key_hash() is not None or self._effective_access_pwd():
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={
                    "code": "UNAUTHORIZED",
                    "message": "Missing X-API-Key header or X-Access-Pwd",
                },
            )
        # No credentials configured at all — open access (dev mode).
        return CallerIdentity(kind="console", tenant_id="default")
