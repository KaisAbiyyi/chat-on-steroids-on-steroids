# 7P-L1 — follow-up independent review and Windows native retest

**Scope:** native **synthetic** WebView2 adapter and installer on the D: test
worktree. This is not the CoSoS production browser, a signed release, proof
of all provider OAuth constraints, or final release-budget acceptance.

## Review findings and corrections

An independent read-only review of PR #42 found two weaknesses after the
earlier CI-green `39e1978` checkpoint.

1. `mock_oauth_profile_isolation` could previously report PASS from a pure
   profile-string comparison without attempting authorization through the
   actual profile B controller. The `CompleteOwnedConsent` path now derives
   the profile from the live `WebView2` controller record, validates that the
   controller is current and its URL is the callback being checked, and tries
   B **while A's grant is still unconsumed**. Only then does A consume the
   synthetic grant. A native same-source run PASSed, and the subsequent
   installed combined run PASSed.
2. Purely lexical `D:\` checks in the synthetic installer/uninstaller did not
   reject `D:\junction\...` pointing through an existing NTFS reparse point.
   The C# offline setup and both PowerShell install/uninstall paths now reject
   reparse-point ancestors. Controlled D:-only junction fixtures verified
   rejection of the EXE and scripts; the target sentinel survived. This is
   **not** a race-free NTFS-handle pinning guarantee against a privileged or
   concurrently mutating adversary; production installer threat-model and
   authorization remain future work.

Normal setup/uninstall uncovered an additional reproducible issue: all files
were removed, but the file-only manifest did not own two empty `runtimes/`
folders. The package now includes a hashed `probe-dirs.txt` listing its own
directories. The uninstaller verifies the list and removes only packaged
directories **when empty**, without recursive traversal into unknown trees.
Positive clean uninstall removed the entire dedicated install root. A second
uninstall preserved an unowned sentinel and a modified packaged file.

## Exact-source Windows evidence

Tested code SHA: `8f0bc630d399ee79e18950d056da3bffa0a20860`.
Installed EXE SHA-256:
`434CE7BC783AC2623C15D67735145913E8B549C3CC6E69449F7B4E0EB4A98F77`.
Evergreen WebView2 runtime `155.0.4283.62`.

- Combined **installed** `--batch --native-auth --native-idle` produced
  **26 PASS / 0 FAIL / 0 UNVERIFIED / 2 NOT RUN**, process exit code 0.
  Receipt at
  `D:\projects\CoS-Rust\.test-tmp\7p-l1\combined-native-8f0bc63\receipt.json`,
  SHA-256 `CE0D6F80AC9E056808DCC1C2F6F9AF6FF5AD93A14925E0BD6EDE90D3919CBAAF`.
- Offline unsigned installer: **68,009,984 bytes**, **84 files**, SHA-256
  `9A1187C7E5FE91B3F9C0A7A88E4F9D93C4F97424831C88553EB1170CB67498BC`.
  SHA-verified D: install and scope-limited uninstall PASS. The packaged
  Evergreen WebView2 runtime is **not** included.
- Six exact-source installed synthetic benchmark runs PASSed (three cold,
  three warm), median **3,242.9 ms cold** / **3,158.5 ms warm** for full
  batch including synthetic tests/shutdown. Host peak sampled working set
  reached **68,300,800 bytes**. These are not UI time-to-interactive nor
  unique/private WebView2 RAM measurements. Raw benchmark:
  `D:\projects\CoS-Rust\.test-tmp\7p-l1\benchmark-8f0bc63\measurements.json`,
  SHA-256 `C8AB5ABA3574A59292FDA55F0D7A816B86F58A8749ABD0151E6E4A1562434CED`.
- Local PolicyTests 15 assertions PASS, extension contract PASS, companion
  16 tests PASS, .NET host build and diff check PASS. The final PR-head CI
  must be checked independently; native qualification is never inferred from
  GitHub CI alone.

## Outstanding gates

The source fixture deliberately keeps `distribution_budgets` and aggregate
`native_windows_gate` at `NOT_RUN` for the **production** desktop app. Real
product installer/signing, private memory measurement, time-to-interactive,
release budgets and aggregate acceptance still need their own phase gate.
No real account OAuth or owner-deferred OS IME was executed. Thus
`native7PL1Accepted=false`, and 7P-L1 remains PARTIAL / NOT ACCEPTED even
though executed synthetic checks are all green. Native 7L1 cannot assume
the unaccepted capability. Source/PR merge, if separately authorized, must
not automatically mark native acceptance Done or waive these requirements.

Machine-readable summary:
[`audit/2026-10-23/7p-l1-8f0bc63-native-security-retest.json`](../../audit/2026-10-23/7p-l1-8f0bc63-native-security-retest.json).
