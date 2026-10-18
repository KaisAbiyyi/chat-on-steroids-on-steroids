# GitHub bootstrap — first public CoS-Rust source publication

Captured 2026-10-18 (Asia/Jakarta). Status: **PUBLICATION PASS; INITIAL CLOUD CI IN PROGRESS AT CAPTURE**. This receipt describes the initial public push and its bounded audit. For the live CI result see [GitHub Actions](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions).

## Published identity

- Public repository: [KaisAbiyyi/chat-on-steroids-on-steroids](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids)
- Default branch: `main` (GitHub default branch and `git ls-remote --symref origin HEAD` verified)
- First published tip: `8637c12ccda8610e5a06534f30af4688034cc9b6` — `docs: use public upstream links and label local artifacts`
- Previous local `main`: `68f7954c45542a8d2f71e201fbd662fb1bab2400`, a verified ancestor of the new main. The advanced local feature branch's full **24 subsequent commits** were retained in canonical `main` without rewriting or force push.
- Initial push command: `git push --set-upstream origin main`, returned a newly created remote branch. API and local remote readback matched the published SHA.
- First CI: [Rust Windows and Linux run 39136263403](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/actions/runs/39136263403). At the captured check, both runners passed checkout/toolchain/format and were executing strict Clippy. Tests and release builds were pending. This is **not yet a green CI claim**.

## Bounded pre-publication screening

- Scanned all 24 local commits reachable through the published branch beyond the previous `main` for high-confidence private-key/API-token markers and long credential assignments: **zero matching files** for those patterns. Also inspected tracked and historical path names for profile/database/secret/binary extensions: none matching intended private data. This is pattern-based screening, not an assertion that arbitrary text is impossible to misclassify.
- Git object integrity (`git fsck --full --no-reflogs`) passed with only unreachable/dangling object reports; unreachable objects are not reachable from the published `main` refs.
- Existing ignore rules exclude local `upstream/`, `target/`, `data/`, `.test-tmp/`, `graphify-out/` and large local release artifacts. README links to ignored local folders were made explicit before publication.
- The committed MIT license and pinned original [source identity](../../audit/2026-10-16/source-identity.json) remain intact. Original upstream source files and the ignored upstream checkout were not published as a nested source tree.
- **Public Git metadata exposes existing author/committer email addresses** from the previously committed history. Consider GitHub noreply author identity for new commits; do not rewrite published historical commits without a separately reviewed decision.

## Scope and remaining gates

The initial publication carries the previously qualified **headless Rust service** plus tests, fixtures and documentation. Earlier Windows/Linux PASS receipts remain historical at their specified SHAs. The full browser, desktop, installer, cross-platform parity and later migration phases have **not** passed acceptance. Cloud CI must complete, and if red, create a separate repair unit with an exact SHA.

The eight pre-existing local recorder/caption/redaction paths remain **dirty/untracked only on the Windows development checkout** and were excluded from every publication commit. They must be reconstructed from pinned public source test cases, or transferred only after a separate safe review. The first cloud unit is `CL-01` (Critical recorder privacy), followed by `CL-02` evidence integration. The local `CL-L1` gate is scheduled after `CL-03`; `YOU REQUIRED: NO` for the backend closure itself.
