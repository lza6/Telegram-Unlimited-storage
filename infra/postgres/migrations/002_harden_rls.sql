-- Enforce PostgreSQL RLS for every tenant- or identity-scoped control-plane table.
-- The runtime must connect as POSTGRES_APP_USER, never as the migration owner,
-- and execute SET LOCAL app.tenant_id / app.user_id after authentication in the
-- same transaction as every tenant query.
BEGIN;

CREATE OR REPLACE FUNCTION app_tenant_id()
RETURNS UUID
LANGUAGE sql
STABLE
AS $$
  SELECT NULLIF(current_setting('app.tenant_id', true), '')::uuid;
$$;

CREATE OR REPLACE FUNCTION app_user_id()
RETURNS UUID
LANGUAGE sql
STABLE
AS $$
  SELECT NULLIF(current_setting('app.user_id', true), '')::uuid;
$$;

DO $$
DECLARE
  table_name TEXT;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'tenants', 'users', 'memberships', 'system_admins', 'tenant_api_keys',
    'web_sessions', 'tenant_quotas', 'assets', 'transfer_jobs', 'usage_ledger',
    'webhook_endpoints', 'webhook_deliveries', 'audit_events'
  ]
  LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
  END LOOP;
END $$;

-- Remove the old partial policies first so this migration repairs already-created
-- local databases as well as clean installs.
DROP POLICY IF EXISTS tenant_assets_scope ON assets;
DROP POLICY IF EXISTS tenant_jobs_scope ON transfer_jobs;
DROP POLICY IF EXISTS tenant_ledger_scope ON usage_ledger;
DROP POLICY IF EXISTS tenant_webhook_scope ON webhook_endpoints;
DROP POLICY IF EXISTS tenant_delivery_scope ON webhook_deliveries;
DROP POLICY IF EXISTS tenant_audit_scope ON audit_events;
DROP POLICY IF EXISTS tenants_tenant_scope ON tenants;
DROP POLICY IF EXISTS users_user_scope ON users;
DROP POLICY IF EXISTS memberships_tenant_scope ON memberships;
DROP POLICY IF EXISTS system_admins_user_scope ON system_admins;
DROP POLICY IF EXISTS tenant_api_keys_tenant_scope ON tenant_api_keys;
DROP POLICY IF EXISTS web_sessions_user_scope ON web_sessions;
DROP POLICY IF EXISTS tenant_quotas_tenant_scope ON tenant_quotas;
DROP POLICY IF EXISTS tenant_assets_tenant_scope ON assets;
DROP POLICY IF EXISTS tenant_jobs_tenant_scope ON transfer_jobs;
DROP POLICY IF EXISTS tenant_ledger_tenant_scope ON usage_ledger;
DROP POLICY IF EXISTS tenant_webhook_endpoints_tenant_scope ON webhook_endpoints;
DROP POLICY IF EXISTS tenant_webhook_deliveries_tenant_scope ON webhook_deliveries;
DROP POLICY IF EXISTS tenant_audit_events_tenant_scope ON audit_events;

CREATE POLICY tenants_tenant_scope ON tenants
  USING (id = app_tenant_id()) WITH CHECK (id = app_tenant_id());
CREATE POLICY users_user_scope ON users
  USING (id = app_user_id()) WITH CHECK (id = app_user_id());
CREATE POLICY memberships_tenant_scope ON memberships
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY system_admins_user_scope ON system_admins
  USING (user_id = app_user_id()) WITH CHECK (user_id = app_user_id());
CREATE POLICY tenant_api_keys_tenant_scope ON tenant_api_keys
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY web_sessions_user_scope ON web_sessions
  USING (user_id = app_user_id()) WITH CHECK (user_id = app_user_id());
CREATE POLICY tenant_quotas_tenant_scope ON tenant_quotas
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_assets_tenant_scope ON assets
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_jobs_tenant_scope ON transfer_jobs
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_ledger_tenant_scope ON usage_ledger
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_webhook_endpoints_tenant_scope ON webhook_endpoints
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_webhook_deliveries_tenant_scope ON webhook_deliveries
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());
CREATE POLICY tenant_audit_events_tenant_scope ON audit_events
  USING (tenant_id = app_tenant_id()) WITH CHECK (tenant_id = app_tenant_id());

-- The app login gets DML only on data that is always protected by tenant RLS.
-- Users, sessions and system-admin records remain owner-only until the W2
-- identity service exposes audited SECURITY DEFINER routines for login/session
-- issuance. This avoids a broad unauthenticated read grant.
\getenv td_app_user POSTGRES_APP_USER
\if :{?td_app_user}
\else
\echo 'POSTGRES_APP_USER is missing from .env'
\quit 3
\endif
SELECT format('GRANT SELECT, INSERT, UPDATE, DELETE ON tenants, memberships, tenant_api_keys, tenant_quotas, assets, transfer_jobs, usage_ledger, webhook_endpoints, webhook_deliveries, audit_events TO %I', :'td_app_user')
\gexec
GRANT EXECUTE ON FUNCTION app_tenant_id() TO PUBLIC;
GRANT EXECUTE ON FUNCTION app_user_id() TO PUBLIC;

INSERT INTO schema_migrations(version) VALUES ('002_harden_rls') ON CONFLICT DO NOTHING;
COMMIT;