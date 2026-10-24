# PERF-04 — idle process and browser-worker lifecycle contracts

**Claim:** COS-PERF-04-20261024-1736. **Environment:** GitHub cloud only. **Issue:** [#99](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/99). **Prerequisite:** PERF-00 accepted in PR #105.

## Implementation

- Central `IdlePoll` schedules owned-process and renderer-PTY child-exit observation at 10, 20, 40, then 80 ms; once idle it remains at 80 ms. Previous fixed 10 ms loops performed up to 100 polls/second **per** process/PTY. The new steady-state schedule permits at most 12.5 polls/second per watcher (theoretical cadence, not measured CPU). Immediate exit receipts can now be delayed by up to one 80-ms polling interval; security ownership, process-tree cleanup and final output-drain barriers remain unchanged.
- Native browser lifecycle controller now has a privileged, idempotent `dispose()` that invalidates all tracked scopes, bumps permission versions, and releases cached partition ownership on host teardown. In-flight permission authorization cannot issue a successful receipt after disposal.
- MV3 verified companion now offers explicit `dispose()`, optionally unsubscribes from the authenticated native event source, refuses *new* events after disposal, and never deletes its persisted intent/ACK/repair ledger. An already-admitted dispatch is allowed to finish its durable write.
- Chrome worker adapter `dispose()` calls companion shutdown, frees cached frame fencing and active sets, and fails closed for future or racing script-injection receipts. Fixed inert extension files and native host authentication rules are unchanged.
- Existing terminal output flow control remains bounded by a 256-Ki UTF-16 outstanding ACK watermark with 64-Ki resume watermark; existing process output retention uses `HeadTail` with 1-MiB per-process unread head/tail budget. The implementation deliberately does **not** reduce those envelopes without measured provenance or weaken ACK custody.

## Tests and acceptance

- Rust unit tests: backoff progression, max delay, independent fresh process latency.
- Existing Windows/Ubuntu `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` and release build.
- Browser Node tests (both operating systems): host teardown, post-teardown tracking/permission denial, racing native permission grant, worker unsubscribe and persist-before-ACK, racing static injection completion.
- Existing `8A`, `8C`, `7C` and terminal backpressure tests must still pass. No physical native memory data or provider accounts were accessed.

## Deliberate boundaries

- Native Windows 2h+ endurance, whole-process RSS, steady-state idle CPU, and real tab retirement are **NOT_RUN**. Those are PERF-05/owner physical gates.
- Inactive individual tab eviction is **not** implemented by arbitrary browser input: trusted host shutdown is the only new in-memory disposal signal. A future per-tab retirement API must authenticate native event sequence and preserve stale-tab-generation fences before reclaiming those entries.
- No numerical lower-RAM guarantee is asserted from an idle polling cadence change. The performance baseline and actual memory effects must be measured with the PERF-00 harness.

**Review gate:** exact final SHA CI on Windows and Linux; implementor self-review code, security, process custody, output receipts and dependency boundaries. Merge only when evidenced PASS.
