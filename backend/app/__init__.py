"""Telegram Drive Web API (Python).

Single Source of Truth for version & build metadata.

All version-facing surfaces (FastAPI app title, health endpoint, state
property, docs) MUST import from here — never hardcode a version string.
"""

from __future__ import annotations

import os
import sys

__version__ = "7.0.0-python"

# Build date stamped at release time; can be overridden via env for reproducible builds.
__build_date__: str = os.environ.get("APP_BUILD_DATE", "2026-07-27")

# Git SHA injected at build time (Dockerfile build-arg -> env). Empty in dev.
__git_sha__: str = os.environ.get("APP_GIT_SHA", "")


def python_version() -> str:
    """Return the running Python version (e.g. '3.11.9')."""
    v = sys.version_info
    return f"{v.major}.{v.minor}.{v.micro}"


def build_metadata() -> dict[str, str]:
    """Return a stable metadata dict for health/config endpoints."""
    meta = {
        "version": __version__,
        "build_date": __build_date__,
        "python_version": python_version(),
    }
    if __git_sha__:
        meta["git_sha"] = __git_sha__
    return meta


__all__ = [
    "__version__",
    "__build_date__",
    "__git_sha__",
    "python_version",
    "build_metadata",
]
