-- Durable upload Saga state for request idempotency, fenced workers and compensation.
BEGIN;

ALTER TABLE transfer_jobs
  ADD COLUMN IF NOT EXISTS request_fingerprint TEXT,
  ADD COLUMN IF NOT EXISTS source_file_name TEXT,
  ADD COLUMN IF NOT EXISTS requested_folder_id BIGINT,
  ADD COLUMN IF NOT EXISTS attempt_token UUID,
  ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS telegram_message_id BIGINT,
  ADD COLUMN IF NOT EXISTS storage_peer_id BIGINT,
  ADD COLUMN IF NOT EXISTS storage_peer_kind TEXT,
  ADD COLUMN IF NOT EXISTS telegram_file_id TEXT,
  ADD COLUMN IF NOT EXISTS telegram_file_name TEXT,
  ADD COLUMN IF NOT EXISTS telegram_file_size BIGINT,
  ADD COLUMN IF NOT EXISTS telegram_mime_type TEXT,
  ADD COLUMN IF NOT EXISTS compensation_status TEXT NOT NULL DEFAULT 'none',
  ADD COLUMN IF NOT EXISTS compensation_error TEXT,
  ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ;

ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_status_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_status_check CHECK (
  status IN (
    'queued', 'running', 'retry_wait', 'telegram_succeeded', 'succeeded',
    'failed', 'cancelled', 'compensation_pending', 'compensated'
  )
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_request_fingerprint_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_request_fingerprint_check CHECK (
  request_fingerprint IS NULL OR request_fingerprint ~ '^[0-9a-f]{64}$'
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_storage_peer_kind_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_storage_peer_kind_check CHECK (
  storage_peer_kind IS NULL OR storage_peer_kind IN ('user', 'group', 'supergroup', 'channel', 'private')
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_compensation_status_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_compensation_status_check CHECK (
  compensation_status IN ('none', 'pending', 'deleted', 'reconcile')
);

CREATE INDEX IF NOT EXISTS idx_jobs_recovery
  ON transfer_jobs(status, lease_expires_at, updated_at)
  WHERE status IN ('running', 'telegram_succeeded', 'compensation_pending');
CREATE INDEX IF NOT EXISTS idx_jobs_receipt
  ON transfer_jobs(storage_peer_id, telegram_message_id)
  WHERE telegram_message_id IS NOT NULL;

INSERT INTO schema_migrations(version)
VALUES ('005_upload_saga_state')
ON CONFLICT DO NOTHING;
COMMIT;
