# 7L1 — physical Windows Rust bridge / MV3 companion qualification

**Decision: BLOCKED (not accepted).** Date 2026-10-23; source and compiled WinForms probe version `8d1f4c5a3fbdb7e5dec1cdf02a9ef4e25077c0b4` (accepted GitHub main after PR #48). Host: Windows 11 Home Single Language 10.0.26300; WebView2 Evergreen 155.0.4283.62. Worktree and temporary profiles under `D:\projects\CoS-Rust\.test-tmp\7l1\`, never a signed-in provider.

## Actually run

| Check | Result | Authority |
| --- | --- | --- |
| `node --test extension/companion/tests/ledger.test.mjs` | 16 PASS / 0 FAIL | 7C in-process JS ledger semantics, mocked privileged source |
| `cargo test -p cos-core native_browser_bridge --locked` | 8 PASS / 0 FAIL | 7B in-process Rust bridge/HMAC/fence semantics |
| `dotnet run --project probes/7p-b/PolicyTests/PolicyTests.csproj -c Release` | 15 PASS | 7P-B synthetic policy only |
| `dotnet build probes/7p-b/WindowsHost/WindowsHost.csproj -c Release` | PASS, 0 warnings, 0 errors | Real WinForms/WebView2 compile |
| Native WindowsHost `--batch --native-idle --native-auth` | 26 PASS, 0 FAIL, 0 UNVERIFIED, 2 NOT_RUN; real 65-second MV3 idle worker changes | Real WebView2; **7P-B synthetic** extension, not 7C |
| Native WindowsHost `--batch --native-auth`, launched with `Start-Process -Wait -PassThru` | Child exit **0**, 18 PASS, 0 FAIL, 1 UNVERIFIED (idle not requested), 2 NOT_RUN | Separate real executable/process exit proof |
| **Real Rust 7B ↔ native extension 7C WebView2 handshake + tab election + restart** | **BLOCKED / NOT RUN** | There is no runtime adapter to execute it |

The 2 native host `NOT_RUN` keys are the original `native_windows_gate` and `distribution_budgets` emitted by the *7P-B probe*, not a regression of accepted scoped 7P-L1 qualification. The final-product distribution gate remains at 16L3. Synthetic OAuth is not real provider sign-in; native host MV3 restart is not evidence for the 7C ledger inside the real worker.

## Evidence provenance

- [Native idle/forced restart receipt](../../audit/2026-10-23/7l1-native-idle-8d1f4c5-raw.json), committed LF-normalized UTF-8 SHA-256 `898C56984D4DD0E1325CFAC7EE006CB2D748C98A0EA8C9028F61102A6B2D4B6E` (original Windows CRLF source SHA-256 `D574870F547E723D1228C3A78D5779BBFF018E751F0A171AB5B7E14586F43E5D`). JSON content is identical.
- [Separate child exit receipt](../../audit/2026-10-23/7l1-native-exit-8d1f4c5-raw.json), committed LF-normalized UTF-8 SHA-256 `88341FB96117A666942385384CE30DF61FF93BDD445A262B08B117AB2887CCD9` (original Windows CRLF source SHA-256 `77C586EDB4232323BA0568B19CA7C62F9E94BC73C50FF43E56E2F182EFCD5D3E`); externally observed child exit code 0. JSON content is identical.
- Compiled WindowsHost executable SHA-256 `32D994C0DFB34FA9F25A217AB691C2789F09D01609D7CAFEED28C20014B9EFB0`; receipt executable version contains full `8d1f4c5` SHA. The native WebView2 probe is synthetic, not installed product Rust GUI.
- Dependencies: 7C merged as `d9a56ef`, 7P-L1 merged via [PR #48](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/48) at `8d1f4c5` with 14 successful CI check-runs.

## Proven integration gap

In [Rust bridge source](../../crates/cos-core/src/native_browser_bridge.rs), `NativeBrowserBridge::register_host`, `complete_handshake`, `observe` and `authorize_current` have no production caller: the only invocation is `#[cfg(test)]`. In [7C companion source](../../extension/companion/worker.mjs), `installVerifiedCompanion` requires a privileged `subscribeVerifiedNativeEvent` and `verifyHostEvent`, but no real WebView2 host or installed MV3 worker calls it. [The existing WebView2 extension](../../probes/7p-b/extension/sw.js) processes a **different synthetic** `chrome.runtime.onMessage` debugger check; its manifest does not load 7C or request `storage`.

The 7B MAC authorization covers the normalized identity frame; a future adapter must additionally bind exact 7C claim/ACK/repair envelope bytes to its authenticated source. The browser host must attest controller/profile/current URL, actual target/tab/frame/document and worker generation, retire stale owner/handshake authority on navigation or worker restart, preserve at-most-one-writer for the ledger, and never inject privileged proof through page-visible script data. In-process 7B handshake proof computed by the same Rust test process is not real extension authentication.

**Unblock:** implement the reviewed cloud/native adapter in [issue #49](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/49), merge on green CI, then rerun 7L1 on exact merged Windows source with 7C actually loaded in MV3 and full negative-source/epoch/tab/restart cases. Until then keep **7L1 BLOCKED** and **7D BLOCKED**, with no user participation required for this synthetic fix. Owner OAuth, OS IME and production release measurements are separate gates.
