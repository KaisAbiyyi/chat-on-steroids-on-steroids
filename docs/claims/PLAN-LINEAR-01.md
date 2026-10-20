# PLAN-LINEAR-01 — phase sequencing and machine-checkable task readiness

- TASK_ID: CoSoS/chat-on-steroids-on-steroids/PLAN-LINEAR-01
- OWNER_SESSION: ChatGPT GPT-6; GitHub cloud-connected; 2026-10-20
- BASE_SHA: 41d9f228191ff3fe893655a61bf9173ddeba727e
- BRANCH: docs/phase-linear-dag-20261020
- CLAIM_VERSION: 1 (coordination, not atomic lock)
- RESOURCE_LOCKS: roadmap-DAG, task-readiness, agent-execution-policy
- STATUS: CLAIMED / RUNNING
- SCOPE: rename the obsolete early-phase-8 feasibility spike as Phase 7 preflight (7P-A/7P-B/7P-L1), normalize remaining Phase 8 task numbers, preserve dependencies, add declarative task graph, deterministic readiness/parallel CLI and CI tests, update canonical plans/orchestration.
- DEPENDENCIES: CL-L1 PASS; CL-04A/B/C merged; main CI qualified.
- SAFETY: docs/tooling only, no runtime Rust changes or local MCP, no mutable legacy upstream fixtures, no protected D: workspace activity
- EXIT: all graph edges resolvable and acyclic; no Phase 8 dependency before Phase 7; accurate first ready wave CL-05A/06A/07A/08A/09A; candidate readiness never bypasses live PR/claim checks; CI PASS and reviewed PR merged
