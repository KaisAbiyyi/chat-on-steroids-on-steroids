# 7P-L1 — final source-bound synthetic Windows security checkpoint

**2026-10-23, tested code `53164da79b32b4a4cacb433678a94ec13de57888`.**
This is a physical Windows 11 WebView2 synthetic browser adapter and unsigned
probe installer, not the production Rust desktop, a release-budget acceptance
or a real provider session.

## Regression and independent review

Independent read-only review of PR #42 found two initial issues: synthetic
OAuth profile isolation tested only a string comparison, and the installers
used lexical D: checks that could follow an NTFS junction. The fixes bind
authorization to an actual native WebView2 controller and reject existing
reparse ancestors in the installer, uninstall and builder. Follow-up review
found another OAuth false-positive if the B controller prerequisite itself
failed; the host now independently proves the current B controller, expected
profile name **and exact callback URL** before checking denial of pending A
authorization. The builder likewise verifies real D: ancestors before
publishing. The last independent spot-check reported **no proven P1/P2 or
false-positive** in the reviewed scope. This is not third-party accreditation.

Native regression of the **installed** executable, version
`1.0.0+53164da79b32b4a4cacb433678a94ec13de57888`, WebView2
`155.0.4283.62`: `--batch --native-auth --native-idle` exit 0,
**26 PASS / 0 FAIL / 0 UNVERIFIED / 2 NOT RUN**. The B-controller proof,
MV3 65-second idle, extension debugger reattach, tab lifecycle and
cross-profile CDP denial PASS. Raw receipt:
`D:\projects\CoS-Rust\.test-tmp\7p-l1\combined-native-53164da\receipt.json`,
SHA-256 `544A830C6A8F2F87C432E19B6D6DBC7C5BECE549244D59EA1326274D61F5388E`.
EXE SHA-256 `1E972C70DDC74200896EB2E5C607094E2892FDA4DF0C6CCACA63870E1DB3A02C`.

## Distribution probe and resource qualification

Unsigned synthetic setup: **68,009,984 bytes**, SHA-256
`90CB1FAB5436BCDA01F431A1E998692634DD90BC2EDA1DC450465C4A62295C34`.
The 84-file payload requires the installed Evergreen WebView2 runtime. D:
positive install, hash verification and **full clean uninstall** PASS;
controlled D:-only junction checks for setup EXE, script install/uninstall
and build outputs reject reparse ancestors, preserving their sentinels.
Another test preserved a user-added file and one intentionally modified
packaged file during uninstall. The package manifest includes a hashed list
of owned directories and removes them only when empty, avoiding traversal
into unknown directory trees.

Six no-FAIL measurements on the **installed** host (3 new/cold and 3 reused
warm isolated profiles) yielded p50 complete synthetic batch durations of
**3,992.6 ms cold** and **3,571.4 ms warm**, with observed host peak working
set **67,379,200 bytes**. These are **not** product time-to-interactive or
private/unique WebView2 memory statistics. Raw receipt:
`D:\projects\CoS-Rust\.test-tmp\7p-l1\benchmark-53164da\measurements.json`,
SHA-256 `5F4C92A964900EFF0871D317CA277292B9AEB1D2023FADD805AFDB01894B85C2`.

Local targeted .NET PolicyTests 15 PASS, synthetic manifest/renderer contract
PASS and companion 16 PASS on the change line. Final PR-head Windows/Linux
GitHub CI must independently succeed before source merge. No private
profiles/accounts, real provider auth or automatic Send was used.

## Acceptance distinction

`distribution_budgets` and `native_windows_gate` remain `NOT_RUN` **for the
production app**. The production GUI/bundle, real account consent, real
user-visible startup/unique RAM and release thresholds do not exist at this
point; user-deferred OS IME remains outside this scoped proof. Thus
`native7PL1Accepted=false`, 7P-L1 is **PARTIAL / NOT ACCEPTED**, and native
7L1 must remain blocked even if this source checkpoint is merged. The
installer reparse defense rejects existing links but is not a substitute
for race-free pinned NTFS handles in a production installer.

Machine-readable final evidence:
[`audit/2026-10-23/7p-l1-53164da-final-synthetic-native-receipt.json`](../../audit/2026-10-23/7p-l1-53164da-final-synthetic-native-receipt.json).
