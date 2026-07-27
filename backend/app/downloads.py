"""Download plumbing — dual transport streaming, Range parsing, headers."""

from __future__ import annotations

import re
from collections.abc import AsyncIterator
from dataclasses import dataclass
from urllib.parse import quote

_RANGE_RE = re.compile(r"bytes=(\d+)-(\d*)")

INLINE_MIME_PREFIXES = ("image/", "video/", "audio/", "text/")
INLINE_EXTENSIONS = {"pdf", "json", "txt"}


@dataclass
class DownloadTarget:
    filename: str
    size: int
    mime_type: str
    stream: AsyncIterator[bytes]
    supports_range: bool = True


def parse_range_header(header: str | None, total: int) -> tuple[int, int] | None:
    """Parse ``bytes=start-end`` → (start, end) clamped to total-1."""
    if not header:
        return None
    m = _RANGE_RE.match(header.strip())
    if not m:
        return None
    start = int(m.group(1))
    end = int(m.group(2)) if m.group(2) else total - 1
    end = min(end, total - 1)
    if start > end or start >= total:
        return None
    return start, end


def content_disposition(filename: str, mime_type: str) -> str:
    """inline for previewable types, attachment otherwise (RFC 5987 names)."""
    ext = filename.rsplit(".", 1)[1].lower() if "." in filename else ""
    is_inline = mime_type.startswith(INLINE_MIME_PREFIXES) or ext in INLINE_EXTENSIONS
    kind = "inline" if is_inline else "attachment"
    ascii_name = filename.encode("ascii", "replace").decode("ascii").replace('"', "_")
    encoded = quote(filename)
    return f"{kind}; filename=\"{ascii_name}\"; filename*=UTF-8''{encoded}"


def guess_mime(filename: str) -> str:
    import mimetypes

    return mimetypes.guess_type(filename)[0] or "application/octet-stream"


async def _user_mode_stream(
    state, folder_id: int | None, message_id: int, offset: int, length: int | None,
    file_size: int = 0,
) -> AsyncIterator[bytes]:
    """Stream from Telethon; offset aligned to 4096 like the Rust impl."""
    MIN_CHUNK = 4096
    aligned_start = (offset // MIN_CHUNK) * MIN_CHUNK
    skip = offset - aligned_start
    remaining = length
    part_size_kb = adaptive_part_size(file_size)
    async for chunk in state.telegram.iter_download_by_id(
        folder_id, message_id, start_byte=aligned_start, part_size_kb=part_size_kb
    ):
        if skip:
            chunk = chunk[skip:]
            skip = 0
        if not chunk:
            continue
        if remaining is not None:
            chunk = chunk[:remaining]
            remaining -= len(chunk)
        yield chunk
        if remaining is not None and remaining <= 0:
            break


def adaptive_part_size(file_size: int) -> int:
    """Return KB part size based on file size for optimal throughput/memory."""
    if file_size < 10 * 1024 * 1024:       # < 10MB
        return 256
    elif file_size < 100 * 1024 * 1024:    # 10-100MB
        return 512
    elif file_size < 1024 * 1024 * 1024:   # 100MB-1GB
        return 1024
    else:                                   # > 1GB
        return 2048


async def resolve_download(
    state,
    folder_id: int | None,
    message_id: int,
    filename_hint: str | None = None,
    offset: int = 0,
    length: int | None = None,
) -> DownloadTarget:
    """Resolve a message id to a byte stream via the active transport."""
    mode = state.effective_transport_mode()

    if mode == "bot":
        row = state.storage.get_bot_file(message_id)
        if row is None:
            raise LookupError(f"bot file mapping not found for message {message_id}")
        filename = filename_hint or row["file_name"] or f"file_{message_id}"
        size = int(row["file_size"] or 0)
        stream = state.bot.stream_download(
            row["telegram_file_id"], offset=offset, length=length
        )
        return DownloadTarget(
            filename=filename,
            size=size,
            mime_type=guess_mime(filename),
            stream=stream,
        )

    # user mode
    message = await state.telegram.get_message(folder_id, message_id)
    meta = state.telegram.message_to_metadata(message, folder_id)
    filename = filename_hint or (meta.name if meta else f"file_{message_id}")
    size = meta.size if meta else 0
    mime = (meta.mime_type if meta else "") or guess_mime(filename)
    stream = _user_mode_stream(state, folder_id, message_id, offset, length, file_size=size)
    return DownloadTarget(
        filename=filename, size=size, mime_type=mime, stream=stream
    )
