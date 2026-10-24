# PERF-00 — cloud measurement harness and process-tree accounting

**Scope:** GitHub cloud implementation only, canonical issue [#95](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/95); lock perf-methodology. Prerequisite 15G accepted at PR #84. This milestone defines a measurement instrument, not proof that CoSoS uses less RAM or that CoS original/native Windows was qualified.

## Components
- [tools/perf_00.py](../../tools/perf_00.py): Python stdlib-only CLI for fixed scenarios, Linux /proc or Windows PowerShell opt-in PID sampling, fail-closed validation and process-tree aggregation. No application/runtime dependency or provider/browser login.
- [fixtures/perf-00/synthetic-trace.json](../../fixtures/perf-00/synthetic-trace.json): deliberately synthetic trace, with app-owned root and child, explicitly attributable external-browser root and child, plus an unrelated large process excluded from totals.
- [tools/test_perf_00.py](../../tools/test_perf_00.py): shape, negative/duplicate/identity/comparability/double-count tests.
- [hosted workflow](../../.github/workflows/perf-00.yml): Windows+Ubuntu stdlib tests and synthetic reporting. No local/account tests.

## Fixed comparable scenario matrix
Use the matrix CLI below to inspect source-controlled parameters: cold_start, warm_start, idle_zero_chat, one_chat, multi_chat (8), large_history (8 synthetic), coding_terminal, external_browser (explicit reuse), embedded_browser (on-demand), and recovery. Record identical conversation counts, input script/fixture digest, output receipt, warm/cold state and distinct external/embedded configurations. Match completed work, not merely app process duration.

**Provenance required:** app variant (cos-original or cosos), exact 40-character source SHA, binary SHA256, build ID, pseudonymous fixed machine ID, OS, CPU count, power mode, profile class (never private path), scenario parameters and time. Separately record app/browser/runtime versions, scripts and fixture digest in a controlled test receipt when needed. Never store chat text, private URLs, credentials, command lines or profile paths in trace JSON. Original CoS baseline is PERF-01 Local Windows.

## CLI — synthetic cloud proof

~~~sh
python tools/perf_00.py matrix
python tools/perf_00.py summarize --input fixtures/perf-00/synthetic-trace.json
python -m unittest discover -s tools -p test_perf_00.py -v
~~~

The fixture has explicitly SYNTHETIC provenance and always reports synthetic-fixture classification and native_product_acceptance=NOT_RUN. Fixture numbers are not native performance evidence.

## Opt-in future Windows/Linux observation — NOT EXECUTED in PERF-00

Only in an authorized isolated lab after verifying exact PID, binary hash and source; output under approved native test directory. The tool never launches apps or scans profiles.

~~~text
python tools/perf_00.py capture --app cosos --workload coding_terminal \
  --root-pid <VERIFIED_COSOS_PARENT_PID> \
  --external-root-pid <VERIFIED_ATTRIBUTABLE_BROWSER_PID> \
  --source-sha <40_HEX_GIT_SHA> --binary-sha256 <64_HEX_BINARY_DIGEST> \
  --build-id <BUILD_ID> --host-id <PSEUDONYM> \
  --power-mode <FIXED_MODE> --profile-class synthetic-isolated \
  --duration 120 --interval 1 --output <APPROVED_TRACE_PATH>
python tools/perf_00.py summarize --input <APPROVED_TRACE_PATH> --output <APPROVED_SUMMARY_PATH>
~~~

Omit --external-root-pid only if no external process is attributable. Reused external browser memory is not free. Choose external browser roots by explicit owner-approved isolation and document shared-browser ambiguity. Capture records only numerical PID/PPID, process birth identity, RSS/working set, CPU, optional private bytes and handle count. It never reads command lines, page titles, process paths, prompts or file contents. On Linux private_bytes is deliberately null: /proc VmRSS is not Windows private bytes.

Accounting traverses descendants from explicitly identified roots, with unique PID counting, owner-first de-duplication and fail-closed missing-root checks. Privilege-inaccessible children, exited processes, out-of-tree helpers, shared browsers, sampling gaps and rapidly changing process identities are limitations. Record collector_ms per observation; Windows CIM/Get-Process enumeration may perturb short sampling intervals. CPU percent reflects observed cumulative deltas as a lower bound, not full system CPU.

Comparison requires same machine, OS, CPU, profile class, power mode, workload and duration. Use compare --baseline A.json --candidate B.json for a *provisional* median whole-tree RSS difference. The baseline must be original CoS, candidate CoSoS; synthetic or incomplete traces are rejected. Even successful comparison returns native_product_acceptance=NOT_RUN. Physical Windows review must verify source, binary, child attribution, renderer/WebView2 and browser sidecars before any claim of accepted RAM improvement.

**Open gates:** PERF-01 native original baseline; PERF-02 browser mode probe; PERF-03 lazy bounded UI/history; PERF-04 worker/tab lifecycle. PERF-05 additionally needs 8L1 and 15L3 native. PERF-06 follows PERF-05 and 16D. No successor automatically claimed or certified.
