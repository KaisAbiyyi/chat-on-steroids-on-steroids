# ADR 2026-10-18 — Mind Place engineering discipline, CoS-Rust local-first

**Status:** Accepted by user direction, 2026-10-18 (Asia/Jakarta).

## Context

CoS-Rust had progressed through many source parity checkpoints for phases 1–6. The longest recent agent run ended while wiring recorder captions, privacy transforms and runtime evidence. The latest committed scope was tested, but its new dirty files were unfinished, and the Git checkout had no configured remote. Historical implementation ledgers contain extensive receipts, yet no short current unit pointer or root `AGENTS.md` made the next agent's behavior unambiguous.

The user explicitly paused that feature implementation and requested the same engineering process used by Mind Place: structured units, security and performance discipline, efficient maintainable code, complete documentation, source traceability, test gates, professional Git history and cross-agent handoff. The sole fundamental workflow difference is **development location**: Mind Place is cloud-first, while CoS-Rust remains local-first.

## Decision

1. Implement, review, benchmark and commit CoS-Rust in the local Windows workspace on **D:**. Native Windows proof is the primary target. Linux qualification is supplementary; macOS stays on hold. GitHub is for eventual PR/CI sync, not an assumed active development environment.
2. Use one bounded named implementation unit at a time, with failure/success contract, tests, security/performance gates, review, SHA and next-unit handoff. Split risky or oversized units rather than accumulate long unreviewed changes.
3. Use `AGENTS.md`, `docs/CURRENT-PHASE.md`, `docs/BIG-IMPLEMENTATION-PLAN.md` and `docs/DEVELOPMENT-WORKFLOW.md` as durable source-of-truth instructions for future agents. Keep dated plan/summary/review and audit receipts as historical evidence. Memory in a chat alone cannot qualify a build or a decision.
4. Preserve the currently dirty recorder work unchanged during this governance task. The next feature unit begins only with a separate explicit instruction; priority is privacy before caption evidence.
5. Stage documentation paths only for this decision and make a dedicated `docs:` commit. Existing qualified commit history and older receipts remain intact. An independent whole-scope review remains open.

## Consequences

- Agent prompts can use an exact ID (`CL-01`, `7A`, etc.). Every stage knows the current file owners, validation, limitations and the following handoff.
- The project may initially have more documentation boundaries, but each one prevents the costly re-analysis and mixed-success status observed in long running migration attempts. Keep high-level plans flexible and evidence specific.
- The local checkout is the only verified Git authority until a remote is explicitly configured. Remote CI status remains `NOT RUN` in reports even if `.github/workflows/ci.yml` exists.
- A docs-only governance commit proves documentation hygiene; it does not validate the uncommitted recorder implementation or promote any partial Rust phase to complete.

## References

- [Project rules](../../AGENTS.md)
- [Current unit and preserved WIP](../CURRENT-PHASE.md)
- [Execution roadmap](../BIG-IMPLEMENTATION-PLAN.md)
- [Development and Git workflow](../DEVELOPMENT-WORKFLOW.md)
- [Historical phase 1–6 closure summary](../implementation-summary/2026-10-17-phase1-6-parity-closure-v1.0.md)
