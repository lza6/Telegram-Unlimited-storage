"""Telegram auth + transport mode endpoints.

Error envelope note: auth_routes in the Rust backend use a FLAT
``{"error": msg}`` envelope (not the code/message form) — reproduced here.
"""

from __future__ import annotations

from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from ..state import AppState

router = APIRouter(prefix="/api/v1", tags=["auth"])


class PhoneRequest(BaseModel):
    phone: str


class CodeRequest(BaseModel):
    code: str


class PasswordRequest(BaseModel):
    password: str


class TransportModeRequest(BaseModel):
    mode: str


def get_state(request: Request) -> AppState:
    return request.app.state.app


def flat_error(message: str, status_code: int = 400) -> JSONResponse:
    return JSONResponse({"error": message}, status_code=status_code)


@router.get("/auth/status")
async def auth_status(request: Request) -> dict:
    state = get_state(request)
    mode = state.effective_transport_mode()
    connected = False
    user: str | None = None
    hint: str | None = None
    if mode == "user":
        try:
            connected = await state.telegram.is_authorized()
            if connected and state.telegram.client is not None:
                me = await state.telegram.client.get_me()
                parts = [getattr(me, "first_name", ""), getattr(me, "last_name", "")]
                user = " ".join(p for p in parts if p) or getattr(me, "username", None)
        except Exception as exc:  # noqa: BLE001
            hint = str(exc)
    credentials_ok = state.user_configured or state.bot_configured
    result = {
        "connected": connected or (mode == "bot" and state.bot_configured),
        "user": user,
        "credentials_ok": credentials_ok,
        "transport_mode": mode,
        "bot_configured": state.bot_configured,
        "user_configured": state.user_configured,
    }
    if hint:
        result["hint"] = hint
    return result


@router.post("/auth/phone/request")
async def phone_request(
    body: PhoneRequest,
    request: Request,
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    result = await state.telegram.request_login_code(body.phone)
    if not result.success:
        return flat_error(result.error or "code request failed")
    return JSONResponse({"sent": True})


@router.post("/auth/phone/sign-in")
async def phone_sign_in(
    body: CodeRequest,
    request: Request,
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    result = await state.telegram.sign_in_with_code(body.code)
    if not result.success:
        return flat_error(result.error or "sign-in failed")
    if result.next_step == "password":
        return JSONResponse({"connected": False, "next_step": "password"})
    return JSONResponse({"connected": True})


@router.post("/auth/phone/password")
async def phone_password(
    body: PasswordRequest,
    request: Request,
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    result = await state.telegram.check_password(body.password)
    if not result.success:
        return flat_error(result.error or "2FA failed")
    return JSONResponse({"connected": True})


@router.post("/auth/qr/start")
async def qr_start(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    result = await state.telegram.qr_start()
    if not result.success:
        return flat_error(result.error or "QR start failed")
    url = result.next_step or ""
    return JSONResponse({"url": url, "authorized": False})


@router.get("/auth/qr/poll")
async def qr_poll(request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    result = await state.telegram.qr_poll()
    connected = result.next_step == "dashboard"
    return JSONResponse(
        {"connected": connected, "next_step": result.next_step or "waiting"}
    )


@router.get("/transport")
async def transport_info(request: Request) -> dict:
    state = get_state(request)
    state.authenticator.require_auth(request)
    modes = []
    if state.user_configured:
        modes.append("user")
    if state.bot_configured:
        modes.append("bot")
    if not modes:
        modes = ["user"]
    return {
        "active_mode": state.effective_transport_mode(),
        "default_mode": state.default_transport_mode,
        "bot_configured": state.bot_configured,
        "user_configured": state.user_configured,
        "available_modes": modes,
    }


@router.post("/transport/mode")
async def set_transport_mode(body: TransportModeRequest, request: Request) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    if body.mode not in ("bot", "user"):
        return flat_error("mode must be 'bot' or 'user'")
    if body.mode == "bot" and not state.bot_configured:
        return flat_error("Bot mode is not configured")
    state.active_transport_mode = body.mode
    # Persist like the Rust impl (transport_mode.json).
    from ..settings_store import SettingsStore

    SettingsStore(state.settings.data_dir).transport.save({"mode": body.mode})
    return JSONResponse({"ok": True, "transport_mode": body.mode})
