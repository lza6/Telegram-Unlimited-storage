"""Application configuration loaded from environment variables / .env file.

Mirrors the environment surface of the legacy Rust headless server so existing
deployments (.env, docker-compose) keep working unchanged.
"""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from typing import Literal, Optional

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


# Repository root: backend/app/config.py -> app -> backend -> repo root.
# Resolve the canonical .env location absolutely so the server loads credentials
# regardless of the process working directory (uvicorn --app-dir, start.bat,
# docker). A CWD-relative ".env" is kept as a secondary override source.
_REPO_ROOT = Path(__file__).resolve().parent.parent.parent


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=(_REPO_ROOT / ".env", ".env"),
        env_file_encoding="utf-8",
        extra="ignore",
        case_sensitive=False,
    )

    # ── Telegram credentials ────────────────────────────────────────────────
    telegram_api_id: Optional[int] = Field(default=None, alias="TELEGRAM_API_ID")
    telegram_api_hash: Optional[str] = Field(default=None, alias="TELEGRAM_API_HASH")
    # Transport mode: "user" (MTProto user account, legacy default) or "bot"
    # (bot pool uploading into a storage channel).
    telegram_transport_mode: Literal["user", "bot"] = Field(
        default="user", alias="TELEGRAM_TRANSPORT_MODE"
    )
    tg_bot_token: Optional[str] = Field(default=None, alias="TG_BOT_TOKEN")
    tg_storage_channel_id: Optional[int] = Field(
        default=None, alias="TG_STORAGE_CHANNEL_ID"
    )
    # Optional SOCKS5 proxy, e.g. socks5://user:pass@host:1080
    proxy_socks5: Optional[str] = Field(default=None, alias="PROXY_SOCKS5")

    # ── Authentication ──────────────────────────────────────────────────────
    # Web console password (header X-Access-Pwd).
    access_pwd: Optional[str] = Field(default=None, alias="ACCESS_PWD")
    # External integration API key (header X-API-Key).
    api_key: Optional[str] = Field(default=None, alias="API_KEY")
    access_lockout_max: int = Field(default=8, alias="ACCESS_LOCKOUT_MAX")
    access_lockout_secs: int = Field(default=300, alias="ACCESS_LOCKOUT_SECS")
    multi_tenant_enabled: bool = Field(default=False, alias="MULTI_TENANT_ENABLED")

    # ── Server ──────────────────────────────────────────────────────────────
    port: int = Field(default=1334, alias="PORT")
    bind_host: str = Field(default="127.0.0.1", alias="BIND_HOST")
    base_url: str = Field(default="http://localhost:1334", alias="BASE_URL")
    data_dir: Path = Field(default=Path("./data"), alias="DATA_DIR")
    static_dir: Optional[Path] = Field(default=None, alias="STATIC_DIR")
    docs_dir: Optional[Path] = Field(default=None, alias="DOCS_DIR")

    # ── Transfer tuning ─────────────────────────────────────────────────────
    download_threads: int = Field(default=8, alias="DOWNLOAD_THREADS")
    chunk_size_mb: int = Field(default=10, alias="CHUNK_SIZE_MB")
    chunk_concurrent: int = Field(default=4, alias="CHUNK_CONCURRENT")
    files_concurrent: int = Field(default=2, alias="FILES_CONCURRENT")
    max_upload_size_mb: int = Field(default=100, alias="MAX_UPLOAD_SIZE_MB")

    # ── Rate limiting ───────────────────────────────────────────────────────
    rate_limit_rpm: int = Field(default=120, alias="RATE_LIMIT_RPM")
    rate_limit_api_rpm: int = Field(default=300, alias="RATE_LIMIT_API_RPM")

    # ── Metadata cache ──────────────────────────────────────────────────────
    metadata_cache_enabled: bool = Field(default=True, alias="METADATA_CACHE_ENABLED")
    metadata_cache_ttl_secs: int = Field(default=300, alias="METADATA_CACHE_TTL_SECS")

    # ── Sharing / downloads ─────────────────────────────────────────────────
    public_file_id_download: bool = Field(
        default=False, alias="PUBLIC_FILE_ID_DOWNLOAD"
    )
    download_signing_secret: str = Field(
        default="insecure-dev-signing-secret-change-me",
        alias="DOWNLOAD_SIGNING_SECRET",
    )
    upload_link_ttl_secs: int = Field(default=0, alias="UPLOAD_LINK_TTL_SECS")
    upload_share_ttl_hours: int = Field(default=0, alias="UPLOAD_SHARE_TTL_HOURS")

    # ── Upload queue backend ────────────────────────────────────────────────
    upload_queue_backend: Literal["memory", "redis"] = Field(
        default="memory", alias="UPLOAD_QUEUE_BACKEND"
    )
    redis_url: Optional[str] = Field(default=None, alias="REDIS_URL")

    # ── CORS ─────────────────────────────────────────────────────────────────
    cors_origins: str = Field(default="http://localhost:1334", alias="CORS_ORIGINS")

    # ── Docs ─────────────────────────────────────────────────────────────────
    disable_docs: bool = Field(default=False, alias="DISABLE_DOCS")

    # ── Misc ────────────────────────────────────────────────────────────────
    webdav_enabled: bool = Field(default=False, alias="WEBDAV_ENABLED")
    metrics_enabled: bool = Field(default=True, alias="METRICS_ENABLED")

    @property
    def chunk_size_bytes(self) -> int:
        return self.chunk_size_mb * 1024 * 1024

    @property
    def session_path(self) -> Path:
        return self.data_dir / "telegram.session"

    @property
    def db_path(self) -> Path:
        return self.data_dir / "shares.db"

    @property
    def api_settings_path(self) -> Path:
        return self.data_dir / "api_settings.json"

    @property
    def resolved_static_dir(self) -> Path:
        if self.static_dir is not None:
            return self.static_dir
        # Repo layout: backend/app/config.py → repo root is parents[2].
        return Path(__file__).resolve().parents[2] / "deploy" / "web"

    @property
    def resolved_docs_dir(self) -> Path:
        if self.docs_dir is not None:
            return self.docs_dir
        return Path(__file__).resolve().parents[2] / "docs"


@lru_cache(maxsize=1)
def get_settings() -> Settings:
    return Settings()
