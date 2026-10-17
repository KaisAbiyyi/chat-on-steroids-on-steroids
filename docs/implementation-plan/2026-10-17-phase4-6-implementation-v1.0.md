# Phase 4–6 implementation v1.0

Date: 2026-10-17 (Asia/Jakarta). Authorization: user requested implementation of phases 4–6.
Baseline: upstream `024731d8083d92c46e8248d64e7e0b36ab0a470f`, app 2.1.39, bridge 14.
This executable plan extends the [16-phase roadmap](2026-10-16-rust-migration-phases-v1.0.md).
Historical phase 1–3 receipts remain valid for their recorded scope; their open gates remain open.

## Contracts and tasks

1. Freeze actual upstream shell-policy, SSH, patch, text and discovery/protocol vectors. Add Rust
   crates `cos-exec`, `cos-tools`, `cos-mcp`. Run functional failing tests before implementation.
   Expected: new behavior tests fail against scaffold implementations; baseline crate tests still pass.
2. Phase 6 process owner: literal allow/deny parser, shell/env/host OpenSSH derivation, bounded head/tail
   output and unread polling, owner-bound continuation, pipe and native PTY input/resize/interrupt,
   signed exit codes, tree cleanup and shutdown. Expected: policy oracle and real isolated process tests pass.
3. Phase 5 file/project owner: canonical approved roots, regular handles, bounded reads/previews/search,
   UTF/BOM/newline preservation, patch preflight and revision-fenced transaction rollback, project/Git
   projection, watcher lifetime and OpenSSH SFTP. Expected: oracle and real temporary filesystem/Git tests pass.
4. Phase 4 gateway: distinct Core/Desktop/Plugins discovery and secret paths, strict schema validation,
   monotonic tool exposure with live revocation, request-bound identity, recording through phase 3,
   MCP HTTP admission/drain, bounded JavaScript and surface-local calls, local control separation,
   generation-fenced tunnel lifecycle. Expected: protocol, loopback HTTP and real interpreter tests pass.
5. Integrate runtime and CLI; execute loopback service / process / file workflows, format, strict Clippy,
   workspace tests and release build. Measure binary separately from installer/full product.
6. One fresh whole-scope review; fix important findings with failing regressions and run the suite.
   Publish dated review and implementation summary in their separate docs directories and update indexes.

## Scope boundaries

Only isolated public fixtures and explicit test profiles are authorized for verification. Desktop,
browser, plugins, outbox and worker implementations belong to later phases; unavailable tools must not
be advertised. Native SSH retains the host's config, agent and known-host policy; external server proof
requires an explicitly configured disposable server. Local protocol fixtures are labeled separately.
Runtime/interpreter dependencies and any deviations must be recorded. A green native Windows suite
cannot stand in for macOS/Linux qualification or full app acceptance.

## Review focus

Check authority after awaited calls; process IDs and model-provided conversation IDs are not custody.
Check link/reparse replacement, file-handle verification, stale/multi-file rollback and encoding edges.
Check body/chunk/request bounds, secret-path/Origin/Host behavior, disconnect vs accepted operations,
monotonic discovery, cross-surface leakage and JavaScript timeout after accepted side effects.
Check native PTY lifecycle, unread completion, foreign stdin, output/resource bounds, cleanup and SSH
quoting. Distinguish tested protocol adapters from external tunnel/SSH and cross-platform evidence.

## Execution ledger

- Setup: upstream remains pinned; root is a dedicated non-Git workspace. No root Git history or worktree
  exists, so review uses a source manifest and dated evidence package rather than fabricated Git refs.
- Setup: phase 1–3 has 54 Rust tests and partial whole-phase acceptance. GUI/browser feasibility remains open.
- Task 1: captured 400 upstream command-policy vectors, SSH quoting and patch/text oracles, actual
  Core Zod declarations and MCP SDK 2.1.0 protocol responses. Earlier selected runs: 64 and 220 PASS
  with one overlapping suite; these are not a unique aggregate. 10 SKIPPED. Rust scaffold tests failed functionally before policy/process/file/code/kernel/protocol
  implementation. Public fixture manifests pin the same upstream commit.
- Task 2: policy, SSH, head/tail and native Windows pipe/PTY tests passed. ConPTY DSR needed a host
  response when no renderer was attached; regression demonstrated missing input and then passed.
  Whole phase 6 qualification is still in progress (delivery, shell batches, remote/live OS gates).
- Task 3: patch/text oracle, stale UTF-16 saves, mixed BOM/newline patch, junction refusal, injected
  commit rollback, native watcher/Git and loopback SFTP v3 protocol tests passed. The rollback test
  exposed retained directory handles preventing Windows parent cleanup; dropping stages fixed it.
- Task 4: direct interpreter limits, explicit-only outputs, fresh context and host-global isolation
  passed. QuickJS 0.11 crashed on interrupt after a Promise child; 0.14 passes the same regression.
  Production code mode uses a separately supervised executable worker; integration proof remains pending.
- Task 2: human terminal owner now has separate IDs, pending-open cancellation, live project/command
  checks, native PTY input/resize, UTF-16 ACK backpressure and bounded owed-exit waiting. Process output
  has publication leases: aborted HTTP bodies restore unread output without rerunning a command.
- Task 3: local in-place edits retain file identity/ACLs; SFTP reads use OPEN/FSTAT, streaming previews,
  canonical remote project approval and patch preflight/rollback. Create-only authority is rechecked
  against the file actually found during preflight. Watchers stop serving a revoked root.
- Task 4: actual SDK protocol fixtures, native HTTP disconnect/drain, QuickJS limits and isolated
  workers pass. Core/Desktop/Plugins remain separate. Control pagination and read/action separation,
  optional tunnel arguments/environment/health and generation supervision are integrated.
- Task 5: pre-review qualification: 107 Rust tests PASS, strict Clippy PASS, release build PASS.
  Native release executable is 8,871,424 bytes (8.460 MiB). Fresh CLI/loopback receipt is being finalized.
  This is service implementation evidence; whole-phase exact acceptance remains PARTIAL.
- Ruling: use a source-hash review package in this dedicated non-Git root — no root history exists,
  and the upstream clone must remain clean — cost if wrong: weaker change-range provenance than Git.
- Ruling: retain JavaScript code mode through isolated QuickJS 0.14 — the original tool executes JS,
  and this removes Node/Electron from the service runtime — cost if wrong: C dependency and JS-engine
  semantic differences require further conformance; this is not a 100-percent-Rust dependency claim.
- Ruling: stage multiline cmd.exe programs as owned temporary .cmd files — raw cmd /c ignored later
  lines in native testing — cost if wrong: batch-file percent expansion can differ from interactive cmd.
- Ruling: keep unimplemented later-phase browser/Desktop/Plugin/action owners unavailable — exposing
  them would advertise handlers that cannot execute — cost if wrong: these workflows remain unavailable
  until their respective owners are ported. Exact full-app acceptance stays open.
- Task 6: one fresh whole-scope source review matched all 92 frozen hashes and returned 0 Critical,
  7 Important and 0 Minor findings. One bounded repair pass followed, without a second review.
  The initial not-ready verdict and every disposition are preserved in the dated review.
- Task 6: functional RED→GREEN regressions cover repeated tunnel stop/restart, partial-write recovery,
  search revocation, a real control-directory junction, patch snapshot/file-count limits and default
  SSH path refusal. Unix custody has two RED→GREEN platform-neutral state regressions; its OS code
  and native test cross-check for Linux, but native POSIX execution remains NOT RUN.
- Task 5/6: post-repair workspace run passed 116 tests, 0 failed/ignored. Strict Clippy passed.
  Final release build, format and all 17 executable/loopback checks passed. The service binary is
  8,887,808 bytes (8.476 MiB); pre-review binary and receipt are archived. Tasks 1–6 of this service
  execution are complete; original whole-phase exact gates remain PARTIAL and documented gaps stay open.
- Final: fixed I1 — `group_termination_is_one_shot_after_leader_reaping` and
  `lost_child_custody_never_signals_a_reused_group` RED→GREEN, suite 116/116. Unix leader cleanup
  precedes reaping and retires signal authority. Native OS qualification is still NOT RUN.
- Final: fixed I2 — `stopped_sidecars_do_not_exhaust_process_capacity` RED→GREEN, suite 116/116.
  Consume sidecar completion before restart; a child that cannot drain causes a bounded stop error.
- Final: fixed I3 — `partial_write_failure_restores_original_contents` RED→GREEN, suite 116/116.
  Guarded progress recovery retains unknown concurrent edits. Persistent storage failure or crash
  recovery remains unqualified; crash journaling is unimplemented.
- Final: fixed I4 — `search_revocation_during_traversal_discards_collected_results` RED→GREEN,
  suite 116/116. Revalidate admission/roots/permissions through scanning and before publication.
- Final: fixed I5 — `control_directory_link_cannot_redirect_credentials_outside_profile`
  RED→GREEN, suite 116/116. Profile-anchored paths refuse linked control child directories;
  concurrent directory replacement races are not all qualified by this test.
- Ruling I6: limit patch transactions to 128 paths and a conservative 256 MiB estimated accounting
  budget for input, snapshots and line workspace; preflight before buffer retention or mutation.
  Cost if wrong: large edits must be split, losing cross-transaction atomicity. This is an explicit
  compatibility deviation from the original per-file-only admission and is not an RSS measurement.
- Ruling I7: SFTP v3 cannot attest canonical paths of opened handles. Refuse kernel SSH filesystem
  operations by default. A human host may use --trust-ssh-server-paths for one runtime on a trusted
  server filesystem; no MCP tool or saved setting can grant this trust. This also gates remote command
  and terminal cwd checks. Cost if wrong: default remote workflows are unavailable; opting in accepts
  server-path race risk. Adversarial remote containment remains unqualified, not a claimed repair.
- Ruling on declined review areas: later owners remain unavailable; external SSH/tunnels/PAC and native
  Linux/macOS remain NOT RUN; QuickJS/Git/search gaps remain PARTIAL; original renderer timeout cause
  remains unresolved. Cost if wrong: these untested environments or incomplete branches may fail.
- Ruling on in-flight rebind: preserve first-proven request attribution for historical recording while
  new admission refuses stale binding. Cost if wrong: historical consumers may expect another
  projection; an explicit audit-contract decision is required before changing phase-3 invariants.
- Ruling on JS timeout: an already accepted native side effect remains owned and completes/records.
  Cost if wrong: timeout is not cancellation-as-rollback. Existing accepted-effect regressions cover it.
  All eight declined-to-judge items and costs are individually recorded in the review; no minors deferred.
