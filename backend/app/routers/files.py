"""Files & folders: list, upload, download, bulk ops, search, index rebuild."""

from __future__ import annotations

import asyncio
import mimetypes
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Optional
from urllib.parse import quote

from fastapi import APIRouter, Form, HTTPException, Request, Response, UploadFile
from fastapi.responses import JSONResponse, StreamingResponse
from pydantic import BaseModel, Field

from ..auth import CallerIdentity

from .. import links
from ..audit import get_audit_logger
from ..classifier import classify_file
from ..downloads import content_disposition, parse_range_header, resolve_download
from ..errors import (
    NotFoundError,
    PayloadTooLargeError,
    StorageError,
    TelegramError,
    ValidationError,
)
from ..settings_store import SettingsStore
from ..state import AppState

router = APIRouter(prefix="/api/v1", tags=["files"])

_NULLISH = {"", "null", "none"}


# Lightweight stand-in for Telethon's FileMeta in bot mode.
# The to_api_file() helper reads .id, .folder_id, .name, .size, .mime_type, .created_at.
@dataclass
class _BotFileMeta:
    id: int
    folder_id: Optional[int]
    name: str
    size: int
    mime_type: str
    created_at: int


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


def parse_folder_id(raw: Optional[str]) -> Optional[int]:
    if raw is None or raw.strip().lower() in _NULLISH:
        return None
    try:
        return int(raw)
    except ValueError:
        return None


def rfc3339(ts: int) -> str:
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _bot_row_to_meta(row: dict[str, Any]) -> _BotFileMeta:
    """Convert bot_file_map row to _BotFileMeta for uniform API response."""
    return _BotFileMeta(
        id=row["message_id"],
        folder_id=None,  # bot mode has no folders
        name=row["file_name"] or f"file_{row['message_id']}",
        size=int(row["file_size"] or 0),
        mime_type=mimetypes.guess_type(row["file_name"] or "")[0] or "application/octet-stream",
        created_at=int(row["created_at"] or 0),
    )


def to_api_file(meta: Any) -> dict[str, Any]:
    return {
        "id": meta.id,
        "folder_id": meta.folder_id,
        "name": meta.name,
        "size": meta.size,
        "mime_type": meta.mime_type,
        "created_at": rfc3339(meta.created_at),
        "category": classify_file(meta.name, getattr(meta, "mime_type", None)),
    }


async def _require_connected(state: AppState) -> None:
    ready = await state.is_ready()
    if not ready:
        raise TelegramError("Telegram transport is not ready")


def require_scope(scope: str):
    """FastAPI dependency factory: require a specific scope for the caller."""
    def _dep(request: Request) -> CallerIdentity:
        state = get_state(request)
        identity = state.authenticator.require_auth(request)
        if identity.kind == "tenant":
            scopes = state.storage.get_tenant_scopes(identity.tenant_id)
            if scopes and scope not in scopes:
                raise HTTPException(
                    status_code=403,
                    detail={"code": "FORBIDDEN", "message": f"Scope '{scope}' required"},
                )
        return identity
    return _dep


# ── folders ─────────────────────────────────────────────────────────────────
@router.get("/folders")
async def list_folders(request: Request) -> JSONResponse:
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    await _require_connected(state)
    # Bot mode has no folders — all files live in a flat storage channel.
    if state.effective_transport_mode() == "bot":
        return JSONResponse([], headers={"X-Metadata-Cache": "MISS"})
    cache_key = f"folders:{identity.owner_id}"
    ttl = state.settings.metadata_cache_ttl_secs
    if state.settings.metadata_cache_enabled:
        cached = await asyncio.to_thread(state.storage.cache_get, cache_key, ttl)
        if cached is not None:
            return JSONResponse(cached, headers={"X-Metadata-Cache": "HIT"})
    try:
        folders = await state.telegram.scan_folders()
    except Exception:
        raise TelegramError("Telegram transport is not ready")
    payload = [
        {"id": f.id, "name": f.name} for f in folders if not f.is_root or f.id is None
    ]
    if state.settings.metadata_cache_enabled:
        await asyncio.to_thread(state.storage.cache_set, cache_key, "folders", payload)
    return JSONResponse(payload, headers={"X-Metadata-Cache": "MISS"})


# ── files list ──────────────────────────────────────────────────────────────
@router.get("/files")
async def list_files(
    request: Request,
    folder_id: Optional[str] = None,
    page: int = 1,
    limit: int = 100,
    search: Optional[str] = None,
    sort: str = "created_at",
    order: str = "desc",
    mime_type: Optional[str] = None,
    size_min: Optional[int] = None,
    size_max: Optional[int] = None,
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    fid = parse_folder_id(folder_id)

    # Bot mode: query storage DB instead of Telethon
    if state.effective_transport_mode() == "bot":
        try:
            # Query all bot files for in-memory filtering/sorting, then paginate
            rows = await asyncio.to_thread(state.storage.list_bot_files)
            files = [_bot_row_to_meta(row) for row in rows]
        except Exception as exc:
            raise StorageError(f"Failed to list bot files: {exc}")
    else:
        try:
            files = await state.telegram.list_files(fid)
        except Exception:
            raise TelegramError("Telegram transport is not ready")

    # In-memory filtering (Rust impl paginates after full fetch too).
    if search:
        q = search.lower()
        files = [f for f in files if q in f.name.lower()]
    if mime_type:
        files = [f for f in files if f.mime_type.startswith(mime_type)]
    if size_min is not None:
        files = [f for f in files if f.size >= size_min]
    if size_max is not None:
        files = [f for f in files if f.size <= size_max]

    reverse = order.lower() != "asc"
    key_attr = sort if sort in ("name", "size", "created_at") else "created_at"
    files.sort(key=lambda f: getattr(f, key_attr) or 0, reverse=reverse)

    total = len(files)
    limit = max(1, min(limit, 1000))
    page = max(1, page)
    start = (page - 1) * limit
    page_files = files[start : start + limit]
    total_pages = (total + limit - 1) // limit if total else 0
    items = [to_api_file(f) for f in page_files]
    return JSONResponse(
        {
            "data": items,
            "files": items,  # compat field
            "page": page,
            "limit": limit,
            "total": total,
            "pagination": {
                "page": page,
                "limit": limit,
                "total": total,
                "total_pages": total_pages,
                "has_next": page < total_pages,
                "has_prev": page > 1,
            },
        }
    )


@router.get("/files/search")
async def search_files(
    request: Request,
    q: Optional[str] = None,
    folder_id: Optional[str] = None,
    recursive: Optional[bool] = None,
) -> JSONResponse:
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    query = (q or "").strip()
    if not query:
        return JSONResponse([])
    fid = parse_folder_id(folder_id)

    # Bot mode: query storage DB instead of Telethon
    if state.effective_transport_mode() == "bot":
        try:
            # Try FTS5 first, fall back to LIKE search
            fts_results = await asyncio.to_thread(state.storage.fts_search, query, 50)
            if fts_results:
                files = []
                for r in fts_results:
                    row = state.storage.get_bot_file(r["message_id"])
                    if row:
                        files.append(_bot_row_to_meta(row))
                if files:
                    return JSONResponse([to_api_file(f) for f in files])
            rows = await asyncio.to_thread(state.storage.search_bot_files, query, 50)
            files = [_bot_row_to_meta(row) for row in rows]
        except Exception as exc:
            raise StorageError(f"Search failed: {exc}")
        return JSONResponse([to_api_file(f) for f in files])

    try:
        if fid is not None or recursive is False:
            files = await state.telegram.list_files(fid)
            ql = query.lower()
            files = [f for f in files if ql in f.name.lower()][:50]
        else:
            files = await state.telegram.search_global(query, limit=50)
    except Exception:  # noqa: BLE001 — session/transport failure → friendly 503
            raise TelegramError("Telegram transport is not ready")
    return JSONResponse([to_api_file(f) for f in files])


@router.get("/files/{message_id}")
async def get_file(message_id: int, request: Request, folder_id: Optional[str] = None):
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    fid = parse_folder_id(folder_id)

    # Bot mode: query storage DB instead of Telethon
    if state.effective_transport_mode() == "bot":
        try:
            row = await asyncio.to_thread(state.storage.get_bot_file, message_id)
        except Exception as exc:
            raise StorageError(f"Failed to fetch file: {exc}")
        if row is None:
            raise NotFoundError("File", str(message_id))
        return JSONResponse(to_api_file(_bot_row_to_meta(row)))

    try:
        message = await state.telegram.get_message(fid, message_id)
    except LookupError:
        raise NotFoundError("File", str(message_id))
    except ValueError as exc:
        raise ValidationError(str(exc))
    except Exception:
        raise StorageError("Failed to fetch file")
    meta = state.telegram.message_to_metadata(message, fid)
    if meta is None:
        raise NotFoundError("File", str(message_id))
    return JSONResponse(to_api_file(meta))


# ── download ────────────────────────────────────────────────────────────────
@router.get("/files/{message_id}/download")
async def download_file(
    message_id: int,
    request: Request,
    folder_id: Optional[str] = None,
    filename: Optional[str] = None,
):
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    fid = parse_folder_id(folder_id)
    range_header = request.headers.get("range")
    try:
        target = await resolve_download(state, fid, message_id, filename)
    except LookupError:
        raise NotFoundError("File", str(message_id))
    except Exception:
        raise StorageError("Download failed")

    total = target.size
    ranged = parse_range_header(range_header, total) if total > 0 else None
    headers = {
        "Accept-Ranges": "bytes",
        "Content-Disposition": content_disposition(target.filename, target.mime_type),
    }
    if ranged and total > 0:
        start, end = ranged
        # Re-resolve with offset for transports that support ranged fetch.
        target = await resolve_download(
            state, fid, message_id, filename, offset=start, length=end - start + 1
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


# ── upload ──────────────────────────────────────────────────────────────────
def _issue_download_link(
    state: AppState, message_id: int, folder_id: Optional[int], owner_id: str, filename: str
) -> tuple[Optional[str], Optional[str], Optional[int], str]:
    """Returns (download_url, share_id, expires_at, link_kind)."""
    settings = state.settings
    base = SettingsStore(settings.data_dir).share_base_url(settings.base_url, None)
    if len(settings.download_signing_secret) >= 32:
        url, exp = links.presigned_url(
            base,
            settings.download_signing_secret,
            message_id,
            folder_id,
            owner_id,
            settings.upload_link_ttl_secs,
        )
        return url, None, exp if exp else None, "presigned"
    if settings.upload_share_ttl_hours > 0:
        token = links.new_share_token()
        import time as _time

        created = int(_time.time())
        expires_at = created + settings.upload_share_ttl_hours * 3600
        state.storage.create_share(
            share_id=token,
            folder_id=folder_id,
            message_id=message_id,
            file_name=filename,
            file_size=0,
            password_hash=None,
            password_salt=None,
            expires_at=expires_at,
            owner_id=owner_id,
        )
        return f"{base}/d/{token}", token, expires_at, "share"
    if settings.public_file_id_download:
        return (
            f"{base}/d?file_id={message_id}&filename={quote(filename)}",
            None,
            None,
            "legacy",
        )
    return None, None, None, "none"


@router.post("/files")
async def upload_file(
    request: Request,
    file: Optional[UploadFile] = None,
    folder_id: Optional[str] = Form(None),
):
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    if file is None or not file.filename:
        raise ValidationError("No file provided")
    await _require_connected(state)
    if not state.transfers.try_acquire_file_slot():
        return JSONResponse(
            {"error": {"code": "UPLOAD_QUEUE_FULL", "message": "Upload queue full"}},
            status_code=503,
            headers={"Retry-After": "3"},
        )
    try:
        # Reject oversized uploads early via Content-Length before reading the body.
        max_bytes = state.settings.max_upload_size_mb * 1024 * 1024
        content_length = request.headers.get("content-length")
        if content_length:
            try:
                if int(content_length) > max_bytes:
                    raise PayloadTooLargeError(state.settings.max_upload_size_mb)
            except ValueError:
                pass
        data = await file.read()
        if len(data) > max_bytes:
            raise PayloadTooLargeError(state.settings.max_upload_size_mb)
        fid = parse_folder_id(folder_id)
        filename = file.filename
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
            message_id = await state.telegram.upload_bytes(fid, data, filename)
        # Invalidate folder list cache.
        state.storage.cache_set(f"files:{identity.owner_id}:{fid}", "files", [])
    except Exception as exc:
        raise StorageError(f"Upload failed: {exc}")
    finally:
        state.transfers.release_file_slot()

    owner_id = (
        f"tenant:{identity.tenant_id}"
        if identity.kind == "tenant"
        else "system:web"
    )
    state.storage.upsert_file_asset(
        message_id=message_id,
        folder_id=fid,
        owner_id=owner_id,
        file_name=filename,
        file_size=len(data),
    )
    download_url, share_id, expires_at, link_kind = _issue_download_link(
        state, message_id, fid, owner_id, filename
    )
    api_download_url = f"/api/v1/files/{message_id}/download"
    if fid is not None:
        api_download_url += f"?folder_id={fid}"
    result_payload: dict[str, Any] = {
        "id": message_id,
        "file_id": str(message_id),
        "folder_id": fid,
        "name": filename,
        "filename": filename,
        "download_url": download_url or "",
        "api_download_url": api_download_url,
        "owner_id": owner_id,
        "link_kind": link_kind,
    }
    if share_id:
        result_payload["share_id"] = share_id
    if expires_at:
        result_payload["expires_at"] = expires_at

    # Audit log: file upload
    audit = get_audit_logger()
    if audit:
        client_ip = request.client.host if request.client else "unknown"
        audit.log_file_upload(
            actor=client_ip,
            file_id=message_id,
            filename=filename,
            size=len(data),
        )

    return JSONResponse(result_payload)


# ── bulk operations ─────────────────────────────────────────────────────────
class BulkRequest(BaseModel):
    action: str
    file_ids: list[Any]
    folder_id: Optional[int] = None
    payload: Optional[dict[str, Any]] = None


@router.post("/files/bulk")
async def bulk_files(body: BulkRequest, request: Request):
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    try:
        ids = [int(x) for x in body.file_ids]
    except (TypeError, ValueError):
        raise ValidationError("file_ids must be integers")
    if not ids:
        raise ValidationError("file_ids is empty")

    if body.action == "delete":
        # Revoke shares pointing at these messages first.
        shares_revoked = 0
        for share in state.storage.list_shares():
            if share["message_id"] in ids and not share["revoked"]:
                state.storage.revoke_share(share["id"])
                shares_revoked += 1

        # Audit log: file deletion (before actual deletion attempt)
        audit = get_audit_logger()
        if audit:
            client_ip = request.client.host if request.client else "unknown"
            audit.log_file_delete(actor=client_ip, file_ids=ids, count=len(ids))

        # Bot mode: delete from storage DB + call bot API
        if state.effective_transport_mode() == "bot":
            deleted_count = 0
            tg_failed_ids = []
            for msg_id in ids:
                try:
                    await state.bot.delete_message(msg_id)
                except Exception:  # noqa: BLE001 — message may already be gone
                    tg_failed_ids.append(msg_id)
                # Always clean DB to maintain consistency, even if
                # the Telegram API call failed (message may be stale).
                try:
                    await asyncio.to_thread(state.storage.delete_bot_file, msg_id)
                    deleted_count += 1
                except Exception:  # noqa: BLE001
                    pass
            return JSONResponse(
                {
                    "success": True,
                    "count": deleted_count,
                    "succeeded_ids": [i for i in ids if i not in tg_failed_ids],
                    "failed_ids": tg_failed_ids,
                    "shares_revoked": shares_revoked,
                }
            )

        # User mode: use Telethon
        try:
            await state.telegram.delete_files(body.folder_id, ids)
        except Exception:
            raise TelegramError("Telegram transport is not ready")
        return JSONResponse(
            {
                "success": True,
                "count": len(ids),
                "succeeded_ids": ids,
                "shares_revoked": shares_revoked,
            }
        )
    if body.action == "move":
        target = (body.payload or {}).get("folder_id")
        if target is None:
            raise ValidationError("move requires payload.folder_id")
        if state.effective_transport_mode() == "bot":
            raise ValidationError("Folder operations are not supported in bot mode")
        try:
            new_ids = await state.telegram.move_files(body.folder_id, target, ids)
        except Exception:
            raise TelegramError("Telegram transport is not ready")
        return JSONResponse(
            {"success": True, "count": len(new_ids), "succeeded_ids": new_ids}
        )
    raise ValidationError(f"unsupported action: {body.action}")


# ── index rebuild (admin, user mode only) ───────────────────────────────────
class RebuildIndexRequest(BaseModel):
    folder_ids: Optional[list[Optional[int]]] = None


@router.post("/files/rebuild-index")
async def rebuild_index(request: Request, body: Optional[RebuildIndexRequest] = None):
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    if state.effective_transport_mode() == "bot":
        raise ValidationError("Index rebuild is not supported in bot mode")
    await _require_connected(state)
    owner_id = (
        f"tenant:{identity.tenant_id}"
        if identity.kind == "tenant"
        else "system:web"
    )
    folder_ids = body.folder_ids if body and body.folder_ids is not None else [None]
    folders_scanned = 0
    files_indexed = 0
    try:
        state.storage.delete_owner_assets(owner_id)
        for fid in folder_ids:
            files = await state.telegram.list_files(fid)
            for meta in files:
                state.storage.upsert_file_asset(
                    message_id=meta.id,
                    folder_id=meta.folder_id,
                    owner_id=owner_id,
                    file_name=meta.name,
                    file_size=meta.size,
                )
                files_indexed += 1
            folders_scanned += 1
        state.telegram.file_index_complete = True
        state.storage.set_meta(f"index_complete:{owner_id}", "1")
    except Exception as exc:  # noqa: BLE001
        raise StorageError(f"Rebuild failed: {exc}")
    return JSONResponse(
        {"folders_scanned": folders_scanned, "files_indexed": files_indexed}
    )


@router.post("/files/sync-channel")
async def sync_channel(request: Request) -> JSONResponse:
    """Show sync status (background poller auto-indexes new channel posts)."""
    state = get_state(request)
    state.authenticator.require_auth(request)
    if state.effective_transport_mode() != "bot":
        raise ValidationError("Channel sync is only available in bot mode")
    files = state.storage.list_bot_files(limit=100)
    return JSONResponse({
        "status": "ok",
        "mode": "auto",
        "total_files": len(files),
        "note": "New channel posts are auto-indexed by background polling. "
                "To import existing messages, forward them from the channel to the bot's DM.",
    })


# ── trash / recycle bin ────────────────────────────────────────────────────
class TrashRestoreRequest(BaseModel):
    message_ids: list[int] = Field(..., min_length=1)


@router.get("/trash")
async def list_trash(request: Request) -> JSONResponse:
    """List soft-deleted files in the recycle bin."""
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    rows = await asyncio.to_thread(state.storage.list_trash, identity.owner_id)
    return JSONResponse([
        {"message_id": r["message_id"], "file_name": r["file_name"],
         "file_size": r["file_size"], "deleted_at": rfc3339(r["deleted_at"])}
        for r in rows
    ])


@router.post("/trash/restore")
async def restore_trash(body: TrashRestoreRequest, request: Request) -> JSONResponse:
    """Restore files from recycle bin."""
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    count = await asyncio.to_thread(
        state.storage.restore_assets, identity.owner_id, body.message_ids
    )
    return JSONResponse({"success": True, "restored": count})


@router.delete("/trash/empty")
async def empty_trash(request: Request) -> JSONResponse:
    """Permanently delete all files in recycle bin."""
    state = get_state(request)
    identity = state.authenticator.require_auth(request)
    retention = state.settings.trash_retention_days
    count = await asyncio.to_thread(
        state.storage.empty_trash, identity.owner_id, retention
    )
    return JSONResponse({"success": True, "permanently_deleted": count})


# ── batch operations ─────────────────────────────────────────────────────────
class BatchDeleteRequest(BaseModel):
    message_ids: list[int] = Field(..., min_length=1)


class BatchMoveRequest(BaseModel):
    message_ids: list[int] = Field(..., min_length=1)
    target_folder_id: Optional[int] = None


@router.post("/files/batch-move")
async def batch_move(body: BatchMoveRequest, request: Request) -> JSONResponse:
    """Move multiple files to a target folder."""
    state = get_state(request)
    state.authenticator.require_auth(request)
    await _require_connected(state)
    mode = state.effective_transport_mode()
    if mode == "bot":
        raise ValidationError("Batch move is only available in user mode")
    try:
        new_ids = await state.telegram.move_files(
            None, body.target_folder_id, body.message_ids
        )
    except Exception as exc:
        raise StorageError(str(exc))
    return JSONResponse({"success": True, "new_ids": new_ids})


@router.post("/files/batch-download")
async def batch_download(body: BatchDeleteRequest, request: Request) -> Response:
    """Download multiple files as a ZIP archive (streaming)."""
    import io
    import zipfile

    state = get_state(request)
    state.authenticator.require_auth(request)
    err = await _require_connected(state)
    if err:
        return err

    # Build a streaming ZIP archive
    buf = io.BytesIO()

    async def _generate_zip():
        with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
            for msg_id in body.message_ids:
                try:
                    target = await resolve_download(state, None, msg_id)
                    chunks = []
                    async for chunk in target.stream:
                        chunks.append(chunk)
                    zf.writestr(target.filename, b"".join(chunks))
                except Exception:
                    zf.writestr(f"error_{msg_id}.txt", f"Download failed for message {msg_id}".encode())
        buf.seek(0)
        while True:
            chunk = buf.read(65536)
            if not chunk:
                break
            yield chunk

    return StreamingResponse(
        _generate_zip(),
        media_type="application/zip",
        headers={"Content-Disposition": 'attachment; filename="batch_download.zip"'},
    )
