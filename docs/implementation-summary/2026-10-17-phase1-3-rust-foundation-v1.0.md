# Implementasi fase 1–3: Rust foundation v1.0

Mulai 2026-10-16; handoff 2026-10-17, Asia/Jakarta. Otorisasi: **“implement fase 1-3”**.
Referensi: [roadmap 16 fase v1.0](../implementation-plan/2026-10-16-rust-migration-phases-v1.0.md).
Source upstream tetap `024731d8083d92c46e8248d64e7e0b36ab0a470f`, app 2.1.39, protocol 14.

**Hasil: Rust foundation dan CLI sudah berjalan. Acceptance exact untuk fase 1, 2, dan 3 masih
PARTIAL.** Hasil ini belum merupakan aplikasi desktop pengganti Chat On Steroids. Seluruh file tracked
upstream tetap bersih; implementasi ada di root workspace, tanpa Git commit/publikasi baru.

## Yang sudah diimplementasikan

| Fase | Hasil konkret | Status acceptance |
| --- | --- | --- |
| 1 | Golden config/default/recovery/migration, history/rebind, chronology, Markdown, Unicode search, token/provider usage dari fungsi upstream yang dipin; original test baseline; Windows crypto interop; parity/probe/deviation register. | PARTIAL: pilihan dan pembuktian GUI/browser/JS/Wasm, live workflow, macOS/Linux serta perbandingan startup/RAM desktop masih terbuka. |
| 2 | `cos-core`: config validation/migration, conservative recovery dan backup, capability/readOnly projection, approved-root identity dan raw native path boundary, durable generations/retry/flush, process instance lease, Windows AES-GCM/DPAPI secrets, diagnostics/redaction, cancellation/bounded shutdown. | PARTIAL: native secrets macOS/Linux, seluruh historical config/crypto matrix, host startup hooks dan fault/power-loss qualification belum lengkap. |
| 3 | `cos-store`: compatible JSONL journal, stable-id message shards dan legacy overlay, terminal replay/restart, local epoch rebind, first-proof correlations, Trust/Block/provenance, recorder APIs, overflow/assets/quota/explicit image retirement, process completion, plan/trace/handoff storage, paging/search/find, Markdown/audit JSON, local estimates dan provider log readers. | PARTIAL: seluruh recorder/recovery/activity/title/summary/image/entity/table/export branches dan large-history qualification belum selesai. |

`cos-app` menyediakan CLI dengan data directory eksplisit, marker profil, verified copy import,
smoke fixture publik, config/session/history/recording/policy/export/usage commands dan persistence shutdown.
Profil CoS lama tanpa marker tidak langsung dibuka untuk ditulis. Import memilih target baru, menyalin
seluruh data fase 1–3 termasuk assets, mengecek hash source dan salinan, lalu memublikasikan target.

[Coverage ledger](../research/2026-10-16-phase1-3-owner-coverage-v1.0.md) mencatat **semua 18 owner fase 2
dan 25 owner fase 3**, termasuk bagian yang belum diport. Tidak ada klaim bahwa file sumber yang
berkaitan dengan host fase berikutnya sudah menjadi implementasi penuh hanya karena tipe/datanya dibaca.

## Validasi akhir

| Pemeriksaan | Hasil |
| --- | --- |
| `cargo test --workspace --locked` | **54 PASS, 0 FAIL, 0 ignored** di Windows x64. |
| `cargo fmt --all --check` | PASS. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS, tanpa warning. |
| `cargo build --release --locked` | PASS. |
| Original selected upstream suites | **478 PASS, 10 SKIPPED**, delapan file test. Skip tetap dicatat terpisah. |
| Electron 44.3.0 ↔ Rust protected secrets | PASS dua arah dengan profil eksplisit terisolasi dan plaintext uji publik; Local State/DPAPI/AES-GCM kompatibel. |
| Original history/shard round-trip dan legacy overlay | PASS pada fixture source-derived; source log tidak ditulis ulang saat canonical message direvisi. |
| Restart/sequence/torn tail/rebind/terminal state | PASS pada regression corpus lokal, termasuk 24 append bersamaan. |
| Windows nested/replaced junction boundary | PASS, target junction dibuktikan hidup sebelum penolakan diuji. |
| Fresh review | Satu review baru menemukan 8 Important issues; satu bounded repair pass dilakukan, dengan RED → GREEN dan full suite sesudahnya. Tidak ada klaim second-review approval. |
| Unix raw-byte path test, native Mac release/sysctl | NOT RUN pada host Windows ini. Test/adapter tersedia; jangan dihitung sebagai cross-OS PASS. |
| Signed-in provider, native UI/browser, installer penuh | NOT RUN. |

Receipt yang dapat diperiksa:

- [Final verification](../../audit/2026-10-16/phase1-3-verification.json)
- [Final Rust test output](../../audit/2026-10-16/phase1-3-tests-final.log)
- [Windows crypto interop](../../audit/2026-10-16/phase1-3-crypto-receipt.json)
- [Release measurement](../../audit/2026-10-16/phase1-3-release-receipt.json)
- [Fresh review dan repair disposition](../implementation-review/2026-10-16-phase1-3-review-v1.0.md)

Delapan repair mencakup approved-root replacement, raw path identity, cleanup byte target, stale durable
generation, keyless Local State, Darwin clipboard mask, explicit context cap dan bounded provider scan.
Provider scan default dibatasi **16 MiB total**; hasil yang belum lengkap menyertakan `limited:true` dan
jumlah byte yang dibaca. `usage --context-cap 400000` memakai input eksplisit; katalog model provider
belum diintegrasikan dan default CLI menyatakan batas bukti tersebut.

## Ukuran dan cara mencoba

Release executable: `D:\projects\CoS-Rust\target\release\cos-rust.exe`.
Ukuran akhir: **3,782,656 byte / 3.607 MiB**. SHA-256:
`53679176a1274edf2586b4c776c09b6a57a7bc72705cf07262abe311ee229210`.
Ini ukuran headless Rust foundation. Ukuran installer/payload desktop lengkap belum diukur; Chromium
host, native UI/media, updater, plugin runtime dan fitur fase selanjutnya belum masuk binary ini.

```powershell
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo info
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo self-test
.\target\release\cos-rust.exe --data-dir D:\projects\CoS-Rust\data\demo sessions list
.\target\release\cos-rust.exe --help
```

Program menggunakan profil baru tersebut. Tidak ada network/provider call dalam smoke run.
Startup receipt mengukur sepuluh proses CLI baru pada filesystem yang sudah hangat; angka itu tidak
dipresentasikan sebagai startup aplikasi desktop. Peak memory dan original desktop comparison NOT RUN.

## Gate yang masih terbuka

Lanjutkan berdasarkan [parity/probe register](../research/2026-10-16-phase1-3-parity-spec-v1.0.md) dan
[owner coverage](../research/2026-10-16-phase1-3-owner-coverage-v1.0.md), bukan dengan menandai tiga fase
ini selesai karena build kecil. Prioritas: GUI/browser/JS/Wasm feasibility; protected secret providers
macOS/Linux; exact recorder/recovery/title/summary/rich-content behavior; dan corpus historical/large
history dengan fault injection. Batas JS lone-surrogate versus Rust UTF-8 juga masih perlu keputusan
compatibility sebelum label “everything exactly to Rust” dapat diterima.

Roadmap/scan awal tetap disimpan sebagai sejarah; [execution ledger](../worklogs/2026-10-16-phase1-3-ledger.md)
menyimpan keputusan, koreksi fixture, review dan bukti. Investigasi lama sudah selesai; implementasi
produk sekarang sudah dimulai dan dapat dijalankan, dengan batas acceptance di atas.
