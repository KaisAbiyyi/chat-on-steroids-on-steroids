# CL-09B — MCP connection, health and tunnel lifecycle

## Ownership and scope

- UNIT: CL-09B; OWNER_SESSION: `chatgpt-20261021-cl09b-01`; CLAIM_GENERATION: 1
- BASE_SHA: `ff5e7fbeb4592ac05238cf8eaa55aac993b31c9e`
- BRANCH: `work/cl-09b-mcp-transport-20261021`; PR: [#30](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/30) (draft pending CI/review)
- Exclusive logical resource: `mcp-transport`; no local D: checkout, real user data, or pinned upstream modification.

## Implemented / checkpoint

- `crates/cos-mcp/src/tunnel.rs`: preserve explicit HTTPS proxy precedence; convert supported ALL_PROXY schemes only when HTTPS_PROXY is absent; append loopback hosts to NO_PROXY without losing existing exclusions. This avoids routing local bearer-path MCP URLs through configured proxy chains.
- Exponential bounded health probe interval (500 ms to 30 s) without spawning duplicate sidecars; reconnect classification after a previously successful handshake, distinct from first connection / unavailable sidecar.
- Health never reports remote control-plane connectivity if local /readyz is down; retryCount appears in bounded tunnel status, reset at start/stop and fenced by sidecar generation.
- `crates/cos-mcp/tests/tunnel.rs`: regression cases for proxy precedence, CRLF/unacceptable proxy fallback, loopback bypass, bounded retry and teardown status. Existing fake-sidecar lifecycle and oracle tests retained.

## Verification status (update with exact final head)

- GitHub Task DAG validation on earlier implementation head: PASS.
- First Rust Windows/Ubuntu run: FAIL at formatting; formatting corrected.
- Second run: Ubuntu Clippy nested-if finding; corrected on isolated branch.
- Latest exact-head Windows and Ubuntu CI: PENDING at this checkpoint. No PASS inferred from an older run; check the PR Actions tab before marking REVIEW or DONE.
- Git diff, security review and merge: PENDING.

## Limitations and next gate

- No real Cloudflare/OpenAI control-plane handshake or network proxy/PAC discovery is claimed. Sidecar reconnection is delegated to its native client; this unit bounds probes and status, not an independent remote retry transport.
- Native Windows CL-L5, credentialed tunnel and external sidecar acceptance: NOT RUN. Independent code/security review and exact-head CI are required before acceptance.

NEXT: obtain exact-head cargo fmt, strict Clippy, workspace tests, build, DAG validation and independent review; merge only after acceptance. CL-L5 remains BLOCKED until CL-09B AND CL-09C are accepted; then run explicit native Windows proof on D:.
