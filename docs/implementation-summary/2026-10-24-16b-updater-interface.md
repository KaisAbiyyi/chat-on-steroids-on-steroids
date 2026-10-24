# 16B — Signed updater/install boundary (cloud stage)

Task: [Notion 16B](https://app.notion.com/p/400c4a6aaf738129b22de554aab5a09f); claim [#106](https://github.com/KaisAbiyyi/chat-on-steroids-on-steroids/issues/106). Dependency 16A merged in PR #103.

## Delivered interface

`tools/release_16b.py` is an inspect-only, fail-closed release-candidate preflight, independent of personal profile migration. It accepts a staged **Windows x64** `release.json` containing exactly `schema`, `product`, `platform`, `version`, `source_sha`, and `files` (each with `path`, `sha256`, `bytes`). The caller supplies a source commit SHA, current version and a separately trusted publisher certificate thumbprint. Manifest-provided issuer claims are never trusted.

The preflight refuses missing/unknown/extraneous files, unsafe paths and symlinks, edited hashes or sizes, oversized bundles, invalid versions, downgrade or same-version update, wrong product/OS/source, missing executable, invalid native Authenticode status and wrong trusted signing certificate. Only a native Windows Authenticode validation of **every** executable, DLL or installer type can produce a production `PASS` receipt. Hosted Ubuntu cannot forge native signature verification. Successful preflight returns metadata only; it does not install, overwrite, migrate, launch, download or auto-update.

## Fixture structure

```json
{
  "schema": "cosos.release.v1",
  "product": "CoSoS",
  "platform": "windows-x64",
  "version": "0.2.0",
  "source_sha": "0123456789012345678901234567890123456789",
  "files": [
    {"path": "cos-rust.exe", "sha256": "<64 lowercase hex digits>", "bytes": 1234}
  ]
}
```

`python tools/release_16b.py --staging-dir <disposable-stage> --source-sha <verified Git SHA> --current-version 0.1.0 --trusted-publisher-thumbprint <preapproved cert hex>`

The supplied thumbprint must come from owner-approved, out-of-band trusted release policy. No key, private profile, installer, user certificate or actual package is committed. Synthetic test injection only exists in Python API tests and cannot be requested by the CLI.

## Exit gate and limitations

Cloud tests on Ubuntu and hosted Windows: `python -m unittest tools.test_release_16b -v`, compiled Python and standard Rust CI. Direct self-review and exact-head PR checks are required before merge. Missing **actual signed installer**, native signature/provenance, deployment service, trust anchor enrollment and clean native installation remain honest **BLOCKED / NOT_RUN**. `16E` will exercise the install/upgrade/rollback matrix; `16L1` is the separate physical Windows gate and must not be inferred from this contract.
