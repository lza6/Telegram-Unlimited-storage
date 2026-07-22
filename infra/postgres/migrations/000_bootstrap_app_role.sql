-- Bootstrap the restricted, non-owner application login.
-- This migration is executed only by scripts/native/migrate-postgres.bat.
-- psql reads these values from environment variables, so the password is never
-- rendered into a command line or migration log.
\getenv td_app_user POSTGRES_APP_USER
\getenv td_app_password POSTGRES_APP_PASSWORD

\if :{?td_app_user}
\else
\echo 'POSTGRES_APP_USER is missing from .env'
\quit 3
\endif

\if :{?td_app_password}
\else
\echo 'POSTGRES_APP_PASSWORD is missing from .env'
\quit 3
\endif

BEGIN;

SELECT format(
  'DO $role$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = %L) THEN CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION PASSWORD %L; END IF; END $role$;',
  :'td_app_user', :'td_app_user', :'td_app_password'
)
\gexec

-- Pin the existing role back to the restricted configuration as well. It keeps
-- bootstrap reruns safe if a local operator accidentally changed its flags.
SELECT format(
  'ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION PASSWORD %L',
  :'td_app_user', :'td_app_password'
)
\gexec

SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'td_app_user')
\gexec
SELECT format('GRANT USAGE ON SCHEMA public TO %I', :'td_app_user')
\gexec

COMMIT;