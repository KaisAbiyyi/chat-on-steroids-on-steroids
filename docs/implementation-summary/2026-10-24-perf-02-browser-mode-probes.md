# PERF-02 — external-browser reuse versus lazy embedded WebView2

**Issue:** [#97](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/97) · **Environment:** GitHub cloud only · **Prerequisite:** PERF-00 accepted PR #105 · **Lock:** `perf-browser-mode` · **Notion:** [execution task](https://app.notion.com/p/403c4a6aaf738171af6ddcaf38cf7b8b).

## Problem and scope

CoSoS is a lean ChatGPT coding workstation: minimize RAM/CPU, idle work, startup and operational complexity without sacrificing custody, durability, no-duplicate-Send or explicit user permission. PERF-02 does **not** choose a winner based on intuition. External browser reuse can avoid a separately owned embedded renderer but external processes and their children still consume memory, have shared ownership complications and may not expose an authorized debugging/extension bridge. Lazy WebView2 can preserve native profile custody and browser control, but adds an isolated renderer tree when requested. Neither is proven faster or smaller in production.

PERF-02 ships a Python, **synthetic-only** architectural transition probe (`tools/perf_02.py`) and guard/regression tests (`tools/test_perf_02.py`). It proposes native actions, **never** launches WebView2, enumerates a browser, opens an external profile, operates on signed-in sessions or executes permission/Send. Its Boolean flags are test doubles, not attestation. Code integration with a production host requires separate native proof and review. Existing 8A–8C browser contracts and native 8L1 remain independently authoritative.

## Architecture comparison

| Boundary | External browser reuse | On-demand isolated WebView2 |
|---|---|---|
| Activation | Explicit user select + fresh consent + host-attested *exclusive session lease* | Explicit user select + fresh consent + host-attested isolated partition |
| Zero-chat idle | No external attach; no new browser | No eager WebView2 controller/runtime |
| Ownership | Shared/external process tree; detach lease **without killing shared browser** | Host-owned controller/process tree; dispose on close/crash/restart |
| ChatGPT session | No auto-reading Chrome/Edge/Brave cookies, profiles, tokens or debug ports; require authorized native adapter | No silent credential import/handoff; 8B profile and 8C consent remain required |
| Privileged operations | No implicit CDP attach, extension, navigation, Send or OS permission | No implicit CDP attach, extension, navigation, Send or OS permission |
| Failure | No fallback into embedded mode; explicit fresh selection/consent | No fallback into external mode; explicit fresh selection/consent |
| Benchmark | Count attributable external browser **and its descendants**; track shared-browser ambiguity | Count Rust, host, renderer, WebView2, workers, sidecars |
| Outcome | Unproven until physical product/account-adapter qualification | Unproven until physical product/native qualification |

**Preferred product direction to evaluate:** no browser created on zero-chat idle; prefer an explicit *capability-gated* external reuse option only when the owner and native host can establish safe session lease and full-process attribution; otherwise offer explicit opt-in lazy WebView2. Do not auto-switch modes. This is a conditional architecture hypothesis, **not an endorsed default or proven implementation**.

## Transition and authority contract

- States: `closed`, `external-reuse`, `embedded-lazy`. Only `open` from closed may activate a mode; no silent choice, fallback or switch.
- A fresh strictly increasing host generation is required. A synthetic open requires true explicit consent, trusted native attestation and isolated profile flags. Reuse additionally requires a host-verified **exclusive external lease**; just finding a running browser process is never enough.
- `idle` proposes **no action**. Close, crash and host restart release once. An external lease produces detach-only, **never process kill**; an embedded instance produces a request for native disposal. Restart invalidates stale events. Unknown keys, strings containing account data, hostile schema, replay, duplicate open or stale generations fail closed.
- Returned `effects` are **requests** to a privileged native host, not proof an OS operation succeeded. Actual atomic grant, lease ownership, disposal, recovery and identity belong to reviewed Rust/native code. One-use consent and native origin checks must be enforced at the actual side-effect boundary; postchecks cannot undo side effects.
- Probe output intentionally contains only effect enums/counters/generations. No profile path, URL, title, user identity, prompt, email, cookie, credential or arbitrary PID accepted. No persisted state or provider session.

## Cloud reproducibility and acceptance

```sh
python -m unittest discover -s tools -p test_perf_02.py -v
python tools/perf_02.py smoke
python -m unittest discover -s tools -p test_perf_00.py -v
```

`.github/workflows/perf-02.yml` runs Windows and Ubuntu hosted matrix; exact PR head must also retain task DAG and Rust security workflows. Include implementor self-review of diff, fences, privacy and previous native/owner statuses. Until those runs pass and PR is reviewed/merged: **cloud acceptance pending**. The smoke report is always `synthetic-architecture-only`; `performance_winner=UNDETERMINED`, `native_product_acceptance=NOT_RUN` and `provider_account_acceptance=NOT_RUN`.

## Future real-product measurement protocol (PERF-05)

Only after native host capability and owner approval: keep machine/OS, power mode, source and binary SHA, profile class, workload, chat count and duration matched. Compare PERF-00 workload IDs `external_browser` and `embedded_browser`, including warm/cold, 0/1/multiple chats, terminal activity, background idle, crash/recovery, process/handle growth. Use PERF-00 explicit process roots with stable PID identity and full *owned plus attributable external* descendant memory; shared-browser ambiguity or incomplete roots blocks claims. Record private bytes, working set/peak/trend, CPU idle/active, startup, p50/p95, 2h endurance and cleanup. Physical D: owner-local native sign-in only with explicit approval. No cloud fixture can prove a 50% saving, recommend a default by measured merit or mark physical gates PASS.

**Still NOT_RUN:** external browser native integration capability, real account/host authorization and consent, Windows installed-product memory comparison, full UI host, native lifecycle resource reclamation and PERF-05 endurance.
