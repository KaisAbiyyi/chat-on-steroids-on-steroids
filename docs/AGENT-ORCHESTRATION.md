# Shared multi-agent development orchestration

**Policy version:** 2026-10-19. **Scope:** `KaisAbiyyi/chat-on-steroids-on-steroids` within **CoSoS**. This is **execution governance**, not a re-planning or completion receipt. The existing BIG plan, phase ordering, quality limits, privacy requirements and current-phase pointer remain authoritative for *what* to build. This policy controls *how independent workers coordinate*. If a product/security rule is stricter, retain the stricter rule.

## Actors and authority

- **Human owner:** absolute product/release authority; approves real-account consent, destructive changes, irreversible publication and subjective acceptance, and initiates named local/native qualification gates. Do not burden the owner with routine agent-to-agent dispatch.
- **ChatGPT Work/GitHub-connected agent, Codex Cloud/local, and optional future Antigravity:** capability-based workers. Any can plan, research, implement, test, review or coordinate if the actual environment has the necessary tools. **Never hard-code a task to one vendor/model.** A reviewer must be independent of the implementer for critical boundaries.
- **GitHub:** durable source of truth for committed work, PRs, CI, SHA-specific evidence and handoffs. Conversation context and temporary cloud VMs are not durable coordination stores.
- **Shared usage:** budget ChatGPT Work and Codex as one constrained pool where they share allowance; do not assume switching between them replenishes usage. Prefer bounded tasks, no idle agent waiting for CI, and measured concurrency over maximum simultaneous sessions. Do not assume rate/usage APIs exist.

## Canonical prerequisite resolution and parallel claims (2026-10-20)

The [machine-readable task registry](task-registry.json) and [parallel board](PARALLEL-EXECUTION-BOARD.md) are the current task topology. Before EVERY new unit, run `python tools/task_board.py check`, `ready --json`, and `task <ID> --json`. The resolver shows prerequisites, inherited blockers, status, and candidate waves; it is intentionally **offline** and **cannot** see GitHub PRs, local Windows receipts not yet merged, real account consent, or concurrent owners. Therefore a `READY_CANDIDATE` is **never an automatic grant**.

Confirm current default branch SHA/CI, PRs, claim ledger, actual files and logical contract overlap and all L/U evidence **after** offline resolution and **before** recording a claim. Single-writer per lock; independent resource strings are not conclusive if both tasks edit the same interface, shared files or dependency contract. Every accepted unit must advance the registry's `status` with durable proof (PR/merged SHA, L receipt or explicit U consent), and every dependent unit must re-run readiness on the updated `main`. An L unit always requires the appropriate local native Windows proof, and overlapping use of one Windows machine should be serialized.

**Owner-approved Phase 7 sequencing (2026-10-22):** `CL-L6 → 7P-A → 7P-B → 7A → 7B → 7C`; native `7P-L1` remains HOLD and mandatory before `7L1`, followed by `7D → 7E → 8A–8C → 8L1 → 8U1`. This allows bounded Cloud implementation, not acceptance of native/browser/user-account tests. The Phase 7 preflight tests the original Phase 1 browser feasibility requirement. Historical plans calling it “early Phase 8” are superseded, not a mandate to perform Phase 8 before Phase 7.

## Neutral roadmaps and work units

Roadmaps and phase documents list task IDs, scope, prerequisites, expected outputs, tests, local (L) and human (U) gates; they **do not preassign agents**. Phase completion is a dependency/acceptance result, never merely a finished conversation. Before starting a task, inspect current `main` SHA, active PRs/claims, instructions, neighboring work, unmerged changes and its dependency DAG.

Every executable unit must have: globally unique `project/repo/task-id`, one bounded deliverable, testable acceptance criteria, dependency IDs, affected repos/files/**logical resources** (e.g. schema/protocol/security authority), explicit cloud/native/human gates, isolation branch, resource/usage-risk budget, recovery checkpoint, and integration plan. Split work if it cannot safely reach a reviewable checkpoint within one agent run. Independent siblings may run in parallel only when their dependency contracts and *logical* write sets are disjoint. The same agent may change role between units.

## Claim, exclusive ownership and safe parallelism

1. **Claim before modifying:** announce `TASK_ID, AGENT, INSTANCE/SESSION, REPO, BRANCH, RESOURCE_LOCKS, BASE_SHA, DEPENDENCIES, CLAIM_VERSION, CHECKPOINT_SHA, LAST_ACTIVITY, STATUS` in the shared task ledger and link the corresponding draft PR/issue. A claimed unit is single-writer. Other agents may inspect/review but must not modify that unit, overlapping files or logical contracts.
2. **Atomicity matters:** issue labels/comments and an unversioned markdown edit **are not locks**. A fully automated controller must use an atomic compare-and-swap/transaction with monotonically increasing claim generation (fencing) and cross-repo resource locks. Until it exists, serialize conflicting units via one coordinator and explicit owner-confirmed assignment; do not claim multi-agent exclusivity just because a task says `TAKEN`.
3. **Lease expiry is not transfer:** classify owner state as `ACTIVE / SUSPECTED_STALLED / INTERRUPTED / BLOCKED / RELEASED`. Browser errors, long tests, Codex quota exhaustion or lack of heartbeat do not prove the first worker stopped. Inspect VM/session availability, outstanding PRs and last remote checkpoint; wait or fence the former owner before transferring a write lease. Old workers must revalidate claim generation before another push.
4. **Recovery choices:** `REPORT_ONLY` may inspect the isolated branch/PR without taking write ownership. `CONTINUE` requires an atomic fenced transfer, a new owner/session, reconciliation of last pushed SHA with active environment changes, and a new task log. Never run two independent writers on one unit.

## Durable checkpoints and recovery

- Create a dedicated `work/<task-id>/<slug>` or comparable short-lived branch; open a **draft PR early** where supported. Use small coherent commits and push safe checkpoints **before costly tests, context/usage exhaustion, provider handoff, and whenever a verifiable substep is done**. Record `DONE / IN_PROGRESS / NEXT / TESTS / RISKS`, branch HEAD and the exact unpublished work boundary. Do not wait for full task completion to push.
- Treat **uncommitted/unpushed Codex Cloud VM changes as at-risk**, even if that task's VM may be resumable. A fresh task/agent cannot be assumed to see those bytes. If push is unavailable, record `CHECKPOINT_NOT_REMOTE`; prioritize resuming the original workspace or making a vetted patch from it rather than falsely claiming recovery.
- Never checkpoint secrets, real tokens, private user data, generated binaries, confidential fixtures or other excluded artifacts. Check `git status`, staged diff, ignored paths and secret screening. For public repositories, a checkpoint branch is **publicly visible** too. Unsafe WIP stays isolated until sanitized.
- An interrupted task remains `INTERRUPTED` with precise last durable SHA and missing steps. After transfer, start from the latest verified remote snapshot, not an assumed conversational state. Re-run appropriate regression tests. Only CI-green reviewed work may merge; `MERGED` does not imply `L/U ACCEPTED`.

## GitHub attribution and traceability

- **Owner-preferred identity:** when the repository owner has explicitly authorized it and the environment supports it, set `git config user.name "Kais Abiyyi"` and `git config user.email "93252311+KaisAbiyyi@users.noreply.github.com"` for **new commits in this repository only**. This noreply form is grounded in this owner's existing repository commit metadata; verify it still belongs to the connected owner before use. Never guess addresses or use another person's credentials. If identity cannot be verified or set, disclose `ATTRIBUTION_EXCEPTION` rather than forging authorship. Respect actual contributor/authorship transparency through task/PR records.
- Commit **author**, GitHub **pusher**, **PR creator**, and **squash-merge author** are different. The remote app/bot may remain visible as PR creator/pusher; an agent cannot guarantee its absence from GitHub's contributor UI. Check resulting GitHub author attribution on committed and final squashed SHAs. Do not rewrite verified published history to cosmetically hide collaborators.
- Keep provenance of actual agent work in task/PR metadata (`agent, role, environment, model if known, task ID, execution/checkpoint/test SHA`). Do not silently append `Co-authored-by: Codex` or another AI credit unless expressly requested; do not falsify historical contribution. Prefer owner-linked author attribution for approved new commits where technically supported, with transparent agent execution records.

## Acceptance, reporting and interruption protocol

Use `UNCLAIMED → CLAIMED → RUNNING → CHECKPOINTED → REVIEW → MERGED → (LOCAL_PENDING / USER_PENDING) → ACCEPTED`; additionally `BLOCKED / INTERRUPTED / FAILED / ABANDONED` with explicit evidence. Preserve the project's existing `PASS / FAIL / PARTIAL / BLOCKED / NOT RUN / HOLD` gate vocabulary. Follow dependency edges: do not begin a blocked dependent unit merely to keep workers busy; select another safe ready unit.

Each handoff must give: `PROJECT, REPO(S), TASK_ID, CLAIM_STATUS/OWNER/GENERATION, ROLE, BASE_SHA, BRANCH, CHECKPOINT_SHA, PR, IMPLEMENTED, TESTS/CI, LIMITATIONS, LOCAL_REQUIRED (L-ID or NONE), HUMAN_REQUIRED (U-ID or NO), NEXT, RECOVERY_INSTRUCTIONS`. On failure report what is **not** saved remotely. Agent reviewers cannot mark device/browser/SketchUp/account/UI gates passed from cloud-only simulation. The human receives consolidated decisions and qualified test instructions, not continuous micro-approval requests.

## Mandatory Notion synchronization at accepted handoff

- **No completion handoff is finished until Notion is updated and read back.** Once PR acceptance/merge evidence is verified, immediately set the task database `Status = Done` and clear its active claim; record merged SHA, exact-head CI, reviewer and known limits. If not accepted, keep `Review` or `Blocked` with the real reason—never announce completion from tests alone.
- Maintain the Project Hub's **00 · Current State** by updating the actual database `Focus Lane` fields: up to five last `Done` tasks, only currently eligible nonconflicting `Now` tasks, and the single next eligible dependency wave as `Up Next` (which may remain blocked). A native `L` gate becomes `Local Gate` only when prerequisites are accepted, never `Done` from cloud CI.
- Verify the displayed Notion board after updates; cite the task page/PR and state any connector failure plainly in the same response. **Do not end a coding turn with a completed GitHub merge and a stale Notion `Now` record.** Avoid another manually maintained status snapshot in the Project Hub page body.

## Repository-specific qualification boundary

CL-L1 and CL-04A/B/C have now passed their recorded qualified gates; the next proposed independent cloud wave is CL-05A, CL-06A, CL-07A, CL-08A and CL-09A after live checks. Protect historical uncommitted D: WIP; phases 1–6 remain partially accepted.

Current phase/dependency evidence: [CURRENT-PHASE.md](CURRENT-PHASE.md). This policy does **not** claim any existing phase as complete or start/stop another agent. No task registry, lock controller, checkpoint automation or agent dispatch API is installed by this docs change; these are enforceable operating requirements for agents and a specification for a future controller.

