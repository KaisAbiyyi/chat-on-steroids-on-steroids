# 7D Cloud — authenticated direct debugger lease and frame-bound snapshots

- **Unit:** 7D — Direct browser tools/debugger/frame ownership.
- **Claim:** ChatGPT GPT-6 Cloud 2026-10-23; exclusive `browser-tools` lock, `work/7d-direct-browser-tools-20261023`.
- **Baseline:** accepted integrated 7L1 PR #51, merged main `83da1f1ce7c5116314e1b658a03831b461cf97ef` (native tested code `2590decb7c618ace066795c2aeb6b4e5e2612405`).
- **Competing documentation:** PR #55 separately normalizes 7L1 status, task DAG and board. Rebase/reconcile before acceptance; do not silently overwrite its facts.

## Cloud implementation boundary

The new `cos_core::browser_tools` module is a **native integration contract**, not a claim that a full browser product UI or an installed provider is wired up.

1. `DirectBrowserTools` enforces an exclusive debugger lease for each host-proven profile+tab. A lease is additionally bound to exact tab/frame/document/session identity, host generation, authenticated 7B channel and non-reusable handle.
2. Every attach, snapshot and detach calls `NativeBrowserBridge::authorize_current` with a fresh MAC-protected full protocol-14 identity frame and strictly advancing channel sequence. A guessed handle or page/content-script message cannot grant ownership.
3. Native implementation supplies `BrowserDebugger`: its attach, detach and capture operate only on privileged native browser events/targets. A captured snapshot is compared to exact current trusted document+frame and origin after acquisition; title, URL and text have explicit byte budgets. A mismatched capture is rejected and its lease retired when debugger cleanup succeeds.
4. Native lifecycle must explicitly invoke `retire_from_native` for navigation, tab close, worker retirement and debugger detach. Stale retirement cannot revoke a replacement owner. A failed native detach holds the lease rather than granting a concurrent debugger.
5. No arbitrary CDP/JS evaluation, clicks, provider Send, session transfer, automatic input or account access. The separate 7L2 physical native acceptance will exercise a real backend, teardown and duplicate-input boundaries.

## Test matrix and acceptance

- Rust tests: exact owner attach/snapshot/release; cross-profile, cross-tab and cross-frame denial; guessed handle; MAC replay/host mismatch; malformed/oversized/cross-origin debugger snapshots; stale retirement/replacement generation; attach and detach runtime faults.
- `.github/workflows/7d-browser-tools.yml` runs targeted `cos-core` regressions on **Windows and Ubuntu**, and formats and Clippy independently of the already existing workspace Rust CI.
- `task-graph.yml` plus the existing all-workspace CI are required for acceptance. Report only actual exact-head statuses; mark NOT_RUN otherwise. GitHub-hosted Windows is not a native WebView2 qualification.
- **Review:** direct critical self-review by the implementor against exact SHA and the actual PR diff (including security, dependency, concurrency, resource release, and native boundary). No delegated reviewer prerequisite.
- **Merge:** only after all source Cloud gates pass and PR #55 conflict is reconciled. A legitimate native runtime adapter and real browser tool lifecycle are explicitly gated by 7L2.
