# 8L1 physical Windows qualification — BLOCKED

## Identity and outcome

- UNIT: `8L1`, Codex local session `COS-8L1-20261024`, generation 1; [claim and repair handoff #71](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/71).
- GITHUB SHA UNDER TEST: `eb9f3dc9684fe97017dcc3a57ae63553d2118547`, merged 15A [PR #67](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/67). Branch `feature/8l1-native-20261024`.
- Prerequisites: 8B [PR #61](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/61), merge `1ecc3ba388997d51eb2c31fe467431b3228e327e`; 8C [PR #65](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/65), merge `c7d4fabc28ea991a2cf9f509273cf9bb6e2ff3db`. Both merged with successful exact-head checks. This receipt reconciles their stale registry entries and 15A's merged status; it does not expand their accepted cloud scope.
- MACHINE/OS/DRIVE: physical Windows NT 10.0.26300, NTFS D:, isolated `D:\projects\CoS-Rust\.test-tmp\8l1\worktree`. All new build, profile, subprocess temp and evidence outputs are under the task's D: directory. NuGet reused the existing D: `issue49-nuget` cache; Cargo downloads used the existing D: `cl-l2/cargo` cache. No account sign-in or existing browser profile was used.
- STATUS: **BLOCKED; `native8L1Accepted=false`**. The qualification investigation and supporting checks ran; the required native integration is absent. This is not a completed/accepted 8L1.

## Reproduced integration gap

The accepted [8C contract](../../extension/adapter/BROWSER-LIFECYCLE.md) explicitly describes a synthetic host-side integration contract, without native permission handlers, process events, storage partitions or persistent recovery. [8B](../../extension/adapter/PROFILE-SESSION.md) similarly delegates storage isolation and one-use consent to a privileged native host.

The tracked call-site inventory finds definitions and synthetic-test callers only for `createChromeWorkerAdapter`, `createProfileSessionController` and `createNativeBrowserLifecycleController`. Neither native host in `probes/7l1` or `probes/7p-b`, nor any production Rust caller, constructs these controllers. The existing 7L1 host implements its separate Rust/MV3 handshake and CDP lease; its `ProcessFailed` handler revokes that 7L1 lease. This does not instantiate or exercise the 8B/8C lifecycle, profile or permission authority. There is no 8L1 native executable runner.

Reproduce the source audit at the exact SHA:

```powershell
git grep -n -e createChromeWorkerAdapter -e createProfileSessionController -e createNativeBrowserLifecycleController -- crates extension probes
git grep -n -e PermissionRequested -e ProcessFailed -e resolveNativePartition -e verifyNativeLifecycleEvent -- crates extension probes
```

Consequently, native clean start/restart, multi-tab/profile preservation, crash/rebind/stale-proof refusal and atomic permission handling cannot currently be exercised against the accepted 8B/8C controllers. Passing their injected-callback tests cannot satisfy this physical gate. No product feature was implemented during this L session.

## Observed validation and limits

| Check | Result and boundary |
| --- | --- |
| JS 8A/8B/8C + 7C/7E regressions | **56 PASS, 0 FAIL, 0 skipped**, Node 24.21.0 on physical Windows; synthetic callbacks only |
| Task-board unit regressions | **14 PASS**; `python -m unittest discover -s tools -p test_task_board.py` |
| DAG consistency | **PASS**, 115 tasks / 72 C / 28 L / 15 U, no cyclic or dangling edges |
| Rust formatting | **PASS**, `cargo fmt --all --check` on the source under test |
| Local `cos-core` regressions | **63 PASS, 0 FAIL, 0 ignored**; `cargo test -p cos-core --locked`. Full workspace strict check still running |
| Current 7L1 WebView2 host build | **PASS**, 0 warnings/errors, Release build with warnings as errors; separate preflight executable |
| Physical WebView2 MV3 discovery | **PASS**, preflight process exit 0, independent live extension worker sessions; no Rust or 8B/8C integration started |
| 8L1 native acceptance | **BLOCKED / NOT_RUN**, missing privileged integration and gate runner |
| Real provider account, OS IME | **NOT_RUN** for 8U1; IME remains owner-deferred **HOLD / UNVERIFIED** |
| Performance / whole-product acceptance | **NOT MEASURED / NOT_RUN**; no product startup, RAM, installer or parity claim |

The [machine-readable receipt](../../audit/2026-10-24/8l1-native-integration-blocked.json) embeds the **unchanged** raw preflight receipt. Its `sourceSha` remains `NOT_RUN`, `preflightOnly=true`, and `native7L1Accepted=false`. The enclosing receipt identifies this session's local source/build hashes as supplemental provenance; it does not pretend the preflight ran the integrated source-attestation gate or 8L1. Local raw logs and fresh synthetic profiles are retained under `.test-tmp/8l1`; their hashes are recorded.

Cloud prerequisite checks observed through GitHub: PR #61 18/18 successful checks, PR #65 22/22, PR #67 19/19. A successful main Rust run on the SHA under test is [40053693894](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/40053693894). Later same-SHA reruns were still running at first inspection; no unfinished check is called PASS. The new receipt PR requires its own exact-head CI and direct self-review before merge.

## Direct self-review and handoff

Self-review — Codex local / `COS-8L1-20261024`: check source caller inventory, native/synthetic attribution, raw receipt integrity, all changed coordination paths, authentic prerequisite SHAs, absence of product edits, and protected WIP before committing. No independent reviewer or Copilot gate is imposed. Review of this documentation cannot change native acceptance.

LOCAL REQUIRED: rerun **8L1 on the exact accepted repair SHA**, using an executable native host adapter and source/binary manifest. Cloud repair must wire privileged context and actual WebView2 storage partitions to 8B, authenticated navigation/process/crash/restart/close/rebind events to 8C, and atomic one-use consent/current-context checks to real permission handling. Supply bounded synthetic native restart/multi-tab/profile-preservation and stale-proof negative tests, default-deny behavior, and Windows/Ubuntu CI. Keep this unit BLOCKED until the physical gate succeeds; `8U1` remains blocked.

YOU REQUIRED: **NO** for the current source audit and synthetic preflight. Real provider consent stays at 8U1 after 8L1 acceptance. The original eight dirty recorder/caption paths remain untouched and outside this branch.

NEXT: cloud integration repair under issue #71, followed by exact-SHA native 8L1 requalification. Release this L session's physical-host/embedded lock after publishing the blocked receipt. `15B` remains independently eligible after accepted 15A, but is **not claimed or started in this session**: the owner's conditional continuation requires 8L1 to be finished and tests to pass; a missing native gate does not satisfy that condition, regardless of remaining account allowance.
