# Verification Plan

- Database: migration replay, non-owner RLS, tenant/no-context denial, idempotent asset/ledger writes, rollback.
- API: upload/download authorization, error behavior, idempotency, quota denial and audit correlation.
- Telegram: real Bot image/video/document upload, download SHA-256 equality, retry/failure evidence.
- UI: component/accessibility tests, browser flows after API paths exist.
- Operations: native CMD startup, PostgreSQL health/migration, encrypted backup/restore, no Docker dependency.
- Review: independent Critic after each high-risk implementation slice and final Evaluator against `spec.md` requirements.
## N-2C
- same idempotency key and same fingerprint replays without a second Telegram call;
- same key with different fingerprint returns conflict;
- active lease returns in-progress; expired lease issues a new fencing token;
- receipt/finalize writes canonical asset/job/ledger/audit IDs;
- finalize failure persists compensation state; delete failure remains reconcilable;
- no real Telegram mutation is used for fault-injection tests.

### N-2C evidence update — 2026-07-20

Local gates passed: migration replay through 009, RLS denial, 20-way concurrent admission, receipt/finalize replay, compensation state, journal durability helper, API route tests, Headless build and fmt. Real Telegram mutation, ambiguous-response reconciliation and crash-after-delete fault injection were deliberately not executed; therefore N-2C remains Review rather than Done.