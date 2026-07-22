# SaaS Gateway, Reliability, and Portal Contract

## Non-negotiable boundary

Browser / desktop client -> SaaS Gateway -> Control Plane and Telegram storage.

Clients never receive the Bot token, Telegram API credentials, database credentials, or a long-lived tenant secret in a URL. The gateway resolves tenant context before every storage action and records an immutable request/usage/audit fact.

## Gateway contract

| Concern | Contract |
|---|---|
| Browser auth | HttpOnly session + CSRF; tenant and role resolved server-side |
| Programmatic API | hashed scoped API key, tenant bound, rotation/revocation, rate limit |
| Mutations | `Idempotency-Key` required for create/upload-complete/share/callback changes |
| Retries | bounded exponential backoff with jitter; retry only classified transient faults |
| Long operations | durable job record with status query and SSE/WebSocket progress; retry/cancel is explicit |
| Storage calls | request correlation ID, tenant authorization, queue admission, ledger events |
| Callbacks | allowlisted HTTPS endpoint, HMAC timestamp signature, durable outbox, retry/dead-letter view |
| Downloads | authorize before cache/range transfer; count byte events and preserve tenant scope |

## Network resilience

1. Uploads use resumable sessions and chunk hashes. A lost client may query the session and continue only its own pending chunks.
2. Frontend retries read operations only when safe; user-visible mutations use one idempotency key across retries.
3. Gateway retries Telegram transient failures only after persistence of the job/outbox fact. FloodWait, quota, auth failures, and validation failures are not blind-retried.
4. Every retry sequence exposes attempt count, next retry time, final cause, and correlation ID to the tenant and system admin scopes.

## Portal surfaces

### Public portal

A marketing/status landing page may show only aggregated, non-sensitive metrics:

- platform uptime and service health category
- total stored bytes rounded to safe units
- total object / image / video counts rounded or delayed
- registered tenant count only when policy permits
- current status incidents and release notes

It never exposes tenant names, individual usage, filenames, exact live capacity, private media, or credentials.

### Tenant SaaS console

Overview, gallery, transfers, usage, API/callback operations, members, audit and themed settings.

### System admin console

Platform aggregates, tenant/user operations, quotas, queues, callback delivery, infrastructure health, audited impersonation support, and drill-down protected by system admin role.

## Visual system

One token-based design system, not duplicated page CSS:

- themes: Midnight, Aurora, Light, high-contrast
- persisted user preference with OS preference fallback
- chart palette and status semantic tokens shared by portal, tenant console and admin console
- keyboard, screen reader and reduced-motion behavior are acceptance requirements

## Required evidence

- cross-tenant API read/write/delete/share denial
- duplicate request and network retry idempotency
- paused/resumed upload and transient Telegram retry
- signed callback delivery / retry / dead-letter
- real image, video and general file bot uploads, metadata classification, gallery rendering, range download and SHA-256 match
- Playwright desktop/web visual and accessibility checks across themes and narrow widths
- PostgreSQL single and two-instance gateway tests, with Redis outage and queue recovery evidence
