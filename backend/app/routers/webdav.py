"""WebDAV interface for Telegram Drive — flat file system, no folders.

Mount in macOS Finder:  Connect to Server → http://IP:1334/webdav/
Authentication:         username=anything, password=ACCESS_PWD
"""
from __future__ import annotations

import base64
import logging
import mimetypes
import time
from typing import Any, Optional
from urllib.parse import quote, unquote
from xml.etree.ElementTree import Element, SubElement, tostring

from fastapi import APIRouter, Request, Response
from fastapi.responses import JSONResponse, StreamingResponse

from ..state import AppState

logger = logging.getLogger("telegram_drive.webdav")

router = APIRouter(prefix="/webdav", tags=["webdav"])

DAV_NS = "DAV:"


# ── helpers ──────────────────────────────────────────────────────────────────

def _ns(tag: str) -> str:
    return f"{{{DAV_NS}}}{tag}"


def _elem(tag: str, text: str = "") -> Element:
    el = Element(_ns(tag))
    if text:
        el.text = text
    return el


def _dav_response(
    href: str,
    props: dict[str, str],
    status: str = "HTTP/1.1 200 OK",
) -> Element:
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


def _multistatus(responses: list[Element]) -> str:
    ms = _elem("multistatus")
    for r in responses:
        ms.append(r)
    return tostring(ms, encoding="unicode", xml_declaration=True)


def _rfc1123(ts: int) -> str:
    return time.strftime("%a, %d %b %Y %H:%M:%S GMT", time.gmtime(ts))


def _state(request: Request) -> AppState:
    return request.app.state.app


async def _authenticate(request: Request) -> Response | None:
    """Returns 401 Response on failure, None on success."""
    state = _state(request)
    # X-Access-Pwd header (from web UI)
    pwd = request.headers.get("X-Access-Pwd", "")
    if pwd and pwd == state.settings.access_pwd:
        return None
    # Basic Auth (Finder uses this)
    auth = request.headers.get("Authorization", "")
    if auth.startswith("Basic "):
        try:
            decoded = base64.b64decode(auth[6:]).decode("utf-8")
            _, password = decoded.split(":", 1)
            if password == state.settings.access_pwd:
                return None
        except Exception:
            pass
    return Response(
        status_code=401,
        headers={"WWW-Authenticate": 'Basic realm="Telegram Drive"'},
    )


# ── WebDAV methods ───────────────────────────────────────────────────────────


@router.api_route("/{path:path}", methods=["PROPFIND"])
async def propfind(request: Request, path: str) -> Response:
    """List all files (flat view, no folders)."""
    auth_err = await _authenticate(request)
    if auth_err:
        return auth_err

    state = _state(request)
    files = state.storage.list_bot_files(limit=1000)

    responses: list[Element] = []

    # Root collection
    root_props = {
        "displayname": "Telegram Drive",
        "resourcetype": _ns("collection"),
        "getlastmodified": _rfc1123(int(time.time())),
    }
    # We need a special format for resourcetype — it's an empty D:collection element, not text
    root_resp = _elem("response")
    root_resp.append(_elem("href", "/webdav/"))
    root_ps = _elem("propstat")
    root_prop = _elem("prop")
    # displayname
    root_prop.append(_elem("displayname", "Telegram Drive"))
    # resourcetype with <D:collection/>
    rt = _elem("resourcetype")
    rt.append(Element(_ns("collection")))
    root_prop.append(rt)
    # getlastmodified
    root_prop.append(_elem("getlastmodified", _rfc1123(int(time.time()))))
    root_ps.append(root_prop)
    root_ps.append(_elem("status", "HTTP/1.1 200 OK"))
    root_resp.append(root_ps)
    responses.append(root_resp)

    # File entries
    for f in files:
        name = f.get("file_name") or f"file_{f['message_id']}"
        fsize = int(f.get("file_size") or 0)
        created = int(f.get("created_at") or 0)
        mime = mimetypes.guess_type(name)[0] or "application/octet-stream"

        file_resp = _elem("response")
        file_resp.append(_elem("href", f"/webdav/{quote(name)}"))
        fps = _elem("propstat")
        fp = _elem("prop")
        fp.append(_elem("displayname", name))
        fp.append(_elem("resourcetype", ""))
        fp.append(_elem("getcontentlength", str(fsize)))
        fp.append(_elem("getcontenttype", mime))
        fp.append(_elem("getlastmodified", _rfc1123(created) if created else _rfc1123(int(time.time()))))
        fps.append(fp)
        fps.append(_elem("status", "HTTP/1.1 200 OK"))
        file_resp.append(fps)
        responses.append(file_resp)

    xml = _multistatus(responses)
    return Response(content=xml, media_type="application/xml; charset=utf-8", status_code=207)


@router.api_route("/{path:path}", methods=["GET", "HEAD"])
async def get_file(request: Request, path: str) -> Response:
    """Download a file by name (WebDAV GET)."""
    auth_err = await _authenticate(request)
    if auth_err:
        return auth_err

    if not path:
        return Response(status_code=404)

    filename = unquote(path)
    state = _state(request)
    files = state.storage.list_bot_files(limit=1000)
    match = None
    for f in files:
        if (f.get("file_name") or f"file_{f['message_id']}") == filename:
            match = f
            break

    if not match:
        return JSONResponse({"error": "File not found"}, status_code=404)

    message_id = match["message_id"]
    file_id = match.get("telegram_file_id")
    file_size = int(match.get("file_size") or 0)
    mime = mimetypes.guess_type(filename)[0] or "application/octet-stream"

    if request.method == "HEAD":
        return Response(
            headers={
                "Content-Type": mime,
                "Content-Length": str(file_size),
                "Content-Disposition": f'attachment; filename="{filename}"',
            }
        )

    if state.bot:
        fid = file_id or ""
        async def _bot_stream():
            async for chunk in state.bot.stream_download(fid):
                yield chunk
        return StreamingResponse(
            _bot_stream(),
            media_type=mime,
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "Content-Length": str(file_size),
            },
        )

    return JSONResponse({"error": "No bot transport available"}, status_code=503)


@router.api_route("/{path:path}", methods=["PUT"])
async def put_file(request: Request, path: str) -> Response:
    """Upload a file (WebDAV PUT)."""
    auth_err = await _authenticate(request)
    if auth_err:
        return auth_err

    if not path:
        return Response(status_code=400)

    filename = unquote(path)
    state = _state(request)

    body = await request.body()
    if not body:
        return Response(status_code=400, content=b"Empty file")

    if state.effective_transport_mode() == "bot" and state.bot:
        try:
            result = await state.bot.upload_bytes(body, filename)
            state.storage.record_bot_file(
                message_id=result.message_id,
                telegram_file_id=result.telegram_file_id,
                file_name=filename,
                file_size=len(body),
                caption=None,
                bot_pool_index=0,
            )
            return Response(status_code=201, headers={"Content-Length": "0"})
        except Exception as exc:
            return JSONResponse({"error": f"Upload failed: {exc}"}, status_code=500)

    return JSONResponse({"error": "No transport available"}, status_code=503)


@router.api_route("/{path:path}", methods=["DELETE"])
async def delete_file(request: Request, path: str) -> Response:
    """Delete a file (WebDAV DELETE)."""
    auth_err = await _authenticate(request)
    if auth_err:
        return auth_err

    if not path:
        return Response(status_code=400)

    filename = unquote(path)
    state = _state(request)
    files = state.storage.list_bot_files(limit=1000)
    match = None
    for f in files:
        if (f.get("file_name") or f"file_{f['message_id']}") == filename:
            match = f
            break

    if not match:
        return JSONResponse({"error": "File not found"}, status_code=404)

    message_id = match["message_id"]
    try:
        # Delete from telegram channel
        if state.bot:
            await state.bot.delete_message(message_id)
        # Delete from local database
        state.storage.delete_bot_file(message_id)
        return Response(status_code=204)
    except Exception as exc:
        return JSONResponse({"error": f"Delete failed: {exc}"}, status_code=500)


@router.api_route("/{path:path}", methods=["OPTIONS"])
async def options(request: Request, path: str) -> Response:
    """Report WebDAV capabilities."""
    return Response(
        status_code=200,
        headers={
            "DAV": "1",  # WebDAV class 1
            "Allow": "OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND",
            "Content-Length": "0",
        },
    )


@router.api_route("/{path:path}", methods=["PROPPATCH", "MKCOL", "MOVE", "COPY", "LOCK", "UNLOCK"])
async def unsupported(request: Request, path: str) -> Response:
    """Unsupported WebDAV methods return 405."""
    return Response(status_code=405)
