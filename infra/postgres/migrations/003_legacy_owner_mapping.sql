-- Compatibility mapping for API-key/legacy owner identifiers during PostgreSQL dual-write.
BEGIN;
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS legacy_owner_key TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_legacy_owner_key ON tenants(legacy_owner_key) WHERE legacy_owner_key IS NOT NULL;
INSERT INTO schema_migrations(version) VALUES ('003_legacy_owner_mapping') ON CONFLICT DO NOTHING;
COMMIT;