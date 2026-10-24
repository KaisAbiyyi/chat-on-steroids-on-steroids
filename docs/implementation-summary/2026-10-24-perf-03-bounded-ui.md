# PERF-03 — bounded UI serialization and lazy history/cache contract

Task: [PERF-03 #98](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/98). Claim COS-PERF-03-20261024-1735. Dependency PERF-00 [PR #105](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/pull/105) merged/accepted. Exclusive lock: perf-ui-state.

## Scope and design

- Reuse 15B conversation paging: no eager whole-session history, at most 24 visible rows and one bounded page in the host model. Page requests still require an authenticated host and ticket checks.
- Add a separate host-side projection cache with a single retained JSON frame (up to 96 KiB), not an unbounded LRU. All cached data is erased/retired on explicit clear, navigation changes and rejected epochs.
- Cache compares revision/epoch/route fences before expensive audit or JSON serialization. Unchanged input produces no new frame: the host should avoid sending repeated idle IPC updates.
- Only project/file/agent/extension metadata for the active panel is serialized. Only the active rich panel, not every rich editor/PDF/terminal panel, is projected for relevant surfaces. Conversation page is serialized only for the matching visible conversation route.
- Optional chat groups contain only title, group key and session keys, with 64 group / 256 session / 32 KiB caps. No full chat, message body, markdown or archive hydration. Group catalog is sent only on the expanded home sidebar.
- Host must reauthenticate the renderer transport and reauthorize every session/catalog request. Epochs and revisions are stale-work fences, not permissions; invalid metadata fails closed. A revoke/logout/rebind must invoke cache.clear immediately, independently of revision updates.

## Synthetic cloud acceptance

Run from the branch exact SHA:

- cargo fmt --all --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked (new suite: perf_03_ui_state)
- Cargo release build on GitHub Windows and Ubuntu
- Task DAG workflow if required by repository CI

Regression tests: 1024 idle refreshes produce exactly one serialized frame, hidden history and panels never enter Home payload, visible conversation can be served without other sessions, routing/inconsistent history fails closed and clears frame, cache clear invalidates, epoch recreation requires rebind, duplicate/oversized/stale chat metadata rejected. Source fixture is synthetic only.

## Limits / next integration

The current repo exposes host UI contracts in cos-core; cos-app is a CLI/service, not an integrated production native renderer. This task supplies the bounded host wire projection contract; native renderer/transport integration and measured end-to-end RAM reduction remain NOT_RUN, not silently claimed. Native Windows sustained 2h+ memory/endurance belongs to PERF-05 after all AND prerequisites are met. Never report memory savings from JSON serialization tests or synthetic fixture sizes.

Self-review must cover latest SHA, security/privacy, redirect to real production bridge integration, and CI. Do not merge while required checks fail.
