#!/usr/bin/env python3
"""Create an encrypted, auditable Telegram Drive database backup."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request
import uuid
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(__file__).resolve().parents[2]
STAGING = ROOT / "data" / "backup-staging"
RETAINED = ROOT / "data" / "backup-retained"
STATE_DIR = ROOT / "data" / "backup-state"
MAX_PUBLIC_BOT_PART_BYTES = 45 * 1024 * 1024
AAD = b"telegram-drive-backup-v1"


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(
        self, req: Any, fp: Any, code: int, msg: str, headers: Any, newurl: str
    ) -> None:
        return None


HTTP = urllib.request.build_opener(NoRedirect)


def read_env(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            values[key.strip()] = value.strip()
    return values


def require(env: dict[str, str], key: str) -> str:
    value = env.get(key, "")
    if not value:
        raise RuntimeError(f"{key} is missing from .env")
    return value


def aes_key(raw: str) -> bytes:
    key = base64.urlsafe_b64decode(raw + "=" * (-len(raw) % 4))
    if len(key) != 32:
        raise RuntimeError("BACKUP_ENCRYPTION_KEY must decode to exactly 32 bytes")
    return key


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def atomic_json(path: Path, body: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(body, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    temporary.replace(path)


def snapshot_sqlite(source: Path, target: Path) -> None:
    source_connection = sqlite3.connect(source)
    target_connection = sqlite3.connect(target)
    try:
        source_connection.backup(target_connection)
    finally:
        target_connection.close()
        source_connection.close()


def dump_postgres(env: dict[str, str], target: Path) -> bool:
    if not env.get("POSTGRES_DB"):
        return False
    pg_dump = Path(
        env.get("PG_DUMP_PATH", r"C:\Program Files\PostgreSQL\16\bin\pg_dump.exe")
    )
    if not pg_dump.is_file():
        raise RuntimeError(f"pg_dump not found: {pg_dump}")
    command = [
        str(pg_dump),
        "--format=custom",
        "--compress=9",
        "--no-owner",
        "--no-privileges",
        "--host",
        env.get("POSTGRES_HOST", "127.0.0.1"),
        "--port",
        env.get("POSTGRES_PORT", "15432"),
        "--username",
        require(env, "POSTGRES_USER"),
        "--file",
        str(target),
        require(env, "POSTGRES_DB"),
    ]
    process_env = os.environ.copy()
    process_env["PGPASSWORD"] = require(env, "POSTGRES_PASSWORD")
    completed = subprocess.run(
        command, env=process_env, capture_output=True, text=True, check=False
    )
    if completed.returncode:
        raise RuntimeError(
            f"pg_dump failed: {completed.stderr.strip() or completed.stdout.strip()}"
        )
    return True


def encrypt(source: Path, target: Path, key: bytes) -> None:
    nonce = os.urandom(12)
    target.write_bytes(
        b"TDBK1" + nonce + AESGCM(key).encrypt(nonce, source.read_bytes(), AAD)
    )


def split_file(source: Path, max_bytes: int) -> list[Path]:
    if source.stat().st_size <= max_bytes:
        return [source]
    parts: list[Path] = []
    with source.open("rb") as handle:
        index = 1
        while block := handle.read(max_bytes):
            part = source.with_name(f"{source.name}.part{index:06d}")
            part.write_bytes(block)
            parts.append(part)
            index += 1
    return parts


def api_base(env: dict[str, str]) -> str:
    raw = env.get("CUSTOM_BOT_API_URL", "https://api.telegram.org").rstrip("/")
    parsed = urllib.parse.urlsplit(raw)
    if (
        parsed.username
        or parsed.password
        or parsed.query
        or parsed.fragment
        or parsed.path not in ("", "/")
    ):
        raise RuntimeError(
            "CUSTOM_BOT_API_URL must be an origin without credentials, path, query, or fragment"
        )
    host = (parsed.hostname or "").lower()
    loopback = host in {"localhost", "127.0.0.1", "::1"}
    if parsed.scheme != "https" and not (parsed.scheme == "http" and loopback):
        raise RuntimeError(
            "CUSTOM_BOT_API_URL must use HTTPS; HTTP is allowed only for loopback Local Bot API"
        )
    if not host:
        raise RuntimeError("CUSTOM_BOT_API_URL host is required")
    return f"{parsed.scheme}://{parsed.netloc}"


def telegram_json(request: urllib.request.Request) -> dict[str, Any]:
    try:
        with HTTP.open(request, timeout=180) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"Telegram API HTTP {error.code}") from error
    if not payload.get("ok"):
        raise RuntimeError(
            f"Telegram API rejected request: {payload.get('description', 'unknown error')}"
        )
    return payload


def send_document(url: str, chat_id: str, caption: str, file_path: Path) -> int:
    boundary = "----telegramdrive" + uuid.uuid4().hex
    body = bytearray()
    for key, value in {"chat_id": chat_id, "caption": caption}.items():
        body.extend(
            f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode()
        )
    body.extend(
        (
            f'--{boundary}\r\nContent-Disposition: form-data; name="document"; filename="{file_path.name}"\r\nContent-Type: application/octet-stream\r\n\r\n'
        ).encode()
    )
    body.extend(file_path.read_bytes())
    body.extend(f"\r\n--{boundary}--\r\n".encode())
    request = urllib.request.Request(
        url,
        data=bytes(body),
        method="POST",
        headers={
            "Content-Type": f"multipart/form-data; boundary={boundary}",
            "Content-Length": str(len(body)),
        },
    )
    result = telegram_json(request).get("result", {})
    message_id = result.get("message_id")
    if not isinstance(message_id, int):
        raise RuntimeError("Telegram sendDocument response did not contain message_id")
    return message_id


def post_form(url: str, fields: dict[str, str]) -> dict[str, Any]:
    request = urllib.request.Request(
        url, data=urllib.parse.urlencode(fields).encode("utf-8"), method="POST"
    )
    return telegram_json(request)


def cleanup_directory(path: Path) -> list[str]:
    errors: list[str] = []
    if not path.exists():
        return errors
    for item in sorted(
        path.rglob("*"), key=lambda candidate: len(candidate.parts), reverse=True
    ):
        try:
            if item.is_dir():
                item.rmdir()
            else:
                item.unlink()
        except OSError as error:
            errors.append(f"{item}: {error}")
    try:
        path.rmdir()
    except OSError as error:
        errors.append(f"{path}: {error}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--keep-local",
        action="store_true",
        help="retain only encrypted .tdbak material after success",
    )
    args = parser.parse_args()
    env = read_env(ROOT / ".env")
    key = aes_key(require(env, "BACKUP_ENCRYPTION_KEY"))
    for directory in (STAGING, RETAINED, STATE_DIR):
        directory.mkdir(parents=True, exist_ok=True)
    backup_id = (
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        + "-"
        + uuid.uuid4().hex[:8]
    )
    work_dir = Path(tempfile.mkdtemp(prefix=f"td-backup-{backup_id}-", dir=STAGING))
    archive = work_dir / f"telegram-drive-{backup_id}.zip"
    encrypted = work_dir / f"telegram-drive-{backup_id}.tdbak"
    retained_dir = RETAINED / backup_id
    state_path = STATE_DIR / f"{backup_id}.json"
    state: dict[str, Any] = {
        "format": "telegram-drive-backup-upload-v1",
        "backup_id": backup_id,
        "status": "creating",
        "message_ids": [],
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
    atomic_json(state_path, state)
    primary_error: BaseException | None = None
    try:
        snapshots: list[Path] = []
        postgres_dump = work_dir / "control-plane.pgcustom"
        if dump_postgres(env, postgres_dump):
            snapshots.append(postgres_dump)
        for relative in ("app/src-tauri/data/shares.db", "data/shares.db"):
            source = ROOT / relative
            if source.is_file():
                target = work_dir / (
                    "sqlite-"
                    + relative.replace("/", "-").replace("\\", "-")
                    + ".snapshot"
                )
                snapshot_sqlite(source, target)
                snapshots.append(target)
        if not snapshots:
            raise RuntimeError("no PostgreSQL or SQLite database was found to back up")
        contents = [
            {
                "file": snapshot.name,
                "bytes": snapshot.stat().st_size,
                "sha256": sha256(snapshot),
            }
            for snapshot in snapshots
        ]
        manifest = {
            "format": "telegram-drive-backup-v1",
            "backup_id": backup_id,
            "created_at": state["created_at"],
            "contents": contents,
        }
        with zipfile.ZipFile(
            archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as bundle:
            bundle.writestr(
                "manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2)
            )
            for snapshot in snapshots:
                bundle.write(snapshot, arcname=snapshot.name)
        encrypt(archive, encrypted, key)
        part_limit = int(
            env.get("BACKUP_TG_PART_BYTES", str(MAX_PUBLIC_BOT_PART_BYTES))
        )
        if not 0 < part_limit <= MAX_PUBLIC_BOT_PART_BYTES:
            raise RuntimeError(
                f"BACKUP_TG_PART_BYTES must be between 1 and {MAX_PUBLIC_BOT_PART_BYTES}"
            )
        parts = split_file(encrypted, part_limit)
        retained_dir.mkdir(parents=True, exist_ok=False)
        retained_parts = []
        for part in parts:
            copied = retained_dir / part.name
            shutil.copy2(part, copied)
            retained_parts.append(copied)
        state.update(
            {
                "status": "encrypted",
                "encrypted_sha256": sha256(encrypted),
                "parts": [
                    {
                        "file": part.name,
                        "bytes": part.stat().st_size,
                        "sha256": sha256(part),
                    }
                    for part in retained_parts
                ],
            }
        )
        atomic_json(state_path, state)
        print(f"BACKUP_ID={backup_id}")
        print(f"BACKUP_ENCRYPTED_BYTES={encrypted.stat().st_size}")
        print(f"BACKUP_PARTS={len(parts)}")
        if args.dry_run:
            state["status"] = "dry_run"
            atomic_json(state_path, state)
            if not args.keep_local:
                cleanup_errors = cleanup_directory(retained_dir)
                if cleanup_errors:
                    state["local_cleanup_errors"] = cleanup_errors
                    atomic_json(state_path, state)
                    raise RuntimeError(
                        "encrypted dry-run material cleanup failed; inspect backup state record"
                    )
            print("BACKUP_DRY_RUN=True")
            return 0
        token = require(env, "TG_BOT_TOKEN")
        chat_id = require(env, "TG_STORAGE_CHANNEL_ID")
        base = api_base(env)
        for index, part in enumerate(retained_parts, 1):
            message_id = send_document(
                f"{base}/bot{token}/sendDocument",
                chat_id,
                f"Telegram Drive encrypted DB backup {backup_id} part {index}/{len(retained_parts)} sha256={sha256(part)}",
                part,
            )
            state["message_ids"].append(message_id)
            state["status"] = "uploading"
            atomic_json(state_path, state)
        completion = post_form(
            f"{base}/bot{token}/sendMessage",
            {
                "chat_id": chat_id,
                "text": f'DB_BACKUP_COMPLETE id={backup_id} parts={len(retained_parts)} encrypted_sha256={state["encrypted_sha256"]}',
            },
        )
        state.update(
            {
                "status": "completed",
                "completion_message_id": completion.get("result", {}).get("message_id"),
                "completed_at": datetime.now(timezone.utc).isoformat(),
            }
        )
        atomic_json(state_path, state)
        if not args.keep_local:
            cleanup_errors = cleanup_directory(retained_dir)
            if cleanup_errors:
                state["local_cleanup_errors"] = cleanup_errors
                atomic_json(state_path, state)
                raise RuntimeError(
                    "encrypted local backup cleanup failed; inspect backup state record"
                )
        print("BACKUP_TELEGRAM_UPLOAD=True")
        return 0
    except BaseException as error:
        primary_error = error
        state["status"] = "failed"
        state["error"] = str(error)
        try:
            token = env.get("TG_BOT_TOKEN", "")
            chat_id = env.get("TG_STORAGE_CHANNEL_ID", "")
            if token and chat_id:
                base = api_base(env)
                rollback = []
                rollback_ids = list(state.get("message_ids", []))
                completion_message_id = state.get("completion_message_id")
                if isinstance(completion_message_id, int):
                    rollback_ids.append(completion_message_id)
                for message_id in rollback_ids:
                    try:
                        post_form(
                            f"{base}/bot{token}/deleteMessage",
                            {"chat_id": chat_id, "message_id": str(message_id)},
                        )
                    except Exception as rollback_error:
                        rollback.append(
                            {"message_id": message_id, "error": str(rollback_error)}
                        )
                if rollback:
                    state["rollback_failures"] = rollback
        except Exception as rollback_setup_error:
            state["rollback_setup_error"] = str(rollback_setup_error)
        try:
            atomic_json(state_path, state)
        except OSError as state_error:
            print(f"BACKUP_STATE_WRITE_WARNING={state_error}", file=sys.stderr)
        raise
    finally:
        cleanup_errors = cleanup_directory(work_dir)
        if cleanup_errors:
            state["plaintext_cleanup_errors"] = cleanup_errors
            try:
                atomic_json(state_path, state)
            except OSError as state_error:
                print(f"BACKUP_STATE_WRITE_WARNING={state_error}", file=sys.stderr)
            if primary_error is None:
                raise RuntimeError(
                    "plaintext backup cleanup failed; inspect backup state and staging directory"
                )
            print(
                "BACKUP_CLEANUP_WARNING=plaintext cleanup failed; inspect backup state and staging directory",
                file=sys.stderr,
            )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"BACKUP_FAILED={error}", file=sys.stderr)
        raise SystemExit(1)
