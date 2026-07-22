# Native PostgreSQL Control Plane (Windows, no Docker)

This directory provisions the production control-plane schema. It is a **foundation**, not evidence that the current SQLite-backed API server has already been switched to PostgreSQL.

## Runtime roles and isolation

- `POSTGRES_USER` is the local migration/owner login. Never configure the API service with it.
- `POSTGRES_APP_USER` is created by `000_bootstrap_app_role.sql` as a non-owner, non-superuser, `NOBYPASSRLS` application login.
- `002_harden_rls.sql` enables and **forces** RLS on every tenant- or identity-scoped table. It grants the app role only tenant-scoped DML; it intentionally does not grant direct user/session reads before the W2 identity routines exist.
- Future PostgreSQL request code must begin each scoped transaction with `SET LOCAL app.tenant_id = '<uuid>'` (and, where needed, `SET LOCAL app.user_id = '<uuid>'`) after authentication. A query without that context returns no tenant rows.
- System-admin cross-tenant reads must use a separate, audited path; never solve this with an owner connection or `BYPASSRLS`.

## Required `.env` keys

Copy the examples from `.env.example`, then use distinct strong values:

```dotenv
SAAS_DATABASE_MODE=sqlite
DATABASE_URL=postgresql://...
POSTGRES_HOST=127.0.0.1
POSTGRES_PORT=15432
POSTGRES_DB=telegram_drive_saas
POSTGRES_USER=postgres
POSTGRES_PASSWORD=...
POSTGRES_APP_USER=telegram_drive_app
POSTGRES_APP_PASSWORD=...
```

`SAAS_DATABASE_MODE` must remain `sqlite` until the Rust control-plane adapter is implemented. The current request path still uses SQLite; setting it to `postgres` now would be a false production claim.

## Commands

Use CMD, not PowerShell profile-dependent commands:

```bat
scripts\native\start-postgres.bat
scripts\native\migrate-postgres.bat
scripts\native\test-postgres-rls.bat
```

The migration runner holds one PostgreSQL advisory lock for the full sequence, creates the migration table if required, and skips versions already recorded. It can be run again safely. The RLS test leaves no data behind: it inserts inside a transaction and rolls back after proving tenant A visibility, tenant B denial, and unscoped denial.

## Migration rules

- Treat applied files as immutable. Add `003_...sql` for every later schema change.
- Do not manually replay an individual migration file against a populated database; use `migrate-postgres.bat` so the version ledger and advisory lock are honored.
- Back up the PostgreSQL database before destructive changes. The current migrations are additive/hardening-only and have no automatic down migration.