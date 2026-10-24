# PERF-00–06 — early performance-first execution plan

Authority: [2026-10-24 product ADR](../decisions/2026-10-24-performance-first-workstation.md). Canonical AND graph: [task registry](../task-registry.json); source status: live Notion/GitHub. PERF is a P0 cross-cutting release initiative, encoded as Phase 16 IDs but starts before final release.

| Unit | Environment | Dependencies (all) | Exit criterion |
|---|---|---|---|
| PERF-00 | Cloud | 15G | Source-bound workload harness + process-tree measurement schema, isolated fixtures; no unsupported benchmark claims |
| PERF-01 | Local Windows | PERF-00 | Real CoS original matched idle/1-chat/multi-chat/coding/browser RAM, CPU, startup and memory-over-time baseline |
| PERF-02 | Cloud | PERF-00 | Probe designs for existing external browser reuse vs opt-in on-demand embedded WebView2, lifecycle and authority fences; native mode decision deferred until receipts |
| PERF-03 | Cloud | PERF-00 | Paged visible chat, bounded history and IPC/cache, lazy heavy rich panels, cheap grouping metadata, idle work regression tests |
| PERF-04 | Cloud | PERF-00 | Worker/tab/process/PTY ownership + shutdown/retire cleanup and bounded terminal output, event-driven/adaptive idle tests |
| PERF-05 | Local Windows | PERF-01/02/03/04 + 8L1 + 15L3 | Actual production host 2h+ workload/endurance native RAM/CPU/latency, no growth/leaks/ownership loss |
| PERF-06 | Cloud | PERF-05 + 16D | Budget freeze with measured p95/variance and CI/native regression policy/exception handling |

Now: PERF-00 is eligible **only as an unclaimed candidate**; live resource lock and CI check before claiming. 7U1-PREP is another independent candidate; 16A ready and 15L3 Local Gate remain separate truthful statuses. Up Next after PERF-00 includes PERF-01..04 if other prerequisites and locks pass. No manufactured native PASS.

Benchmarks: fixed Windows machine/build/power, original version, profile class, same user/code workload and chat/worker count; cold/warm app and browser; 0, 1 and many chats; larger saved history, rich panels, active terminal/agent, background idle and termination. Measure **all** relevant process trees including reused browser attribution. Include idle CPU, working set, private bytes, peak, growth over time, tab/worker marginal memory, start-to-interactive, p50/p95 tool latency, process/handle leaks, crash/restart. Distinguish disk package, memory sample, and actual full-product runtime.

Tentative goal: >=50% matched total RAM saving against CoS; idle CPU <=1% aspirational; startup <=2s aspirational. These are **not achieved** and not yet qualified release limits. Security, file/Send semantics, data integrity and owner account consent remain inviolable. Windows physical work stays on approved D: test worktrees with synthetic data unless explicit account consent. Cloud work never touches local files.

Performance levers to investigate, not assume: redundant browser renderer duplication, WebView2 always-on instances, worker idle tabs, expensive listeners/polling, eager history projections, unbounded caches/terminal logs, repeated serialization and rich-panel preloads. Mind Place is an independent product, not a shared memory store.

Optional features: metadata-only chat grouping performance-gated; pets, dictation, broad plugins, full original browser chrome, visual-perfect 13-language feature parity and non-coding multimedia deferred. Keep proven existing interfaces safe until separately reviewed removal.
