# Phase 4–6 implementation review v1.0

Date: 2026-10-17, Asia/Jakarta.
Status: **review performed; one bounded repair pass verified (116 Rust tests and 17 executable checks)**.
Whole-phase exact acceptance: **PARTIAL** for phases 4, 5 and 6.

## Review identity and scope

One fresh reviewer, `phase456_final_review`, used gpt-6-luna with maximum reasoning effort.
The reviewer inspected all 92 entries of the frozen
[pre-repair source manifest](../../audit/2026-10-17/phase4-6-source-manifest.json); their hashes matched.
This was independent source inspection, not an independent rerun of the reported tests. The
[review package](../../audit/2026-10-17/phase4-6-review-package.md) contains the exact scope and focus.
The root is not Git. The pinned upstream clone remains a separate, unchanged repository.

Initial verdict: **not ready**, with **0 Critical, 7 Important and 0 Minor findings**. The reviewer
recognized the separation of Rust owners, upstream oracles, real Windows process/file proof,
surface boundaries and honest partial coverage. Their objections concerned failure paths and
authority. No second review was dispatched. The dispositions below are the implementer's bounded
repair and verification, not a fresh independent approval.

## Important findings and repair

| ID | Pre-repair trigger and effect | Disposition and regression |
|---|---|---|
| I1 | Unix `tree.rs` retained a numeric process group ID after the leader was reaped. Later termination/Drop could signal a reused group. | Implemented one-shot custody. `waitid(WNOWAIT)` observes exit without reaping; group cleanup precedes `Child::try_wait` in process, human-terminal and Git owners. Lost child custody retires signal authority. Removed redundant retained-session child killers. Two platform-neutral custody regressions were RED, then GREEN. Linux `cos-tools --tests` cross-check passes, including `cos-exec`. A native POSIX descendant-pipe test is present but **NOT RUN** on this Windows host. Native Unix qualification stays open. |
| I2 | Tunnel stop advanced its generation before consuming child completion, retaining unread internal sessions until restart 65 hit the 64-session cap. | Stop fences the observer, terminates and drains the owned process/readers before dropping its health directory. A two-second drain failure refuses restart. The real local 70-cycle restart test reproduced failure at cycle 64 before repair and passes after repair. No public tunnel was used. |
| I3 | An in-place write could change a prefix and fail. Rollback accepted only complete old/new content, leaving the target corrupted despite a backup. | Track bytes actually written and successful truncation. Rollback restores the original through the held handle only when its current bytes match that exact write progress; retain containment, inode and concurrent-content fences. A transient four-byte write fault was RED and is now GREEN, with original bytes restored and no staging residue. Persistent storage failure/crash recovery is not qualified. |
| I4 | Search checked its grant only before a potentially ten-second traversal and could publish results after revocation. | Recheck cancellation, caller admission, search permission and root identity during traversal, per-file/line batches and before publication. A mid-traversal revocation regression was RED and is now GREEN; revoked scans return an error without collected results. |
| I5 | A pre-existing `profile/control-api` symlink/junction was canonicalized as a new owner, allowing token/endpoint publication outside the profile. | Resolve the child directory and files from the existing canonical profile; reject link/reparse components before acquiring the control lease or writing credentials. A real Windows junction regression was RED and is now GREEN; the external directory remains empty. This does not qualify every concurrent directory-replacement race. |
| I6 | A small patch could name many large files. Per-file 512 MiB limits did not bound aggregate snapshots and decoded workspace. | Shared local/SFTP preflight limits: 128 paths and a conservative 256 MiB accounting budget. Charge input, snapshots and line-vector workspace; bound reads before allocation, reserve update growth before rendering, and bound revision re-reads. Three 12 MiB files and a 129-file patch now refuse before mutation, locally and through the installed local OpenSSH subsystem. Local and remote file-count regressions were RED before repair. The budget is an admission estimate, not a measured RSS ceiling. |
| I7 | SFTP REALPATH followed by OPEN/FSTAT cannot prove that the opened handle still belongs to the approved path under adversarial server-side replacement. | **Mitigated by default refusal; stronger containment remains unqualified.** All kernel SSH filesystem/path operations, including command/terminal cwd checks, require explicit host-only `--trust-ssh-server-paths` opt-in for one runtime. Neither MCP arguments nor configuration persist this trust. A cached local protocol regression was RED and is now GREEN for default refusal, explicit grant and restart reset. Low-level `RemoteFiles` is documented as a trusted-server adapter. Opt-in does not prove adversarial handle containment. |

The I7 test initially used the wrong cache key and failed through SSH connection setup instead of
the intended gate; that result was discarded as a regression proof. After correcting the fixture key,
the expected functional RED occurred entirely against its cached local transport. No real server,
credential or personal profile was used; external SSH authentication remains NOT RUN.

SFTP v3 specifies pathname REALPATH separately from handle FSTAT and defines no canonical-handle
attestation in these requests. Default refusal is an inference from that interface, not a claim
that another REALPATH check prevents an ABA race.
[Primary protocol specification, sections 6.3, 6.8 and 6.11](https://www.openssh.org/txt/draft-ietf-secsh-filexfer-02.txt).
Unix cleanup uses the non-consuming status behavior described by
[POSIX status information](https://pubs.opengroup.org/onlinepubs/9799919799/functions/V2_chap02.html).

## Executor rulings on behavior the reviewer declined to judge

Each ruling preserves an explicit acceptance limit; no silence counts as approval.

1. **Later Desktop/browser/plugin/worker owners:** retain unadvertised tools until their real owners
   exist in later phases. Cost if wrong: unavailable workflows may be needed earlier;
   full-product acceptance remains open.
2. **Actual cloudflared/OpenAI/PAC/reconnect behavior:** retain argument/environment/health and local
   process evidence only. Cost if wrong: external auth, routing or recovery may fail despite local
   tests; external tunnel acceptance remains NOT RUN.
3. **External SSH authentication/host keys:** retain host OpenSSH policy and mark external proof
   NOT RUN. The installed subsystem test proves SFTP packets/files, not authentication. Cost if wrong:
   aliases, agent or known-host workflows may fail on a real host.
4. **Linux/macOS native execution and filesystem behavior:** accept the Linux compile check only as
   compile evidence; native PTY, shell, group and filesystem gates remain NOT RUN. Cost if wrong:
   OS-specific behavior may still differ or fail. I1 is not declared natively qualified.
5. **Complete QuickJS/Git/search parity:** retain the enumerated gaps in the coverage register.
   Cost if wrong: supported-looking edge cases can differ; exact phase acceptance stays PARTIAL.
6. **Original renderer/JSDOM timeout cause:** leave the cause unresolved. Preserve the initial
   268-pass/1-timeout/12-skip receipt and unchanged focused 3/3 retry at its original timeout.
   Cost if wrong: a flaky or environmental renderer defect remains undiscovered. Do not combine
   overlapping counts or label the original combined run green.
7. **Recording during session rebind:** preserve first-proven request attribution as historical
   audit evidence. Correlations do not move on reobservation; new admission refuses the stale
   session/conversation binding, while the recorder can append an already accepted call with its
   original conversation attribution. Existing phase-3 authority/epoch tests cover these invariants;
   no new in-flight rebind integration proof is claimed. Cost if wrong: a consumer may expect a
   different historical projection. Changing this requires an explicit audit-contract decision.
8. **Accepted nested effects after JavaScript timeout:** keep an already admitted native operation
   owned by its service and record completion. Timeout ends the JS worker's wait; it does not roll
   back an accepted file change or replay a command. Existing interpreter/HTTP/executable regressions
   cover this behavior. Cost if wrong: a caller expecting cancellation-as-rollback will still
   observe the accepted side effect.

No minor findings were proposed or deferred.

## Qualification and handoff

Final verification receipts and repaired source identity are linked from the
[implementation summary](../implementation-summary/2026-10-17-phase4-6-rust-services-v1.0.md).
The [coverage register](../research/2026-10-17-phase4-6-coverage-v1.0.md) remains the outstanding ledger.
The repair pass does not promote service evidence to complete phase or full-app approval. I7's
adversarial remote containment, native Unix behavior and documented original-source gaps remain
open. The [plan](../implementation-plan/2026-10-17-phase4-6-implementation-v1.0.md) records decisions
and compatibility costs.
