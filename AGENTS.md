# Working on CoS-Rust

Read `README.md`, `docs/CURRENT-PHASE.md`, `docs/BIG-IMPLEMENTATION-PLAN.md`,
and `docs/DEVELOPMENT-WORKFLOW.md` before planning or changing this project.
For the active Rust owner, inspect callers and the applicable source contract first.

## Operating model

- **Cloud-first**: after the GitHub bootstrap is safely published, implement in cloud agents using `KaisAbiyyi/chat-on-steroids-on-steroids` as the canonical repository. Ship coherent changes through a branch/PR, successful CI and exact-SHA handoff. The Windows checkout on D: is principally for native qualification after a phase or early feasibility gate, and for protecting prior unpublished work.
- Follow the Mind Place engineering discipline: small named cloud units, explicit state and dependencies, source-linked contracts, security and performance gates, professional Git, complete handoffs, and selective user acceptance.
- The 16-phase parity objective is unchanged. Windows is the active native target; Linux may be qualified using isolated test infrastructure; macOS and other targets remain on hold. Headless service evidence never proves desktop or installer acceptance.
- **Feature implementation remains paused for safe GitHub bootstrap**. Do not resume the uncommitted recorder/caption feature work merely because its files exist. Only after publication, an explicit next cloud unit and a verified handoff may implementation resume.
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

Read [the cloud-first development workflow](docs/DEVELOPMENT-WORKFLOW.md) for branch, PR, local and user acceptance steps. `origin` is configured, but **the GitHub repository is still empty** as of the bootstrap audit; no cloud code unit should assume its base exists until the first reviewed publication completes.
