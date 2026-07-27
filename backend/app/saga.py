"""Saga upload manager (TASK-P0-03).

Provides transactional coordination for Telegram message sending + Database asset creation.
Rolls back (compensates) by deleting orphan Telegram messages if database updates fail,
ensuring strict atomicity across distributed systems.
"""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional

from .audit import AuditEvent, get_audit_logger

if TYPE_CHECKING:
    from .storage import Storage
    from .telegram_state import TelegramState

logger = logging.getLogger("telegram_drive.saga")

SAGA_EXPIRATION_SECS = 3600  # 1 hour


@dataclass
class SagaStep:
    saga_id: str
    state: str  # started | tg_sent | db_written | completed | compensating | compensated
    message_id: Optional[int]
    peer_id: Optional[int]
    file_name: str
    file_size: int
    owner_id: str
    idempotency_key: str
    created_at: int
    updated_at: int


class SagaManager:
    """Manages transactional state machine for upload Saga distributed commits."""

    def __init__(self, storage: Storage, telegram: TelegramState) -> None:
        self.storage = storage
        self.telegram = telegram

    def _now(self) -> int:
        return int(time.time())

    def start_saga(
        self,
        saga_id: str,
        file_name: str,
        file_size: int,
        owner_id: str,
        idempotency_key: str,
    ) -> SagaStep:
        """Create new Saga entry in saga_uploads."""
        now = self._now()
        # Query idempotency first
        existing = self.storage._query(
            "SELECT * FROM saga_uploads WHERE idempotency_key = ?",
            (idempotency_key,),
        )
        if existing:
            r = existing[0]
            return SagaStep(
                saga_id=r["saga_id"],
                state=r["state"],
                message_id=r["message_id"],
                peer_id=r["peer_id"],
                file_name=r["file_name"],
                file_size=r["file_size"],
                owner_id=r["owner_id"],
                idempotency_key=r["idempotency_key"],
                created_at=r["created_at"],
                updated_at=r["updated_at"],
            )

        self.storage._write(
            "INSERT INTO saga_uploads (saga_id, state, message_id, peer_id, file_name, file_size, owner_id, idempotency_key, created_at, updated_at) "
            "VALUES (?, 'started', NULL, NULL, ?, ?, ?, ?, ?, ?)",
            (saga_id, file_name, file_size, owner_id, idempotency_key, now, now),
        )

        return SagaStep(
            saga_id=saga_id,
            state="started",
            message_id=None,
            peer_id=None,
            file_name=file_name,
            file_size=file_size,
            owner_id=owner_id,
            idempotency_key=idempotency_key,
            created_at=now,
            updated_at=now,
        )

    def update_tg_sent(self, saga_id: str, message_id: int, peer_id: int) -> None:
        """Move Saga to TG_SENT state, capturing message location."""
        self.storage._write(
            "UPDATE saga_uploads SET state = 'tg_sent', message_id = ?, peer_id = ?, updated_at = ? WHERE saga_id = ?",
            (message_id, peer_id, self._now(), saga_id),
        )

    def update_db_written(self, saga_id: str) -> None:
        """Move Saga to DB_WRITTEN state."""
        self.storage._write(
            "UPDATE saga_uploads SET state = 'db_written', updated_at = ? WHERE saga_id = ?",
            (self._now(), saga_id),
        )

    def complete_saga(self, saga_id: str) -> None:
        """Move Saga to COMPLETED state (or delete it to reclaim disk, logging audit)."""
        self.storage._write(
            "UPDATE saga_uploads SET state = 'completed', updated_at = ? WHERE saga_id = ?",
            (self._now(), saga_id),
        )
        # Optionally prune completed sagas immediately or keep for 7 days
        self.storage._write("DELETE FROM saga_uploads WHERE saga_id = ?", (saga_id,))

    async def compensate_saga(self, saga_id: str, step: SagaStep) -> bool:
        """Execute compensation transaction: delete Telegram message, mark compensated."""
        # Fetch the latest state from storage to ensure we have the real message_id
        row = self.storage._query_one("SELECT * FROM saga_uploads WHERE saga_id = ?", (saga_id,))
        if not row:
            return True
        current_step = SagaStep(
            saga_id=row["saga_id"],
            state=row["state"],
            message_id=row["message_id"],
            peer_id=row["peer_id"],
            file_name=row["file_name"],
            file_size=row["file_size"],
            owner_id=row["owner_id"],
            idempotency_key=row["idempotency_key"],
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )

        if not current_step.message_id or not current_step.peer_id:
            self.storage._write(
                "UPDATE saga_uploads SET state = 'compensated', updated_at = ? WHERE saga_id = ?",
                (self._now(), saga_id),
            )
            return True

        self.storage._write(
            "UPDATE saga_uploads SET state = 'compensating', updated_at = ? WHERE saga_id = ?",
            (self._now(), saga_id),
        )

        success = False
        try:
            # Under user-mode, self.telegram has the client; let's call delete
            client = getattr(self.telegram, 'client', None)
            if client:
                # Resolve the entity peer first
                entity = await client.get_input_entity(current_step.peer_id)
                await client.delete_messages(entity, [current_step.message_id])
                success = True
                logger.info("Saga compensation: deleted orphan message %d from peer %d", current_step.message_id, current_step.peer_id)
        except Exception as exc:
            logger.warning("Saga compensation failed for saga %s: %s", saga_id, exc)

        audit = get_audit_logger()
        if audit:
            audit.log(
                event=AuditEvent.FILE_DELETE,
                actor="system:saga_compensator",
                target=str(current_step.message_id),
                success=success,
                action="compensate_saga",
                saga_id=saga_id,
            )

        self.storage._write(
            "UPDATE saga_uploads SET state = 'compensated', updated_at = ? WHERE saga_id = ?",
            (self._now(), saga_id),
        )
        return success

    async def recover_orphans(self) -> int:
        """Lifespan recovery task scans for hung or interrupted sagas.

        Any saga stuck in started / tg_sent state for > 5 minutes gets rolled back
        by triggering the compensating delete transaction.
        """
        now = self._now()
        threshold = now - 300  # 5 minutes
        rows = self.storage._query(
            "SELECT * FROM saga_uploads WHERE state IN ('started', 'tg_sent', 'compensating') AND updated_at < ?",
            (threshold,),
        )
        if not rows:
            return 0

        recovered_count = 0
        for r in rows:
            step = SagaStep(
                saga_id=r["saga_id"],
                state=r["state"],
                message_id=r["message_id"],
                peer_id=r["peer_id"],
                file_name=r["file_name"],
                file_size=r["file_size"],
                owner_id=r["owner_id"],
                idempotency_key=r["idempotency_key"],
                created_at=r["created_at"],
                updated_at=r["updated_at"],
            )
            # Try compensating delete
            await self.compensate_saga(step.saga_id, step)
            recovered_count += 1

        return recovered_count
