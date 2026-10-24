# CoSoS 15C — rich panel state contract (cloud-only)

Claim: ChatGPT GPT-6 Cloud / COS-15C-20261024-0958; GitHub #68; lock `ui-rich`.
Prerequisite: accepted 15A PR #67, merged main `eb9f3dc9684fe97017dcc3a57ae63553d2118547`.

## Bounded delivery

`cos_core::ui_rich::RichDeck` is a host-owned, inert representation for five UI surfaces:

| Surface | Host authority | Renderer-allowed change |
| --- | --- | --- |
| Editor | Publish document identity, title, syntax hint, initial text and read-only flag | Replace ephemeral text only when not read-only |
| Terminal | Publish bounded display output (stripped of unsafe controls upstream) | Activate/view only; **cannot execute shell commands or inject output** |
| Code | Publish language-tagged read-only source/diff text | Activate/view only |
| PDF | Publish opaque asset handle, bounded page count and current page | Change page inside host-declared bounds; **cannot fetch or open paths/URLs** |
| Math | Publish inert source string for later native renderer | Replace bounded expression source; **does not evaluate HTML/scripts** |

State includes an active panel, monotonic revision, host epoch, and bounded panel vector. The renderer receives only a deliberately small discriminated request union: snapshot, activate, replace editor text, replace math source, or change PDF page. There is no renderer command for file write, arbitrary asset loading, browser navigation, terminal input, process launch, capabilities, or host-side publish/remove.

Input is strict JSON with unknown fields/actions denied, one sequence per epoch and atomic expected-revision checks. Accepted commands advance the sequence even when refused, so rejected mutations cannot later replay. A host rebind requires a strictly higher epoch and clears private panel state. The native transport **must separately authenticate its sender**; epoch and sequence are not credentials. Host publish/remove is only exposed as direct Rust methods and must never be bound to untrusted renderer IPC.

Hard limits: inbound frame 16 KiB; per-text 8 KiB; serialized snapshot 32 KiB; up to 8 panels; opaque IDs 128 ASCII bytes; title 256 bytes; PDF up to 100,000 pages; JavaScript-safe integer sequence/epoch/revision. Control characters (other than CR/LF/tab) and Unicode bidi overrides/isolate marks are refused. Panel IDs are **keys, never paths**. A renderer must render all strings as escaped text, and separately apply a trustworthy math/PDF engine; no HTML execution is conferred by this model.

## Regression coverage

`crates/cos-core/tests/ui_rich_panels.rs` exercises all five surfaces, editable versus immutable authority, invalid/unknown messages, unsafe IDs/text, oversized payload/snapshot, capacity and PDF bounds, stale revisions/epochs, sequence replay, rebind state reset and rejection atomicity.

Cloud acceptance: exact-head Windows and Ubuntu fmt / Clippy / tests / release / DAG via GitHub Actions, plus implementor direct self-review of the exact SHA/diff and overlaps. **No tests were run locally or against real user data.** CI results are not to be called PASS until GitHub reports success for the exact candidate SHA.

## Deliberate exclusions / handoff

This unit provides Rust domain-state and IPC contracts, **not** a native editor, terminal emulator, PDF viewer, math renderer, code highlighter or full CoS desktop parity. Terminal PTY wiring, trusted file I/O, PDF byte transfer, math typesetting, editor cursor/undo semantics, screen-reader support, copied text policies, GPU/DPI/theming and physical WebView2 integration remain unproven.

The separate `15L1` physical native gate must exercise real packaged Windows embedded renderer behavior. `15F` fixed-DPI/theme visual regression and later Phase 15 integration remain distinct; do not treat cloud CI as their acceptance.
