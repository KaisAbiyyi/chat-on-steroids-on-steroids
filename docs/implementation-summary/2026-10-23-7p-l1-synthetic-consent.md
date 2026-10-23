# 7P-L1 — isolated Windows synthetic OAuth consent probe (2026-10-23)

**Disposition:** scoped synthetic `sign_in_constraints=PASS`; the full 7P-L1
remains PARTIAL / NOT ACCEPTED. This is a physical Windows WebView2 probe,
not authenticated provider access or a production Rust browser implementation.

Code SHA: `f68d186de39194b56f34bb75d79f03526f5ead3e`, branch
`docs/7p-l1-native-20261022`, PR #42. The built WindowsHost reports its
matching informational version. WebView2 Evergreen `155.0.4283.62`, Windows
11 build 26300, two isolated local D: profiles, OS SSO disabled. The executable
SHA-256 is `93C82D11DD9E47BADC0EF8E42FA4C4F20133E71D973D9FF2516A5EC11396C0DB`.

The raw receipt is at `D:\projects\CoS-Rust\.test-tmp\7p-l1\mock-oauth-f68d186-exact\receipt.json`
(SHA-256 `3025ECAC0651D1554A6992B3F2F0DE53D572FFA2A0FEDF0BCCF8199CED4FB3B3`),
with a normalized tracked index at
`audit/2026-10-23/7p-l1-synthetic-auth-receipt.json`.

**Native checks: 18 PASS / 0 FAIL / 1 UNVERIFIED / 2 NOT RUN.** The five
new fixture gates PASS for loading the consent page on a separately mapped
virtual host, approval and single-use callback, rejection of forged origin/state
and cross-profile authority, a navigation of that callback in profile B without
transferring authorization, and refusal of consent with `access_denied`.
`sign_in_constraints` is also PASS for these synthetic checks. Fail-closed
contract validation and 15 pre-existing pure policy assertions PASS; WinForms
host compiled with zero warnings/errors and renderer Vite built successfully.

The fixture uses `login.synthetic.cos.test` and `synthetic.cos.test` as
WebView2 virtual hosts mapped to checked local renderer assets. The host
generates the expected state and owns its profile-specific pending grant.
Untrusted page input cannot decide which callback or profile to authorize,
and each valid callback can be consumed only once. No credentials or personal
browser profiles were accessed. The fixed mock code is a public fixture and
is **not** a token, real PKCE proof, exchange with an OAuth server or real
provider compatibility proof.

**Open acceptance:** the separate non-idle OAuth run leaves MV3 natural idle
`UNVERIFIED`; a previous exact-SHA native idle run recorded that behavior PASS.
`distribution_budgets` and the aggregate `native_windows_gate` remain NOT RUN.
Provider sign-in/consent and authentic OAuth constraints remain for 7U1/8U1,
and the separate plugin token-rotation/authorization work remains in Phase 13.
The owner-directed OS IME deferral remains HOLD. No user account action is needed
for this scoped synthetic receipt. Do not merge #42 or mark 7P-L1 Done on this
receipt alone.
