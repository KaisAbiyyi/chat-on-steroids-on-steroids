# ADR — Windows unsigned installer and release distribution are supported

**Date:** 2026-10-28 (Asia/Jakarta)  
**Status:** Owner-approved product decision; implementation/physical qualification pending  
**Scope:** CoSoS Windows x64 installer, update, provenance, and Phase 16 local acceptance

## Decision

The owner explicitly **removes mandatory third-party/publisher Authenticode signing** as a prerequisite for both personal Windows development/qualification and transparent **public distribution of an unsigned installer**. A purchased certificate, approved external signing provider, temporary trusted TEST root, or private publisher key is not required to ship/test the default unsigned product. Publisher signing may be added as a future **optional** enhancement, but must not block Phase 16 progress or 16L1 qualification.

This changes a prior owner-authored Phase 16 release contract. It does **not** reclassify historical, already accepted TEST-signed synthetic CI results as proof of unsigned product readiness.

## Non-negotiable verification and security contracts

- Use the actual native Rust + C#/WebView2 renderer product payload; no headless-only probe or fake 16E fixture passed off as production.
- Publish reproducible exact-source SHA, stable SHA-256 installer/payload/manifest receipts and externally verifiable GitHub CI/release provenance. Tell users how to retrieve a **trusted** digest and verify it out-of-band. A hash/manifest supplied exclusively by the untrusted package cannot establish authentic publisher identity.
- Design a distinct, explicit unsigned release channel. The current non-TEST `CoSoSInstaller.exe` enforces Authenticode and **will refuse** an unsigned setup. Do not merely strip signature checks, enable `COS_16PKG_TEST` in production, accept caller-supplied arbitrary publisher proofs, downgrade or auto-trust unknown code. Document where signature-bound trust properties are unavailable and what independent trust roots replace them.
- Fail closed on unexpected files/directories, NTFS ACL/ownership violations, junctions/reparse points, mutable payload data, downgrade/replay, corrupted/foreign proofs, interrupted transactions and unexpected filesystem roots. Do not recursively delete unowned data. Updates must require independently verified source integrity; **no silent updates from unverified sources**.
- Local qualification should use disposable D:-isolated directories and test profiles, not production CoS/CoSoS user profiles. Require explicit opt-in/disclosure for unsigned execution and any Windows SmartScreen unknown-publisher prompts; never advise blanket system-protection disablement. **Do not import TEST certificates or modify machine trust stores.**
- Keep all real-account/profile migration consent under separate 16U1/16L2 gates. Keep native physical 16L1 and full Windows release 16L3 NOT_RUN until actually executed and recorded.

## Phase 16 execution order

1. `16PKG-01` (existing Draft PR #131 / issue #130; exclusive `release-native-installer` lock) is the cloud work unit implementing and qualifying the safe unsigned native product installer/release channel. Previous 2026-10-28 independent review PASS on exact HEAD `513ae3ef9efdfd553a81c21caff8ca067c3086c8` applies ONLY to the earlier TEST-signed crash/security correction; the **new unsigned acceptance remains pending**.
2. `16L1` requires accepted `16E` and accepted/merged `16PKG-01` plus an artifact with independent SHA/provenance checks and a safe Windows local test plan. Until then it stays **Blocked / 03 Up Next** (direct successor). Once ready, move to **Local Gate / 02 Now** and actually test physical install, update, rollback, recovery and uninstall. No real signing required.
3. `16L3` and `16U2` still govern full native/release and owner acceptance; `16U1` governs private-profile migration consent.
4. Update `docs/task-registry.json`, current Phase 16 unit documentation and Notion in the implementation PR. Do not falsify Done, claimed ownership or physical acceptance. Older signing-gate documentation is historical where it conflicts with this owner decision.

## Evidence

- Prior bounded cloud review: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/131#pullrequestreview-5682650064
- Owner policy applied to issue #130: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/130#issuecomment-6469786860
- Physical 16L1 issue #129: https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/129#issuecomment-6469787820
- Canonical Notion hub: https://app.notion.com/p/400c4a6aaf738177b4adfd851b5e47c7
