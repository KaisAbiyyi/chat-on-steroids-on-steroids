# Implementation plans

- [Phase 1–6 parity closure v1.0: current Windows scope and live ledger](2026-10-17-phase1-6-parity-closure-v1.0.md)
- [Phase 4–6 v1.0: executable scope, review focus and execution ledger](2026-10-17-phase4-6-implementation-v1.0.md)
- [Original 16-phase Rust migration roadmap v1.0](2026-10-16-rust-migration-phases-v1.0.md)

The current executable task DAG is [../task-registry.json](../task-registry.json); see [../PARALLEL-EXECUTION-BOARD.md](../PARALLEL-EXECUTION-BOARD.md) and run `python tools/task_board.py ready --json` to see prerequisite-eligible, **unclaimed candidates**. Only the execution ledger can mark tasks accepted after verified CI/native/human gates.

**Execution numbering update:** the former early-Phase-8 architecture feasibility work is **Phase 7 preflight `7P-A / 7P-B / 7P-L1`**. Full Phase 8 begins at `8A` only after Phase 7. Archived snapshots preserve their original IDs for provenance and are not valid current task claims.

Plans define scope. Passing implementation checks do not replace each original exactness gate.
