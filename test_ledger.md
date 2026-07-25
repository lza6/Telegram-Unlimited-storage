# Test Ledger

## Seam Registry
| Seam | Module A | Module B | Contract | Status | Evidence |
|---|---|---|---|---|---|
| S-02 | API upload | PostgreSQL control plane | scoped asset/ledger/audit transaction | pending | none |
| S-03 | API download | PostgreSQL control plane | scoped usage/audit transaction | pending | none |
| S-04 | native backup | Telegram Bot API | encrypted state and rollback | review | pending Critic re-review |

## Test Status
| Node | Test type | Status | Notes |
|---|---|---|---|
| N-1 | PostgreSQL RLS integration | passed | native denial test |
| N-2 | dual write | pending | not implemented |
| N-7 | backup/restore | review | Critic re-review pending |

| S-05 | legacy owner key | PostgreSQL tenant/RLS | resolver returns canonical tenant UUID before SET LOCAL | 2026-07-20 | locked | `evidence/N-2A-tenant-resolver.json` |
| S-06 | upload transport | PostgreSQL asset identity | receipt carries actual peer/message identity; Saga hook still disconnected | 2026-07-20 | pending | `evidence/N-2D-upload-receipt.json` |

| N-2A | tenant resolver/RLS | passed | 2026-07-20 | migration 004 + runtime resolver | existing different UUID mapping reused; cross-tenant=0 |
| N-2B | runtime role/TLS boundary | passed | 2026-07-20 | checked connection | loopback NoTls only; unsafe role flags rejected |
| N-2D | upload receipt | passed (unit/build) | 2026-07-20 | Bot/User receipt contract | real Telegram receipt parity not executed yet |
| N-2C | durable upload Saga | pending | — | not implemented |

| S-07 | REST upload | PostgreSQL upload Saga | idempotency, fenced receipt/finalize and compensation journal | 2026-07-20 | review | `evidence/N-2C-upload-saga.json` |
| S-08 | recovery journal | PostgreSQL/Telegram compensation | node-bound replay with fail-closed delete | 2026-07-20 | review | real Telegram delete not executed |
| N-2C | durable upload Saga | passed locally / review | 2026-07-20 | migrations 005-009 + REST + recovery worker | 3 PostgreSQL tests, 13 API tests; real Telegram and crash injection remain open |
| S-09 | Bot upload receipt | Telegram compensation delete | delete target is receipt peer, not current channel config | 2026-07-20 | review | `evidence/N-2C-bot-compensation-peer.json` |
| N-2C-P0-BOT-PEER | targeted unit/build/fmt | passed locally / review | 2026-07-20 | 1 targeted + 7 transport tests; Headless check/fmt | real Telegram delete not executed; peer-scoped bot map pending |
| S-10 | Telegram accepted receipt | PostgreSQL journal/finalize then SQLite projections | local projection failure must not retry sendDocument | 2026-07-20 | review | vidence/N-2C-receipt-first.json |
| S-11 | persisted Bot receipt | compensation token/peer resolver | stable uploader identity, no Bot0 fallback, strict purge | 2026-07-20 | review | 10 transport + 6 file access tests; real Telegram unverified |
| N-2C-RECEIPT-FIRST | Rust unit/build | passed locally / review | 2026-07-20 | 168 passed, 4 ignored; Headless check passed | independent Critic pending |
