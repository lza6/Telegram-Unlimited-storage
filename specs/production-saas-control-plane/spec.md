# Production SaaS Control Plane Specification

## Objective
Deliver a no-Docker Telegram Drive SaaS in which PostgreSQL is the tenant-aware production control plane; existing Bot storage remains the transport backend; tenant/user/admin experiences are connected to real APIs and verified against local Telegram Bot flows.

## In scope
- PostgreSQL request-path integration, legacy owner compatibility, tenant context, RBAC, sessions, API keys, audit/usage/quota, durable transfer jobs, callback outbox, gallery/preview, tenant/admin consoles, public permanent gateway links, Windows-native operation, documentation and verification.

## Out of scope
- Automatic deployment/public release; unapproved destructive migration of existing user data; exposing Telegram Bot tokens; claiming Telegram is an unlimited database without enforced product constraints.

## Key paths
1. Register/login → tenant/workspace → role-scoped session.
2. Tenant/API-key upload → admission/quota → Telegram transfer → PostgreSQL asset/ledger/audit → task/UI update.
3. Authorized download/public asset route → range stream → usage/audit.
4. Admin aggregate dashboard → audited cross-tenant scope.
5. Scheduled encrypted backup → private Telegram channel → verified restore.

## Requirements traceability
| ID | Requirement | Source | Status | Implementation target | Verification |
|---|---|---|---|---|---|
| R-01 | PostgreSQL is production control plane with forced tenant RLS | user | partial | `postgres_control_plane.rs`, migrations | scoped transaction tests |
| R-02 | Existing upload/download writes asset, usage and audit events | user | pending | API routes + adapter | real Bot comparison |
| R-03 | Tenant registration/login/memberships/RBAC | user | pending | auth routes + React | cross-tenant tests |
| R-04 | Tenant/global usage, media counts, quota and admin totals | user | pending | ledger/aggregate APIs/UI | reconciliation tests |
| R-05 | Global/tenant/bot/channel queue, retry, idempotency, resume | user | pending | durable jobs/scheduler | failure injection |
| R-06 | Gallery, previews/posters, transfers/notifications, themes/a11y | user | pending | React/API/media jobs | component + E2E |
| R-07 | Stable public permanent gateway links with range downloads | user | pending | public asset route | API/range tests |
| R-08 | API keys, callbacks/outbox, tenant audit | user | pending | API/callback services | contract tests |
| R-09 | Native Windows startup and encrypted Telegram backup/restore | user | partial | scripts/docs | real backup/restore evidence |
| R-10 | No-Docker production docs and independent review | user | partial | docs/status/reviews | evaluator report |

## Non-functional constraints
- Browser never receives Telegram, database, callback or durable API secrets.
- SQLite remains a compatibility store until dual-write evidence permits transition.
- Public assets are explicitly public; private assets remain fail-closed.
- Every mutable external operation has idempotency/correlation semantics before it contacts Telegram.
- Production claims require real test evidence, not mocks alone.