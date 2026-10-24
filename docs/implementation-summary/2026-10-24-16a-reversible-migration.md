# CoSoS 16A — Versioned reversible backup / restore contract (cloud)

Task: [16A](https://app.notion.com/p/400c4a6aaf73819882ffe6b39f2c00c5) · [exclusive GitHub claim #102](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/102) · branch work/16a-versioned-reversible-migration-20261024.

## Behavior and boundary

Phase 16A adds an opt-in **copy-only** backup and a separate **non-destructive** restore. The original `import --source` command and its format-1 profile marker remain compatible. Nothing selects a default source, touches an existing destination, modifies the original profile, deletes a backup, or operates on personal data within cloud CI.

- `cos-rust --data-dir <new-absolute-backup> backup --source <absolute-profile>` snapshots the explicitly supported data ownership scope: `config.json`, `secrets.bin`, `Local State`, `state/`, `sessions/`. It rejects links/nonregular entries and excessive depth/file count (existing import inventory rules), total bytes above 64 GiB, and data changed while copying.
- Published backup has a `cos-rust-backup-manifest.json` schema version **1** with file sizes and SHA-256, a pinned upstream identity, and no secret plaintext. It is stored alongside the complete data snapshot in a newly created backup folder. The CLI receipt returns `manifestSha256`; record that **outside the snapshot** using a trusted channel so modification of files and manifest can be detected.
- `cos-rust --data-dir <new-absolute-restore-target> restore --backup <absolute-backup> --manifest-sha256 <64-hex-trusted-receipt>` hashes/checks the manifest, rejects unknown schema/format/upstream and any file mismatch, then invokes the existing copy-only verified import to a **new marked** profile. Running the restored profile is a separate operator decision. No in-place downgrade/upgrade and no reusing a destination.
- A failed copy, parse, checksum, validation or publication does not publish a new marked profile; staging is owned by a scoped temporary directory. Both backup and restore refuse overlapping source and destination paths.
- Restore of an already-imported profile does not claim to roll back application logic or any still-running process: it builds a separate verified replacement directory. Operator preserves the original and switches to restored copy manually after independent validation. This is **rollback via profile selection**, not an in-place mutation.

## Caveats and follow-up

1. This snapshots only the explicitly supported `cos-app` data scope, not arbitrary WebView2/browser provider stores, external repositories, SSH server data, or app installer state. Such stores require separate scoped consent/qualification.
2. Neither SHA-256 nor its printed receipt signs the snapshot or authenticates a hostile issuer. Protect the expected manifest hash independently; storage ACLs, volume durability, external archival and adversarial rename races need physical / installer-level qualification.
3. Source file changes that produce exactly the same content at both inventory boundaries are not a reliable concurrent process lock. Backup a quiescent profile/copy, never an active personal process without owner approval.
4. The existing importer performs bounded multi-pass SHA-256 rechecks and sequential streaming copy. This intentionally trades disk I/O for consistency and low memory use. No benchmark or installed-app speed claim yet.
5. Real profile-copy approval belongs to **16U1**, after cloud 16E synthetic tests and backup-plan review. Physical qualification of an approved *copy* belongs to **16L2** after 16U1. Existing native/owner gates remain NOT_RUN.

## Regression scope

Five cloud synthetic Rust unit tests exercise basic backup+restore, modified data, wrong manifest digest and future version, non-overwrite/nesting, and Unix symlink denial. Required before acceptance: `cargo test -p cos-app --locked`, workspace Rust fmt/Clippy/tests/release on Ubuntu and Windows, direct exact-head self-review, a conflict-free merged PR, and Notion/GitHub sync. CI is not yet evidence of real Windows backup or rollback.

**No secrets from user profiles were read, copied or committed for 16A.**
