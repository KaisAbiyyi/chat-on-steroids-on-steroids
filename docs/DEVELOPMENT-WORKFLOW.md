# CoS-Rust cloud-first development workflow

Decision updated: 2026-10-18 (Asia/Jakarta). The user now requests **the same cloud-first development model as Mind Place**, with more explicit Windows device/local agent gates. This is a process contract, not proof that current feature implementations are finished.

## 1. Sources of truth

1. The user's current instruction controls the active task.
2. `AGENTS.md` sets persistent agent rules; `docs/CURRENT-PHASE.md` names the one active work unit and its evidence.
3. `docs/BIG-IMPLEMENTATION-PLAN.md` supplies sequencing and exit criteria; the original [16-phase assessment](implementation-plan/2026-10-16-rust-migration-phases-v1.0.md) remains the technical feature/parity scope.
4. Dated `implementation-plan/`, `implementation-summary/`, `implementation-review/`, `research/`, and `audit/` contain historical contracts and immutable evidence. They can contradict later conclusions because they capture different SHAs or environments; do not rewrite their history.
5. **After the first reviewed GitHub publication**, canonical implementation comes from `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids` and the exact merged SHA. The remote is presently configured but empty; GitHub is not canonical until initial publication succeeds. Earlier local Git commits remain historical checkpoints and pinned upstream/public synthetic fixtures remain the behavioral reference.

For handoff across agents or long conversations, committed docs and exact GitHub SHA are the durable project memory. Chat memory may assist discovery but is not source-control evidence. `CURRENT-PHASE.md` records the last merged unit and outstanding L/U gates.

## 2. Unit lifecycle

```text
cloud agent reads CURRENT-PHASE + GitHub SHA + source contract
                 |
                 v
scope one bounded unit; define baseline / RED behavior
                 |
                 v
cloud implementation on topic branch + RED/GREEN tests
                 |
                 v
strict Windows/Linux CI + security/performance review
                 |
                 v
review security, performance, durability, fixture authenticity
                 |
                 +-- FAIL/BLOCKED --> retain evidence, narrow fix unit
                 |
                 v
PR -> review -> merge -> exact SHA cloud handoff
                 |
                 v
local D: qualification only at named L gate
                 |
                 +-- FAIL -> cloud fix unit / new SHA / retest
                 |
                 v
user acceptance only at named U gate
                 |
                 v
next named unit
```

Each **cloud unit** is one API contract, invariant, security boundary or vertical integration boundary, sized for one agent run and reviewable GitHub outcome. Split when independent subsystems, platform dependencies or fault matrices make review uncertain. Never compress security, durable schema, filesystem authority, browser identity or destructive state transitions merely to reduce cloud-unit count. See the [cloud/local/user breakdown](implementation-plan/2026-10-18-cloud-first-units.md).

At the **start** of a cloud unit, read GitHub branch/PR status, root instructions, `CURRENT-PHASE`, callers, upstream baseline and prior evidence. Define an observable RED case or missing contract. Work in a short-lived branch/worktree; do not assume local uncommitted WIP is present in the cloud.

During **cloud implementation**, keep edits within the unit; reuse explicit crate owners, isolate platform adapters, guard resource admission and preserve failure semantics. Favor clear Rust, small modules, typed errors and bounded behavior. Independent reviewer agents can inspect security/architecture boundaries; one owner integrates and verifies the final GitHub PR.

At **cloud qualification**, run targeted Rust tests, Rustfmt, strict Clippy and relevant GitHub Actions Windows/Linux jobs, with pinned synthetic source-oracle comparisons. At a named **L gate**, the user asks the local agent to pull the **exact merged SHA** on D:, verify a clean qualification checkout, and exercise real NTFS/SSH, browser, OS/device or packaged-app behavior. It returns PASS/FAIL/BLOCKED with receipts. A native failure becomes a cloud fix unit and a retest against the new SHA. At a named **U gate**, the user personally handles real account permissions, irreversible actions or subjective UX. **Consent U gates follow cloud/synthetic safety preflight but precede any L test that accesses private accounts, copies profiles, or requests OS input/capture; subjective U acceptance follows L qualification.** Without scoped consent, use synthetic-only tests and mark the private/live gate BLOCKED.

At **handoff**, update unit status, checked scope, merged GitHub SHA, scheduled L/U gates and exact next unit. The next cloud agent reads the committed handoff. The existing local recorder/caption WIP is protected and cannot be silently pushed or merged during bootstrap; transfer only a reviewed, sanitized contract or explicitly approved safe patch.

## 3. Quality matrix

| Concern | Default proof and engineering policy |
| --- | --- |
| Source compatibility | Pin the exact source baseline; compare complete observable outputs/errors and stable identities where representable; document conservative differences. |
| Security and privacy | Current capability/identity/revision checks, redaction before storage/logs/assets, safe path/SSH boundaries, no implicit grants or credential/profile access. |
| Concurrency and recovery | Stale-owner, revoked-root, cancellation, partial write, crash/restart, retry idempotence, process/worker custody tests. |
| Performance | Same input/environment/commit class, baseline versus candidate, p50/p95 when relevant, elapsed time/RSS/disk/binary-size units, bounded fixture work. |
| Maintainability | Clear crate responsibility, typed errors, modest dependencies, reusable existing mechanisms, no dead compatibility paths. |
| Validation | Focused RED/GREEN, affected test suites, Rustfmt, strict Clippy, Windows/Linux cloud CI and device tests at named L gates. |
| Evidence integrity | Commit and fixture hashes, raw output/receipts, clear test counts; preserve FAIL/PARTIAL/NOT RUN without relabeling. |
| Documentation | Update `CURRENT-PHASE`, relevant versioned plan/summary/review, architectural decisions and README only when the behavior actually changed. |

No number of passing Rust unit tests substitutes for a missing desktop, provider, installer or browser acceptance gate. Do not claim a smaller binary is faster or that a headless service release proves the installed application.

## 4. Git and review

Use the explicit `C` unit ID on a short-lived GitHub branch/PR and Conventional Commits. Each PR delivers a coherent outcome with tests, relevant documentation, review and green CI. Merge/rebase only as appropriate to the approved GitHub workflow; do not rewrite published qualified history. Avoid noisy WIP commits, unreviewed sweeping formatting, hidden test skips and false evidence.

**Bootstrap exception:** GitHub is currently a public empty repository. `BOOT-L1` must audit *the entire committed Git history*, sensitive data, tracked files and license obligations before initial publication; `BOOT-U1` requires user review of that public push. Only after verified bootstrap does the GitHub merged SHA become canonical. Before this, retain the historical local commits and WIP unchanged. Never reset/clean/overwrite other agents' dirty work; stage explicit paths only. Never use a forced push to overwrite an existing remote branch.

Commit subjects: `fix(store): redact recorder payloads before persistence`, `test(mcp): prove nonzero process recording`, `docs: adopt cloud-first implementation gates`. Preserve historical qualifying commits and receipts and make additive corrections rather than rewriting them.

Required **cloud C** unit handoff (show these fields to the user even when no manual step is needed):

```text
UNIT: <ID and behavioral outcome>
GITHUB: <topic branch, PR URL, merge state and exact merged SHA>
CHANGES: <modules, observable behavior, schema/protocol decisions>
CLOUD CHECKS: <exact CI jobs, PASS/FAIL, fixture/source evidence>
PERFORMANCE: <baseline/candidate with units or NOT MEASURED>
LOCAL REQUIRED: <NONE or exact L-ID; immediately / phase gate>
YOU REQUIRED: <NO or YES; exact user action and WHEN>
KNOWN LIMITS: <deviations, failed proofs, compatibility and security gaps>
NEXT: <next C-ID or L-ID, with dependencies>
```

Required **local L** result: `GITHUB SHA UNDER TEST`, `MACHINE/OS/DRIVE`, `NATIVE TESTS`, `EVIDENCE`, `STATUS`, `YOU REQUIRED`, `NEXT`. A failed test creates a cloud fix unit; the local agent must hand back a reproducible failure without silently modifying canonical source. A **U** gate is a short checklist for the user's own account/permissions/subjective judgment: consent before a sensitive native action, subjective acceptance after native proof.

## 5. Storage, resources and publication

Cloud runners use isolated cloud-owned cache/storage, with no personal profiles. Keep **local qualification** at `D:\projects\CoS-Rust`; use ignored `.test-tmp/`, `.cache/` and `target/` or explicitly approved D: paths for artifacts and large temporary data. Set local subprocess temp directories on D:. Budget disk/memory before heavy native tests and builds and provide bounded progress evidence. Never redirect local caches, exports or fixtures to C:. Clean only verified task-owned paths, never global Docker/Cargo/temp data.

**Docker/WSL preflight:** before optional Linux or SSH-container qualification, determine where Docker Desktop's Windows-host disk data and WSL2 virtual disks physically reside. Verify they are backed by D: before executing builds/pulls or creating large caches; in-container paths such as `/work/target` and Linux `DockerRootDir` alone do not establish Windows physical storage. The current Linux runner reserves 12 GiB free on the system drive but does **not** enforce D: backing. If storage cannot be verified, report the optional qualification as `BLOCKED` and keep the previously recorded Linux receipts as historical evidence. Do not move or delete Docker/WSL data without a separately reviewed action.

GitHub `origin` is configured as `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids.git`, but this **does not mean anything was pushed**. Publishing a public initial history is a separate reviewed bootstrap step; the user requested a tidy later push. After publication, PRs and CI are normal cloud development gates, and local Windows remains the device-qualification target.
