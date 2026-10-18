# CoS-Rust — BIG implementation plan (cloud-first)

Direction updated: 2026-10-18, Asia/Jakarta. This document is the current *execution roadmap*. The original [16-phase feature/parity assessment](implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) continues to define full product scope and dependencies. The [phase 1–6 closure ledger](implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md) remains historical evidence. Detailed cloud unit names, local gates and manual testing are in the [unit and acceptance matrix](implementation-plan/2026-10-18-cloud-first-units.md).

**CoS-Rust is now cloud-first, following Mind Place's development flow.** Cloud ChatGPT/Codex agents implement directly against [the designated GitHub repository](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids) once its first reviewed bootstrap is published. Local Windows agents perform the device/runtime qualification named by a gate; the user participates only for account consent, irreversible choices, or subjective acceptance. The original local history and current dirty work remain protected until their provenance is reviewed. A configured remote alone does not make an empty repository the Git source of truth.

## Product and acceptance principles

The objective remains a smaller, Rust-owned Chat On Steroids with the original features, security boundaries and observable behavior preserved. The frozen upstream source is commit `024731d8083d92c46e8248d64e7e0b36ab0a470f` (source app 2.1.39, bridge 14); exact differences must be consciously registered. A Rust service running on Windows is one milestone, not the finished desktop product.

Priority order: **security and privacy; durable correctness, identity and provenance; deterministic contracts and testability; measured performance and resource cost; maintainable code; product experience and documentation**. All are requirements; priority determines how to resolve a conflict. Prefer a small correct, bounded interface and a measured improvement over a large speculative rewrite. Keep native-platform differences explicit and preserve prior valid data and receipts.

Windows x64 is the **primary shipped and native-qualification focus**, including cloud Windows CI. Linux cloud CI is useful for cross-platform Rust code. Linux Docker on the user's device requires proof of D: backing storage; macOS and other targets are **HOLD** under the present user directive, not deleted from full migration scope.

## Cloud development and handoff

```text
reviewed GitHub bootstrap -> named cloud unit / topic branch
       -> implement + RED/GREEN + Windows/Linux CI + security review
       -> PR + merged SHA + complete cloud handoff
       -> repeat cloud units, with early native feasibility gates as named
       -> phase cloud-complete -> handoff exact SHA to user
       -> user asks local agent to pull exact SHA on D:
       -> local native qualification L (PASS / FAIL / BLOCKED)
       -> FAIL: cloud fix unit -> new SHA -> local retest
       -> PASS: user acceptance U only where explicitly assigned
       -> next phase
```

One instruction such as `implement CL-02` or `implement phase 7C` names a coherent cloud outcome. Units should fit within one agent run, even when they need multiple commits. If underestimated, complete a reviewable portion and split the remainder into another named cloud unit. CI-green changes merge to GitHub before the next dependent unit; native fixes go back to cloud rather than turning the local test checkout into the implementation source. If a unit must be tested on a real Windows device before proceeding (e.g. browser host feasibility), its named `L` gate runs **immediately after the relevant cloud unit**, before dependent code is built.

Every unit has `ID`, scope/owners, dependency, measurable exit condition, cloud checks, review and exact SHA, local gate if necessary, and explicit **YOU REQUIRED: NO/YES (action and moment)**. Local qualification has its own evidence/commit identity and device/runtime context. `PASS / FAIL / PARTIAL / BLOCKED / NOT RUN / HOLD` are statuses, never promises. Phase acceptance requires all mandatory unit reviews, native gates and user gates; historical receipts prove only the exact older scope.

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

## GitHub bootstrap and source authority

`origin` points to `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids.git`. The target GitHub repository is **public and has no branches or commits** at the last check. Before any feature work, `BOOT-L1` must inspect historical Git objects, artifacts, permissions, accepted branch ancestry and the clean publication set; `BOOT-U1` is the user's review of the public GitHub publication. The published default `main` **must contain all approved previous feature history**, not just the old local baseline `main` commit; verify the approved tip is an ancestor of the published main SHA. Only then push the reviewed history and verify GitHub CI. This initial transfer is local preparation for cloud-first work; later feature implementations live on GitHub.

The previous local branch is 20+ commits ahead of its baseline `main` and has eight uncommitted recorder/caption paths. None is automatically available to a cloud agent. Preserve the dirty work and its captured public-fixture contract; transfer only a checked, safe patch/fixture or reconstructed implementation once `CL-01` starts. Do not push unexamined private or secret-bearing Git history merely because `.gitignore` looks clean.

## Active workstream: close phase 1–6 partials

The existing closure [ledger](implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md) groups work into eight tasks. `CL-00` is historical governance; it is excluded from future cloud-unit counts. `CL-01` through `CL-10` are the remaining closure workstream; complex owner groups have now been split into **18 separate cloud work units** with exact names in the [unit matrix](implementation-plan/2026-10-18-cloud-first-units.md).

| Unit | Specific outcome | Prerequisite / proposed proof |
| --- | --- | --- |
| `CL-00` | Historical audit and governance baseline; superseded by the cloud-first decision | Committed locally, documentation only |
| `CL-01` | Resolve **Critical** recorder privacy: redact arguments, results and derived caption data **before** inline/asset persistence, including clipboard, environment and base64 payload handling | Resume isolated WIP only after ownership review; source privacy corpus, actual restart/assets and negative tests |
| `CL-02` | Feed structured runtime evidence, actual process exit/running state, search counts and patch changes into recorded captions | `CL-01` safe persistence; execute real nonzero/background/find/patch routes, avoid inferred success |
| `CL-03` | Finish source-compatible running/completed captions, metadata publication and cross-restart activity projection | `CL-02`; pinned caption corpus and native persist/restart proof |
| `CL-04` | Close remaining recorder/provider/history/retention/large-history faults and global unpaired-string representability decisions | `CL-03`; source differential cases, crash/rebind/resource evidence; split as contracts emerge |
| `CL-05` | Close missing execution/session custody, PTY/terminal and native SSH lifecycle contracts | Prior process receipts; native cancellation/revocation and unread output proof |
| `CL-06` | Close remaining project Git/file-panel/watcher/SFTP exactness, mutation races and accepted platform differences | Native Windows security tests and isolated SSH; split independent owners as necessary |
| `CL-07` | Close remaining search/patch/ripgrep/JS-regex/provider traversal edge cases | Pinned source oracle, real files and time/resource bounds; split if distinct paths |
| `CL-08` | Close applicable Windows startup, config, crypto, logging, diagnostics and packaging-adjacent runtime gaps | Cold start/restart and actual host behavior; Linux supplementary only |
| `CL-09` | Close connection/control/MCP/code-engine conformance and pressure tests | Real loopback transports, error/retry/revocation, no unsupported backend claims |
| `CL-10` | Complete integrated **Rust service/backend** closure qualification and independent review of the implemented phase 1–6 owners | Source contracts, Windows/CI proof and remaining full-product/architecture dependencies recorded without claiming desktop/browser completion |

`CL-04`–`CL-09` are category labels; actual cloud instructions target `CL-04A` etc. Each cloud unit requires a PR, CI and distinct handoff; native Windows proof uses the six specifically scheduled `CL-L1`–`CL-L6` gates. Their completion proves the **backend closure scope only**. The original Phase 1 embedded-browser/renderer feasibility decision and full phase acceptance remain open for subsequent early browser/UI gates; neither full desktop nor original Phase 1 can be called DONE solely from `CL-10`.

## Future phases 7–16: cloud implementation, local acceptance

Each phase begins with a small design/contract cloud unit, continues through 3–6 bounded cloud units and ends with named acceptance gates. The [detailed matrix](implementation-plan/2026-10-18-cloud-first-units.md) lists every cloud unit, local session and human action. The original Phase 1 demands browser/extension/debugger and rich UI toolkit feasibility **before** a long browser rewrite. Therefore `8A` + `8B` run as an early cloud feasibility spike **after `CL-L6` and before Phase 7**, followed immediately by local Windows `8L1`. If `8L1` fails, do not start Phase 7; revisit the architecture. Only full Phase 8 implementation (`8C` onward) waits on Phase 7. These early units are counted once in Phase 8 C/L totals.

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

The original 16-phase dependency map is authoritative: browser capability must be proven before higher-level interactions assume it. Do not merge several security/identity/migration boundaries into a single long agent run. Cloud Windows CI establishes isolated native correctness; full installed browser/desktop/user proof remains at explicit local gates.

### Planned workload and involvement

| Scope | Cloud units (C) | Local agent sessions (L) | Direct user sessions (U) |
| --- | ---: | ---: | ---: |
| GitHub bootstrap | 0 | 1 | 1 |
| Remaining phases 1–6 `CL` | 18 | 6 | 0 |
| Phase 7 — bridge | 5 | 2 | 1 |
| Phase 8 — embedded browser | 5 | 2 | 1 |
| Phase 9 — outbox/uploads | 5 | 2 | 1 |
| Phase 10 — Compact & Resume | 5 | 2 | 1 |
| Phase 11 — workers | 5 | 1 | 1 |
| Phase 12 — Goal/Loop | 4 | 2 | 1 |
| Phase 13 — plugins/Skills/OAuth | 6 | 2 | 2 |
| Phase 14 — native desktop | 6 | 3 | 2 |
| Phase 15 — desktop UI | 7 | 3 | 3 |
| Phase 16 — migration/release | 6 | 3 | 2 |
| **Remaining development** | **72** | **28** | **15** |
| **With bootstrap** | **72** | **29** | **16** |

`C` means a GitHub/CI cloud agent run, `L` means a local Windows agent session (the user triggers the handoff; the agent executes tests), and `U` means **the user personally needs to act**. These are base handoff counts; failure/retry units are separate. `U0` for the backend closure means no manual UX/account test is planned. Each cloud handoff tells the user explicitly whether to do anything **right now**, later at the phase local gate, or not at all.

## Quality and release gates

For every cloud Rust implementation unit use an actual observable RED→GREEN regression, focused tests, `cargo fmt --all --check`, strict Clippy and appropriate Windows/Linux GitHub Actions CI. Local qualification uses `python tools/check-workspace.py --strict`, `tools/qualify-session.py`, `qualify-search.py`, native SSH/process tests or GUI/browser scripts **only when named by an L gate**. Always run with isolated synthetic profiles. A release-affecting unit records executable size/runtime cost, and the later local release gate verifies actual packaged size separately.

Security gate: no credential leakage into args/results/assets/captions, no stale authority, no unsafe filesystem escape or unclear side-effect retry. Performance gate: baseline/candidate with exact commit, fixture, units and comparable setup; actual p95/timing/RSS/size evidence if the unit changes critical latency or memory. Review gate: independent Critical/Important inspection for permissions, durability, concurrency, protocol and native boundary changes.

Record one of `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT RUN`, `HOLD` for each gate. Historical failing receipts remain available. Never claim `READY` for the full product based solely on unit tests or a headless service release.

## Handoff and Git expectations

Every named cloud unit returns the [standard cloud handoff](DEVELOPMENT-WORKFLOW.md#4-git-and-review) with branch/PR/merged SHA, exact CI validation, `LOCAL REQUIRED`, `YOU REQUIRED` and next action. Source code and unit notes are committed through reviewable GitHub branches/PRs; release and device receipts are labeled by the exact pulled SHA. The D: Windows workspace is the **qualification** authority for machine-specific behavior after bootstrap; canonical implementation commits live on GitHub. Historical local and source receipts remain immutable.
