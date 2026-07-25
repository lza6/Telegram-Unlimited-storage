"""Bot-mode transport — pure HTTP Bot API (port of telegram_transport.rs).

Uploads go through ``sendDocument`` into the storage channel
(``TG_STORAGE_CHANNEL_ID``); downloads resolve ``getFile`` and stream from
``https://api.telegram.org/file/bot<token>/<file_path>`` with HTTP Range.
Single-file cap is 20 MB (Bot API limit).
"""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import Any, AsyncIterator, Optional

import httpx

logger = logging.getLogger("telegram_drive.bot")

BOT_SINGLE_FILE_MAX = 20 * 1024 * 1024
BOT_API_BASE = "https://api.telegram.org"


class BotTransportError(Exception):
    pass


class BotFloodWaitError(BotTransportError):
    def __init__(self, seconds: int) -> None:
        super().__init__(f"FLOOD_WAIT_{seconds}")
        self.seconds = seconds


@dataclass
class BotUploadResult:
    message_id: int
    telegram_file_id: str
    file_name: str
    file_size: int
    chat_id: int


class BotTransport:
    """HTTP Bot API transport for a single bot token."""

    def __init__(
        self,
        bot_token: str,
        storage_channel_id: str,
        proxy_url: Optional[str] = None,
    ) -> None:
        if not bot_token:
            raise BotTransportError("TG_BOT_TOKEN is required for bot mode")
        if not storage_channel_id:
            raise BotTransportError(
                "TG_STORAGE_CHANNEL_ID is required for bot mode"
            )
        self.bot_token = bot_token
        self.storage_channel_id = storage_channel_id
        transport = httpx.AsyncHTTPTransport(retries=3)
        client_kwargs: dict[str, Any] = {
            "transport": transport,
            "timeout": httpx.Timeout(300.0, connect=15.0),
        }
        if proxy_url:
            client_kwargs["proxy"] = proxy_url
        self._client = httpx.AsyncClient(**client_kwargs)
        self._flood_until: float = 0.0
        self._storage: Any = None

    async def close(self) -> None:
        self.stop_polling()
        await self._client.aclose()

    def _url(self, method: str) -> str:
        return f"{BOT_API_BASE}/bot{self.bot_token}/{method}"

    @property
    def is_ready(self) -> bool:
        return True

    def flood_wait_remaining(self) -> float:
        return max(0.0, self._flood_until - time.time())

    async def _call(self, method: str, **kwargs: Any) -> dict[str, Any]:
        if self.flood_wait_remaining() > 0:
            raise BotFloodWaitError(int(self.flood_wait_remaining()))
        resp = await self._client.post(self._url(method), **kwargs)
        try:
            payload = resp.json()
        except ValueError as exc:
            raise BotTransportError(f"Bot {method} parse failed") from exc
        if not payload.get("ok"):
            code = payload.get("error_code")
            desc = payload.get("description", "unknown error")
            if code == 429:
                secs = int(
                    (payload.get("parameters") or {}).get("retry_after", 60)
                )
                self._flood_until = time.time() + secs
                raise BotFloodWaitError(secs)
            raise BotTransportError(f"Bot {method} rejected: {desc}")
        return payload.get("result") or {}

    async def get_me(self) -> dict[str, Any]:
        return await self._call("getMe")

    async def upload_bytes(
        self, data: bytes, filename: str, caption: str = ""
    ) -> BotUploadResult:
        if len(data) > BOT_SINGLE_FILE_MAX:
            raise BotTransportError(
                f"file exceeds {BOT_SINGLE_FILE_MAX // (1024 * 1024)} MB bot limit"
            )
        files = {"document": (filename, data)}
        form = {"chat_id": self.storage_channel_id, "caption": caption}
        result = await self._call("sendDocument", data=form, files=files)
        message_id = result.get("message_id")
        document = result.get("document") or {}
        file_id = document.get("file_id")
        if message_id is None or not file_id:
            raise BotTransportError("Bot sendDocument returned incomplete result")
        chat = result.get("chat") or {}
        return BotUploadResult(
            message_id=int(message_id),
            telegram_file_id=file_id,
            file_name=filename,
            file_size=len(data),
            chat_id=int(chat.get("id") or 0),
        )

    async def get_file_path(self, telegram_file_id: str) -> str:
        result = await self._call("getFile", params={"file_id": telegram_file_id})
        path = result.get("file_path")
        if not path:
            raise BotTransportError("Bot getFile returned no file_path")
        return path

    async def delete_message(self, message_id: int) -> None:
        await self._call(
            "deleteMessage",
            data={"chat_id": self.storage_channel_id, "message_id": message_id},
        )

    async def stream_download(
        self,
        telegram_file_id: str,
        offset: int = 0,
        length: Optional[int] = None,
        chunk_size: int = 64 * 1024,
    ) -> AsyncIterator[bytes]:
        """Stream file bytes, optionally ranged (offset/length)."""
        file_path = await self.get_file_path(telegram_file_id)
        url = f"{BOT_API_BASE}/file/bot{self.bot_token}/{file_path}"
        headers: dict[str, str] = {}
        if offset or length:
            end = f"{offset + length - 1}" if length else ""
            headers["Range"] = f"bytes={offset}-{end}"
        remaining = length
        async with self._client.stream("GET", url, headers=headers) as resp:
            if resp.status_code not in (200, 206):
                raise BotTransportError(
                    f"Bot file download failed: HTTP {resp.status_code}"
                )
            async for chunk in resp.aiter_bytes(chunk_size):
                if remaining is not None:
                    chunk = chunk[:remaining]
                    remaining -= len(chunk)
                yield chunk
                if remaining is not None and remaining <= 0:
                    break

    # ── channel post polling (sync new manual posts) ─────────────────────────

    def start_polling(self, storage: Any) -> asyncio.Task:
        """Start background task that polls getUpdates for channel posts.

        First call with offset=0 fetches all pending updates (including
        recent channel posts before the bot started). Subsequent calls
        use long-polling for new posts.
        """
        self._storage = storage
        self._poller_task = asyncio.create_task(self._poll_channel_posts())
        return self._poller_task

    def stop_polling(self) -> None:
        if hasattr(self, "_poller_task"):
            self._poller_task.cancel()

    async def _poll_channel_posts(self) -> None:
        """Long-poll getUpdates for channel_post and index files."""
        offset = 0
        while True:
            try:
                params: dict[str, Any] = {
                    "timeout": 30,
                    "allowed_updates": ["channel_post", "message"],
                    "offset": offset,
                }
                result = await self._call("getUpdates", params=params)
                if not result:
                    continue
                for update in result:
                    uid = update.get("update_id", 0)
                    offset = max(offset, uid + 1)
                    post = update.get("channel_post") or update.get("message")
                    if not post:
                        continue
                    await self._index_update(post)
            except asyncio.CancelledError:
                break
            except BotFloodWaitError:
                await asyncio.sleep(5)
            except Exception:
                logger.exception("channel poll error, retry in 10s")
                await asyncio.sleep(10)

    async def _index_update(self, post: dict) -> None:
        """Index a single channel_post / forwarded message into bot_file_map."""
        chat_id = post.get("chat", {}).get("id")
        if str(chat_id) != self.storage_channel_id:
            return

        message_id = post.get("message_id")
        if message_id is None:
            return

        caption = post.get("caption") or ""
        doc = post.get("document")
        photo = post.get("photo")
        video = post.get("video")
        audio = post.get("audio")

        file_id = file_name = file_size = None

        if doc:
            file_id = doc.get("file_id")
            file_name = doc.get("file_name") or f"file_{message_id}"
            file_size = doc.get("file_size", 0)
        elif video:
            file_id = video.get("file_id")
            ext = (video.get("mime_type") or "video/mp4").split("/")[-1] or "mp4"
            file_name = f"video_{message_id}.{ext}"
            file_size = video.get("file_size", 0)
        elif audio:
            file_id = audio.get("file_id")
            ext = (audio.get("mime_type") or "audio/ogg").split("/")[-1] or "ogg"
            file_name = audio.get("file_name") or f"audio_{message_id}.{ext}"
            file_size = audio.get("file_size", 0)
        elif photo:
            largest = max(photo, key=lambda p: p.get("file_size", 0))
            file_id = largest.get("file_id")
            file_name = f"photo_{message_id}.jpg"
            file_size = largest.get("file_size", 0)

        if not file_id:
            return

        storage = getattr(self, "_storage", None)
        if not storage:
            return

        try:
            existing = storage.get_bot_file(message_id)
            if existing:
                return
            storage.record_bot_file(
                message_id=message_id,
                telegram_file_id=file_id,
                file_name=file_name,
                file_size=file_size or 0,
                caption=caption or None,
                bot_pool_index=0,
            )
            logger.info(
                "indexed channel post %d: %s (%d bytes)",
                message_id, file_name, file_size or 0,
            )
        except Exception:
            logger.exception("failed to index channel post %d", message_id)
