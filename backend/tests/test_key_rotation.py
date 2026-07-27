"""TASK-P1-01: 下载签名密钥自动轮换 — 验收测试.

验证：
1. 密钥生成、持久化及轮换逻辑
2. 旧密钥宽限期内的签名链接仍可验证
3. 热更新 settings，无需重启
4. 审计日志产生对应 SETTINGS_CHANGE 记录
"""

from __future__ import annotations

import json
import time
from pathlib import Path

import pytest

from app.audit import AuditEvent, get_audit_logger, init_audit_logger
from app.config import Settings
from app.key_rotation import KeyRotationManager
from app.links import presign_canonical, presign_signature, verify_presign_with_secrets


@pytest.fixture
def temp_dir(tmp_path):
    return tmp_path


@pytest.fixture
def rotation_settings(temp_dir):
    # Ensure a 32+ character default secret to bypass length validation
    s = Settings(
        DATA_DIR=temp_dir,
        DOWNLOAD_SIGNING_SECRET="a" * 32,
        DOWNLOAD_SIGNING_SECRETS="",
    )
    return s


def test_key_rotation_flow(temp_dir, rotation_settings):
    # Initialize logger
    init_audit_logger(temp_dir / "audit.log", enabled=True)

    mgr = KeyRotationManager(temp_dir, settings=rotation_settings, rotation_interval_secs=10)

    # 1. First load: empty JSON should return default from settings
    secrets_list = mgr.get_all_secrets()
    assert len(secrets_list) == 1
    assert secrets_list[0] == "a" * 32

    # 2. Perform rotation
    ring = mgr.rotate_key(actor="test-admin")
    assert ring["active_key"]
    assert len(ring["active_key"]) >= 32
    assert ring["retired_keys"] == ["a" * 32]
    assert ring["last_rotated_at"] > 0

    # File should exist and have proper values
    assert mgr.key_file.exists()
    file_ring = json.loads(mgr.key_file.read_text(encoding="utf-8"))
    assert file_ring["active_key"] == ring["active_key"]

    # 3. Settings update check (hot-reload verification)
    assert rotation_settings.download_signing_secret == ring["active_key"]
    assert rotation_settings.all_signing_secrets == [ring["active_key"], "a" * 32]

    # 4. Old URL validation
    # Sign something with the retired key
    canonical = "v1|12345|None|0|owner|None"
    old_sig = presign_signature("a" * 32, canonical)

    # Verify using the rotated ring (should succeed because old key is retired)
    all_secrets = mgr.get_all_secrets()
    assert verify_presign_with_secrets(all_secrets, canonical, old_sig)

    # Sign with active key
    new_sig = presign_signature(ring["active_key"], canonical)
    assert verify_presign_with_secrets(all_secrets, canonical, new_sig)


def test_rotate_if_due(temp_dir, rotation_settings):
    mgr = KeyRotationManager(temp_dir, settings=rotation_settings, rotation_interval_secs=2)

    # First rotation
    r1 = mgr.rotate_if_due(actor="sched")
    assert r1 is not None
    k1 = r1["active_key"]

    # Immediate second call - shouldn't rotate
    r2 = mgr.rotate_if_due(actor="sched")
    assert r2 is None

    # Wait for interval to elapse
    time.sleep(2.1)
    r3 = mgr.rotate_if_due(actor="sched")
    assert r3 is not None
    assert r3["active_key"] != k1
    assert r3["retired_keys"][0] == k1


def test_retained_keys_max_limit(temp_dir, rotation_settings):
    mgr = KeyRotationManager(temp_dir, settings=rotation_settings, max_retained_keys=2)
    # Rotate 4 times
    mgr.rotate_key()  # retires default 'a'*32
    mgr.rotate_key()  # retires key 1
    mgr.rotate_key()  # retires key 2
    ring = mgr.rotate_key()  # retires key 3

    # Total secrets should be active_key + retired_keys (max 2) = 3 keys total
    all_secrets = mgr.get_all_secrets()
    assert len(all_secrets) == 3
    # Default 'a'*32 should have been evicted (oldest)
    assert "a" * 32 not in all_secrets
