# Telegram Drive Roadmap

Date: 2026-07-20

This document lists future work only. It does not describe features as completed.

| Priority | Direction | Technical approach | Expected impact | Boundary from current delivery |
|---|---|---|---|---|
| P0 | Durable upload/download Saga | PostgreSQL pending jobs, request fingerprints, idempotency keys, Telegram receipt finalization, compensation/reconcile workers | prevents duplicate messages and orphaned assets after crashes or DB failures | not implemented in this slice |
| P0 | Layered global concurrency | durable scheduler keyed by global/tenant/bot/account/peer/transport/method, shared retry_after/FLOOD_WAIT cooldowns | tenant fairness and safe Telegram throughput | current in-memory gate is insufficient |
| P0 | Identity and RBAC | tenant registration, sessions, memberships, system admin, audited SECURITY DEFINER auth routines | real multi-tenant SaaS access control | not implemented |
| P1 | Transfer center UX | queued/running/retry_wait states, bytes, speed, ETA, queue position, retry deadline, cancel/retry, notifications | transparent long-running transfers | UI not implemented |
| P1 | Resumable transfer | staged chunk manifests, per-part hashes, MTProto offset/limit, restart recovery | recover interrupted uploads/downloads | public Bot API alone cannot provide native Range/resume |
| P1 | Gallery and media preview | durable media-index jobs, image thumbnails, video posters, codec/duration metadata, failed-processing states | advanced asset browsing and preview | not implemented |
| P1 | Stable public links | project-owned asset IDs, revocable public state, Range-capable gateway/cache, rate limits | permanent URLs independent of temporary Telegram file_path | not implemented |
| P1 | Observability | queue depth/age, per-lane saturation, FloodWait, p50/p95 speeds, staging disk, reconciliation backlog | production diagnosis and capacity planning | only local logs exist |
| P2 | Remote PostgreSQL TLS | rustls/native-tls connector, CA verification, rotation and health checks | safe remote database deployment | current runtime rejects non-loopback NoTls |
| P2 | Security hardening | token-redaction regression tests, webhook SSRF controls, API-key scopes, audit export | reduces credential and integration risk | token URL redaction implemented; broader work open |
| P2 | Commercial controls | tenant plans, immutable quota ledger, retention policies, overage and admin reporting | monetization and governance | no billing implementation |
| P2 | Accessibility/performance | WCAG 2.2 keyboard/live regions/reduced motion; virtualized lists; thumbnail budgets | usable high-volume desktop/web UI | deferred until real UI paths exist |

## N-2C follow-up boundary (2026-07-20)

| Priority | Direction | Value | Boundary |
|---|---|---|---|
| P1 | Correlation marker and recent-message reconciliation | resolves ambiguous Telegram accepted/response-lost uploads without blind duplicate upload | not implemented in N-2C local closure |
| P1 | Mock transport crash matrix | proves delete replay, post-delete crash and token-rotation behavior without real side effects | not implemented |
| P2 | Split `postgres_upload_saga.rs` | reduces review surface and restores repository file-size convention | behavior-preserving follow-up |
| P2 | Windows ACL hardening for Saga journal | restricts local fencing/receipt metadata beyond inherited `DATA_DIR` ACL | not implemented |
| P2 | Desktop REST recovery startup parity | gives desktop-hosted API the same startup worker as Headless | not verified |