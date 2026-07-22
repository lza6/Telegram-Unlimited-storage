-- Resolve a legacy/API owner key to the canonical tenant UUID under forced RLS.
-- The runtime role cannot insert or discover unscoped tenant rows directly, so this
-- narrow SECURITY DEFINER routine is the only compatibility bridge.
BEGIN;

CREATE OR REPLACE FUNCTION public.resolve_legacy_tenant(
  p_legacy_owner_key TEXT,
  p_candidate_id UUID,
  p_display_name TEXT
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
  resolved_id UUID;
BEGIN
  IF p_legacy_owner_key IS NULL
     OR btrim(p_legacy_owner_key) = ''
     OR length(p_legacy_owner_key) > 255 THEN
    RAISE EXCEPTION 'legacy owner key is invalid' USING ERRCODE = '22023';
  END IF;
  IF p_candidate_id IS NULL THEN
    RAISE EXCEPTION 'candidate tenant id is required' USING ERRCODE = '22023';
  END IF;
  IF p_display_name IS NULL
     OR btrim(p_display_name) = ''
     OR length(p_display_name) > 255 THEN
    RAISE EXCEPTION 'tenant display name is invalid' USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.tenants(id, slug, display_name, legacy_owner_key)
  VALUES (
    p_candidate_id,
    'legacy-' || left(replace(p_candidate_id::text, '-', ''), 20),
    p_display_name,
    p_legacy_owner_key
  )
  ON CONFLICT (legacy_owner_key) WHERE legacy_owner_key IS NOT NULL
  DO UPDATE SET updated_at = public.tenants.updated_at
  RETURNING id INTO resolved_id;

  RETURN resolved_id;
END;
$$;

REVOKE ALL ON FUNCTION public.resolve_legacy_tenant(TEXT, UUID, TEXT) FROM PUBLIC;
\getenv td_app_user POSTGRES_APP_USER
\if :{?td_app_user}
\else
\echo 'POSTGRES_APP_USER is missing from .env'
\quit 3
\endif
SELECT format(
  'GRANT EXECUTE ON FUNCTION public.resolve_legacy_tenant(TEXT, UUID, TEXT) TO %I',
  :'td_app_user'
)
\gexec

INSERT INTO schema_migrations(version)
VALUES ('004_secure_legacy_tenant_resolver')
ON CONFLICT DO NOTHING;
COMMIT;
