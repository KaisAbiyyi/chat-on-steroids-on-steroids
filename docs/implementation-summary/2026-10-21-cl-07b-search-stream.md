# CL-07B — regex admission and bounded streamed search (2026-10-21)

## Implementation
- Validate and compile regex a single time before search metadata, directory traversal or content workers begin; invalid regex on empty folders/oversized content can no longer misleadingly succeed with no hits.
- Share the immutable Regex with the existing 12 concurrent workers, which consume bounded native and SFTP 64 KiB read chunks and retain one request-authority gate on the coordinator thread.
- Explicitly reject JavaScript backreference/lookaround constructs that Rust/ripgrep semantics cannot implement, avoiding false semantic equivalence.
- Keep the existing 2 MiB per-content-file limit, maxResults 1..500 and 10-second whole-tool search deadline, plus one-extra-hit semantics for explicit-file truncation.

## Synthetic regressions
- Invalid regex before empty-directory/large-file scan, fail-closed unsupported JS regex constructs, explicit file regex hit cap/truncation, overlong query rejection.
- Existing upstream pinned search oracle tests and 12-content-worker authorization tests are unchanged.
- Exact-head Windows/Ubuntu format/strict Clippy/full tests/release + task DAG: NOT RUN, pending Actions; independent review NOT RUN.

## Known limits
- This branch does not bundle or invoke an external ripgrep binary, and does not claim full native ripgrep results/output streaming or full JavaScript regex equivalence. Existing content readers stream into a bounded 2 MiB buffer before matching; client receives only the final bounded outcome, not incremental partial events. These gaps remain CL-07B acceptance review targets, not retroactive PASS claims.
- UNIT CL-07B; BRANCH work/cl-07b-search-stream-20261021; PR #28; PERFORMANCE not measured; LOCAL REQUIRED CL-L3 after CL-06B also accepted; YOU REQUIRED no; NEXT CI, parity review and remaining streamed-result scope, then merge/reconciliation if qualified.
