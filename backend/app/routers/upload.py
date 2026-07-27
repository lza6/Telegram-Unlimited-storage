"""Resumable Upload API Routes (TASK-P0-02).

Provides chunk-level manifest tracking and idempotency verification to enable
interrupted uploads (network drops, browser restarts) to resume without
re-uploading completed chunks.
"""

from __future__ import annotations

from fastapi import APIRouter, File, Form, Request, UploadFile
from fastapi.responses import JSONResponse

from ..resume import ResumeManager
from ..state import AppState

router = APIRouter(prefix="/api/v1", tags=["upload"])


def get_state(request: Request) -> AppState:
    return request.app.state.app


def api_error(code: str, message: str, status_code: int) -> JSONResponse:
    return JSONResponse(
        {"error": {"code": code, "message": message}}, status_code=status_code
    )


@router.post("/upload/init")
async def upload_init(
    request: Request,
    filename: str = Form(...),
    total_size: int = Form(...),
    total_chunks: int = Form(...),
    file_hash: str = Form(...),
    owner_id: str = Form("default"),
):
    state = get_state(request)
    state.authenticator.require_auth(request)

    if total_size <= 0 or total_chunks <= 0:
        return api_error("BAD_REQUEST", "total_size and total_chunks must be positive", 400)
    if not file_hash:
        return api_error("BAD_REQUEST", "file_hash is required", 400)

    rm = ResumeManager(state.storage)
    session = rm.init_session(
        filename=filename,
        total_chunks=total_chunks,
        total_size=total_size,
        file_hash=file_hash,
        owner_id=owner_id,
    )

    return JSONResponse(
        {
            "session_id": session.session_id,
            "filename": session.filename,
            "total_chunks": session.total_chunks,
            "total_size": session.total_size,
            "file_hash": session.file_hash,
            "status": session.status,
            "created_at": session.created_at,
            "expires_at": session.expires_at,
        }
    )


@router.get("/upload/status/{session_id}")
async def upload_status(session_id: str, request: Request):
    state = get_state(request)
    state.authenticator.require_auth(request)

    rm = ResumeManager(state.storage)
    session = state.storage.get_upload_session(session_id)
    if not session:
        return api_error("NOT_FOUND", "Session not found", 404)

    missing = rm.get_missing_chunks(session_id)
    chunks = state.storage.list_upload_chunks(session_id)
    uploaded_count = len([c for c in chunks if c.get("status") == "uploaded"])

    return JSONResponse(
        {
            "session_id": session_id,
            "filename": session["filename"],
            "total_chunks": session["total_chunks"],
            "uploaded_chunks": uploaded_count,
            "missing_chunks": missing,
            "status": session["status"],
            "manifest_file_id": session.get("manifest_file_id"),
        }
    )


@router.post("/upload/chunk/{session_id}/{chunk_index}")
async def upload_chunk(
    session_id: str,
    chunk_index: int,
    request: Request,
    chunk: UploadFile | None = File(None),
    sha256: str = Form(""),
    total_chunks: int = Form(0),
):
    state = get_state(request)
    state.authenticator.require_auth(request)

    session = state.storage.get_upload_session(session_id)
    if not session:
        return api_error("NOT_FOUND", "Session not found or expired", 404)

    data = await chunk.read() if chunk else b""
    if not data:
        return api_error("BAD_REQUEST", "missing chunk payload", 400)

    rm = ResumeManager(state.storage)
    success = rm.record_chunk(session_id, chunk_index, data, expected_sha256=sha256)
    if not success:
        return api_error("CHECKSUM_MISMATCH", "Chunk SHA-256 does not match", 409)

    # Track progress event
    if total_chunks > 0:
        state.transfers.ensure_progress(session_id, session["filename"], total_chunks)
    chunks = state.storage.list_upload_chunks(session_id)
    uploaded_count = len([c for c in chunks if c.get("status") == "uploaded"])
    state.transfers.update_progress(session_id, uploaded_chunks=uploaded_count)

    return JSONResponse({"ok": True, "chunk_index": chunk_index, "sha256": sha256})


@router.post("/upload/complete/{session_id}")
async def upload_complete(
    session_id: str,
    request: Request,
    folder_id: str | None = Form(None),
):
    state = get_state(request)
    state.authenticator.require_auth(request)

    session = state.storage.get_upload_session(session_id)
    if not session:
        return api_error("NOT_FOUND", "Session not found", 404)

    if session["status"] == "completed" and session.get("manifest_file_id"):
        return JSONResponse(
            {
                "ok": True,
                "session_id": session_id,
                "status": "completed",
                "manifest_file_id": session["manifest_file_id"],
            }
        )

    rm = ResumeManager(state.storage)
    if not rm.is_complete(session_id):
        return api_error("INCOMPLETE", "Not all chunks are uploaded yet", 400)

    # Get all uploaded chunk IDs in sequence to generate manifest
    chunks = state.storage.list_upload_chunks(session_id)
    final_ids: list[str] = []
    for c in chunks:
        if not c.get("file_id"):
            return api_error("MISSING_FILE_ID", f"Chunk {c['chunk_index']} missing Telegram file_id", 400)
        final_ids.append(c["file_id"])

    fid: int | None = None
    if folder_id not in (None, "", "null"):
        try:
            fid = int(folder_id)
        except ValueError:
            pass

    # Build the manifest and upload it to Telegram
    manifest = session["filename"] + "\n" + "".join(f"{cid}\n" for cid in final_ids)
    try:
        manifest_id = await state.telegram.upload_bytes(
            fid, manifest.encode("utf-8"), "fileAll.txt", caption=session["filename"]
        )
    except Exception as exc:
        return api_error("TELEGRAM_UPLOAD_FAILED", str(exc), 500)

    # Update session status and manifest linkage
    state.storage.update_upload_session_status(session_id, "completed", str(manifest_id))
    state.storage.upsert_file_asset(
        manifest_id, fid, "web", session["filename"], len(manifest.encode("utf-8"))
    )

    # Mark progress completed
    state.transfers.update_progress(
        session_id,
        uploaded_chunks=len(final_ids),
        total_chunks=session["total_chunks"],
        status="completed",
        file_id=str(manifest_id),
    )

    return JSONResponse(
        {
            "ok": True,
            "session_id": session_id,
            "status": "completed",
            "manifest_file_id": str(manifest_id),
            "file_id": str(manifest_id),
        }
    )
