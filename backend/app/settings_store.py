"""JSON settings files: ui_settings.json, network_settings.json, transport_mode.json."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Optional

DEFAULT_PROXY: dict[str, Any] = {
    "enabled": False,
    "proxy_type": "socks5",
    "host": "",
    "port": 1080,
    "username": "",
    "password": "",
}

DEFAULT_VPN: dict[str, Any] = {
    "enabled": True,
    "timeout_multiplier": 2.0,
    "retry_attempts": 3,
    "retry_base_backoff_ms": 500,
    "retry_max_backoff_ms": 8000,
    "adaptive_polling": True,
    "polling_min_sec": 1,
    "polling_max_sec": 15,
    "preferred_dc": "auto",
    "dc_fallback_attempts": 2,
    "flood_wait_respect": True,
    "peer_cache_size": 500,
    "bandwidth_limit_up_kbs": 0,
    "bandwidth_limit_down_kbs": 0,
    "chunk_size_kb": 512,
    "keep_alive_interval_sec": 60,
    "auto_detect_vpn": True,
}


class JsonFileStore:
    """Atomic read/merge/write for a single JSON settings file."""

    def __init__(self, path: Path, defaults: Optional[dict[str, Any]] = None) -> None:
        self.path = path
        self.defaults = defaults or {}

    def load(self) -> dict[str, Any]:
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
            if isinstance(data, dict):
                merged = dict(self.defaults)
                merged.update(data)
                return merged
        except (OSError, ValueError):
            pass
        return dict(self.defaults)

    def save(self, data: dict[str, Any]) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
        tmp.replace(self.path)

    def merge(self, patch: dict[str, Any]) -> dict[str, Any]:
        current = self.load()
        for key, value in patch.items():
            if isinstance(value, dict) and isinstance(current.get(key), dict):
                current[key] = {**current[key], **value}
            else:
                current[key] = value
        self.save(current)
        return current


class SettingsStore:
    def __init__(self, data_dir: Path) -> None:
        self.data_dir = data_dir
        self.ui = JsonFileStore(data_dir / "ui_settings.json", {"share_domain": ""})
        self.network = JsonFileStore(
            data_dir / "network_settings.json",
            {"proxy": dict(DEFAULT_PROXY), "vpn": dict(DEFAULT_VPN)},
        )
        self.transport = JsonFileStore(data_dir / "transport_mode.json", {"mode": None})

    # ── share link base resolution ──────────────────────────────────────────
    def share_base_url(self, env_base_url: str, request_host: Optional[str]) -> str:
        """ui_settings.share_domain > BASE_URL env > request Host."""
        share_domain = (self.ui.load().get("share_domain") or "").strip()
        if share_domain:
            return share_domain.rstrip("/")
        if env_base_url:
            return env_base_url.rstrip("/")
        if request_host:
            return f"http://{request_host}"
        return "http://localhost:1334"

    # ── network view (password redacted) ────────────────────────────────────
    def network_view(self) -> dict[str, Any]:
        data = self.network.load()
        proxy = dict(data.get("proxy") or DEFAULT_PROXY)
        proxy["password_set"] = bool(proxy.get("password"))
        proxy["password"] = ""  # never expose
        return {"proxy": proxy, "vpn": data.get("vpn") or DEFAULT_VPN}
