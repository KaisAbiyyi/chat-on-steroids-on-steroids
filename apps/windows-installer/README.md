# 16PKG — Windows native installer trust and qualification

16PKG packages the **real** Rust runtime, C#/WebView2 host and renderer assets, their dependencies and license into a Windows x64 installer. It supports two explicit transaction modes. The owner approved transparent **unsigned** personal and public-beta distribution on 2026-10-28. Authenticode publisher signing is optional.

## Unsigned manual-digest channel

The installer uses `--unsigned-sha256 EXPECTED_EXE_SHA256` with each mutation: `--install`, `--update`, `--rollback`, `--finalize`, `--recover` and `--uninstall`. This is an affirmative per-invocation acknowledgment of unsigned software. The digest must have been obtained **independently** from trusted GitHub artifact/source provenance before running the EXE. The installer compares that caller-provided digest against its own current executable; incorrect hashes fail before an install-root mutation. There is no background or silent remote updater. Do not use an installer-reported hash as its own approval.

To establish trust, obtain the artifact through a known legitimate repository/workflow, verify the GitHub Sigstore/SLSA attestation for the **installer bytes** and **candidate ZIP**, check the signer workflow and the source claim, verify the separately attested `package-receipt.json`, then compare the actual installer and archive SHA-256 hashes with the receipt. The attestation is an externally anchored GitHub Actions workflow identity; it is **not** publisher identity, code signing, or automatically proof that a PR build should be trusted. Independently review the exact source commit and release authorization before distributing it. In particular, `pull_request` attestations show a CI candidate: they do **not** prove a reviewed, protected-main production release.

Example with the **verified exact-head Actions run and artifact name** (substitute real values; never infer SHA-256 from the EXE itself):

```powershell
gh run download <VERIFIED_RUN_ID> --repo KaisAbiyyi/chat-on-steroids-on-steroids --name <EXACT_SHA_ARTIFACT_NAME> --dir D:\projects\CoS-Rust\qualification\unsigned
Set-Location D:\projects\CoS-Rust\qualification\unsigned
gh attestation verify .\package-receipt.json --repo KaisAbiyyi/chat-on-steroids-on-steroids --signer-workflow KaisAbiyyi/chat-on-steroids-on-steroids/.github/workflows/16pkg-native-installer.yml
gh attestation verify .\installer-package\CoSoSInstaller.exe --repo KaisAbiyyi/chat-on-steroids-on-steroids --signer-workflow KaisAbiyyi/chat-on-steroids-on-steroids/.github/workflows/16pkg-native-installer.yml
gh attestation verify .\candidate.zip --repo KaisAbiyyi/chat-on-steroids-on-steroids --signer-workflow KaisAbiyyi/chat-on-steroids-on-steroids/.github/workflows/16pkg-native-installer.yml
$receipt=Get-Content .\package-receipt.json -Raw | ConvertFrom-Json
$actual=(Get-FileHash .\installer-package\CoSoSInstaller.exe -Algorithm SHA256).Hash.ToLowerInvariant()
if($receipt.sourceSha -ne '<INDEPENDENTLY_REVIEWED_SOURCE_SHA>' -or $actual -ne $receipt.setupSha256){throw 'Source or setup digest mismatch'}
$trustedDigest=$receipt.setupSha256
```

Only after independent verification, in an elevated disposable Windows console and an owner-approved isolated D: test path:

```powershell
$setup=(Resolve-Path .\installer-package\CoSoSInstaller.exe).Path
$root='D:\projects\CoS-Rust\qualification\unsigned\installed'
& $setup --install $root --unsigned-sha256 $trustedDigest
# Later: verify a NEW >high-water version's own attested installer and digest.
& $newSetup --update $root --unsigned-sha256 $newTrustedDigest
& $newSetup --rollback $root --unsigned-sha256 $newTrustedDigest
& $newSetup --recover $root --unsigned-sha256 $newTrustedDigest
& $newSetup --finalize $root --unsigned-sha256 $newTrustedDigest
& $newSetup --uninstall $root --unsigned-sha256 $newTrustedDigest
```

Windows may display an **Unknown Publisher** warning or SmartScreen protection for unsigned binaries. Do not disable Defender/SmartScreen, weaken UAC, install TEST roots, or import any certificate for this channel. If Windows blocks the file, stop and record the refusal. Installing does not migrate or access existing CoS/CoSoS profiles.

The unsigned channel stores a SHA-256 marker next to each exact copied release issuer inside the administrator/SYSTEM-owned protected slot. The marker + embedded bundle checks detect post-install modifications and bind cleanup to the original file allowlist. Protected `active.json` preserves high-water version state; the mutex, safe D: ancestry, denied reparse points, ACL tests, atomic moves, rollback, recovery and refusal of foreign data remain required. Older signed installations are a separate trust domain: their state publisher must match; the unsigned channel cannot silently adopt them.

**Security limits:** A raw SHA-256 is an integrity check, not independent authentication unless the expected value was obtained out of band. A digest and manifest both supplied by the same untrusted package are insufficient. A compromised GitHub maintainer/workflow can produce an attested malicious build. An administrator able to take ownership or replace verifier/OS ACLs lies outside the documented security boundary. Unsigned binaries lack Authenticode publisher identity and may be blocked by Windows SmartScreen. No guarantee is claimed against administrators or an altered host OS.

## Compatibility: optional Authenticode mode

The original `--publisher PIN` path still requires a valid publisher-signed executable, compile-time pinned thumbprint, issuer-signed installed slots and exact identity. It does not import or trust test certificates for production. Synthetic signed-TEST CI uses an elevated disposable GitHub-hosted Windows runner, temporary LocalMachine Root/TrustedPublisher trust and CurrentUser My key, with explicit cleanup and separate evidence; no private Windows machine is modified.

## Cloud evidence and physical gate

`.github/workflows/16pkg-native-installer.yml` builds the real source-bound Rust + WebView2 desktop bundle, the actual unsigned installer, real unsigned native lifecycle assertions, SHA-256 receipts and separate GitHub OIDC attestations for the EXE/ZIP/receipt. The companion `.github/workflows/16pkg-installer-lifecycle.yml` preserves historical signed-TEST synthetic lifecycle coverage. Confirm final **exact-head** CI and artifacts before claiming either gate PASS.

The physical `16L1` remains **NOT RUN** until reviewed code is merged, the exact installer digest/provenance is verified and the designated local Windows qualification agent runs an isolated D: test. CI does not attest user-device security, interactive SmartScreen/UAC behavior, or native physical acceptance. Use the checked-in local handoff script and its exact commit-bound receipt for that separate step.
