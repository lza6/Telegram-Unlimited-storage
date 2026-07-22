# Implementation Plan

## P0: Control-plane integration
1. Add additive migration for legacy API-owner to PostgreSQL tenant mapping.
2. Add scoped PostgreSQL adapter using the restricted app role.
3. Dual-write upload success: tenant, asset, ledger, audit and transfer job.
4. Dual-write download completion: usage and audit.
5. Verify RLS, idempotency and actual Bot asset parity before changing read authority.

## P1: Identity, quotas and durable jobs
6. Add registration/login/session/RBAC and tenant context resolver.
7. Replace compatibility owner mapping with authenticated tenants.
8. Quota reservation/release, immutable ledger reconciliation and aggregates.
9. Durable transfer job/outbox/scheduler; only then extend current memory/Redis gate.

## P2: Product surface
10. Tenant console, usage, gallery, media preview, transfer center, API/callback pages.
11. System admin console and audited aggregate views.
12. Explicit public permanent asset state and stable range gateway route.

## P3: Final acceptance
13. Real image/video/document upload/download, interrupted transfer, duplicate idempotency, tenant denial, callback retry and two-instance evidence.
14. Critic review, repair loop, Evaluator report, no-Docker operations documentation.
## N-2C durable Saga slice
1. Add additive transfer-job Saga columns and replay-safe migration.
2. Implement begin/receipt/finalize/fail/compensation transitions with attempt fencing.
3. Require a stable Idempotency-Key in PostgreSQL mode and fingerprint the staged request.
4. Inject upload/finalize/delete failures without contacting Telegram.
5. Reconnect REST upload only after Critic approval.
