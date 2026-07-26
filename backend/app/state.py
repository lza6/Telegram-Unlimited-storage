"""Application state shared across routers."""

from __future__ import annotations

import secrets
import time
from dataclasses import dataclass, field
from typing import Optional

from .auth import Authenticator
from .bot_transport import BotTransport
from .config import Settings
from .storage import Storage
from .telegram_state import TelegramState
from .transfers import TransferManager


@dataclass
class AppState:
    settings: Settings
    storage: Storage
    telegram: TelegramState
    bot: Optional[BotTransport]
    authenticator: Authenticator
    transfers: TransferManager
    started_at: float = field(default_factory=time.time)
    # Session-level token guarding /stream URLs (constant-time compared).
    stream_token: str = field(default_factory=lambda: secrets.token_hex(16))
    # Runtime transport mode override (persisted in transport_mode.json).
    active_transport_mode: Optional[str] = None
    # Share password-verify brute-force limiter (keyed by share token).
    share_verify_attempts: dict[str, list[float]] = field(default_factory=dict)

    @property
    def version(self) -> str:
        return "2.0.0-python"

    @property
    def uptime_secs(self) -> int:
        return int(time.time() - self.started_at)

    # ── transport mode resolution ───────────────────────────────────────────
    @property
    def bot_configured(self) -> bool:
        return bool(self.settings.tg_bot_token and self.settings.tg_storage_channel_id)

    @property
    def user_configured(self) -> bool:
        return bool(self.settings.telegram_api_id and self.settings.telegram_api_hash)

    @property
    def default_transport_mode(self) -> str:
        return self.settings.telegram_transport_mode

    def effective_transport_mode(self) -> str:
        """active override (transport_mode.json) → default (env)."""
        mode = self.active_transport_mode or self.default_transport_mode
        # Fall back like the Rust impl: bot requested but unconfigured → user.
        if mode == "bot" and not self.bot_configured:
            return "user" if self.user_configured else "bot"
        return mode

    async def is_ready(self) -> bool:
        mode = self.effective_transport_mode()
        if mode == "bot":
            return self.bot_configured
        return await self.telegram.is_authorized()
