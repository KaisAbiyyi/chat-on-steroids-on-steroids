# 16L1 native stage publish failure — new attested candidate handoff

> STOP: physical 16L1 remains FAIL / BLOCKED. This is cloud-only remediation.
> Do not run the new installer until exact artifact hashes and source identity
> have been independently checked and the PR has been reviewed and merged.

## Historical physical evidence — preserve it

- Original build source: 9939ea0a9fdc592e6821296e07eb0b7db3653adb
- Original GitHub Actions run: 40710902296 / attempt 1
- Original installer SHA-256: ce28a4f8c9c1edbd09a70146eadfab5f46d508d1fba7df92fe2427c064ab4fb4
- The owner verified provenance and five Sigstore proofs, yet native --install
  returned exit 3. Final installed root was absent; an isolated protected stage
  retained active.json only. Original physical evidence must remain unchanged.
- No Defender 1123/1124 events were found in the inspected window; this does
  not prove that filesystem scanning or a handle lock was absent.

## Code-path diagnosis and repair

InstallOrUpdate seals the staged slot, verifies its signed/unsigned source
identity, writes the state selector, and then publishes the sibling stage
using Directory.Move. The old cleanup could delete the issuer/slot after
publication failure, leaving active.json and hiding the original exception.
That is consistent with owner observations, but no old Win32 operation code
was recorded; Directory.Move cannot yet be conclusively named as the failed
native call.

The replacement checks stage ACL/reparse and absent destination on each
publish attempt, performs bounded retries on Windows error 5/32/33, and
never grants new permissions or overwrites an existing target. Every
critical phase reports operation, source/destination, exception type,
HRESULT and Win32 code (where available). The primary failure is always
preserved. After a failed publication, the stage is quarantined intact,
not automatically deleted; recover refuses stage shells that already
have a selector. The owner may inspect and retain the existing failed
stage without unsupported cleanup.

## New GitHub source-attested artifact

Changing Program.cs changes the native installer executable. The previous
run, artifact ID and SHA-256 are NOT approval for the replacement.

The new 16PKG native GitHub-hosted Windows workflow must, for the final
candidate source commit:
1. Build a real runtime, renderer and installer with identical source SHA.
2. Pass protected-owner-pattern NTFS directory rename and full unsigned
   lifecycle testing, plus signed TEST fixture negative publish scenarios.
3. Generate new package-receipt.json and Sigstore/OIDC attestations for the
   receipt, actual CoSoSInstaller.exe, candidate.zip and update executables.
4. Record NEW build source SHA, original PR event SHA, run ID/attempt,
   GitHub workflow ID, artifact ID, artifact SHA-256, setup SHA-256,
   update setup digests and manifest digests. Verify the new tuple independently.
5. Obtain an independent security review of PR #135 and explicit merge
   approval. Do not treat PR-run OIDC as approved release authority.

## Owner-local qualification — NOT AUTHORIZED YET

Only after new approval tuple and local verifier have been pinned and reviewed:
- Keep the previous D: qualification workspace, stage and evidence untouched.
- Reuse only the already verified protected top-level boundary
  D:\CoSoS-16L1-Qualification, never change D: or D:\projects permissions.
- Choose a fresh isolated direct child, for example:
  D:\CoSoS-16L1-Qualification\16L1-<NEW_SOURCE_SHA>-<NEW_8_TO_32_HEX_NONCE>
- Download only the new artifact using its exact independently verified
  workflow/run/attempt/artifact ID; verify SHA-256 and Sigstore/OIDC claims.
- The existing local runner and guards are intentionally pinned to the
  historical 9939... source and must NOT be bypassed or reused for this
  new candidate until a separately security-reviewed pin/verification update
  exists. If any pin or verified tooling is missing, STOP.
- In a separately authorized elevated PowerShell 7 session, test native
  clean install, launch, upgrade, rollback, recovery, finalization,
  uninstall, and negative failures using only disposable profiles.
- Record a new qualification.json under the fresh workspace; preserve
  the failed historical evidence for comparison. Do not claim PASS from
  cloud CI or a successful provenance preflight alone.

Do not disable Defender, SmartScreen or UAC. Do not import TEST roots,
modify any owner-machine ACL from cloud or touch existing private profiles.
