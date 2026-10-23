# 8A — Cloud Chrome API / extension worker world

## Scope, baseline and boundaries

- Repository `KaisAbiyyi/chat-on-steroids-on-steroids`, cloud base `f5b27921d54c005cd027f3afdb8af9e11d736557` (accepted 7E).
- This unit introduces a deliberately bounded Chrome scripting/world adapter that requires 7B/7L1 native authority and reuses 7C journal. The new adapter is not enabled for user accounts and never installs a generic `runtime.onMessage` or page→host trust route.
- Separate real gates 8L1 (Windows), 8U1 (owner sign-in) and 7L2 native/browser qualification are NOT_RUN in this cloud unit. No local file or user session access.

## Implementation

1. Validate exact native context and explicit HTTPS origin allowlist; fail closed on unauthorized/stale/extra/invalid data.
2. Reauthorize before/after every asynchronous Chrome interaction; corroborate `tabs.get`, `webNavigation.getFrame` identity, and document ID; refuse cross-origin and stale navigation/host generations.
3. Call `chrome.scripting.executeScript` only with hardcoded inert files and `documentIds` target; no arbitrary code, provider actions, debugger attach, cookies, profile transfer or credentials.
4. Coalesce callback/Promise Chrome API semantics with `runtime.lastError` fail closed. Require matching post-injection `InjectionResult` and report no success on uncertain races.
5. Preserve the 7C authenticated native event ledger; worker suspension remains persist-before-reply, and old claims cannot be automatically resent.
6. Add synthetic Node race/security regression tests and Windows/Ubuntu GitHub workflow. Update task DAG to record verified 7E acceptance and new 8A readiness.

## Quality evidence

- PR and final SHA: pending at initial cloud checkpoint.
- **Self-review by implementor:** pending exact PR head/diff and CI; never infer CI PASS from files merely being committed.
- **Cloud:** 8A Node tests, 7C/7E regression, Windows/Ubuntu Rust formatting/Clippy/workspace and task DAG required; NOT_RUN until GitHub Actions reports it.
- **Known limits:** WebView2 support for `documentIds` and `InjectionResult.documentId` is an explicit fail-closed qualification requirement; a post-injection check cannot undo JS already run after a navigation race, so only inert audited world files are shipped. Native proof method is injected by the native host, not implemented from page-supplied fields.
- **NEXT:** inspect exact PR diff; repair credible P0/P1/P2; wait for exact-head checks; merge only if accepted, then mark Notion Done and unlock 8B.
