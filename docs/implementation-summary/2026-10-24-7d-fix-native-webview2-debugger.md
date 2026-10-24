# 7D-FIX — Browser-owned WebView2 debugger integration (cloud implementation)

**Issue:** [#62](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/62)  
**PR:** [#86](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/86)  
**Implementor / direct reviewer:** ChatGPT GPT-6 Cloud, claim `COS-7D-FIX-20261024-1250`.  
**Scope:** cloud source/build/CI only; physical Windows 7L2 remains **BLOCKED / NOT_RUN** until a distinct real-machine acceptance.

## Gap corrected

The accepted 7D `DirectBrowserTools` was called only from fake `BrowserDebugger` Rust unit tests. The 7L1 native WinForms host could authenticate real MV3 worker signatures, but it could not execute the Rust 7D debugger contract. Therefore earlier CI could not prove 7L2.

This change adds `crates/cos-core/examples/7l2_bridge.rs` as a **private process-pipe** 7B/7D host service and implements `BrowserDebugger` through callbacks exclusively serviced by the source-attested WebView2 host's real browser-owned CDP surface. `probes/7l1/WindowsHost/Program.cs` now supports a separate `--7l2` runtime, preserving the previously accepted `--native-idle` 7L1 path. Native code and MV3 worker extension are pinned to the same host/controller/profile/tab and fresh generation.

## Security boundaries reviewed

- Host provenance: exact git HEAD, clean native authority sources, full artifact directory file SHA256 manifest and no reparse/junction in path.
- Authority: the trusted host owns source-attested profile/tab/frame/document/navigation/worker; 7B `NativeBrowserBridge` independently validates every signed tool authorization with the current channel, strict sequence and MAC, not a tab ID supplied by a renderer.
- CDP: callback opcode is one of `attach`, `snapshot`, `detach`; C# rechecks actual browser-owned controller identity, page target, installed MV3 worker target, and exact profile before/after async CDP calls. No page message or content-script listener grants a debugger.
- Ownership: Rust reserves an exclusive debugger lease before attach. Uncertain attach/detach retains a reservation and denies another contender. A handle alone cannot read; correct scope, channel, generation, sequence and MAC are required.
- Snapshot: fixed read-only CDP expression, URL must belong to expected HTTPS origin, title/text/url bounded by Rust and host. No cookie access, mutation, clicks, arbitrary developer-input CDP/JS, automatic Send or user account interaction.
- Retirement: native navigation, disposal and MV3 worker target-destroyed signal revoke authority. Rust cleans only matching exact-channel leases; stale retirement never frees a different owner's debugger.
- 7C retry: actual native MV3 `chrome.storage.session` `claim`/`repair` paths are exercised; abandoned/ambiguous claim never permits a second Send.

## 7L2 physical test matrix

The source-owned `probes/7l1/WindowsHost/Native7L2.cs` runner requires **all** named checks to pass, including real WebView2 profile isolation, authentic 7B worker handshake, CDP debugger attach, duplicate attach denial, bounded snapshot, guessed handle and invalid MAC refusal, cross-profile/tab/frame scope denial, ambiguous ACK no-resend, real detach and verified reattach, injected native detach error retaining lease, physical navigation retirement, actual tab disposal, and real worker target destruction with stale signed snapshot rejection.

The runner emits `receipt.json` with source SHA, host/Rust binary SHA256, per-check PASS/FAIL/NOT_RUN and Rust process exit. Any missing/failed required check makes `native7L2Accepted=false`. A cloud build, fake debugger test or preflight **cannot** satisfy this gate.

## Hosted build and handoff

`.github/workflows/7d-fix-native-debugger.yml` runs Ubuntu/Windows Rust fmt, strict Clippy, source-aware 7B/7D tests, MV3 worker/ledger regressions, and hosted Windows release compilation of Rust helper and .NET8 WinForms WebView2 host. Artifact `cos-7d-fix-7l2-windows-<exact-git-sha>` contains both native binaries, `source.sha` and `hashes.json` with SHA256 of each file.

Other GitHub PR workflows may test a synthetic merge-ref, but the 7D-FIX workflow explicitly checks out **the exact PR HEAD** and names the artifact with the exact git SHA it builds; its `source.sha` and hash manifest match that source. After merging, the **push-to-main** workflow separately builds the actual merged source, emits a new artifact with the real `main` commit SHA, and records that same SHA in `source.sha`. **Use only the post-merge main artifact** for physical 7L2 acceptance.

Only when cloud PR is accepted and merged, run `probes/7l2/run-native.ps1` on a separately authorized isolated Windows D: checkout with the **same real merged main SHA** as the post-merge artifact:

```powershell
pwsh -File probes/7l2/run-native.ps1 -Repo D:\projects\CoS-Rust\verified -ArtifactRoot D:\projects\CoS-Rust\artifacts\7l2 -Out D:\projects\CoS-Rust\.test-tmp\7l2\new-run
```

This example does **not** represent an actual executed native run, and source SHA must match the accepted built artifact. Retain honest local receipts. Keep 7L2 Blocked until actual physical PASS, and do not unlock 7U1 from cloud checks.

## Review checklist

- [ ] CI Windows/Ubuntu and Task DAG exact PR HEAD all PASS.
- [ ] Implementor independently re-inspected exact HEAD and changed file diff, unsafe edge cases, no Send, build artifact and logs; post a signed review receipt on PR.
- [ ] Merge only after gates prove accepted cloud scope; sync Notion/GitHub. Native 7L2 remains separate.
