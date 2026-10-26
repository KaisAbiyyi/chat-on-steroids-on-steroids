# 8L1 native repair and owner-operated qualification v2.0

## Result and source

**Physical synthetic 8L1 gate PASS** at `62bbfdde6fb4506f5cb78d768e6960b50975dfab`, run07, Codex local `COS-8L1-20261026` generation3. [PR125](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/125) contains the repair, original failures, new deterministic regressions and source/binary-bound physical evidence. Final PR-head CI and eligible merge remain publication gates; final handoff requires an exact merge-SHA rerun. The native gate does not approve generalized provider origins, production OAuth, full JS 8B/8C equivalence, real provider sign-in, desktop performance, installer or OS IME.

## Changes and resolved failures

1. **Native profile bootstrap:** the actual machine-installed Microsoft-signed WebView2 process provides two known component extensions in an empty profile. Verify Windows Authenticode file integrity/trust and Microsoft publisher of that actual OS process, canonical installed runtime path without reparse ancestors, and empty first-use profile before runtime creation. Admit only the exact two native component IDs and, on reuse, the pinned source-owned fixture ID. Display names confer no trust. Unknown, duplicate, noncanonical, unverified, or missing required identities fail closed.
2. **Crash transport:** `Browser.crash` can kill its own transport without resolving the native CDP Task. Bound that reply wait, while requiring real `ProcessFailed`, all native scope revocations and confirmed Rust retirements. A timeout alone cannot pass crash qualification.
3. **Crash/restart fixture recovery:** immediate browser death may occur before initial unpacked-extension metadata flush. Restore only the previously pinned fixture from the already source-hashed retained path, require its exact original ID/enabled state, recheck the entire inventory, then require fresh Rust/MV3 proof. Never accept a missing fixture or arbitrary existing extension as authority.
4. **Permission callback lifetime:** WebView2 forbids modal nested message loops inside its native callback. Hold a using-scoped permission deferral, yield the initial callback, recheck native snapshot/lease, then show the actual owner dialog. Initial Deny, no persisted permission, profile-action exclusion and post-modal exact current-scope grant checks remain. The probe waits for handler completion rather than event entry; independent permission requests use real new documents and freshly authenticated channels. Receipt contains decision counters only, no location response.

The sharing assessment compared active cloud124 head0c205bf: its production Native8P/OAuth/flags changes do not touch the failing SetupAsync section or Native8L1/project/workflow hunks. No edits or takeover of124 branch; this owner-authorized local repair is published on125. Original root WIP and historical receipts remain untouched.

## Validation

- Physical **27 named checks PASS in each of 2 independent host/Rust processes**; **4 cross-process/crash/retained-profile checks PASS**. Same manifested binaries, profile paths, extension IDs and nonempty synthetic storage markers across restart; distinct host/Rust PIDs. Real same-profile multi-tab, cross-profile refusal, conflicting session claim refusal, physical tab close, navigation revocation, fresh rebind, actual browser crash and all Rust retirements.
- Both rounds: actual owner **Yes, No, Yes** permission choices; **Yes** synthetic navigation; **Yes** logical handoff. Permission counters per round: 3 prompts, 1 grant, 1 denial, 4 real events, 1 replay refusal, 1 stale-allow refusal, all 4 settled, 0 busy. Handoff atomicity, replay refusal and new-profile observations PASS. No account/credential used or copied; no agent-operated Yes/No.
- Windows strict workspace: **405 tests PASS / 0 failed / 0 ignored**, formatting and all-target Clippy `-D warnings`, exit0. Explicit Rust bridge example **3 PASS**. No Rust implementation changed during this repair.
- Linked-source C# policy: **25 PASS** locally including the actual signed runtime; hosted pure policy uses 23 assertions. Both product and shared legacy native host strict builds PASS; native authority and MV3 retirement self-tests PASS.
- JavaScript **65 PASS**, DAG **16 PASS**, browser guardrails **16 PASS**. The original failed snapshot checks were reproduced RED after accepted native evidence changed the registry; updated only status assertions and kept the blocked-native forecast regression against an explicitly reopened blocked copy. New test validates both actual raw native round receipts, source/binary identity, owner counters and retained restart proof.
- Hosted final-head Windows/Linux checks: require all SUCCESS at the final published HEAD before merge; source62bbfdd checks were 35/37 SUCCESS at summary preparation, remaining Windows jobs in progress. Never reuse canceled superseded runs as final evidence.

## Evidence interpretation and review

[Aggregate PASS receipt](../../audit/2026-10-26/8l1-native-repair/qualification-pass.json), [before restart](../../audit/2026-10-26/8l1-native-repair/owner-run07-before-pass.json), [after restart](../../audit/2026-10-26/8l1-native-repair/owner-run07-after-pass.json), [restart comparison](../../audit/2026-10-26/8l1-native-repair/owner-run07-restart-pass.json). Original run01 timeout, run02 crash/fixture failure, run04 permission failure, run05 interruption and run06 handoff No remain preserved. Owner confirmed run06 No; correct denial is not a successful positive handoff. Run07 supplies the positive proof.

Raw per-process `native8L1Accepted=false` and wrapper `physical8L1Accepted=false` remain unchanged by design: they never self-certify the whole L gate. This separately reviewed aggregate requires all **27** named in-process checks in both receipts and all **4** real cross-process comparisons. The per-process `native_host_process_restart=NOT_RUN` placeholder is resolved by actual parent-runner process/profile comparisons; no in-process fixture was rewritten to claim restart.

**Self-review — Codex local / COS-8L1-20261026:** direct complete base-to-HEAD review covers native/profile/source authority, secret exclusion, callback/deferral lifetime, modal reentrancy, stale grant/lease fencing, exact extension recovery and bounded crash waits. Material P1 failures above resolved with original RED receipts and real GREEN run07; no unresolved P0/P1 in this bounded synthetic repair. No original fixture, old receipt, JS acceptance contract or Rust schema was relaxed. SHA256 manifest lists every loadable artifact. Machine-installed signed WebView2 is the supported runtime for this gate; alternate installation layouts fail closed. Same-user native binary/profile tampering is not made a new sandbox claim. No subjective product/security approval for unrelated124 is supplied.

## Handoff

- **UNIT:** 8L1; local Windows, generation3; branch feature/8l1-native-20261026 / PR125. Keep claim through final CI/merge/retest.
- **CLOUD VALIDATION:** exact final-head Windows/Linux full checks required; pending publication completion at this document checkpoint.
- **PERFORMANCE:** no product memory/CPU/size improvement claimed.
- **LOCAL REQUIRED:** exact merge-SHA source-bound full four-switch rerun before final handoff; preserve receipt separately without rewriting earlier source62bbfdd evidence.
- **YOU REQUIRED:** actual dialogs, already completed for source62bbfdd; repeat for exact merge-SHA rerun. Real provider sign-in remains separate8U1/7U1.
- **KNOWN LIMITS:** production 8P-HOST124 remains Draft/Changes Required and owns generalized provider/native contract; 8U1 remains blocked, not unlocked into sign-in by a synthetic host test. OS IME HOLD retained.
- **NEXT:** successful final-head CI, eligible125 merge, exact merge-SHA native retest, GitHub/Notion read-back synchronization. No final completion while a required current check fails.
