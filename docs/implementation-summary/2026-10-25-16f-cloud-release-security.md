# CoSoS 16F — Security and provenance closure (cloud-only)

Task: [16F](https://app.notion.com/p/400c4a6aaf7381c685aaea92e95f639a) · exclusive [cloud claim #121](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/121). Branch `work/16f-release-provenance-20261025`. Exclusive lock `release-closure`. Source baseline `9ab33009aafdd24ebd4ec093e7c46f4e45985846`.

## Release verdict

**CLOUD SOURCE/CONTRACT AUDIT ONLY — NOT RELEASE READY.** Do not ship a native installer, migrate a user profile, claim full GUI performance, or authorize public product release from this document. No real signed native installer or out-of-band trusted publisher certificate has been examined. No independent legal certification is asserted. This document closes only cloud report preparation, subject to independent review, exact-head Linux/Windows checks and merge.

## Auditable source chain

The exact GitHub merge commits were read before claim. `tools/release_16f.py` checks ancestry on the exact PR SHA (not the synthetic PR merge-ref), re-hashes the checked-out version of pinned contracts and compares it to the Git blob at HEAD. It rejects missing parents/files, changed DAG, dirty protected contracts, source mismatch and tampered/forged reports. It does not assert GitHub Actions statuses by reading its own workflow or assume signing authenticity from a SHA alone.

| Unit | Scope and security boundary | Accepted merged PR | Exact merge SHA |
| --- | --- | --- | --- |
| 16A | Copy-only backup/restore, new destinations; original unchanged; hash receipt alone not issuer authenticity | [#103](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/103) | `a2d3781dd499378ef53e022b543c891fe12df93f` |
| 16B | Staged Windows release preflight; native Authenticode and pinned publisher required; **no installer** | [#110](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/110) | `3b8e6d2fe006035dda98ab9fa38dfe73ec1f1a68` |
| 16C | Locked dependencies, notices, hashed headless ZIP; unsigned, no WebView2/native host | [#111](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/111) | `326b0fca66c8deefce85161e298ff20262a57358` |
| 16D | Headless CLI runner-only timing, root working-set and ZIP size, not GUI/product budget | [#119](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/119) | `9ab33009aafdd24ebd4ec093e7c46f4e45985846` |
| 16E | Injected-fault, disposable synthetic install/rollback transaction; no actual system changes | [#118](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/118) | `c9f17ed5654e42cd4497f01309d9b6cfcf3dd0f1` |

## Threat and license/provenance review

- **Artifact substitution / malicious publisher:** 16B requires an out-of-band trusted thumbprint, a source-pinned release manifest, byte sizes and SHA-256 and native Authenticode on every relevant executable. These gates are only contract tests until an actual trusted signed product candidate exists. Synthetic signer injections are restricted to test API, not CLI.
- **Unsigned and incomplete distribution:** 16C bundle is deliberately unsigned/headless and lacks actual desktop WebView2 host and installed runtime. Hash consistency proves bytes against local source, not publisher identity or production ship-ability. Verify collected license text and platform-specific redistributable obligations with a real release before distribution; a locked SPDX allowlist is not legal advice or clearance.
- **Private-data exposure and rollback:** 16A copies only its known file scope, refuses overlap/symlink/tamper and requires a trusted external manifest hash. WebView2/provider storage, filesystem ACLs, live-process consistency, retention and backup authenticity are outside that cloud test. Owner consent (16U1) is required **before** copying private data; 16L2 must use only an approved copy.
- **Atomic installation:** 16E exercises synthetic crash/integrity paths, never elevation, reboot, package manager, real updater, uninstall, rollback permissions or persistence. 16L1 must independently demonstrate these native cases.
- **Misleading performance evidence:** 16D metrics are GitHub-hosted CLI-only (new-profile vs reused-profile) and process-root working set. Browser tree RAM, comparable original CoS workloads, desktop interactivity, package startup/size and frozen performance budgets are NOT proven. Product performance acceptance belongs to native 16L3 and PERF gates.
- **Provenance claim limitations:** Git ancestry and blob hashes bind the source under audit; they do not independently authenticate repository control, merge reviewer identity, binary signatures, CI receipts, or supplier trust. Exact-head CI link, reviewer name, signed candidate and native gate receipts must be appended by the checker.

## Automated evidence and reproducibility

The Windows+Ubuntu [16F workflow](../../.github/workflows/16f-release-report.yml) checks out the exact push/PR HEAD with full history, runs positive and negative predecessor regressions, generates a bounded JSON source report, re-verifies it against checked-out Git blobs, and uploads a 14-day artifact. Run:

```sh
python -m unittest tools.test_release_16f tools.test_release_16b tools.test_release_16c tools.test_release_16d tools.test_release_16e -v
python tools/release_16f.py report --expected-head "$(git rev-parse HEAD)" --receipt 16f-cloud-release-report.json
python tools/release_16f.py verify --expected-head "$(git rev-parse HEAD)" --receipt 16f-cloud-release-report.json
```

Artifact fields `native_owner_gates` are uniformly `NOT_RUN`, `product_release_ready=false`, and cloud closure is provisional until exact-head CI and independent/self-review acceptance outside that artifact. No owner data, API token, environment values, provider transcript, workstation identifier or private profile is captured in the artifact.

## Mandatory handoffs / blockers

1. **16L1 — native Windows install/uninstall/update:** unproven; verify signed candidate, publisher root/trust, WebView2 availability, permissions and real rollback, using isolated disposable profile.
2. **16U1 — owner consent:** unproven; explicit approval of source, copied scope, backup destination and irreversible/privacy risks before private-profile operation.
3. **16L2 — migration physical copy only:** unproven; gated by 16U1, no live original edits.
4. **16L3 — final native startup/RAM/size:** gated by 16F, 16L1 and 16L2; use matched real product workload, full process tree/GUI and reproducible source receipts.
5. **16U2 — owner final decision:** only after 16L3 acceptance. No silent release signoff.
6. **PERF-05/06:** separate matched native/product performance and frozen-budget decisions; never promote hosted headless metrics to RAM savings or release budgets.

### Acceptance ledger (checker to complete)

- 16F exact source SHA: **PENDING PR HEAD**.
- 16F Windows+Ubuntu workflow: **NOT_RUN at authored stage**; fill exact run IDs after execution.
- Existing workspace Rust CI and task-graph CI: **NOT_RUN at authored stage**; fill exact run IDs after execution.
- Security/negative-case self-review: **PENDING**.
- Independent review and merged PR: **PENDING**.
- 16L1, 16U1, 16L2, 16L3, 16U2, PERF-05/06: **NOT_RUN** / never inferred from cloud.

**Next:** checker confirms exact PR HEAD CI + diff + evidence; merge cloud 16F only if required checks pass, update Notion, release `release-closure`. 16L3 stays blocked until **all** its AND prerequisites (16F/16L1/16L2) are accepted.
