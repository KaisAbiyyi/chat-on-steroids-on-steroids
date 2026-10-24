# CoSoS 15E — cloud localization, accessibility and keyboard focus

**Cloud implementor/self-review:** ChatGPT GPT-6 / COS-15E-20261024-0959, GitHub issue #70.  
**Prerequisite:** 15A PR #67 merged at eb9f3dc9684fe97017dcc3a57ae63553d2118547.  
**Lock:** ui-i18n; dedicated work/15e-locale-a11y-focus-20261024. No local machine/files/profile accessed.

## Implemented contracts

- 13 canonical UI tags read from the frozen upstream inventory (audit/2026-10-16/contracts.json): en, de, es, fr, pt-BR, pt-PT, ru, tr, vi, ja, ko, zh-CN, zh-TW. No invented Indonesian or Arabic entries. Unsupported/malformed stored values default to English, never a guessed prefix.
- 18 static translated shell keys per language (navigation, controls, accessible labels) in cos-core/ui_localization. Text-only constants, no HTML or dynamic string formatting. **This is a deliberately bounded shell subset, NOT upstream complete UI locale catalog parity** (original JSON assets have thousands of keys). All full catalog extraction, proofreading and complete renderer integration remain follow-up requirements.
- Host-owned UiFocus model with known Navigation/Main/Composer/Confirm/Cancel landmarks, bidirectional Tab traversal, modal Tab trap, Escape-to-restore, and strict composition/modified-key pass-through. Host-only focus and modal APIs, strict typed bounded keyboard JSON frame, epoch and ordered sequence replay fences, fixed error codes and rebind state reset. It can never synthesize an OS keystroke, access privileged tools or choose arbitrary DOM selectors.
- Keyboard frame counters are not authentication or authorization. Native host MUST authenticate renderer and origin before calling this model. Actual DOM roles, screen-reader announcements, high contrast, IME, native focus, DPI and device parity are separate UI/native/owner gates.

## Tests and acceptance

Targeted synthetic unit test: cargo test -p cos-core --test ui_localization_focus --locked. Covers every tag/key, regional variant, fallback, Tab/Shift+Tab, dialog restore/trap, IME composition, modifier pass-through, stale/duplicate/out-of-order/oversize/malformed frames and rebind.

Required CI: both Windows and Ubuntu Rust fmt, strict Clippy, workspace tests, release builds plus Task DAG. Implementor direct self-review of exact head/diff and security before merge; no fake PASS. The user must separately run/accept real native accessibility and international keyboard behavior through 15L2/15U2. 15F requires acceptance of 15B, 15C, 15D and 15E and is not unlocked by this branch alone.
