# 16FS-01 — Fresh-start release contract (cloud candidate)

- [Claim and source tracking #137](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/137)
- [Notion 16FS-01](https://app.notion.com/p/408c4a6aaf73810e8404e9a06a1c0855)
- Owner policy (16U1, 2026-10-29): initialize CoSoS from a **new empty profile**; **no authorization** to inspect, read, copy, export, back up or migrate any original/private CoS profile.
- 16E / 16F / 16L1: accepted per live external Notion/host receipts. Historical registry statuses lag and are not silently bulk-rewritten.
- This PR is **not** physical Windows qualification, approval to execute private migration or permission to release. All statuses below are pending independent review.

## Old vs proposed phase 16 dependency

Before: \`16L3 <- (16F AND 16L1 AND 16L2)\`, which would force optional private-profile migration even after owner denial.

After independent review and merge ONLY: \`16L3 <- (16F AND 16L1 AND (16L2 OR 16LF-01))\`. The retained 16L2 branch is only eligible after **new, scoped, explicit owner migration approval**, plus privately authorized copy-only host receipt and independent reviewer acceptance. An owner decision recorded as "Done" is **not** affirmative consent. Current 16L2: \`BLOCKED / NOT_RUN / NOT_AUTHORIZED\`.

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

\`tools/task_board.py\` models an AND/OR dependency and verifies structural evidence; tests simulate positive/negative records without private data or native execution. It **does not cryptographically prove** physical execution, reviewer identity, Sigstore trust, provenance, Windows ACL conditions or receipt truth. A well-formed invented JSON URL/digest may pass structural preflight, but MUST never count as authoritative qualification: external proof, source ancestry, authentic GitHub review and owner-local run receipts need independent validation before switching a blocked task to done. No self-approval or merge authorized for this PR.

No owner-local or synthetic Windows runtime is executed in 16FS-01; hosted CI exercises only source-level DAG and contract tests. Actual 16LF-01/16L3 are both NOT_RUN. Prior 16F and 16L1 acceptance is preserved; release is not activated.
