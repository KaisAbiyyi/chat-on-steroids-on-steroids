# 16D — Source-bound cloud headless startup/RAM/size gates

Task: [Notion 16D](https://app.notion.com/p/400c4a6aaf73812398abe69856b01b70) · [Claim #116](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/116)
Predecessor: 16C [accepted bundle](2026-10-24-16c-bundle-licenses.md), merged `326b0fca66c8deefce85161e298ff20262a57358`. Resource lock: `release-perf`.

## Scope, metric and evidence

`tools/release_16d.py` measures the **real release-mode Rust `cos-rust` headless executable**, bound to the exact checked-out GitHub SHA by the 16C source-pinned ZIP. It verifies the bundle, archive SHA256 and file sizes and checks that the runtime executable is byte-for-byte the binary recorded by the trusted 16C bundle index. The executable is invoked only as `--data-dir <fresh synthetic temp profile> info`: no user accounts, private profiles, remote providers or owner API credentials. No stdout or profile contents are retained in receipts.

On each Windows/Ubuntu CI runner:
- Three new-profile launches: wall clock from process spawn to validated CLI JSON completion (this is **not** GUI start-to-interactive or OS cold boot).
- One excluded primer and three reused-profile launches, measuring the same CLI completion.
- OS process **peak resident/working-set of root CLI only**, sampled using `/proc/<pid>/status:VmHWM` on Linux or `GetProcessMemoryInfo(PeakWorkingSetSize)` on Windows, including a sample count. Missing root RAM observations fail closed.
- Bundle and binary bytes/SHA256, p50 (median), p95 and individual measurements, source SHA and platform. No environment IDs, paths, provider content, user profiles or command lines in the artifact.

The 16C archive safety cap remains a **package correctness guard**, not an empirically tuned performance target. `release_performance_budget=NOT_FROZEN`, `original_cos_comparison=NOT_RUN`, and native/product acceptance remain `NOT_RUN` by construction. GitHub-hosted runner variance, OS cache and background pressure prevent fair cold-start comparisons or claimed RAM savings. Repeated runs may be reported but cannot establish competitive budgets, UI memory, GUI startup or release-wide regression limits.

## Validation and handoff

`python -m unittest tools.test_release_16d tools.test_release_16c -v` checks positive fixture and negative source, platform, artifact digest, receipt distribution, forged budget/native PASS and absent memory cases. `.github/workflows/16d-release-perf.yml` builds `cos-app` release on `windows-latest` and `ubuntu-latest`, regenerates/verifies the 16C bundle and 16D receipt, and uploads bounded JSON evidence for 14 days. Standard Rust CI separately runs format, strict Clippy, all tests and full release build.

Acceptance for 16D requires this cloud implementation's exact-head CI and self-review, then eligible merge. Physical Windows `16L3`, native GUI/browser lifecycle, matched PERF-01/PERF-05 and final PERF-06 statistically grounded budgets **remain separate gates**. Never promote this CLI receipt to a native product PASS.
