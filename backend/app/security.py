"""Password / API-key hashing — Argon2id for new hashes, SHA-256 legacy compat.

Python port of the Rust `password_kdf` module. Hash strings are PHC-format
Argon2id (`$argon2id$...`) and verify cross-language: parameters are embedded
in the hash itself, so hashes written by either backend verify in the other.
"""

from __future__ import annotations

import hashlib
import hmac

from argon2 import PasswordHasher
from argon2.exceptions import InvalidHashError, VerifyMismatchError

ARGON2_MARKER = "$argon2"

# argon2id (PasswordHasher default). Verification reads parameters from the
# stored PHC string, so hashes produced by the Rust backend verify here too.
_hasher = PasswordHasher()


def _legacy_sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def hash_api_key(key: str) -> str:
    """Hash an API key for storage (always Argon2id PHC string)."""
    return _hasher.hash(key)


def verify_api_key(plaintext: str, stored_hash: str) -> tuple[bool, bool]:
    """Verify an API key against a stored hash.

    Returns ``(is_valid, should_upgrade)`` — ``should_upgrade`` is True when a
    legacy SHA-256 hex hash validated successfully and should be migrated to
    Argon2id by the caller.
    """
    if stored_hash.startswith(ARGON2_MARKER):
        try:
            _hasher.verify(stored_hash, plaintext)
            return True, False
        except (VerifyMismatchError, InvalidHashError, ValueError):
            return False, False
    # Legacy SHA-256 hex (constant-time comparison).
    computed = _legacy_sha256_hex(plaintext.encode("utf-8"))
    valid = hmac.compare_digest(computed, stored_hash)
    return valid, valid


def verify_api_key_legacy(plaintext: str, stored_hash: str) -> bool:
    """Bool-only verification for backward compatibility."""
    return verify_api_key(plaintext, stored_hash)[0]


def hash_share_password(password: str) -> tuple[str, str | None]:
    """New share password: Argon2 hash only; salt column left empty."""
    return hash_api_key(password), None


def verify_share_password(
    password: str, stored_hash: str, salt: str | None
) -> bool:
    """Verify a share password (Argon2 PHC or legacy SHA-256(password||salt))."""
    if stored_hash.startswith(ARGON2_MARKER):
        return verify_api_key_legacy(password, stored_hash)
    computed = _legacy_sha256_hex(
        password.encode("utf-8") + (salt or "").encode("utf-8")
    )
    return hmac.compare_digest(computed, stored_hash)
