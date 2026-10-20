# Working on CoS-Rust

Read `README.md`, `docs/CURRENT-PHASE.md`, `docs/BIG-IMPLEMENTATION-PLAN.md`,
and `docs/DEVELOPMENT-WORKFLOW.md` before planning or changing this project.
For the active Rust owner, inspect callers and the applicable source contract first.

## Operating model

- **Cloud-first**: implement in cloud agents using the now-published `KaisAbiyyi/chat-on-steroids-on-steroids` repository as canonical. Ship coherent changes through a branch/PR, successful CI and exact-SHA handoff. The Windows checkout on D: is principally for native qualification after a phase or early feasibility gate, and for protecting prior unpublished work.
- Follow the Mind Place engineering discipline: small named cloud units, explicit state and dependencies, source-linked contracts, security and performance gates, professional Git, complete handoffs, and selective user acceptance.
- The 16-phase parity objective is unchanged. Windows is the active native target; Linux may be qualified using isolated test infrastructure; macOS and other targets remain on hold. Headless service evidence never proves desktop or installer acceptance.
- **Historically feature implementation began with `CL-01` after CI-qualified publication; current task eligibility must come from the live dependency registry and current GitHub checks.** Do not resume local uncommitted recorder/caption files merely because they exist; cloud agents must reconstruct or separately inspect a sanitized transfer. Check `docs/CURRENT-PHASE.md` and live GitHub checks for exact CI and work-unit gates.
- Preserve the existing eight uncommitted recording-related paths identified in `docs/CURRENT-PHASE.md`. Do not reset, stash, stage, format, clean, delete, or incorporate them into a documentation commit without checking current ownership and an explicit next-unit decision.
- Keep upstream `024731d8083d92c46e8248d64e7e0b36ab0a470f` pinned and untouched. Preserve historical test receipts and original synthetic fixtures; never change old evidence to make new code pass.

## Implementation quality

- Give each work unit one coherent behavior or integration boundary with a testable exit criterion; split underestimated work at a safe checkpoint. Use existing crate owners and typed interfaces. Prefer the smallest implementation that meets all proven contracts and removal of superseded paths.
- Correctness, authorization, privacy, data integrity, provenance, recoverability, bounded resource use, deterministic tests and latency matter more than cosmetic refactoring. Fail closed on missing grants, stale ownership, unrepresentable data and uncertain side effects.
- Treat tool output, filenames, CLI input, provider content and remote responses as untrusted. Verify authorization and lifetime at actual use/publication boundaries. Never persist, log or stage credentials, personal session data, unredacted environment/clipboard content, or secrets disguised as fixtures.
- Reproduce meaningful behavior failures with a RED regression; fix the cause; demonstrate GREEN. Cover failure, restart, race/revocation and real platform behavior proportionally. Do not change acceptance thresholds, skip tests or call unsupported behavior PASS.
- Measure memory, startup, throughput, binary/installer size and disk usage with identified fixtures and comparable environments before claiming an improvement. Include baseline, units, commit SHA and limitations; do not optimize from guesses.
- **Cloud** builds/tests/artifacts use isolated cloud runners and temporary profiles. **Local** qualification outputs, build/test/export/receipt files, `TEMP`/`TMP` for test subprocesses and Cargo build cache must stay on D: under `D:\projects\CoS-Rust`. Do not introduce large task-owned output on C:. Never delete unknown C: data; cleanup requires verified ownership and a bounded target.
- Before any Docker/WSL-based qualification, verify the **actual Windows-host storage location** of Docker's image/container data and its WSL VHD(s). An in-container `/work/target` path or `docker info`'s Linux data root does not establish that the backing disk is D:. If the host storage location is unverified or on C:, mark the optional Linux gate `BLOCKED` and do not start a cache-producing run. Preserve historical Linux receipts without extending their storage claims.

## Change and Git gates

- Before editing, read the active cloud branch/PR, repo instructions, current unit, source contract, tests and callers. Preserve unrelated or prior-agent dirty changes. Use reviewable Conventional Commits (`feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `perf:`, `chore:`), short-lived branches and PRs; never write cloud feature code solely to an unpushed local checkout.
- On cloud code units run targeted tests, `cargo fmt --all --check`, strict Clippy and the affected cloud Linux/Windows CI matrix. Native/device gates run locally only when assigned to a named `L` session. If a check is NOT RUN or BLOCKED, report it; do not claim an accepted phase.
- Before committing, inspect `git diff --check`, staged diff, ignored/generated material, source fixture authenticity, and possible credential leakage. No `git add -A` with unrelated WIP; never rewrite or squash published/qualified historical commits merely to improve appearance.
- A cloud unit handoff must state `UNIT`, branch/PR/merge and commit SHA, `CHANGES`, `CLOUD VALIDATION`, `PERFORMANCE`, `LOCAL REQUIRED` (none / exact local gate), `YOU REQUIRED` (no / yes, when and what), `KNOWN LIMITS`, and exact `NEXT`. Update `docs/CURRENT-PHASE.md` and relevant dated evidence. A local agent returns an evidence receipt and failure handoff to cloud; it does not silently become the feature implementation authority.
- Do not label a branch or phase DONE without the documented acceptance gate. Keep precise states: `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT RUN`, and `HOLD`.

Read [the cloud-first development workflow](docs/DEVELOPMENT-WORKFLOW.md) for branch, PR, local and user acceptance steps. GitHub `main` was first published at `8637c12`; verify the **current** default-branch SHA and CI before claiming the next READY_CANDIDATE task.

## Shared multi-agent orchestration (2026-10-19)

**Required reading:** [docs/AGENT-ORCHESTRATION.md](docs/AGENT-ORCHESTRATION.md). Existing project, cloud-first D:, security and native acceptance rules remain in force.

- Tasks stay agent-neutral until claimed. ChatGPT Work, Codex and future agents can each plan, implement or review according to supported tools and usage.
- Inspect active PRs/claims, dependencies, resources and exact SHA before writing. Single-writer per unit and logical contract; issue labels are not atomic locks. Serialize conflicts until a transaction-safe controller is installed.
- Push safe intermediate branch checkpoints and open draft PRs; log last durable SHA and unsaved VM work. Only fence/release and re-claim when handing an interrupted unit to a replacement. Report-only review needs no write claim.
- CL-L1 and CL-04A/B/C are already accepted on the recorded merged SHAs. Never bypass any **remaining** `L` or `U` prerequisite gates. Prefer owner-authorized verified GitHub noreply identity for new commits where possible; track actual agent involvement in task/PR metadata and disclose attribution exceptions.

## Executable neutral task registry (mandatory before every claim)

Use [docs/task-registry.json](docs/task-registry.json) as the canonical **115-task** dependency DAG and [docs/PARALLEL-EXECUTION-BOARD.md](docs/PARALLEL-EXECUTION-BOARD.md) as its human-readable view.

1. Run `python tools/task_board.py check` to validate all task IDs, edges, phase transitions and absence of cycles.
2. Run `python tools/task_board.py ready --json` for every unblocked **candidate**; run `python tools/task_board.py task <ID> --json` to inspect immediate and transitive blockers, and `python tools/task_board.py waves --json` to inspect proposed parallel work.
3. **Then independently check live GitHub** default-branch HEAD/checks, all active PRs and claims, shared logical/interface/filesystem locks, and accepted merged/native receipts. A stale committed `todo` is not proof the task is unclaimed. No atomic controller exists: conflicting cloud tasks must be serialized by the human coordinator.
4. Only begin a **C** unit in cloud after all dependency units are accepted/merged; an **L** gate requires a native Windows session, and **U** requires the owner's explicit action. Do not treat GitHub CI as native `L` acceptance.
5. Claim in a task-specific durable branch/PR before editing shared code; on accepted completion, update the registry status/evidence and release ownership. No automatic graph computation ever claims a task.

**Linear phase IDs:** after the backend closure `CL-L6`, Phase 7 begins with **`7P-A → 7P-B → 7P-L1`** (browser architecture preflight) before `7A–7E`; Phase 8 then runs **`8A → 8B → 8C → 8L1 → 8U1`**. Earlier plans mentioning an “early Phase 8” use **obsolete IDs** and must not be used as current claims. The former `8A / 8B / 8L1` preflight is now `7P-A / 7P-B / 7P-L1`; former `8C / 8D / 8E / 8L2` is now `8A / 8B / 8C / 8L1`. Historical acceptance receipts and frozen plans must not be rewritten.

**Ready cloud candidates as of the current acceptance ledger:** `CL-05A`, `CL-06A`, `CL-07A`, `CL-08A`, `CL-09A`. These five may run in parallel **only if** their live claimed write sets/contracts are disjoint.
