# PostgreSQL Telegram Upload Saga Design

Date: 2026-07-20
Risk: L3

## Goal
Connect Telegram upload to PostgreSQL without pretending two external systems share one transaction. Preserve SQLite compatibility until reconciliation evidence permits cutover.

## Options

### A. Post-upload best-effort dual write
Simple, but PostgreSQL failure leaves an untracked Telegram message and retries create duplicates. Rejected.

### B. Synchronous delete compensation only
Preflight PostgreSQL, upload to Telegram, then commit metadata; on failure delete Telegram. Better, but delete can fail and process crashes leave no durable recovery. Rejected as sole mechanism.

### C. Durable saga with pending job and reconciliation
Resolve tenant and validate restricted PostgreSQL role first. Create a pending job using a business idempotency key before Telegram. Telegram returns a structured receipt containing actual peer and message IDs. Finalize asset/ledger/audit using database-returned IDs. On failure, persist compensation/reconcile state and attempt Telegram deletion. Recommended.

## Required state transitions
`queued -> running -> succeeded | retry_wait | failed | cancelled`

A retry with the same tenant, direction and idempotency key reuses the existing job. Inconsistent replay payloads fail explicitly.

## Security boundaries
- Legacy owner is resolved by an audited SECURITY DEFINER routine; runtime does not assume deterministic tenant ID matches existing data.
- Runtime role must be non-superuser, non-BYPASSRLS and non-owner.
- NoTls is allowed only for loopback native PostgreSQL. Remote PostgreSQL requires a later TLS connector with CA validation before acceptance.
- Browser never receives PostgreSQL or Telegram credentials.

## Rollback
The new saga is feature-gated by `SAAS_DATABASE_MODE`. SQLite compatibility remains available. Additive migrations are not dropped automatically; rollback disables PostgreSQL mode after preserving reconciliation records.

## Validation
- Existing and new legacy mapping under forced RLS.
- Restricted-role fail-closed checks.
- Idempotent pending job replay and mismatched replay rejection.
- Actual Telegram receipt peer parity for Bot and User modes.
- Telegram success/PG failure, delete failure, process restart and reconcile tests.
## Implemented safety slices

- N-2A: `resolve_legacy_tenant()` is a fixed-search-path, least-privilege `SECURITY DEFINER` routine. It reuses an existing `legacy_owner_key` mapping even when the caller proposes a different deterministic UUID.
- N-2B: runtime PostgreSQL connections reject non-loopback `NoTls` hosts and verify that the connected role is not superuser, does not bypass RLS, does not own control-plane tables, and has no role memberships.
- N-2D foundation: upload transport can return a `TelegramUploadReceipt` containing the actual peer ID and peer kind for Bot channel, User Saved Messages, group, or channel destinations.
- Conflict handling now uses IDs returned by PostgreSQL `RETURNING`, so an existing asset/job row with a different UUID is not followed by writes using a fabricated candidate ID.
- The unsafe API dual-write hook remains intentionally disconnected until the pending-job Saga and durable compensation state exist.

## Red Team result

The most dangerous remaining assumption is that synchronous Telegram deletion is sufficient compensation. It is not: delete can fail, the process can crash after Telegram success, and an HTTP response can be lost. The minimum acceptable next experiment is a PostgreSQL pending job with a request fingerprint and idempotency key, followed by injected finalize failure and restart/reconcile tests. Conclusion: implement the durable Saga before reconnecting the request path.

## N-2C executable design

### Data model

The Saga reuses `transfer_jobs` as the durable business idempotency record and adds only additive columns: request fingerprint, source metadata, attempt fencing token, lease expiry, Telegram receipt fields, compensation status/error and completion time. The existing unique key `(tenant_id, direction, idempotency_key)` remains the admission lock.

### State machine

```text
queued -> running -> telegram_succeeded -> succeeded
                   \-> failed
telegram_succeeded -> compensation_pending -> compensated
running lease expired -> running with a new attempt token
```

A worker may mutate a running Saga only while its `attempt_token` matches. A reused idempotency key with a different request fingerprint is rejected. A succeeded replay returns the persisted receipt and does not contact Telegram.

### Transaction boundaries

1. `begin_upload`: resolve canonical tenant, set tenant scope, insert/select job under unique idempotency key, compare fingerprint, acquire a fenced attempt and lease.
2. Telegram upload: outside PostgreSQL transaction.
3. `record_receipt`: persist actual Telegram peer/message/file identity before asset finalization.
4. `finalize_upload`: one tenant-scoped transaction inserts/updates asset, ledger and audit using database-returned IDs, then marks the job succeeded.
5. Finalize failure: mark compensation pending, attempt Telegram deletion, then mark compensated or retain a durable receipt for reconciliation.

### Red Team

- Duplicate workers: unique idempotency key plus attempt fencing prevents two finalizers; expired lease takeover is allowed only with a new token.
- Same key/different file: request fingerprint mismatch is a hard conflict.
- PostgreSQL failure after Telegram success: receipt persistence and compensation state are required before reconnecting the route. A local receipt journal is the fallback when PostgreSQL cannot accept the receipt.
- Ambiguous Telegram response: exact-once cannot be guaranteed by Bot API alone. Correlation marker/recent-message reconciliation remains required before production acceptance.

Decision: implement the durable state machine and fault-injection tests first; keep real Telegram E2E disabled until compensation and ambiguous-response reconciliation evidence exist.

## Implementation update — 2026-07-20

The implemented state names are `pending -> running -> telegram_succeeded -> finalized`, with `compensation_pending -> compensated` and terminal `failed`. Expired `running` jobs are not blindly re-uploaded because the Telegram result may be ambiguous. Recovery is node-bound and consumes an append-only local receipt journal; database-only `telegram_succeeded` and `compensation_pending` rows are claimed through a role-bound `SECURITY DEFINER` function. The remaining production boundary is correlation-marker/recent-message reconciliation for a Telegram success whose response is lost before a receipt can be journaled.