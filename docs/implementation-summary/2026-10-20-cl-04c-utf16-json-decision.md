# CL-04C — UTF-16 / JSON representation boundary

Date: 2026-10-20 (Asia/Jakarta). GitHub cloud implementation on `work/cl-04c-lossless-json-20261020`, PR #15. Base `a4728fb282aa4dc616e1c6652a13fdc10d2af862` with CL-04A and CL-04B merged.

## Source compatibility decision

JavaScript string values are sequences of UTF-16 code units. `JSON.parse` accepts JSON escapes for isolated high or low surrogates, and `JSON.stringify` preserves them as escapes. Rust `String` and `serde_json::Value::String` support Unicode scalar values, not isolated UTF-16 units.

**Decision:** For isolated JSON `\\uD800`-`\\uDFFF` code units, CL-04C deliberately reports `std::io::ErrorKind::Unsupported` and preserves the original file. It **does not** claim parity for the actual JavaScript string value and never replaces such code units with U+FFFD, changes a canonical record, falls back to an older copy, or marks the input as generic corruption. Implementing true lossless storage would require a separately reviewed alternative representation and wire API, and is **out of scope**. This is an explicit documented deviation, not a successful parity claim.

Valid adjacent high+low JSON escapes form a scalar and are parsed exactly, including emoji. `\\\\uD800` is a literal backslash-u sequence, **not** an isolated surrogate and must not be refused. Already-valid raw UTF-8 and escaped quotes must continue to behave normally. Existing upstream encoded metadata behavior decodes malformed UTF-8 using replacement before parsing; the pinned fixture contract remains unchanged and original bytes are not overwritten.

## Changes

- Reuse one byte-oriented lexical surrogate detector at the durable file ingress. It scans JSON string tokens without allocating transformed data, correctly distinguishing valid high+low escape pairs and escaped literal backslashes. JSON syntax validation still belongs to `serde_json`.
- Apply refusal to **metadata**, **JSONL history journal**, **legacy message map**, **canonical message shards**, **traces** and **history documents**. A lone surrogate can no longer be silently ignored as an invalid journal line/shard or silently overwritten by trace re-normalization. All refusal paths are read-only.
- Keep normal malformed/torn non-surrogate journal-line recovery and other source-specific bad-file fallbacks unchanged.
- Add a synthetic `cl_04c_json_edges.rs` suite for nested metadata/backup safety, positive Unicode surrogate pair/literal backslash, journal refusal/restart and positive parsing, shard and map refusal, trace and document refusal, plus existing pinned `session-metadata.json`, `session-title.json`, legacy recovery and HTML-entity tests.

## Acceptance

- Format: `cargo fmt --all --check`
- Lint: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Tests: `cargo test --workspace --locked` (both platforms)
- Build: `cargo build --release --locked` (both platforms)
- No pinned source fixture edits, tests must verify exact persisted bytes before/after rejection.
- **CI at document authoring: PENDING**. Qualify exact PR head, then merge only after CI passes; post-merge `main` CI is a separate gate.
- **Performance:** NOT MEASURED; no throughput assertion. **LOCAL REQUIRED:** NONE for this cloud unit. **YOU REQUIRED:** NO manual account/device test. Full Phase 1–6 and later native UI/browser/installer work remain PARTIAL.
- **Known limitations:** malformed non-UTF-8 in non-metadata JSON files still follows existing malformed-file handling, old unpaired strings are preserved but cannot be read through Rust `Value`, and an eventual surrogate-preserving representation must be scoped as a future separate unit, not silently conflated with CL-04C.
