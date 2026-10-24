# CoSoS 15B — Conversation presentation and bounded large history

**Claim:** ChatGPT GPT-6 Cloud / COS-15B-20261024-1031, [GitHub issue #76](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/76).
**Base:** 5a97ae9949d8c58ee4bdbacd376ae6910638d199.
**Branch:** work/15b-ui-conversation-20261024. **Lock:** ui-conversation.
**Prerequisite:** 15A merged and accepted PR #67, eb9f3dc9684fe97017dcc3a57ae63553d2118547.

## Bounded contract
Rust module: crates/cos-core/src/ui_conversation.rs, with synthetic tests in crates/cos-core/tests/ui_conversation_history.rs.

This is a presentation-only model, NOT a native UI. The privileged host alone authorizes session access through the existing store, projects trusted store chronology into stable contiguous display positions, and publishes safe plaintext records to an independently authenticated renderer transport. The renderer can request only older/newer page navigation; it cannot select a session, read files, grant permissions, execute tools, or render arbitrary HTML.

At most one page and 24 entries (2 KiB plaintext each, 64 KiB serialized page) are retained. Host-only session switch immediately erases prior visible history and issues a new generation-bound initial load ticket. Each async completion must match the exact pending ticket and session ID, page totals and chronological adjacency. Malformed, duplicate-identity, noncontiguous, stale, oversized and mismatched pages are rejected without changing the visible page. Host load failure keeps existing content but clears pending state for explicit retry. Rebind to a greater host epoch erases session content and pending tickets. A changed history total must be refreshed as a new session/window generation rather than risking chronology mixups.

Strict JSON renderer IPC accepts only snapshot/load-older/load-newer, fenced by protocol=1, epoch, sequence and expected revision. Duplicate fields, unknown fields/commands, stale or replayed frames and requests over 8 KiB fail closed. Valid rejected commands consume sequence. Epoch and ticket fields are correlation fences, NOT authentication; the native host must authorize both actual history reads and publication. UI adapters MUST use text nodes, never evaluate untrusted message HTML/JavaScript or treat links as capabilities, and never leak raw backend exceptions.

## Tests / acceptance
Targeted: cargo test -p cos-core --test ui_conversation_history --locked.
Cases: latest/older/newer; 1,000 page windows representing 24,000 synthetic rows; stale async/cross-session/rebind; duplicate ID/gaps/total/adjacency; injection/control/BiDi/size limits; malformed/duplicate-field commands and replay; empty history and counter boundaries.

Hosted Windows and Ubuntu workspace fmt, strict Clippy, tests, release builds, plus task DAG checks must be PASS on the exact PR head. Direct implementor self-review reviews exact diff, security, pagination, allocation bounds, regressions and limits. Leave Review if CI pending or failing. No personal data is read or staged.

## Known limits
This change does not implement a native WebView renderer, 24k real-item latency measurement, persistent scroll-anchor behavior, cross-page cache, provider session persistence or screenshot/keyboard/locale parity. Adapter integration must preserve source-store chronology and validate ownership at the privileged read boundary. Native Windows/owner proof remains a separate gate under 15F/15L/15U.

**Next:** exact-head CI and direct self-review, then accepted merge and synchronized Notion/GitHub. 15F requires all 15B/15C/15D/15E acceptance.
