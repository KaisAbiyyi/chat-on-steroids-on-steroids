# 7P-L1 native Keyboard/IME follow-up

Date: 2026-10-22, Asia/Jakarta. This is a scoped local qualification continuation
of PR #42, not production feature implementation or full 7P-L1 acceptance.

## Identity and boundary

- UNIT: `7P-L1`, `windows-browser` continuation explicitly requested by the owner.
- Branch: `docs/7p-l1-native-20261022`; starting documentation head
  `64dd70f67c8b04e4ec0e21df17ba6e1bce3b35c5`.
- Tested code: `6555032738ab4ef0272129f35ca23ede3c77f3d5`, packaged self-contained
  `WindowsHost.exe`, product version `1.0.0+6555032738ab4ef0272129f35ca23ede3c77f3d5`,
  SHA-256 `287E84C686DF5B8B64A3F67D98BD449CD3DEE7764BCD60C5F5D889947B3E77E0`.
- Windows 11 x64 build 26300, WebView2 `155.0.4283.62`, actual bundled CodeMirror
  with synthetic independent A/B profiles. No real account or provider actions.
- `@oai/sky` Windows automation supplied native input and UI Automation observations.
  No CDP input dispatch, JavaScript value assignment or UIA SetValue was used.
  This is native automation evidence, not a human physical-keyboard usability test.
- Profiles, logs, temporary files and verification harness remain under
  `D:\projects\CoS-Rust\.test-tmp\7p-l1\keyboard-ime-20261022`.

## Keyboard result

Native keyboard/editor scope: **PASS**. Ten checks with 17 exact assertions verify
Ctrl+A selection, ASCII replacement, Ctrl+Z exact restoration, Windows Ctrl+Y redo,
the `x` character key, Shift+Left partial selection, Unicode `é 日本 😀`, Unicode
undo and independent profile B remaining unchanged. Ctrl+Shift+Z left content
unchanged; the Windows redo test uses Ctrl+Y, with the unsuccessful attempt retained.

UIA `document_text` and root-level `focused_element` were stale for edited text;
the verifier uses the actual editable control's `Value` plus `selected_text`, with
live screenshot inspection. Original observations and unsuccessful setup attempts
are preserved. A first hidden launch omitted asset/extension arguments and failed
before browser setup; the corrected interactive launches supply both explicitly.

## IME and remaining gates

OS IME qualification: **HOLD / UNVERIFIED by explicit owner direction**.
The owner requested skipping further IME tests now and retaining the gap in the
roadmap as `IME-DEFER-20261022`, carried to 8L1 / Phase 15 multilingual editor
acceptance. See the [scheduling exception](../BIG-IMPLEMENTATION-PLAN.md#owner-directed-ime-deferral--2026-10-22).
Initially only the US keyboard was active.
The owner explicitly approved installing Windows Japanese Basic typing (Settings
displayed a 1 MB download) on C:. This is an explicit exception for that Windows
system component; qualification profiles and outputs remain on D:. Other optional
language features were not selected.

The user stopped Computer Use with Escape before IME testing could be completed,
then explicitly deferred the task. The originally configured `en-US`, `id`,
`en-ID` language order and input tips (including the originally empty Indonesian
tips), empty input-method override and both false language-bar legacy flags have
been restored and compared with the pre-test snapshot. Owned probe processes were
stopped. The previously authorized Windows system-component installation was not
uninstalled and no Windows servicing process was stopped; completion of that
installation is not claimed. Servicing logs showed a 376,811,350-byte download,
so the Settings label of 1 MB was not the observed servicing payload size.

All 67 renderer/extension files used in the interactive test and the host EXE
match the existing final installer staging bytes. This was a packaged executable
run with explicit matching fixture paths, not another install/uninstall test.

Screen-reader operation, clipboard parity and full 7P-L1 remain unaccepted. Native
keyboard evidence does not waive those requirements or unblock 7A.

## Validation and handoff

- `node .../verify-keyboard.mjs`: PASS, 17 assertions, exit 0.
- `python tools/task_board.py check`: PASS, 115 nodes, no dangling/cyclic edges.
- Product implementation: not changed. Rust tests/fmt/Clippy were not rerun for
  this documentation/evidence-only follow-up.
- PERFORMANCE: no improvement claimed or new benchmark run.
- Cleanup: PASS, exact language/tip, override and language-bar comparison.
- Evidence: [scoped receipt](../../audit/2026-10-22/7p-l1-keyboard-ime-deferred-receipt.json);
  raw observations, verifier, package identity and cleanup receipts remain on D:.
- LOCAL REQUIRED: none now for IME; deferred 8L1 / Phase 15 gate as above.
- YOU REQUIRED: no now. KNOWN LIMITS: IME, screen-reader and broader native gate
  remain unaccepted; this follow-up does not establish full-product readiness.
- NEXT: preserve the IME roadmap gap and review remaining 7P-L1 requirements.
