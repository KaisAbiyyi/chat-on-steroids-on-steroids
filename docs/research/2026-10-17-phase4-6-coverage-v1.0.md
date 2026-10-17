# Phase 4–6 service coverage and exactness gates v1.0

Baseline: upstream `024731d8083d92c46e8248d64e7e0b36ab0a470f`, app 2.1.39, bridge 14.
The original migration map assigns 38 files to phase 4, 30 to phase 5 and 18 to phase 6;
35, 27 and 15 respectively are `src/main` files. Mapping a service to Rust is not proof
that every branch of its original owner is ported or qualified. All three whole-phase gates are **PARTIAL**.

| Phase / source family | Rust implementation | Current proof | Remaining exactness work |
|---|---|---|---|
| 4: MCP kernel, surfaces, tools, call context, persistent paths, inbound/server | cos-mcp kernel/schema/protocol/http/patch/read; phase-3 correlations and recorder | Actual SDK 2.1.0 fixtures, Core tool declarations, schema boundaries, live grants, distinct token paths, Origin/Host/body bounds, native loopback disconnect and drain | Complete error/declaration/instruction branches; browser connector identity/proof depends on phase 7; later tools remain unadvertised |
| 4: code-mode runtime/worker/tool | QuickJS 0.14 in a separately supervised Rust executable; bounded bridge and explicit emissions | Promise calls, fresh contexts, no Node/fetch globals, CPU/memory/output limits, accepted mutation survives timeout, parent-call provenance | QuickJS versus original JS-engine edge cases; Linux/macOS OS-limit qualification; C interpreter is an explicit dependency exception |
| 4: connection/tunnel/proxy/health/locate | Rust process/generation supervisor, cloudflared/OpenAI flags, secret environment, poll/status health, explicit binary path/PATH | Native fake sidecar lifecycle and upstream health oracles; local readiness is distinct from control-plane handshake | Actual external sidecars, full reconnect/outage/auth/route-settle classifier, system/PAC proxy discovery, setup/UI flow and optional additional surface tunnels |
| 4: control reads/stream/actions/workers | Separate opt-in listener, rotating file token, bounded projections/pages/SSE, separate live action admission | Loopback auth/Origin/read/action-off checks and usable session cursors | Complete original response projections, pressure and worker routes; accepted action execution needs phase 9/11 owners; no action backend is advertised |
| 5: rawfs/fsops/filesystem/read-backend/view-image/project-files | cap-std roots and regular handles, bounded UTF-8/BOM/UTF-16 preview, safe images, revision-fenced save, transactional edits | Real files, mixed terminators/BOM, stale revision, hardlink identity, junction refusal, failure rollback, large reads | Complete human file-panel APIs and all native OS/reparse race cases; crash recovery journal is not implemented |
| 5: apply-patch parser/hunk/text/update/mode/index | Canonical patch parser, ordered changes, matching fallbacks, encoding/newline preservation, atomic preflight and guarded rollback | Actual parser/text vectors, real multi-file errors/rollback, strict UTF-8 patch rejection of UTF-16 | Bounded source-excerpt/out-of-order diagnostics, normalize-to-LF modes, streaming parser and shell invocation/interception grammar are not fully ported |
| 5: projects/workspace/project-git/watcher | Durable local/remote catalog and session binding, read-only Git status/branch/revision, native bounded watchers | Real temporary Git repository/watcher; native SFTP project approval and idempotence | Complete Git branch/diff/tree projection, project color/remove/cancelled-create/remote feature details and frontend integration |
| 5: search/ripgrep/text-match/toolchain | Native bounded Rust search, default exclusions, literal/case/regex/glob options and truncation reports | Real temporary file searches and option tests | Full original ordering/exclusion/regex vectors; JS lookaround/backreferences are explicitly rejected by Rust regex; toolchain discovery is not ported |
| 5: remote project-fs/SFTP | Host OpenSSH, SFTP v3 packets/error codes, realpath boundaries, OPEN/FSTAT regular reads, streaming previews, saves and patch rollback | Public protocol fixture plus installed OpenSSH sftp-server executable, read/write/rename/project/large preview/rollback | External SSH authentication/aliases/agent/known-host proof; remote replacement metadata/inode behavior and adversarial server races remain unqualified |
| 6: unified exec/manager/ownership/buffering/truncation/batches | Owner-bound process sessions, pipe/PTY, bounded head/tail, unread completion/publication/replay, shell batches, interruption and tree custody | Native Windows processes/ConPTY, cmd quoting, batch exit/variables, HTTP disconnect recovery, actual truncation/policy/SSH oracles | Complete cancellation/TTL/identical-launch/exit-classification branches, all shell-path and Unicode chunk edges, native POSIX qualification |
| 6: human workspace terminal/IPC | Separate terminal owner and kernel factory, project revalidation, 8-session bound, pause at 262144 UTF-16 units and resume below 65536 | Native PTY, ACK backpressure, permission revocation, cancelled pending creation | GUI/renderer IPC wiring belongs to phase 15; external SSH interactive proof and other OS shutdown qualification are not run |

No personal CoS, browser or provider profile was used. SSH retains host OpenSSH configuration,
agent and known-host handling. The native SFTP subsystem proof does not prove SSH authentication.
The binary is a service/CLI artifact; installer, full desktop payload, startup and memory comparisons
remain NOT RUN. Phase 1–3 historical receipts retain their original scope and partial gates.

## Post-review restrictions and evidence

The whole-scope review found seven Important failure-path/authority issues. The bounded repair pass
adds draining of stopped tunnel children (70 real local restarts), exact-progress partial-write
recovery, live search revocation, profile-anchored control paths and shared local/SFTP patch admission.
Patch transactions are limited to 128 paths and a conservative 256 MiB estimate including input,
snapshots and line-vector workspace; the estimate is not a hard process RSS measurement.

Unix cleanup now observes the leader with `waitid(WNOWAIT)`, cleans its process group before reaping
and retires signal authority afterward. Platform-neutral custody tests pass; `cos-exec` and
`cos-tools` including test sources compile-check for x86_64 Linux. Native Linux/macOS qualification
remains NOT RUN, including the POSIX descendant-pipe regression.

SFTP OPEN/FSTAT proves regular-file type, not canonical opened-handle containment. The MCP kernel
therefore refuses remote filesystem/path operations by default. Explicit host flag
`--trust-ssh-server-paths` opts in for one runtime on a trusted server filesystem, including remote
command/terminal cwd checks. Trust is not available through MCP arguments or saved config and does
not persist after restart. Low-level SFTP protocol fixtures use a trusted local subsystem.
Adversarial remote containment remains **UNQUALIFIED** even with opt-in; the compatibility cost is
that default remote workflows are unavailable. The original per-file-only patch admission also
differs from the new aggregate budget. These are explicit deviations, not exact-parity passes.

See [implementation summary](../implementation-summary/2026-10-17-phase4-6-rust-services-v1.0.md)
and [plan/ledger](../implementation-plan/2026-10-17-phase4-6-implementation-v1.0.md).
