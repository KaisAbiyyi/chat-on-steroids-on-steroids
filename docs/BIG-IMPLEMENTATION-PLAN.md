# CoS-Rust — BIG implementation plan (local-first)

Decision: 2026-10-18, Asia/Jakarta. This document is the current *execution roadmap*. The original [16-phase feature/parity assessment](implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) continues to define full product scope and dependencies. The detailed [phase 1–6 closure ledger](implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md) remains historical evidence and the source of outstanding contract gaps.

Like Mind Place, this project is divided into bounded, named work units with explicit acceptance and handoff. **Unlike Mind Place, CoS-Rust is local-first:** an agent implements, tests and records checkpoints directly in `D:\projects\CoS-Rust`, followed by independent local/phase qualification and short user acceptance only when necessary. The local repository is currently the Git authority; no remote is configured. Future GitHub PR/CI support adds a review gate without moving implementation into the cloud.

## Product and acceptance principles

The objective remains a smaller, Rust-owned Chat On Steroids with the original features, security boundaries and observable behavior preserved. The frozen upstream source is commit `024731d8083d92c46e8248d64e7e0b36ab0a470f` (source app 2.1.39, bridge 14); exact differences must be consciously registered. A Rust service running on Windows is one milestone, not the finished desktop product.

Priority order: **security and privacy; durable correctness, identity and provenance; deterministic contracts and testability; measured performance and resource cost; maintainable code; product experience and documentation**. All are requirements; priority determines how to resolve a conflict. Prefer a small correct, bounded interface and a measured improvement over a large speculative rewrite. Keep native-platform differences explicit and preserve prior valid data and receipts.

Windows x64 is the implementation/qualification focus. Native Linux source/runtime checks are allowed when isolated and feasible. macOS and other targets are **HOLD** under the present user directive, not deleted from full migration scope.

## Workflow and work unit boundaries

```text
current unit + acceptance baseline
             |
             v
local agent implements on D:
             |
             v
RED/GREEN proof + native strict checks + metrics
             |
             v
independent review for risky boundaries
             |
             v
reviewable focused commit + SHA + receipts
             |
             v
additional unit in same phase, or phase qualification
             |
             +-- FAIL/BLOCKED --> isolated fix unit + recheck
             |
             v
manual user gate only for irreducible real-world behavior
             |
             v
next phase
```

One instruction such as `implement CL-02` names one coherent outcome. A unit can comprise multiple reviewed commits, but must fit inside a single agent work session without a hidden multi-hour chain of unrelated edits. If its scope grows, stop at a safe tested checkpoint, document the remainder as a separately named unit and hand off. Independent agents can review but do not silently commit over another agent's work.

Each unit has: `ID`, contract/dependencies, owned files/components, baseline and failure test, security and performance considerations, specific automatic/native/optional human qualification, evidence pointers, final SHA, and `PASS / FAIL / PARTIAL / BLOCKED / NOT RUN / HOLD` disposition. No forced user testing for a headless deterministic unit if its required native proof is already sufficient. Phase completion additionally needs an independent scope review and no unresolved critical/important safety/correctness gaps.

## Current migration map

| Original phase | Scope | Confirmed status as of 2026-10-18 | Required future gate |
| --- | --- | --- | --- |
| 1 | Frozen compatibility baseline and feasibility | PARTIAL | Remaining architectural/visual/browser feasibility evidence |
| 2 | Config/runtime/security primitives | PARTIAL | Remaining exactness and platform integration cases |
| 3 | Sessions, recording, history and usage | PARTIAL | Recorder privacy/evidence/history/retention, restart cases |
| 4 | MCP/control/connection/code mode | PARTIAL | Connection/control/protocol pressure, native clients |
| 5 | Files/projects/Git/patch/search/SFTP | PARTIAL | Outstanding source/runtime edge cases and races |
| 6 | Processes/PTYs/shell/SSH execution | PARTIAL | Remaining process/terminal and native lifecycle cases |
| 7 | Browser bridge/companion/direct tools | NOT ACCEPTED | Original bridge protocol, attribution, browser epoch and duplicate input checks |
| 8 | Embedded browser/sign-in | NOT ACCEPTED | Real browser host, profile, Chrome API compatibility and sign-in |
| 9 | Outbox/uploads/steering/finish | NOT ACCEPTED | Actual-send/receipt boundaries and upload recovery |
| 10 | Compact & Resume/recovery | NOT ACCEPTED | Crash and uncertain-receipt simulations with real integration |
| 11 | Workers/inboxes/worktrees | NOT ACCEPTED | Prime isolation, worker lifecycle and ownership proofs |
| 12 | Goal/Loop/helper decisions | NOT ACCEPTED | Rebind/restart/provider-switch and task obligation semantics |
| 13 | Plugins/Skills/OAuth | NOT ACCEPTED | Runtime auth, update/import and revocation gates |
| 14 | Native desktop/media/dictation/pets | NOT ACCEPTED | Windows capture/input/accessibility, media and lifecycle gates |
| 15 | Full desktop UI and localization | NOT ACCEPTED | Visual/interaction parity, real accessibility/large history and 13 languages |
| 16 | Migration/install/size/cross-platform release | NOT ACCEPTED | Final packaging, secure migration, regression, memory, startup and size evidence |

`NOT ACCEPTED` indicates no verified phase-level delivery; it does not assert every component is absent. The dated implementation summaries and receipts establish narrower passing contracts. A previously measured backend binary cannot satisfy phases 8, 14, 15 or 16.

## Active workstream: close phase 1–6 partials

The existing closure [ledger](implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md) groups work into eight tasks. The following **named boundaries** structure the remaining work; they are not claims that a task is completed. `CL-00` is the current governance/audit handoff; further feature units are **paused** pending the user's explicit next-unit instruction.

| Unit | Specific outcome | Prerequisite / proposed proof |
| --- | --- | --- |
| `CL-00` | Audit live checkout, protect WIP, establish local-first agent rules, Git/documentation policy and active-state ledger | Read-only status/evidence; docs-only review and SHA |
| `CL-01` | Resolve **Critical** recorder privacy: redact arguments, results and derived caption data **before** inline/asset persistence, including clipboard, environment and base64 payload handling | Resume isolated WIP only after ownership review; source privacy corpus, actual restart/assets and negative tests |
| `CL-02` | Feed structured runtime evidence, actual process exit/running state, search counts and patch changes into recorded captions | `CL-01` safe persistence; execute real nonzero/background/find/patch routes, avoid inferred success |
| `CL-03` | Finish source-compatible running/completed captions, metadata publication and cross-restart activity projection | `CL-02`; pinned caption corpus and native persist/restart proof |
| `CL-04` | Close remaining recorder/provider/history/retention/large-history faults and global unpaired-string representability decisions | `CL-03`; source differential cases, crash/rebind/resource evidence; split as contracts emerge |
| `CL-05` | Close missing execution/session custody, PTY/terminal and native SSH lifecycle contracts | Prior process receipts; native cancellation/revocation and unread output proof |
| `CL-06` | Close remaining project Git/file-panel/watcher/SFTP exactness, mutation races and accepted platform differences | Native Windows security tests and isolated SSH; split independent owners as necessary |
| `CL-07` | Close remaining search/patch/ripgrep/JS-regex/provider traversal edge cases | Pinned source oracle, real files and time/resource bounds; split if distinct paths |
| `CL-08` | Close applicable Windows startup, config, crypto, logging, diagnostics and packaging-adjacent runtime gaps | Cold start/restart and actual host behavior; Linux supplementary only |
| `CL-09` | Close connection/control/MCP/code-engine conformance and pressure tests | Real loopback transports, error/retry/revocation, no unsupported backend claims |
| `CL-10` | Complete integrated phase 1–6 qualification and independent whole-scope review | All unit evidence and open-gap register reconciled; Windows strict/native acceptance and recorded exceptions |

Do not automatically implement the next unit when a governance-only/audit instruction was given. `CL-04` onward are **planning containers**: before coding each, the agent must inventory remaining source owners and split into `CL-04A`, `CL-04B`, etc., if a container exceeds one reviewable unit. Do not count an unverified test fixture or partial source capture as a completed feature.

## Future phases 7–16: local development and selective acceptance

Each future phase begins with a **design/contract unit**, proceeds through 2–6 bounded implementation units sized to its actual components, then concludes with a distinct native qualification/review gate. Exact counts are intentionally reserved until the preceding dependencies pass: pretending to know them today would hide the unresolved embedded-browser feasibility gate.

| Phase | First unit / engineering priority | Automatic qualification | Manual gate when applicable |
| --- | --- | --- | --- |
| 7 | `7A` bridge protocol/host architecture | Simulated epoch/reconnect/attribution plus real local browser service | Signed-in provider flows as needed |
| 8 | `8A` embedded browser feasibility | Browser host/extension/debugger/native sign-in evidence | Actual account/profile flow |
| 9 | `9A` durable input custody | Crash/retry/send-ACK proof and native upload fixtures | Ambiguous Send/attachment UX |
| 10 | `10A` continuation journal/ownership | Restart-at-each-boundary, stale session/input suppression | Provider continuation where automation insufficient |
| 11 | `11A` worker run identity | Multi-prime concurrency and cross-worktree isolation | Human delegation/UX checks |
| 12 | `12A` goal/loop obligation model | Restart/turn switch/provider switch deterministic tests | Real provider-driven workflow acceptance |
| 13 | `13A` plugin/Skill runtime trust | Sandboxed import, capability revocation, OAuth/runtime fixtures | Real third-party consent flows |
| 14 | `14A` native Windows desktop adapter | Actual Windows capture/input/audio/clipboard/native resource tests | Privacy/permissions and UX |
| 15 | `15A` UI information architecture / renderer | Screenshot/interaction/accessibility/multilingual regression | Real desktop usability acceptance |
| 16 | `16A` migration/release gate | Reproducible install/update/rollback/security, size/RSS/startup benchmarks | Final full product walkthrough |

The original 16-phase dependency map is authoritative: browser capability must be proven before higher-level interactions assume it. Do not merge several security/identity/migration boundaries into a single long agent run. Portable test fixtures are allowed, but real Windows acceptance stays local and explicit.

## Quality and release gates

For every Rust implementation unit use an actual observable RED→GREEN regression, focused tests and `python tools/check-workspace.py --strict` where applicable. Qualification may require `tools/qualify-session.py`, `qualify-search.py`, `qualify-project-files.mjs`, native SSH/process tests, or relevant later GUI/browser tooling. Always run tests against isolated synthetic profiles. `cargo build --release --locked` plus runtime and binary checks is required for a unit that affects executable behavior/distribution cost; package metrics are separate from CLI size.

Security gate: no credential leakage into args/results/assets/captions, no stale authority, no unsafe filesystem escape or unclear side-effect retry. Performance gate: baseline/candidate with exact commit, fixture, units and comparable setup; actual p95/timing/RSS/size evidence if the unit changes critical latency or memory. Review gate: independent Critical/Important inspection for permissions, durability, concurrency, protocol and native boundary changes.

Record one of `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT RUN`, `HOLD` for each gate. Historical failing receipts remain available. Never claim `READY` for the full product based solely on unit tests or a headless service release.

## Handoff and Git expectations

Every named unit returns the [standard handoff](DEVELOPMENT-WORKFLOW.md#4-git-and-review) with branch/commit, exact validation and next action. Commit implementation plus relevant tests coherently; publish versioned dated plans/summaries/reviews only when their status/evidence changes. Update `CURRENT-PHASE.md` atomically with the committed outcome when possible. Conventional commits and short-lived review branches become standard; no rewriting qualified history. When a GitHub remote is attached, PR and CI must be green before merge, but the Windows local workspace remains the implementation authority.
