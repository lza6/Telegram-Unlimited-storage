"""Share management + public download endpoints.

Covers two surfaces:
- ``/api/v1/shares`` CRUD (authenticated, owner-scoped).
- Public download routes ``/d/{token}``, ``/d/{token}/verify``, ``/d/signed``,
  ``/d`` and ``/stream/{folder_id}/{message_id}`` (token / cookie / signature
  gated — no auth header required).

Error envelopes reproduce the Rust backend exactly:
- API CRUD uses ``{"error":{"code","message"}}``.
- Public ``/d/*`` text errors are plain text ("Shared link not found", …).
"""

from __future__ import annotations

import hmac
import time
from html import escape as _html_escape
from typing import Any, Optional

from fastapi import APIRouter, Form, Request
from fastapi.responses import HTMLResponse, JSONResponse, PlainTextResponse, RedirectResponse, StreamingResponse

from .. import links, security
from ..downloads import content_disposition, parse_range_header, resolve_download
from ..settings_store import SettingsStore
from ..state import AppState

router = APIRouter(tags=["shares"])

# Brute-force limiter for password-protected share verify (5 attempts / 300s).
_VERIFY_MAX_ATTEMPTS = 5
_VERIFY_WINDOW_SECS = 300


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


def _share_base_url(state: AppState, request: Request) -> str:
    host = request.headers.get("host")
    return SettingsStore(state.settings.data_dir).share_base_url(
        state.settings.base_url, host
    )


def _owner_filter(state: AppState, identity) -> Optional[str]:
    """Tenant → scoped owner; admin (console/api) → None (see all)."""
    if identity.kind == "tenant":
        return f"tenant:{identity.tenant_id}"
    return None


def _to_share_info(share: dict[str, Any], base: str) -> dict[str, Any]:
    return {
        "id": share["id"],
        "file_name": share["file_name"],
        "file_size": share["file_size"],
        "created_at": share["created_at"],
        "expires_at": share["expires_at"],
        "has_password": bool(share.get("password_hash")),
        "link": f"{base}/d/{share['id']}",
    }


# ── CRUD ────────────────────────────────────────────────────────────────────
@router.get("/api/v1/shares")
async def list_shares(request: Request) -> JSONResponse:
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    owner = _owner_filter(state, identity)
    state.storage.cleanup_expired_shares()
    rows = state.storage.list_shares(owner)
    base = _share_base_url(state, request)
    # Rust filters revoked=0 in SQL; storage.list_shares does not — filter here.
    active = [r for r in rows if not r.get("revoked")]
    return JSONResponse([_to_share_info(r, base) for r in active])


@router.post("/api/v1/shares")
async def create_share(request: Request) -> JSONResponse:
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    try:
        body = await request.json()
    except ValueError:
        return api_error("BAD_REQUEST", "invalid JSON body", 400)
    message_id = body.get("message_id")
    file_name = (body.get("file_name") or "").strip()
    try:
        message_id = int(message_id)
    except (TypeError, ValueError):
        message_id = 0
    if message_id <= 0 or not file_name:
        return api_error(
            "BAD_REQUEST",
            "message_id must be positive and file_name is required",
            400,
        )
    folder_id = body.get("folder_id")
    folder_id = int(folder_id) if folder_id not in (None, "") else None
    file_size = int(body.get("file_size") or 0)
    password = (body.get("password") or "").strip()
    password_hash: Optional[str] = None
    password_salt: Optional[str] = None
    if password:
        password_hash, password_salt = security.hash_share_password(password)
    expiry_hours = body.get("expiry_hours")
    expires_at: Optional[int] = None
    if expiry_hours:
        try:
            hours = int(expiry_hours)
        except (TypeError, ValueError):
            hours = 0
        if hours > 0:
            expires_at = int(time.time()) + hours * 3600
    owner = _owner_filter(state, identity) or "system:web"
    token = links.new_share_token()
    share = state.storage.create_share(
        share_id=token,
        folder_id=folder_id,
        message_id=message_id,
        file_name=file_name,
        file_size=file_size,
        password_hash=password_hash,
        password_salt=password_salt,
        expires_at=expires_at,
        owner_id=owner,
    )
    base = _share_base_url(state, request)
    return JSONResponse(_to_share_info(share, base))


@router.delete("/api/v1/shares/{share_id}")
async def delete_share(share_id: str, request: Request) -> JSONResponse:
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    if identity.kind == "tenant":
        share = state.storage.get_share(share_id)
        if share is None:
            return JSONResponse({"error": "Share not found"}, status_code=404)
        expected_owner = f"tenant:{identity.tenant_id}"
        if share.get("owner_id") not in (expected_owner, None):
            return JSONResponse({"error": "Forbidden"}, status_code=403)
        state.storage.revoke_share(share_id)
        return JSONResponse({"revoked": True})
    # Admin (console / api): revoke any share.
    state.storage.revoke_share(share_id)
    return JSONResponse({"revoked": True})


# ── public download helpers ─────────────────────────────────────────────────
async def _stream_target(
    state: AppState,
    folder_id: Optional[int],
    message_id: int,
    filename_hint: Optional[str],
    range_header: Optional[str],
) -> StreamingResponse:
    target = await resolve_download(state, folder_id, message_id, filename_hint)
    total = target.size
    headers = {
        "Accept-Ranges": "bytes",
        "Content-Disposition": content_disposition(target.filename, target.mime_type),
    }
    ranged = parse_range_header(range_header, total) if total > 0 else None
    if ranged and total > 0:
        start, end = ranged
        target = await resolve_download(
            state, folder_id, message_id, filename_hint,
            offset=start, length=end - start + 1,
        )
        headers["Content-Range"] = f"bytes {start}-{end}/{total}"
        headers["Content-Length"] = str(end - start + 1)
        return StreamingResponse(
            target.stream, status_code=206, media_type=target.mime_type, headers=headers
        )
    if total > 0:
        headers["Content-Length"] = str(total)
    return StreamingResponse(
        target.stream, status_code=200, media_type=target.mime_type, headers=headers
    )


_PASSWORD_FORM = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Password Protected File - Telegram Drive</title>
<style>
  body {{ background:#182533; color:#e6edf3; font-family:system-ui,sans-serif;
         display:flex; align-items:center; justify-content:center; min-height:100vh; margin:0; }}
  .card {{ background:#202b36; padding:2rem; border-radius:12px; width:100%; max-width:380px;
          box-shadow:0 8px 32px rgba(0,0,0,.35); }}
  h1 {{ font-size:1.15rem; margin:0 0 .35rem; }}
  p {{ color:#9fb3c8; font-size:.85rem; margin:0 0 1.25rem; word-break:break-all; }}
  input {{ width:100%; box-sizing:border-box; padding:.65rem .75rem; border-radius:8px;
          border:1px solid #33475b; background:#182533; color:#e6edf3; font-size:.95rem; }}
  button {{ width:100%; margin-top:.9rem; padding:.7rem; border:0; border-radius:8px;
           background:#40a7e3; color:#08131f; font-weight:600; font-size:.95rem; cursor:pointer; }}
  button:hover {{ background:#5cb6ec; }}
  .error {{ color:#ff7b72; font-size:.85rem; margin:0 0 1rem; }}
</style>
</head>
<body>
  <div class="card">
    <h1>Password Protected File</h1>
    <p>{file_name}</p>
    {error_html}
    <form method="post" action="/d/{token}/verify">
      <input type="password" name="password" placeholder="Enter password" autofocus>
      <button type="submit">Verify &amp; Download</button>
    </form>
  </div>
</body>
</html>"""


def _password_form(token: str, file_name: str, error: Optional[str] = None) -> HTMLResponse:
    error_html = f'<p class="error">{_html_escape(error)}</p>' if error else ""
    html = _PASSWORD_FORM.format(
        file_name=_html_escape(file_name),
        token=_html_escape(token),
        error_html=error_html,
    )
    return HTMLResponse(html)


def _share_blocked_response(share: dict[str, Any]) -> Optional[PlainTextResponse]:
    """Returns a 404/410 response if the share is revoked or expired."""
    if share.get("revoked"):
        return PlainTextResponse("This shared link has been revoked", status_code=404)
    expires_at = share.get("expires_at")
    if expires_at and int(expires_at) < int(time.time()):
        return PlainTextResponse("This shared link has expired", status_code=410)
    return None


@router.get("/d/signed")
async def signed_download(
    request: Request,
    file_id: str = "",
    exp: int = 0,
    owner: str = "",
    sig: str = "",
    folder_id: Optional[str] = None,
    max_downloads: Optional[int] = None,
):
    state = get_state(request)
    secret = state.settings.download_signing_secret
    if len(secret) < 32:
        return api_error("PRESIGN_DISABLED", "Presigned downloads are not configured", 503)
    try:
        message_id = int(file_id)
    except ValueError:
        return api_error("BAD_REQUEST", "invalid file_id", 400)
    fid: Optional[int] = None
    if folder_id not in (None, "", "null"):
        try:
            fid = int(folder_id)
        except ValueError:
            fid = None
    canonical = links.presign_canonical(message_id, fid, exp, owner, max_downloads)
    if not links.verify_presign_signature(secret, canonical, sig):
        return api_error("INVALID_SIGNATURE", "Invalid or tampered download link", 403)
    if exp > 0 and int(time.time()) > exp:
        return api_error("LINK_EXPIRED", "This download link has expired", 410)
    if max_downloads is not None and max_downloads > 0:
        count = state.transfers.count_download(sig)
        if count > max_downloads:
            return api_error("DOWNLOAD_LIMIT_REACHED", "Download limit reached", 403)
    try:
        return await _stream_target(state, fid, message_id, None, request.headers.get("range"))
    except LookupError:
        return api_error("NOT_FOUND", "File not found", 404)
    except Exception:  # noqa: BLE001
        return api_error("DOWNLOAD_FAILED", "Download failed", 500)


@router.get("/d/{token}")
async def share_download(token: str, request: Request):
    state = get_state(request)
    share = state.storage.get_share(token)
    if share is None:
        return PlainTextResponse("Shared link not found", status_code=404)
    blocked = _share_blocked_response(share)
    if blocked:
        return blocked
    password_hash = share.get("password_hash")
    if password_hash:
        cookie = request.cookies.get(f"share_auth_{token}")
        if not cookie or not links.verify_share_cookie(token, password_hash, cookie):
            return _password_form(token, share["file_name"])
    try:
        return await _stream_target(
            state, share.get("folder_id"), share["message_id"],
            share["file_name"], request.headers.get("range"),
        )
    except LookupError:
        return PlainTextResponse("Shared link not found", status_code=404)
    except Exception:  # noqa: BLE001
        return PlainTextResponse("Download failed", status_code=500)


@router.post("/d/{token}/verify")
async def share_verify(token: str, request: Request, password: str = Form("")):
    state = get_state(request)
    share = state.storage.get_share(token)
    if share is None:
        return PlainTextResponse("Shared link not found", status_code=404)
    if share.get("revoked"):
        return PlainTextResponse("This shared link has been revoked", status_code=404)
    # Brute-force limiter keyed by token (stored on AppState for process safety).
    attempts = state.share_verify_attempts
    now = time.time()
    recent = [
        t for t in attempts.get(token, [])
        if now - t < _VERIFY_WINDOW_SECS
    ]
    if len(recent) >= _VERIFY_MAX_ATTEMPTS:
        attempts[token] = recent
        return PlainTextResponse("Too many attempts, try again later", status_code=429)
    password_hash = share.get("password_hash")
    if not password_hash:
        return PlainTextResponse("No password required for this link", status_code=400)
    if security.verify_share_password(password, password_hash, share.get("password_salt")):
        attempts.pop(token, None)
        cookie_val = links.share_cookie_value(token, password_hash)
        secure = request.url.scheme == "https"
        response = RedirectResponse(url=f"/d/{token}", status_code=302)
        response.set_cookie(
            f"share_auth_{token}",
            cookie_val,
            max_age=links.SHARE_COOKIE_MAX_AGE,
            path=f"/d/{token}",
            httponly=True,
            secure=secure,
            samesite="strict",
        )
        return response
    recent.append(now)
    state.share_verify_attempts[token] = recent
    return _password_form(token, share["file_name"], "Incorrect password. Please try again.")


def _raw_file_id_allowed(state: AppState, request: Request) -> bool:
    if state.settings.public_file_id_download:
        return True
    pwd = request.headers.get("X-Access-Pwd")
    if pwd:
        client_ip = request.client.host if request.client else "unknown"
        return state.authenticator.verify_access_pwd(pwd, client_ip)
    api_key = request.headers.get("X-API-Key")
    if api_key:
        return state.authenticator.verify_api_key(api_key) is not None
    return False


@router.get("/d")
async def legacy_download(
    request: Request,
    file_id: str = "",
    filename: Optional[str] = None,
    folder_id: Optional[str] = None,
):
    state = get_state(request)
    if not _raw_file_id_allowed(state, request):
        return api_error("RAW_FILE_ID_DISABLED", "Raw file_id downloads are disabled", 403)
    try:
        message_id = int(file_id)
    except ValueError:
        return PlainTextResponse("invalid file_id", status_code=400)
    fid: Optional[int] = None
    if folder_id not in (None, "", "null"):
        try:
            fid = int(folder_id)
        except ValueError:
            fid = None
    hint = filename if (filename and filename != "fileAll.txt") else None
    try:
        return await _stream_target(state, fid, message_id, hint, request.headers.get("range"))
    except LookupError:
        return PlainTextResponse("File not found", status_code=404)
    except Exception:  # noqa: BLE001
        return PlainTextResponse("Download failed", status_code=500)


@router.get("/stream/{folder_id}/{message_id}")
async def stream_media(folder_id: str, message_id: int, request: Request, token: str = ""):
    state = get_state(request)
    if not token or not hmac.compare_digest(token, state.stream_token):
        return PlainTextResponse("Invalid or missing stream token", status_code=403)
    fid: Optional[int]
    if folder_id.lower() in ("me", "home", "null"):
        fid = None
    else:
        try:
            fid = int(folder_id)
        except ValueError:
            return PlainTextResponse("Invalid folder ID", status_code=400)
    try:
        return await _stream_target(state, fid, message_id, None, request.headers.get("range"))
    except LookupError:
        return PlainTextResponse("File not found", status_code=404)
    except Exception:  # noqa: BLE001
        return PlainTextResponse("Download failed", status_code=500)
