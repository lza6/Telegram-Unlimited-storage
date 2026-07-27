"""HTTP ETag / conditional-request helpers (TASK-P1-03, v8.0).

Implements weak-validator ETag computation and RFC 7232 If-None-Match
matching. ETags are computed from file identity (message_id + size + name)
rather than full content hashing — this avoids re-reading the whole file on
every request. The trade-off (a rename or resize invalidates the cache) is
acceptable: those are content changes that SHOULD bust the cache.
"""

from __future__ import annotations

import hashlib


def compute_etag(message_id: int, size: int, filename: str) -> str:
    """Return a weak ETag (``W/<16-hex>``) for a file asset.

    Uses sha256 of ``"message_id|size|filename"`` truncated to 16 hex chars.
    Weak validator (``W/``) because it is identity-based, not byte-exact.
    """
    material = f"{message_id}|{size}|{filename}".encode()
    digest = hashlib.sha256(material).hexdigest()[:16]
    return f"W/{digest}"


def etag_matches(client_header: str | None, server_etag: str) -> bool:
    """RFC 7232 If-None-Match matching (weak comparison).

    - ``*`` matches any non-empty server ETag.
    - Direct equality after stripping ``W/`` prefixes (weak comparison).
    - Empty/None client header → no match (serve the body).
    """
    if not client_header:
        return False
    if client_header == "*":
        return bool(server_etag)

    def _strip(tag: str) -> str:
        return tag[2:] if tag.startswith("W/") else tag

    # If-None-Match may contain a comma-separated list of validators.
    for candidate in (c.strip() for c in client_header.split(",")):
        if not candidate:
            continue
        if _strip(candidate) == _strip(server_etag):
            return True
    return False


__all__ = ["compute_etag", "etag_matches"]
