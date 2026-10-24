# 16C — Runtime bundling and software-license inventory (cloud)

Task: [Notion 16C](https://app.notion.com/p/400c4a6aaf7381399d55f2607802a3fc); claim [#107](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/107). Accepted 16A PR #103 is the prerequisite.

## Delivered interface

`tools/release_16c.py` creates a deterministic **headless-only** ZIP for the `cos-rust` release binary, project MIT license and verbatim dependency license texts. It inspects the exact `cargo metadata --locked --format-version 1` and `Cargo.lock` graph; rejects unknown/unreviewed SPDX expressions, missing notice/license text, unpinned registry dependencies, git dependencies, ambiguous package inventory and file changes. Generated `bundle-index.json` records artifact hashes/sizes, source SHA, target OS, sorted dependency versions/SPDX/checksums, and explicit `signature: NOT_SIGNED`, `native_host: NOT_INCLUDED`, `webview2_runtime: EXTERNAL_NOT_VERIFIED` markers.

The verifier checks exact file coverage, hashes, source SHA, archive path safety, member types and bounded sizes. It **does not** extract or execute the archive, bundle a real desktop product, provide code signing, or satisfy a native installer gate. Windows WebView2 is intentionally external and unverified at this cloud gate. License expressions are conservatively allowlisted; introducing a different SPDX identifier needs a documented license decision, not silently loosening the gate. For approved licenses, collected dependency files remain included verbatim rather than relying only on SPDX labels.

## CI

`python -m unittest tools.test_release_16c -v` runs on Windows and Linux with negative fixtures. The release-bundle workflow additionally builds `cargo build --release --locked -p cos-app`, captures locked Cargo metadata, constructs ZIP from the exact runner SHA and verifies the ZIP before reporting PASS. Existing workspace CI separately runs Rust fmt/Clippy/tests/build. Source/branch/bundle hashes and CI links are documented in the PR and task evidence once established.

## Downstream gates

`16D`: actual RAM/startup/size measurement with specified fixture and host. `16E`: install/update/rollback test matrix after accepted 16B/16C. `16L1`: native clean installation, runtime-dependency discovery and uninstall. `16L3`: final Windows product/installer performance and size; **all remain NOT_RUN** here. This cloud artifact is not a shippable signed CoSoS desktop installer. A separate product native host is still a prerequisite.
