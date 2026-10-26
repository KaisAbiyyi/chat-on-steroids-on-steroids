# 8L1 native repair execution — 2026-10-26

Owner explicitly directs continued local repair until failing automated gates PASS. Original receipts remain immutable. Unit COS-8L1-20261026 generation3, branch feature/8l1-native-20261026 / PR125. Protected original root WIP remains outside this checkout. Owner consent/profile-action dialogs remain deferred and cannot be called PASS.

1. Preserve native RED: two source-aeb6fa8 retained-profile startup failures and independent live runtime inventory in the 2026-10-26 qualification receipt.
2. Repair native bootstrap inventory: prove empty first-use profile before runtime creation; authenticate actual OS browser process executable with Windows Authenticode and Microsoft publisher under machine-installed EdgeWebView runtime; permit only exact two known runtime component IDs plus the exact retained source-owned fixture. Reject unknown/duplicate/noncanonical/missing identities and unverified runtime. Do not trust display names or page/worker input. Bound to synthetic 8L1 retained profiles; no provider/OAuth/Native8P changes.
3. Add linked-source deterministic adversarial policy tests plus actual signed-runtime positive/unsigned binary negative. Add 8L1 per-stage bounded diagnostics to isolate earlier no-receipt timeout.
4. Source-bound native rebuild and physical D: restart/multi-tab/crash runs; resolve each observed cause with regressions and fresh immutable receipts. Preserve all original failures. Run affected C#/Rust/JS plus strict Windows checks and exact-head Windows/Linux CI.
5. Direct full-diff self-review, repair material findings, update implementation summary/Notion and physical gate. Merge only after all mandatory tests and owner interactions actually PASS. No completion claim for deferred dialogs.

Shared-source inspection: active8P-HOST124 at0c205bf changes other Program.cs methods/flags, not SetupAsync; Native8L1, project file and8L1 workflow untouched there. This repair reserves narrow synthetic bootstrap and diagnostic hunks, leaves124 branch/source authority intact and publishes checkpoint125. Runtime trust follows WinVerifyTrust file signature validation with cache-only URL retrieval, never certificate extraction alone.

Ledger:
- RED: two real retained-profile receipts fail Unexpected extension in clean profile before marker/session checks; single-process run times out without receipt.
- Policy GREEN: 17 native assertions PASS, including installed signed runtime. Host strict build PASS. Physical repaired run pending.

- Native run01 at ed4754f clears component guard and authenticates real channels. Browser.crash kills profile A OS browser while its native CDP Task never resolves; B remains live. Bounded crash-request wait is required; retain ProcessFailed/all-scopes/Rust-retirement assertions unchanged. Earlier run timeout retained.

- Run02 at b420bae: first-round native auth/multi-tab/navigation/real crash+all Rust retire PASS. After-restart RED: crashed browser A had not flushed unpacked fixture metadata; B retained it. Repair only source-hashed pinned fixture from retained extension path; require exact previous ID and full post-install inventory, then fresh Rust proof. Missing fixture alone never confers authority; unknown extensions still refuse.

- Full owner run04 RED: owner confirms Yes/No and no third dialog. Native permission harness had observed event entry as completion while modal callbacks can reenter; permission result may also be cached per document. Require settled-handler counts and no busy modal; fresh native documents/Rust proof for independent requests; retain all Allow/Deny/replay/stale assertions. Publish only decision counters, never location data.

- Run05 interrupted for root-level correction: WebView2 threading contract prohibits modal UI synchronously inside native callbacks. PermissionRequested now holds a using-scoped GetDeferral, yields callback, rechecks native scope/lease, then prompts. Stale navigation can actually execute during modal; initial Deny and current-scope grant recheck retained. Microsoft source: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/threading-model .

- GREEN run07 at62bbfdd: all 27 native checks in both OS process rounds plus4 restart comparisons PASS, actual owner dialogs. Final strict405Rust/fmt/Clippy PASS; native25policy +65JS +16DAG +16guardrails PASS. Self-review fixes complete; final CI/merge/exact-merge retest remains.
