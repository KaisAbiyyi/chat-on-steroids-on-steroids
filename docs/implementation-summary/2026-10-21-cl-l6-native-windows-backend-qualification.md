# CL-L6 — native Windows integrated backend release qualification

**Date:** 2026-10-21 (Asia/Jakarta). **Gate:** PASS for the phase 1–6
Rust headless service/backend boundary. **Exact source SHA tested:**
`b85471c9bce70842fd84242d4c2f72fe31df5532`, descendant of accepted
CL-10 [PR #38](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/38)
merge `6fe62bdddff4f1d272ec7620fe0e03c1ce6be277`.

## Native environment and scope

Windows 11 x64 NT 10.0.26300, real NTFS `D:` volume. Source from clean,
detached-at-test-time worktree
`D:\projects\CoS-Rust\.test-tmp\cl-l6\worktree`. `CARGO_HOME` reused
the on-D: crate cache in `.test-tmp\cl-l3\cargo-home`, `CARGO_TARGET_DIR`
was `D:\projects\CoS-Rust\target`, and TEMP/TMP and all synthetic fixture
profiles were under `.test-tmp\cl-l6`. Cargo 1.91.1 and rustc 1.91.1.
The original eight local recorder/caption WIP paths were untouched.

| Native check | Exact result |
| --- | --- |
| `cargo fmt --all --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets --locked --offline -j 4 -- -D warnings` | PASS, exit 0 |
| `cargo test --workspace --locked --offline -j 4 -- --test-threads=2` | **312 PASS**, 0 failed, 0 ignored across 80 suites; exit 0 |
| `cargo build --release --locked --offline -j 4` | PASS, exit 0 |
| External Windows native release-binary harness | **17 PASS**, 0 failed; exit 0 |

The native harness drove the actual release process, with synthetic public
profiles and a real loopback transport. It verified explicit profile and
protocol-14 behavior, durable recorder/history/search/export/usage across
processes and restart, Windows secrets capability, MCP schema and code-worker
execution, unauthenticated/unsafe model-call refusal, unmarked profile refusal,
host/origin/capability URL security boundaries, bearer-protected local control
and refused mutation, live control-session reads, graceful endpoint retirement
and token rotation. Five fresh-profile cold starts and five existing-profile
warm starts were measured separately.

## Release resource receipt

- Native Windows release executable: **12,853,760 bytes**; SHA-256
  `456fbbe5f0303ed0367d32cf048d28b08a9ef4c5e076f17b36fb5ae5012f8275`.
- Cold ready latency (five samples): median **23.55 ms**, p95 **25.70 ms**.
- Warm ready latency (five samples): median **33.97 ms**, p95 **38.48 ms**.
- Windows process working-set snapshot p95 across ten samples:
  **7,749,632 bytes** (~7.39 MiB).
- This is a bounded synthetic one-machine observation, not a peak-memory,
  throughput, long-duration or speed-improvement benchmark. Earlier CL-L4
  timings used a different harness and workload and are not directly comparable.

## Evidence, corrective trace and boundaries

The [committed JSON receipt](../../audit/2026-10-21/cl-l6-b85471c-receipt.json)
contains source SHA, platform, test counts/exits, hashes of raw stdout/stderr,
release binary hash, harness/report hashes and resource samples. Original raw
logs, two harness variants, a first failed fixture receipt and disposable
profiles stay ignored on D:. The first test attempt incorrectly omitted a valid
approved root in the negative-identity fixture; the test received a different
error, so it did **not** count as proof of identity enforcement. The corrected
fixture supplied a public approved root and explicitly disabled unattributed
calls; the next complete test run passed all 17 scenarios. No production code
change was made, and the original failure trace remains preserved.

CL-10's earlier independent cloud source/security audit is the integration
review for this backend scope; this local gate checks platform execution and
evidence. **No separately evidenced third-party security sign-off** is claimed.
No GUI, embedded browser, browser extension, installer, signed-in provider,
private profile, external SSH host, proxy, tunnel, screen/audio or final product
parity was tested. These require the future 7P-A, 7P-B, 7P-L1 and later gates.

```text
UNIT: CL-L6
GITHUB SOURCE UNDER TEST: b85471c9bce70842fd84242d4c2f72fe31df5532
HOST: physical Windows 11 x64, D: NTFS
VALIDATION: 312 Rust tests; strict Clippy; rustfmt; release; 17 native runtime checks — PASS
PERFORMANCE: five cold/five warm latencies and Windows working-set samples; no comparable baseline
LOCAL REQUIRED: NONE for the bounded phase 1–6 headless backend closure
YOU REQUIRED: NO
KNOWN LIMITS: browser/UI/installer and real-account/device acceptance remain NOT RUN
NEXT: 7P-A (cloud), then 7P-B (cloud), then 7P-L1 (native Windows feasibility)
```
