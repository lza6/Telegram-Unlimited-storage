-- Separate the shared database login from per-process Saga node identity.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS public.saga_node_registry (
  node_id TEXT PRIMARY KEY,
  token_hash BYTEA NOT NULL CHECK (octet_length(token_hash) = 32),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'draining', 'disabled')),
  is_operator BOOLEAN NOT NULL DEFAULT false,
  registered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_authenticated_at TIMESTAMPTZ,
  drain_requested_at TIMESTAMPTZ,
  disabled_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  CHECK (node_id = btrim(node_id) AND length(node_id) BETWEEN 1 AND 128),
  CHECK ((status <> 'draining' OR drain_requested_at IS NOT NULL)
     AND (status <> 'disabled' OR disabled_at IS NOT NULL))
);
REVOKE ALL ON TABLE public.saga_node_registry FROM PUBLIC;

CREATE OR REPLACE FUNCTION public.authenticate_saga_node(
  p_node_id TEXT, p_node_token TEXT,
  p_require_operator BOOLEAN DEFAULT false,
  p_allow_draining BOOLEAN DEFAULT false
) RETURNS public.saga_node_registry
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE authenticated public.saga_node_registry%ROWTYPE;
BEGIN
  IF p_node_id IS NULL OR p_node_id <> btrim(p_node_id)
     OR length(p_node_id) NOT BETWEEN 1 AND 128
     OR p_node_token IS NULL OR length(p_node_token) NOT BETWEEN 32 AND 4096 THEN
    RAISE EXCEPTION 'invalid Saga node credentials' USING ERRCODE = '42501';
  END IF;
  SELECT r.* INTO authenticated FROM public.saga_node_registry r
   WHERE r.node_id = p_node_id
     AND r.token_hash = digest(convert_to(p_node_token, 'UTF8'), 'sha256');
  IF NOT FOUND OR authenticated.status = 'disabled'
     OR (authenticated.status = 'draining' AND NOT p_allow_draining)
     OR (p_require_operator AND NOT authenticated.is_operator) THEN
    RAISE EXCEPTION 'invalid Saga node credentials' USING ERRCODE = '42501';
  END IF;
  UPDATE public.saga_node_registry SET last_authenticated_at = now(), updated_at = now()
   WHERE node_id = authenticated.node_id;
  RETURN authenticated;
END;
$$;

-- PostgreSQL cannot rename input parameters via CREATE OR REPLACE; replace the legacy signature explicitly.
DROP FUNCTION IF EXISTS public.claim_upload_saga_recovery(TEXT, INTEGER);

-- Fail closed for callers that omit SAGA_NODE_TOKEN.
CREATE OR REPLACE FUNCTION public.claim_upload_saga_recovery(TEXT, INTEGER)
RETURNS TABLE (
  tenant_id UUID, job_id UUID, correlation_id UUID, status TEXT, attempt_token UUID,
  telegram_message_id BIGINT, storage_peer_id BIGINT, storage_peer_kind TEXT,
  telegram_file_id TEXT, telegram_file_name TEXT, telegram_file_size BIGINT,
  telegram_mime_type TEXT, receipt_recorded_at TIMESTAMPTZ, requested_folder_id BIGINT,
  source_ref TEXT, transport_mode TEXT, target TEXT
) LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
  RAISE EXCEPTION 'legacy recovery claim is disabled; node token is required'
    USING ERRCODE = '42501';
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_upload_saga_recovery(
  p_node_id TEXT, p_node_token TEXT, p_limit INTEGER
) RETURNS TABLE (
  tenant_id UUID, job_id UUID, correlation_id UUID, status TEXT, attempt_token UUID,
  telegram_message_id BIGINT, storage_peer_id BIGINT, storage_peer_kind TEXT,
  telegram_file_id TEXT, telegram_file_name TEXT, telegram_file_size BIGINT,
  telegram_mime_type TEXT, receipt_recorded_at TIMESTAMPTZ, requested_folder_id BIGINT,
  source_ref TEXT, transport_mode TEXT, target TEXT
) LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
  PERFORM public.authenticate_saga_node(p_node_id, p_node_token, false, false);
  IF p_limit IS NULL OR p_limit NOT BETWEEN 1 AND 100 THEN
    RAISE EXCEPTION 'recovery claim limit must be between 1 and 100' USING ERRCODE = '22023';
  END IF;
  RETURN QUERY
  WITH claimable AS (
    SELECT c.id FROM public.transfer_jobs c
     WHERE c.saga_version = 1 AND c.direction = 'upload'
       AND c.request_spec ->> 'staging_node_id' = p_node_id
       AND c.status IN ('telegram_succeeded', 'compensation_pending')
       AND (c.attempt_token IS NULL OR c.lease_owner IS NULL
            OR c.lease_expires_at IS NULL OR c.lease_expires_at <= now())
     ORDER BY c.last_reconciled_at NULLS FIRST, c.created_at, c.id
     FOR UPDATE SKIP LOCKED LIMIT p_limit
  )
  UPDATE public.transfer_jobs j
     SET attempt_token = gen_random_uuid(), lease_owner = 'recovery:' || p_node_id,
         lease_expires_at = now() + interval '5 minutes',
         reconcile_attempt_count = j.reconcile_attempt_count + 1,
         last_reconciled_at = now(), updated_at = now()
    FROM claimable WHERE j.id = claimable.id
  RETURNING j.tenant_id, j.id, j.correlation_id, j.status, j.attempt_token,
    j.telegram_message_id, j.storage_peer_id, j.storage_peer_kind, j.telegram_file_id,
    j.telegram_file_name, j.telegram_file_size, j.telegram_mime_type,
    j.receipt_recorded_at, j.requested_folder_id, j.request_spec ->> 'source_ref',
    j.request_spec ->> 'transport_mode', j.request_spec ->> 'target';
END;
$$;

REVOKE ALL ON FUNCTION public.authenticate_saga_node(TEXT, TEXT, BOOLEAN, BOOLEAN) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.claim_upload_saga_recovery(TEXT, INTEGER) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.claim_upload_saga_recovery(TEXT, TEXT, INTEGER) FROM PUBLIC;
\getenv td_app_user POSTGRES_APP_USER
\if :{?td_app_user}
\else
\echo 'POSTGRES_APP_USER is missing from .env'
\quit 3
\endif
SELECT format('REVOKE ALL ON FUNCTION public.claim_upload_saga_recovery(TEXT, INTEGER) FROM %I', :'td_app_user') \gexec
SELECT format('GRANT EXECUTE ON FUNCTION public.claim_upload_saga_recovery(TEXT, TEXT, INTEGER) TO %I', :'td_app_user') \gexec

INSERT INTO public.schema_migrations(version)
VALUES ('010_saga_node_registry_and_shared_role') ON CONFLICT DO NOTHING;
COMMIT;
