# Telegram Storage Production Limits and Queue Policy

> Reviewed on 2026-07-19. This is an implementation constraint, not marketing copy.
>
> Authoritative sources: Telegram Bot API [`getFile` / `File`](https://core.telegram.org/bots/api#file) documentation; upstream [Local Bot API server documentation](https://github.com/tdlib/telegram-bot-api#local-bot-api-server). Field observation: upstream GitHub [issue #755](https://github.com/tdlib/telegram-bot-api/issues/755), opened on 2025-06-20, reports MTProto `-429` and timeout symptoms at approximately 15-20 `getFile` RPS with 15+ workers. It is not an official rate contract.

## Hard product boundary

The public Telegram Bot API is not an unlimited object-storage transport:

- Official API upload is limited to 50 MB and `getFile` download to 20 MB.
- A self-hosted Local Bot API server in local mode raises upload to 2000 MB and removes the download size limit, but it itself is HTTP-only and needs TLS termination when exposed remotely.
- A Bot cannot use Telegram Premium, so it must not promise uploads above 2000 MB even when Telegram user accounts can use larger limits.

Therefore this product must advertise and enforce an explicit per-file limit based on the active transport, never an unqualified claim of unlimited per-file uploads.

## Queue hierarchy

1. `tenant admission`: storage quota, monthly transfer quota, active-job quota, idempotency and tenant fairness.
2. `global admission`: process-wide CPU/disk/network and database/outbox health.
3. `bot admission`: one adaptive limiter per bot token; a second bot token adds a distinct lane, not an exemption from every Telegram limit.
4. `chat admission`: storage channel sends are serialized by channel/bot lane. Start at one media send per second per storage chat and increase only with evidence.
5. `method admission`: separate send-media, metadata, `getFile`, and download-stream buckets.

## Initial safe concurrency configuration

These are conservative startup values, not Telegram-guaranteed limits:

| Lane | Initial policy | Adaptive response |
|---|---:|---|
| storage channel send media | 1 in-flight per bot/channel | on 429/FloodWait, suspend lane until retry_after then halve burst |
| bot upload workers | 2 files per bot | additive increase after 100 clean jobs; multiplicative decrease on transient limit |
| bot metadata calls | 8 RPS per bot | token bucket, retry_after wins |
| Bot API `getFile` / media download | 6 concurrent per bot | worker queue; hard backoff on 429/timeout |
| tenant uploads | plan-configured, default 2 | reject over quota with Retry-After, never silently drop |
| tenant downloads | plan-configured, default 4 streams | authorization before byte reservation |
| global disk staging | bounded bytes + age TTL | fail closed before disk exhaustion |

A 2025 upstream issue reported MTProto 429 and timeout symptoms around 15–20 `getFile` requests/s even with a Local Bot API server; this is field evidence, not a contractual Telegram rate. We will begin materially below it and tune only from observed queues and errors.

## Retry rules

- Persist `transfer_job` and idempotency state before contacting Telegram.
- Respect explicit `retry_after` / FloodWait exactly; it overrides exponential scheduling.
- Retry only timeouts, connection resets, 5xx, and classified temporary Telegram errors.
- Do not retry auth, permission, validation, file-too-large, quota, or unsupported-media errors automatically.
- Persist attempt count, next attempt, error code, correlation ID, and final outcome.
- Upload input must be replayable from staged chunks; never retry a consumed non-seekable stream.

## Download and preview policy

- Authorize and reserve download bytes before cache/range response.
- Cache only encrypted/tenant-scoped derivatives or public-share derivatives with independently revocable keys.
- For videos, index Telegram metadata first; generate thumbnail/poster and duration/codec metadata in a durable media-index job.
- Use range streaming where transport supports it. If the active official Bot API cannot retrieve an object over its download limit, present an actionable transport-limit state rather than a broken player.
- Resume uses a stable asset/version, byte range, hash, and authenticated short-lived ticket. Resume never accepts a raw Telegram message ID as authority.

## Operations dashboard requirements

Show queue depth, oldest job age, active workers, FloodWait remaining, retry rate, p50/p95 upload/download speed, per-bot lane saturation, per-tenant quota rejections, staging disk usage, callback delivery backlog, and transport-specific file-limit failures.

## Production deployment decision

- Small files only: official Bot API can remain the baseline.
- Media SaaS with files over 50 MB or direct downloads over 20 MB: Local Bot API plus a TLS reverse proxy is mandatory, or the product must route large objects through a separately authorized MTProto/user transport.
- Multi-bot improves lane capacity but does not eliminate the storage-channel and Telegram-side limits; per-bot metrics and queue fairness remain mandatory.

## 2026-07-20 source refresh

- Public Bot API `sendDocument` remains capped at 50 MB and `getFile` at 20 MB. The returned `file_path` URL is temporary and is not a permanent public object URL.
- Local Bot API local mode documents uploads up to 2000 MB, unlimited download size, local-path uploads, and HTTP-only service exposure; remote use therefore still requires a TLS-terminating proxy.
- Official Bot API does not expose byte-offset/limit parameters for `getFile`; application HTTP Range must use MTProto `upload.getFile(offset, limit)` or a project-controlled cache/staging layer.
- `max_webhook_connections` controls inbound update delivery only. It is not an outbound upload/download concurrency guarantee.
- Bot API 429 `retry_after` and MTProto `FLOOD_WAIT_X` are shared scheduler state. Sleeping only one worker allows other workers or replicas to continue hitting the same lane.
- The pinned Grammers repository was archived on 2026-02-10. Protocol limits and error handling must therefore be maintained from Telegram primary documentation and runtime evidence, not assumed to receive upstream fixes.

Primary references: [Bot API](https://core.telegram.org/bots/api), [Bot FAQ](https://core.telegram.org/bots/faq), [MTProto file API](https://core.telegram.org/api/files), [RPC errors](https://core.telegram.org/api/errors), [Local Bot API server](https://github.com/tdlib/telegram-bot-api), [Bot API Range decision](https://github.com/tdlib/telegram-bot-api/issues/141), [2 GB bot boundary](https://github.com/tdlib/telegram-bot-api/issues/583), [Telethon repository](https://github.com/LonamiWebs/Telethon).
