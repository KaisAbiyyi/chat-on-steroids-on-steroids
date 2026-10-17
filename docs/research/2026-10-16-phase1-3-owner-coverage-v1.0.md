# Phase 1–3 implementation coverage v1.0

This is a source-owner ledger for the 16-phase roadmap, not a percentage completion claim.
Every original phase-2 and phase-3 product owner is listed. Rust code and passing tests establish
specific owner behavior; unported branches remain visible. See the final implementation summary
for review fixes and current validation counts.

## Phase 2: 18 source owners

| Original owner | Rust implementation | Remaining acceptance |
| --- | --- | --- |
| main/config.ts | cos-core/config.rs + persistence::ConfigStore; actual defaults/schema/recovery/migration fixtures | All legacy/profile variants; settings publisher/resource-retirement hooks for future service owners; lone-surrogate JS boundary. |
| main/durable.ts | persistence::DurableStore; immediate stage+commit fence, generation-checked retries/flush and strict reads | Process-kill/power-loss qualification. |
| main/env.ts | No command/shell environment adapter yet | Required with phase-6 process host; current CLI does not launch tools. |
| main/index.ts | cos-app Runtime boot, profile guard, instance lease and CLI | GUI/tray/update/browser/MCP/process service startup belongs to later hosts. |
| main/keep-awake.ts | Windows AwakeGuard API | Host integration and macOS/Linux power backends unqualified. |
| main/keychain-notice.ts | No UI prompt projection | Keychain host/UI unavailable; no fake prompt success. |
| main/logger.ts | RAM DiagnosticLog with bounded capacity/redaction | Full log mirror/crash report projection and UI subscriptions. |
| main/platform.ts | Release-aware capability projection in cos-core/config.rs; Darwin native sysctl adapter | Native Mac host probe; Linux native automation remains unavailable. |
| main/redaction.ts | Exact recording API-key redactor separate from diagnostic redactor | Full external-report secret-shape scrubber. |
| main/report-scrub.ts | Not exposed | Support/report export is not a current CLI feature. |
| main/runtime-gc.ts | Rust ownership/drop and bounded queues | Browser/renderer runtime collection is not selected. |
| main/sandbox.ts | cos-core/sandbox.rs; canonical local/virtual/native path resolution, missing targets, Windows segments | WSL alias/reparse/case matrix on real hosts; SSH owner phase 5; descriptor custody phase 5. |
| main/secrets.ts | SecretStore + WindowsCipher; unavailable provider fails closed | macOS/Linux adapters, historical versions/profile rotation matrix. |
| main/setup-profiles.ts | Schema/default fields preserved by config | Setup automation/credential-provider integration later. |
| main/shutdown.ts | Cancellation + bounded sequential shutdown, persistence phases always attempted | Whole desktop teardown hooks and child process termination later. |
| shared/capabilities.ts | 13 capability fields + write mask preserved | Actual tool registrars phase 4. |
| shared/setup-profile.ts | Config validation/migrations preserve setup records | Full application setup workflow later. |
| shared/types.ts | JSON wire fields preserved, Root typed, platform config data | Future host-specific typed APIs grow with their owning phase. |

## Phase 3: 25 source owners

| Original owner | Rust implementation | Remaining acceptance |
| --- | --- | --- |
| main/provider-usage.ts | Explicit read-only homes, seven-day file filter, 16 MiB total scan/4 MiB line bounds, limited-result signal, opt-out, Codex/Claude projections | Incremental per-file cache, exact directory filtering and live historical host corpus. |
| session/blocked-chats.ts | ConversationPolicy persistent blocked ledger | Full IPC lifecycle later. |
| session/conversation-access.ts | Exact prime/committed-ancestor projection with revoke recheck | Actual worker broker callback phase 11. |
| session/correlation.ts | Correlations first-proof epoch, v3–6 restore, bounded wait and commit barrier | Request-plan/agent listeners phase 9/11; full historical migration corpus. |
| session/image-request.ts | Not ported | Image intent classification and context sensitivity. |
| session/markdown-export.ts | Compatible Markdown transcript + complete audit JSON CLI | Native save/open/print routes and rich content transformation. |
| session/progress.ts | History event recording/folding | Running tool/workflow progress adapter. |
| session/read-model.ts | Read/recent/page APIs and cursor CLI | Complete activity/evidence/read-side recovery projections. |
| session/recorder.ts | Session pickup, exact request attribution, stable prose, terminal replay and unattributed journal | Full browser observation schema, request evidence repair, native assets, edit review, continued-work/recovery/error/reminted document integrations. |
| session/search.ts | Unicode/accent fold, bounded terms/results, overflow-aware search/find | Background index/cache parity, entity projection, large-history responsiveness. |
| session/store.ts | Compatible journal/shards/legacy reads, hydration, rebind, processes, assets, plans, traces, handoffs and explicit cleanup | All recovery/activity/handoff/prune/delete/query branches; all fault injection and quota races. No automatic retention deletion. |
| session/summarize.ts | Generic CLI call labels only | Exact tool-specific summaries and evidence/diffstat projections. |
| session/title.ts | Basic first-user title fallback | Exact automatic/provider/origin/project/title migration rules. |
| session/trusted-chats.ts | Persistent batch trust with failure compensation | Full IPC lifecycle later. |
| session/usage.ts | Per-call bounded context comparison, model/effort attribution, verified sends and observed-limit owner | Revision cache/time-window matrix, real provider catalog cap and UI integration. |
| main/usage-tool.ts | Local usage API/CLI only | MCP tool phase 4. |
| shared/chronology.ts | Turn identity, request relation, projectTimeline, residence and turn-local order | Full upstream differential corpus beyond selected vectors. |
| shared/content-reference.ts | Unknown stored fields preserved | Provider entity to Markdown/rendered text conversion. |
| shared/dil-tables.ts | Stored raw text remains intact | Specialized table formatting. |
| shared/markdown-export.ts | Captured final transcript selection and session Markdown format | All answer anchors/continuation/entity/rich HTML cases. |
| shared/provider-usage.ts | Codex cumulative reset and Claude exact-message dedup | Broad provider-version fixtures. |
| shared/session-activity.ts | Basic summary counters | Complete activity UI/presentation state. |
| shared/session.ts | Identity, text/token policy, outcomes, progress fold, JSON fields | Full event variants/formatters/localized origin projections. |
| shared/turn-trace.ts | Bounded normalized trace with first call time and question fence | Full parser differential vectors. |
| shared/usage.ts | Family identification, dates and estimation owner | Alias/rate/formula editing/grouped cost UI functions. |

The remaining items mean **phases 2 and 3 cannot yet be labelled exact full ports**. The result is a
working Rust foundation with concrete regression proof and explicit continuation scope. Phase 1 still
needs the GUI/browser/JS/Wasm feasibility decisions documented in the parity specification.
