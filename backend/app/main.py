"""FastAPI application assembly — replaces the Rust/Tauri headless server.

Wires together storage, Telegram state, bot transport, authentication and the
transfer manager into a single ``AppState`` exposed on ``request.app.state.app``
(every router reads it from there). Serves the static Web UI (``deploy/web``)
and the ``docs/`` directory, and mounts all six API routers.

Security middleware reproduces the Rust backend's posture: security headers
with a per-request CSP nonce injected into served HTML, an ``X-Request-Id``
request logger, a per-IP sliding-window rate limiter (health paths exempt) and
CORS.
"""

from __future__ import annotations

import logging
import mimetypes
import secrets
import time
import uuid
from collections import deque
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse, JSONResponse, Response
from starlette.middleware.gzip import GZipMiddleware

from . import security
from .auth import Authenticator
from .bot_transport import BotTransport
from .config import Settings, get_settings
from .routers import auth as auth_router
from .routers import files as files_router
from .routers import health as health_router
from .routers import legacy as legacy_router
from .routers import settings as settings_router
from .routers import shares as shares_router
from .routers import webdav as webdav_router
from .settings_store import SettingsStore
from .state import AppState
from .storage import Storage
from .telegram_state import TelegramState
from .transfers import TransferManager

logger = logging.getLogger("telegram_drive")

# Paths exempt from rate limiting (probes hit these constantly).
_RATE_LIMIT_EXEMPT_PREFIXES = ("/health", "/api/v1/health", "/metrics")
_RATE_WINDOW_SECS = 60.0


# ── tenant bootstrap ────────────────────────────────────────────────────────
def _bootstrap_tenants(settings: Settings, storage: Storage) -> None:
    """Seed a default tenant from API_KEY when multi-tenant + table empty.

    Mirrors the Rust bootstrap: if the tenants table already has rows we leave
    it alone; otherwise a configured ``API_KEY`` becomes the ``default`` tenant.
    """
    if not settings.multi_tenant_enabled:
        return
    if storage.list_tenants():
        return
    if settings.api_key:
        storage.upsert_tenant(
            tenant_id="default",
            api_key_hash=security.hash_api_key(settings.api_key),
            display_name="Default",
        )
        logger.info("bootstrapped default tenant from API_KEY")


def _load_transport_override(settings: Settings) -> Optional[str]:
    """Restore the persisted transport_mode.json override (Rust parity)."""
    try:
        mode = SettingsStore(settings.data_dir).transport.load().get("mode")
    except OSError:
        return None
    return mode if mode in ("bot", "user") else None


def build_state(settings: Settings) -> AppState:
    """Construct the full AppState graph (no network I/O yet)."""
    settings.data_dir.mkdir(parents=True, exist_ok=True)
    storage = Storage(settings.db_path)
    telegram = TelegramState(
        api_id=settings.telegram_api_id,
        api_hash=settings.telegram_api_hash,
        data_dir=settings.data_dir,
        proxy_url=settings.proxy_socks5,
    )
    bot: Optional[BotTransport] = None
    if settings.tg_bot_token and settings.tg_storage_channel_id:
        bot = BotTransport(
            bot_token=settings.tg_bot_token,
            storage_channel_id=str(settings.tg_storage_channel_id),
            proxy_url=settings.proxy_socks5,
        )
    authenticator = Authenticator(settings, storage)
    transfers = TransferManager(
        file_slots=settings.files_concurrent,
        chunk_slots=settings.chunk_concurrent,
    )
    _bootstrap_tenants(settings, storage)
    return AppState(
        settings=settings,
        storage=storage,
        telegram=telegram,
        bot=bot,
        authenticator=authenticator,
        transfers=transfers,
        active_transport_mode=_load_transport_override(settings),
    )


# ── rate limiter ────────────────────────────────────────────────────────────
class SlidingWindowRateLimiter:
    """Per-IP fixed-window-over-60s request counter (in-memory)."""

    def __init__(self, max_requests: int) -> None:
        self.max_requests = max(1, max_requests)
        self._hits: dict[str, deque[float]] = {}

    def allow(self, key: str) -> tuple[bool, int]:
        """Returns (allowed, retry_after_secs)."""
        now = time.time()
        window = self._hits.setdefault(key, deque())
        while window and now - window[0] > _RATE_WINDOW_SECS:
            window.popleft()
        if len(window) >= self.max_requests:
            retry_after = int(_RATE_WINDOW_SECS - (now - window[0])) + 1
            return False, max(1, retry_after)
        window.append(now)
        return True, 0


# ── middleware ──────────────────────────────────────────────────────────────
def _client_ip(request: Request) -> str:
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"


def _security_headers(response: Response, nonce: str) -> None:
    response.headers.setdefault("X-Content-Type-Options", "nosniff")
    response.headers.setdefault("X-Frame-Options", "DENY")
    response.headers.setdefault("Referrer-Policy", "strict-origin-when-cross-origin")
    response.headers.setdefault(
        "Permissions-Policy", "camera=(), microphone=(), geolocation=()"
    )
    response.headers.setdefault(
        "Content-Security-Policy",
        "default-src 'self'; "
        f"script-src 'self' 'nonce-{nonce}'; "
        f"style-src 'self' 'nonce-{nonce}'; "
        "img-src 'self' data: blob:; "
        "media-src 'self' blob:; "
        "connect-src 'self' ws: wss:; "
        "font-src 'self' data:; "
        "base-uri 'self'; frame-ancestors 'none'",
    )


def _inject_nonce(html: str, nonce: str) -> str:
    """Add the CSP nonce to inline <script>/<style> tags in served HTML."""
    return (
        html.replace("<script>", f'<script nonce="{nonce}">')
        .replace("<script ", f'<script nonce="{nonce}" ')
        .replace("<style>", f'<style nonce="{nonce}">')
        .replace("<style ", f'<style nonce="{nonce}" ')
    )


# ── static file serving ─────────────────────────────────────────────────────
def _safe_resolve(root: Path, rel: str) -> Optional[Path]:
    """Resolve rel under root, rejecting path traversal (returns None if unsafe)."""
    root = root.resolve()
    target = (root / rel).resolve()
    try:
        target.relative_to(root)
    except ValueError:
        return None
    return target


def _serve_file(path: Path, nonce: str) -> Response:
    mime, _ = mimetypes.guess_type(str(path))
    if path.suffix.lower() in (".html", ".htm"):
        html = path.read_text(encoding="utf-8")
        response = HTMLResponse(_inject_nonce(html, nonce))
        _security_headers(response, nonce)
        return response
    return FileResponse(path, media_type=mime or "application/octet-stream")


# ── lifespan ────────────────────────────────────────────────────────────────
@asynccontextmanager
async def lifespan(app: FastAPI):
    import asyncio
    settings: Settings = app.state.settings
    state = build_state(settings)
    app.state.app = state
    app.state.rate_limiter = SlidingWindowRateLimiter(settings.rate_limit_rpm)
    logger.info(
        "Telegram Drive %s starting on %s:%s (transport=%s)",
        state.version,
        settings.bind_host,
        settings.port,
        state.effective_transport_mode(),
    )
    if settings.download_signing_secret == "insecure-dev-signing-secret-change-me":
        logger.warning(
            "DOWNLOAD_SIGNING_SECRET is using the default value — "
            "set a unique secret for production deployments"
        )

    # Background task: periodic cleanup of expired progress/download records
    async def _periodic_prune():
        try:
            while True:
                await asyncio.sleep(300)  # every 5 minutes
                state.transfers.prune_progress()
        except asyncio.CancelledError:
            pass

    prune_task = asyncio.create_task(_periodic_prune())

    # Background task: poll channel posts (bot mode)
    poll_task = None
    if state.bot is not None:
        poll_task = state.bot.start_polling(state.storage)
        logger.info("started channel post polling for bot mode")

    try:
        yield
    finally:
        prune_task.cancel()
        try:
            await prune_task
        except asyncio.CancelledError:
            pass
        if poll_task is not None:
            poll_task.cancel()
            try:
                await poll_task
            except asyncio.CancelledError:
                pass
        logger.info("shutting down")
        try:
            await state.telegram.disconnect()
        except Exception as exc:  # noqa: BLE001
            logger.warning("telegram disconnect failed: %s", exc)
        if state.bot is not None:
            try:
                await state.bot.close()
            except Exception as exc:  # noqa: BLE001
                logger.warning("bot close failed: %s", exc)
        try:
            state.storage.close()
        except Exception as exc:  # noqa: BLE001
            logger.warning("storage close failed: %s", exc)


def create_app(settings: Optional[Settings] = None) -> FastAPI:
    settings = settings or get_settings()
    app = FastAPI(
        title="Telegram Drive",
        version="1.0.0-python",
        lifespan=lifespan,
        docs_url="/api/docs" if not settings.disable_docs else None,
        redoc_url="/api/redoc" if not settings.disable_docs else None,
    )
    app.state.settings = settings

    # GZip before CORS so compressed responses get correct headers.
    app.add_middleware(GZipMiddleware, minimum_size=1024)

    origins = [o.strip() for o in settings.cors_origins.split(",") if o.strip()]
    app.add_middleware(
        CORSMiddleware,
        allow_origins=origins,
        allow_credentials=True,
        allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
        allow_headers=["*"],
    )

    # ── request pipeline: X-Request-Id + rate limit + security headers ──────
    @app.middleware("http")
    async def request_pipeline(request: Request, call_next):
        request_id = request.headers.get("x-request-id") or uuid.uuid4().hex
        nonce = secrets.token_urlsafe(16)
        request.state.nonce = nonce

        path = request.url.path
        if not path.startswith(_RATE_LIMIT_EXEMPT_PREFIXES):
            limiter: SlidingWindowRateLimiter = app.state.rate_limiter
            allowed, retry_after = limiter.allow(_client_ip(request))
            if not allowed:
                return JSONResponse(
                    {"error": {"code": "RATE_LIMITED", "message": "Too many requests"}},
                    status_code=429,
                    headers={"Retry-After": str(retry_after)},
                )

        response = await call_next(request)
        response.headers["X-Request-Id"] = request_id
        # Security headers (CSP nonce only meaningful for HTML we serve).
        _security_headers(response, nonce)
        logger.info(
            "%s %s %s rid=%s",
            request.method,
            path,
            response.status_code,
            request_id,
        )
        return response

    # ── API routers ─────────────────────────────────────────────────────────
    app.include_router(health_router.router)
    app.include_router(auth_router.router)
    app.include_router(files_router.router)
    app.include_router(shares_router.router)
    app.include_router(settings_router.router)
    app.include_router(legacy_router.router)
    app.include_router(webdav_router.router)

    # ── docs directory: /docs/* ─────────────────────────────────────────────
    @app.get("/docs/{rel:path}")
    async def serve_docs(rel: str, request: Request):
        root = app.state.settings.resolved_docs_dir
        target = _safe_resolve(root, rel)
        if target is None or not target.is_file():
            return JSONResponse({"error": "Not found"}, status_code=404)
        return _serve_file(target, request.state.nonce)

    # ── static Web UI: /* (multi-page, index.html fallback) ─────────────────
    @app.get("/{rel:path}")
    async def serve_static(rel: str, request: Request):
        root = app.state.settings.resolved_static_dir
        target = _safe_resolve(root, rel)
        if target is not None and target.is_file():
            return _serve_file(target, request.state.nonce)
        # SPA-style fallback to index.html for unknown non-API paths.
        index = root / "index.html"
        if index.is_file():
            return _serve_file(index, request.state.nonce)
        return JSONResponse({"error": "Not found"}, status_code=404)

    return app


app = create_app()


def main() -> None:
    import uvicorn

    settings = get_settings()
    uvicorn.run(
        "app.main:app",
        host=settings.bind_host,
        port=settings.port,
        log_level="info",
    )


if __name__ == "__main__":
    main()
