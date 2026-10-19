# CoSoS — full phase task dependency / parallel board

**Baseline:** 2026-10-19. This board indexes **existing** detailed task contracts in [the CL/phase unit matrix](implementation-plan/2026-10-18-cloud-first-units.md) and [the BIG plan](BIG-IMPLEMENTATION-PLAN.md). See [current phase](CURRENT-PHASE.md) for live status and [orchestration policy](AGENT-ORCHESTRATION.md) for claims. This is a planning overlay, not evidence that any future unit passed.

## Claim syntax / authority

Ask `ambil CL-05A` or `ambil 11C` in a CoSoS conversation. The worker must inspect the latest claim/PR registry and state `CLAIM <ID>, owner session, base SHA, resource lock(s), branch, planned test` before editing. Claims are **not preassigned** to ChatGPT or Codex. Without an atomic controller, a single coordinator serializes write conflicts. Report-only inspections do not acquire write ownership; continuation after interrupted browser/quota requires fenced transfer and reconciliation with last remote checkpoint.

`CL-01`, `CL-02`, `CL-03` are cloud-merged. **CL-L1 is the next local native gate** and blocks CL-04A. The original historical local WIP remains protected. Phase 1–6 whole-product parity remains partial. Phase 8A/8B/8L1 is a deliberate *early feasibility spike* after CL-L6 and before all Phase 7 units, not a reordered full Phase 8.

## Full remaining registry

| Task | Deliverable | Dependencies | Logical resource lock | Qualification / note |
|---|---|---|---|---|
| `CL-L1` | Windows recorder/privacy/process restart proof | `CL-03` | `windows-native` | LOCAL; next currently, not cloud |
| `CL-04A` | Provider/model/rebind/title authority | `CL-L1` | `session-identity` | C |
| `CL-04B` | History retention/replay crash limits | `CL-04A` | `session-store` | C |
| `CL-04C` | Unpaired UTF-16/JSON source edge representation | `CL-03` | `unicode-boundary` | C; independent of 04B |
| `CL-05A` | Child ownership, cancellation and output custody | `CL-L1` | `process-supervisor` | C |
| `CL-05B` | PTY/shell/SSH native process semantics | `CL-05A` | `process-pty` | C |
| `CL-L2` | Native Windows pipe/ConPTY/shell/SSH proof | `CL-05B` | `windows-pty` | LOCAL |
| `CL-06A` | Git/project/file-panel/watcher parity | `CL-L1` | `project-files` | C |
| `CL-06B` | SFTP authorization/containment/native errors | `CL-06A` | `sftp` | C |
| `CL-07A` | Search/patch traversal and transactional parity | `CL-L1` | `search-patch` | C |
| `CL-07B` | Ripgrep/JS regex streamed search exactness | `CL-07A` | `search-stream` | C |
| `CL-L3` | NTFS file-panel/search + native SFTP proof | `CL-06B`, `CL-07B` | `windows-files` | LOCAL |
| `CL-08A` | Windows startup/config/secret-store integration | `CL-L1` | `runtime-secrets` | C |
| `CL-08B` | Diagnostics/logging/awake/shutdown bounded behavior | `CL-08A` | `runtime-diagnostics` | C |
| `CL-L4` | Native startup/secret/diagnostics resource proof | `CL-08B` | `windows-startup` | LOCAL |
| `CL-09A` | MCP schema/errors/permission conformance | `CL-L1` | `mcp-protocol` | C |
| `CL-09B` | Connection retries/proxy/health/tunnel teardown | `CL-09A` | `mcp-transport` | C |
| `CL-09C` | Code-engine and nested execution limits | `CL-09A`, `CL-05A` | `code-engine` | C; may overlap 09B if contracts frozen |
| `CL-L5` | Live loopback MCP/code-worker native proof | `CL-09B`, `CL-09C` | `windows-mcp` | LOCAL |
| `CL-10` | Integrated backend parity and independent audit | `CL-04B`, `CL-04C`, `CL-L2`, `CL-L3`, `CL-L4`, `CL-L5` | `backend-closure` | C |
| `CL-L6` | Native Windows integrated backend release receipt | `CL-10` | `windows-backend` | LOCAL |
| `8A` | Early embedded-browser architecture feasibility | `CL-L6` | `browser-architecture` | C; Phase 8 spike first |
| `8B` | Embedded host, extension, debugger, UI renderer probe | `8A` | `browser-host` | C |
| `8L1` | Real Windows embedded browser feasibility gate | `8B` | `windows-browser` | LOCAL; blocks Phase 7 |
| `7A` | Browser bridge protocol 14/identity contract | `8L1` | `browser-protocol` | C |
| `7B` | Native authenticated bridge and generation fencing | `7A` | `browser-bridge` | C |
| `7C` | Extension companion ACK/repair/MV3 lifecycle | `7B` | `browser-extension` | C |
| `7L1` | Real extension handshake/MV3 restart | `7C` | `windows-bridge` | LOCAL |
| `7D` | Direct browser tools/debugger/frame ownership | `7C`, `7L1` | `browser-tools` | C |
| `7E` | Differential transport fuzz/error/reconnect review | `7D` | `browser-qualification` | C |
| `7L2` | Native browser tools/duplicate input safety | `7E` | `windows-browser-tools` | LOCAL |
| `7U1` | Owner real signed-in observe-only browser test | `7L2` | `human-provider` | HUMAN |
| `8C` | Chrome API/extension adapter and worker world | `7E` | `embedded-adapter` | C |
| `8D` | Sign-in, isolated profiles, session transfer | `8C` | `browser-profiles` | C |
| `8E` | Native browser lifecycle and permission hardening | `8D` | `browser-lifecycle` | C |
| `8L2` | Native embedded restart/multi-tab/profile behavior | `8E` | `windows-embedded` | LOCAL |
| `8U1` | Owner provider sign-in and profile UX | `8L2` | `human-provider` | HUMAN |
| `9A` | Durable outbox attachment ownership | `8U1` | `outbox-schema` | C |
| `9B` | Queue/edit/reorder/cancel state machine | `9A` | `outbox-queue` | C |
| `9C` | Send claim/ACK and retry fencing | `9B` | `outbox-send` | C |
| `9L1` | Native ambiguous ACK and send claim proof | `9C` | `windows-send` | LOCAL |
| `9D` | Native uploads and steering/rotation | `9C`, `9L1` | `outbox-upload` | C |
| `9E` | Crash/reconnect/duplicate-action qualification | `9D` | `outbox-recovery` | C |
| `9L2` | Real native attachment/upload/cancellation proof | `9E` | `windows-upload` | LOCAL |
| `9U1` | Owner bounded Send/upload approval | `9L2` | `human-send` | HUMAN |
| `10A` | Compact & Resume WAL/checkpoint identity | `9C` | `resume-journal` | C |
| `10B` | Source/destination binding and lineage | `10A` | `resume-identity` | C |
| `10C` | Resume gate, uncertain send idempotence | `10B` | `resume-protocol` | C |
| `10L1` | Real native/browser resumed conversation isolation | `10C` | `windows-resume` | LOCAL |
| `10D` | Repair/retry/timeout and worker integration | `10C` | `resume-worker` | C |
| `10E` | Simulated crash points and source differential | `10D` | `resume-qualification` | C |
| `10L2` | Real restart and recovery of pending uploads | `10E` | `windows-recovery` | LOCAL |
| `10U1` | Owner compact-resume conversation acceptance | `10L2` | `human-resume` | HUMAN |
| `11A` | Durable family/worker IDs and custody | `10E` | `worker-identity` | C |
| `11B` | Worker admission and bounded slot schedule | `11A` | `worker-scheduler` | C |
| `11C` | Inbox addressing, sleep and resume/reuse | `11A` | `worker-inbox` | C; parallel with 11B |
| `11D` | Safe worktree creation/disposal and archive | `11A` | `worker-worktree` | C; parallel with 11B/11C |
| `11E` | Isolation/concurrency/custody negative suite | `11B`, `11C`, `11D` | `worker-integration` | C |
| `11L1` | Multi-run Windows worktree/inbox qualification | `11E` | `windows-workers` | LOCAL |
| `11U1` | Owner delegation/stop UX acceptance | `11L1` | `human-workers` | HUMAN |
| `12A` | Durable Goal/Loop obligation ledger | `11E` | `goal-state` | C |
| `12B` | Provider/helper selection, routing and limits | `12A` | `goal-helper` | C |
| `12L1` | Native hosted/local helper connectivity | `12B` | `windows-helper` | LOCAL |
| `12C` | Restart/resume/Off-On fences | `12A` | `goal-lifecycle` | C; parallel with 12B until integration |
| `12D` | Cancel/stale response conformance | `12B`, `12C` | `goal-qualification` | C |
| `12L2` | Real native provider Goal/Loop run | `12D` | `windows-goal` | LOCAL |
| `12U1` | Owner bounded agent autonomy acceptance | `12L2` | `human-goal` | HUMAN |
| `13A` | Plugin manifest and capability catalog | `11E` | `plugin-catalog` | C |
| `13B` | Sandboxed stdio/HTTP adapters | `13A` | `plugin-runtime` | C |
| `13C` | OAuth scoped authorization/revocation | `13A` | `oauth-auth` | C; separate security owner |
| `13U1` | Owner test account OAuth consent | `13C` | `human-oauth` | HUMAN; before real OAuth |
| `13D` | Skills install/hash/permission contract | `13A` | `skill-import` | C; parallel with 13B/13C |
| `13E` | Optional runtimes/provenance/licensing | `13D` | `plugin-provenance` | C |
| `13F` | Plugin/Skill/OAuth integration security suite | `13B`, `13C`, `13E` | `plugin-integration` | C |
| `13L1` | Real account OAuth callbacks/token rotation | `13U1`, `13F` | `windows-oauth` | LOCAL |
| `13L2` | Native Skills install/revoke/upgrade | `13F` | `windows-plugin` | LOCAL |
| `13U2` | Owner capabilities and permissions UX | `13L1`, `13L2` | `human-plugin` | HUMAN |
| `14A` | Native windows/foreground/capture adapter | `8U1` | `desktop-window` | C |
| `14B` | UIA/accessibility/DPI target references | `14A` | `desktop-accessibility` | C |
| `14U1` | Owner Windows permissions consent before capture | `14B` | `human-os-consent` | HUMAN |
| `14C` | Keyboard/mouse/clipboard custody fences | `14B` | `desktop-input` | C |
| `14D` | Image/media export and processing | `14A` | `desktop-media` | C; parallel with 14C |
| `14L1` | Real DPI/UIA/capture authorized target proof | `14U1`, `14B` | `windows-capture` | LOCAL |
| `14L2` | Real input/clipboard/media latency proof | `14C`, `14D`, `14L1` | `windows-input` | LOCAL |
| `14E` | Dictation and target-draft isolation | `14C` | `desktop-dictation` | C |
| `14F` | Pet overlay/native lifecycle/security | `14D`, `14E` | `desktop-overlay` | C |
| `14L3` | Native dictation/pet lifecycle proof | `14F` | `windows-pet` | LOCAL |
| `14U2` | Owner dictation, overlay privacy acceptance | `14L3` | `human-desktop` | HUMAN |
| `15A` | UI navigation and IPC contracts | `8E` | `ui-architecture` | C |
| `15B` | Conversation rendering and large history | `15A` | `ui-conversation` | C |
| `15C` | Rich editor/terminal/code/PDF/math panels | `15A` | `ui-rich` | C; parallel with 15B |
| `15L1` | Native editor/terminal/PDF/math proof | `15C` | `windows-ui-rich` | LOCAL |
| `15D` | Projects/files/agents/plugins settings | `15A` | `ui-settings` | C; parallel with 15B/C |
| `15E` | 13 locales, accessiblity and focus keyboard | `15A` | `ui-i18n` | C; parallel with 15B/C/D |
| `15F` | Screenshot/regression fixed DPI/themes | `15B`, `15C`, `15D`, `15E` | `ui-snapshots` | C |
| `15L2` | Native 13-locale/DPI interaction matrices | `15F` | `windows-ui-i18n` | LOCAL |
| `15G` | Full desktop integration/performance review | `15F` | `ui-integration` | C |
| `15L3` | Real large-history desktop RAM/performance | `15G` | `windows-ui-load` | LOCAL |
| `15U1` | Owner daily navigation/editor approval | `15L1` | `human-ui` | HUMAN |
| `15U2` | Owner keyboard/i18n/accessibility approval | `15L2` | `human-ui` | HUMAN |
| `15U3` | Owner feature-completeness acceptance | `15L3` | `human-ui` | HUMAN |
| `16A` | Versioned reversible migration/backup contract | `15G` | `release-migration` | C |
| `16B` | Updater/signing/installer interface | `16A` | `release-updater` | C |
| `16C` | Bundling/runtime dependencies and licenses | `16A` | `release-bundle` | C; parallel with 16B |
| `16D` | Startup/RAM/size performance gates | `16C` | `release-perf` | C |
| `16E` | Install/upgrade/rollback automated matrix | `16B`, `16C` | `release-installer-test` | C |
| `16U1` | Owner private profile migration consent | `16E` | `human-migration` | HUMAN; before real profile access |
| `16L1` | Native clean install/uninstall/updater | `16E` | `windows-installer` | LOCAL |
| `16L2` | Only owner-approved profile copy migration | `16U1`, `16E` | `windows-migration` | LOCAL |
| `16F` | Security/provenance final release report | `16D`, `16E` | `release-closure` | C |
| `16L3` | Full Windows release/startup/size proof | `16F`, `16L1`, `16L2` | `windows-release` | LOCAL |
| `16U2` | Owner guided final release decision | `16L3` | `human-release` | HUMAN |

## Parallel execution lanes

- **After CL-L1 PASS**, distinct owners may claim *independent* `CL-04A` (session identity), `CL-05A` (process custody), `CL-06A` (Git/files), `CL-07A` (search/patch), `CL-08A` (startup/secret), and `CL-09A` (MCP protocol) **only after verifying disjoint crate/interface/write locks**. Do not use parallelism to bypass CL-L1. `CL-04C` may run as a separate UTF-16 source-oracle lane.
- Inside a lane follow its dependencies; several L gates can execute independently after the required merged CL units. `CL-10` and `CL-L6` are shared closure gates, not parallel feature tasks.
- Phase 8 early browser spike 8A→8B→8L1 is strictly gated before Phase 7. In later phases safe disjoint implementations include 11B/11C/11D after 11A, 13B/13C/13D after 13A, 14C/14D after appropriate targets, and 15B/15C/15D/15E after 15A. Integration units wait for all edges.
- Cloud Windows/Linux CI never substitutes native ConPTY/NTFS/SSH/provider/desktop/installer gates or explicit OS/account consent. Old receipt SHA is not a new run's passing evidence.

## Handoff contract

Every worker records `UNIT, OWNER_SESSION, CLAIM_GENERATION, BRANCH/PR, BASE_SHA, LAST_PUSHED_CHECKPOINT_SHA, IMPLEMENTED, TESTS, RESOURCES, REMAINING, LOCAL_REQUIRED, YOU_REQUIRED, NEXT`. Checkpoint safe partial work on an isolated branch; no direct main writes, secrets or unpublished personal WIP. If the current agent cannot push, label the task `CHECKPOINT_NOT_REMOTE` and do not pretend another worker can resume its files.
