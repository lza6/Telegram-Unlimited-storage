"""tg-disk compatible legacy endpoints (form/multipart, plain-text errors).

Covers: ``/verify``, ``/upload``, ``/upload_chunk``, ``/upload_status``,
``/merge_chunks``, ``/upload_progress_token``, ``/upload_events`` (SSE) and
``/upload_ws`` (WebSocket). Error strings reproduce the Rust backend byte for
byte ("密码错误", "missing chunk", "session not found", …).
"""

from __future__ import annotations

import hashlib
import hmac
import json
import re
import time
from typing import Any
from urllib.parse import quote

from fastapi import APIRouter, Form, Request, UploadFile, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse, PlainTextResponse, Response, StreamingResponse

from .. import links
from ..settings_store import SettingsStore
from ..state import AppState

router = APIRouter(tags=["legacy"])

OWNER_WEB = "system:web"
_MAX_TOTAL_CHUNKS = 10000
_SESSION_TTL_SECS = 86400 * 7  # 7 days (Rust create_upload_session)


def _replay_idempotency(cached: tuple) -> Response:
    """Replay a cached idempotency response."""
    status, media_type, body = cached
    return Response(content=body, status_code=status, media_type=media_type)


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


# ── password helpers (no lockout; /verify handles lockout separately) ───────
def _effective_pwd(state: AppState) -> str | None:
    return state.authenticator._effective_access_pwd()


def _pwd_form_ok(state: AppState, pwd: str) -> bool:
    expected = _effective_pwd(state)
    if not expected:
        return False
    # Rust trims the STORED value only, not the input.
    return hmac.compare_digest(pwd, expected.strip())


def _pwd_header_ok(state: AppState, request: Request) -> bool:
    expected = _effective_pwd(state)
    header = request.headers.get("X-Access-Pwd")
    if not expected or header is None:
        return False
    return hmac.compare_digest(header, expected)


def _host_base(state: AppState, request: Request) -> str:
    host = request.headers.get("host")
    return SettingsStore(state.settings.data_dir).share_base_url(
        state.settings.base_url, host
    )


def _flood_wait_seconds(exc: BaseException) -> int | None:
    secs = getattr(exc, "seconds", None)
    if isinstance(secs, int):
        return secs
    match = re.search(r"FLOOD_WAIT[_: ]*(\d+)", str(exc))
    return int(match.group(1)) if match else None


def _flood_response(exc: BaseException) -> JSONResponse | None:
    secs = _flood_wait_seconds(exc)
    if secs is None:
        return None
    return JSONResponse(
        {"error": {"code": "FLOOD_WAIT", "retry_after": secs}},
        status_code=503,
        headers={"Retry-After": str(secs)},
    )


# ── upload transport (bot vs user) ──────────────────────────────────────────
async def _upload_bytes(
    state: AppState, folder_id: int | None, data: bytes, filename: str, caption: str = ""
) -> int:
    mode = state.effective_transport_mode()
    if mode == "bot":
        result = await state.bot.upload_bytes(data, filename, caption=caption)
        state.storage.record_bot_file(
            message_id=result.message_id,
            telegram_file_id=result.telegram_file_id,
            file_name=filename,
            file_size=len(data),
            caption=caption or None,
            bot_pool_index=0,
        )
        return result.message_id
    return await state.telegram.upload_bytes(folder_id, data, filename, caption=caption)


# ── download link issuance (Rust issue_upload_download_link) ────────────────
def _issue_upload_link(
    state: AppState,
    base: str,
    folder_id: int | None,
    message_id: int,
    filename: str,
    file_size: int,
    merged: bool,
) -> tuple[str, str]:
    """Returns (download_url, file_id); raises ValueError if no mode enabled."""
    settings = state.settings
    ttl = settings.upload_link_ttl_secs
    if len(settings.download_signing_secret) >= 32:
        url, _ = links.presigned_url(
            base, settings.download_signing_secret, message_id, folder_id, OWNER_WEB, ttl
        )
        return url, str(message_id)
    if settings.upload_share_ttl_hours > 0:
        token = links.new_share_token()
        expires_at = int(time.time()) + settings.upload_share_ttl_hours * 3600
        state.storage.create_share(
            share_id=token, folder_id=folder_id, message_id=message_id,
            file_name=filename, file_size=file_size, password_hash=None,
            password_salt=None, expires_at=expires_at, owner_id=OWNER_WEB,
        )
        return f"{base}/d/{token}", str(message_id)
    if settings.public_file_id_download:
        if merged:
            return f"{base}/d?file_id={message_id}", str(message_id)
        return (
            f"{base}/d?file_id={message_id}&filename={quote(filename)}",
            str(message_id),
        )
    raise ValueError(
        "No download link mode: set DOWNLOAD_SIGNING_SECRET or UPLOAD_SHARE_TTL_HOURS"
    )


def _emit_chunk_progress(state: AppState, session_id: str, filename: str) -> None:
    session = state.storage.get_upload_session(session_id)
    if not session:
        return
    chunks = state.storage.list_upload_chunks(session_id)
    uploaded = sum(1 for c in chunks if c.get("status") == "uploaded")
    state.transfers.ensure_progress(
        session_id, filename or session["filename"], session["total_chunks"]
    )
    state.transfers.update_progress(
        session_id, uploaded_chunks=uploaded, status=session["status"]
    )


# ── /verify ─────────────────────────────────────────────────────────────────
@router.post("/verify")
async def verify(request: Request, pwd: str = Form("")):
    state = get_state(request)
    client_ip = request.client.host if request.client else "unknown"
    guard = state.authenticator.guard
    try:
        guard.check(client_ip)
    except Exception:  # noqa: BLE001 — lockout raises HTTPException(429)
        secs = guard.window_secs
        return PlainTextResponse(
            f"登录尝试过多，请 {secs} 秒后重试",
            status_code=429,
            headers={"Retry-After": str(secs)},
        )
    if _pwd_form_ok(state, pwd) or _pwd_header_ok(state, request):
        guard.clear(client_ip)
        return PlainTextResponse("ok")
    guard.record_failure(client_ip)
    return PlainTextResponse("密码错误", status_code=401)


# ── /upload (single file) ───────────────────────────────────────────────────
@router.post("/upload")
async def legacy_upload(
    request: Request,
    file: UploadFile | None = None,
    pwd: str = Form(""),
    folder_id: str | None = Form(None),
):
    state = get_state(request)
    if not (_pwd_form_ok(state, pwd) or _pwd_header_ok(state, request)):
        return PlainTextResponse("密码错误", status_code=401)

    # Idempotency: replay cached response for repeated requests with the same key.
    idem_key = request.headers.get("Idempotency-Key", "").strip()
    if idem_key:
        cached = state.transfers.idempotency_get(idem_key)
        if cached is not None:
            return _replay_idempotency(cached)

    if file is None or not file.filename:
        return PlainTextResponse("missing file", status_code=400)
    if not state.transfers.try_acquire_file_slot():
        return PlainTextResponse("upload busy", status_code=503, headers={"Retry-After": "3"})

    # Serialise concurrent retries with the same idempotency key.
    lock = state.transfers.idempotency_lock(idem_key) if idem_key else None
    if lock is not None:
        async with lock:
            # Double-check cache after acquiring the lock (another request may
            # have completed while we waited).
            if idem_key:
                cached = state.transfers.idempotency_get(idem_key)
                if cached is not None:
                    state.transfers.release_file_slot()
                    return _replay_idempotency(cached)
            response = await _do_legacy_upload(state, file, folder_id, request)
    else:
        response = await _do_legacy_upload(state, file, folder_id, request)

    # Cache the response so future retries get an instant replay.
    if idem_key:
        body: Any
        if isinstance(response, PlainTextResponse):
            body = response.body.decode("utf-8") if hasattr(response.body, "decode") else str(response.body)
        elif isinstance(response, JSONResponse):
            body = response.body
        else:
            body = None
        if body is not None:
            state.transfers.idempotency_put(
                idem_key,
                response.status_code,
                dict(response.headers),
                body,
            )

    return response


async def _do_legacy_upload(
    state: AppState,
    file: UploadFile,
    folder_id: str | None,
    request: Request,
) -> PlainTextResponse | JSONResponse:
    try:
        data = await file.read()
        max_bytes = state.settings.max_upload_size_mb * 1024 * 1024
        if max_bytes > 0 and len(data) > max_bytes:
            return PlainTextResponse(
                f"file exceeds {state.settings.max_upload_size_mb} MB limit",
                status_code=413,
            )
        fid: int | None = None
        if folder_id not in (None, "", "null"):
            try:
                fid = int(folder_id)
            except ValueError:
                fid = None
        filename = file.filename
        message_id = await _upload_bytes(state, fid, data, filename)
    except Exception as exc:  # noqa: BLE001
        flood = _flood_response(exc)
        if flood:
            return flood
        return PlainTextResponse("internal error", status_code=500)
    finally:
        state.transfers.release_file_slot()

    state.storage.upsert_file_asset(message_id, fid, OWNER_WEB, filename, len(data))
    base = _host_base(state, request)
    try:
        download_url, file_id = _issue_upload_link(
            state, base, fid, message_id, filename, len(data), merged=False
        )
    except ValueError as exc:
        return PlainTextResponse(str(exc), status_code=500)
    return JSONResponse(
        {"filename": filename, "file_id": file_id, "download_url": download_url}
    )


# ── /upload_chunk ───────────────────────────────────────────────────────────
@router.post("/upload_chunk")
async def upload_chunk(
    request: Request,
    chunk: UploadFile | None = None,
    pwd: str = Form(""),
    chunk_index: str = Form(""),
    total_chunks: str = Form(""),
    filename: str = Form(""),
    session_id: str = Form(""),
):
    state = get_state(request)
    if not (_pwd_form_ok(state, pwd) or _pwd_header_ok(state, request)):
        return PlainTextResponse("密码错误", status_code=401)
    if not state.transfers.try_acquire_chunk_slot():
        return PlainTextResponse("upload busy", status_code=503, headers={"Retry-After": "5"})
    try:
        data = await chunk.read() if chunk is not None else b""
        if not data:
            return PlainTextResponse("missing chunk", status_code=400)
        max_chunk = state.settings.chunk_size_bytes
        if len(data) > max_chunk:
            return PlainTextResponse(
                f"chunk exceeds {state.settings.chunk_size_mb} MB limit", status_code=413
            )
        try:
            idx = int(chunk_index)
        except ValueError:
            return PlainTextResponse("invalid chunk_index", status_code=400)
        try:
            total = int(total_chunks)
        except ValueError:
            return PlainTextResponse("invalid total_chunks", status_code=400)
        if idx < 0 or total <= 0 or idx >= total or total > _MAX_TOTAL_CHUNKS:
            return PlainTextResponse("invalid chunk parameters", status_code=400)
        if not session_id:
            return PlainTextResponse("missing session_id", status_code=400)
        if not filename:
            return PlainTextResponse("missing filename", status_code=400)

        sha256_hash = hashlib.sha256(data).hexdigest()
        # Idempotent session creation (pre-creates pending chunk rows).
        state.storage.create_upload_session(
            session_id, filename, total, int(time.time()) + _SESSION_TTL_SECS
        )
        # Retry-safe: identical chunk already recorded → return its receipt.
        existing = state.storage.get_upload_chunk(session_id, idx)
        if existing and existing.get("status") == "uploaded":
            if existing.get("sha256") == sha256_hash and existing.get("file_id"):
                _emit_chunk_progress(state, session_id, filename)
                return JSONResponse(
                    {"file_id": existing["file_id"], "sha256": sha256_hash}
                )
            return PlainTextResponse("chunk content conflicts with session", status_code=409)

        caption = f"blob [{idx}/{total}] - {filename}"
        message_id = await _upload_bytes(state, None, data, "blob", caption=caption)
        file_id = str(message_id)
        state.storage.record_upload_chunk(session_id, idx, file_id, sha256_hash)
        _emit_chunk_progress(state, session_id, filename)
        return JSONResponse({"file_id": file_id, "sha256": sha256_hash})
    except Exception as exc:  # noqa: BLE001
        flood = _flood_response(exc)
        if flood:
            return flood
        return PlainTextResponse("internal error", status_code=500)
    finally:
        state.transfers.release_chunk_slot()


# ── progress token auth ─────────────────────────────────────────────────────
def _progress_auth_ok(state: AppState, request: Request) -> bool:
    session_id = request.query_params.get("session_id", "")
    token = request.query_params.get("token", "")
    exp_raw = request.query_params.get("exp")
    if not exp_raw:
        return False
    try:
        exp = int(exp_raw)
    except ValueError:
        return False
    pwd = _effective_pwd(state)
    if not pwd:
        return False
    return links.verify_progress_token(pwd, session_id, exp, token)


@router.get("/upload_status")
async def upload_status(request: Request):
    state = get_state(request)
    if not _progress_auth_ok(state, request):
        return PlainTextResponse(
            "missing or invalid upload progress auth", status_code=401
        )
    session_id = request.query_params.get("session_id", "")
    if not session_id:
        return PlainTextResponse("missing session_id", status_code=400)
    session = state.storage.get_upload_session(session_id)
    if not session:
        return PlainTextResponse("session not found", status_code=404)
    chunks = state.storage.list_upload_chunks(session_id)
    uploaded_count = sum(1 for c in chunks if c.get("status") == "uploaded")
    file_id: str | None = None
    download_url: str | None = None
    if session["status"] == "completed" and session.get("manifest_file_id"):
        try:
            mid = int(session["manifest_file_id"])
            base = _host_base(state, request)
            download_url, file_id = _issue_upload_link(
                state, base, None, mid, session["filename"], 0, merged=True
            )
        except (ValueError, TypeError):
            pass
    payload: dict[str, Any] = {
        "session_id": session_id,
        "filename": session["filename"],
        "total_chunks": session["total_chunks"],
        "uploaded_chunks": uploaded_count,
        "status": session["status"],
        "chunks": [
            {
                "chunk_index": c["chunk_index"],
                "status": c["status"],
                "sha256": c.get("sha256"),
            }
            for c in chunks
        ],
    }
    if file_id is not None:
        payload["file_id"] = file_id
    if download_url is not None:
        payload["download_url"] = download_url
    return JSONResponse(payload)


# ── /merge_chunks ───────────────────────────────────────────────────────────
@router.post("/merge_chunks")
async def merge_chunks(
    request: Request,
    pwd: str = Form(""),
    filename: str = Form(""),
    session_id: str = Form(""),
    folder_id: str | None = Form(None),
    chunk_ids: str = Form(""),
):
    state = get_state(request)
    if not (_pwd_form_ok(state, pwd) or _pwd_header_ok(state, request)):
        return PlainTextResponse("密码错误", status_code=401)
    if not state.transfers.try_acquire_chunk_slot():
        return PlainTextResponse("upload busy", status_code=503, headers={"Retry-After": "5"})
    try:
        if not filename:
            return PlainTextResponse("missing filename", status_code=400)
        fid: int | None = None
        if folder_id not in (None, "", "null"):
            try:
                fid = int(folder_id)
            except ValueError:
                fid = None

        base = _host_base(state, request)
        # Idempotency: completed session → return existing manifest link.
        if session_id:
            session = state.storage.get_upload_session(session_id)
            if session and session.get("manifest_file_id"):
                try:
                    mid = int(session["manifest_file_id"])
                    download_url, file_id = _issue_upload_link(
                        state, base, fid, mid, filename, 0, merged=True
                    )
                    return JSONResponse(
                        {"filename": filename, "file_id": file_id, "download_url": download_url}
                    )
                except (ValueError, TypeError):
                    pass

        # Parse caller-supplied chunk_ids (JSON array of strings).
        caller_ids: list[str] = []
        if chunk_ids:
            try:
                parsed = json.loads(chunk_ids)
                caller_ids = [str(x) for x in parsed]
            except (ValueError, TypeError):
                return PlainTextResponse("chunk_ids invalid", status_code=400)
        elif not session_id:
            return PlainTextResponse("missing chunk_ids or session_id", status_code=400)

        # Session is authoritative when present.
        if session_id:
            db_chunks = state.storage.list_upload_chunks(session_id)
            final_ids: list[str] = []
            for c in db_chunks:
                if c.get("status") != "uploaded":
                    return PlainTextResponse(
                        f"chunk {c['chunk_index']} not uploaded yet", status_code=400
                    )
                if not c.get("file_id"):
                    return PlainTextResponse(
                        f"chunk {c['chunk_index']} missing file_id", status_code=400
                    )
                final_ids.append(c["file_id"])
            if not final_ids:
                return PlainTextResponse("no chunks found for session", status_code=400)
            if caller_ids and caller_ids != final_ids:
                return PlainTextResponse(
                    "chunk_ids do not match upload session", status_code=409
                )
        else:
            if not caller_ids:
                return PlainTextResponse("chunk_ids empty", status_code=400)
            final_ids = caller_ids

        manifest = filename + "\n" + "".join(f"{cid}\n" for cid in final_ids)
        manifest_id = await _upload_bytes(
            state, fid, manifest.encode("utf-8"), "fileAll.txt", caption=filename
        )
        if session_id:
            state.storage.update_upload_session_status(
                session_id, "completed", str(manifest_id)
            )
            state.transfers.ensure_progress(session_id, filename, len(final_ids))
            state.transfers.update_progress(
                session_id,
                uploaded_chunks=len(final_ids),
                total_chunks=len(final_ids),
                status="completed",
                file_id=str(manifest_id),
            )
        state.storage.upsert_file_asset(
            manifest_id, fid, OWNER_WEB, filename, len(manifest.encode("utf-8"))
        )
        download_url, file_id = _issue_upload_link(
            state, base, fid, manifest_id, filename, 0, merged=True
        )
        return JSONResponse(
            {"filename": filename, "file_id": file_id, "download_url": download_url}
        )
    except Exception as exc:  # noqa: BLE001
        flood = _flood_response(exc)
        if flood:
            return flood
        return PlainTextResponse("internal error", status_code=500)
    finally:
        state.transfers.release_chunk_slot()


# ── /upload_progress_token (admin only — X-Access-Pwd, NOT API key) ─────────
@router.post("/upload_progress_token")
async def upload_progress_token(request: Request):
    state = get_state(request)
    if not _pwd_header_ok(state, request):
        return api_error(
            "ADMIN_REQUIRED",
            "X-Access-Pwd is required to issue an upload progress token",
            401,
        )
    try:
        body = await request.json()
    except ValueError:
        body = {}
    session_id = str(body.get("session_id") or "").strip()
    if not session_id:
        return api_error("MISSING_SESSION", "session_id required", 400)
    pwd = _effective_pwd(state) or ""
    expires_at = int(time.time()) + links.PROGRESS_TOKEN_TTL_SECS
    token = links.issue_progress_token(pwd, session_id, expires_at)
    return JSONResponse(
        {"session_id": session_id, "token": token, "expires_at": expires_at}
    )


# ── progress event streaming ────────────────────────────────────────────────
def _legacy_event(snapshot: dict[str, Any]) -> dict[str, Any]:
    return {
        "session_id": snapshot.get("session_id"),
        "filename": snapshot.get("filename", ""),
        "uploaded_chunks": snapshot.get("uploaded_chunks", 0),
        "total_chunks": snapshot.get("total_chunks", 0),
        "status": snapshot.get("status", "active"),
    }


@router.get("/upload_events")
async def upload_events(request: Request):
    state = get_state(request)
    if not _progress_auth_ok(state, request):
        return PlainTextResponse(
            "missing or invalid upload progress auth", status_code=401
        )
    session_id = request.query_params.get("session_id", "")
    if not session_id:
        return PlainTextResponse("missing session_id", status_code=400)

    async def event_stream():
        bus = state.transfers.bus_for(session_id)
        queue = await bus.subscribe()
        try:
            progress = state.transfers.get_progress(session_id)
            if progress:
                snap = progress.snapshot()
                yield f"data: {json.dumps(_legacy_event(snap))}\n\n"
                if snap.get("status") in ("completed", "failed"):
                    return
            while True:
                snapshot = await queue.get()
                if snapshot is None:
                    break
                if snapshot.get("session_id") != session_id:
                    continue
                yield f"data: {json.dumps(_legacy_event(snapshot))}\n\n"
                if snapshot.get("status") in ("completed", "failed"):
                    break
        finally:
            bus.unsubscribe(queue)

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "Connection": "keep-alive"},
    )


@router.websocket("/upload_ws")
async def upload_ws(websocket: WebSocket):
    state: AppState = websocket.app.state.app
    params = websocket.query_params
    session_id = params.get("session_id", "")
    token = params.get("token", "")
    exp_raw = params.get("exp")
    pwd = _effective_pwd(state) or ""
    try:
        exp = int(exp_raw) if exp_raw else 0
    except ValueError:
        exp = 0
    if not session_id or not links.verify_progress_token(pwd, session_id, exp, token):
        await websocket.close(code=4401)
        return
    await websocket.accept()
    bus = state.transfers.bus_for(session_id)
    queue = await bus.subscribe()
    try:
        progress = state.transfers.get_progress(session_id)
        if progress:
            snap = progress.snapshot()
            await websocket.send_text(json.dumps(_legacy_event(snap)))
            if snap.get("status") in ("completed", "failed"):
                await websocket.close()
                return
        while True:
            snapshot = await queue.get()
            if snapshot is None:
                break
            if snapshot.get("session_id") != session_id:
                continue
            await websocket.send_text(json.dumps(_legacy_event(snapshot)))
            if snapshot.get("status") in ("completed", "failed"):
                await websocket.close()
                break
    except WebSocketDisconnect:
        pass
    finally:
        bus.unsubscribe(queue)
