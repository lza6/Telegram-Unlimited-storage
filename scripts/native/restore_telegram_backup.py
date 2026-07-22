#!/usr/bin/env python3
"""Verify and decrypt a Telegram Drive .tdbak archive without touching live databases."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import shutil
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(__file__).resolve().parents[2]
AAD = b"telegram-drive-backup-v1"
PART_PATTERN = re.compile(r"^(?P<base>.+\.tdbak)\.part(?P<index>\d+)$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


def read_key() -> bytes:
    value = ""
    for raw in (ROOT / ".env").read_text(encoding="utf-8").splitlines():
        if raw.startswith("BACKUP_ENCRYPTION_KEY="):
            value = raw.split("=", 1)[1].strip()
            break
    key = base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))
    if len(key) != 32:
        raise RuntimeError("BACKUP_ENCRYPTION_KEY must decode to 32 bytes")
    return key


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def resolve_input(source: Path) -> list[Path]:
    if source.is_file():
        if PART_PATTERN.match(source.name):
            raise RuntimeError(
                "pass the directory containing all .partNNNNNN files, not one part"
            )
        if source.suffix != ".tdbak":
            raise RuntimeError("input file must have the .tdbak extension")
        return [source]
    if not source.is_dir():
        raise RuntimeError(
            "input must be an existing .tdbak file or a directory of parts"
        )
    singles = [
        path for path in source.iterdir() if path.is_file() and path.suffix == ".tdbak"
    ]
    candidates = [
        path
        for path in source.iterdir()
        if path.is_file() and PART_PATTERN.match(path.name)
    ]
    if singles and candidates:
        raise RuntimeError(
            "input directory cannot mix a full archive with multipart backup files"
        )
    if len(singles) == 1:
        return singles
    if len(singles) > 1:
        raise RuntimeError("input directory must contain exactly one .tdbak archive")
    if not candidates:
        raise RuntimeError("input directory has no .tdbak or .tdbak.partNNNNNN files")
    grouped: dict[str, list[tuple[int, Path]]] = {}
    for path in candidates:
        match = PART_PATTERN.match(path.name)
        assert match is not None
        grouped.setdefault(match.group("base"), []).append(
            (int(match.group("index")), path)
        )
    if len(grouped) != 1:
        raise RuntimeError("input directory must contain parts for exactly one backup")
    items = next(iter(grouped.values()))
    items.sort(key=lambda item: item[0])
    expected = list(range(1, len(items) + 1))
    if [index for index, _ in items] != expected:
        raise RuntimeError("backup parts are missing, duplicated, or non-contiguous")
    return [path for _, path in items]


def validate_manifest(manifest: Any, names: set[str]) -> list[dict[str, Any]]:
    if (
        not isinstance(manifest, dict)
        or manifest.get("format") != "telegram-drive-backup-v1"
    ):
        raise RuntimeError("invalid backup manifest format")
    if not isinstance(manifest.get("backup_id"), str) or not manifest["backup_id"]:
        raise RuntimeError("manifest backup_id is required")
    if not isinstance(manifest.get("created_at"), str) or not manifest["created_at"]:
        raise RuntimeError("manifest created_at is required")
    contents = manifest.get("contents")
    if not isinstance(contents, list) or not contents:
        raise RuntimeError("manifest contents must be a non-empty list")
    expected_names = {"manifest.json"}
    seen: set[str] = set()
    for item in contents:
        if not isinstance(item, dict):
            raise RuntimeError("manifest content entry is invalid")
        name, size, digest = item.get("file"), item.get("bytes"), item.get("sha256")
        if not isinstance(name, str) or Path(name).name != name or name in seen:
            raise RuntimeError("manifest file names must be unique base names")
        if (
            not isinstance(size, int)
            or size < 0
            or not isinstance(digest, str)
            or not SHA256_PATTERN.fullmatch(digest)
        ):
            raise RuntimeError(f"invalid manifest metadata for {name}")
        seen.add(name)
        expected_names.add(name)
    if names != expected_names:
        raise RuntimeError("ZIP members do not exactly match the manifest")
    return contents


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--input",
        required=True,
        type=Path,
        help="a .tdbak file or directory containing all .partNNNNNN files",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="non-existing directory for verified snapshots",
    )
    args = parser.parse_args()
    if args.output.exists():
        raise RuntimeError("output directory must not already exist")
    parts = resolve_input(args.input)
    encrypted = b"".join(part.read_bytes() for part in parts)
    if not encrypted.startswith(b"TDBK1") or len(encrypted) < 33:
        raise RuntimeError("invalid encrypted backup header")
    plaintext = AESGCM(read_key()).decrypt(encrypted[5:17], encrypted[17:], AAD)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary_dir = Path(
        tempfile.mkdtemp(prefix=".td-restore-", dir=args.output.parent)
    )
    archive_path = temporary_dir / "archive.zip"
    try:
        archive_path.write_bytes(plaintext)
        with zipfile.ZipFile(archive_path) as archive:
            if archive.testzip() is not None:
                raise RuntimeError("ZIP CRC verification failed")
            names = set(archive.namelist())
            if any(Path(name).name != name for name in names):
                raise RuntimeError("ZIP contains an unsafe member name")
            manifest = json.loads(archive.read("manifest.json"))
            contents = validate_manifest(manifest, names)
            for item in contents:
                destination = temporary_dir / item["file"]
                with archive.open(item["file"]) as source, destination.open(
                    "wb"
                ) as target:
                    shutil.copyfileobj(source, target)
                if (
                    destination.stat().st_size != item["bytes"]
                    or sha256(destination) != item["sha256"]
                ):
                    raise RuntimeError(
                        f"integrity verification failed for {item['file']}"
                    )
        archive_path.unlink(missing_ok=True)
        temporary_dir.replace(args.output)
        print("BACKUP_RESTORE_VERIFY=True")
        print("BACKUP_ID=" + manifest["backup_id"])
        return 0
    except Exception:
        shutil.rmtree(temporary_dir, ignore_errors=True)
        raise


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"BACKUP_RESTORE_FAILED={error}", file=sys.stderr)
        raise SystemExit(1)
