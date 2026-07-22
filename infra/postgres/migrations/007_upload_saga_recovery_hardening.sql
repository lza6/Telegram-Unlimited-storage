-- Additive recovery hardening for upload Saga rows after 005/006 are deployed.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

-- Bind pre-007 Saga rows to a deliberately non-routable legacy node. Operators
-- must rebind those rows explicitly before a node-specific recovery worker can
-- claim them.
UPDATE public.transfer_jobs
SET request_spec = jsonb_set(
      CASE
        WHEN jsonb_typeof(request_spec) = 'object' THEN request_spec
        ELSE '{}'::jsonb
      END,
      '{staging_node_id}',
      to_jsonb('legacy-unbound'::text),
      true
    ),
    updated_at = now()
WHERE saga_version = 1
  AND direction = 'upload'
  AND (
    jsonb_typeof(request_spec -> 'staging_node_id') IS DISTINCT FROM 'string'
    OR COALESCE(btrim(request_spec ->> 'staging_node_id'), '') = ''
  );

ALTER TABLE public.transfer_jobs
  DROP CONSTRAINT IF EXISTS transfer_jobs_saga_request_check;
ALTER TABLE public.transfer_jobs
  ADD CONSTRAINT transfer_jobs_saga_request_check CHECK (
    saga_version IS NULL OR (
      saga_version = 1
      AND direction = 'upload'
      AND status IN (
        'pending','running','telegram_succeeded','finalized',
        'compensation_pending','compensated','failed'
      )
      AND request_fingerprint ~ '^[0-9a-f]{64}$'
      AND length(idempotency_key) BETWEEN 1 AND 200
      AND btrim(idempotency_key) <> ''
      AND max_attempts BETWEEN 1 AND 100
      AND request_spec IS NOT NULL
      AND jsonb_typeof(request_spec) = 'object'
      AND request_spec ? 'source_ref'
      AND jsonb_typeof(request_spec -> 'source_ref') = 'string'
      AND COALESCE(btrim(request_spec ->> 'source_ref'), '') <> ''
      AND request_spec ? 'transport_mode'
      AND jsonb_typeof(request_spec -> 'transport_mode') = 'string'
      AND COALESCE(btrim(request_spec ->> 'transport_mode'), '') <> ''
      AND request_spec ? 'target'
      AND jsonb_typeof(request_spec -> 'target') = 'string'
      AND COALESCE(btrim(request_spec ->> 'target'), '') <> ''
      AND request_spec ? 'staging_node_id'
      AND jsonb_typeof(request_spec -> 'staging_node_id') = 'string'
      AND COALESCE(btrim(request_spec ->> 'staging_node_id'), '') <> ''
    )
  );

-- Recreate the intended definition unconditionally so a same-named, drifted
-- index cannot survive an IF NOT EXISTS shortcut. The transaction restores the
-- old index automatically if the replacement cannot be built.
DROP INDEX IF EXISTS public.uq_jobs_upload_telegram_receipt;
CREATE UNIQUE INDEX uq_jobs_upload_telegram_receipt
  ON public.transfer_jobs(storage_peer_id, telegram_message_id)
  WHERE saga_version = 1
    AND direction = 'upload'
    AND storage_peer_id IS NOT NULL
    AND telegram_message_id IS NOT NULL;

ALTER TABLE public.transfer_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.transfer_jobs FORCE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION public.claim_upload_saga_recovery(
  p_node_id TEXT,
  p_limit INTEGER
)
RETURNS TABLE (
  tenant_id UUID,
  job_id UUID,
  correlation_id UUID,
  status TEXT,
  attempt_token UUID,
  telegram_message_id BIGINT,
  storage_peer_id BIGINT,
  storage_peer_kind TEXT,
  telegram_file_id TEXT,
  telegram_file_name TEXT,
  telegram_file_size BIGINT,
  telegram_mime_type TEXT,
  receipt_recorded_at TIMESTAMPTZ,
  requested_folder_id BIGINT,
  source_ref TEXT,
  transport_mode TEXT,
  target TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  IF p_node_id IS NULL OR btrim(p_node_id) = '' THEN
    RAISE EXCEPTION 'recovery node id is required' USING ERRCODE = '22023';
  END IF;
  IF p_limit IS NULL OR p_limit < 1 OR p_limit > 100 THEN
    RAISE EXCEPTION 'recovery claim limit must be between 1 and 100' USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH claimable AS (
    SELECT candidate.id
    FROM public.transfer_jobs AS candidate
    WHERE candidate.saga_version = 1
      AND candidate.direction = 'upload'
      AND candidate.request_spec ->> 'staging_node_id' = p_node_id
      AND candidate.status IN ('telegram_succeeded', 'compensation_pending')
      AND (
        candidate.attempt_token IS NULL
        OR candidate.lease_owner IS NULL
        OR candidate.lease_expires_at IS NULL
        OR candidate.lease_expires_at <= now()
      )
    ORDER BY candidate.last_reconciled_at NULLS FIRST, candidate.created_at, candidate.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  )
  UPDATE public.transfer_jobs AS claimed
  SET attempt_token = gen_random_uuid(),
      lease_owner = 'recovery:' || btrim(p_node_id),
      lease_expires_at = now() + interval '5 minutes',
      reconcile_attempt_count = claimed.reconcile_attempt_count + 1,
      last_reconciled_at = now(),
      updated_at = now()
  FROM claimable
  WHERE claimed.id = claimable.id
  RETURNING
    claimed.tenant_id,
    claimed.id,
    claimed.correlation_id,
    claimed.status,
    claimed.attempt_token,
    claimed.telegram_message_id,
    claimed.storage_peer_id,
    claimed.storage_peer_kind,
    claimed.telegram_file_id,
    claimed.telegram_file_name,
    claimed.telegram_file_size,
    claimed.telegram_mime_type,
    claimed.receipt_recorded_at,
    claimed.requested_folder_id,
    claimed.request_spec ->> 'source_ref',
    claimed.request_spec ->> 'transport_mode',
    claimed.request_spec ->> 'target';
END;
$$;

REVOKE ALL ON FUNCTION public.claim_upload_saga_recovery(TEXT, INTEGER) FROM PUBLIC;
\getenv td_app_user POSTGRES_APP_USER
\if :{?td_app_user}
\else
\echo 'POSTGRES_APP_USER is missing from .env'
\quit 3
\endif
SELECT format(
  'GRANT EXECUTE ON FUNCTION public.claim_upload_saga_recovery(TEXT, INTEGER) TO %I',
  :'td_app_user'
)
\gexec

INSERT INTO public.schema_migrations(version)
VALUES ('007_upload_saga_recovery_hardening')
ON CONFLICT DO NOTHING;
COMMIT;
