BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

ALTER TABLE assets
  ADD COLUMN IF NOT EXISTS transport_mode TEXT,
  ADD COLUMN IF NOT EXISTS storage_peer_kind TEXT,
  ADD COLUMN IF NOT EXISTS uploader_bot_id TEXT,
  ADD COLUMN IF NOT EXISTS bot_pool_index INTEGER;

ALTER TABLE transfer_jobs
  ADD COLUMN IF NOT EXISTS uploader_bot_id TEXT,
  ADD COLUMN IF NOT EXISTS bot_pool_index INTEGER;

UPDATE assets AS asset
SET storage_peer_kind = job.storage_peer_kind,
    transport_mode = COALESCE(job.request_spec->>'transport_mode', 'user'),
    uploader_bot_id = job.uploader_bot_id,
    bot_pool_index = job.bot_pool_index
FROM transfer_jobs AS job
WHERE job.asset_id = asset.id
  AND (asset.storage_peer_kind IS NULL OR asset.transport_mode IS NULL);

CREATE INDEX IF NOT EXISTS idx_assets_tenant_canonical_locator
  ON assets(tenant_id, transport_mode, storage_channel_id, storage_peer_kind, telegram_message_id)
  WHERE deleted_at IS NULL AND status = 'ready';

CREATE INDEX IF NOT EXISTS idx_jobs_download_due
  ON transfer_jobs(tenant_id, status, next_attempt_at, created_at)
  WHERE direction = 'download';

COMMENT ON COLUMN assets.storage_channel_id IS
  'Canonical Telegram bot-api dialog peer id; historical name retained for compatibility.';
COMMENT ON COLUMN assets.storage_peer_kind IS
  'Telegram peer kind captured from the upload receipt.';
COMMENT ON COLUMN assets.transport_mode IS
  'Immutable transport used to create the asset: bot or user.';
COMMENT ON COLUMN assets.uploader_bot_id IS
  'Redacted stable uploader Bot identity; never a Bot token.';

INSERT INTO schema_migrations(version) VALUES ('013_asset_locator_and_download_accounting')
ON CONFLICT (version) DO NOTHING;
COMMIT;
