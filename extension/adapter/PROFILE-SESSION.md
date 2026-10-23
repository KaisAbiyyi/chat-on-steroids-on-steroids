# 8B — Cloud profile isolation, sign-in and session handoff contract

Dependency: accepted 8A PR #59 on main `eb91712d3c92c74f8714ef5f573939ad4fb19575`. Claim: [issue #60](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/60). Exclusive lock: `browser-profiles`.

The JavaScript `profile-session.mjs` controller is a **privileged host-side integration contract**, not a renderer API, account login implementation, browser cookie importer, or proof of real WebView2 segregation. Only an authenticated native 7B/8A host may construct it, pass opaque native proofs, and own its methods. Never export controller methods directly to MAIN/ISOLATED page scripts, IPC, content scripts, or untrusted `runtime.onMessage` senders.

- `authorizeNativeTarget(proof)`: repeatable native ownership attestation with exactly the 8A eleven context fields; host generation, navigation, worker generation and session identity are verified before critical operations.
- `resolveNativePartition(proof)`: host-certified unique storage partition handle and generation for each profile; **opaque IDs are not paths**. A real native host must actually create isolated, exclusive WebView2 user data partitions and validate true OS-level ownership; the synthetic Cloud controller only rejects reused or rebound partition handles in its 64-profile bounded state.
- `beginNativeSignIn({nativeProof,consentProof,providerId,origin,expectedContext,expectedPartition})`: native host verifies fresh user consent and starts navigation **inside the selected isolated profile** to an explicit allowlisted HTTPS provider origin. The controller reports `navigation-started`, **never** "authenticated" or account access. No automatic login, cross-provider actions or provider cookies.
- `commitNativeHandoff({sourceProof,targetProof,consentProof,expectedSource,expectedTarget,sourcePartition,targetPartition})`: native host must **atomically** revalidate two distinct live profile partitions, source/target session epochs and scopes; consume user consent once; transfer *application session ownership*; assign a new empty source session; increase epochs beyond previous values; and commit or roll back. It must never transfer browser cookies, passwords, OAuth credentials, localStorage, files or WebView2 storage. An uncertain reply must never cause an automatic retry. Each privilege boundary is rechecked before and after the native operation.
- Only same-provider-origin, distinct-profile, distinct-session handoffs are allowed; profile writes are serialized; unexpected fields, generation rollback, stale document or partition rebinding fail closed.

## Qualification

CI synthetic Windows/Ubuntu Node tests cover consent denial, origin policy, authority spoofing, partition reuse, stale/unknown profiles, concurrent operations, replay rejection, no credential accesses and postcommit uncertainty. Existing 8A and 7C regressions run alongside them. Rust matrix and Task DAG remain required. The command-line profile import is a **different** legacy-data operation: not used for browser authentication handoff.

**NOT RUN by 8B cloud:** native WebView2 partition creation/integrity, installed host multi-profile restart, real provider OAuth/session, user login, consent UI, cookies, physical Windows, disk migration and native lifecycle. Those belong to 8C/8L1 and 8U1, subject to user consent. No local file access in this task.

## Acceptance

Implementor self-reviews exact PR SHA, tests, isolation/TOCTOU and privacy; all required hosted GitHub checks must pass before merged acceptance. Keep 8B **Review** while any required check is pending. Merged 8B unlocks 8C (not native or owner gates).
