# gemelli Spout publisher 設計

日付: 2026-07-08
ステータス: ステークホルダー承認待ち(会話内)
前提: Phase 1 (core + CLI) / Phase 2 (GUI) 完了・main merge 済み。
macOS 向け `gemelli-syphon`(sender-only Metal publisher)は実装済みで、本 crate はその
Windows 対応版(mirror)である。

## 目的

Windows で webcam 映像を Spout 経由で共有できるようにする。`gemelli-core` の
`TexturePublisher` trait を実装した Windows 専用 crate `gemelli-spout` を追加し、
CLI / GUI から macOS の Syphon と同じインターフェースで利用できるようにする。

`gemelli-syphon`(`crates/syphon`)と対称な構造・規約(crate 全体 cfg gate、
`links` メタデータ、release-please 登録方式)をそのまま踏襲する。

## 決定事項(ステークホルダー確認済み)

- **送信のみ (sender-only)**。受信は対象外。`SyphonPublisher` と対称に `SpoutPublisher` のみ。
- **CPU ピクセル経路**を使う。`gemelli-core::Frame` は BGRA8 tightly-packed の CPU バッファ
  であり、`TexturePublisher::publish(&Frame)` は常に CPU バイト列を渡す。よって
  参照実装(electron-texture-bridge)の GPU テクスチャ共有(`SendTexture` / NT handle)
  ではなく、`spoutDX::SendImage(pixels, w, h, pitch)` を用いる。
- **Spout2 SDK は fetch script で `vendor/Spout2/` に取得**する(git submodule ではない)。
  `.gitignore` は既に `/vendor/Spout2` と `/_spout2_tmp` を無視しており、`scripts/fetch-fonts.sh`
  と同じ「fetch → vendor/(gitignore 済み)」パターンに揃える。
- **CI は Ubuntu 単一ジョブ**。OS ごとの matrix は設けない。native bridge(mac/win)は
  ローカルビルド時のみ検証し、CI は portable な workspace(native crate は cfg で空 stub 化)
  の fmt/clippy/test を Ubuntu で回す。現行 `ci.yml`(macos-15)を ubuntu へ変更する。
- **検証は実機 E2E まで**。Windows + MSVC2019 上で PoC 送信 → 実 Spout 受信アプリで映像確認。
- **release-please に `crates/spout` を登録**する(config + manifest)。

## アーキテクチャ

```
crates/core (portable)                     crates/spout (Windows only)
  trait TexturePublisher                     #![cfg(target_os = "windows")]
    fn publish(&Frame) -> Result             SpoutPublisher: TexturePublisher
  Frame = BGRA8 tightly-packed                 └ FFI ─→ cpp/spout_bridge.cpp
                                                          └ spoutDX::SendImage (Spout2 SDK)

crates/cli  create_publisher()   ─ #[cfg(windows)] → gemelli_spout::SpoutPublisher::new
crates/gui  open_publisher()     ─ #[cfg(windows)] → gemelli_spout::SpoutPublisher::new
```

macOS の `gemelli-syphon` とファイル構成・責務を 1:1 に対応させる。

### crate `crates/spout` (`gemelli-spout`)

- `Cargo.toml`: `edition.workspace`, `license.workspace`, `repository.workspace`,
  `links = "spout_bridge"`, `[lints] workspace = true`, `dependencies: gemelli-core`,
  `build-dependencies: cc`。version は `0.2.0`(他 crate と同一起点)。
- `src/lib.rs`: 先頭に crate 全体 gate `#![cfg(target_os = "windows")]`(release-please の
  Rust manifest updater が `cfg()` target table を解釈できない問題を回避するため、
  syphon と同じく crate 本体側で gate する。downstream の Cargo.toml では target 無しの
  通常依存として記述する)。
  - `SpoutPublisher { handle: NonNull<ffi::SpoutBridgeHandle> }`
  - `unsafe impl Send`(SyphonPublisher と同じ根拠: 非 Clone・ハンドル二重取得不可)
  - `new(server_name: &str) -> Result<Self, PublishError>`: `CString::new` で内部 NUL 検査
    → `spout_bridge_create` → `NonNull::new` で null 検査 → `PublishError::ServerCreate`
  - `impl TexturePublisher::publish`: `pitch = width * 4`(`checked_mul` でオーバーフロー検査)
    → `spout_bridge_send_bgra` → false は `PublishError::Publish`
  - `impl Drop`: `spout_bridge_destroy`
- `src/ffi.rs`: opaque `SpoutBridgeHandle`(`_private: [u8; 0]`)、edition 2024 の
  `unsafe extern "C"` ブロックで `spout_bridge_create` / `spout_bridge_send_bgra` /
  `spout_bridge_destroy` を宣言。
- `cpp/spout_bridge.h` / `cpp/spout_bridge.cpp`: `extern "C"` の薄いラッパ。
  - `struct SpoutBridge { spoutDX sender; }`(送信のみなので参照実装より大幅に簡素)
  - `spout_bridge_create(name)`: `sender.OpenDirectX11()` → 失敗で null。
    `sender.SetSenderName(name)` → 失敗で `CloseDirectX11` して null。
    `sender.SetSenderFormat(DXGI_FORMAT_B8G8R8A8_UNORM)`(Frame が BGRA のため)。
  - `spout_bridge_send_bgra(h, pixels, w, height, pitch)`: null/0 検査、
    `pitch >= w*4` 検査、`sender.SendImage(pixels, w, height, pitch)` の bool を返す。
  - `spout_bridge_destroy(h)`: `sender.ReleaseSender()` → `sender.CloseDirectX11()` → delete。
- `build.rs`: `CARGO_CFG_TARGET_OS != "windows"` なら no-op(空 crate をビルド)。
  windows のとき `cc::Build`(`.cpp(true)`, `/EHsc`, `/std:c++17`)で
  `spout_bridge.cpp` + Spout2 SDK の 7 ファイルをコンパイル:
  `SpoutDirectX/SpoutDX/SpoutDX.cpp`, `SpoutGL/{SpoutDirectX,SpoutSenderNames,
  SpoutFrameCount,SpoutUtils,SpoutCopy,SpoutSharedMemory}.cpp`。
  include: `vendor/Spout2/SpoutDirectX/SpoutDX`, `vendor/Spout2/SpoutGL`, `cpp`。
  link: `d3d11 dxgi user32 gdi32 shell32 ole32 comdlg32 comctl32 shlwapi`。
  `rerun-if-changed` を bridge ソースと vendor ディレクトリに設定。
  **rpath 機構は不要**: Syphon の dylib framework と異なり Spout2 SDK は静的リンク
  されるため、CLI の `DEP_SYPHON_BRIDGE_RPATH` に相当する仕組みは持たない。

### 参照実装との差分(なぜ簡素になるか)

electron-texture-bridge の `spout_bridge.cpp`(705 行)は Chromium の GPU テクスチャを
NT handle で受け渡す sender と、`spoutSenderNames`/`spoutFrameCount`/`spoutDirectX` を
直接叩く receiver を含む。gemelli は CPU フレームの sender のみなので、`spoutDX::SendImage`
一本で済み、receiver・NT handle・discovery は実装しない。ただし SDK 依存 .cpp
(SpoutDirectX 等)は SpoutDX が内部で使うため同じ 7 ファイルをコンパイルする。

### CLI / GUI への配線

- `crates/cli/Cargo.toml` / `crates/gui/Cargo.toml`: `gemelli-spout = { path = "../spout" }`
  を(syphon と同様に target gate 無しで)追加。
- `crates/cli/src/run.rs::create_publisher`: `#[cfg(target_os = "windows")]` の arm を追加し
  `gemelli_spout::SpoutPublisher::new(server_name)` を返す。
  `#[cfg(not(any(target_os = "macos", target_os = "windows")))]` を `UnsupportedPlatform` に。
- `crates/gui/src/worker.rs::open_publisher`: 同様に windows arm を追加。
- 既存の `UnsupportedPlatform` / エラーメッセージ(“Syphon/Spout publishing …”)は既に
  両者を想定した文言なので変更不要。

### fetch script

- `scripts/fetch-spout.sh`: `fetch-fonts.sh` を踏襲。pinned な leadedge/Spout2 の
  リリース(タグ固定)を取得し、`vendor/Spout2/` に SDK のディレクトリ構造を保持して展開
  (`SpoutDirectX/SpoutDX/`, `SpoutGL/`)。作業は `_spout2_tmp` を使い最後に掃除。
  README の Setup / Windows 節に実行手順を追記。

## ピクセル形式・向きの注意

- `Frame` は BGRA8。`SetSenderFormat(DXGI_FORMAT_B8G8R8A8_UNORM)` で BGRA として送る。
  `SendImage` は format 引数を持たず sender format に従うため、BGRA バイト列 + BGRA format
  で色順は一致するはず。PoC で受信色を確認し、チャネル反転が出た場合のみ format / swizzle
  を調整する。
- 上下の向き: Syphon は `flipped:YES` で publish している。Spout の `SendImage` に invert 引数は
  無い。PoC で受信映像の上下を確認し、反転する場合は送信前に行反転(または将来的な bInvert 付き
  API 検討)で対処する。判断は PoC 結果に委ねる。

## テスト方針 (t-wada TDD)

- 単体(host OS 非依存で回るもの):
  - `SpoutPublisher::new` が内部 NUL を含む名前を `PublishError::ServerCreate` で拒否する
    (CString 段階で失敗、GPU 不要)。syphon の `new_rejects_interior_nul` と対称。
- `#[ignore]` スモーク(実 GPU 必須、手動):
  - solid color frame を 1 枚 publish し、数秒 sleep して Spout 受信アプリで確認。
  - syphon の `publish_one_solid_color_frame` と対称。
- `run.rs` / `worker.rs` の publisher 選択は既存の generic なテストで担保済み。windows arm 追加時に
  該当テストが Windows でグリーンであることを確認する。

## 検証 (実機 E2E)

1. `scripts/fetch-spout.sh` → `cargo build -p gemelli-spout`(MSVC2019)。
2. `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`。
3. `gemelli`(CLI)を実カメラで起動 → Spout 受信アプリ(例: Spout の SpoutReceiver、
   または OBS + Spout2 plugin)で `gemelli`(server name)の映像を確認。色順・向き・解像度・
   rotate/flip/crop/scale が反映されることを確認。
4. GUI(`gemelli-gui`)でも publish → 受信確認。
5. 結果を README「Manual verification checklist」に Windows 項目として追記。

## CI 変更

- `ci.yml` を `ubuntu-latest` 単一ジョブへ変更。submodule(Syphon source)は Ubuntu では
  不要になるが、`fetch-fonts.sh`(GUI の include_bytes! 依存)は引き続き必要。
  native crate(syphon/spout)は cfg gate により Ubuntu では空 stub としてコンパイルされる。
  nokhwa の Linux ビルド依存(v4l 系)が必要なら apt ステップを追加する。
- fmt → clippy → test を Ubuntu で実行。mac/win の native ビルドはローカル(手動)に委ねる。

## 影響範囲まとめ

- 追加: `crates/spout/**`, `scripts/fetch-spout.sh`,
  `docs/superpowers/specs/2026-07-08-gemelli-spout-design.md`(本書), plan ファイル。
- 変更: `Cargo.toml`(workspace members に `crates/spout`), `crates/cli/Cargo.toml`,
  `crates/gui/Cargo.toml`, `crates/cli/src/run.rs`, `crates/gui/src/worker.rs`,
  `release-please-config.json`, `.release-please-manifest.json`, `THIRD-PARTY-NOTICES`
  (Spout2 = BSD-2-Clause 追記), `.github/workflows/ci.yml`, `README.md`。
- `gemelli-core` は無変更(trait/Frame をそのまま利用)。
```
