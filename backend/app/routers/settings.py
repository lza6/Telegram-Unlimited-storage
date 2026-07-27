"""Admin settings: share domain (/api/v1/settings) + network (/api/v1/network).

Merge semantics and clamp ranges reproduce the Rust ``settings_routes``
exactly: only fields present in the patch override; proxy password/secret are
overwritten only when non-empty; VPN numeric fields are clamped to their
documented ranges.
"""

from __future__ import annotations

from typing import Any, Optional

from fastapi import APIRouter, Query, Request
from fastapi.responses import JSONResponse

from ..audit import query_audit_log
from ..settings_store import DEFAULT_PROXY, DEFAULT_VPN, SettingsStore
from ..state import AppState

router = APIRouter(prefix="/api/v1", tags=["settings"])


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


def _clamp(value: Any, low: float, high: float) -> float:
    try:
        return max(low, min(high, float(value)))
    except (TypeError, ValueError):
        return low


def _effective_base_url(state: AppState, request: Request) -> str:
    """BASE_URL env when set, else the request Host (Rust effective_base_url)."""
    if state.settings.base_url:
        return state.settings.base_url.rstrip("/")
    host = request.headers.get("host")
    return f"http://{host}" if host else "http://localhost:1334"


def _effective_share_base_url(state: AppState) -> str:
    """share_domain > http://localhost:{port} (Rust share_base_url_from_data_dir)."""
    store = SettingsStore(state.settings.data_dir)
    share_domain = (store.ui.load().get("share_domain") or "").strip()
    if share_domain:
        return share_domain.rstrip("/")
    return f"http://localhost:{state.settings.port}"


def _effective_share_link_base(state: AppState, request: Request) -> str:
    """Base actually used when minting share links: domain > BASE_URL > Host."""
    host = request.headers.get("host")
    return SettingsStore(state.settings.data_dir).share_base_url(
        state.settings.base_url, host
    )


# ── /api/v1/settings ────────────────────────────────────────────────────────
@router.get("/settings")
async def get_settings(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    store = SettingsStore(state.settings.data_dir)
    settings = state.settings
    return JSONResponse(
        {
            "share_domain": (store.ui.load().get("share_domain") or "").strip(),
            "env_base_url": settings.base_url,
            "effective_base_url": _effective_base_url(state, request),
            "effective_share_base_url": _effective_share_base_url(state),
            "effective_share_link_base": _effective_share_link_base(state, request),
            "chunk_size_mb": settings.chunk_size_mb,
            "chunk_concurrent": settings.chunk_concurrent,
            "files_concurrent": settings.files_concurrent,
            "download_threads": settings.download_threads,
            "stream_port": settings.port,
            "max_upload_size_mb": settings.max_upload_size_mb,
        }
    )


@router.put("/settings")
async def put_settings(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    try:
        body = await request.json()
    except ValueError:
        body = {}
    store = SettingsStore(state.settings.data_dir)
    current = store.ui.load()
    share_domain = body.get("share_domain")
    if share_domain is not None:
        current["share_domain"] = str(share_domain).strip()
    try:
        store.ui.save(current)
    except OSError as exc:
        return api_error("SAVE_FAILED", str(exc), 500)
    return JSONResponse(
        {
            "ok": True,
            "share_domain": current.get("share_domain", ""),
            "effective_base_url": _effective_base_url(state, request),
            "effective_share_base_url": _effective_share_base_url(state),
            "effective_share_link_base": _effective_share_link_base(state, request),
        }
    )


# ── /api/v1/network ─────────────────────────────────────────────────────────
def _merge_proxy(base: dict[str, Any], patch: dict[str, Any]) -> dict[str, Any]:
    next_proxy = dict(base)
    if "enabled" in patch and patch["enabled"] is not None:
        next_proxy["enabled"] = bool(patch["enabled"])
    if "proxy_type" in patch and patch["proxy_type"] is not None:
        next_proxy["proxy_type"] = str(patch["proxy_type"])
    if "host" in patch and patch["host"] is not None:
        next_proxy["host"] = str(patch["host"]).strip()
    if "port" in patch and patch["port"] is not None:
        try:
            next_proxy["port"] = int(patch["port"])
        except (TypeError, ValueError):
            pass
    if "username" in patch and patch["username"] is not None:
        next_proxy["username"] = str(patch["username"])
    # Secrets: overwrite only when non-empty (empty = keep existing).
    if patch.get("password"):
        next_proxy["password"] = str(patch["password"])
    if patch.get("secret"):
        next_proxy["secret"] = str(patch["secret"])
    return next_proxy


def _merge_vpn(base: dict[str, Any], patch: dict[str, Any]) -> dict[str, Any]:
    next_vpn = dict(base)
    bool_keys = ("enabled", "adaptive_polling", "flood_wait_respect", "auto_detect_vpn")
    for key in bool_keys:
        if key in patch and patch[key] is not None:
            next_vpn[key] = bool(patch[key])
    clamped_int = {
        "retry_attempts": (0, 5),
        "retry_base_backoff_ms": (500, 5000),
        "retry_max_backoff_ms": (8000, 60000),
        "polling_min_sec": (10, 30),
        "polling_max_sec": (45, 120),
        "dc_fallback_attempts": (1, 4),
        "peer_cache_size": (100, 2000),
        "chunk_size_kb": (64, 512),
    }
    for key, (low, high) in clamped_int.items():
        if key in patch and patch[key] is not None:
            next_vpn[key] = int(_clamp(patch[key], low, high))
    if "timeout_multiplier" in patch and patch["timeout_multiplier"] is not None:
        next_vpn["timeout_multiplier"] = _clamp(patch["timeout_multiplier"], 1, 5)
    if "preferred_dc" in patch and patch["preferred_dc"] is not None:
        next_vpn["preferred_dc"] = str(patch["preferred_dc"])
    for key in ("bandwidth_limit_up_kbs", "bandwidth_limit_down_kbs"):
        if key in patch and patch[key] is not None:
            try:
                next_vpn[key] = int(patch[key])
            except (TypeError, ValueError):
                pass
    if "keep_alive_interval_sec" in patch and patch["keep_alive_interval_sec"] is not None:
        try:
            v = int(patch["keep_alive_interval_sec"])
        except (TypeError, ValueError):
            v = 0
        next_vpn["keep_alive_interval_sec"] = 0 if v == 0 else int(_clamp(v, 30, 120))
    return next_vpn


@router.get("/network")
async def get_network(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    store = SettingsStore(state.settings.data_dir)
    return JSONResponse(store.network_view())


@router.put("/network")
async def put_network(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    try:
        body = await request.json()
    except ValueError:
        return api_error("BAD_REQUEST", "invalid JSON body", 400)
    store = SettingsStore(state.settings.data_dir)
    data = store.network.load()
    proxy = dict(data.get("proxy") or DEFAULT_PROXY)
    vpn = dict(data.get("vpn") or DEFAULT_VPN)
    if isinstance(body.get("proxy"), dict):
        proxy = _merge_proxy(proxy, body["proxy"])
    if isinstance(body.get("vpn"), dict):
        vpn = _merge_vpn(vpn, body["vpn"])
    if proxy.get("enabled") and not str(proxy.get("host") or "").strip():
        return api_error("INVALID_CONFIG", "Proxy enabled but host is empty", 400)
    try:
        store.network.save({"proxy": proxy, "vpn": vpn})
    except OSError as exc:
        return api_error("SAVE_FAILED", str(exc), 500)
    # Redacted view (password never exposed).
    public_proxy = dict(proxy)
    public_proxy["password_set"] = bool(public_proxy.get("password"))
    public_proxy["password"] = ""
    return JSONResponse({"proxy": public_proxy, "vpn": vpn})


# ── key rotation ────────────────────────────────────────────────────────────
@router.post("/admin/rotate-keys")
async def rotate_keys(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    client_ip = request.client.host if request.client else "unknown"
    try:
        new_ring = state.key_rotation.rotate_key(actor=f"admin:{client_ip}")
    except Exception as exc:
        return api_error("ROTATION_FAILED", str(exc), 500)
    return JSONResponse(
        {
            "ok": True,
            "last_rotated_at": new_ring["last_rotated_at"],
            "retired_keys_count": len(new_ring["retired_keys"]),
        }
    )


# ── audit log query ────────────────────────────────────────────────────────
@router.get("/admin/audit")
async def query_audit(
    request: Request,
    since: Optional[str] = Query(None, description="ISO timestamp filter"),
    event: Optional[str] = Query(None, description="Event type filter"),
    actor: Optional[str] = Query(None, description="Actor filter"),
    limit: int = Query(100, ge=1, le=1000),
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    log_path = state.settings.data_dir / "audit.log"
    entries = query_audit_log(
        log_path,
        since=since,
        event_type=event,
        actor=actor,
        limit=limit,
    )
    return JSONResponse(entries)
