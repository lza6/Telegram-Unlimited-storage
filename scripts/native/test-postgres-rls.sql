\set ON_ERROR_STOP on
BEGIN;
SET LOCAL app.tenant_id = '11111111-1111-1111-1111-111111111111';
INSERT INTO tenants (id, slug, display_name)
VALUES ('11111111-1111-1111-1111-111111111111', 'rls-test-a', 'RLS test tenant A');
SELECT 'RLS_TENANT_A_VISIBLE=' || count(*) FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111';
SELECT count(*) = 1 AS tenant_a_ok FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111' \gset
\if :tenant_a_ok
\else
\quit 4
\endif

SET LOCAL app.tenant_id = '22222222-2222-2222-2222-222222222222';
SELECT 'RLS_TENANT_B_SEES_A=' || count(*) FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111';
SELECT count(*) = 0 AS tenant_b_denied FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111' \gset
\if :tenant_b_denied
\else
\quit 4
\endif

SET LOCAL app.tenant_id = '';
SELECT 'RLS_UNSCOPED_SEES_A=' || count(*) FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111';
SELECT count(*) = 0 AS unscoped_denied FROM tenants WHERE id = '11111111-1111-1111-1111-111111111111' \gset
\if :unscoped_denied
\else
\quit 4
\endif

SELECT resolve_legacy_tenant(
  'rls-resolver-owner',
  '33333333-3333-3333-3333-333333333333',
  'RLS resolver tenant'
) AS resolved_tenant_id
\gset
SELECT 'RLS_RESOLVER_CREATED=' || (:'resolved_tenant_id' = '33333333-3333-3333-3333-333333333333')::int;
SELECT :'resolved_tenant_id' = '33333333-3333-3333-3333-333333333333' AS resolver_created \gset
\if :resolver_created
\else
\quit 4
\endif
SELECT resolve_legacy_tenant(
  'rls-resolver-owner',
  '44444444-4444-4444-4444-444444444444',
  'RLS resolver tenant replay'
) AS replay_tenant_id
\gset
SELECT 'RLS_RESOLVER_REUSES_EXISTING=' || (:'replay_tenant_id' = :'resolved_tenant_id')::int;
SELECT :'replay_tenant_id' = :'resolved_tenant_id' AS resolver_reused \gset
\if :resolver_reused
\else
\quit 4
\endif
SET LOCAL app.tenant_id = :'resolved_tenant_id';
SELECT 'RLS_RESOLVER_SCOPED_VISIBLE=' || count(*) FROM tenants WHERE id = :'resolved_tenant_id'::uuid;
SELECT count(*) = 1 AS resolver_scoped_visible FROM tenants WHERE id = :'resolved_tenant_id'::uuid \gset
\if :resolver_scoped_visible
\else
\quit 4
\endif
SET LOCAL app.tenant_id = '22222222-2222-2222-2222-222222222222';
SELECT 'RLS_RESOLVER_CROSS_TENANT_VISIBLE=' || count(*) FROM tenants WHERE id = :'resolved_tenant_id'::uuid;
SELECT count(*) = 0 AS resolver_cross_denied FROM tenants WHERE id = :'resolved_tenant_id'::uuid \gset
\if :resolver_cross_denied
\else
\quit 4
\endif
ROLLBACK;