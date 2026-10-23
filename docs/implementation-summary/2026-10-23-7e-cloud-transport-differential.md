# 7E Cloud transport source-wire differential qualification (2026-10-23)

## Scope and ownership

- Unit **7E**; owner **ChatGPT GPT-6 Cloud**, branch `work/7e-transport-differential-20261023`, exclusive `browser-qualification` lock, base main `a80d504af6ace64e5d03d5723faf241d1507ed1a` (7D PR #56 accepted).
- Synthetic-only Rust 7B native signed companion-event receipt gate + JS 7C durable ledger. **Neither exercises a real WebView2 debugger, physical browser lifecycle, user credentials, provider Send or installed desktop**; those belong to 7L2/7U1.
- A shared 34-case event corpus checks exact allow/deny parity between authenticated Rust event admission and a trusted synthetic JS host adapter; Rust also checks MAC-bound receipts, no denied-message sequence advancement, deterministic 224-case authenticated bad transport mutation, replay and reconnect generation fencing. JS exercises 224 deterministic malformed ledger attempts, ambiguous persistent-store commits and cold MV3 restart with stale ACK rejection.

## Proven source gap and repair

Rust `CompanionEvent` used `Option<T>` for optional keys. Serde treats an explicit `null` in `receiptId`/`intentId`/`digest` as absent, so 7B could sign a payload that 7C strictly rejected for extra keys. Fixed by enforcing per-kind exact signed `event` keys **after** serde validation and **before** receipt issuance; negative cases `attach-extra-null`, `retire-extra-null`, `claim-extra-null`, and `repair-extra-null` protect against regression. MAC correctness alone does not authorize a widened schema.

## Validation / honest acceptance

- Required GitHub exact-head Windows/Ubuntu Rust differential CI, JS corpus, format, strict Clippy, previous 7B/7C regressions, full Rust workspace CI, and Task DAG CI. Capture exact run URLs and head SHA in PR/Notion; do not mark unseen jobs PASS.
- Self-review is by the implementor directly, covering exact diff, shared-corpus integrity, message/identity scope, authentication, ledger mutation/rollback, reconnect ambiguity and limits. The JS fixture's `verifyHostEvent` is synthetic and does **not** prove native source attestation or physical channel security.
- `7D` registry status was stale `todo` after PR #56 merge; updated to `done` to reflect independently recorded acceptance and allow 7E readiness without inventing a native receipt.
- Keep 7E **Review**, no merge/Done while required checks are pending or failing. Next native gate `7L2` remains unclaimed/unqualified; Phase 8 `8A` is blocked until 7E accepted.
