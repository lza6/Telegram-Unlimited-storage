# Tasks

| ID | Task | Dependency | Risk | State |
|---|---|---|---|---|
| T-01 | Close backup Critic P1 and re-review | none | L2 | in_progress |
| T-02 | Legacy tenant compatibility migration | T-01 | L3 | pending |
| T-03 | Scoped PostgreSQL adapter and transaction tests | T-02 | L3 | pending |
| T-04 | Upload dual-write | T-03 | L3 | pending |
| T-05 | Download accounting dual-write | T-03 | L3 | pending |
| T-06 | Real Bot parity acceptance | T-04,T-05 | L3/external | pending |
| T-07 | Identity/RBAC | T-06 | L3 | pending |
| T-08 | Ledger/quotas/durable jobs | T-07 | L3 | pending |
| T-09 | UI/admin/public gateway | T-08 | L2/L3 | pending |
| T-10 | Final audit/evaluation | T-09 | L3 | pending |
| T-04A | Durable pending upload Saga schema/state machine | T-03 | L3 | review |
| T-04B | REST upload Saga integration and compensation | T-04A | L3 | review |
| T-04C | Failure injection and independent review | T-04B | L3 | in_progress |
