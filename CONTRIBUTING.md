# Contributing to CoS-Rust

Contributions are welcome. Windows is the active qualification target. Linux work is welcome with
native execution evidence; macOS and other platforms are currently on hold. See
[current implementation work](docs/implementation-plan/2026-10-17-phase1-6-parity-closure-v1.0.md).

## Build and test

Install Rust 1.91 or newer with `rustfmt` and `clippy`, Git, and host OpenSSH. Linux tests additionally
need an installed `sftp-server` (`openssh-server` on Debian/Ubuntu). Windows uses the OpenSSH subsystem
included in Git for Windows at `C:/Program Files/Git/usr/lib/ssh/sftp-server.exe`.

```sh
cargo test --workspace --locked -j 2
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings
cargo build --release --locked -j 2
```

On Windows, prefer `python tools/check-workspace.py --strict`. It uses the workspace's
ignored `.test-tmp/` for child temporary files and `target/` for Cargo, preserves an explicitly
configured `CARGO_TARGET_DIR`, and requires 8 GiB free on the workspace drive. Environment changes
apply only to the check subprocesses. This avoids large fixture files filling the system Temp.
Development/test profiles omit debug symbols and incremental state by default; debug assertions,
overflow checks and test limits remain enabled. For a debugger, request symbols through Cargo's
`CARGO_PROFILE_DEV_DEBUG=2` / `CARGO_PROFILE_TEST_DEBUG=2` environment overrides. Different profiles
produce additional cache artifacts, so retain a single profile unless debugging requires another.

Tests use temporary public fixtures and fresh profiles. The service always requires an explicit
absolute `--data-dir`; never point tests at a personal CoS, browser or provider profile. Optional
external SSH/tunnel tests must state their host, authorization and isolation. An installed SFTP
subsystem proves file transport, not SSH authentication. Never commit secrets or profile data.

## Source parity and regression evidence

The comparison baseline is Chat On Steroids commit `024731d8083d92c46e8248d64e7e0b36ab0a470f`.
Rust tests run from checked-in fixtures without downloading upstream. To regenerate or inspect an
oracle, clone the original repository into the ignored `upstream/chat-on-steroids` directory using
the repository URL and exact commit in
[source identity](audit/2026-10-16/source-identity.json). Keep that clone unchanged. Capture scripts
require its JavaScript development dependencies and must not import the application bootstrap.

For behavior changes, demonstrate a functional failing regression before implementation, then make
it pass and run the affected suite plus integrated checks. Preserve raw data, request/epoch authority,
permission boundaries, source fixtures and historical receipts. Do not replace a real-input test with
a fixture merely to obtain green output, remove a failing test, or relabel NOT RUN as PASS.

## Changes and review

Keep commits focused. Describe the concrete trigger, resulting behavior, validation and remaining
limits. Record implementation work in dated/versioned documents under `docs/implementation-plan/`,
`docs/implementation-review/` and `docs/implementation-summary/`, and update `docs/README.md`.
Distinguish source implementation, native runtime tests, package proof and complete desktop acceptance.

Use only synthetic data in issues and pull requests. Report potential credential or authority issues
without publishing private profiles, live secret URLs or exploit details containing personal data.
Large runtime dependencies need an explicit size/feature rationale and license attribution. The
current JavaScript engine includes QuickJS C; describe it accurately when changing that boundary.

## Isolated SSH qualification

`tools/qualify-ssh.py --container <owned-linux-container> --output <receipt-path>` exercises the actual
host OpenSSH client against a disposable loopback sshd. The container must carry the label
`com.cos-rust.task=phase1-6-closure`, contain `openssh-server` and `netcat-openbsd`, and be dedicated
to public fixtures. The script generates temporary keys/config/known_hosts, uses a Docker stdio proxy
without publishing a host port, and stops its own daemon. It never selects personal SSH configuration.
This explicit check is separate from the normal installed-subsystem tests and external-server proof.

`python tools/qualify-linux.py --output audit/<date>/<receipt>.json` executes an exact Git commit
in that task-owned pinned Rust container. It creates a separate source checkout, shares only its
container build cache and records the commit, image, kernel, test counts and log hash. It does not
include uncommitted host changes, start Docker or install system dependencies. Add `--strict` for
formatting and all-target Clippy; the container then also needs those rustup components. Use a new
output path for each run so that historical receipts cannot be overwritten.
The default `--lean-build` profile uses no debug symbols or incremental cache,
one shared `/work/target`. Assertions and test limits are unchanged. Host source archives live in
the ignored workspace `.cache/qualification/`; the runner reserves 8 GiB on that drive and 12 GiB
on the system drive before using Docker. It records the profile and cache usage in the receipt.
Do not alternate build profiles unnecessarily or prune unrelated Docker data.
Use `--no-lean-build` only when full debug symbols/incremental state are deliberately needed.

`python tools/qualify-search.py --binary target/release/cos-rust.exe --output audit/<date>/<receipt>.json`
compares the actual loopback MCP service with the captured registered source handler, checks its
protocol metadata and exercises find through the isolated code worker. Public fixtures and fresh
profiles live under workspace `.test-tmp/`. Both directory-order inputs are captured; the actual
filesystem input selects the oracle without changing result counts or response acceptance.

`python tools/qualify-session.py --binary <service.exe> --output audit/<date>/<receipt>.json`
checks actual CLI Markdown payloads against the original export function and named model/title
metadata fields against source steps. It uses new marked profiles under workspace `.test-tmp/`.
Its receipt lists three store-API-only cases separately: two non-finite times and an explicit owner
lookup that is a different operation from recorder conversation routing. All 17 store API steps
remain covered by native tests. Clipboard/dialog and full desktop acceptance are separate.

`node tools/qualify-project-files.mjs [binary]` compares public native files against selected pinned
source declarations. It requires the upstream development dependencies, including Babel's parser,
esbuild and Sharp; ordinary Rust tests need none of these JavaScript packages. It records exact
snapshot/save and preview projection comparisons, with raster/locale/race limits stated separately.

The file-panel sorter uses ICU4X's Rust collator (`icu_collator` 2.1.1, Unicode-3.0 license) to retain
locale ordering instead of byte ordering. Its compiled Unicode data is a deliberate size cost;
record release size and qualify locale/version differences. `ryu-js` 1.0.3 (Apache-2.0 OR BSL-1.0)
retains JavaScript Number spelling for revision hashes, including native identities above 2^53.

Patch shell interception uses `tree-sitter`/`tree-sitter-bash` 0.25.1 (MIT), retaining the source Bash
query for every outer shell. These Rust bindings include a C parser and external scanner; they are
an explicit runtime dependency exception, not a pure-Rust parser. The checked-in 504-case corpus and
`audit/2026-10-18/bash-grammar-provenance.json` record grammar/query evidence. Their generated parser
parse tables match the npm source, with only package patch-version metadata differing. Record the
release footprint when changing this dependency, and retain its MIT license attribution in packages.
