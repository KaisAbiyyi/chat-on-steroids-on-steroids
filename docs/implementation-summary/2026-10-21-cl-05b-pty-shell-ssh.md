# CL-05B — cloud implementation summary (2026-10-21)

## Unit and claim

- UNIT: CL-05B (CoSoS cloud implementation only)
- BRANCH: `work/cl-05b-pty-shell-ssh-20261021`
- CLAIM: `docs/claims/CL-05B.md` (active until accepted)
- DEPENDENCY: CL-05A accepted
- GITHUB PR: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/26
- NOTION: https://app.notion.com/p/3ffd8330eb468186801fef1b5136e786
- STATUS: IMPLEMENTED ON DRAFT BRANCH; cloud CI and independent review determine acceptance, not code presence.

## Implementation scope

- Serialized renderer terminal write and resize against close; rechecked live command/project authority before native PTY operation.
- Rejected invalid PTY dimensions before process creation.
- Retained terminal ID ownership through final exit event to avoid stale exit notifications to a reopened terminal.
- Removed inherited secret/Electron variables from terminal environments and killed Unix or suspended Windows PTY children if host-side setup failed.
- Added permission, invalid-dimension, and ID-reuse regression cases.

## Qualification

- Regression tests: checked in with the current implementation branch; follow exact final-head GitHub Actions job and PR check receipts.
- GitHub CI: `cargo fmt --all --check`, strict all-target Clippy, `cargo test --workspace --locked`, `cargo build --release --locked`, 14 DAG tests on Windows and Ubuntu; **recheck the final PR HEAD** before changing status to accepted.
- PERFORMANCE: NOT MEASURED; no speed/memory/size improvement claimed.
- LOCAL REQUIRED: CL-L2; native Windows gate separate from cloud implementation.
- YOU REQUIRED: No user action for cloud review; native Windows gate only after accepted merge and eligibility.

## Known limitations, safety boundary and next step

- CL-L2 Windows named native pipe, ConPTY, shell, and OpenSSH proof is still NOT RUN.
- Shell/SSH/host behavior needs dedicated Windows manual qualification; no full application parity claim.
- Independent critical-boundary reviewer has not approved these changes.

NEXT: Obtain green exact-head Windows/Ubuntu checks; independent review and follow-up fixes; only then merge and mark `docs/task-registry.json` and the Notion task Done/release the exclusive claim. Keep native gate open.
