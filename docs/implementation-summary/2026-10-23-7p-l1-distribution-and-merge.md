# 7P-L1 — Windows synthetic distribution qualification and PR merge reconciliation

**2026-10-23 / scope: local Windows synthetic probe only.** Exact tested
source `c1d6234b21fc794ecefb36244caa73b0d55ae465`, built as the
informational version of installed `WindowsHost.exe`; Evergreen WebView2
`155.0.4283.62`. This source includes an isolated merge of cloud `main`
`d9a56ef93546b42e9043fc72f918ffcb1c983b62`. The only textual conflict
was competing status blocks at the start of `docs/CURRENT-PHASE.md`; both
histories were retained. Worktree `.test-tmp/7p-l1/worktree` and the original
main checkout's recorder/caption WIP were not reset or modified.

## Distribution and runtime evidence

The unsigned offline synthetic-only installer is
`D:\projects\CoS-Rust\.test-tmp\7p-l1\installer\release-c1d6234\CoSoS-7PL1-Synthetic-Probe-Setup.exe`:
**68,008,448 bytes**, SHA-256
`BEAE09729D16FA667816908F7EA82A909B128156B50BF457D148159F52D58997`.
It packages 82 manifested files, requires previously installed Evergreen
WebView2, and excludes product signing/MSI/Rust GUI. Installation to the
owned D: destination and launch of its packaged host succeeded; the exact
executable SHA-256 is
`DBE969C34B898595B31266950738B10D365F3D44392AC1659641BA84FAFE9886`.
Installed files total **167,787,649 bytes**. The owned-file uninstaller
completed and removed that installation without deleting or changing
unowned files.

Six installed-host runs (three new/cold synthetic profile roots and three
warm reuses of the same isolated profile) were nonfailing. Median full
**batch wall runtimes** were **2,817.3 ms cold** and **2,649.4 ms warm**.
Observed host peak *working set* during sampling ranged from 63,385,600 to
68,444,160 bytes. Separately sampled WebView2 subprocess working-set sums
were approximately 1.175–1.245 GB; summing working sets does not measure
unique/private RAM. Batch runtime measures full synthetic fixture tests and
shutdown, **not cold-start-to-interactive**. Source-bound raw receipt:
`D:\projects\CoS-Rust\.test-tmp\7p-l1\benchmark-c1d6234\measurements.json`,
SHA-256 `A61F018836EC4EC9E3EC3E511FD69B295F63BAE8C8A31676B008EFC497C7E273`.

## RED → GREEN integration regression

The first native merged-source `--native-auth --native-idle` run on commit
`d370d6d` was **22 PASS / 4 FAIL / 2 NOT RUN**. Consent page title
became visible before its external script installed click handlers; both
approval and denial attempts could occur too early. That failed raw receipt
is preserved at `.test-tmp/7p-l1/combined-native-d370d6d/receipt.json`.
The fix at `c1d6234` uses a page-owned readiness signal set only after
handlers are installed, and the native host waits for it before clicking.
Three isolated quick retests PASS. The recompiled **installed** host's full
`--batch --native-auth --native-idle` run then returned **exit 0: 26 PASS,
0 FAIL, 0 UNVERIFIED, 2 NOT RUN**. MV3 natural idle, debugger reattach,
tab recreation, stale/cross-profile CDP lease denial, synthetic OAuth
callback/state/replay, and consent refusal all PASS at the same source SHA.
Raw receipt:
`D:\projects\CoS-Rust\.test-tmp\7p-l1\combined-native-c1d6234\receipt.json`,
SHA-256 `0C4AEB04CCDB53E9E9A37102F6A9D242674D09110BF4A58166D8583AE7480205`.

## Acceptance boundary

These measurements qualify **synthetic Windows feasibility only**. The
host's `distribution_budgets` and aggregate `native_windows_gate` remain
`NOT_RUN` for the actual production desktop, because production bundle,
signing, user-facing time-to-interactive, private RSS baseline, and release
thresholds are unavailable. `native7PL1Accepted=false` remains correct.
Provider account authorization/consent remains a later owner gate; physical
OS IME is deferred under `IME-DEFER-20261022`. Consequently **7P-L1 is
PARTIAL / NOT ACCEPTED**, and native 7L1 remains blocked. PR #42 can be
made conflict-free while staying draft until its native acceptance contract
is explicitly resolved. No user action was needed to run the synthetic tests.

The normalized tracked receipt is
[`audit/2026-10-23/7p-l1-windows-distribution-after-merge.json`](../../audit/2026-10-23/7p-l1-windows-distribution-after-merge.json).
