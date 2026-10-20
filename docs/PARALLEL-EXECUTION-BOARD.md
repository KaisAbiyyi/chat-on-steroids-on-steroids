# CoSoS — full phase task dependency / parallel board

**Current execution board:** 2026-10-20; canonical DAG in [`docs/task-registry.json`](task-registry.json). Historical phase plans remain reference material, not live eligibility. This board indexes **existing** detailed task contracts in [the CL/phase unit matrix](implementation-plan/2026-10-18-cloud-first-units.md) and [the BIG plan](BIG-IMPLEMENTATION-PLAN.md). See [current phase](CURRENT-PHASE.md) for live status and [orchestration policy](AGENT-ORCHESTRATION.md) for claims. This is a planning overlay, not evidence that any future unit passed.

## Claim syntax / authority

Ask `ambil CL-05A` or `ambil 11C` in a CoSoS conversation. The worker must inspect the latest claim/PR registry and state `CLAIM <ID>, owner session, base SHA, resource lock(s), branch, planned test` before editing. Claims are **not preassigned** to ChatGPT or Codex. Without an atomic controller, a single coordinator serializes write conflicts. Report-only inspections do not acquire write ownership; continuation after interrupted browser/quota requires fenced transfer and reconciliation with last remote checkpoint.

`CL-01` through `CL-04C` and `CL-L1` are accepted for their scoped gates. **CL-06A accepted:** PR #19, merge `dd45a2585e8371bb51c1e00e84c5796a1b3dc163`, exact-head Windows/Ubuntu CI PASS. **BASELINE READY CANDIDATES:** `CL-05A`, `CL-06B`, `CL-07A`, `CL-09A`; **CL-08A is CLAIMED in [PR #23](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/23)** (lock `runtime-secrets`), so it is not available for parallel assignment (separate logical locks); no work unit has been claimed by this planning task. Preserve local WIP and treat broader Phase 1–6 parity as PARTIAL. **Phase order is strictly 1–6 closure → Phase 7 preflight `7P-A → 7P-B → 7P-L1` → Phase 7 browser bridge `7A–7E` → Phase 8 built-in browser `8A–8C`.** The preflight validates the old Phase 1 feasibility contract but is *executed and counted in Phase 7*, never called early Phase 8.

## Live prerequisites and executable readiness

`python tools/task_board.py check` validates every ID, missing edge, cycle, completion state and phase transition.

`python tools/task_board.py ready --json` returns current ready **candidates**; `python tools/task_board.py task CL-09C --json` shows direct and transitive blockers; `python tools/task_board.py waves --json` computes a conflict-limited potential schedule.

¹ `READY · PARALLEL` is a committed snapshot **not an active grant/lock**. Immediately before every claim, inspect live GitHub `main` SHA and checks, open PRs, current claims, transitive prerequisites, logical contracts and paths; never assume different named locks imply non-overlapping code. `L` gates cannot run in GitHub-only CI and should be serialized on one physical Windows host. `U` gates require the owner's explicit scoped action. After a gate is accepted, update the registry status and evidence through a reviewed PR, then rerun the resolver; never edit status just to unlock successors.

## Full 16-phase task registry

| Task | Deliverable | Prerequisites (ALL) | Resource lock | Execution | Candidate status¹ |
|---|---|---|---|---|---|
| `CL-01` | Recorder privacy before persistence | NONE | `recorder-privacy` | C (cloud) | DONE |
| `CL-02` | Trusted process/tool outcome evidence | `CL-01` | `tool-evidence` | C (cloud) | DONE |
| `CL-03` | Durable source-aligned caption and timeline | `CL-02` | `caption-projection` | C (cloud) | DONE |
| `CL-L1` | Windows recorder/privacy/process restart proof | `CL-03` | `windows-native` | L (Windows) | DONE |
| `CL-04A` | Provider/model/rebind/title authority | `CL-L1` | `session-identity` | C (cloud) | DONE |
| `CL-04B` | History retention/replay crash limits | `CL-04A` | `session-store` | C (cloud) | DONE |
| `CL-04C` | Unpaired UTF-16/JSON source edge representation | `CL-03` | `unicode-boundary` | C (cloud) | DONE |
| `CL-05A` | Child ownership, cancellation and output custody | `CL-L1` | `process-supervisor` | C (cloud) | **READY · PARALLEL** |
| `CL-05B` | PTY/shell/SSH native process semantics | `CL-05A` | `process-pty` | C (cloud) | BLOCKED |
| `CL-L2` | Native Windows pipe/ConPTY/shell/SSH proof | `CL-05B` | `windows-pty` | L (Windows) | BLOCKED |
| `CL-06A` | Git/project/file-panel/watcher parity | `CL-L1` | `project-files` | C (cloud) | DONE |
| `CL-06B` | SFTP authorization/containment/native errors | `CL-06A` | `sftp` | C (cloud) | **READY · PARALLEL** |
| `CL-07A` | Search/patch traversal and transactional parity | `CL-L1` | `search-patch` | C (cloud) | **READY · PARALLEL** |
| `CL-07B` | Ripgrep/JS regex streamed search exactness | `CL-07A` | `search-stream` | C (cloud) | BLOCKED |
| `CL-L3` | NTFS file-panel/search + native SFTP proof | `CL-06B`, `CL-07B` | `windows-files` | L (Windows) | BLOCKED |
| `CL-08A` | Windows startup/config/secret-store integration | `CL-L1` | `runtime-secrets` | C (cloud) | BLOCKED |
| `CL-08B` | Diagnostics/logging/awake/shutdown bounded behavior | `CL-08A` | `runtime-diagnostics` | C (cloud) | BLOCKED |
| `CL-L4` | Native startup/secret/diagnostics resource proof | `CL-08B` | `windows-startup` | L (Windows) | BLOCKED |
| `CL-09A` | MCP schema/errors/permission conformance | `CL-L1` | `mcp-protocol` | C (cloud) | **READY · PARALLEL** |
| `CL-09B` | Connection retries/proxy/health/tunnel teardown | `CL-09A` | `mcp-transport` | C (cloud) | BLOCKED |
| `CL-09C` | Code-engine and nested execution limits | `CL-09A`, `CL-05A` | `code-engine` | C (cloud) | BLOCKED |
| `CL-L5` | Live loopback MCP/code-worker native proof | `CL-09B`, `CL-09C` | `windows-mcp` | L (Windows) | BLOCKED |
| `CL-10` | Integrated backend parity and independent audit | `CL-04B`, `CL-04C`, `CL-L2`, `CL-L3`, `CL-L4`, `CL-L5` | `backend-closure` | C (cloud) | BLOCKED |
| `CL-L6` | Native Windows integrated backend release receipt | `CL-10` | `windows-backend` | L (Windows) | BLOCKED |
| `7P-A` | Early embedded-browser architecture feasibility | `CL-L6` | `browser-architecture` | C (cloud) | BLOCKED |
| `7P-B` | Embedded host, extension, debugger, UI renderer probe | `7P-A` | `browser-host` | C (cloud) | BLOCKED |
| `7P-L1` | Real Windows embedded browser feasibility gate | `7P-B` | `windows-browser` | L (Windows) | BLOCKED |
| `7A` | Browser bridge protocol 14/identity contract | `7P-L1` | `browser-protocol` | C (cloud) | BLOCKED |
| `7B` | Native authenticated bridge and generation fencing | `7A` | `browser-bridge` | C (cloud) | BLOCKED |
| `7C` | Extension companion ACK/repair/MV3 lifecycle | `7B` | `browser-extension` | C (cloud) | BLOCKED |
| `7L1` | Real extension handshake/MV3 restart | `7C` | `windows-bridge` | L (Windows) | BLOCKED |
| `7D` | Direct browser tools/debugger/frame ownership | `7C`, `7L1` | `browser-tools` | C (cloud) | BLOCKED |
| `7E` | Differential transport fuzz/error/reconnect review | `7D` | `browser-qualification` | C (cloud) | BLOCKED |
| `7L2` | Native browser tools/duplicate input safety | `7E` | `windows-browser-tools` | L (Windows) | BLOCKED |
| `7U1` | Owner real signed-in observe-only browser test | `7L2` | `human-provider` | U (owner) | BLOCKED |
| `8A` | Chrome API/extension adapter and worker world | `7E` | `embedded-adapter` | C (cloud) | BLOCKED |
| `8B` | Sign-in, isolated profiles, session transfer | `8A` | `browser-profiles` | C (cloud) | BLOCKED |
| `8C` | Native browser lifecycle and permission hardening | `8B` | `browser-lifecycle` | C (cloud) | BLOCKED |
| `8L1` | Native embedded restart/multi-tab/profile behavior | `8C` | `windows-embedded` | L (Windows) | BLOCKED |
| `8U1` | Owner provider sign-in and profile UX | `8L1` | `human-provider` | U (owner) | BLOCKED |
| `9A` | Durable outbox attachment ownership | `8U1` | `outbox-schema` | C (cloud) | BLOCKED |
| `9B` | Queue/edit/reorder/cancel state machine | `9A` | `outbox-queue` | C (cloud) | BLOCKED |
| `9C` | Send claim/ACK and retry fencing | `9B` | `outbox-send` | C (cloud) | BLOCKED |
| `9L1` | Native ambiguous ACK and send claim proof | `9C` | `windows-send` | L (Windows) | BLOCKED |
| `9D` | Native uploads and steering/rotation | `9C`, `9L1` | `outbox-upload` | C (cloud) | BLOCKED |
| `9E` | Crash/reconnect/duplicate-action qualification | `9D` | `outbox-recovery` | C (cloud) | BLOCKED |
| `9L2` | Real native attachment/upload/cancellation proof | `9E` | `windows-upload` | L (Windows) | BLOCKED |
| `9U1` | Owner bounded Send/upload approval | `9L2` | `human-send` | U (owner) | BLOCKED |
| `10A` | Compact & Resume WAL/checkpoint identity | `9C` | `resume-journal` | C (cloud) | BLOCKED |
| `10B` | Source/destination binding and lineage | `10A` | `resume-identity` | C (cloud) | BLOCKED |
| `10C` | Resume gate, uncertain send idempotence | `10B` | `resume-protocol` | C (cloud) | BLOCKED |
| `10L1` | Real native/browser resumed conversation isolation | `10C` | `windows-resume` | L (Windows) | BLOCKED |
| `10D` | Repair/retry/timeout and worker integration | `10C` | `resume-worker` | C (cloud) | BLOCKED |
| `10E` | Simulated crash points and source differential | `10D` | `resume-qualification` | C (cloud) | BLOCKED |
| `10L2` | Real restart and recovery of pending uploads | `10E` | `windows-recovery` | L (Windows) | BLOCKED |
| `10U1` | Owner compact-resume conversation acceptance | `10L2` | `human-resume` | U (owner) | BLOCKED |
| `11A` | Durable family/worker IDs and custody | `10E` | `worker-identity` | C (cloud) | BLOCKED |
| `11B` | Worker admission and bounded slot schedule | `11A` | `worker-scheduler` | C (cloud) | BLOCKED |
| `11C` | Inbox addressing, sleep and resume/reuse | `11A` | `worker-inbox` | C (cloud) | BLOCKED |
| `11D` | Safe worktree creation/disposal and archive | `11A` | `worker-worktree` | C (cloud) | BLOCKED |
| `11E` | Isolation/concurrency/custody negative suite | `11B`, `11C`, `11D` | `worker-integration` | C (cloud) | BLOCKED |
| `11L1` | Multi-run Windows worktree/inbox qualification | `11E` | `windows-workers` | L (Windows) | BLOCKED |
| `11U1` | Owner delegation/stop UX acceptance | `11L1` | `human-workers` | U (owner) | BLOCKED |
| `12A` | Durable Goal/Loop obligation ledger | `11E` | `goal-state` | C (cloud) | BLOCKED |
| `12B` | Provider/helper selection, routing and limits | `12A` | `goal-helper` | C (cloud) | BLOCKED |
| `12L1` | Native hosted/local helper connectivity | `12B` | `windows-helper` | L (Windows) | BLOCKED |
| `12C` | Restart/resume/Off-On fences | `12A` | `goal-lifecycle` | C (cloud) | BLOCKED |
| `12D` | Cancel/stale response conformance | `12B`, `12C` | `goal-qualification` | C (cloud) | BLOCKED |
| `12L2` | Real native provider Goal/Loop run | `12D` | `windows-goal` | L (Windows) | BLOCKED |
| `12U1` | Owner bounded agent autonomy acceptance | `12L2` | `human-goal` | U (owner) | BLOCKED |
| `13A` | Plugin manifest and capability catalog | `11E` | `plugin-catalog` | C (cloud) | BLOCKED |
| `13B` | Sandboxed stdio/HTTP adapters | `13A` | `plugin-runtime` | C (cloud) | BLOCKED |
| `13C` | OAuth scoped authorization/revocation | `13A` | `oauth-auth` | C (cloud) | BLOCKED |
| `13U1` | Owner test account OAuth consent | `13C` | `human-oauth` | U (owner) | BLOCKED |
| `13D` | Skills install/hash/permission contract | `13A` | `skill-import` | C (cloud) | BLOCKED |
| `13E` | Optional runtimes/provenance/licensing | `13D` | `plugin-provenance` | C (cloud) | BLOCKED |
| `13F` | Plugin/Skill/OAuth integration security suite | `13B`, `13C`, `13E` | `plugin-integration` | C (cloud) | BLOCKED |
| `13L1` | Real account OAuth callbacks/token rotation | `13U1`, `13F` | `windows-oauth` | L (Windows) | BLOCKED |
| `13L2` | Native Skills install/revoke/upgrade | `13F` | `windows-plugin` | L (Windows) | BLOCKED |
| `13U2` | Owner capabilities and permissions UX | `13L1`, `13L2` | `human-plugin` | U (owner) | BLOCKED |
| `14A` | Native windows/foreground/capture adapter | `8U1` | `desktop-window` | C (cloud) | BLOCKED |
| `14B` | UIA/accessibility/DPI target references | `14A` | `desktop-accessibility` | C (cloud) | BLOCKED |
| `14U1` | Owner Windows permissions consent before capture | `14B` | `human-os-consent` | U (owner) | BLOCKED |
| `14C` | Keyboard/mouse/clipboard custody fences | `14B` | `desktop-input` | C (cloud) | BLOCKED |
| `14D` | Image/media export and processing | `14A` | `desktop-media` | C (cloud) | BLOCKED |
| `14L1` | Real DPI/UIA/capture authorized target proof | `14U1`, `14B` | `windows-capture` | L (Windows) | BLOCKED |
| `14L2` | Real input/clipboard/media latency proof | `14C`, `14D`, `14L1` | `windows-input` | L (Windows) | BLOCKED |
| `14E` | Dictation and target-draft isolation | `14C` | `desktop-dictation` | C (cloud) | BLOCKED |
| `14F` | Pet overlay/native lifecycle/security | `14D`, `14E` | `desktop-overlay` | C (cloud) | BLOCKED |
| `14L3` | Native dictation/pet lifecycle proof | `14F` | `windows-pet` | L (Windows) | BLOCKED |
| `14U2` | Owner dictation, overlay privacy acceptance | `14L3` | `human-desktop` | U (owner) | BLOCKED |
| `15A` | UI navigation and IPC contracts | `8C` | `ui-architecture` | C (cloud) | BLOCKED |
| `15B` | Conversation rendering and large history | `15A` | `ui-conversation` | C (cloud) | BLOCKED |
| `15C` | Rich editor/terminal/code/PDF/math panels | `15A` | `ui-rich` | C (cloud) | BLOCKED |
| `15L1` | Native editor/terminal/PDF/math proof | `15C` | `windows-ui-rich` | L (Windows) | BLOCKED |
| `15D` | Projects/files/agents/plugins settings | `15A` | `ui-settings` | C (cloud) | BLOCKED |
| `15E` | 13 locales, accessiblity and focus keyboard | `15A` | `ui-i18n` | C (cloud) | BLOCKED |
| `15F` | Screenshot/regression fixed DPI/themes | `15B`, `15C`, `15D`, `15E` | `ui-snapshots` | C (cloud) | BLOCKED |
| `15L2` | Native 13-locale/DPI interaction matrices | `15F` | `windows-ui-i18n` | L (Windows) | BLOCKED |
| `15G` | Full desktop integration/performance review | `15F` | `ui-integration` | C (cloud) | BLOCKED |
| `15L3` | Real large-history desktop RAM/performance | `15G` | `windows-ui-load` | L (Windows) | BLOCKED |
| `15U1` | Owner daily navigation/editor approval | `15L1` | `human-ui` | U (owner) | BLOCKED |
| `15U2` | Owner keyboard/i18n/accessibility approval | `15L2` | `human-ui` | U (owner) | BLOCKED |
| `15U3` | Owner feature-completeness acceptance | `15L3` | `human-ui` | U (owner) | BLOCKED |
| `16A` | Versioned reversible migration/backup contract | `15G` | `release-migration` | C (cloud) | BLOCKED |
| `16B` | Updater/signing/installer interface | `16A` | `release-updater` | C (cloud) | BLOCKED |
| `16C` | Bundling/runtime dependencies and licenses | `16A` | `release-bundle` | C (cloud) | BLOCKED |
| `16D` | Startup/RAM/size performance gates | `16C` | `release-perf` | C (cloud) | BLOCKED |
| `16E` | Install/upgrade/rollback automated matrix | `16B`, `16C` | `release-installer-test` | C (cloud) | BLOCKED |
| `16U1` | Owner private profile migration consent | `16E` | `human-migration` | U (owner) | BLOCKED |
| `16L1` | Native clean install/uninstall/updater | `16E` | `windows-installer` | L (Windows) | BLOCKED |
| `16L2` | Only owner-approved profile copy migration | `16U1`, `16E` | `windows-migration` | L (Windows) | BLOCKED |
| `16F` | Security/provenance final release report | `16D`, `16E` | `release-closure` | C (cloud) | BLOCKED |
| `16L3` | Full Windows release/startup/size proof | `16F`, `16L1`, `16L2` | `windows-release` | L (Windows) | BLOCKED |
| `16U2` | Owner guided final release decision | `16L3` | `human-release` | U (owner) | BLOCKED |

## Parallel execution lanes

- **READY · PARALLEL cloud candidates today:** `CL-05A` (process custody), `CL-06A` (Git/files), `CL-07A` (search/patch), `CL-08A` (startup/secret), and `CL-09A` (MCP protocol). `CL-04A/B/C` and `CL-L1` are already accepted. This is a *possible independent wave*, conditional on live claim/PR screening and disjoint file/interface/contract ownership.
- Inside a lane follow its dependencies; several L gates can execute independently after the required merged CL units. `CL-10` and `CL-L6` are shared closure gates, not parallel feature tasks.
- Phase 7 preflight `7P-A → 7P-B → 7P-L1` must pass before `7A`. Full Phase 8 begins *after* `7E`; its first task is `8A`. In later phases safe disjoint implementations include 11B/11C/11D after 11A, 13B/13C/13D after 13A, 14C/14D after appropriate targets, and 15B/15C/15D/15E after 15A. Integration units wait for all edges.
- Cloud Windows/Linux CI never substitutes native ConPTY/NTFS/SSH/provider/desktop/installer gates or explicit OS/account consent. Old receipt SHA is not a new run's passing evidence.

## Handoff contract

Every worker records `UNIT, OWNER_SESSION, CLAIM_GENERATION, BRANCH/PR, BASE_SHA, LAST_PUSHED_CHECKPOINT_SHA, IMPLEMENTED, TESTS, RESOURCES, REMAINING, LOCAL_REQUIRED, YOU_REQUIRED, NEXT`. Checkpoint safe partial work on an isolated branch; no direct main writes, secrets or unpublished personal WIP. If the current agent cannot push, label the task `CHECKPOINT_NOT_REMOTE` and do not pretend another worker can resume its files.
