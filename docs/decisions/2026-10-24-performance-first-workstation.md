# ADR — performance-first ChatGPT coding workstation (2026-10-24)

**Owner-approved P0 product direction.** Supersedes only the previous mandatory full CoS feature/visual parity objective; cloud-first discipline, state/security contracts and accepted receipts remain intact.

## Product
CoSoS is a lean, reliable, high-use Codex-like coding workstation using ChatGPT as provider and Rust as authority for sessions, tools and processes. **Mind Place remains isolated** (no shared archive, graph, memory, project intelligence or mandatory integration). Optimize memory, CPU, startup, long-running robustness and coding latency with evidence. Small binary/payload is not evidence of small runtime RSS.

**Coding-critical:** authenticated provider/browser bridge, MCP, files/search/patch/edit, Git, terminal/PTY, scoped approvals, durable session/Send identity, safe bounded workers/worktrees and recovery. Cheap lazy project chat grouping is permitted, no eager chat scanning or history indexing.

**Optional or deferred, not forced from CoS:** pets, overlays, full embedded multi-tab browser, rich PDF/media outside coding workflows, broad plugin marketplace, thirteen languages/visual-perfect parity, dictation, advanced Goal/Loop. Keep already-implemented functionality dormant as appropriate; deletion/migration requires a separate reviewed decision.

## Performance hypotheses, not proven bottlenecks
Electron + duplicate browser runtime; multiple active ChatGPT tabs/workers; eager history/re-render; excessive background polling; duplicate IPC state and buffers; plugin/overlay lifetimes. Existing audit reports CoS 2.1.39 Electron/Chromium as 63.44% of **disk payload**, not runtime RAM. Native CLI 6.73 MiB working set and 15G 192 KiB synthetic serialized snapshot likewise do **not** measure full app RAM.

## Measurement protocol
PERF-00 reproducible harness; PERF-01 physical CoS original baseline; PERF-02 instrument external-browser reuse versus lazy WebView2; PERF-03 lazy bounded UI/history/cache; PERF-04 lifecycle/buffer cleanup; PERF-05 Windows 2h+ full-product endurance (after 8L1 and 15L3 native product gates); PERF-06 measured release gate with CI/native checks (after 16D). Exact source, binary, machine, workload, warm/cold, power mode and consent. Count Rust, renderer, WebView2 children, worker/tabs, optional tunnel/sidecars and attributable external browser memory. Record private bytes, working set/peak/trends, CPU idle/active, p50/p95 response, startup, tab+worker increments, recovery and storage separately. Compare equivalent workloads, not CoSoS foreground only vs all CoS processes.

**Provisional aims, NOT acceptance evidence:** >=50% reduction in matched total memory vs CoS original; idle CPU near 0% (aspiration <=1%), start-to-interactive <=2s when possible, no unbounded 2h memory growth. Freeze actual budgets at PERF-06 from real receipts. Performance cannot override privacy, durability, authority, no duplicate Send, tool side-effect safety or current owner gates.

## Sequencing and status
PERF-00 depends on accepted 15G and is an **unclaimed cloud Ready candidate**. Existing 7U1-PREP must complete before owner 7U1; 8L1 native host and Issue #85 real renderer remain blocked. 15L3 physical RAM testing is NOT_RUN (not proven by synthetic UI). 16A migration remains independently Ready; 16D remains behind 16C. All tasks remain on the universal unlimited Now and direct-successor-only Up Next board policy. Never auto-claim from a proposed roadmap.

Historical 16-phase assessment and receipts remain accessible with their dates; future decisions choose feature scope, not silently rewrite historical experiments.
