# CL-08A — Windows startup, settings and credential migration

**Execution:** ChatGPT GitHub cloud session `chatgpt-20261021-cl08a-01`; branch `work/cl-08a-startup-secrets-20261021`; resource lock `runtime-secrets`; prerequisite `CL-L1` accepted.
**Base:** `7942906cc845a02df4629de3b7830f2f729573d4`.
**Review:** [PR #23](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/23) and [Notion CL-08A](https://app.notion.com/3ffd8330eb468111945ffde33dab0e83).

## Code changes

- `cos-core/secrets.rs`: permit legacy **direct DPAPI** vaults without an existing Chromium `Local State` encryption key to be migrated only after authenticated OS decryption, JSON parse, and string-map validation. This generates the new wrapped AES key without resetting unrelated `Local State` settings. Migrated vault is rewritten to `v10` when first read; subsequent cold restart uses persisted wrapped key.
- Refuse missing AES key for an existing `v10` ciphertext, damaged direct-DPAPI, oversize or non-string credential object, without creating a replacement key or modifying the preexisting vault.
- `SecretStore::get` returns a clear unreadable credential error, instead of treating a damaged credential vault as an empty map.
- `ConfigStore::open` propagates filesystem I/O failures rather than pretending an inaccessible `config.json` is malformed configuration and silently starting in recovery mode. Truly malformed readable JSON retains prior recovery backup/read-only contract.
- `crates/cos-core/tests/cl_08a_startup.rs` exercises cross-platform failed vault and blocked configuration, and Windows-only actual DPAPI encrypted fixture, migration, cold restart, keyless v10/corrupt refusal and unchanged bytes.

## Gate matrix and constraints

- CI: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --release --locked` on Windows and Ubuntu, plus task DAG validation.
- **Native installed Windows CL-L4 remains NOT RUN** until CL-08B is accepted. No real Windows user secrets, D: files, provider login or installed desktop runtime were accessed.
- Pinned upstream/oracle fixtures unchanged. Scope does not modify CL-05A `process-supervisor`, CL-07A `search-patch` or CL-09A `mcp-protocol`.
- Mark `Done` only on exact-head PASS, independent review, merged commit, and reconciled Notion/registry with receipt. CL-08B remains blocked until then.
