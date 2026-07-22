-- Serialize concurrent legacy-tenant resolution for one owner key.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

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

  PERFORM pg_advisory_xact_lock(hashtextextended(p_legacy_owner_key, 0));
  SELECT tenant.id
  INTO resolved_id
  FROM public.tenants AS tenant
  WHERE tenant.legacy_owner_key = p_legacy_owner_key;
  IF resolved_id IS NOT NULL THEN
    RETURN resolved_id;
  END IF;

  INSERT INTO public.tenants(id, slug, display_name, legacy_owner_key)
  VALUES (
    p_candidate_id,
    'legacy-' || left(replace(p_candidate_id::text, '-', ''), 20),
    p_display_name,
    p_legacy_owner_key
  )
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

INSERT INTO public.schema_migrations(version)
VALUES ('008_serialize_legacy_tenant_resolver')
ON CONFLICT DO NOTHING;
COMMIT;