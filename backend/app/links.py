"""Share links, presigned URLs and progress tokens.

Reproduces the Rust signing formats byte-for-byte so links issued by either
backend verify in the other:

- share token: 32 random bytes → 64 hex chars (``shared_links.id``)
- share auth cookie: ``HMAC-SHA256(key=token, msg=password_hash)`` hex
- presigned: ``HMAC-SHA256(secret, "v1|{message_id}|{folder_id}|{exp}|{owner}|{max_downloads}")``
- progress token: ``HMAC-SHA256(SHA256("upload-progress-v1|"+pwd), "v1|{session_id}|{exp}")``
"""

from __future__ import annotations

import hashlib
import hmac
import secrets
import time
from typing import Optional
from urllib.parse import quote

PROGRESS_TOKEN_TTL_SECS = 300
SHARE_COOKIE_MAX_AGE = 1800
PRESIGN_VERSION = "v1"


def new_share_token() -> str:
    return secrets.token_hex(32)


def share_cookie_value(token: str, password_hash: str) -> str:
    return hmac.new(
        token.encode("utf-8"), password_hash.encode("utf-8"), hashlib.sha256
    ).hexdigest()


def verify_share_cookie(token: str, password_hash: str, cookie: str) -> bool:
    expected = share_cookie_value(token, password_hash)
    return hmac.compare_digest(expected, cookie)


# ── presigned download URLs ─────────────────────────────────────────────────
def presign_canonical(
    message_id: int,
    folder_id: Optional[int],
    expires_at: int,
    owner_id: str,
    max_downloads: Optional[int],
) -> str:
    folder_part = str(folder_id) if folder_id is not None else ""
    max_part = str(max_downloads) if max_downloads is not None else ""
    return f"{PRESIGN_VERSION}|{message_id}|{folder_part}|{expires_at}|{owner_id}|{max_part}"


def presign_signature(secret: str, canonical: str) -> str:
    return hmac.new(
        secret.encode("utf-8"), canonical.encode("utf-8"), hashlib.sha256
    ).hexdigest()


def verify_presign_signature(secret: str, canonical: str, signature: str) -> bool:
    expected = presign_signature(secret, canonical)
    return hmac.compare_digest(expected, signature)


def verify_presign_with_secrets(secrets: list[str], canonical: str, signature: str) -> bool:
    """Verify a pre-signed signature against all valid secrets (key rotation)."""
    for secret in secrets:
        if verify_presign_signature(secret, canonical, signature):
            return True
    return False


def presigned_url(
    base_url: str,
    secret: str,
    message_id: int,
    folder_id: Optional[int],
    owner_id: str,
    ttl_secs: int,
    max_downloads: Optional[int] = None,
) -> tuple[str, int]:
    """Build /d/signed?... — returns (url, expires_at); exp=0 means never."""
    expires_at = int(time.time()) + ttl_secs if ttl_secs > 0 else 0
    canonical = presign_canonical(message_id, folder_id, expires_at, owner_id, max_downloads)
    sig = presign_signature(secret, canonical)
    params = f"file_id={message_id}&exp={expires_at}&owner={quote(owner_id)}&sig={sig}"
    if folder_id is not None:
        params += f"&folder_id={folder_id}"
    if max_downloads is not None:
        params += f"&max_downloads={max_downloads}"
    return f"{base_url.rstrip('/')}/d/signed?{params}", expires_at


# ── upload progress tokens ──────────────────────────────────────────────────
def _progress_key(access_pwd: str) -> bytes:
    # Rust impl: hex(SHA256("upload-progress-v1|"+pwd)) used as a STRING key.
    return hashlib.sha256(f"upload-progress-v1|{access_pwd}".encode("utf-8")).hexdigest().encode("utf-8")


def issue_progress_token(access_pwd: str, session_id: str, expires_at: int) -> str:
    msg = f"v1|{session_id}|{expires_at}".encode("utf-8")
    return hmac.new(_progress_key(access_pwd), msg, hashlib.sha256).hexdigest()


def verify_progress_token(
    access_pwd: str, session_id: str, expires_at: int, token: str
) -> bool:
    if not session_id.strip() or not token.strip() or expires_at <= 0:
        return False
    if int(time.time()) >= expires_at:
        return False
    expected = issue_progress_token(access_pwd, session_id, expires_at)
    return hmac.compare_digest(expected, token.strip())
