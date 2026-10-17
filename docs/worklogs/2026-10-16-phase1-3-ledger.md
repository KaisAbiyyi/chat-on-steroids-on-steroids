# Execution ledger — plan: docs/implementation-plan/2026-10-16-rust-migration-phases-v1.0.md, phases 1–3

Authorization: user requested "implement fase 1-3". Upstream baseline remains
024731d8083d92c46e8248d64e7e0b36ab0a470f; roadmap v1.0 and original scan remain historical records.

## Execution boundaries

- Implement the requested Rust core and session scope directly, with source-derived differential fixtures and fail-first tests.
- Rust root is a new, dedicated workspace and is not a Git repository. Keep the nested upstream repository unchanged; do not initialize/publish unrelated Git history.
- Ruling: use portable Rust owner APIs for phase 2/3; keep GUI/browser framework selection and live cross-platform proof as explicit phase-1 gates. Those choices do not define the on-disk or permission APIs. Cost if wrong: future host adapters may need revision; no visual/runtime parity claim is permitted meanwhile.
- Ruling: generate validation rules and golden data from the pinned upstream implementation, then execute those contracts in Rust; no JavaScript runtime is used by the Rust foundation. Cost if wrong: a source transform not represented in the captured contract must fail closed and receive a specific port/test.
- The existing roadmap is an assessment, not task-level instructions. This ledger binds implementation tasks to its phase 1–3 acceptance requirements; user implementation authorization supplies the execution decision without another approval cycle.
- Never use live personal AppData, credentials, ChatGPT chats or production bridge ports in tests. Test instances receive explicit dedicated data paths.
- Existing history is not trimmed for size. Invalid authority state is not a fresh grant. API-key plaintext is never written to diagnostics.

## Task and evidence ledger

1. Baseline: source-generated config/default/recovery/migration, history, rebind, chronology, Markdown, search and provider usage fixtures captured. Original selected eight suites: 478 PASS, 10 SKIPPED. Windows Electron 44.3.0 safeStorage and Rust v10 AES-GCM/DPAPI profile read each other's synthetic ciphertext. Broader architecture acceptance remains PARTIAL.
2. Foundation: three-platform config data, permission projection, owned paths, atomic persistence/serialized updates, durable retry/flush, instance lease, Windows protected secrets, RAM diagnostic redaction and bounded shutdown implemented. Twelve foundation/config tests and seven runtime tests passed; non-Windows native secret providers remain unavailable/fail-closed.
3. Sessions: JSONL journal, canonical stable-id shards/legacy overlay, terminal revisions, restart hydration, epoch rebind, process completion, assets/overflow, explicit image retirement, plans/traces/handoffs, exact request correlation, access policy, recording, search/Markdown/audit JSON and usage implemented. Twelve history, seven authority and three provider/search projection tests passed before final review. Full browser recorder integration and all upstream edge cases remain qualification work.
4. CLI implemented with explicit marked profiles and verified import copies. Final Windows workspace: 54 PASS, zero failures, format/strict Clippy/release PASS. One fresh review returned 8 Important findings; bounded repairs and focused RED→GREEN plus full suite completed. Final headless binary 3,782,656 bytes (3.607 MiB); crypto interop re-run PASS. Summary/handoff dated 2026-10-17; receipts retain the run's 2026-10-16 directory.

## Execution rulings and corrections

- Ruling: root remains a dedicated non-Git workspace. Use source hashes as the review identity rather than inventing BASE/HEAD commits. No upstream tracked file has changed.
- Ruling: do not point the runtime at an unmarked legacy profile. The import command copies only phase 1–3 durable data into a new target, hashes all copied files and rechecks the source before publishing; the original remains the audit record.
- The first two Electron harness invocations changed userData too late to control Chromium's already-selected Local State. They are not isolation/migration evidence. The qualified rerun passes `--user-data-dir` before script startup, explicitly sets both data paths and verifies them in the receipt. No real CoS profile, credentials, browser/provider session or bridge port was used. An early hidden harness error was terminated by its exact owned command line; final harness exits normally.
- Source-derived correction: Zod 4.6.5 length checks count Unicode code points, while JS slice/token helpers use UTF-16 units. Rust differential tests include emoji and accent cases.
- Test correction: the manually calculated generic read call estimate was 9, but its exact arguments/result/title produce 11 under the upstream formula. The assertion now uses 11. A user-only transcript has no per-call billing comparison rows; the usage attribution test now includes an actual tool-call record. Neither change relaxes an acceptance gate.
- Source-derived correction: Windows secrets need both `secrets.bin` and the DPAPI-wrapped AES key in `Local State`. Direct DPAPI-only decrypt was not assumed compatible; actual two-way Electron/Rust reads passed.
- Config rule regeneration is pinned by SHA-256 in Rust so an unported transform beneath a Zod catch cannot silently become a fallback setting.
- Clippy's mechanical fixes were applied only to new Rust source. Strict warning checks then passed.
- Review repairs: immutable approved-root boundary, raw Unix/Windows path components, preselected image retirement subset, serialized immediate stage+commit and generation-fenced retries, keyless Local State initialization, Darwin release mask, explicit usage comparison cap and bounded provider scan. See docs/implementation-review/2026-10-16-phase1-3-review-v1.0.md.
- Regression-fixture correction: PowerShell junction targets use DOS paths, with a positive read through the junction before testing refusal; granted capability input is used to exercise the Darwin mask. Initial false-positive fixtures are preserved in RED logs and are not acceptance proof.
- Ruling: provider scans may return explicit incomplete results at the 16 MiB budget. No full-total claim is made when `limited:true`. Exact incremental caching remains pending.
- Ruling: no second review is inferred from implementer verification. Review input manifest remains immutable; the final manifest separately identifies repaired source.
- Whole-phase acceptance remains PARTIAL for phases 1, 2, 3. The owner coverage ledger lists code gaps and external gates rather than translating 54 passing tests into full parity.

## External evidence

macOS/Linux native runtime, signed-in provider workflows and final rich UI equivalence require corresponding hosts or future UI/browser implementation. These are NOT RUN, not assumed PASS. Phase status will distinguish implemented code from complete acceptance.
