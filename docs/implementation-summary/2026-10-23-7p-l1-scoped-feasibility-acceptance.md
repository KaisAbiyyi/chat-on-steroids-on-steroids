# 7P-L1 — physical Windows browser feasibility acceptance (scoped)

Date: 2026-10-23 (Asia/Jakarta).

## Decision and source

The owner resumed the previously deferred 7P-L1 and explicitly requested that
the two umbrella tests which the *synthetic* WebView2 host left `NOT_RUN` be
**executed**. The physical-Windows, separately launched
`probes/7p-b/tools/qualify-native-7pl1.ps1` is the authoritative **browser
feasibility** L-gate receipt. It is deliberately distinct from the raw 7P-B
synthetic browser test receipt, which cannot approve its own native acceptance.

Exact browser/installer source:
`53164da79b32b4a4cacb433678a94ec13de57888`, reviewed and already merged
by [PR #42](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/42)
into main `ae14311c587e68b49cd4d26ad4f4d498248d5259`. The code contains
the independently reviewed browser-profile callback binding, current controller
identity, stale CDP refusal, and D: package/reparse containment fixes.
This separate L-gate runner and the revised scheduling contract are subsequently
submitted for source review and CI, then merged before task-status acceptance.

## Executed physical Windows evidence

- Host Windows 11 x64 build 26300 with system WebView2 `155.0.4283.62`.
- Launch a **fresh installation** from the actual unsigned offline synthetic
  setup on D:, verify its embedded archive and the packaged SHA-256 ownership
  manifest, compare the installed `WindowsHost.exe` against the pinned exact
  source binary SHA and launch **that installed executable** with the **bundled
  renderer and extension from the same installation**, never development assets.
- In a fresh synthetic profile, run combined `--batch --native-idle --native-auth`
  including actual MV3 natural idle of 65 seconds, extension debugger
  attach/event/detach/restart, profile A/B controller security, mock OAuth
  callback consent/denial/replay, tab close/reopen and navigation revocation,
  rich renderer/KaTeX/PDF/xterm behavior.
- Observe the native child process exit directly; require exit code zero,
  exactly 26 named PASS checks, no FAIL/UNVERIFIED, no unknown check/status.
  The raw host's two `NOT_RUN` umbrella entries are evaluated separately
  within this L gate; they are never silently rewritten.
- Verify SHA-pinned earlier 3 cold and 3 warm installed-probe full-batch
  measurements, size manifest, and recorded runtime/working-set samples.
  The candidate synthetic probe installer ceiling is 80 MiB and synthetic
  payload 200 MiB. These are **candidate architectural footprint envelopes**,
  not approved production release budgets.

| Check | Scoped L-gate result / measurement |
| --- | --- |
| `distribution_budgets` | **PASS — SYNTHETIC FEASIBILITY ONLY** |
| `native_windows_gate` | **PASS — PHYSICAL WINDOWS EMBEDDED BROWSER FEASIBILITY** |
| Actual installer size | 68,009,984 bytes, pinned SHA `90CB1FAB5436BCDA01F431A1E998692634DD90BC2EDA1DC450465C4A62295C34` |
| Installed package payload | 167,789,479 bytes; 84 manifest files |
| Source-bound historical cold/warm full-batch medians | 3,992.6 ms / 3,571.4 ms (3 runs each) |
| Historical max sampled WinForms host working set | 67,379,200 bytes (not unique/private WebView2 RSS) |
| `production_release_budgets_16L3` | **NOT RUN — different Phase 16 product gate** |
| Real provider sign-in `7U1/8U1` | **HOLD — separate owner consent** |
| Real physical OS IME `IME-DEFER-20261022` | **HOLD — previously deferred by owner** |

No real accounts or personal browser profiles were accessed, and no automatic
provider Send occurred. Prior native Windows editor keyboard automation passed
17 exact assertions; owner-observed synthetic KaTeX clipboard/Notepad and
limited Narrator controls remain supported by their original independently
retained receipts. The user-directed OS IME deferral remains in force.

## Limitations and follow-up

The raw host always has `native7PL1Accepted=false` since it is only the
synthetic sub-probe. Its separately executed Windows L-gate receipt is the
source for the **scoped** `native7PL1Accepted` flag. This accepts early
WebView2 extension and renderer feasibility only; it does **not** prove an
integrated Rust/WebView2 UI, actual bridge transport, genuine signed-in provider
flow, product installer, code signature, release time-to-interactive, private
RAM, or Windows product UX. All latter capabilities remain distinct tasks:
native `7L1` for actual Rust bridge+MV3 companion, `7U1/8U1` for account
consent, `8L1`/Phase 15 for the explicitly deferred OS IME, and `16L3` for
full Windows release/startup/size budgets.

The historical benchmark script did not record process exit codes for each
of its six runs; it did record SHA-bearing no-FAIL receipts and nonzero
resource samples. The **new complete native browser test's** exit code is
observed directly, while the historical six-run ledger is cryptographically
pinned to avoid accidental/unreviewed substitution. Neither historic batch
timing nor working-set sampling is represented as product time-to-interactive
or private RSS.

The separate native receipt contains source, installer, native output and
benchmark hashes and independently evaluated named checks. Raw local
evidence is retained exclusively on the approved D: task root at
`D:\projects\CoS-Rust\.test-tmp\7p-l1\closure-live-20261023`; the
normalized source-bound receipt is
[`audit/2026-10-23/7p-l1-native-feasibility-acceptance.json`](../../audit/2026-10-23/7p-l1-native-feasibility-acceptance.json).
The **post-fix installed-bundle test receipt** is
`native-feasibility-package-proof.json` (SHA-256
`46258595A8808E1DEFAB40CBA1B96C2ADBDD45027CCB485286015B1A3C4D207C`);
its freshly observed real WebView2 raw receipt is
`runner-native-package-proof/receipt.json` (SHA-256
`AAE99B56DDF1A6D33656CA0CABC3E281E2911139A8C0D0496FA8CE4A8DC68E28`).
The installed Windows executable returned **exit 0, 26 PASS, 0 FAIL** using
the renderer/extension shipped inside the exact offline package. The acceptance
script's D: path-reparse refusal, installed-EXE and archive hash pins, raw
unexpected-status refusal and scope labeling received independent read-only
review with **no remaining proven P1/P2** for this synthetic feasibility gate.
Local source contract validator PASS; 15 .NET synthetic policy assertions PASS;
`git diff --check` PASS.
Only after reviewer approval, exact-head CI and merge may the canonical DAG
mark `7P-L1` Done and make native `7L1` an unclaimed ready candidate.

The earlier [tracking issue #47](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/47)
records why splitting this early feasibility gate from the later production
release proof is necessary to avoid a phase 7→16 dependency cycle. It must not
be closed before that scoped contract change is reviewed and merged.
