"""Quota management API routes (TASK-P1-03).

Provides admin endpoints to manage tenant quotas and an upload pre-check
that rejects uploads exceeding storage limits.
"""

from __future__ import annotations

import asyncio
import logging

from fastapi import APIRouter, Depends, Request
from fastapi.responses import JSONResponse

from .audit import AuditEvent, get_audit_logger
from .rbac import require_scope
from .state import AppState

logger = logging.getLogger("telegram_drive.quota")

router = APIRouter(prefix="/api/v1", tags=["quota"])


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


class QuotaExceededError(Exception):
    """Raised when an upload would exceed tenant storage quota."""

    def __init__(self, tenant_id: str, used: int, limit: int, attempted: int) -> None:
        self.tenant_id = tenant_id
        self.used = used
        self.limit = limit
        self.attempted = attempted
        super().__init__(
            f"quota exceeded for tenant {tenant_id}: "
            f"used={used} + attempted={attempted} > limit={limit}"
        )


def check_upload_quota(state: AppState, tenant_id: str, file_size: int) -> None:
    """Pre-check upload against tenant quota. Raises QuotaExceededError on overage.

    v8 (TASK-P2-02): also emits a quota.alert audit event when usage crosses
    the configured threshold (default 80%) so operators can act before the
    hard limit is hit.
    """
    quota = state.storage.get_tenant_quota(tenant_id)
    if not quota:
        return  # no quota configured = unlimited

    storage_limit = int(quota.get("storage_bytes_limit") or 0)
    if storage_limit <= 0:
        return  # 0 = unlimited

    files_limit = int(quota.get("files_count_limit") or 0)
    storage_used = int(quota.get("storage_bytes_used") or 0)
    files_used = int(quota.get("files_count_used") or 0)

    if storage_used + file_size > storage_limit:
        raise QuotaExceededError(tenant_id, storage_used, storage_limit, file_size)

    if files_limit > 0 and files_used >= files_limit:
        raise QuotaExceededError(tenant_id, storage_used, storage_limit, file_size)

    # v8: soft alert when usage is at/above the warning threshold.
    _maybe_alert_quota(state, tenant_id, storage_used + file_size, storage_limit,
                      files_used + 1, files_limit)


def _maybe_alert_quota(
    state: AppState,
    tenant_id: str,
    storage_used: int,
    storage_limit: int,
    files_used: int,
    files_limit: int,
    threshold: float = 0.8,
) -> None:
    """Emit a quota.alert audit event when usage ratio >= threshold."""
    ratio = storage_used / storage_limit if storage_limit > 0 else 0.0
    if ratio < threshold:
        return
    audit = get_audit_logger()
    if audit is None:
        return
    # Use a synthetic actor; quota alerts are system-side.
    audit.log(
        AuditEvent.SETTINGS_CHANGE, "system",
        target=f"quota:{tenant_id}", success=True,
        action="quota.alert",
        ratio=round(ratio, 3),
        storage_used=storage_used, storage_limit=storage_limit,
        files_used=files_used, files_limit=files_limit,
    )


@router.post("/admin/tenants/{tenant_id}/quota", dependencies=[Depends(require_scope("admin"))])
async def set_tenant_quota(
    tenant_id: str,
    request: Request,
    storage_bytes_limit: int = 0,
    files_count_limit: int = 0,
) -> JSONResponse:
    """Set storage/file limits for a tenant (admin scope required)."""
    state = get_state(request)
    if tenant_id != "default":
        all_tenants = state.storage.list_tenants()
        if not any(t.get("tenant_id") == tenant_id for t in all_tenants):
            return api_error("NOT_FOUND", "Tenant not found", 404)

    state.storage.upsert_tenant_quota(
        tenant_id=tenant_id,
        storage_bytes_limit=max(0, storage_bytes_limit),
        files_count_limit=max(0, files_count_limit),
    )

    audit = get_audit_logger()
    if audit:
        client_ip = request.client.host if request.client else "unknown"
        audit.log(
            AuditEvent.SETTINGS_CHANGE, client_ip,
            target=f"quota:{tenant_id}", success=True,
            action="set_quota",
            storage_bytes_limit=storage_bytes_limit,
            files_count_limit=files_count_limit,
        )

    return JSONResponse({"ok": True, "tenant_id": tenant_id,
                         "storage_bytes_limit": storage_bytes_limit,
                         "files_count_limit": files_count_limit})


@router.get("/admin/tenants/{tenant_id}/quota", dependencies=[Depends(require_scope("admin"))])
async def get_tenant_quota(tenant_id: str, request: Request) -> JSONResponse:
    state = get_state(request)
    quota = state.storage.get_tenant_quota(tenant_id)
    if not quota:
        return JSONResponse({
            "tenant_id": tenant_id,
            "storage_bytes_limit": 0,
            "storage_bytes_used": 0,
            "files_count_limit": 0,
            "files_count_used": 0,
        })
    return JSONResponse(dict(quota))


async def periodic_quota_reconcile(state: AppState) -> None:
    """Lifespan task: hourly reconcile of tenant quota counters."""
    try:
        while True:
            await asyncio.sleep(3600)
            try:
                tenants = state.storage.list_tenants()
                for t in tenants:
                    tid = t.get("tenant_id")
                    if tid:
                        state.storage.recompute_tenant_quota(tid)
                logger.debug("tenant quota reconciliation complete")
            except Exception as exc:  # noqa: BLE001
                logger.warning("quota reconciliation failed: %s", exc)
    except asyncio.CancelledError:
        pass

