# CL-04A — cloud task claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-04A
- AGENT: ChatGPT GPT-6, GitHub-connected cloud implementer
- INSTANCE/SESSION: user-requested CL-04A conversation (2026-10-20)
- REPO: KaisAbiyyi/chat-on-steroids-on-steroids
- BRANCH: work/cl-04a-session-authority-20261020
- RESOURCE_LOCKS: session-identity; cos-store session/provider title/model/correlation
- BASE_SHA: ae892add3126eb7bb65accc56dedd03f08a035b0
- DEPENDENCIES: CL-L1 PASS (merged repair a00edd4a67e6c8acc9fc45bee659094719d6673a); main CI PASS
- CLAIM_VERSION: 1 (human-coordinated; not an atomic mutex)
- CHECKPOINT_SHA: see topic branch; claim commit first
- LAST_ACTIVITY: 2026-10-20 Asia/Jakarta
- STATUS: CLAIMED / RUNNING
- ACCEPTANCE: protect request ownership, stale provider/model/title observations and rebind authority across durable restart; add negative regression cases; require Windows/Ubuntu CI
- RESOURCE POLICY: GitHub cloud only; pinned upstream fixture untouched; no local D: workspace use
- NEXT: implement bounded regressions and reviewable PR; record CI and known limits. Other agents must not edit overlapping ownership contracts until claim is released.
