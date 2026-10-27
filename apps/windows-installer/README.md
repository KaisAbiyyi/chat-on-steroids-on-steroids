# 16PKG native release and security boundaries

16PKG builds the actual `cos-rust.exe` + C#/WebView2 renderer and an offline `CoSoSInstaller.exe`. The PR workflow publishes an **unsigned source-bound preview** with GitHub OIDC candidate provenance; no PR-produced artifact is an authorized production release.

## Production trust policy

- Mutations require a Windows administrator token, a valid Windows Authenticode signature on the executing setup binary, and a thumbprint pinned into that **already-signed** binary at release build time. Thumbprint is not selected by the caller.
- Every installed slot retains the exact publisher-signed installer under `authority/issuer.exe`. During any update, rollback, recovery or uninstall the executable is verified with Windows Authenticode and queried for its immutable embedded release manifest digest, exact source commit, version and publisher. Hashes read from a writable `release.json` never alone authorize installed bytes.
- Installer state and all child files/directories use admin/SYSTEM-only NTFS ACLs with their owner set to built-in Administrators. Non-elevated users, including a split-token instance of the same Windows account, do not own the installed files. An administrator-equivalent adversary able to replace ACLs, the system verifier or the approved signing keys is **outside the claimed threat boundary**.
- A named OS mutex serializes mutations per absolute installation root; symlinks/junctions and unexpected children fail closed. File indexes and embedded issuer hashes are reverified before publication and deletion. Rust 15HOST continues to independently require the authorized protected-main GitHub OIDC attestation of the exact renderer-integrity manifest.
- Updates enforce a monotonic high-water version even after rollback. Previous slots can be finalized and pruned. Interrupted stages, orphaned signed slots and uninstall tombstones have fail-closed recovery. Unowned files and unexpected directories cause refusal rather than deletion.

## Cloud CI qualification

`.github/workflows/16pkg-native-installer.yml` runs Windows native compilation, full 16B/16C/16E regression suites and separate **positive** disposable install/update/rollback/re-update/uninstall tests using a CI-generated test-only code-signing certificate. The test signer is imported only into the runner's ephemeral user trust store and removed afterwards. Test fixtures and D: paths are synthetic; this is **not** vendor code-signing authorization and is **not** a 16L1 physical qualification. The candidate bundle, setup EXE, exact commit, artifact hashes, test-only issuer receipt and OIDC candidate evidence are retained in the workflow artifacts/logs.

`workflow_dispatch` accepts only canonical `x.y.z` versions; a new authorized production update must be strictly greater than the installed high-water version. No synthetic fixture can authorize a real release.

## Required external production gates

1. An authorized and protected publisher certificate / managed code-signing provider, with independently approved thumbprint and no secrets in repository, logs or test fixture.
2. The actual protected-`main` release workflow identity and attested renderer manifest (15HOST) bound to the exact release commit, plus approved code-signature verification for all executable components per 16B.
3. Review of the exact head, dedicated issuer/provenance build evidence, and real physical Windows 16L1 install/uninstall/updater/rollback using **disposable** profiles, with owner acceptance.

Until all three are satisfied, keep PR #131 **Draft**, task 16PKG-01 **Review/Pending**, and 16L1 **Blocked/NOT_RUN**. Do not merge or promote the preview ZIP and test-signed fixtures to production.
