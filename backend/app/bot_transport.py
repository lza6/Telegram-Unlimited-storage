"""Bot-mode transport — pure HTTP Bot API (port of telegram_transport.rs).

Uploads go through ``sendDocument`` into the storage channel
(``TG_STORAGE_CHANNEL_ID``); downloads resolve ``getFile`` and stream from
``https://api.telegram.org/file/bot<token>/<file_path>`` with HTTP Range.
Single-file cap is 20 MB (Bot API limit).
"""

from __future__ import annotations

import asyncio
import json
import logging
import mimetypes
import time
from dataclasses import dataclass
from pathlib import Path
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

    async def close(self) -> None:
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

    def start_polling(self, storage) -> asyncio.Task:
        """Start background task that polls getUpdates for channel posts.
        
        First call with offset=0 fetches all pending updates (including
        recent channel posts before the bot started). Subsequent calls
        use long-polling for new posts.
        """
        self._storage = storage
        self._chat_page: dict[int, int] = {}  # track current page per chat
        self._chat_album: dict[int, list[int]] = {}  # track album msg_ids per chat
        self._chat_list_msg: dict[int, int] = {}  # track list msg_id per chat
        self._poller_task = asyncio.create_task(self._poll_channel_posts())
        return self._poller_task

    def stop_polling(self) -> None:
        if hasattr(self, "_poller_task"):
            self._poller_task.cancel()

    async def _poll_channel_posts(self) -> None:
        """Long-poll getUpdates for channel_posts and DM messages."""
        offset = 0
        while True:
            try:
                params: dict[str, Any] = {
                    "timeout": 30,
                    "allowed_updates": ["channel_post", "message", "callback_query"],
                    "offset": offset,  # 0 on first call → fetch all pending
                }
                result = await self._call("getUpdates", params=params)
                if not result:
                    continue
                for update in result:
                    uid = update.get("update_id", 0)
                    offset = max(offset, uid + 1)

                    # Handle callback queries (inline button clicks)
                    cq = update.get("callback_query")
                    if cq:
                        await self._handle_callback(cq)
                        continue

                    post = update.get("channel_post") or update.get("message")
                    if not post:
                        continue

                    chat_id = post.get("chat", {})
                    cid = str(chat_id.get("id")) if isinstance(chat_id, dict) else ""

                    if cid == self.storage_channel_id:
                        await self._index_update(post)
                    elif isinstance(chat_id, dict) and chat_id.get("type") == "private":
                        await self._handle_dm(post)
            except asyncio.CancelledError:
                break
            except BotFloodWaitError:
                await asyncio.sleep(5)
            except Exception:
                logger.exception("channel poll error, retry in 10s")
                await asyncio.sleep(10)

    # ── DM command handling ──────────────────────────────────────────────────

    async def _check_channel_member(self, user_id: int) -> bool:
        """Check if a user is a member of the storage channel."""
        try:
            result = await self._call("getChatMember", data={
                "chat_id": self.storage_channel_id,
                "user_id": user_id,
            })
            status = result.get("status", "")
            return status in ("creator", "administrator", "member")
        except Exception:
            return False

    async def _fetch_text_preview(self, file_id: str) -> str:
        """Fetch first 10 readable characters of a file."""
        try:
            fpath = await self.get_file_path(file_id)
            url = f"{BOT_API_BASE}/file/bot{self.bot_token}/{fpath}"
            async with self._client.stream("GET", url) as resp:
                if resp.status_code == 200:
                    raw = b""
                    async for chunk in resp.aiter_bytes():
                        raw += chunk
                        if len(raw) >= 200:
                            break
                    decoded = raw.decode("utf-8")
                    return decoded[:10].strip()
        except Exception:
            return ""
        return ""

    async def _handle_dm(self, msg: dict) -> None:
        """Process a DM sent to the bot."""
        chat = msg.get("chat", {})
        chat_id = chat.get("id") if isinstance(chat, dict) else None
        from_id = msg.get("from", {}).get("id") if isinstance(msg.get("from"), dict) else chat_id
        if not chat_id:
            return

        # Auth check: only channel members
        if not await self._check_channel_member(from_id or chat_id):
            await self._send_message(chat_id,
                "This bot is only available to members of the storage channel. "
                "Join the channel first and try again.")
            return

        text = (msg.get("text") or "").strip()
        entities = msg.get("entities") or []

        # Check for bot commands
        cmd = ""
        if entities:
            for e in entities:
                if e.get("type") == "bot_command":
                    cmd = text[e.get("offset", 0):e.get("offset", 0) + e.get("length", 0)]
                    break

        if cmd == "/start" or cmd == "/help":
            await self._send_message(chat_id,
                "Telegram Drive Bot\n\n"
                "Commands:\n"
                "/files — list all files\n"
                "/search <name> — search files by name\n"
                "/help — this message\n\n"
                "Or just type a filename to search.")
        elif cmd == "/files":
            await self._send_file_list(chat_id, page=1)
        elif cmd == "/search" or text.startswith("/search "):
            query = text[len("/search "):].strip() if text.startswith("/search ") else ""
            if query:
                await self._send_search_results(chat_id, query)
            else:
                await self._send_message(chat_id, "Usage: /search <filename>")
        else:
            # Treat bare text as filename search
            await self._send_search_results(chat_id, text)

    async def _send_message(self, chat_id: int, text: str, **kwargs) -> None:
        """Send a text message (optionally with reply_markup)."""
        data: dict[str, Any] = {"chat_id": chat_id, "text": text}
        if "reply_markup" in kwargs:
            data["reply_markup"] = json.dumps(kwargs["reply_markup"])
        try:
            await self._call("sendMessage", data=data)
        except Exception:
            logger.exception("sendMessage failed to %d", chat_id)

    async def _send_file_list(self, chat_id: int, page: int = 1) -> None:
        """Show paginated file list with all media inline."""
        storage = getattr(self, "_storage", None)
        if not storage:
            await self._send_message(chat_id, "Storage not available")
            return

        files = storage.list_bot_files(limit=1000)
        if not files:
            await self._send_message(chat_id, "No files yet.")
            return

        per_page = 5
        total_pages = max(1, (len(files) + per_page - 1) // per_page)
        page = max(1, min(page, total_pages))
        self._chat_page[chat_id] = page
        start = (page - 1) * per_page
        page_files = files[start:start + per_page]

        # ── build list text ──────────────────────────────────────────────────
        lines = [f"\U0001F4C1 Files (page {page}/{total_pages}):\n"]
        for f in page_files:
            name = f.get("file_name") or f"file_{f['message_id']}"
            size = int(f.get("file_size") or 0)
            size_str = f"{size / 1024:.1f}KB" if size < 1024 * 1024 else f"{size / 1024 / 1024:.1f}MB"
            ts = int(f.get("created_at") or 0)
            time_str = time.strftime("%Y-%m-%d %H:%M", time.gmtime(ts)) if ts else "unknown"

            preview = ""
            mime = (mimetypes.guess_type(name)[0] or "").lower()
            fidi = f.get("telegram_file_id")
            if fidi and (mime.startswith("text/") or ".md" in name.lower() or ".txt" in name.lower()):
                preview = await self._fetch_text_preview(fidi)

            line = f"\U0001F4CE {name} — {size_str}  [{time_str}]"
            if preview:
                line += f"\n   {preview}..."
            lines.append(line)
        list_text = "\n".join(lines)

        # ── collect all media from current page ──────────────────────────────
        media_group: list[dict[str, str]] = []
        for f in page_files:
            name = f.get("file_name") or f"file_{f['message_id']}"
            fid = f.get("telegram_file_id")
            mime = (mimetypes.guess_type(name)[0] or "").lower()
            if not fid:
                continue
            if mime.startswith("image/"):
                media_group.append({"type": "photo", "media": fid})
            elif mime.startswith("video/"):
                media_group.append({"type": "video", "media": fid})

        # ── inline keyboard ──────────────────────────────────────────────────
        keyboard: list[list[dict[str, str]]] = []
        if total_pages > 1:
            row: list[dict[str, str]] = []
            if page > 1:
                row.append({"text": "\u25C0 Prev", "callback_data": f"page_{page - 1}"})
            row.append({"text": f"{page}/{total_pages}", "callback_data": "noop"})
            if page < total_pages:
                row.append({"text": "Next \u25B6", "callback_data": f"page_{page + 1}"})
            keyboard.append(row)
        for f in page_files:
            mid = f["message_id"]
            nm = f.get("file_name") or f"file_{mid}"
            keyboard.append([{"text": f"\U0001F4CE {nm}", "callback_data": f"get_{mid}"}])
        keyboard.append([{"text": "\u274C Close", "callback_data": "cancel"}])

        # ── always delete old album + list (from previous page or request) ────
        old_album = self._chat_album.pop(chat_id, [])
        for old_mid in old_album:
            try:
                await self._call("deleteMessage", data={"chat_id": chat_id, "message_id": old_mid})
            except Exception:
                pass
        old_list = self._chat_list_msg.pop(chat_id, None)
        if old_list:
            try:
                await self._call("deleteMessage", data={"chat_id": chat_id, "message_id": old_list})
            except Exception:
                pass

        # ── send album ───────────────────────────────────────────────────────
        if media_group:
            try:
                result = await self._call("sendMediaGroup", data={
                    "chat_id": chat_id,
                    "media": json.dumps(media_group),
                })
                self._chat_album[chat_id] = [r["message_id"] for r in result]
            except Exception:
                logger.exception("sendMediaGroup failed")
        else:
            self._chat_album.pop(chat_id, None)

        # ── send list + keyboard ─────────────────────────────────────────────
        try:
            msg = await self._call("sendMessage", data={
                "chat_id": chat_id,
                "text": list_text,
                "reply_markup": json.dumps({"inline_keyboard": keyboard}),
            })
            self._chat_list_msg[chat_id] = msg.get("message_id", 0)
        except Exception as e:
            logger.warning("sendMessage failed: %s", e)

    async def _send_search_results(self, chat_id: int, query: str) -> None:
        """Search files by name and show results with previews."""
        if not query:
            await self._send_message(chat_id, "Send a filename to search.")
            return

        storage = getattr(self, "_storage", None)
        if not storage:
            return

        files = storage.list_bot_files(limit=1000)
        matches = [f for f in files if query.lower() in (f.get("file_name") or "").lower()]

        if not matches:
            await self._send_message(chat_id, f"No files matching \"{query}\".")
            return

        # Build result text
        lines = [f"Found {len(matches)} file(s) for \"{query}\":\n"]
        for f in matches[:10]:
            name = f.get("file_name") or f"file_{f['message_id']}"
            size = int(f.get("file_size") or 0)
            size_str = f"{size / 1024:.1f}KB" if size < 1024 * 1024 else f"{size / 1024 / 1024:.1f}MB"
            ts = int(f.get("created_at") or 0)
            time_str = time.strftime("%Y-%m-%d %H:%M", time.gmtime(ts)) if ts else "unknown"

            preview = ""
            fid = f.get("telegram_file_id")
            if fid:
                fmime = (mimetypes.guess_type(name)[0] or "").lower()
                if fmime.startswith("text/") or name.lower().endswith((".md", ".txt")):
                    preview = await self._fetch_text_preview(fid)

            line = f"\U0001F4CE {name} — {size_str}  [{time_str}]"
            if preview:
                line += f"\n   {preview}..."
            lines.append(line)
        list_text = "\n".join(lines)

        keyboard = []
        for f in matches[:10]:
            mid = f["message_id"]
            name = f.get("file_name") or f"file_{mid}"
            keyboard.append([{"text": f"\U0001F4CE {name}", "callback_data": f"get_{mid}"}])

        await self._send_message(
            chat_id, list_text,
            reply_markup={"inline_keyboard": keyboard},
        )

    async def _handle_callback(self, cq: dict) -> None:
        """Handle inline keyboard callback."""
        data = (cq.get("data") or "").strip()
        msg = cq.get("message") or {}
        chat = msg.get("chat") or {}
        chat_id = chat.get("id") if isinstance(chat, dict) else None
        cq_id = cq.get("id")
        msg_id = msg.get("message_id")
        if not chat_id or not cq_id:
            return

        # Answer callback to remove loading state
        try:
            await self._call("answerCallbackQuery", data={"callback_query_id": cq_id})
        except Exception:
            pass

        if data == "noop":
            return

        if data == "cancel":
            for mid in self._chat_album.pop(chat_id, []):
                try:
                    await self._call("deleteMessage", data={"chat_id": chat_id, "message_id": mid})
                except Exception:
                    pass
            list_mid = self._chat_list_msg.pop(chat_id, None)
            if list_mid:
                try:
                    await self._call("deleteMessage", data={"chat_id": chat_id, "message_id": list_mid})
                except Exception:
                    pass
            if msg_id:
                try:
                    await self._call("deleteMessage", data={"chat_id": chat_id, "message_id": msg_id})
                except Exception:
                    pass
            return

        if data.startswith("page_"):
            page = int(data.split("_")[1])
            await self._send_file_list(chat_id, page=page)
        elif data.startswith("get_"):
            file_mid = int(data.split("_")[1])
            await self._send_file_to_dm(chat_id, file_mid)

    async def _send_file_to_dm(self, chat_id: int, message_id: int) -> None:
        """Send a file from the storage channel to the user's DM."""
        storage = getattr(self, "_storage", None)
        if not storage:
            return

        row = storage.get_bot_file(message_id)
        if not row:
            await self._send_message(chat_id, "File not found.")
            return

        file_id = row.get("telegram_file_id")
        name = row.get("file_name") or f"file_{message_id}"
        if not file_id:
            await self._send_message(chat_id, "File unavailable.")
            return

        mime = (mimetypes.guess_type(name)[0] or "").lower()
        name_lower = name.lower()

        # Image files → sendPhoto for inline preview
        if mime.startswith("image/"):
            try:
                await self._call("sendPhoto", data={
                    "chat_id": chat_id,
                    "photo": file_id,
                    "caption": name,
                })
                return
            except Exception:
                pass  # fall through to copyMessage

        # Text-like files → download first bytes and show preview
        preview = ""
        try:
            fpath = await self.get_file_path(file_id)
            url = f"{BOT_API_BASE}/file/bot{self.bot_token}/{fpath}"
            async with self._client.stream("GET", url) as resp:
                if resp.status_code == 200:
                    raw = b""
                    async for chunk in resp.aiter_bytes():
                        raw += chunk
                        if len(raw) >= 200:
                            break
                    # Check if decodable as UTF-8 text
                    try:
                        decoded = raw.decode("utf-8")
                        preview = decoded[:10].strip()
                    except (UnicodeDecodeError, UnicodeError):
                        preview = ""
        except Exception:
            pass

        caption = name
        if preview:
            caption += f"\n\n{preview}..."
        try:
            await self._call("sendDocument", data={
                "chat_id": chat_id,
                "document": file_id,
                "caption": caption,
            })
            return
        except Exception:
            pass  # fall through

        try:
            # Forward the file from the storage channel to the user
            await self._call("copyMessage", data={
                "chat_id": chat_id,
                "from_chat_id": self.storage_channel_id,
                "message_id": message_id,
            })
        except Exception:
            # Fallback: send via file_id
            try:
                await self._call("sendDocument", data={
                    "chat_id": chat_id,
                    "document": file_id,
                })
            except Exception as exc:
                await self._send_message(chat_id, f"Failed to send: {exc}")

    async def _index_update(self, post: dict) -> None:
        """Index a single channel_post / forwarded message into bot_file_map."""
        chat_id = post.get("chat", {}).get("id")
        if str(chat_id) != self.storage_channel_id:
            return  # not our storage channel

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
            return  # not a file we can handle

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
