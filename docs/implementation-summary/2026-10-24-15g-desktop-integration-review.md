# 15G — Full desktop integration / bounded performance review (cloud)

Claim: ChatGPT GPT-6 Cloud — COS-15G-20261024-1157, GitHub issue #80. Base main: `b4169442e661819cc50622b091fd697d8574efb7` (15F PR #79). Resource lock: `ui-integration`.

## Implementation and integration boundary

- `cos_core::ui_integration::DesktopAudit::inspect` creates an ephemeral, **privacy-minimized**, read-only receipt across 15A navigation, 15B history, 15C rich content, 15D panels, and 15E focus, reusing the 15F snapshot projection. Only the authenticated host may invoke this diagnostic on source-owned snapshots.
- Rejects mixed epoch/revision, asynchronous history-generation mismatch, impossible pending/session state, unsynchronized panel-route pairings on panel-owned routes, and oversize aggregate presentations. 192 KiB is a *serialized diagnostic-state budget*, not process RSS.
- Reports visible conversation rows only while on the matching conversation route; background/hidden history rows are not reported as visible. Audit does **not** erase hidden session data: the privileged host must manage session lifecycle and authorization.
- The diagnostic serializes bounded snapshots in memory only to count bytes. It never returns those bytes, writes logs or drives an actual renderer. No renderer-supplied action is accepted by this new API.

## Cloud regression and performance protocol

`cargo test -p cos-core --test ui_desktop_integration --locked` verifies panel-route convergence, stale asynchronous page and window-rebind fencing, modal/rich/history presentation, sanitized receipts, explicit upper limits and 156 fixed DPI × theme × locale × viewport **metadata** combinations.

The synthetic large-history scenario uses 24 visible rows (100,000 logical total), 240 synthetic catalog items and four rich code panels. CI asserts the retained serialized state stays below 192 KiB. A repeated 256-iteration diagnostic prints non-gating elapsed time and exact bytes to the hosted test log. This does **not** measure user experience, native WebView, CPU utilization, app start time, physical RAM, scroll frame time, GPU resources or battery. Do not use hosted runner wall-clock as product FPS/RSS pass/fail.

Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, and `cargo build --release --locked` on hosted **Windows and Ubuntu** at the final PR head SHA; plus Task DAG CI. Read the exact diff, test/security surfaces and dependencies in a direct implementor self-review, then record receipts on PR + Notion. Any FAIL/NOT_RUN remains explicit.

## Separate acceptance gates

15L1 native rich host; 15L2 physical 13-locale, DPI, theme and interaction captures; 15L3 real large-history RAM/performance; 15U1/15U2/15U3 owner navigation, localization/accessibility, complete workflow experience. Cloud 15G cannot mark any of these PASS; it only qualifies the bounded Rust UI integration contract and unlocks separately scheduled gates after PR acceptance.
