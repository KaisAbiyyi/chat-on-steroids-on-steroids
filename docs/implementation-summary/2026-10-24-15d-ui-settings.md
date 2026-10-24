# CoSoS 15D — Projects, files, agents, skills/plugins and settings panels

**Claim:** ChatGPT GPT-6 Cloud / COS-15D-20261024-1000; [issue #69](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/69).  
**Base:** `eb9f3dc9684fe97017dcc3a57ae63553d2118547` (15A merged PR #67).  
**Lock:** `ui-settings`. **Implementation:** `crates/cos-core/src/ui_panels.rs`.

## Implementation scope

The 15D module provides a strict, source-owned **presentation contract** for the Projects, Files, Agents, Skills, Plugins, Settings and Tools panels. Hosts populate bounded typed catalogs containing project identities/names, project-relative entries, available agents, extension kind/status and **nonsecret allowlisted** appearance/language/tool-safety preferences. A renderer can request snapshots or select/open existing UI entries; it cannot write preferences, mutate project records, read paths, invoke tools, install plugins, authorize OAuth, or execute agents. These remain existing privileged backend/consent responsibilities.

The model uses protocol version 1 aligned with 15A, a native-host-assigned epoch, increasing sequence, state revision, idempotent UI operations, a 16-KiB frame cap and 64-KiB/256-item catalog caps. JSON rejects unknown/duplicate fields and unknown command variants. Catalog inserts are validated atomically for duplicate IDs, orphan files, traversal-like paths, unsafe display strings and bounded preferences. Selected IDs are checked against the current catalog (not trusted capabilities). A rejected well-formed command consumes its sequence to prevent replay; epoch rebind retires old renderer state and **clears potentially sensitive catalog metadata**.

## Contract examples

Renderer selection (only after a separately authenticated native transport and host catalog):
```json
{"protocol":1,"hostEpoch":1,"sequence":1,"command":{"action":"select-project","project_id":"synthetic-01","expected_revision":2}}
```

Server emits `PanelReply` with `state` and incremented `revision`. `PanelError::code()` is stable and avoids reflecting raw inputs.

## Self-review and acceptance

- Verify `ui_panels.rs`, `lib.rs` and this note against the exact PR HEAD, including possible conflict with parallel 15B/C/E. Scope lock is only `ui-settings`.
- Unit tests cover catalog selection, idempotence, missing identifiers, stale revision, request replay, epoch retirement, malformed/oversize frames, unknown command/fields, traversal/drive-path input, duplicate/orphan catalog items, unsafe Unicode labels, bounded preferences and overflow.
- Hosted acceptance requires both Windows and Ubuntu `cargo fmt --all --check`, strict Clippy, workspace tests, release build, and Task DAG CI at the **exact HEAD**. Mark NOT_RUN while checks are pending; do not invent native PASS.
- 15D ends at testable **cloud contract**; full host/renderer wiring, real project-folder permissions, installed plugin/agent UI, user settings persistence, pixel-level screenshot parity and native/owner UX are NOT_RUN and remain future 15F/15G and local/owner gates. All filesystem/tool/plugin/OAuth authority stays behind existing host security checks.
- 15F unlocks only when **15B + 15C + 15D + 15E** all have accepted merges.
