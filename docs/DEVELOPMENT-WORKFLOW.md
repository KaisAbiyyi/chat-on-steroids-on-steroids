# CoS-Rust local-first development workflow

Decision date: 2026-10-18 (Asia/Jakarta). Authority: the user's direction to apply the structured Mind Place engineering workflow while keeping implementation **local-first**. This is an operating contract, not a claim of feature completeness.

## 1. Sources of truth

1. The user's current instruction controls the active task.
2. `AGENTS.md` sets persistent agent rules; `docs/CURRENT-PHASE.md` names the one active work unit and its evidence.
3. `docs/BIG-IMPLEMENTATION-PLAN.md` supplies sequencing and exit criteria; the original [16-phase assessment](implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) remains the technical feature/parity scope.
4. Dated `implementation-plan/`, `implementation-summary/`, `implementation-review/`, `research/`, and `audit/` contain historical contracts and immutable evidence. They can contradict later conclusions because they capture different SHAs or environments; do not rewrite their history.
5. Actual local Git commits are reproducible checkpoints. The pinned upstream source and public synthetic fixtures are the behavior comparison baseline. No Git remote is configured as of this decision.

For handoff across agents or long conversations, the repository's committed docs and exact Git SHA are the durable shared memory. Chat memory may assist discovery but must never be the only source of a project decision.

## 2. Unit lifecycle

```text
read CURRENT-PHASE + repo status + source contract
                 |
                 v
scope one bounded unit; define baseline / RED behavior
                 |
                 v
local implementation on D: + targeted tests
                 |
                 v
strict workspace checks + necessary native/integration proof
                 |
                 v
review security, performance, durability, fixture authenticity
                 |
                 +-- FAIL/BLOCKED --> retain evidence, narrow fix unit
                 |
                 v
commit focused changes + write precise evidence handoff
                 |
                 v
phase qualification --> user acceptance only when truly needed
                 |
                 v
next named unit
```

Each unit should be one API contract, invariant, security boundary or vertically testable feature slice. It is small enough for one agent invocation without relying on context compaction. Split a unit when independent subsystems, unclear platform dependencies, or expanding fault matrices make review uncertain. Never compress security, durable schema, filesystem authority, browser identity, or destructive state transitions merely to reduce unit counts.

At the **start**, read the relevant repository instructions, `CURRENT-PHASE`, relevant files/callers, upstream baseline and prior evidence; record worktree state and identify pre-existing changes. Identify the failure or missing contract and define concrete success/failure tests before coding.

During **implementation**, keep changes within the unit; reuse explicit crate owners, isolate platform adapters, avoid scattered compatibility special cases, guard all resource admission, and preserve failure semantics. Favor clear and maintainable Rust, small modules and typed errors; avoid both speculative abstraction and repeated ad-hoc logic. Independent reviewer agents may inspect separate tasks, but one owner integrates and verifies the final patch.

At **qualification**, prove behavior with targeted Rust tests, the strict workspace check, and the unit's real Windows/CLI/MCP/browser/process gate as appropriate. Use pinned synthetic upstream comparisons for exactness, and real disposable runtime flows where a mock would miss a failure. Linux validation is supplementary when allowed. User acceptance is limited to account-bound or subjective flows automation cannot establish.

At **handoff**, update unit status and proof with SHA, checked scope, unverified scope and next unit. The next agent starts from this handoff. A new feature run must not silently pick up unknown dirty files from a previous attempt.

## 3. Quality matrix

| Concern | Default proof and engineering policy |
| --- | --- |
| Source compatibility | Pin the exact source baseline; compare complete observable outputs/errors and stable identities where representable; document conservative differences. |
| Security and privacy | Current capability/identity/revision checks, redaction before storage/logs/assets, safe path/SSH boundaries, no implicit grants or credential/profile access. |
| Concurrency and recovery | Stale-owner, revoked-root, cancellation, partial write, crash/restart, retry idempotence, process/worker custody tests. |
| Performance | Same input/environment/commit class, baseline versus candidate, p50/p95 when relevant, elapsed time/RSS/disk/binary-size units, bounded fixture work. |
| Maintainability | Clear crate responsibility, typed errors, modest dependencies, reusable existing mechanisms, no dead compatibility paths. |
| Validation | Focused RED/GREEN, affected test suites, Rustfmt, strict Clippy, full workspace checks at unit/phase gate, real native route where necessary. |
| Evidence integrity | Commit and fixture hashes, raw output/receipts, clear test counts; preserve FAIL/PARTIAL/NOT RUN without relabeling. |
| Documentation | Update `CURRENT-PHASE`, relevant versioned plan/summary/review, architectural decisions and README only when the behavior actually changed. |

No number of passing Rust unit tests substitutes for a missing desktop, provider, installer or browser acceptance gate. Do not claim a smaller binary is faster or that a headless service release proves the installed application.

## 4. Git and review

Use an issue/unit ID, a short-lived topic branch where the current worktree permits it, and Conventional Commits. Keep a commit limited to one coherent change (implementation plus directly relevant tests, or docs-only governance), with a message that explains the outcome. Multiple commits per unit are fine. Avoid noisy "work in progress", fabricated verification receipts, unrelated mass formatting and ambiguous messages.

Never reset/clean/overwrite other agents' dirty changes. With shared WIP, stage explicit file paths; review both `git diff --cached` and the unstaged status immediately before a commit. Use a separate branch/worktree for unrelated work if necessary. Before publishing when GitHub is configured, open a reviewable PR, require applicable CI and human gates, and merge a green coherent outcome. Until then, local commits and receipt-linked SHA are canonical; remote CI is **NOT RUN**, not PASS.

Commit subject examples: `fix(store): redact recorder payloads before persistence`, `test(mcp): prove nonzero process recording`, `docs: establish local-first engineering workflow`. Preserve historical qualifying commits and their receipts; make additive corrections instead of rewriting them.

Required unit handoff:

```text
UNIT: <ID and behavioral outcome>
GIT: <branch, commit SHA, clean/dirty state, remote/PR status>
CHANGES: <owner modules, observable behavior, protocol/schema decisions>
VALIDATION: <exact commands, PASS/FAIL counts, native/CI status, evidence paths>
PERFORMANCE: <baseline/candidate with units or NOT MEASURED>
LOCAL / MANUAL ACCEPTANCE: <PASS, NOT RUN, BLOCKED, HOLD; reason>
KNOWN LIMITS: <deviations, failed proofs, compatibility and security gaps>
NEXT: <one exact named unit or qualification gate>
```

## 5. Storage, resources and publication

Keep repository work at `D:\projects\CoS-Rust`; use ignored `.test-tmp/`, `.cache/` and `target/` or other explicitly approved D: paths for large temporary data. Set tool/test subprocess temp directories on D:. Budget disk and memory before large builds and release suites; avoid simultaneous heavy builds when free RAM is low. Keep bounded progress evidence on long operations. Never redirect caches, exports or bulk fixtures to C: as a convenience. Any cleanup first verifies file ownership and exact targets; no system-wide Docker/Cargo/temp purge.

**Docker/WSL preflight:** before optional Linux or SSH-container qualification, determine where Docker Desktop's Windows-host disk data and WSL2 virtual disks physically reside. Verify they are backed by D: before executing builds/pulls or creating large caches; in-container paths such as `/work/target` and Linux `DockerRootDir` alone do not establish Windows physical storage. The current Linux runner reserves 12 GiB free on the system drive but does **not** enforce D: backing. If storage cannot be verified, report the optional qualification as `BLOCKED` and keep the previously recorded Linux receipts as historical evidence. Do not move or delete Docker/WSL data without a separately reviewed action.

Do not publish or push a new GitHub repository or existing local-history rewrite implicitly. External publication requires a concrete known destination and reviewable result. When a remote is eventually configured, record it here and adopt PR/CI as additional gates; it does not change the project's local-first development location.
