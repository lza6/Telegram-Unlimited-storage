"""Telegram user-mode client management (Telethon port of the grammers layer).

Data-format compatibility contract (from the Rust backend):
- Files are plain Telegram documents with an EMPTY caption; metadata comes
  from native document attributes only.
- Folders are broadcast channels titled "{name} [TD]" with about text
  "Telegram Drive Storage Folder\n[telegram-drive-folder]".
- folder_id = raw channel id; None means Saved Messages (self peer).

Note: Telethon's SQLite session format is NOT compatible with grammers' —
users must re-login once after the Python migration.
"""

from __future__ import annotations

import asyncio
import logging
import re
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

from telethon import TelegramClient, errors
from telethon.tl.functions.channels import (
    CreateChannelRequest,
    DeleteChannelRequest,
    GetFullChannelRequest,
)
from telethon.tl.functions.messages import SetHistoryTTLRequest
from telethon.tl.types import (
    Document,
    InputMessagesFilterDocument,
    MessageMediaDocument,
    MessageMediaPhoto,
)

logger = logging.getLogger("telegram_drive.telegram")

FOLDER_TITLE_SUFFIX = " [TD]"
FOLDER_ABOUT_TEXT = "Telegram Drive Storage Folder\n[telegram-drive-folder]"
FOLDER_ABOUT_MARKER = "[telegram-drive-folder]"
SESSION_BACKUP_NAME = "telegram.session.backup"
SQLITE_MAGIC = b"SQLite format 3\x00"

_FLOOD_RE = re.compile(r"FLOOD_WAIT.*?\(value:\s*(\d+)\)|FLOOD_WAIT_(\d+)", re.I)


def map_telegram_error(exc: BaseException) -> str:
    """Map a Telethon error to the legacy error-code string."""
    if isinstance(exc, errors.FloodWaitError):
        return f"FLOOD_WAIT_{exc.seconds}"
    text = str(exc)
    m = _FLOOD_RE.search(text)
    if m:
        return f"FLOOD_WAIT_{m.group(1) or m.group(2)}"
    return text


def _parse_proxy(proxy_url: Optional[str]):
    """Parse socks5://[user:pass@]host:port into a python-socks tuple."""
    if not proxy_url or not proxy_url.lower().startswith("socks5://"):
        return None
    rest = proxy_url[len("socks5://") :]
    user = password = None
    if "@" in rest:
        creds, rest = rest.rsplit("@", 1)
        if ":" in creds:
            user, password = creds.split(":", 1)
        else:
            user = creds
    if ":" not in rest:
        return None
    host, port_s = rest.rsplit(":", 1)
    try:
        port = int(port_s)
    except ValueError:
        return None
    return ("socks5", host, port, True, user, password)


@dataclass
class FolderInfo:
    id: Optional[int]  # None = Saved Messages
    name: str
    is_root: bool = False


@dataclass
class FileMetadata:
    id: int  # message id
    folder_id: Optional[int]
    name: str
    size: int
    mime_type: str
    file_ext: str
    created_at: int
    icon_type: str


@dataclass
class AuthResult:
    success: bool
    next_step: Optional[str] = None  # "code" | "password" | "dashboard" | "waiting"
    error: Optional[str] = None


@dataclass
class TelegramState:
    """Live Telegram session state — mirrors the Rust TelegramState."""

    api_id: Optional[int]
    api_hash: Optional[str]
    data_dir: Path
    proxy_url: Optional[str] = None

    client: Optional[TelegramClient] = None
    # Pending phone-code login: phone + phone_code_hash between request/sign-in.
    pending_phone: Optional[str] = None
    pending_phone_code_hash: Optional[str] = None
    password_pending: bool = False
    qr_login_obj: Any = None

    peer_cache: dict[int, Any] = field(default_factory=dict)
    cancelled_transfers: set[str] = field(default_factory=set)
    file_index_complete: bool = False

    _lock: asyncio.Lock = field(default_factory=asyncio.Lock)

    # ── session file handling ───────────────────────────────────────────────
    @property
    def session_path(self) -> Path:
        return self.data_dir / "telegram.session"

    def _session_str(self) -> str:
        # Telethon appends ".session" itself.
        return str(self.data_dir / "telegram")

    def _ensure_valid_session(self) -> None:
        """Restore the backup session file if the main one is corrupt/missing.

        Also detects grammers (Rust) session files: they are valid SQLite but
        have an EMPTY ``version`` table, which crashes Telethon's
        ``SQLiteSession.__init__`` (``fetchone()[0]`` on ``None``).  Such files
        are treated as incompatible so Telethon starts a fresh session.
        """
        main = self.session_path
        backup = self.data_dir / SESSION_BACKUP_NAME
        main_ok = self._is_valid_sqlite(main) and self._is_telethon_session(main)
        if main_ok:
            return
        if (
            backup.exists()
            and self._is_valid_sqlite(backup)
            and self._is_telethon_session(backup)
        ):
            logger.warning("main session corrupt/incompatible — restoring backup")
            shutil.copy2(backup, main)
        elif main.exists():
            # Corrupt or grammers-format with no usable backup: remove so
            # Telethon starts fresh (user re-logins once after migration).
            logger.warning(
                "session incompatible (grammers/corrupt) — removing for fresh start"
            )
            for suffix in ("", "-wal", "-shm"):
                p = Path(str(main) + suffix)
                if p.exists():
                    p.unlink()

    @staticmethod
    def _is_valid_sqlite(path: Path) -> bool:
        try:
            with open(path, "rb") as fh:
                header = fh.read(100)
            return len(header) >= 100 and header[:16] == SQLITE_MAGIC
        except OSError:
            return False

    @staticmethod
    def _is_telethon_session(path: Path) -> bool:
        """Return False for grammers/incompatible sessions that would crash Telethon.

        Telethon expects ``select version from version`` to return a row when
        the ``version`` table exists.  grammers creates the table but leaves it
        empty → ``fetchone()`` is ``None`` → ``None[0]`` raises ``TypeError``.
        A missing ``version`` table is fine (Telethon creates it from scratch).
        """
        try:
            import sqlite3 as _sqlite3

            conn = _sqlite3.connect(str(path))
            try:
                cur = conn.execute(
                    "select name from sqlite_master "
                    "where type='table' and name='version'"
                )
                if cur.fetchone() is None:
                    return True  # no version table — Telethon will create it
                cur = conn.execute("select version from version")
                return cur.fetchone() is not None  # grammers → empty → False
            finally:
                conn.close()
        except Exception:  # noqa: BLE001 — unreadable DB treated as invalid
            return False

    def backup_session(self) -> None:
        main = self.session_path
        if self._is_valid_sqlite(main):
            shutil.copy2(main, self.data_dir / SESSION_BACKUP_NAME)

    # ── client lifecycle ────────────────────────────────────────────────────
    # 有界连接预算：3 次尝试 × 5s 超时 ≈ 15s 最坏情况，
    # 远小于 httpx 客户端 30s 耐心，确保网络不可达时快速返回 503。
    _CONNECT_TIMEOUT: int = 5
    _CONNECT_RETRIES: int = 2

    def _build_client(self) -> TelegramClient:
        self._ensure_valid_session()
        proxy = _parse_proxy(self.proxy_url)
        client = TelegramClient(
            self._session_str(),
            self.api_id or 0,
            self.api_hash or "",
            proxy=proxy,
            timeout=self._CONNECT_TIMEOUT,
            connection_retries=self._CONNECT_RETRIES,
        )
        return client

    async def connect(self) -> TelegramClient:
        """Return a connected client, creating one if needed (with retries)."""
        async with self._lock:
            if self.client is not None and self.client.is_connected():
                return self.client
            if self.client is None:
                self.client = self._build_client()
            last_exc: Optional[Exception] = None
            for attempt in range(self._CONNECT_RETRIES + 1):
                try:
                    await asyncio.wait_for(
                        self.client.connect(),
                        timeout=self._CONNECT_TIMEOUT,
                    )
                    return self.client
                except Exception as exc:  # noqa: BLE001
                    last_exc = exc
                    if attempt < self._CONNECT_RETRIES:
                        logger.warning(
                            "connect attempt %d failed: %s", attempt + 1, exc
                        )
                        await asyncio.sleep(1.0 * (attempt + 1))
            raise last_exc or ConnectionError("connect failed")

    async def is_authorized(self) -> bool:
        try:
            client = await self.connect()
            return await client.is_user_authorized()
        except Exception as exc:  # noqa: BLE001 — surfaced as status, not raised
            logger.warning("authorization probe failed: %s", exc)
            return False

    async def disconnect(self) -> None:
        async with self._lock:
            if self.client is not None:
                try:
                    await self.client.disconnect()
                finally:
                    self.client = None

    async def logout(self) -> None:
        """Sign out, clear state, and delete session files."""
        async with self._lock:
            if self.client is not None:
                try:
                    if self.client.is_connected():
                        await self.client.log_out()
                except Exception as exc:  # noqa: BLE001 — best effort
                    logger.warning("sign_out failed: %s", exc)
                try:
                    await self.client.disconnect()
                except Exception:  # noqa: BLE001
                    pass
                self.client = None
        self.pending_phone = None
        self.pending_phone_code_hash = None
        self.password_pending = False
        self.qr_login_obj = None
        self.peer_cache.clear()
        self.cancelled_transfers.clear()
        self.file_index_complete = False
        for suffix in ("", "-wal", "-shm"):
            p = Path(str(self.session_path) + suffix)
            if p.exists():
                p.unlink()
        backup = self.data_dir / SESSION_BACKUP_NAME
        if backup.exists():
            backup.unlink()

    # ── auth flows ──────────────────────────────────────────────────────────
    async def request_login_code(self, phone: str) -> AuthResult:
        try:
            client = await self.connect()
            result = await client.send_code_request(phone)
            self.pending_phone = phone
            self.pending_phone_code_hash = getattr(result, "phone_code_hash", None)
            return AuthResult(success=True, next_step="code")
        except Exception as exc:  # noqa: BLE001
            return AuthResult(success=False, error=map_telegram_error(exc))

    async def sign_in_with_code(self, code: str) -> AuthResult:
        if not self.pending_phone or not self.pending_phone_code_hash:
            return AuthResult(
                success=False, error="No pending login — request a code first"
            )
        try:
            client = await self.connect()
            await client.sign_in(
                phone=self.pending_phone,
                code=code,
                phone_code_hash=self.pending_phone_code_hash,
            )
            self._on_login_success()
            return AuthResult(success=True, next_step="dashboard")
        except errors.SessionPasswordNeededError:
            self.password_pending = True
            return AuthResult(success=True, next_step="password")
        except Exception as exc:  # noqa: BLE001
            return AuthResult(success=False, error=map_telegram_error(exc))

    async def check_password(self, password: str) -> AuthResult:
        try:
            client = await self.connect()
            await client.sign_in(password=password)
            self._on_login_success()
            return AuthResult(success=True, next_step="dashboard")
        except Exception as exc:  # noqa: BLE001
            return AuthResult(success=False, error=f"2FA Failed: {map_telegram_error(exc)}")

    async def qr_start(self) -> AuthResult:
        try:
            client = await self.connect()
            qr = client.qr_login()
            await qr.wait(0)  # generate the token without blocking
            self.qr_login_obj = qr
            return AuthResult(success=True, next_step=qr.url)
        except Exception as exc:  # noqa: BLE001
            return AuthResult(success=False, error=map_telegram_error(exc))

    async def qr_poll(self) -> AuthResult:
        """Poll authorization status WITHOUT re-exporting the login token."""
        try:
            if await self.is_authorized():
                self._on_login_success()
                return AuthResult(success=True, next_step="dashboard")
            return AuthResult(success=True, next_step="waiting")
        except Exception:  # noqa: BLE001
            return AuthResult(success=True, next_step="waiting")

    def _on_login_success(self) -> None:
        self.pending_phone = None
        self.pending_phone_code_hash = None
        self.password_pending = False
        self.qr_login_obj = None
        self.backup_session()

    # ── peer / folder resolution ────────────────────────────────────────────
    async def resolve_peer(self, folder_id: Optional[int]):
        """None → self (Saved Messages); Some(channel_id) → cached/scanned peer."""
        client = await self.connect()
        if folder_id is None:
            return await client.get_me()
        cached = self.peer_cache.get(folder_id)
        if cached is not None:
            return cached
        await self._warm_peer_cache()
        cached = self.peer_cache.get(folder_id)
        if cached is None:
            raise ValueError(f"folder not found: {folder_id}")
        return cached

    async def _warm_peer_cache(self) -> None:
        client = await self.connect()
        cache: dict[int, Any] = {}
        async for dialog in client.iter_dialogs():
            entity = dialog.entity
            channel_id = getattr(entity, "id", None)
            if channel_id is not None and getattr(entity, "broadcast", False):
                cache[channel_id] = entity
        # Trim like the Rust impl (max 500).
        if len(cache) > 500:
            cache = dict(list(cache.items())[:500])
        self.peer_cache.update(cache)

    async def scan_folders(self) -> list[FolderInfo]:
        """Discover folders: title contains '[td]' or about has the marker."""
        client = await self.connect()
        folders: list[FolderInfo] = [FolderInfo(id=None, name="Saved Messages", is_root=True)]
        seen: set[int] = set()
        async for dialog in client.iter_dialogs():
            entity = dialog.entity
            if not getattr(entity, "broadcast", False):
                continue
            channel_id = getattr(entity, "id", None)
            if channel_id is None or channel_id in seen:
                continue
            title = getattr(entity, "title", "") or ""
            is_folder = "[td]" in title.lower()
            display = title
            if not is_folder and getattr(entity, "creator", False):
                try:
                    full = await client(GetFullChannelRequest(entity))
                    about = getattr(full.full_chat, "about", "") or ""
                    if FOLDER_ABOUT_MARKER in about:
                        is_folder = True
                except Exception:  # noqa: BLE001 — skip unreadable channels
                    continue
            if not is_folder:
                continue
            seen.add(channel_id)
            if display.lower().endswith(FOLDER_TITLE_SUFFIX.lower()):
                display = display[: -len(FOLDER_TITLE_SUFFIX)]
            self.peer_cache[channel_id] = entity
            folders.append(FolderInfo(id=channel_id, name=display))
        return folders

    async def create_folder(self, name: str) -> FolderInfo:
        client = await self.connect()
        title = f"{name}{FOLDER_TITLE_SUFFIX}"
        result = await client(
            CreateChannelRequest(
                title=title,
                about=FOLDER_ABOUT_TEXT,
                broadcast=True,
                megagroup=False,
            )
        )
        # Disable auto-delete history.
        chat = getattr(result, "chats", [None])[0]
        try:
            await client(SetHistoryTTLRequest(peer=chat, period=0))
        except Exception as exc:  # noqa: BLE001 — non-fatal
            logger.warning("SetHistoryTTL failed: %s", exc)
        channel_id = getattr(chat, "id", None)
        if channel_id is not None:
            self.peer_cache[channel_id] = chat
        return FolderInfo(id=channel_id, name=name)

    async def delete_folder(self, folder_id: int) -> None:
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        await client(DeleteChannelRequest(peer))
        self.peer_cache.pop(folder_id, None)

    # ── file metadata ───────────────────────────────────────────────────────
    @staticmethod
    def message_to_metadata(message: Any, folder_id: Optional[int]) -> Optional[FileMetadata]:
        """Map a Telegram message with media to FileMetadata (empty-caption format)."""
        media = getattr(message, "media", None)
        if media is None:
            return None
        doc: Optional[Document] = None
        if isinstance(media, MessageMediaDocument):
            d = media.document
            if isinstance(d, Document):
                doc = d
        if doc is not None:
            name = ""
            mime = doc.mime_type or ""
            for attr in doc.attributes:
                if getattr(attr, "file_name", None):
                    name = attr.file_name
                    break
            if not name:
                name = f"file_{message.id}"
            ext = name.rsplit(".", 1)[1].lower() if "." in name else ""
            return FileMetadata(
                id=message.id,
                folder_id=folder_id,
                name=name,
                size=doc.size or 0,
                mime_type=mime,
                file_ext=ext,
                created_at=int(getattr(message.date, "timestamp", lambda: 0)()),
                icon_type=_icon_type(ext, mime),
            )
        if isinstance(media, MessageMediaPhoto):
            return FileMetadata(
                id=message.id,
                folder_id=folder_id,
                name="Photo.jpg",
                size=0,
                mime_type="image/jpeg",
                file_ext="jpg",
                created_at=int(getattr(message.date, "timestamp", lambda: 0)()),
                icon_type="image",
            )
        return None

    async def list_files(self, folder_id: Optional[int]) -> list[FileMetadata]:
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        files: list[FileMetadata] = []
        async for message in client.iter_messages(peer):
            meta = self.message_to_metadata(message, folder_id)
            if meta is not None:
                files.append(meta)
        return files

    async def get_message(self, folder_id: Optional[int], message_id: int):
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        messages = await client.get_messages(peer, ids=[message_id])
        if not messages or messages[0] is None:
            raise LookupError(f"message {message_id} not found")
        return messages[0]

    async def upload_bytes(
        self,
        folder_id: Optional[int],
        data: bytes,
        filename: str,
        progress_callback: Any = None,
        caption: str = "",
    ) -> int:
        """Upload raw bytes as a document. Returns message id.

        Caption defaults to empty (the file-metadata format); legacy chunk
        uploads pass ``blob [{idx}/{total}] - {filename}`` instead.
        """
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        import io

        message = await client.send_file(
            peer,
            io.BytesIO(data),
            caption=caption,
            attributes=[],
            file_name=filename,
            force_document=True,
            progress_callback=progress_callback,
        )
        return message.id

    async def upload_stream(
        self,
        folder_id: Optional[int],
        file_obj: Any,
        file_size: int,
        filename: str,
        progress_callback: Any = None,
    ) -> int:
        """Stream-upload a file object. Returns the resulting message id."""
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        uploaded = await client.upload_file(
            file_obj, file_size=file_size, file_name=filename
        )
        message = await client.send_file(
            peer, uploaded, caption="", file_name=filename, force_document=True
        )
        return message.id

    async def delete_files(self, folder_id: Optional[int], message_ids: list[int]) -> None:
        client = await self.connect()
        peer = await self.resolve_peer(folder_id)
        await client.delete_messages(peer, message_ids)

    async def move_files(
        self,
        source_folder_id: Optional[int],
        target_folder_id: Optional[int],
        message_ids: list[int],
    ) -> list[int]:
        """Forward messages to target, then delete originals. Returns new ids."""
        client = await self.connect()
        source = await self.resolve_peer(source_folder_id)
        target = await self.resolve_peer(target_folder_id)
        forwarded = await client.forward_messages(target, message_ids, from_peer=source)
        new_ids = [m.id for m in forwarded]
        await client.delete_messages(source, message_ids)
        return new_ids

    async def iter_download(self, message: Any, part_size_kb: int = 512):
        """Yield download chunks for a message's media."""
        client = await self.connect()
        media = getattr(message, "media", None)
        if media is None:
            raise LookupError("message has no media")
        async for chunk in client.iter_download(media, part_size_kb=part_size_kb):
            yield chunk

    async def iter_download_by_id(
        self,
        folder_id: Optional[int],
        message_id: int,
        start_byte: int = 0,
        part_size_kb: int = 512,
    ):
        """Yield download chunks for a message located by folder + id.

        ``start_byte`` is implemented by skipping leading bytes of a single
        pass (Telethon has no byte-offset seek on iter_download).
        """
        client = await self.connect()
        message = await self.get_message(folder_id, message_id)
        media = getattr(message, "media", None)
        if media is None:
            raise LookupError("message has no media")
        skipped = 0
        async for chunk in client.iter_download(media, part_size_kb=part_size_kb):
            if skipped < start_byte:
                skipped += len(chunk)
                if skipped <= start_byte:
                    continue
                # This chunk straddles the start offset — yield its tail.
                yield chunk[len(chunk) - (skipped - start_byte) :]
                continue
            yield chunk

    async def search_global(self, query: str, limit: int = 50) -> list[FileMetadata]:
        client = await self.connect()
        from telethon.tl.functions.messages import SearchGlobalRequest

        results: list[FileMetadata] = []
        result = await client(
            SearchGlobalRequest(
                q=query,
                filter=InputMessagesFilterDocument(),
                min_date=0,
                max_date=0,
                offset_rate=0,
                offset_peer=None,
                offset_id=0,
                limit=limit,
            )
        )
        for message in getattr(result, "messages", []):
            meta = self.message_to_metadata(message, None)
            if meta is not None:
                results.append(meta)
        return results


def _icon_type(ext: str, mime: str) -> str:
    if mime.startswith("image/") or ext in {"png", "jpg", "jpeg", "gif", "webp", "svg"}:
        return "image"
    if mime.startswith("video/") or ext in {"mp4", "mkv", "webm", "mov", "avi"}:
        return "video"
    if mime.startswith("audio/") or ext in {"mp3", "flac", "wav", "ogg", "m4a"}:
        return "audio"
    if ext == "pdf":
        return "pdf"
    if ext in {"zip", "rar", "7z", "tar", "gz"}:
        return "archive"
    return "file"
