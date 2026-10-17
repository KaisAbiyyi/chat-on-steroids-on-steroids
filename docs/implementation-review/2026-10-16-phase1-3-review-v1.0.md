# Phase 1–3 fresh review and bounded repair v1.0

Review source identity: `audit/2026-10-16/phase1-3-review-manifest.json`, SHA-256
`5e31617993afa5b5b714f453f96d6262c25195f7c0a4332c4d272d813b57352d`.
The root has no Git history; this immutable manifest identifies the review input.
Reviewer: fresh-context gpt-6-luna, maximum reasoning, read-only, one whole-scope pass as required by
the executing-plans/requesting-code-review skills. The reviewer verified 49 input hashes and the
clean upstream commit. It **did not run tests**; execution evidence below is the implementer's work.

Initial reviewer verdict: **REQUEST CHANGES**, eight Important findings, no separate Critical list.
The good boundaries noted were conservative config/authority restoration, higher-level durable
publication fences and separation of estimates from provider usage.

| Finding | Original defect | Implemented correction / evidence |
| --- | --- | --- |
| R1 | Resolving an approved root anew followed a replacement junction outside the original boundary. | Recheck stored and current canonical identity both ways. A real Windows junction fixture demonstrates the original failure and corrected refusal; nested escape also refused. |
| R2 | Lossy path text could alias non-UTF-8 Unix siblings through replacement characters. | Authorization now uses raw Path components on Unix and raw UTF-16 Windows ordinal comparisons. WSL aliases preserve Linux tail case. Windows projection tests PASS; Unix raw-byte test is present but NOT RUN on this host. |
| R3 | Cleanup retired every image reference before applying its byte deletion target. | Choose the actual subset before rewriting shards; zero target leaves all history/bytes untouched. Zero and one-byte-target regression went RED → GREEN. |
| R4 | Immediate durable staging could race per-file commit order; a stale retry snapshot could overwrite newer authority. | Per-file fence covers immediate stage+commit. Every background/flush snapshot has a generation fence. Deterministic stale immediate snapshot regression went RED → GREEN. Recovery backup/write shares the same fence. |
| R5 | Existing keyless Local State prevented storing the first secret. | If no encrypted secret exists, atomically add the DPAPI-wrapped key while preserving other Local State/os_crypt preferences. Existing ciphertext with a missing key still refuses. Windows regression went RED → GREEN. |
| R6 | Darwin 21.0–21.3 retained clipboard grants. | Release-aware capability projection retains browser screen/control and masks native clipboard before 21.4. Native Mac release is obtained through sysctl; projection test PASS, native Mac run NOT RUN. |
| R7 | CLI comparison cap was fixed to 256k even for Pro. | `usage --context-cap 400000` accepts explicit qualified input and labels its source. Default explicitly says provider catalog unavailable. Automatic model-catalog integration remains pending; no claim that this CLI knows account availability. Regression went RED → GREEN. |
| R8 | Provider usage rescanned arbitrarily large input on every call. | Seven-day mtime filter, 16 MiB aggregate default scan budget, 4 MiB line bound, bounded inventory and `limited:true` with scanned/budget bytes. Budget and stale-file regressions went RED → GREEN. Incremental persistent cache remains pending. |

The first junction regression accidentally used a verbatim Windows path in PowerShell's junction
target, so it did not create the intended live escape. The corrected fixture uses ordinary DOS paths
and proves the target file is readable through the junction before testing the Rust refusal.
The first Darwin projection fixture used fresh Mac defaults that already denied clipboard. It was
corrected to start from explicit granted capabilities so the test exercises the OS mask.

RED receipts: `audit/2026-10-16/phase1-3-review-red.log`.
Focused GREEN receipts: `audit/2026-10-16/phase1-3-review-green.log`.
Final workspace/Clippy/release receipts are separate and include these repairs.

## Declined to judge and execution rulings

- The reviewer did not classify correlation fallback scanning at most 100 sessions ×1,024 events as
  a new Rust defect because upstream uses the same fallback. Ruling: retain that source-compatible
  bound, and keep broad historical ownership migration acceptance pending.
- GUI/browser integration, actual model-catalog host and macOS/Linux native secret providers were not
  judged complete. Ruling: these remain open acceptance gates, as documented in the parity and owner
  coverage registers; they are not silently excluded from the requested phases.

There was no second fresh review or approval claim after repair. Final disposition is **repairs
implemented and locally verified within the stated host limits; full phase acceptance PARTIAL**.
