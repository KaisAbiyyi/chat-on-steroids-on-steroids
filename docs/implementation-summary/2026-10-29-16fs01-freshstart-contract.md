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

1. Independently review and merge 16FS-01. Pin approved source commit SHA, immutable 16L1 verified artifact plus SHA-256 and original provenance/attestation, reviewer and session ID. Reject unreviewed/changed bytes.
2. Obtain separate owner authorization to use **only a new task-owned disposable Windows workspace**. Establish protected isolation boundaries against all existing CoS/CoSoS profiles before launching. Do **not** enumerate, open, inspect, back up, copy, modify or remove private profiles; no private-path presence checks using private contents. Use a fresh nonce-owned path; fail closed on ACL/ownership/reparse hazards.
3. Initialize a genuinely **empty** application-owned profile; verify first-run creation and no re-use of prior profile markers or environment defaults. Use synthetic fixture identities/content only; deny network account logins, private credentials and leakage to logs/temp/artifacts.
4. Induce bounded synthetic failures and rollback; verify preserved unrelated directories and unchanged prior data boundaries without reading their contents; verify recovery and no alias/overlap/cross-profile import.
5. Clean only task-created disposable files after a validated owned-path check, and preserve protected evidence / failure artifacts until reviewed. Reject broad or recursive cleanup of unknown paths and symlinks.
6. Produce redacted, source-bound receipt: exact 40-char commit SHA, verified artifact and receipt SHA-256, unique local session ID, GH issue-attached receipt link, native OS/build and checks (\`fresh_init\`, \`isolated_storage\`, \`synthetic_only\`, \`existing_profiles_untouched\`, \`no_leakage\`, \`rollback\`, \`failure_isolation\`, \`owned_cleanup\`), each PASS/FAIL/NOT_RUN. Require an **independent** review URL and matching reviewed source SHA before proposing \`16LF-01=done\`. Missing/forged/incomplete evidence is rejection.
7. 16L3 remains its own real native startup, size, RSS, battery/interaction release gate, separately accepted; 16U2 stays a separate owner decision. 16L1's previous 8/8 and 5/5 receipts are immutable historical evidence and are not rerun.

## Validation boundaries

\`tools/task_board.py\` separates \`fresh_receipt_structurally_valid()\` from \`fresh_receipt_status()\` and authoritative \`accepted()\`. Structural validation is only schema/shape preflight. A correctly formatted receipt, SHA, GitHub issue URL, fabricated review URL, self-authored \`PASS\`, or task-registry status cannot establish acceptance.

The offline resolver has **no production trust root or external verifier** for authentic receipt existence, owner-attested GPT-6 review identity/mode, actual review verdict and reviewed SHA, receipt-to-artifact digest consistency, artifact provenance/source binding, or the owner-local fresh-start checks. Accordingly, a missing receipt is reported as \`MISSING\`, malformed evidence as \`INVALID\`, and structurally valid evidence without a trusted verifier as \`PENDING_EXTERNAL_VERIFICATION\`. Failed external facts are rejected. None of these states can satisfy \`16LF-01\`, unlock \`16L3\`, or enter release waves. The optional verifier contract returns a typed \`FreshEvidenceVerification\` only after independently establishing every required fact; it is not loaded from task JSON. The production CLI deliberately injects no verifier and remains fail-closed.

No reviewer allowlist, trust root, signature, provenance result, external review, or successful local proof is invented. A separately trusted verifier integration is a remaining prerequisite before any genuine \`16LF-01\` evidence can be accepted. No self-approval or merge is authorized for this PR.

No owner-local or synthetic Windows runtime is executed in 16FS-01; hosted CI exercises only source-level DAG and contract tests. Actual 16LF-01/16L3 are both NOT_RUN. Prior 16F and 16L1 acceptance is preserved; release is not activated.


## Final corrections — 2026-10-30 (targeted candidate; GPT-6 technical review and owner acceptance pending)

### P1 — authoritative fresh-start receipt acceptance

- Renamed the receipt check to \`fresh_receipt_structurally_valid()\`; it is explicitly structural-only.
- Added \`fresh_receipt_status()\` with \`MISSING\`, \`INVALID\`, \`PENDING_EXTERNAL_VERIFICATION\`, \`REJECTED_EXTERNAL_VERIFICATION\`, \`VERIFICATION_ERROR\`, and \`ACCEPTED\` states.
- Added a typed \`FreshEvidenceVerification\` trust-boundary contract covering actual receipt, review and artifact existence; reviewer identity and independence; actual PASS verdict; reviewed source SHA; exact receipt/review/source/artifact digest binding; receipt-to-artifact digest consistency; artifact provenance and source binding; fresh-start checks; and evidence completeness.
- The offline CLI does not provide a verifier because no defensible repository trust root or implementation was found. Therefore structurally valid candidates remain pending external verification, while missing or malformed evidence is separately reported; none can satisfy \`accepted(16LF-01)\`, the 16L3 OR dependency, readiness, validation, or wave forecasting.
- Tests use a clearly test-only verifier double to exercise the positive integration path; it is not evidence of a real GitHub review, artifact, or Windows physical run.

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
- Candidate acceptance remains pending final exact-HEAD Windows/Ubuntu CI and GPT-6 technical review. Owner acceptance and merge remain separate.


### Private migration remains disabled

- 16L2 cannot become ready or enter wave forecasts in this phase, even if owner consent is later changed. A dedicated trusted private-migration verifier is not implemented.
- Self-reported `physical_windows`, `independent_review_verdict`, or other task-registry receipt fields cannot establish 16L2 acceptance.
- The 16L3 OR alternative is structurally preserved for future policy evolution, but only 16LF-01 is a viable path under the current fresh-start decision and verifier plan.
