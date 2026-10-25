# CoSoS 16E — Disposable install / upgrade / rollback matrix

Task: [16E in Notion](https://app.notion.com/p/400c4a6aaf7381aea103e09ca25f3dd3). Exclusive claim: [#117](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/117). Work branch: work/16e-install-matrix-20261025; lock: release-installer-test.

## Scope and dependencies

- Accepted 16B PR #110 provides fail-closed staged release preflight (canonical version, pinning, manifest hashes, trusted publisher/Authenticode). Production 16B CLI NEVER accepts a fake signer.
- Accepted 16C PR #111 provides source-bound unsigned headless-only bundle verification, dependency licensing and provenance. Its ZIP cannot be installed as a signed native release.
- Accepted 16A PR #103 provides copy-only backup/restore in Rust, with verified manifest hash. The hosted 16E workflow reruns its cos-app Rust unit/regression suite on both OS targets.
- This task adds only a synthetic disposable transaction harness; there is deliberately no real installation path, downloaded updater, elevated process, silent migration, in-place downgrade, signing key or user profile touch.

## Invariants exercised

1. Clean install of a canonical release and version-increasing upgrade after 16B preflight; full rehash/re-signature simulation after copying to a new retained slot.
2. Atomic selector points to exactly one fully verified release; incomplete copy or denied preflight never publishes. A fault after atomic publish may leave either previous or next complete release, never partial installation.
3. Explicit rollback is selection of a retained, revalidated release slot, not a 16B downgrade installation or restoration of a running application's data.
4. Failpoints: after preflight, after copy, before publish, after publish, rollback before/after publish; recovery checks selector state in each case.
5. Negative coverage: downgrade, equal version, absent prior version, hash/manifest tamper, unexpected staged file, different source commit, unauthorized publisher, corrupt/traversing selector, symlink, corrupted retained rollback source.
6. An unsigned 16C headless archive remains BLOCKED for native installation even when its reproducible bundle contract independently verifies.

## How to run

Hosted [16E workflow](../../.github/workflows/16e-release-matrix.yml) on Windows and Ubuntu:

    python -m unittest tools.test_release_16b tools.test_release_16c tools.test_release_16e -v
    python -m py_compile tools/release_16b.py tools/release_16c.py tools/release_16e.py tools/test_release_16b.py tools/test_release_16c.py tools/test_release_16e.py
    cargo test --locked -p cos-app

This is a cloud functional contract test, not native desktop installation verification. GitHub Actions Windows runner does not prove user-machine WebView2 deployment, signed MSI/MSIX trust enrollment, privilege/ACL behavior, persistence across reboot, real installer rollback, retention/final cleanup, or native performance.

## Follow-up gates (NOT_RUN for this task)

- 16L1: Windows actual clean install/uninstall/updater on disposable local profiles; validate real signatures and runtime prerequisites before considering shipment.
- 16U1: owner consent for any private profile copy. No automatic consent or access.
- 16L2: only owner-approved profile copy and backup/rollback physical recovery; never the live original.
- 16F: final release provenance/security/compliance review, requiring traceable exact-head CI and independent acceptance.

**Acceptance:** exact pull-request HEAD, Windows+Ubuntu workflow success and workspace CI success, independent review, then merge receipt. Do not set 16E Done based only on authored tests.
