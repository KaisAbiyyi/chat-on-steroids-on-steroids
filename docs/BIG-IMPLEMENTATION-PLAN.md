# CoS-Rust — BIG implementation plan (cloud-first)

Direction updated: 2026-10-24, Asia/Jakarta. This document is the current *performance-first execution roadmap*. The original [16-phase feature/parity assessment](implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) is historical feature coverage and dependency reference, **not an all-features release requirement**. The [phase 1–6 closure ledger](implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md) remains historical evidence. Detailed cloud unit names, local gates and manual testing are in the [unit and acceptance matrix](implementation-plan/2026-10-18-cloud-first-units.md).

**CoS-Rust is now cloud-first, following Mind Place's development flow.** Cloud ChatGPT/Codex agents implement directly against [the published GitHub repository](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids), starting from a CI-qualified `main` SHA. Local Windows agents perform the device/runtime qualification named by a gate; the user participates only for account consent, irreversible choices or subjective acceptance. The old local history is included in public `main`; eight unfinished local recorder/caption paths remain protected and unpublished.

## Product and acceptance principles

The objective is a lightweight, reliable, Rust-owned ChatGPT coding workstation with coding-critical tools and security behavior preserved; non-coding original features are selectively optional. The frozen upstream source is commit `024731d8083d92c46e8248d64e7e0b36ab0a470f` (source app 2.1.39, bridge 14); exact differences must be consciously registered. A Rust service running on Windows is one milestone, not the finished desktop product.

Priority order: **security and privacy; durable correctness, identity and provenance; deterministic contracts and testability; measured performance and resource cost; maintainable code; product experience and documentation**. All are requirements; priority determines how to resolve a conflict. Prefer a small correct, bounded interface and a measured improvement over a large speculative rewrite. Keep native-platform differences explicit and preserve prior valid data and receipts.

Windows x64 is the **primary shipped and native-qualification focus**, including cloud Windows CI. Linux cloud CI is useful for cross-platform Rust code. Linux Docker on the user's device requires proof of D: backing storage; macOS and other targets are **HOLD** under the present user directive, not deleted from full migration scope.

## Product strategy supersession — 2026-10-24

**P0 OWNER DECISION — 2026-10-24:** CoSoS is a lean Codex-like ChatGPT coding workstation. Measured RAM/CPU/latency, reliability and fast startup are the highest product priority after non-negotiable security, permissions, durable state and tool/Send correctness. Mandatory full CoS feature/visual parity is superseded. Mind Place stays fully isolated; no memory, archive or knowledge-engine dependency. Keep coding-critical terminal, files, patch, Git, MCP, browser bridge, bounded workers and recovery. Optional chat grouping must be metadata-only and lazy. Pets, broad plugins, full built-in tab UI, 13-locale perfection, dictation, decorative effects and advanced Goal/Loop are optional/deferred, not secretly deleted. No unmeasured product memory claims. See [ADR](decisions/2026-10-24-performance-first-workstation.md) and [PERF workstream](implementation-plan/2026-10-24-performance-first-workstream.md).

Active cross-cutting units PERF-00 through PERF-06 execute earlier than release Phase 16 if their AND dependencies and exclusive locks allow. Historical feature-parity matrices and measurement receipts remain immutable references, not current release feature obligations. Real native Windows 15L3, product WebView2 8L1 and owner 7U1 remain unproven where their evidence says NOT_RUN/BLOCKED. Cloud tests do not certify full product RAM. New tasks are unclaimed unless genuinely claimed in GitHub and Notion.

## Cloud development and handoff

### Owner-directed IME deferral — 2026-10-22

The owner explicitly directed skipping further OS IME qualification now and
carrying it in the roadmap. Track **`IME-DEFER-20261022`: HOLD / UNVERIFIED**.
Native automated keyboard input, selection, undo/redo and Unicode have scoped
PASS evidence in the [7P-L1 follow-up](implementation-summary/2026-10-22-7p-l1-keyboard-ime-native.md).
Real Windows IME preedit, candidate selection, commit/cancel and undo have not
been proven. Unicode insertion and Chromium CDP IME simulation do not prove them.

This is a scheduling exception for the **OS IME portion only** of `rich_editor`:
do not repeat it or treat it alone as an immediate development prerequisite.
Other 7P-L1 requirements and the explicit review/acceptance of that unit remain;
this exception does not mark 7P-L1 PASS or automatically unblock 7A. Preserve the
original feasibility matrix and historical receipts rather than rewriting them.

Carry the gap to **8L1**, using the representative embedded browser/editor, and
retain any remaining gap for **Phase 15 native editor/user acceptance before
claiming full multilingual editor parity**. The exit criterion is a source- and
binary-bound real Windows IME test of preedit, candidate conversion, commit,
cancel, selection and undo without duplicated/lost text, including focus changes.
Do not mark it PASS until evidence exists or silently drop it from release scope.
No further local IME installation or testing is assigned now. This does not waive
screen-reader, clipboard, distribution or other outstanding acceptance gates.

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

`origin` points to `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids.git`. The public GitHub repository received its **first main push on 2026-10-18**, at verified SHA `8637c12ccda8610e5a06534f30af4688034cc9b6`. The user explicitly authorized the publication. The old local baseline `main=68f7954` was fast-forwarded to retain all 24 subsequent approved feature/governance commits, and the new GitHub branch was verified to descend from that history. The [bootstrap receipt](implementation-summary/2026-10-18-github-bootstrap.md) records the security screening, WIP isolation and first CI run. Cloud feature work begins only after the required CI gate qualifies; later feature implementations live on GitHub branches/PRs.

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

**2026-10-23 scoped native feasibility closure supersedes only the 7P-L1 HOLD scheduling state below when reviewed/merged.** The requested missing umbrella checks were executed by an independent physical Windows qualifier that installs/launches the actual source-pinned synthetic WebView2 adapter. Accept 7P-L1 only for the early embedded browser feasibility contract, including real extension debugger/MV3/ownership and measured synthetic distribution. Defer *production* installer/signature, private RSS, real product startup and approved release budgets to 16L3 as always planned. Signed-in provider auth remains a separate 7U1/8U1 user gate; OS IME stays explicitly deferred to 8L1/Phase 15. Native 7L1 must still prove the actual integrated bridge and companion against the accepted architecture; its completion is not implied by this preflight.

Phase 7 follows an **owner-approved implementation-first, native-gated sequence (2026-10-22)**. After `CL-L6`, `7P-A` and `7P-B` are accepted. `7P-L1` is **HOLD** (local/native physical Windows feasibility is partial and not accepted); bounded cloud implementation `7A → 7B → 7C` may proceed from accepted `7P-B` without claiming native acceptance. However, `7L1` requires **both `7C` and completed `7P-L1`** before real extension handshake/native browser testing can be accepted; `7D–7E` and later Phase 8 remain transitively gated. If the deferred native feasibility reveals contradictions, redesign must precede native integration. This is sequencing-only: do not assert keyboard/IME, screenreader, clipboard, browser lifecycle, sign-in, installer or product parity from synthetic/cloud tests. The [task DAG](task-registry.json) is the authoritative machine-readable graph and the [parallel board](PARALLEL-EXECUTION-BOARD.md) shows current status.

| Phase | First unit / engineering priority | Automatic qualification | Manual gate when applicable |
| --- | --- | --- | --- |
| 7 | `7P-A` browser feasibility preflight, then `7A` bridge protocol | Native preflight host/debugger/rendering, then simulated bridge/epoch/reconnect proof | Signed-in provider flows as needed |
| 8 | `8A` embedded browser API/worker compatibility | Profile/session lifecycle and real native embed checks | Actual account/profile flow |
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
| Phase 7 — preflight + bridge plus owner runner prep | 8 | 3 | 1 |
| Phase 8 — embedded browser | 3 | 1 | 1 |
| Phase 9 — outbox/uploads | 5 | 2 | 1 |
| Phase 10 — Compact & Resume | 5 | 2 | 1 |
| Phase 11 — workers | 5 | 1 | 1 |
| Phase 12 — Goal/Loop | 4 | 2 | 1 |
| Phase 13 — plugins/Skills/OAuth | 6 | 2 | 2 |
| Phase 14 — native desktop | 6 | 3 | 2 |
| Phase 15 — desktop UI | 7 | 3 | 3 |
| Phase 16 — migration/release (legacy units) | 6 | 3 | 2 |
| **P0 PERF-00–06 cross-cutting (early)** | **5** | **2** | **0** |
| **Full DAG unit inventory (includes accepted historical units)** | **78** | **30** | **15** |
| **With historical bootstrap** | **78** | **31** | **16** |

`C` means a GitHub/CI cloud agent run, `L` means a local Windows agent session (the user triggers the handoff; the agent executes tests), and `U` means **the user personally needs to act**. These are total canonical task IDs, **not** all unfinished work or all mandatory feature commitments under the new owner decision. The old 72/28/15 full-parity tally was superseded by 7U1-PREP plus PERF-00–06; failure/retry units are separate. `U0` for the backend closure means no manual UX/account test is planned. Each cloud handoff tells the user explicitly whether to do anything **right now**, later at the phase local gate, or not at all.

## Quality and release gates

For every cloud Rust implementation unit use an actual observable RED→GREEN regression, focused tests, `cargo fmt --all --check`, strict Clippy and appropriate Windows/Linux GitHub Actions CI. Local qualification uses `python tools/check-workspace.py --strict`, `tools/qualify-session.py`, `qualify-search.py`, native SSH/process tests or GUI/browser scripts **only when named by an L gate**. Always run with isolated synthetic profiles. A release-affecting unit records executable size/runtime cost, and the later local release gate verifies actual packaged size separately.

Security gate: no credential leakage into args/results/assets/captions, no stale authority, no unsafe filesystem escape or unclear side-effect retry. Performance gate: baseline/candidate with exact commit, fixture, units and comparable setup; actual p95/timing/RSS/size evidence if the unit changes critical latency or memory. Review gate: direct critical implementor **self-review** of exact SHA/diff, permissions, durability, concurrency, protocol and native boundaries; independent feedback is optional and not a merge gate.

Record one of `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT RUN`, `HOLD` for each gate. Historical failing receipts remain available. Never claim `READY` for the full product based solely on unit tests or a headless service release.

## Handoff and Git expectations

Every named cloud unit returns the [standard cloud handoff](DEVELOPMENT-WORKFLOW.md#4-git-and-review) with branch/PR/merged SHA, exact CI validation, `LOCAL REQUIRED`, `YOU REQUIRED` and next action. Source code and unit notes are committed through reviewable GitHub branches/PRs; release and device receipts are labeled by the exact pulled SHA. The D: Windows workspace is the **qualification** authority for machine-specific behavior after bootstrap; canonical implementation commits live on GitHub. Historical local and source receipts remain immutable.

## Shared multi-agent execution governance (2026-10-19)

Roadmap tasks remain **unassigned** until actively claimed. Read [agent orchestration policy](AGENT-ORCHESTRATION.md) for exclusive claims, independent parallel units, remote checkpoints, interruption/takeover safety, dynamic agent roles, shared usage budgeting and Git author attribution. No backend acceptance gate, including CL-L1, changes through this governance update.


## Parallel claimable task index

The [full dependency and parallel work-unit map](PARALLEL-EXECUTION-BOARD.md) records all remaining named CL/Phase 7–16 units, native/human gates and shared-resource locks. IDs remain neutral until claimed. The original phase order and acceptance contracts are unchanged.
