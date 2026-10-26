# 8L1 physical Windows qualification — FAIL / incomplete

- **UNIT:** `8L1`; Codex local `COS-8L1-20261026`, claim generation 2. Owner authorized local tests and conditional merge only after all mandatory tests pass. Prior generation 1 was released.
- **Source / artifact:** merged PR #120 at `aeb6fa886f84ebb0215a88aa8111c4ce9f52d4cd`; [hosted run 40182064158](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/40182064158), exact-SHA `cos-8l1-host-windows-aeb6fa886f84ebb0215a88aa8111c4ce9f52d4cd` artifact ID `12594850863`. All 14 manifested files verified, and in-host source/clean-source/full-manifest attestation PASS in both failed native receipts. Isolated branch `feature/8l1-native-20261026` in `D:/projects/CoS-Rust/.test-tmp/8l1/worktree`.
- **CHANGES:** dated native failure receipts and coordination only. Host/8B/8C production code, original fixtures, and eight protected recorder/caption paths unchanged. Active cloud 8P-HOST PR #124 owns overlapping native sources; this L session does not edit them.

## Findings and observed native behavior

**P1 — retained-profile clean start rejects runtime built-in extensions.** `SetupAsync` at `probes/7l1/WindowsHost/Program.cs:363` treats any enumerated extension in a new retained WebView2 profile as unexpected. On this host, WebView2 `155.0.4283.62` creates Microsoft Clipboard Extension and Microsoft Edge PDF Viewer in a fresh profile before the fixture is installed. Independent WebView2 `GetBrowserExtensionsAsync` inventory confirmed both actual IDs/names/enabled values, and two separate new profile roots reproduced `UnauthorizedAccessException: Unexpected extension in clean profile`. No personal profile or provider account was used. Browser profile markers were never reached. The crash wrapper's error names the later missing crash check; the receipt's **actual earliest cause is clean-profile extension admission**.

A separate diagnostic launch without retained profiles (`--8l1 --8l1-crash`) remained alive without a final receipt for 196 seconds and was stopped by exact owned PID/child identity. Record **TIMEOUT**, not native PASS and not a proven second root cause. No consent/profile-action flags were supplied. This source needs bounded per-step timeout/progress evidence to diagnose that path safely.

## Validation

- Artifact integrity/source provenance and compiled C# native authority and MV3 retirement self-tests: **PASS**. The self-test exercises consent/replay/stale-scope/origin policy without physical owner dialogs.
- All extension plus native-worker JavaScript tests: **65 PASS**, none skipped.
- Task DAG: **15 PASS**; browser feasibility guardrails: **16 PASS**.
- Rust workspace: **405 PASS / 0 FAIL / 0 ignored**, exit 0; explicit native Rust `7l2_bridge` example: **3 PASS**, exit 0. Independent `python tools/check-workspace.py --strict` also exited **0**, including workspace tests, formatting and all-target Clippy with `-D warnings`: **PASS**.
- Physical retained-profile clean start: **FAIL twice**. Retained restart **BLOCKED**; actual crash/multi-tab **NOT_RUN** after early failure; exploratory single-process path **TIMEOUT**.
- Owner consent and synthetic sign-in/handoff dialog paths: **NOT_RUN**. Owner explicitly chose automated tests first and cannot operate dialogs now. No agent-generated substitute for owner consent.
- Protected WIP: **8/8 SHA256 unchanged**; pinned upstream HEAD unchanged at `024731d8083d92c46e8248d64e7e0b36ab0a470f` and clean.
- Source-SHA hosted Windows/Linux checks: **SUCCESS**, distinct from this failed physical gate. No local Linux/Docker/WSL qualification run.

## Self-review and handoff

**Self-review — Codex local / COS-8L1-20261026:** changes contain synthetic sanitized JSON receipts and task status only. No browser profiles, Secure Preferences, credentials, raw environment/clipboard or generated binaries are published. Historical receipts are unchanged. P1 remains open; no weakened freshness/manifest/permission check and no native acceptance from hosted CI.

- **CLOUD VALIDATION:** exact source's hosted checks SUCCESS; evidence PR checks pending. No cloud product code written in this local gate.
- **PERFORMANCE:** no improvement/whole-product RAM claim; diagnostic timeout is an observation, not a benchmark.
- **LOCAL REQUIRED:** rerun exact accepted repair SHA with `-VerifyRestart -CrashProbe -ConsentProbe -ProfileActionsProbe` on new D: output, actual retained profiles and new host/Rust process IDs, fresh authenticated channels, real crash retirement and owner dialogs.
- **YOU REQUIRED:** yes, later real Windows Yes/No choices for consent and profile actions, as required by the accepted PR #120 gate. No owner interaction requested while automated-only work runs.
- **KNOWN LIMITS:** native8L1Accepted=false; 8L1 FAIL/incomplete, 8U1/7U1/product renderer/performance unaccepted; OS IME remains HOLD/UNVERIFIED. Native synthetic qualification does not prove production provider login.
- **NEXT:** hand failure to existing cloud native-host owner (#123/#124) for a source-reviewed clean-profile attestation repair that still refuses unknown extensions; add bounded diagnostics for the hanging single-process run; exact-head CI and source-bound artifact; then repeat full physical gate. Keep this evidence PR **Draft / NO MERGE** because the owner's all-PASS condition is unmet. Local claim and physical-host lock **released** after the durable failure handoff; task remains Blocked, not accepted.

Evidence: [qualification JSON](../../audit/2026-10-26/8l1-native/qualification.json), [first failure](../../audit/2026-10-26/8l1-native/native-retained-first.json), [repeat failure](../../audit/2026-10-26/8l1-native/native-retained-repeat.json), [fresh profile API inventory](../../audit/2026-10-26/8l1-native/fresh-profile-extension-inventory.json), [bounded diagnostic timeout](../../audit/2026-10-26/8l1-native/native-single-timeout.json).

Cloud failure handoff: [active native-host issue #123](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/123#issuecomment-6407375914). Evidence [Draft PR #125](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/125), no merge while physical gates are incomplete.
