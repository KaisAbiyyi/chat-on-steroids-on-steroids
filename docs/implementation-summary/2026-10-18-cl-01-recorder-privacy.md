# CL-01 — Recorder privacy boundary (2026-10-18)

**Scope:** recorder arguments and tool results entering persistent session history.
**PR:** [#3](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/3).
**Baseline:** published GitHub `main` at `b3b3cc5`.
**Status:** implementation in review; accepted only after required Windows/Linux CI, independent review and merge.

## Proven root cause

Previously `Recorder::record_call_details` passed raw `args` and `result` to `SessionStore::store_text`, which writes inline JSONL fields or full overflow assets. Redacting views, JSON logs or summaries after that boundary cannot remove the persisted bytes.

## Implementation

- `cos-store/src/redaction.rs`: recursively redact recognized secret/token/password/authorization/cookie/credential fields; preserve environment variable names but drop values; redact clipboard writes and read results, base64/binary/image payloads and download handles.
- Sanitize textual credentials in both arguments and results (including command assignment formats, JSON credential fields, bearer headers, JWTs, PEM private-key blocks and data-URI base64).
- Erase sensitive argument values echoed into results. Fail closed for clipboard reads and for secret-input sets too large to safely scrub.
- `Recorder::record_call_details`: sanitize before **either** `store_text` call; both returned call and appended event use the sanitized values. Overflow asset digests derive from sanitized bytes only.
- Synthetic tests cover normal non-secret command evidence, nested secrets, env, clipboard, binary/image inputs, both argument/result overflow assets, real journal files, complete cold restart and rehydrated reads. No private device profile was used.

## Security and compatibility limitations

- Arbitrary secrets not labelled as sensitive, matching a recognizable textual credential pattern, or present among known sensitive inputs cannot be discovered with certainty from arbitrary tool output. A whole-result discard policy would change source logging semantics. This limitation requires explicit continued evaluation in the future source-differential parity/security review.
- Unpublished local caption/recording WIP is **not** in this PR. No caption module has been enabled. When CL-02/CL-03 adds generated captions they must be derived exclusively from sanitized fields and genuine structured outcomes.
- The existing `store_text` API remains unchanged for non-recorder callers. Historic data written before this fix is not automatically redacted or migrated; old private journals/assets require separate authorized cleanup/migration.
- Full native Windows recorder qualification remains `CL-L1` **after CL-03**. This cloud gate does not accept phase 1–6 as a whole.

## Cloud acceptance checks

Require `cargo fmt --all --check`, strict `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` and `cargo build --release --locked` on Windows and Ubuntu; review PR #3's exact head SHA and run result. Earlier failed formatting jobs remain historical FAIL evidence and must not be reported PASS.

**LOCAL REQUIRED:** none for CL-01 (CL-L1 follows CL-03).  
**YOU REQUIRED:** NO.  
**NEXT:** CL-02 after PR #3 is reviewed, CI-green and merged.
