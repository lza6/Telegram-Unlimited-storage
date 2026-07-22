\set ON_ERROR_STOP on
\getenv migration_root TD_MIGRATIONS_ROOT

\if :{?migration_root}
\else
\echo 'TD_MIGRATIONS_ROOT is not set'
\quit 3
\endif

SELECT pg_advisory_lock(hashtext('telegram_drive_saas_migrations'));
CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

\set migration_version '000_bootstrap_app_role'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 000_bootstrap_app_role (already applied)'
\else
\i :migration_root/000_bootstrap_app_role.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '001_saas_control_plane'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 001_saas_control_plane (already applied)'
\else
\i :migration_root/001_saas_control_plane.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '002_harden_rls'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 002_harden_rls (already applied)'
\else
\i :migration_root/002_harden_rls.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '003_legacy_owner_mapping'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 003_legacy_owner_mapping (already applied)'
\else
\i :migration_root/003_legacy_owner_mapping.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '004_secure_legacy_tenant_resolver'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 004_secure_legacy_tenant_resolver (already applied)'
\else
\i :migration_root/004_secure_legacy_tenant_resolver.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '005_upload_saga_state'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 005_upload_saga_state (already applied)'
\else
\i :migration_root/005_upload_saga_state.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '006_harden_upload_saga'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 006_harden_upload_saga (already applied)'
\else
\i :migration_root/006_harden_upload_saga.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '007_upload_saga_recovery_hardening'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 007_upload_saga_recovery_hardening (already applied)'
\else
\i :migration_root/007_upload_saga_recovery_hardening.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '008_serialize_legacy_tenant_resolver'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 008_serialize_legacy_tenant_resolver (already applied)'
\else
\i :migration_root/008_serialize_legacy_tenant_resolver.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '009_bind_recovery_claim_to_role'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 009_bind_recovery_claim_to_role (already applied)'
\else
\i :migration_root/009_bind_recovery_claim_to_role.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '010_saga_node_registry_and_shared_role'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 010_saga_node_registry_and_shared_role (already applied)'
\else
\i :migration_root/010_saga_node_registry_and_shared_role.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '011_upload_recovery_retry_policy'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 011_upload_recovery_retry_policy (already applied)'
\else
\i :migration_root/011_upload_recovery_retry_policy.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '012_saga_node_drain_rebind'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 012_saga_node_drain_rebind (already applied)'
\else
\i :migration_root/012_saga_node_drain_rebind.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '013_asset_locator_and_download_accounting'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 013_asset_locator_and_download_accounting (already applied)'
\else
\i :migration_root/013_asset_locator_and_download_accounting.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '014_durable_transfer_scheduler'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 014_durable_transfer_scheduler (already applied)'
\else
\i :migration_root/014_durable_transfer_scheduler.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '015_bootstrap_saga_nodes'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 015_bootstrap_saga_nodes (already applied)'
\else
\i :migration_root/015_bootstrap_saga_nodes.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif

\set migration_version '016_fix_scheduler_acquire_ambiguity'
SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = :'migration_version') AS migration_applied \gset
\if :migration_applied
\echo 'Skipping 016_fix_scheduler_acquire_ambiguity (already applied)'
\else
\i :migration_root/016_fix_scheduler_acquire_ambiguity.sql
INSERT INTO schema_migrations(version) VALUES (:'migration_version') ON CONFLICT DO NOTHING;
\endif
SELECT pg_advisory_unlock(hashtext('telegram_drive_saas_migrations'));
