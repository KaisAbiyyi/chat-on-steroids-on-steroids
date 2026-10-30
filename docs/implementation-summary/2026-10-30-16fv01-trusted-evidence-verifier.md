# 16FV-01 — Production Trusted Evidence Verifier

**Task:** 16FV-01  
**Issue:** https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/139  
**Implementation PR:** https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/141 (Draft; not merged)  
**Existing branch:** work/16fv-01-trusted-evidence-verifier-20261030  
**Original base SHA:** cc180575498e9f45d13c05c86a0a3bb4e8e0c1be  
**Reclaim starting HEAD:** 5e494b3dc76d97e56cffc9a7521e024a93b8b3c0  
**Resource lock:** freshstart-evidence-verifier  
**Execution:** GitHub integration and GitHub Actions only  
**Owner production trust root:** NOT CONFIGURED

## Acceptance boundary

16FV-01 implements the production verifier required before 16LF-01 may accept an owner-local Windows synthetic fresh-start receipt. The verifier is fail-closed and consumes immutable GitHub records, actual GitHub PR metadata and diff bytes, the historical 16L1 Actions artifact and Sigstore provenance, and a digest-verified evidence bundle.

The accepted review governance is exactly GPT6_OWNER_ATTESTED:

1. GPT-6 technical review report with a PASS verdict, canonical schema, and exact source SHA/diff SHA-256 bindings.
2. A separate immutable owner-attestation GitHub comment accepting that exact report digest, receipt digest, source SHA and diff digest.

Both records may be authored by the same owner account. This is not GitHub independent approval. Owner attestation authenticates the owner's acceptance of the technical report; it is not cryptographic proof of model authorship. Legacy independent_review and reviewer_independent fields are rejected.

No production root is established by this implementation. Missing root returns TRUST_ROOT_UNCONFIGURED; malformed or non-pinned root returns TRUST_ROOT_INVALID; API/provenance failures return VERIFICATION_ERROR. Test-only fixtures are accepted only by the internal test constructor and are rejected by the production environment loader.

## Production components

- tools/fresh_evidence_contract.py — canonical PR/issue/repository/owner identities, trust-root schema v2, verifier-review and owner-attestation schemas/markers, predecessor/artifact identities and protected workflow path.
- tools/fresh_evidence_verifier.py — read-only GitHub API client, protected-environment trust-root v2 parser, merged 16FS-01 and PR #141 review/owner-acceptance checks, exact repository/comment identity, source/diff/merge ancestry binding, verifier-file SHA binding, five digest-scoped Sigstore proofs, Windows path containment/reparse-point evidence, structured evidence-bundle verifier and typed FreshEvidenceVerification constructor.
- tools/fixtures/16fv-trust-root.test.json — explicitly test-only trust-root fixture; authority is test-only and the production environment loader rejects it.
- tools/fixtures/16fs01-pr138.diff — byte-exact golden diff from merged predecessor PR #138; the production verifier recomputes its SHA-256 against the pinned 16FS-01 contract.
- tools/task_board.py — schema-v3 receipt validation, typed 17-fact binding, production verifier loading in the CLI, fail-closed acceptance path and canonical DAG constraints.
- tools/test_fresh_evidence_verifier.py — positive and adversarial verifier tests, synthetic artifact archive tests, five mocked Sigstore proof checks, production root isolation and CLI fail-closed tests.
- tools/test_task_board.py and tools/test_release_16f.py — canonical DAG, receipt and release-gate regression coverage.
- .github/workflows/16fv-trusted-evidence-verifier.yml — Windows/Ubuntu exact-HEAD matrix, machine-readable qualification artifact and historical 16L1 artifact revalidation; it receives no production trust-root secret.
- .github/workflows/16fv-production-verify.yml — manual workflow restricted to workflow_dispatch on main, exact main checkout, protected fresh-evidence-production Environment, read-only permissions and secret access only inside the protected job.
- tools/record_16fv_qualification.py — verifies git rev-parse HEAD equals the workflow's claimed tested SHA before recording the verifier file SHA-256, runner OS, run ID/attempt, test outcomes and explicit non-acceptance of production authority.
- tools/test_record_16fv_qualification.py — tests exact checkout binding, run URL identity and fail-closed qualification records.

The production verifier is imported by the normal task-board CLI. It is not a test adapter and there is no unconditional success or legacy acceptance fallback. The owner-controlled trust-root schema v2 contains a separate verifier_review object for PR #141. The verifier fetches the actual PR, recomputes the GitHub diff digest, verifies the exact source and merge SHAs, checks that the 16FS-01 merge is in PR #141 source ancestry, and validates two distinct immutable owner-authored comments: the GPT-6 PASS report and the separate owner acceptance. Both comments bind the exact repository, PR, source SHA, diff SHA and GPT6_OWNER_ATTESTED policy; each digest is canonical and self-excluding. The owner account is authenticated, but GPT-6 model authorship is explicitly owner-attested rather than cryptographically authenticated. trust-root-status now performs the remote checks instead of merely parsing the secret. The CLI does not read a trust root from task-registry JSON, receipt fields, PR comments or candidate-supplied booleans.

## 17-fact contract mapping

| # | Contract fact | Production verification | Adversarial coverage |
|---:|---|---|---|
| 1 | receipt_exists | Fetches the exact canonical issue-comment ID from the registry URL; checks actual API record and issue identity. | Missing receipt record. |
| 2 | receipt_integrity_verified | SHA-256 over canonical receipt JSON; exact equality between immutable comment payload and registry payload. | Tampered receipt and digest mismatch. |
| 3 | review_report_exists | Fetches the exact review-report issue comment; verifies canonical record identity and unchanged timestamps. | Missing report. |
| 4 | artifact_exists | Fetches exact Actions artifact metadata and run artifact listing; checks ID, name, run, repository, head SHA, expiry and archive digest. | Missing, expired or modified artifact. |
| 5 | review_mode_authorized | Pinned trust-root mode and report policy must equal GPT6_OWNER_ATTESTED. | Wrong mode, legacy schema and legacy independent-review field. |
| 6 | owner_identity_verified | Fetches /users/KaisAbiyyi; numeric GitHub user ID must equal the owner-root ID. | Unauthorized owner ID. |
| 7 | owner_attestation_exists | Fetches the separately referenced owner-attestation issue comment. | Missing attestation. |
| 8 | owner_attestation_valid | Canonical attestation SHA-256; exact accepted report, receipt, source, diff, artifact and review-mode bindings. | Modified attestation, wrong source and wrong diff. |
| 9 | owner_attestation_author_identity_verified | Actual receipt, report and attestation comment authors must match the pinned owner login and numeric ID. | Attestation author mismatch and forged issue provenance. |
| 10 | gpt6_review_verdict_pass | Requires report schema cosos.release.cloud-provenance.v2, verdict PASS, and exact owner-attested policy. | Non-PASS, wrong schema and legacy policy. |
| 11 | reviewed_source_sha_matches | Fetches source PR #135 and binds report, receipt and attestation to its exact merged head SHA. | Wrong source SHA. |
| 12 | reviewed_diff_digest_matches | Fetches the actual PR #135 diff bytes from GitHub and recomputes SHA-256. | Wrong report digest and changed provider diff. |
| 13 | receipt_review_artifact_digests_match | Cross-binds receipt, review report, owner attestation, artifact installer digest and package-receipt digest. | Tampered receipt and artifact digest mismatch. |
| 14 | artifact_provenance_verified | Downloads actual archive; checks payload files and package receipt; runs gh attestation verify on five separately attested files and validates SLSA certificate/run identity. | Missing/tampered archive and forged Sigstore provenance. |
| 15 | artifact_source_binding_verified | Checks PR #135, PR #136, exact source/tooling commits, original pull-request event, run attempt, merge ancestry and main ancestry. | Wrong source/run/artifact identity. |
| 16 | fresh_start_checks_verified | Downloads the owner-uploaded evidence bundle; checks ZIP safety, bundle SHA-256, complete manifest, every file hash, all eight exact PASS checks and one raw evidence file per check. | Missing check, missing file, modified file and bundle digest mismatch. |
| 17 | evidence_complete | Requires schema-v3 receipt fields, canonical URLs, repository/PR identity, exact 16FS and 16FV source/merge/file hashes, session/nonce/workspace, Windows build, timestamps, commands, all checks, and complete bundle manifest. | Incomplete, malformed and partially populated receipts. |

FreshEvidenceVerification.authoritative_for() independently rechecks typed identity, source/diff, owner-author, schema, record uniqueness, exact merged predecessor/verifier SHAs and digest bindings before task-board acceptance.

## Receipt and evidence-bundle contract

The task registry stores the schema-v3 receipt plus three canonical immutable issue-comment URLs on issue #140:

- receipt_url — comment containing the marker COSOS-16LF01-RECEIPT-V1 and canonical JSON {schema, receipt, receipt_sha256}.
- review_report_url — separate comment containing the marker COSOS-GPT6-REVIEW-V2 and the v2 PASS report.
- owner_attestation_url — separate comment containing the marker COSOS-OWNER-ATTESTATION-V1 and the owner acceptance record.

The receipt JSON digest is calculated over the exact receipt payload object. The review report and owner attestation each carry their own canonical self-excluding digest. The receipt record is posted first; subsequent report/attestation links and hashes live in the task-registry envelope to avoid circular comment-ID references.

The receipt points to an owner-uploaded ZIP attachment at a GitHub user-attachment URL. The ZIP must contain evidence-manifest.json, qualification.json, and one structured JSON evidence record for every required fresh-start check. The manifest carries SHA-256 for every other file. The qualification record binds repository, source SHA, artifact ID/hash, exact 16FS source/merge SHA, exact 16FV PR #141 head/merge SHA and verifier-file SHA-256, Windows build, unique session ID, workspace nonce/path, timestamps, commands, eight PASS checks and per-check evidence paths.

All file paths are relative and ZIP traversal/symlink/duplicate entries are rejected. The evidence bundle's SHA-256 is checked against the receipt before its contents are used.

Each check evidence file is a JSON record using schema cosos.16lf01.check-evidence.v1. It repeats the repository, source SHA, exact 16FS/16FV pins, artifact, session and workspace bindings and carries a timestamp within the recorded session. The verifier validates check-specific observations rather than trusting a PASS boolean alone:

- fresh_init: new profile created, profile was empty, unique profile ID.
- isolated_storage: Windows drive-absolute workspace/profile/canonical paths are validated with ntpath semantics; profile and canonical paths must be strict descendants on the same drive, traversal/UNC/device paths are rejected, and every path component from the drive root through canonical storage must have explicit GetFileAttributesW(FILE_ATTRIBUTE_REPARSE_POINT) evidence showing no reparse point or junction.
- synthetic_only: artifact is unsigned, native16L1Accepted is false, and inputs are synthetic-only.
- existing_profiles_untouched: before/after snapshot SHA-256 values match.
- no_leakage: leakage_detected is false, cross-profile access count is zero, external data reads are zero.
- rollback: rollback completed, test profile removed, rollback target recorded.
- failure_isolation: injected failure occurred, remained contained, and existing profiles remained unchanged.
- owned_cleanup: workspace and temporary files were removed by the owner.

All eight paths must be distinct safe checks/*.json entries, each file must be covered by the bundle manifest, and its observations must pass the exact check-specific contract. The qualification.json PASS map must equal the statuses derived from these validated records.


## Historical 16L1 artifact trust pins

The production root pins the investigation leads only after validating their actual GitHub provenance and bytes:

- Artifact ID 12868909286, archive SHA-256 07d68ffbd6eb30d444f153c51a6a0430d7019f9c919c141181e267de19e99cd0.
- Installer SHA-256 66adbc6160f99730b029d6f8866aba0291ec98f55a48fa08458954faa29cc76b.
- Source PR #135 head fb1d97a040a28abe74ccbcf5f92e2d33064b85ad, merged at 0685f218e4d4c2b3e6031aad290ea6a4848c7f6e.
- Original pull-request event digest b903132e2c8a32cde5143d40696813fb3233d11d.
- Actions run 40808144696, attempt 1, workflow ID 398496557.
- Accepted verifier-tooling PR #136 head e526c2ed398b0b21dd10f659d2ab1f1ed7f85b07, merge bb3ca8daba5d467b9f097b7e84833da3ee129373.
- Package receipt digest 9c473677c03cead075cfbf0bb6015962fdfc8f2e785e5c2334bb95282a56b308; provenance lookup is digest-scoped at https://api.github.com/repos/KaisAbiyyi/chat-on-steroids-on-steroids/attestations/sha256:9c473677c03cead075cfbf0bb6015962fdfc8f2e785e5c2334bb95282a56b308.

The verifier does not trust these strings merely because they appear in code or a receipt. It re-fetches artifact/run/PR/commit metadata, downloads the actual archive, recomputes archive and file hashes, validates the package receipt, and verifies five distinct Sigstore proofs individually against the exact expected subject-file SHA-256, repository, signer workflow, original PR event digest, run ID and attempt. GitHub's supported gh attestation verify JSON and REST list-by-subject-digest interfaces do not expose a stable unique attestation ID; the unused UI-ID pins are removed. The narrower guarantee is cryptographic provenance for each exact subject digest, not identity of a particular GitHub attestation UI ID. Duplicate, missing, multi-subject, mismatched or unauthorized proof results fail closed. See the [GitHub CLI verification output contract](https://cli.github.com/manual/gh_attestation_verify) and [REST repository attestations API](https://docs.github.com/en/rest/repos/attestations#list-attestations).

## Canonical DAG

- 16FS-01 is reconciled to Done only because merged PR #138 and its separate owner acceptance are verified.
- 16FV-01 is the canonical claimed Cloud task for Issue #139, implemented by Draft PR #141, with dependencies on 16FS-01, 16U1 and 16L1 and exclusive lock freshstart-evidence-verifier. A future Done status is accepted only after the production verifier revalidates the merged 16FS-01 contract, the merged PR #141 identity/source/merge/file digest, the exact PR #141 diff, predecessor ancestry, the GPT-6 PASS report, and the separate immutable owner acceptance pinned by the owner-controlled verifier_review root; a registry Done flag or PR URL alone is never acceptance.
- 16LF-01 remains Blocked / NOT_RUN and now depends on accepted 16FV-01 in addition to 16FS-01, 16U1 and 16L1.
- 16L2 remains Blocked / NOT_RUN / NOT_AUTHORIZED.
- 16L3 remains exactly 16F AND 16L1 AND (16L2 OR 16LF-01).
- No downstream physical qualification or release wave is authorized by this implementation.

## Exact-HEAD GitHub Actions qualification

The dedicated workflow triggers on pushes to work/16fv-*, pull requests and manual dispatch. The required matrix executes on windows-latest and ubuntu-latest, checks out the exact PR head SHA (or pushed SHA), compiles the implementation, runs focused adversarial verifier tests, runs task-board/DAG/release-gate tests and writes machine-readable evidence with the exact tested SHA, verifier-file SHA-256 and OS.

A separate Windows job revalidates the actual historical 16L1 artifact with the existing PowerShell provenance verifier. It does not rerun 16L1 physical qualification. The workflow does not create GitHub Environments or configure secrets. The owner trust root remains a separate owner-controlled action.

A platform is qualified only when its uploaded evidence identifies the same final source SHA and every required step passes. Earlier green runs, skipped jobs or a different HEAD are not qualification.

## Owner-controlled production trust-root handoff — NOT ESTABLISHED

No production trust-root secret or GitHub Environment protection is created or configured by this PR.

The production payload is not embedded in verifier source, the task registry, receipts, PR content or the test fixture. The only production authority source is the owner-controlled secret `COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON`, exposed only to the protected main-only workflow.

After PR #141 has been technically reviewed, separately owner-attested and merged, the owner must:

1. Create the GitHub Environment `fresh-evidence-production`.
2. Configure its required reviewers / deployment protection and restrict which branches may deploy to `main`.
3. Independently construct the JSON payload using trust-root schema v2 with `authority: "owner-controlled"`, repository `KaisAbiyyi/chat-on-steroids-on-steroids` / ID `1424422098`, owner `KaisAbiyyi` / user ID `93252311`, review mode `GPT6_OWNER_ATTESTED`, receipt issue #140, predecessor PR #138 pins, verifier_review PR #141 source/merge/diff/report-comment/report-digest/owner-attestation-comment/attestation-digest pins, and historical 16L1 PR #135 / tooling PR #136 / artifact / run / five subject-file digest pins. Obtain every value from actual GitHub metadata and owner-controlled accepted records; do not copy authority from candidate source, the registry, a receipt or the test fixture.
4. Store that independently assembled JSON as the Environment secret `COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON`.
5. Keep the production workflow token read-only: contents, pull-requests, issues, actions and attestations.
6. Manually dispatch `.github/workflows/16fv-production-verify.yml` with the selected ref set to `main`. The job checks out the exact dispatched main SHA, enters the protected Environment, and runs `python tools/task_board.py trust-root-status`. That command now performs the actual repository/owner, predecessor PR/report/attestation, PR #141 verifier-review/owner-acceptance, diff/source ancestry, historical artifact byte and five Sigstore proof checks; it fails closed unless every remote pin verifies.

The production workflow is `workflow_dispatch` only, has a job-level main-ref guard, uses a protected GitHub Environment and read-only permissions, and reads the secret only in that protected job. The PR-triggered Windows/Ubuntu qualification workflow has no reference to the Environment or secret. No `pull_request_target` job executes candidate code with owner secrets.

The verifier rejects a test-only authority, a wrong repository/owner/review policy, malformed predecessor or artifact pins, or any secret presented outside the exact main-only production workflow context. Missing configuration returns `TRUST_ROOT_UNCONFIGURED`; malformed or out-of-context configuration returns `TRUST_ROOT_INVALID`; remote/API/provenance failures return `VERIFICATION_ERROR`.

No owner trust-root setup is performed by this implementation. Until the owner completes the protected configuration and verification, production trust remains unconfigured and 16LF-01 / 16L3 remain blocked.


## PR #141 verifier-review contract

The owner-controlled trust-root schema is now `cosos.fresh-evidence-trust-root.v2`. Its required `verifier_review` object contains:

- `pr_number` = 141 and `review_mode` = `GPT6_OWNER_ATTESTED`.
- Exact PR head/source SHA and merge commit SHA.
- SHA-256 recomputed from the actual GitHub PR #141 diff bytes.
- Separate immutable report and owner-attestation comment IDs.
- Canonical self-excluding SHA-256 for each record.

The report uses schema `cosos.16fv.verifier-review.v1`, verdict exactly `PASS`, exact repository/PR/source/diff bindings, the owner login/numeric ID, and explicit semantics `OWNER_ATTESTED_NOT_GITHUB_INDEPENDENT_APPROVAL` plus `OWNER_ATTESTED_NOT_CRYPTOGRAPHICALLY_AUTHENTICATED` for model authorship. The separate attestation uses schema `cosos.16fv.owner-attestation.v1`, requires explicit `accepted: true`, and binds the exact report digest, source SHA, diff digest, repository, PR, owner and review mode. Both GitHub comments must be authored by the pinned owner, belong to the exact canonical repository/PR #141 issue URL, have unchanged timestamps, and be distinct. Missing, changed, wrong-author, wrong-repository or mismatched records fail closed.

The production trust-root workflow does not merely parse schema: `trust-root-status` performs the same remote verifier-review checks, in addition to predecessor and historical artifact checks. The production secret remains unconfigured by this PR.

## Windows path and reparse-point evidence

For `isolated_storage`, `profile_storage_root` must be a strict descendant of the disposable workspace and `canonical_path` a strict descendant of the profile storage root. All three must be absolute local Windows paths on the same drive. The verifier rejects traversal components before normalization, mixed-separator traversal, UNC/device paths, drive changes and sibling-prefix escapes. It uses `ntpath` and case-insensitive Windows containment semantics on both CI platforms.

The evidence must include `reparse_point_check_method: GetFileAttributesW(FILE_ATTRIBUTE_REPARSE_POINT)` and one `reparse_point_checks` entry for every path component from the drive root through the workspace, profile root and canonical path. Each entry must bind the exact component path and report `is_reparse_point: false`. Missing, incomplete, duplicated, mismatched or positive reparse-point evidence is rejected. This is an evidence contract only; no owner-local Windows physical qualification is performed by this cloud implementation.

## Historical Sigstore proof guarantee and limitation

The trust root no longer carries the five UI attestation IDs because the supported GitHub interfaces do not expose a stable unique ID for the exact verified proof object. Each proof is instead looked up/verified by its exact subject file digest and must return exactly one verified attestation whose subject digest matches the pinned file. The CLI certificate and SLSA statement must bind the repository ID, workflow, PR merge ref, original event digest, run ID and attempt. The five mappings are package receipt, installer, candidate ZIP, update-02 installer and update-03 installer. The verifier does not claim that a particular GitHub UI attestation ID was pinned.

## Trust-root rotation, replacement and revocation

- **Initial setup:** after PR #141 has its exact-SHA GPT-6 technical PASS report and separate owner acceptance, the owner independently pins the actual comment IDs and self-excluding digests in `verifier_review`, then stores the completed v2 root only in the protected main-only Environment secret.
- **Review replacement:** never edit a pinned comment. Publish a new report and a separate new owner-acceptance comment, then replace both IDs and digests in the protected secret after checking the exact new PR head and actual diff.
- **Owner-attestation revocation:** remove or invalidate the owner-controlled secret to make the trust root unconfigured/invalid, or publish a replacement attestation and atomically update the pinned comment ID/digest. An edited comment fails the unchanged-timestamp check.
- **Trust-root rotation:** rotate the Environment secret as one complete v2 payload. Keep the old root out of candidate files, task-registry JSON, PR text and receipts. Dispatch the main-only verification workflow after rotation and require all remote checks to pass.
- **Failure/revocation:** absent configuration returns `TRUST_ROOT_UNCONFIGURED`; malformed root returns `TRUST_ROOT_INVALID`; missing or mismatched remote records return `VERIFICATION_ERROR`. No task status is promoted automatically.

## Explicit non-deliverables

- No owner trust-root configuration or activation.
- No owner-local filesystem access or local Windows test.
- No 16L1 rerun.
- No 16LF-01 physical PASS.
- No 16L3 acceptance or release activation.
- No PR merge.
