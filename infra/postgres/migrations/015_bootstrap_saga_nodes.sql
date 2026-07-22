-- Bootstrap one operator credential and one runtime Saga node without granting
-- the shared application database role direct registry writes.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';

\getenv saga_operator_id SAGA_OPERATOR_ID
\getenv saga_operator_token SAGA_OPERATOR_TOKEN
\getenv saga_node_id SAGA_NODE_ID
\getenv saga_node_token SAGA_NODE_TOKEN
\if :{?saga_operator_id}
\else
\echo 'SAGA_OPERATOR_ID is missing from .env'
\quit 3
\endif
\if :{?saga_operator_token}
\else
\echo 'SAGA_OPERATOR_TOKEN is missing from .env'
\quit 3
\endif
\if :{?saga_node_id}
\else
\echo 'SAGA_NODE_ID is missing from .env'
\quit 3
\endif
\if :{?saga_node_token}
\else
\echo 'SAGA_NODE_TOKEN is missing from .env'
\quit 3
\endif

CREATE TEMP TABLE saga_bootstrap_credentials(
  operator_id text, operator_token text, runtime_node_id text, runtime_node_token text
) ON COMMIT DROP;
INSERT INTO saga_bootstrap_credentials
VALUES (:'saga_operator_id', :'saga_operator_token', :'saga_node_id', :'saga_node_token');

DO $bootstrap$
DECLARE
  operator_id text;
  operator_token text;
  runtime_node_id text;
  runtime_node_token text;
BEGIN
  SELECT c.operator_id, c.operator_token, c.runtime_node_id, c.runtime_node_token
    INTO operator_id, operator_token, runtime_node_id, runtime_node_token
    FROM saga_bootstrap_credentials c;

  IF operator_id <> btrim(operator_id) OR length(operator_id) NOT BETWEEN 1 AND 128
     OR runtime_node_id <> btrim(runtime_node_id) OR length(runtime_node_id) NOT BETWEEN 1 AND 128
     OR length(operator_token) NOT BETWEEN 32 AND 4096
     OR length(runtime_node_token) NOT BETWEEN 32 AND 4096 THEN
    RAISE EXCEPTION 'invalid Saga bootstrap credentials' USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.saga_node_registry(node_id, token_hash, status, is_operator, metadata)
  VALUES (operator_id, digest(convert_to(operator_token, 'UTF8'), 'sha256'), 'active', true,
          jsonb_build_object('bootstrap', true, 'purpose', 'operator'))
  ON CONFLICT(node_id) DO UPDATE
    SET token_hash=EXCLUDED.token_hash, status='active', is_operator=true,
        metadata=EXCLUDED.metadata, drain_requested_at=NULL, disabled_at=NULL, updated_at=now();

  INSERT INTO public.saga_node_registry(node_id, token_hash, status, is_operator, metadata)
  VALUES (runtime_node_id, digest(convert_to(runtime_node_token, 'UTF8'), 'sha256'), 'active', false,
          jsonb_build_object('bootstrap', true, 'purpose', 'runtime'))
  ON CONFLICT(node_id) DO UPDATE
    SET token_hash=EXCLUDED.token_hash, status='active', is_operator=false,
        metadata=EXCLUDED.metadata, drain_requested_at=NULL, disabled_at=NULL, updated_at=now();
END
$bootstrap$;

INSERT INTO public.schema_migrations(version)
VALUES ('015_bootstrap_saga_nodes') ON CONFLICT DO NOTHING;
COMMIT;