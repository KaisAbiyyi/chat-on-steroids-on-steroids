# CoSoS 15A — UI navigation and renderer IPC contract

**Claim:** ChatGPT GPT-6 Cloud / COS-15A-20261024-0028, GitHub issue #66; cloud-only.  
**Base:** `c7d4fabc28ea991a2cf9f509273cf9bb6e2ff3db` (accepted 8C PR #65).  
**Resource lock:** `ui-architecture`. **Implementation:** `crates/cos-core/src/ui_contract.rs`.

## Bounded implementation

The Rust `UiNavigation` model owns navigation state for one native-host-assigned epoch: Home, Conversation, Projects, Project, Files, Agents, Skills, Plugins, Settings and Activity. It has no capability to open files, invoke tools, read history, access browser credentials, or perform arbitrary navigation by URL. Conversation and project keys are opaque identifiers; native/provider data resolution belongs to later stages.

Renderer-side JSON commands use typed `UiRequest`, `UiCommand`, `UiRoute` and structured `UiReply` envelopes with `protocol=1`, `hostEpoch`, strict increasing `sequence` and optimistic `expected_revision` on mutations. The payload limit is 8 KiB; identifiers are ASCII, bounded to 128 bytes and reject slash, traversal, URL and markup forms. Unknown commands/fields/variants and duplicate JSON fields are rejected. No mutation occurs on invalid IDs, stale epochs/revisions or parser failures. A well-formed but refused command consumes its sequence, preventing later replay; successful navigation updates state only when it differs. Rebind requires a strictly higher host-owned epoch and returns the route to Home to avoid resurrecting a previous session.

**Important security boundary:** epochs and sequence counters are **fences, not authentication**. Native host/renderer origin and identity attestation must happen *before* calling `dispatch_json`. A hostile renderer that can directly access this API must not gain filesystem, tools, tab permissions, browser credentials or other privileged capabilities. Host consent and 7B/8C browser authority are separate contracts; this module never grants either.

## Acceptance / handoff

- Regression: `cargo test -p cos-core --test ui_navigation_ipc --locked` covers navigation revision/idempotence, stale/replay/order, rebind, route spoofing/path injections, malformed/oversized frames, protocol/epoch/JS-integer boundaries.
- Global CI: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --release --locked` on Windows and Ubuntu, plus Task DAG validation.
- Direct implementor self-review: verify exact diff and head, public-fork fixture safety, privileges, overflow, error determinism and compatibility.
- Not attempted: actual desktop window/nav renderer, UI screenshots, native IPC plumbing, source screenshot parity, user accessibility/keyboard acceptance; these belong to 15B–15G and 15L/15U gates. Hosted CI is not proof of device behavior.
- Do not promote this unit to Done until the PR is reviewed, exact-head checks succeed and merge receipt is recorded. Keep 15B/15C/15D/15E blocked until accepted 15A.

## Consumer contract

```json
{"protocol":1,"hostEpoch":4,"sequence":1,"command":{"action":"navigate","expected_revision":1,"route":{"view":"conversation","session_id":"synthetic-01"}}}
```

Success is `{"protocol":1,"hostEpoch":4,"sequence":1,"state":{...}}` with the resulting route and revision. Error messages are fixed `UiError::code()` values; adapters should return a non-sensitive structured error with correlation to their already-attested transport rather than echo untrusted payloads.
