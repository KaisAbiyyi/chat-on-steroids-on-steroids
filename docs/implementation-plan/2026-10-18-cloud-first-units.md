> 2026-10-24 supersession: [P0 performance-first ADR](../decisions/2026-10-24-performance-first-workstation.md) replaces mandatory full upstream feature parity. [Early PERF-00–06 gates](2026-10-24-performance-first-workstream.md) are cross-cutting. Do not alter previously accepted security/data C/L/U receipts or falsely declare native product readiness.

> **2026-10-28 OWNER POLICY:** [Unsigned Windows distribution ADR](../decisions/2026-10-28-unsigned-windows-distribution.md) overrides mandatory production Authenticode signing for personal testing **and public unsigned distribution**. Existing TEST-signed CI alone does NOT qualify an unsigned product. `16PKG-01` issue #130/Draft PR #131 has now implemented and executed a bounded source-verified unsigned product path, real Windows negative/lifecycle tests and a source-bound artifact on prior reviewed source `74013072`. Independent review #5684556516 requested narrow handoff corrections now committed at `9939ea0`; exact-SHA CI, new independent re-review, acceptance and merge still precede `16L1`. Keep `16L1` Blocked until accepted; no TEST cert installation or private profile migration. Historic signed-only lines below are superseded.

# CoS-Rust: bounded cloud units and native acceptance schedule

Decision 2026-10-18 (Asia/Jakarta). **Planning matrix, not a progress report.** Apply alongside [BIG plan](../BIG-IMPLEMENTATION-PLAN.md), [current state](../CURRENT-PHASE.md) and the [original 16-phase dependency map](2026-10-16-rust-migration-phases-v1.0.md). Scope may be subdivided when source review proves a unit too large, but never merged merely to save prompts. Only phases 1–6 have existing partial Rust/service work.

## Notation and default workflow

- **C**: one cloud coding-agent work unit, with its own bounded scope, functional tests, documentation, PR and CI-green merged GitHub SHA. One work unit should not require a multi-hour uninterrupted agent session.
- **L**: a distinct local qualification session, initiated by the user when provided an exact GitHub SHA and short Windows checklist. The local agent tests device-dependent behavior on D: and returns evidence. It does not implement features unless separately instructed to change the workflow.
- **U**: a short direct user action for real account consent, ambiguous user-visible actions, privacy/migration approval or subjective experience. **Consent/permission U gates run BEFORE any local test touching real private profiles/accounts or OS-granted capture/input; subjective acceptance U gates follow the relevant automated/local checks.** U=0 means no scheduled direct user intervention for that phase.
- A phase may have multiple C, L, U gates. An *early* gate interrupts dependent cloud development until its result is known; other native checks can wait for the completed phase. **Owner-approved ordering (2026-10-22): after `CL-L6` and accepted `7P-A → 7P-B`, begin Cloud `7A → 7B → 7C`; physical Windows `7P-L1` is HOLD and must pass before native `7L1` with `7C`.** These are Phase 7 tasks, not early Phase 8. Full Phase 8 starts at `8A` after Phase 7 closure. Additional fix/retry units are not counted in baseline estimates.
- CI coverage needs Windows + Linux where applicable. An OS runner proves its isolated scope, not installed consumer browser/profile/network/UI fidelity. Windows local profile and all locally generated artifacts remain on D:.

## Phase workload matrix

| Scope | C (cloud) | L (local agent) | U (you) | Timing / why |
| --- | ---: | ---: | ---: | --- |
| Bootstrap `BOOT` | 0 | 1 | 1 | Local publishing audit + one explicit confirmation of public initial GitHub publication; no feature code |
| Phases 1–6 remaining closure `CL` | 18 | 6 | 0 | Native security, filesystem/SSH, runtime, session and interoperability |
| Phase 7 — preflight + browser bridge | 7 | 3 | 1 | Preflight browser feasibility before bridge work; actual extension/protocol and provider action confirmation |
| Phase 8 — built-in browser | 3 | 1 | 1 | Chrome adapter, embedded profiles and native lifecycle after Phase 7 |
| Phase 9 — outbox/uploads | 5 | 2 | 1 | Durable send, native uploads and uncertain receipts |
| Phase 10 — Compact & Resume | 5 | 2 | 1 | Crash/rebind with real provider sessions |
| Phase 11 — workers/worktrees | 5 | 1 | 1 | Real multi-agent desktop flow and human delegation UX |
| Phase 12 — Goal/Loop | 4 | 2 | 1 | Actual provider model and continuation behavior |
| Phase 13 — plugins/Skills/OAuth | 6 | 2 | 2 | Real OAuth/installed third-party runtimes and permission UX |
| Phase 14 — native desktop/media | 6 | 3 | 2 | Windows capture/input/audio/DPI/clipboard/permissions |
| Phase 15 — full desktop UI | 7 | 3 | 3 | Native rich UI, localization, accessibility, usability |
| Phase 16 — migration/release | 6 | 3 | 2 | Actual install/migrate/update/rollback/startup/RAM/size |
| **Development after bootstrap (CL + 7–16)** | **72** | **28** | **15** | Planned phase handoff boundaries, excludes retries |
| **Including bootstrap** | **72** | **29** | **16** | Bootstrap's initial publication is a separate one-time gate |

These numbers are a safe initial breakdown, **not** an elapsed-time estimate or a commitment to force 72 prompts. Cloud units may be split if a browser/native feasibility or source-oracle gap requires it. `L` is an *agent-run local session*, while `U` explicitly indicates whether the user needs to personally act. A phase with `U=0` still needs the user to initiate a named `L` handoff where scheduled; the local agent conducts the actual test.

## Bootstrap — GitHub source of truth (one L, one U)

`BOOT-L1`: on the existing Windows checkout, inspect the whole **committed history** for unintended personal profiles, credential data, improperly tracked binaries, generated artifacts, nested upstream source and license obligations. Verify `origin` exactly equals `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids.git` and that its remote has no pre-existing commits/branches. Record size, scope, branch ancestry and clean publication selection, preserving all eight existing WIP paths. Dry-run or remote inspection is preferred first. The documentation change preparing the workflow can be a separate local governance commit; it is not a cloud feature unit.

`BOOT-U1`: user reviews the publication receipt because the specified destination is **public** and publication makes committed source/history visible. Choose the **most recent approved clean local feature-branch commit** (including the verified phase 1–6 backend progress, original receipts and cloud-first governance) as the canonical bootstrap tip. The GitHub default development `main` **must descend from this approved feature history**, rather than publishing old local `main=68f7954` while hiding subsequent progress in a side branch. Use a reviewed fast-forward/merge or direct initial branch publication that preserves the approved commit chain. Verify ancestry (`git merge-base --is-ancestor <approved-sha> <published-main-sha>`), compare published refs and run GitHub CI before handing off `CL-01` based on the published main SHA. Never force-overwrite remote commits, rewrite qualified historical commits or push secret-bearing objects. WIP recorder/caption data stays in the local protected working tree and is communicated to the first cloud agent through sanitized contracts and source-oracle captures **only after** publication; uncommitted code is not automatically available in cloud. If cloud CI fails, repair in a separate cloud bootstrap unit with exact SHA.

**Handoff after bootstrap:** repository URL, exact default branch SHA, CI status, old→new branch mapping, WIP disposition and the exact first cloud unit (`CL-01`). Until published, GitHub remains empty and `CLOUD READY` is **BLOCKED**.

## Remaining phases 1–6 — cloud closure (`CL`, 18 C / 6 L / 0 U)

`CL-00` was the previous governance audit, **already a docs-only local checkpoint**; it is not counted among the following 18 cloud implementation units. Do not treat this as acceptance of phases 1–6 or of uncommitted recorder changes.

| C unit | Cloud outcome and source proof |
| --- | --- |
| `CL-01` | Critical recorder privacy: redact environment, clipboard, secret and binary payloads before storing inline/asset or derived captions; cold-restart negative proof |
| `CL-02` | Actual tool outcome evidence: nonzero/timeout/background process, find match counts and patch result paths from trusted structured execution |
| `CL-03` | Timeline/running captions, activity taxonomy and durable projection compatibility with pinned source corpus |
| `CL-04A` | Provider/model/correlation/rebind reconciliation, durable identity and manual/provider title precedence |
| `CL-04B` | History retention, large-session budgets, restart/fault/corrupt metadata and replay recovery |
| `CL-04C` | Lossless or explicitly rejected source UTF-16/JSON edges with no raw-data corruption and documented deviation decision |
| `CL-05A` | Child process owner and output custody, cancellation/timeout/unread completion races |
| `CL-05B` | PTY/shell/SSH child environment and lifecycle parity, native-platform errors and boundary tests |
| `CL-06A` | Git/project/file-panel parity: diff, preview, optimistic save, binary/symlink and watcher invalidation |
| `CL-06B` | SFTP SSH files, handle containment/trust choices, races/limits and cross-platform error projections |
| `CL-07A` | Search/patch traversal/exclusions, source ordering, limits and transaction error exactness |
| `CL-07B` | Ripgrep/JS regex, streamed search chunks, partial hits, and server search caps |
| `CL-08A` | Windows startup/config/secret-store lifecycle and secure migration within current backend scope |
| `CL-08B` | Logging, diagnostics, awake/shutdown and bounded reporting integration |
| `CL-09A` | MCP tools/schema/protocol error and permission conformance for each surface |
| `CL-09B` | Connection retries/proxy/health/tunnel/control publication and generation-safe teardown |
| `CL-09C` | Code-engine and nested tool execution, cancellation/revocation, resource admission and proof output |
| `CL-10` | Integrated phase 1–6 **Rust backend/service** parity ledger, security review, Windows/Linux CI and exact handoff SHA; explicitly retain original Phase 1 browser/UI architecture gates as open |

**Local sessions**, all after their necessary merged cloud units and CI:

- `CL-L1` after `CL-03`: real Windows recorder/process result to persistent store, restart and redaction of inline/assets; no private user profile.
- `CL-L2` after `CL-05B`: Windows pipes/ConPTY, shell and process custody, native OpenSSH in an isolated environment.
- `CL-L3` after `CL-07B`: NTFS, junction/reparse and watcher races, file-panel/search + real disposable SFTP transport.
- `CL-L4` after `CL-08B`: native config/startup/secret-store, cold-start shutdown and disk/RAM measurements.
- `CL-L5` after `CL-09C`: live loopback MCP and local control/client session, code-worker negative cases and disconnect/retry.
- `CL-L6` after `CL-10`: integrated Windows headless service checks, source parity, resource and release receipts; independent **backend closure** review. `L6` does not claim original Phase 1 architecture accepted or desktop ready.

`CL` has **U0**. User need only request the next `L` command/handoff and review a phase completion report; no real signed-in account or subjective UX session is mandatory here. A privacy or external-action approval that emerges unexpectedly becomes a named additional gate, not a hidden task.

## Phase 7 — feasibility preflight, browser bridge and direct tools (`7C / 3L / 1U`)

**Owner-approved exception:** `7P-L1` is deferred (HOLD, unaccepted) for Cloud implementation of `7A–7C`, but is mandatory before native `7L1`; real browser/security/UX acceptance is not waived. **Phase 7 preflight (first):** `7P-A` select/prototype native browser/extension and rich UI renderer feasibility; `7P-B` prove minimal host, Chrome extension/debugger isolated world, tabs/profiles, editor/PDF/math/terminal probes; `7P-L1` run the actual isolated Windows browser host viability test. **STOP native 7L1 and dependent 7D–7E if 7P-L1 fails** and re-evaluate the architecture. With accepted 7P-B, `7A` freezes bridge protocol 14 and document/session identity; `7B` implement native bridge authenticated handshake and generation fencing; `7C` implement extension companion events, claim/ACK/repair and MV3 suspension; `7D` direct browser tools, debugger/snapshot/frame and handle ownership; `7E` source-wire differential fuzz/error/reconnect tests and independent security review.

`7L1` after both `7C` and accepted `7P-L1` **native gate**: real Windows bridge/extension handshake, tab elections and MV3 restart. `7L2` after `7E`: actual browser tool lifecycle/duplicate input prevention. `7U1` after `7L2`: user signs into own provider and verifies a safe observed conversation without automatic Send. Further phases may not assume browser support before `7L1` passes.

## Phase 8 — embedded browser and sign-in (`3C / 1L / 1U`)

**Starts only after Phase 7 browser bridge qualification.** `8A` implements the Chrome API/extension worker/debugger compatibility adapter; `8B` implements sign-in and safe isolated profile/transfer paths; `8C` closes native browser lifecycle, permission and security behavior. There is **no Phase 8 work before Phase 7**.

`8L1` after `8C`: real Windows clean-start/restart/multi-tab/profile-preservation proof. `8U1` after `8L1`: owner approves actual provider sign-in/consent and session stability. The original Phase 1 architectural feasibility obligation is executed earlier as **Phase 7 preflight `7P-A/B/L1`**, and remains blocking for the bridge.

## Phase 9 — outbox, uploads, steering, finish (`5C / 2L / 1U`)

`9A` durable input/attachment ownership and immutable bytes; `9B` queue/reorder/cancel and input form validation; `9C` Send claim/ACK and ambiguous retry fencing; `9D` actual native upload + steering/turn rotation integration; `9E` crash/reconnect duplicate-action differential qualification.

`9L1` after `9C`: actual bridge/send acceptance and uncertain ACK behavior with isolated test account/fixture when available. `9L2` after `9E`: real file upload, resume and cancellation. `9U1`: user approves and observes a bounded test Send/attachment, not unattended posting.

## Phase 10 — Compact & Resume and recovery (`5C / 2L / 1U`)

`10A` continuation WAL/identity and checkpoints; `10B` destination/source binding and lineage; `10C` resume-gate and uncertain send/idempotence logic; `10D` retry/repair/timeout and worker integration; `10E` simulated crash points and source semantics regression matrix.

`10L1` after `10C`: real Windows browser + resumed provider turn isolation. `10L2` after `10E`: restart, crash recovery, pending uploads and safe rebind. `10U1`: user observes a controlled compact/resume exchange in their actual provider account.

## Phase 11 — workers/inboxes/worktrees (`5C / 1L / 1U`)

`11A` durable family/run/worker identities and ownership; `11B` worker slots/admission; `11C` inbox addressing and sleeping/reuse; `11D` safe worktree creation/disposal and worker archive; `11E` concurrency/custody/permission regression suite.

`11L1` after `11E`: multiple real native agent windows/runs, cross-family isolation and safe worktrees. `11U1`: user verifies actual delegation, response delivery and Stop flow feel correct.

## Phase 12 — Goal, Loop and helper decisions (`4C / 2L / 1U`)

`12A` durable goal/obligation/reply model; `12B` provider/helper selection, limits and routing; `12C` restart/resume/Off→On decision fences; `12D` full nonzero/cancel/stale-response conformance.

`12L1` after `12B`: hosted/local helper connectivity and actual runtime. `12L2` after `12D`: multi-turn provider workflow, Stop/Loop/recovery. `12U1`: user judges bounded autonomous workflow expectations with explicit oversight.

## Phase 13 — plugins, Skills and OAuth (`6C / 2L / 2U`)

`13A` plugin manifest/capability catalog; `13B` sandboxed stdio/HTTP client adapters; `13C` OAuth authorization/refresh/revocation; `13D` Skills install/import/hash/permissions; `13E` optional package/runtimes and provenance/licensing; `13F` end-to-end security/compatibility tests.

`13U1` **after `13C` cloud security checks but BEFORE `13L1`**: user grants scoped OAuth consent for an approved disposable account; without consent `13L1` is BLOCKED and can only use simulated callbacks. `13L1`: real supported OAuth callback/token rotation for that approved account. `13L2` after `13F`: plugin/Skill install/update/revocation with isolated fixtures or previously approved access. `13U2` after `13L2`: reviews live plugin/Skill permissions and visible capability experience.

## Phase 14 — native desktop/media/dictation/pets (`6C / 3L / 2U`)

`14A` native window/foreground/capture adapter; `14B` UIA/accessibility references, stale-frame/DPI safety; `14C` keyboard/mouse/pointer/clipboard with real custody fences; `14D` image/media pipelines and export; `14E` dictation and target-draft isolation; `14F` pet overlay/lifecycle and native security/performance review.

`14U1` **after cloud `14B` and before any `14L1` OS consent/capture or input operation that needs user permission**: user explicitly grants only the necessary Windows capture/accessibility permissions and selects a safe target. `14L1`: Windows capture, DPI and UIA target correctness on approved targets. `14L2` after `14D`: real input/clipboard/capture/image latency/resource tests with separately scoped allowed targets. `14L3` after `14F`: dictation/pet/window lifecycle. `14U2` after native gates: subjective dictation/pet behavior and privacy review. Tests without consent use synthetic/restricted targets.

## Phase 15 — full desktop UI/localization (`7C / 3L / 3U`)

`15A` UI architecture, IPC contracts and navigation; `15B` conversation/turn/activity and large-history rendering; `15C` rich editor/terminal/code/PDF/math surfaces; `15D` projects/files/agents/skills/plugin settings and tool panels; `15E` 13-language localization, accessibility and focus/keyboard; `15F` visual/interaction regression baselines for fixed Windows DPI/themes; `15G` integration stabilization and independent parity review.

`15L1` after `15C`: packaged WebView/native editor, terminal/PDF/math fidelity. `15L2` after `15F`: screenshot and interaction comparison in 13 locales, DPI/theming/scale. `15L3` after `15G`: large real-history memory/performance desktop run. `15U1`: daily navigation/editor usability; `15U2`: localization/accessibility/keyboard experience; `15U3`: feature completeness and real-workflow acceptance.

## Phase 16 — data migration, packaging and release (`6C / 3L / 2U`)

`16A` migration schema/version/backup/rollback; `16B` updater, package integrity and historical signer contract (signing now optional); `16C` bundling/runtime/license/optional-dependency plan; `16D` startup/RAM/size benchmark and profiling gates; `16E` install/upgrade/downgrade/recovery automated tests; `16F` release provenance/security/compliance notes and complete acceptance report.

`16L1` after accepted `16E` and accepted/merged `16PKG-01`: actual Windows native **unsigned**, externally hash/provenance-verified install/uninstall/update/rollback/recover using disposable D:-isolated profiles; no publisher certificate, TEST root or private profile consent is required for this isolated gate. The unsigned implementation and prior-SHA source-bound artifact exist on Draft PR #131, but corrected exact-SHA CI, independent re-review, acceptance, merge and artifact verification are mandatory before 16L1 begins. `16U1` **after cloud `16E` synthetic migration tests and backup-plan review, BEFORE copying, reading or migrating any private user profile**: user approves exact source, copy, backup and reversible migration scope. `16L2` after `16U1` approval: migrate only the approved *copy*, test fault rollback and no leakage; otherwise local agent runs synthetic-only qualification and marks private-profile path BLOCKED. `16L3` after `16F`: actual final desktop/installer size, cold startup, RSS, battery/interaction benchmarks on isolated data. `16U2`: final guided real-product acceptance on their machine after required L gates.

### 2026-10-29 — 16FS-01 fresh-start alternative (proposed; requires GPT-6 technical review, owner acceptance and merge)

Owner's 16U1 decision is **fresh start**, **not** authorization to read, copy, inspect, export or migrate any private CoS profile. Private 16L2 remains **BLOCKED / NOT_RUN / NOT_AUTHORIZED**. Its old approval requirement is not silently waived.

Proposed contract: \`16L3 = 16F AND 16L1 AND (16L2 OR 16LF-01)\`. 16FS-01 is a cloud policy/task-graph amendment only. New 16LF-01 is a distinct **owner-local physical Windows** first-run gate requiring accepted 16FS-01 policy, accepted 16L1 artifact and 16U1 owner fresh-start decision, plus source-pinned evidence of disposable empty-profile initialization, isolation from all prior CoS/CoSoS profiles, synthetic fixtures only, failure/rollback, no leakage, and safe task-owned cleanup. 16L2 remains disabled until a dedicated trusted private-migration verifier exists, even if the owner later changes consent. For 16LF-01, evidence must include exact source/artifact hashes, a GPT-6 technical-review report bound to the exact source/diff, and a separate explicit owner-attestation record. The accepted review mode is GPT6_OWNER_ATTESTED: the owner identity and separate attestation must be verified, and the attestation must bind the exact GPT-6 report digest and source revision. This is not GitHub independent approval; model authorship is owner-attested, not cryptographically authenticated. Structural task-board checks cannot authenticate self-reported JSON; the production verifier and owner-controlled trust root remain mandatory before marking the local gate done. No hosted CI is a substitute for 16LF-01 or 16L3 native evidence.

16U2 stays downstream of real accepted 16L3 and a separate owner release decision. This branch changes no private profile, installer, or accepted 16L1 receipt. See [version-controlled contract](../implementation-summary/2026-10-29-16fs01-freshstart-contract.md).

macOS/other cross-target builds remain `HOLD`, with separate target-specific gates when unpaused; an unavailable platform is never counted PASS for the full multi-platform roadmap.

## Handoff rule

For a cloud unit, return `UNIT`, `GITHUB (branch/PR/merged SHA)`, `IMPLEMENTED`, `CLOUD CHECKS`, `PERFORMANCE`, `LOCAL REQUIRED (none or exact L-ID)`, `YOU REQUIRED (NO/YES and exact action, BEFORE or AFTER which gate)`, `LIMITATIONS`, `NEXT`. For an L session, return `SHA UNDER TEST`, `MACHINE/OS`, `PASS/FAIL/BLOCKED`, `RECEIPTS`, `MANUAL REQUIRED (NO/YES)`, and `NEXT`. For a U session, provide a brief executable checklist; never ask the user to debug an unqualified implementation. **Never touch live private data/accounts or OS input/capture before scoped permission**, even if the ordinary subjective-U workflow happens after native qualification. A failed L gate generates a cloud fix unit, then repeats the L gate against the new SHA.


## 16FV-01 — Production trusted evidence verifier (2026-10-30)

### Execution and ordering

16FS-01 / PR #138 is merged and owner-attested under GPT6_OWNER_ATTESTED. 16FV-01 is the next Cloud implementation task, claimed on the existing branch work/16fv-01-trusted-evidence-verifier-20261030 under resource lock freshstart-evidence-verifier. A future Done status is accepted only after the production verifier revalidates the merged 16FS-01 contract and owner acceptance, then fetches merged PR #139, verifies its main ancestry and hashes the verifier source file at the exact PR head; a registry Done flag alone is not acceptance. 16LF-01 remains Blocked / NOT_RUN until 16FV-01 is accepted and the owner-controlled production trust root is configured.

Canonical dependencies:

- 16FS-01 depends on 16U1, 16L1 and 16F.
- 16FV-01 depends on 16FS-01, 16U1 and 16L1.
- 16LF-01 depends on 16FS-01, 16FV-01, 16U1 and 16L1.
- 16L2 remains Blocked / NOT_RUN / NOT_AUTHORIZED.
- 16L3 remains 16F AND 16L1 AND (16L2 OR 16LF-01).

### Evidence record contract

A genuine 16LF-01 receipt uses schema version 3 and is a claim until the production verifier fetches its actual GitHub records and evidence bytes. The task-registry envelope links three separate immutable comments on issue #140:

1. Receipt record with canonical receipt JSON and SHA-256.
2. GPT-6 technical-review report with schema cosos.release.cloud-provenance.v2, verdict PASS, exact PR source SHA and exact GitHub diff SHA-256.
3. Separate owner-attestation record accepting that exact receipt/report/source/diff/artifact tuple.

The accepted mode is GPT6_OWNER_ATTESTED. The two review records may be authored by the same owner. No GitHub independent approval is claimed or required; owner attestation is not cryptographic proof of model authorship. Legacy independent_review fields are rejected.

The receipt and evidence bundle bind both merged implementation contracts: the exact 16FS-01 source SHA and merge commit, and the exact 16FV-01 PR #139 head SHA, merge commit and verifier-file SHA-256. The verifier also re-fetches the actual PR #138 diff, recomputes its SHA-256, and recomputes the immutable predecessor GPT-6 report comment's body hash excluding its final self-digest line. Artifact download and Sigstore verification require GitHub CLI (gh) and a read-only token. The verifier fetches actual merged PR #139 metadata, checks its merge commit is an ancestor of main, retrieves tools/fresh_evidence_verifier.py at the exact PR head, and recomputes its SHA-256.

The evidence bundle is a GitHub user attachment containing qualification.json, evidence-manifest.json and structured JSON raw per-check evidence. Its archive SHA-256, every member digest, safe relative paths, all eight fresh-start PASS checks, and exact source/artifact/verifier/session/workspace/timestamp/command bindings are checked.

Each check evidence file is a JSON record using schema cosos.16lf01.check-evidence.v1. It repeats the repository, source SHA, exact 16FS/16FV pins, artifact, session and workspace bindings and carries a timestamp within the recorded session. The verifier validates check-specific observations rather than trusting a PASS boolean alone:

- fresh_init: new profile created, profile was empty, unique profile ID.
- isolated_storage: storage root is beneath the disposable workspace, canonical storage path is beneath that root, and it is outside existing profile roots.
- synthetic_only: artifact is unsigned, native16L1Accepted is false, and inputs are synthetic-only.
- existing_profiles_untouched: before/after snapshot SHA-256 values match.
- no_leakage: leakage_detected is false, cross-profile access count is zero, external data reads are zero.
- rollback: rollback completed, test profile removed, rollback target recorded.
- failure_isolation: injected failure occurred, remained contained, and existing profiles remained unchanged.
- owned_cleanup: workspace and temporary files were removed by the owner.

All eight paths must be distinct safe checks/*.json entries, each file must be covered by the bundle manifest, and its observations must pass the exact check-specific contract. The qualification.json PASS map must equal the statuses derived from these validated records.
 The actual historical 16L1 artifact ID 12868909286 is downloaded and rehashed; its pinned package payloads and five Sigstore attestations are checked against the exact original PR135 event, workflow and run attempt.

### Owner trust-root configuration

Production authority is not configured by the Cloud implementation. The only accepted production environment input is COSOS_FRESH_EVIDENCE_TRUST_ROOT_JSON, whose exact pinned value is exposed as PRODUCTION_TRUST_ROOT in tools/fresh_evidence_verifier.py. The owner must create the protected GitHub Environment fresh-evidence-production, add the exact JSON as an environment secret, and preserve read-only workflow permissions. Then use a separate owner-controlled main-only protected workflow/job to run the production task-board CLI and confirm CONFIGURED; this PR's CI workflow intentionally does not reference the Environment or secret, so it cannot auto-create an unprotected Environment. The production CLI returns TRUST_ROOT_UNCONFIGURED if absent and TRUST_ROOT_INVALID if changed. A test-only authority value cannot be loaded by the production parser.

The root pins repository ID 1424422098, owner KaisAbiyyi / user ID 93252311, the merged 16FS-01 PR/source/merge/report/attestation, and the actual 16L1 artifact/run/archive/file/provenance identities. No root secret, environment protection, owner-local access or physical qualification is created by this task.

### CI and handoff

The dedicated 16FV-01 workflow runs the focused verifier tests and task-board/DAG/release-gate tests on windows-latest and ubuntu-latest, checking out the exact PR head or pushed source SHA. Each platform uploads machine-readable evidence with tested SHA, OS, run ID/attempt, test outcomes and verifier-file SHA-256. A separate Windows job revalidates the immutable historical 16L1 artifact; it does not rerun 16L1 qualification.

16FV-01 is not promoted to Done until the final exact-HEAD Windows and Ubuntu runs pass, the Draft PR and changed-file set are verified, and the GPT-6 technical review plus separate owner acceptance bind the exact final source SHA and diff. The owner trust root remains a separate handoff. No 16LF physical PASS, 16L3 acceptance, release activation or merge is authorized here.
