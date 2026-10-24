# CoSoS 15F — fixed-DPI/theme screenshot regression capture contract

**Claim:** ChatGPT GPT-6 Cloud, COS-15F-20261024-1104 (generation 1). [Issue #78](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/78).  
**Cloud branch:** work/15f-snapshots-dpi-themes-20261024. **Base:** ff4fd9205b03a5036531f1a681273dbc83caf870. **Exclusive lock:** ui-snapshots.  
**AND prerequisites:** accepted/merged 15B PR #77, 15C PR #72, 15D PR #73 and 15E PR #74.

## Proven scope and capture protocol

- New \`cos_core::ui_snapshots\` module accepts five **host-published** snapshots from 15A/B/C/D/E: navigation, focus, panels/preferences, rich deck and conversation. All must share the same nonzero host epoch and valid revisions. Epoch is a consistency fence, **not** an authentication credential. Transport authorization stays with the native host.
- The projection normalizes routes to an allowlisted category and includes only sidebar, focus, modal, selected panel kind, rich panel type/count and bounded conversation window counts. It intentionally **never persists session IDs, project IDs, filenames, content, raw pixel bytes or secret settings**.
- Captures use fixed logical viewports **800 × 600** and **1280 × 720**, DPI **96 / 144 / 192**, resolved **light/dark** theme (no unresolved \`System\`), and exactly 13 supported language codes from 15E. That is **156 controlled appearance combinations** before selecting a UI surface. At 144 DPI, an 800 × 600 viewport must return exactly **1200 × 900 physical pixels**. Capture size is checked using integer math and bounded at **16 MiB**.
- Trusted renderer/fixture adapter must provide a *tightly packed, row-major sRGB RGBA8* framebuffer at the exact required dimensions. Every alpha byte must equal 255: transparent pixels could conceal nondeterministic RGB or user text. The cloud contract computes SHA-256 of the pixel payload **in memory** and returns a privacy-reduced \`CaptureProof\` only; it does not copy/capture a screen, access an OS profile, record PNGs or perform native screen scraping.
- Screenshot/snapshot verification compares both exact typed metadata (viewport, DPI, locale, theme, surface, source visual state, version) and pixel SHA-256, failing closed on either mismatch. No adjustable tolerance, arbitrary DPI, implicit golden rewrite or “update snapshot” shortcut exists. A captured pixel mismatch remains \`ChangedPixels\`; structural/appearance mismatches remain \`ChangedContract\`.
- Explicit surface contracts: \`Shell\`, \`Conversation\` (must match authorized session route), \`Rich\` (requires an active bound rich panel), \`Panels\` (requires a compatible route), \`Modal\` (requires modal focus). Preferences from 15D must match the capture's resolved theme and selected locale. Content used to seed any actual capture **must be synthetic**.

## Cloud regression coverage

\`cargo test -p cos-core --test ui_snapshot_regression --locked\` covers the 156 fixed viewport/DPI/theme/locale cases; exact DPR-to-pixel dimensions and memory caps; invalid viewport/DPI/System; deterministic comparison and one-channel pixel delta; theme/DPI/locale mismatches; malformed alpha/truncated/oversized frames; cross-epoch/stale-revision denial; navigation-to-conversation binding; privacy-preserving metadata; rich/modal constraints; appearance/preferences mismatch; and source-state changes with identical pixels.

**Important:** The in-memory test pixels are synthetic test vectors only. They are not a captured screenshot of the production CoSoS UI. The module is the **mechanism** for future locked baseline frames, not evidence of visual parity with the upstream Electron renderer. An approved native renderer must be integrated in 15G/15L2; synthetic golden proofs must be independently reviewed and checked in before claiming **pixel-level product parity**.

## Validation and acceptance gates

- Direct implementor self-review on exact final SHA and diff, including bounds, privacy, consistency, stale state and native-host authority.
- Required at exact HEAD: Windows and Ubuntu \`cargo fmt --all --check\`, strict Clippy, full-workspace tests, release build and Task DAG validation. A failed/pending run is not PASS; repair and retest on the new SHA.
- No local checkout/filesystem/Windows profile, native UI screenshot, actual screen-reader/IME/keyboard capture, physical DPI or owner UX test is authorized or claimed by 15F.
- Cloud acceptance requires reviewed code, passing hosted CI and merged PR. Native 15L2 and broader desktop integration 15G remain separate successor tasks. Do **not** mark those gates Done from this cloud PR alone.

## Handoff

**UNIT:** 15F. **CHANGES:** bounded source-projection and pixel-hash capture verifier, synthetic negative regression tests and this contract. **CLOUD VALIDATION:** fill from exact GitHub CI after PR. **PERFORMANCE:** no latency measurements claimed; frame bytes capped. **LOCAL REQUIRED:** separate 15L2 native DPI/locale interaction and screenshot matrix. **YOU REQUIRED:** owner UX gates remain later. **KNOWN LIMITS:** no working production renderer or physical screenshot backend in this cloud unit. **NEXT:** self-review, hosted CI and merge if genuinely green; then eligible 15G/15L2.
