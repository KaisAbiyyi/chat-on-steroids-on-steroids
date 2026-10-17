# CoS-Rust

Upstream Chat On Steroids is cloned at [`upstream/chat-on-steroids`](upstream/chat-on-steroids).
Rust runtime, sessions, MCP, files/projects/SSH and process/terminal services now run in this workspace.
The original **16-phase** roadmap remains the scope reference. **Phases 1–6 remain PARTIAL against
full exact parity**. Windows service implementation and evidence are recorded separately from GUI,
browser, external SSH/tunnel, packaging and native macOS/Linux acceptance.

- `cos-core`: config/migrations, capabilities, path boundaries, durable writes, protected Windows secrets, diagnostics and shutdown.
- `cos-store`: session epochs, journal/shards/legacy reads, terminal replay, correlation/access/recording, assets/plans/traces/handoffs, search/export and usage.
- `cos-exec`: literal shell policy, owner-bound processes, pipes/ConPTY, batches, publication/unread output and separate human terminal owners.
- `cos-tools`: regular approved handles, bounded previews/search/images, revision-fenced edits/patch rollback, projects/Git/watchers and host OpenSSH/SFTP.
- `cos-mcp`: separate tool surfaces, actual SDK protocol contracts, authenticated loopback HTTP/control, isolated JavaScript workers and optional tunnel supervision.
- `cos-app`: explicit-profile CLI, persistent service mode and verified legacy-copy import.

Build and try a new synthetic profile from PowerShell:

```powershell
cargo build --release --locked
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo info
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo self-test
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo sessions list
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo serve
.\target\release\cos-rust.exe --help
```

`history <id> --from 1 --limit 200`, `search <query>` and `export <id> --format md/json` read the stored
record. `usage --context-cap 400000` selects an explicit comparison cap; the CLI has no provider catalog.
`provider-usage` requires explicit `--codex-home` and `--claude-home`, respects opt-out and reports a
truncated scan with `limited:true`. No signed-in provider profile is selected automatically.

`serve` prints distinct secret Core/Desktop/Plugins URLs and stays running until Ctrl-C (or
`--duration-ms N`). Tool calls require current grants, approved roots and proven request ownership;
the source's explicit Allow unattributed calls setting remains off by default. Desktop/Plugins tools
stay unavailable until their real owners exist. Local control remains opt-in; action execution also
requires its later-phase backend. `projects add-ssh <address> <folder>` uses host OpenSSH. Public tunnels
run only with an explicit `serve --tunnel openai/cloudflared`; `--tunnel-binary` selects an installed
sidecar. OpenAI credentials are read from the selected explicit profile's encrypted slot and passed
only through child environment variables.

Service access to SSH filesystem paths and terminals requires the explicit host flag
`--trust-ssh-server-paths`, valid for one runtime only. SFTP v3 does not attest canonical paths of
opened handles; opt in only for a trusted server filesystem. This does not provide a remote OS sandbox.
Patch transactions admit at most 128 paths and a conservative 256 MiB workspace estimate before
retaining snapshots; split larger edits into separate transactions.

`tools list/call`, `projects list/add/add-ssh/git/bind` and `files read/save/patch` expose the service
owners. A one-shot CLI process ends its process sessions on exit; use persistent `serve` for command
continuation. JavaScript code mode uses statically linked QuickJS C in an isolated Rust worker.

Existing unmarked profiles are refused. `--data-dir <new absolute directory> import --source <original>`
copies config, secrets plus Local State, durable state and complete sessions/assets, verifies hashes
and publishes a new marked target. Test a copied profile before using it; broader historical migration
acceptance remains pending. `data/`, target outputs and probe profiles are ignored.

- [Deep scan and measured package breakdown](docs/research/2026-10-16-chat-on-steroids-deep-scan-v1.0.md)
- [16 migration phases, dependencies and acceptance gates](docs/implementation-plan/2026-10-16-rust-migration-phases-v1.0.md)
- [Implementation summary and validation status](docs/implementation-summary/2026-10-16-clone-deep-scan-v1.0.md)
- [Phase 1–3 implementation summary](docs/implementation-summary/2026-10-17-phase1-3-rust-foundation-v1.0.md)
- [Phase 4–6 implementation summary and evidence](docs/implementation-summary/2026-10-17-phase4-6-rust-services-v1.0.md)
- [Phase 4–6 plan and execution ledger](docs/implementation-plan/2026-10-17-phase4-6-implementation-v1.0.md)
- [Phase 4–6 exactness/coverage register](docs/research/2026-10-17-phase4-6-coverage-v1.0.md)
- [Phase 4–6 review and disposition](docs/implementation-review/2026-10-17-phase4-6-review-v1.0.md)
- [Parity specification and probe/deviation register](docs/research/2026-10-16-phase1-3-parity-spec-v1.0.md)
- [Coverage of all 18 phase-2 and 25 phase-3 source owners](docs/research/2026-10-16-phase1-3-owner-coverage-v1.0.md)
- [Fresh review and repair disposition](docs/implementation-review/2026-10-16-phase1-3-review-v1.0.md)
- [All 1,442 tracked files and proposed migration owners](audit/2026-10-16/migration-file-map.csv)
- [Interactive architecture graph](graphify-out/graph.html) — navigation aid with documented unresolved references.

The source baseline is `024731d8083d92c46e8248d64e7e0b36ab0a470f` on `main`.
Measured public installer: `v2.1.39`, commit `87ae9d631f82d8da2f462d910410031ad8f97ac5`.
The source snapshot is 87 commits ahead of that release; release size is not a build-size measurement of main.

Public installer bytes and an extracted ASAR are retained under `audit/2026-10-16/release-artifact/`
for reproducible inspection. The installer was not executed or installed. Qualification used public
synthetic data and an explicitly isolated Electron crypto profile; no real CoS/provider profile was used.

All original tracked upstream files remain unchanged. Rust source and artifacts live outside the clone.
Current service binary measurement is in `audit/2026-10-17/phase4-6-release-receipt.json`; the archived
phase 1–3 binary retains its historical receipt. Complete desktop
size, startup and memory comparisons remain NOT RUN. This CLI is not a replacement desktop application yet.

Verify with `cargo test --workspace --locked`, `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets --locked -- -D warnings`. `python tools/verify-foundation.py`
checks the saved final receipts, source hashes and upstream cleanliness.
`python tools/verify-phase4-6.py --binary target/release/cos-rust.exe` runs real MCP/JavaScript/process/file
workflows in a disposable public profile, including graceful shutdown and recording provenance.
