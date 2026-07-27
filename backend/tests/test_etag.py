"""TDD tests for ETag / conditional-request cache layer (TASK-P1-03, v8.0)."""

from __future__ import annotations

import hashlib

from app.etag import compute_etag, etag_matches


def test_compute_etag_is_weak_deterministic() -> None:
    """ETag is a weak validator (W/ prefix) and deterministic for same inputs."""
    e1 = compute_etag(message_id=100, size=4096, filename="a.txt")
    e2 = compute_etag(message_id=100, size=4096, filename="a.txt")
    assert e1 == e2
    assert e1.startswith("W/")


def test_compute_etag_changes_with_inputs() -> None:
    """Different inputs → different ETag."""
    base = compute_etag(message_id=100, size=4096, filename="a.txt")
    assert compute_etag(100, 4096, "b.txt") != base
    assert compute_etag(100, 8192, "a.txt") != base
    assert compute_etag(101, 4096, "a.txt") != base


def test_compute_etag_is_short_hash() -> None:
    """ETag body is a 16-char hex sha256 prefix (not the full digest)."""
    e = compute_etag(1, 10, "x")
    body = e.removeprefix("W/")
    assert len(body) == 16
    expected = hashlib.sha256(b"1|10|x").hexdigest()[:16]
    assert body == expected


def test_etag_matches_exact() -> None:
    """Direct equality match (client If-None-Match == server ETag)."""
    assert etag_matches("W/abc", "W/abc") is True
    assert etag_matches("W/abc", "W/xyz") is False


def test_etag_matches_star() -> None:
    """If-None-Match: * matches any non-empty server ETag."""
    assert etag_matches("*", "W/anything") is True
    assert etag_matches("*", "") is False


def test_etag_matches_weak_strong() -> None:
    """Weak comparison: W/x matches x (RFC 7232 weak validator)."""
    assert etag_matches("W/abc", "abc") is True
    assert etag_matches("abc", "W/abc") is True


def test_etag_matches_none() -> None:
    """No client header (None/empty) → no match (must serve body)."""
    assert etag_matches(None, "W/abc") is False
    assert etag_matches("", "W/abc") is False
