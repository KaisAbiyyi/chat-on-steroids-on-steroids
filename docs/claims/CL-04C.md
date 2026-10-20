# CL-04C — Cloud task claim

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/CL-04C
- AGENT: ChatGPT GPT-6, GitHub cloud implementer, 2026-10-20
- CLAIM_VERSION: 1 (human-coordinated; not atomic mutex)
- BASE_SHA: a4728fb282aa4dc616e1c6652a13fdc10d2af862 (CL-04A/B merged + handoff)
- BRANCH: work/cl-04c-lossless-json-20261020
- LOCK: lossless-json-ingress / UTF-16 representations; store replay and metadata ingestion only
- DEPENDS_ON: CL-03 (merged); CL-04A and CL-04B merged; CI of handoff docs separately pending
- STATUS: CLAIMED / RUNNING
- ACCEPTANCE: reject lone-surrogate JSON input explicitly without altering raw persisted bytes or silently dropping valid history, preserve exact source-compatible scalar/pair UTF-16 cases, add negative tests, Windows and Ubuntu CI
- DECISION: explicit refusal for any lone-surrogate JSON string that Rust serde_json cannot represent losslessly; no substitution, no surrogate escaping invented
- LOCAL_REQUIRED: NONE; cloud code only; user local WIP intact
- NEXT: implement, test, PR, validate on exact SHA; document acceptable deviation; release lock after merged acceptance
