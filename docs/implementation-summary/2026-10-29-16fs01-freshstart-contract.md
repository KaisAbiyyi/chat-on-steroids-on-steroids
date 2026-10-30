# 16FS-01 — Fresh-start release contract (cloud candidate)

- [Claim and source tracking #137](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/137)
- [Notion 16FS-01](https://app.notion.com/p/408c4a6aaf73810e8404e9a06a1c0855)
- Owner policy (16U1, 2026-10-29): initialize CoSoS from a **new empty profile**; **no authorization** to inspect, read, copy, export, back up or migrate any original/private CoS profile.
- 16A–16F, 16PKG-01, 16L1, 16U1: accepted per live Notion/GitHub receipts; only these relevant Phase 16 statuses are selectively reconciled to Done, preserving all unrelated history. Fresh-start acceptance itself and later local gates remain unaccepted.
- This PR is **not** physical Windows qualification, approval to execute private migration or permission to release. All statuses below are pending GPT-6 technical review and explicit owner acceptance. No separate GitHub reviewer is available; no GitHub independent approval will be claimed.

## Old vs proposed phase 16 dependency

Before: \`16L3 <- (16F AND 16L1 AND 16L2)\`, which would force optional private-profile migration even after owner denial.

After GPT-6 technical review, explicit owner acceptance and merge ONLY: \`16L3 <- (16F AND 16L1 AND (16L2 OR 16LF-01))\`. The retained 16L2 branch is **disabled in this phase**. Even if the owner later grants new scoped copy-only consent, it remains blocked until a dedicated trusted private-migration verifier exists; self-reported receipt/review fields cannot accept it. An owner decision recorded as "Done" is **not** affirmative consent. Current 16L2: \`BLOCKED / NOT_RUN / NOT_AUTHORIZED\`.

16FS-01 is a cloud policy change only, never native qualification. New 16LF-01: separate \`L\` (local physical Windows) task under \`windows-freshstart\`; it depends on accepted 16FS-01, 16U1 owner decision and 16L1 native install. Current 16LF-01: \`BLOCKED / NOT_RUN\`. No "migration PASS" is manufactured.

## Local synthetic-only 16LF-01 runbook (NOT EXECUTED)

1. Complete the GPT-6 technical review of 16FS-01 and publish a PASS report bound to the exact source SHA and diff digest. The owner must separately attest acceptance of that exact report digest and source revision. This is GPT6_OWNER_ATTESTED, not GitHub independent approval. Merge only after owner acceptance. Pin the approved source commit SHA, immutable 16L1 verified artifact plus SHA-256 and original provenance/attestation, and local session ID. Reject unreviewed/changed bytes.
2. Obtain separate owner authorization to use **only a new task-owned disposable Windows workspace**. Establish protected isolation boundaries against all existing CoS/CoSoS profiles before launching. Do **not** enumerate, open, inspect, back up, copy, modify or remove private profiles; no private-path presence checks using private contents. Use a fresh nonce-owned path; fail closed on ACL/ownership/reparse hazards.
3. Initialize a genuinely **empty** application-owned profile; verify first-run creation and no re-use of prior profile markers or environment defaults. Use synthetic fixture identities/content only; deny network account logins, private credentials and leakage to logs/temp/artifacts.
4. Induce bounded synthetic failures and rollback; verify preserved unrelated directories and unchanged prior data boundaries without reading their contents; verify recovery and no alias/overlap/cross-profile import.
5. Clean only task-created disposable files after a validated owned-path check, and preserve protected evidence / failure artifacts until reviewed. Reject broad or recursive cleanup of unknown paths and symlinks.
6. Produce redacted, source-bound receipt: exact 40-char commit SHA, verified artifact and receipt SHA-256, unique local session ID, GH issue-attached receipt link, native OS/build and checks (\`fresh_init\`, \`isolated_storage\`, \`synthetic_only\`, \`existing_profiles_untouched\`, \`no_leakage\`, \`rollback\`, \`failure_isolation\`, \`owned_cleanup\`), each PASS/FAIL/NOT_RUN. Require review mode `GPT6_OWNER_ATTESTED`, an actual GPT-6 review report with PASS bound to the exact source SHA and diff digest, and a separate owner attestation bound to the exact report digest and source revision before proposing `16LF-01=done`. Missing/forged/incomplete evidence is rejection; GPT-6 is not represented as a GitHub independent reviewer.
7. 16L3 remains its own real native startup, size, RSS, battery/interaction release gate, separately accepted; 16U2 stays a separate owner decision. 16L1's previous 8/8 and 5/5 receipts are immutable historical evidence and are not rerun.

## Validation boundaries

\`tools/task_board.py\` separates \`fresh_receipt_structurally_valid()\` from \`fresh_receipt_status()\` and authoritative \`accepted()\`. Structural validation is only schema/shape preflight. A correctly formatted receipt, SHA, GitHub issue URL, fabricated review URL, self-authored \`PASS\`, or task-registry status cannot establish acceptance.

The offline resolver has **no production trust root or external verifier** for actual receipt existence/integrity, review-report and 16L1 artifact existence, actual PASS verdict, authorized GPT6_OWNER_ATTESTED mode, owner identity, separate owner-attestation existence/validity, exact report/source/diff bindings, receipt/report/attestation/artifact digest consistency, 16L1 artifact provenance/source binding, all eight fresh-start checks, or evidence completeness. Accordingly, a missing receipt is reported as \\`MISSING\\`, malformed evidence as \\`INVALID\\`, and structurally valid evidence without a trusted verifier as \\`PENDING_EXTERNAL_VERIFICATION\\`. Failed external facts are rejected. None of these states can satisfy \\`16LF-01\\`, unlock \\`16L3\\`, or enter release waves. The typed verifier contract requires each fact and exact binding; it is not loaded from task JSON or receipt fields. The production CLI deliberately injects no verifier and remains fail-closed.

No production trust root, signature, provenance result, external review, or successful local proof is invented. The GPT-6 report digest and separate owner-attestation digest are pinned in the receipt; the owner attestation must reference the exact report digest and source SHA. Model authorship is owner-attested, not cryptographically authenticated. A separately trusted verifier integration is a remaining prerequisite before any genuine \\`16LF-01\\` evidence can be accepted. No GitHub independent approval is claimed; owner acceptance and merge remain separate.

No owner-local or synthetic Windows runtime is executed in 16FS-01; hosted CI exercises only source-level DAG and contract tests. Actual 16LF-01/16L3 are both NOT_RUN. Prior 16F and 16L1 acceptance is preserved; release is not activated.


## Final corrections — 2026-10-30 (targeted candidate; GPT-6 technical review and owner acceptance pending)

### P1 — authoritative fresh-start receipt acceptance

- Renamed the receipt check to \`fresh_receipt_structurally_valid()\`; it is explicitly structural-only.
- Added \`fresh_receipt_status()\` with \`MISSING\`, \`INVALID\`, \`PENDING_EXTERNAL_VERIFICATION\`, \`REJECTED_EXTERNAL_VERIFICATION\`, \`VERIFICATION_ERROR\`, and \`ACCEPTED\` states.
- Added a typed \`FreshEvidenceVerification\` trust-boundary contract covering actual receipt, review and artifact existence; authorized GPT6_OWNER_ATTESTED mode; verified owner identity; separate owner-attestation existence/validity; actual GPT-6 PASS verdict; exact reviewed source SHA and diff digest; receipt/report/artifact digest consistency; artifact provenance and source binding; fresh-start checks; and evidence completeness.
- The offline CLI does not provide a verifier because no defensible repository trust root or implementation was found. Therefore structurally valid candidates remain pending external verification, while missing or malformed evidence is separately reported; none can satisfy \`accepted(16LF-01)\`, the 16L3 OR dependency, readiness, validation, or wave forecasting.
- Tests use a clearly test-only verifier double to exercise the positive typed-contract path; it is not imported by the production CLI and is not evidence of a real review, artifact, or Windows physical run.

### P2 — complete 16F native-gate inventory

- Added \`16LF-01_owner_local_fresh_start\` to the canonical 16F \`GATES\` set.
- Source-bound report generation now requires 16LF-01 to remain registry-\`blocked\` with no evidence and reports it as \`NOT_RUN\`.
- Exact-set and no-promotion regression tests cover omission, forged PASS, and registry status/evidence drift.
- \`product_release_ready=false\`, existing provenance disclaimers, 16L2 restrictions, and exact-source checks remain unchanged.

### Corrective candidate boundaries

- Existing Draft PR #138 and its branch are preserved; no merge or replacement PR.
- 16U1 remains fresh-start with migration consent declined.
- 16L2 remains \`BLOCKED / NOT_RUN / NOT_AUTHORIZED\`.
- 16LF-01 remains \`BLOCKED / NOT_RUN\`; 16L3 remains \`BLOCKED / NOT_RUN\`; 16U2 remains \`NOT_RUN\`.
- No owner-local Windows access, installer execution, private-profile access, migration implementation, or release authorization was performed.
- Candidate acceptance remains pending final exact-HEAD Windows/Ubuntu CI, GPT-6 technical review report, and the owner's separate acceptance of that exact report/source revision. Owner acceptance and merge remain separate.


### Private migration remains disabled

- 16L2 cannot become ready or enter wave forecasts in this phase, even if owner consent is later changed. A dedicated trusted private-migration verifier is not implemented.
- Self-reported physical, verdict, consent, or other task-registry receipt fields cannot establish 16L2 acceptance.
- The 16L3 OR alternative is structurally preserved for future policy evolution, but only 16LF-01 is a viable path under the current fresh-start decision and verifier plan.


## Owner-attested review contract correction — 2026-10-30

- Replaced the mandatory GitHub-independent-review contract with the single accepted review mode `GPT6_OWNER_ATTESTED`. No GitHub independent approval is required or claimed, and GPT-6 is never represented as a GitHub reviewer.
- `FreshEvidenceVerification` now requires actual receipt existence/integrity, review-report and artifact existence, authorized review mode, verified owner identity, separate owner-attestation existence/validity, actual GPT-6 PASS verdict, exact reviewed source SHA and diff digest, receipt/report/artifact digest consistency, 16L1 provenance and source binding, all eight fresh-start checks, and evidence completeness.
- The typed result binds the report URL/digest/verdict/source/diff and artifact digest; it verifies the receipt, report and owner-attestation digests independently, and binds the owner attestation to the exact report digest and source revision. It also binds the actual 16L1 artifact ID, digest, source SHA and provenance digest.
- Structural receipt validation is schema v2 and shape-only. A self-reported review mode, owner login, URL, digest, PASS check or verification boolean cannot establish acceptance. The production CLI supplies no verifier.
- The owner-controlled trust root and production verifier remain explicitly out of scope; 16FV-01 (#139) owns that later implementation after 16FS-01 is accepted and merged. Missing trust root, missing/mismatched owner attestation, unsupported mode, source/diff/report/receipt/artifact mismatch, missing provenance, or verifier failure remains pending/rejected.
- The positive verifier is a test-only double and does not establish production authority. Model authorship is owner-attested, not cryptographically authenticated.
- Preserved: 16L2 `BLOCKED / NOT_RUN / NOT_AUTHORIZED`; 16LF-01 `BLOCKED / NOT_RUN`; 16L3 `BLOCKED / NOT_RUN`; 16U2 `NOT_RUN`; no private-profile access, installer execution, local Windows qualification, merge or release activation.

**Expected handoff:** READY FOR GPT-6 TECHNICAL REVIEW — NOT MERGED — OWNER ACCEPTANCE PENDING.


### GPT-6 final review corrections — 2026-10-30

- The typed verifier now requires canonical provider-native identities for the receipt, GPT-6 review report and owner attestation. The three immutable record identities must be present and pairwise distinct; URL paths and anchors are locations, not proof of record distinctness. Alternate GitHub URL forms for the same comment/review must normalize to the same provider identity and be rejected when reused.
- The verifier separately records the authorized owner identity and the actual owner-attestation author identity. The latter must be verified and match the authorized owner login. The model-review report and owner acceptance may still be authored under the same owner account; they must be separate immutable records. This does not claim GitHub independent approval or cryptographic proof of model authorship.
- OR dependency structures are validated before graph traversal. Cycle detection consumes only validated AND edges and validated OR groups. Malformed groups or members produce controlled validation errors; readiness remains blocked and wave forecasting returns no waves for malformed dependency graphs.
- The production CLI still supplies no verifier or trust root. 16LF-01 remains pending external verification; 16L2 remains BLOCKED / NOT_RUN / NOT_AUTHORIZED; 16L3 remains BLOCKED / NOT_RUN; 16U2 remains NOT_RUN. No private-profile access, migration, physical qualification, merge or release activation is authorized.
