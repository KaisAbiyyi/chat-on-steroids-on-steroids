# 16PKG-01: hosted native Windows security verification (2026-10-28)

## Unit and ownership
- Repository: `KaisAbiyyi/chat-on-steroids-on-steroids`.
- Issue [#130](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/130), existing [Draft PR #131](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/131), branch `work/16pkg-01-native-installer-20261027`, exclusive lock `release-native-installer`. Do not merge this draft as a production release.
- Test-only Windows hosted security source `6df964879a4dbd2791002b73914b90f9dafcbf34` passed [workflow 40645499739](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/40645499739); its artifact #12797511576 has SHA-256 `7003fb2f52adbddd66a69961e08c115a5d3528419c1e6bb132eb2c46b22b8e78`. This is a historical test checkpoint, not evidence for subsequent commits.

## Corrections and boundaries
1. The Python release builder and Rust payload accept at most **127 renderer members**. A synthetic 128th renderer member must be refused; independent controlled child fixtures exercise large stdout, large stderr, oversized streams, malformed outputs, nonzero exits and timeout/reaping.
2. An ephemeral short-lived TEST certificate is installed in the disposable runner's LocalMachine Root + TrustedPublisher. CurrentUser Root trust import hung on the earlier runner. Independent [diagnostic 40642249837](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/40642249837) proved LocalMachine trust and Authenticode Valid plus teardown. The C# verifier pins the installed Windows PowerShell security module search path rather than inheriting untrusted `PSModulePath`.
3. Production-compiled binaries reject short-lived TEST identities and self-signed leaf signers even when the temporary issuer happens to be trusted by Windows. Test facilities and fault injection are compiled behind `COS_16PKG_TEST`; no runtime environment variable/CLI argument enables them in production.
4. Native TEST suite executes clean install, update, crash recovery, rollback, re-update, finalize, signed slot proof, interrupted uninstall recovery, downgrade/replay refusal, state and manifest tampering, same-publisher foreign release proof refusal, tampered binary and joint binary/manifest/state refusal, junction/path denial, and preservation of unrelated files and directories.
5. During security self-review, recovery of empty stage-shaped sibling folders was tightened: pattern matching alone is insufficient ownership evidence. The stage must have an authenticated admin/SYSTEM ACL before deletion; unprotected interrupted stages remain quarantined.

## Acceptance
- `CLOUD_CODE_BUILD`: verify latest exact source through Windows native package workflow and Rust Windows/Ubuntu/fmt/strict Clippy; do not extrapolate from an earlier SHA.
- `CLOUD_NATIVE_E2E`: prior checkpoint `PASS`, but test the final exact source and artifact separately.
- `ISOLATED_WINDOWS_E2E`: GitHub-hosted disposable runner qualifies the synthetic transaction matrix. No physical user-device signature acceptance is implied.
- `PRODUCTION_SIGNING`: `BLOCKED` awaiting approved publisher certificate/provider, protected-main source-bound issuer/provenance, and independently issued 15HOST attestation. PR OIDC signed unsigned-candidate receipt is not release authorization.
- `PHYSICAL_16L1`: `NOT_RUN` until an authorized, fully signed production candidate is prepared.
- Local fallback harness is not invoked for this scope while disposable hosted Windows E2E is functional; no owner drive or profile was accessed.

## Next
Independent exact-HEAD security/cloud review after successful GitHub CI. Preserve Draft PR, open issue and exclusive lock pending sign-off; release signing and physical Windows 16L1 proceed as distinct authorized gates.
