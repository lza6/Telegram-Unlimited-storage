-- Harden the already-applied 005 upload Saga prototype without rewriting history.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

ALTER TABLE transfer_jobs
  ADD COLUMN IF NOT EXISTS saga_version SMALLINT,
  ADD COLUMN IF NOT EXISTS request_spec JSONB,
  ADD COLUMN IF NOT EXISTS max_attempts INTEGER,
  ADD COLUMN IF NOT EXISTS lease_owner TEXT,
  ADD COLUMN IF NOT EXISTS receipt_recorded_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS reconcile_attempt_count INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS last_reconciled_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS compensation_attempt_count INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS finalized_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS compensated_at TIMESTAMPTZ;

ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_status_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_status_check CHECK (
  status IN (
    'queued','retry_wait','succeeded','cancelled',
    'pending','running','telegram_succeeded','finalized',
    'compensation_pending','compensated','failed'
  )
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_saga_request_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_saga_request_check CHECK (
  saga_version IS NULL OR (
    saga_version = 1 AND direction = 'upload'
    AND status IN ('pending','running','telegram_succeeded','finalized','compensation_pending','compensated','failed')
    AND request_fingerprint ~ '^[0-9a-f]{64}$'
    AND length(idempotency_key) BETWEEN 1 AND 200
    AND jsonb_typeof(request_spec) = 'object'
    AND request_spec ? 'source_ref'
    AND request_spec ? 'transport_mode'
    AND request_spec ? 'target'
    AND max_attempts BETWEEN 1 AND 100
  )
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_saga_lease_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_saga_lease_check CHECK (
  saga_version IS DISTINCT FROM 1 OR status <> 'running'
  OR (attempt_token IS NOT NULL AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_saga_receipt_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_saga_receipt_check CHECK (
  saga_version IS DISTINCT FROM 1
  OR status NOT IN ('telegram_succeeded','finalized')
  OR (telegram_message_id IS NOT NULL AND storage_peer_id IS NOT NULL AND storage_peer_kind IS NOT NULL AND telegram_file_size IS NOT NULL AND receipt_recorded_at IS NOT NULL)
);
ALTER TABLE transfer_jobs DROP CONSTRAINT IF EXISTS transfer_jobs_saga_terminal_check;
ALTER TABLE transfer_jobs ADD CONSTRAINT transfer_jobs_saga_terminal_check CHECK (
  saga_version IS DISTINCT FROM 1 OR (
    (status <> 'finalized' OR (asset_id IS NOT NULL AND finalized_at IS NOT NULL AND completed_at IS NOT NULL))
    AND (status <> 'compensated' OR (compensated_at IS NOT NULL AND completed_at IS NOT NULL))
    AND (status <> 'failed' OR completed_at IS NOT NULL)
  )
);

DROP INDEX IF EXISTS idx_jobs_receipt;
CREATE UNIQUE INDEX IF NOT EXISTS uq_jobs_upload_telegram_receipt
  ON transfer_jobs(storage_peer_id,telegram_message_id)
  WHERE saga_version=1 AND direction='upload' AND storage_peer_id IS NOT NULL AND telegram_message_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_jobs_upload_saga_due
  ON transfer_jobs(status,next_attempt_at,created_at,id)
  WHERE saga_version=1 AND direction='upload' AND status IN ('pending','telegram_succeeded','compensation_pending');
CREATE INDEX IF NOT EXISTS idx_jobs_upload_saga_expired_lease
  ON transfer_jobs(lease_expires_at,created_at,id)
  WHERE saga_version=1 AND direction='upload' AND status='running';

ALTER TABLE transfer_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE transfer_jobs FORCE ROW LEVEL SECURITY;

INSERT INTO schema_migrations(version) VALUES ('006_harden_upload_saga') ON CONFLICT DO NOTHING;
COMMIT;
