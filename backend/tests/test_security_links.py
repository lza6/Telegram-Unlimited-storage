"""Security + links signing tests (byte-format parity with the Rust backend)."""

from __future__ import annotations

import hashlib
import hmac
import time

from app import links, security


def test_api_key_argon2_roundtrip():
    h = security.hash_api_key("secret-key")
    valid, upgrade = security.verify_api_key("secret-key", h)
    assert valid is True
    assert upgrade is False  # already Argon2


def test_api_key_wrong_rejected():
    h = security.hash_api_key("secret-key")
    valid, _ = security.verify_api_key("wrong", h)
    assert valid is False


def test_api_key_legacy_sha256_upgrade_flag():
    legacy = hashlib.sha256(b"secret-key").hexdigest()
    valid, upgrade = security.verify_api_key("secret-key", legacy)
    assert valid is True
    assert upgrade is True  # should upgrade to Argon2


def test_share_password_roundtrip():
    h, salt = security.hash_share_password("pw123")
    assert security.verify_share_password("pw123", h, salt) is True
    assert security.verify_share_password("bad", h, salt) is False


def test_share_token_is_64_hex():
    token = links.new_share_token()
    assert len(token) == 64
    int(token, 16)  # valid hex


def test_share_cookie_hmac():
    cookie = links.share_cookie_value("tok", "hash")
    expected = hmac.new(b"tok", b"hash", hashlib.sha256).hexdigest()
    assert cookie == expected
    assert links.verify_share_cookie("tok", "hash", cookie) is True
    assert links.verify_share_cookie("tok", "hash", "deadbeef") is False


def test_progress_token_roundtrip():
    exp = int(time.time()) + 300
    token = links.issue_progress_token("pwd", "sess1", exp)
    assert links.verify_progress_token("pwd", "sess1", exp, token) is True
    assert links.verify_progress_token("pwd", "sess1", exp, "bad") is False
    assert links.verify_progress_token("wrong", "sess1", exp, token) is False


def test_progress_token_expired():
    exp = int(time.time()) - 1
    token = links.issue_progress_token("pwd", "sess1", exp)
    assert links.verify_progress_token("pwd", "sess1", exp, token) is False


def test_presign_signature_roundtrip():
    canonical = links.presign_canonical(42, None, 0, "owner", None)
    sig = links.presign_signature("secret", canonical)
    assert links.verify_presign_signature("secret", canonical, sig) is True
    assert links.verify_presign_signature("secret", canonical, "bad") is False
