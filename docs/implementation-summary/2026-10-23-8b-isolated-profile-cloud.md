# 8B — Isolated browser profile and session transfer, Cloud (2026-10-23)

- Source: `main` 8A merge `eb91712d3c92c74f8714ef5f573939ad4fb19575`; task [#60](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/60); branch `work/8b-isolated-profiles-session-20261023`, `browser-profiles` lock.
- Implementation: native-host-only bounded profile partition attestation, explicit provider origin and consent-gated sign-in navigation; separate atomic native-owned logical session handoff with no cookies/credentials/profile data. Dedicated synthetic Windows/Linux GitHub tests. See `extension/adapter/PROFILE-SESSION.md`.
- **CI:** PENDING at initial checkpoint; do not treat repository content as PASS.
- **Self-review:** PENDING exact PR head inspection, security/privacy/TOCTOU and CI. No external review gate.
- **Local:** NOT_RUN. No local files touched, real provider data or Windows desktop access.
- **Owner:** real sign-in and user acceptance at 8U1, not 8B. 8C and 8L1 remain future gates.
- **Performance:** no measured claims; controller bounds 64 profiles and 16 providers.
- **Next:** verify CI, inspect diff, repair material issues, merge on evidence, reconcile Notion and DAG before enabling 8C.
