# ADR 2026-10-18B — CoS-Rust cloud-first development, Windows local qualification

**Status:** Accepted, supersedes [ADR 2026-10-18 local-first](2026-10-18-local-first-engineering.md). The earlier ADR remains a historical decision and does not govern future implementation.

## User direction

CoS-Rust adopts the Mind Place cloud-first workflow. Agents implement and review cloud units in GitHub, with source code, CI, versioned contracts and final merged commits there. Many CoS-Rust features require Windows-native, browser, provider, terminal, filesystem, SSH, clipboard, input or packaged-desktop proof. Therefore local agent and user testing must be explicitly scheduled in phase/unit handoffs. Cloud and local responsibilities stay separate.

The user supplied `https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids.git` as the intended destination and asked for a careful later push. On audit it was a **public empty repository**. The local `origin` remote was configured without pushing. Local uncommitted recorder/caption/redaction WIP remains protected; it is not automatically available to cloud agents.

## Decisions

1. **GitHub becomes canonical after reviewed initial publication.** Until then local committed history and protected WIP are the only available artifacts. First use a publication audit (`BOOT-L1`) and user review (`BOOT-U1`) of the public history; do not push uninspected Git objects or sensitive data. Never force-rewrite remote history.
2. After bootstrap, each coherent cloud implementation unit produces a branch/PR, appropriate Windows/Linux CI, code review, merged SHA and documented handoff. Cloud development never requires the user to keep the local Windows machine active.
3. Native qualification happens at named `L` gates: a local agent pulls the approved exact SHA on drive D, tests a clean target and returns a verifiable report. Test failures go back as cloud fix units. Early architecture gates can block dependent cloud units before an entire phase is written.
4. `U` gates require the user **personally** only for authenticated provider access, permissions/consent, irreversible privacy/migration and subjective usability. Each cloud handoff must say clearly `YOU REQUIRED: NO/YES`, what to do, and when. A local agent can do local automated tests without the user debugging them.
5. Keep the entire Rust migration parity objective (16 phases), source fixtures, authority/security guarantees, performance/size evidence, historical receipts and existing versioned documentation. Plan work as [72 base cloud units, 28 local sessions, 15 user sessions](../implementation-plan/2026-10-18-cloud-first-units.md), plus one publication local/user gate. Counts are planning boundaries, not promises or hidden mandatory manual work.
6. Any local build, native artifact, temp file, cache or benchmark stays on **D:**. Cloud CI artifacts belong in cloud runners. Docker/WSL qualification is blocked until its physical Windows-host data is verified on D. No blanket cleanup of C.

## Consequences and handoff

The old local-first instructions in repository docs must be replaced with the [cloud-first workflow](../DEVELOPMENT-WORKFLOW.md), [BIG plan](../BIG-IMPLEMENTATION-PLAN.md), and [current phase handoff](../CURRENT-PHASE.md). The main README and contributing guide direct agents to these files. This ADR records the direction as repository memory; only after publication do cloud agents have the same committed instructions.

The pending critical recorder privacy defect is first in cloud closure (`CL-01`), followed by actual process evidence (`CL-02`); neither is claimed complete. The local uncommitted patch must be sanitized and reviewed before any transfer, or safely reconstructed from source fixtures. This governance checkpoint does not constitute cloud CI or product acceptance.
