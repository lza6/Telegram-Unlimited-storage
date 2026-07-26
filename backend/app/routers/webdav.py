"""WebDAV protocol — exposes Telegram Drive files to system file managers.

Implements a minimal WebDAV subset (RFC 4918):
- PROPFIND: list collection / resource properties (Depth: 0/1/infinity)
- GET: stream file bytes (Range supported via resolve_download)
- PUT: upload bytes through the active transport
- DELETE: remove file or collection
- MKCOL: create a folder (user mode only; bot mode has no folders)
- OPTIONS: advertise DAV class 1 capabilities

Auth mirrors the REST API: ``X-Access-Pwd`` or ``X-API-Key`` headers, OR
HTTP Basic where username is ignored and password carries the same credential.
Gated by ``WEBDAV_ENABLED`` (config). Mounted under ``/webdav``.
"""

from __future__ import annotations

import base64
import logging
import time
from typing import Optional
from urllib.parse import quote, unquote
from xml.etree.ElementTree import Element, tostring

from fastapi import APIRouter, Request, Response
from fastapi.responses import PlainTextResponse, StreamingResponse

from ..downloads import content_disposition, parse_range_header, resolve_download
from ..state import AppState

logger = logging.getLogger("telegram_drive.webdav")

router = APIRouter(tags=["webdav"])

DAV_NS = "DAV:"


def _ns(tag: str) -> str:
    return f"{{{DAV_NS}}}{tag}"


def _elem(tag: str, text: str = "") -> Element:
    el = Element(_ns(tag))
    if text:
        el.text = text
    return el


def get_state(request: Request) -> AppState:
    return request.app.state.app


def _client_ip(request: Request) -> str:
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"


def _auth_via_basic(state: AppState, request: Request) -> bool:
    ip = _client_ip(request)
    try:
        state.authenticator.guard.check(ip)
    except Exception:
        return False  # locked out — reject immediately
    header = request.headers.get("authorization", "")
    if not header.lower().startswith("basic "):
        return False
    try:
        decoded = base64.b64decode(header[6:].strip()).decode("utf-8", "replace")
    except Exception:
        return False
    if ":" not in decoded:
        return False
    _user, _, password = decoded.partition(":")
    if not password:
        return False
    if state.authenticator.verify_access_pwd(password, ip):
        return True
    if state.authenticator.verify_api_key(password) is not None:
        return True
    # Failure already recorded once by verify_access_pwd — do not double-count.
    return False


def _require_webdav_auth(state: AppState, request: Request) -> Optional[Response]:
    ip = _client_ip(request)
    # Determine if the client is already locked out
    is_locked = False
    try:
        state.authenticator.guard.check(ip)
    except Exception:
        is_locked = True

    # If already locked out, return 429
    if is_locked:
        return Response(
            content="Too many failed attempts — locked out",
            status_code=429,
        )

    # Try normal authentication
    auth_succeeded = False
    try:
        state.authenticator.require_auth(request)
        auth_succeeded = True
    except Exception:
        pass
    if not auth_succeeded:
        auth_succeeded = _auth_via_basic(state, request)

    if auth_succeeded:
        return None

    # Authentication failed. Return 401
    return Response(
        content="WebDAV authentication required",
        status_code=401,
        headers={"WWW-Authenticate": 'Basic realm="Telegram Drive WebDAV"'},
    )


def _safe_segments(rel: str) -> list[str]:
    raw = unquote(rel)
    if raw.startswith("/webdav"):
        raw = raw[len("/webdav") :]
    parts: list[str] = []
    for seg in raw.split("/"):
        if seg in ("", "."):
            continue
        if seg == "..":
            continue
        parts.append(seg)
    return parts


def _rfc1123(ts: int) -> str:
    return time.strftime("%a, %d %b %Y %H:%M:%S GMT", time.gmtime(ts))


def _dav_response(href: str, props: dict[str, str], status: str = "HTTP/1.1 200 OK") -> Element:
    resp = _elem("response")
    resp.append(_elem("href", href))
    ps = _elem("propstat")
    prop = _elem("prop")
    for k, v in props.items():
        prop.append(_elem(k, v))
    ps.append(prop)
    ps.append(_elem("status", status))
    resp.append(ps)
    return resp


def _propstat_response(
    href: str, is_collection: bool, name: str, size: int, created_at: int
) -> Element:
    """Build one <D:response> block for a PROPFIND multistatus."""
    if is_collection:
        props: dict[str, str] = {
            "displayname": name,
            "getlastmodified": _rfc1123(created_at or int(time.time())),
        }
        resp = _dav_response(href, props)
        # Add collection element
        rt_elem = Element(_ns("resourcetype"))
        rt_elem.append(Element(_ns("collection")))
        for ps in resp.findall(f".//{{{DAV_NS}}}propstat"):
            prop_el = ps.find(f".//{{{DAV_NS}}}prop")
            if prop_el is not None:
                prop_el.append(rt_elem)
        return resp
    else:
        props = {
            "displayname": name,
            "resourcetype": "",
            "getcontentlength": str(size),
            "getcontenttype": "application/octet-stream",
            "getlastmodified": _rfc1123(created_at or int(time.time())),
        }
        return _dav_response(href, props)


def _multistatus(responses: list[Element]) -> str:
    ms = _elem("multistatus")
    for r in responses:
        ms.append(r)
    return tostring(ms, encoding="unicode", xml_declaration=True)


async def _resolve_file_id(state: AppState, segments: list[str]) -> Optional[int]:
    if not segments:
        return None
    target = segments[-1]
    mode = state.effective_transport_mode()
    if mode == "bot":
        rows = state.storage.search_bot_files(target, 50)
        for row in rows:
            if row["file_name"] == target:
                return int(row["message_id"])
        files = state.storage.list_bot_files(limit=1000)
        for f in files:
            if (f.get("file_name") or f"file_{f['message_id']}") == target:
                return int(f["message_id"])
        return None
    if not state.user_configured:
        return None
    folder_id: Optional[int] = None
    try:
        files = await state.telegram.list_files(folder_id)
    except Exception:
        return None
    for f in files:
        if f.name == target:
            return int(f.id)
    return None


# ── OPTIONS: advertise WebDAV support ───────────────────────────────────────
@router.options("/webdav/{rel:path}")
async def webdav_options(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    return Response(
        status_code=200,
        headers={
            "DAV": "1",
            "Allow": "OPTIONS, PROPFIND, GET, HEAD, PUT, DELETE, MKCOL",
            "MS-Author-Via": "DAV",
        },
    )


@router.options("/webdav")
async def webdav_options_root(request: Request) -> Response:
    return await webdav_options("", request)


# ── PROPFIND: list resources ─────────────────────────────────────────────────
async def _do_propfind(state: AppState, segments: list[str], depth: str) -> Response:
    mode = state.effective_transport_mode()
    is_root = len(segments) == 0

    responses: list[Element] = []

    # Root collection
    root_resp = _propstat_response("/webdav/", True, "Telegram Drive", 0, int(time.time()))
    responses.append(root_resp)

    if depth == "0":
        if is_root:
            xml = _multistatus(responses)
            return Response(content=xml, media_type="application/xml; charset=utf-8", status_code=207)
        msg_id = await _resolve_file_id(state, segments)
        if msg_id is None:
            xml = _multistatus([])
            return Response(content=xml, media_type="application/xml; charset=utf-8", status_code=207)
        name = segments[-1]
        size = 0
        created = int(time.time())
        if mode == "bot":
            row = state.storage.get_bot_file(msg_id)
            if row:
                name = row["file_name"] or name
                size = int(row["file_size"] or 0)
                created = int(row["created_at"] or 0)
        else:
            try:
                message = await state.telegram.get_message(None, msg_id)
                meta = state.telegram.message_to_metadata(message, None)
                if meta:
                    name = meta.name
                    size = meta.size
                    created = meta.created_at
            except Exception:
                pass
        fr = _propstat_response(f"/webdav/{quote(name)}", False, name, size, created)
        xml = _multistatus([fr])
        return Response(content=xml, media_type="application/xml; charset=utf-8", status_code=207)

    # Depth: 1 or infinity — list files
    if mode == "bot":
        rows = state.storage.list_bot_files(limit=1000)
        for row in rows:
            fname = row.get("file_name") or f"file_{row['message_id']}"
            fsize = int(row.get("file_size") or 0)
            fcreated = int(row.get("created_at") or 0)
            responses.append(
                _propstat_response(
                    f"/webdav/{quote(fname)}", False, fname, fsize, fcreated
                )
            )
    elif state.user_configured:
        try:
            files = await state.telegram.list_files(None)
            for f in files:
                fname = f.name or f"file_{f.id}"
                fsize = int(getattr(f, "size", 0) or 0)
                fcreated = int(getattr(f, "created_at", 0) or 0)
                responses.append(
                    _propstat_response(
                        f"/webdav/{quote(fname)}", False, fname, fsize, fcreated
                    )
                )
        except Exception:
            pass

    xml = _multistatus(responses)
    return Response(content=xml, media_type="application/xml; charset=utf-8", status_code=207)


@router.api_route("/webdav/{rel:path}", methods=["PROPFIND"])
async def webdav_propfind(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    err = _require_webdav_auth(state, request)
    if err:
        return err
    depth = request.headers.get("depth", "infinity").lower()
    segments = _safe_segments(rel)
    return await _do_propfind(state, segments, depth)


@router.api_route("/webdav", methods=["PROPFIND"])
async def webdav_propfind_root(request: Request) -> Response:
    return await webdav_propfind("", request)


# ── GET / HEAD: stream file ──────────────────────────────────────────────────
async def _do_get(state: AppState, segments: list[str], request: Request) -> Response:
    if not segments:
        return PlainTextResponse("Telegram Drive WebDAV root", status_code=200)
    msg_id = await _resolve_file_id(state, segments)
    if msg_id is None:
        return PlainTextResponse("Not found", status_code=404)
    range_header = request.headers.get("range")
    try:
        target = await resolve_download(state, None, msg_id, segments[-1])
    except LookupError:
        return PlainTextResponse("Not found", status_code=404)
    except Exception:
        return PlainTextResponse("Download failed", status_code=500)

    total = target.size
    headers = {
        "Accept-Ranges": "bytes",
        "Content-Disposition": content_disposition(target.filename, target.mime_type),
    }
    ranged = parse_range_header(range_header, total) if total > 0 else None
    if ranged and total > 0:
        start, end = ranged
        target = await resolve_download(
            state, None, msg_id, segments[-1], offset=start, length=end - start + 1
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


@router.get("/webdav/{rel:path}")
async def webdav_get(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    err = _require_webdav_auth(state, request)
    if err:
        return err
    return await _do_get(state, _safe_segments(rel), request)


@router.get("/webdav")
async def webdav_get_root(request: Request) -> Response:
    return await webdav_get("", request)


@router.head("/webdav/{rel:path}")
async def webdav_head(rel: str, request: Request) -> Response:
    resp = await webdav_get(rel, request)
    if hasattr(resp, "body_iterator"):
        return Response(
            headers=dict(resp.headers),
            media_type=resp.media_type,
            status_code=resp.status_code,
        )
    return resp


# ── PUT: upload bytes ─────────────────────────────────────────────────────────
@router.put("/webdav/{rel:path}")
async def webdav_put(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    err = _require_webdav_auth(state, request)
    if err:
        return err
    segments = _safe_segments(rel)
    if not segments:
        return PlainTextResponse("Cannot PUT to root collection", status_code=405)
    filename = segments[-1]
    ready = await state.is_ready()
    if not ready:
        return PlainTextResponse("Telegram transport is not ready", status_code=503)
    max_bytes = state.settings.max_upload_size_mb * 1024 * 1024
    content_length = request.headers.get("content-length")
    if content_length:
        try:
            if int(content_length) > max_bytes:
                return PlainTextResponse("Payload too large", status_code=413)
        except ValueError:
            pass

    if not state.transfers.try_acquire_file_slot():
        return PlainTextResponse(
            "Upload queue full",
            status_code=503,
            headers={"Retry-After": "3"},
        )
    try:
        data = await request.body()
        if len(data) > max_bytes:
            return PlainTextResponse("Payload too large", status_code=413)
        mode = state.effective_transport_mode()
        if mode == "bot":
            result = await state.bot.upload_bytes(data, filename)
            message_id = result.message_id
            state.storage.record_bot_file(
                message_id=message_id,
                telegram_file_id=result.telegram_file_id,
                file_name=filename,
                file_size=len(data),
                caption="",
                bot_pool_index=0,
            )
        else:
            message_id = await state.telegram.upload_bytes(None, data, filename)
    except Exception as exc:
        return PlainTextResponse(f"Upload failed: {exc}", status_code=500)
    finally:
        state.transfers.release_file_slot()

    owner = "system:web"
    state.storage.upsert_file_asset(
        message_id=message_id,
        folder_id=None,
        owner_id=owner,
        file_name=filename,
        file_size=len(data),
    )
    return Response(status_code=201, headers={"Location": f"/webdav/{quote(filename)}"})


# ── DELETE: remove file ───────────────────────────────────────────────────────
@router.delete("/webdav/{rel:path}")
async def webdav_delete(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    err = _require_webdav_auth(state, request)
    if err:
        return err
    segments = _safe_segments(rel)
    if not segments:
        return PlainTextResponse("Cannot delete root collection", status_code=405)
    msg_id = await _resolve_file_id(state, segments)
    if msg_id is None:
        return PlainTextResponse("Not found", status_code=404)
    mode = state.effective_transport_mode()
    try:
        if mode == "bot":
            try:
                await state.bot.delete_message(msg_id)
            except Exception:
                pass
            state.storage.delete_bot_file(msg_id)
        else:
            await state.telegram.delete_files(None, [msg_id])
    except Exception:
        return PlainTextResponse("Delete failed", status_code=500)
    return Response(status_code=204)


# ── MKCOL: create folder (user mode only) ────────────────────────────────────
@router.api_route("/webdav/{rel:path}", methods=["MKCOL"])
async def webdav_mkcol(rel: str, request: Request) -> Response:
    state = get_state(request)
    if not state.settings.webdav_enabled:
        return PlainTextResponse("WebDAV disabled", status_code=404)
    err = _require_webdav_auth(state, request)
    if err:
        return err
    segments = _safe_segments(rel)
    if not segments:
        return PlainTextResponse("Collection already exists", status_code=405)
    if state.effective_transport_mode() == "bot":
        return PlainTextResponse(
            "Folder operations are not supported in bot mode",
            status_code=403,
        )
    folder_name = segments[-1]
    try:
        await state.telegram.create_folder(folder_name)
    except Exception as exc:
        return PlainTextResponse(f"MKCOL failed: {exc}", status_code=500)
    return Response(status_code=201, headers={"Location": f"/webdav/{quote(folder_name)}/"})


@router.api_route("/webdav", methods=["MKCOL"])
async def webdav_mkcol_root(request: Request) -> Response:
    return PlainTextResponse("Collection already exists", status_code=405)


# ── Unsupported methods ──────────────────────────────────────────────────────
@router.api_route("/webdav/{rel:path}", methods=["PROPPATCH", "MOVE", "COPY", "LOCK", "UNLOCK"])
async def webdav_unsupported(rel: str, request: Request) -> Response:
    return PlainTextResponse("Method not supported", status_code=405)
