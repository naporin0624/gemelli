# 配布成果物 (.app / .dmg / CLI tarball) + アプリアイコン Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development で task ごとに実装。各 step は checkbox (`- [ ]`)。

**Goal:** gemelli CLI/GUI を他マシンで動く配布物 (`.dmg` + `.tar.gz`) にし、双子/ミラーのアプリアイコンを `.app` と実行時ウィンドウ/Dock に埋め込む。

**Spec:** `docs/superpowers/specs/2026-07-08-distribution-artifacts-design.md`(承認済み)

**Branch:** `feature/distribution-prep` 継続 (Phase 3.5 HEAD 上に積む)。タスク番号は Phase 3.5 の Task 10 に続けて **Task 19〜24**(11〜18 は Phase 3.5b compact UI 用に予約済みのため飛ばす)。

## Global Constraints (Phase 3 と同一)

- workspace lints: `clippy::unwrap_used` / `expect_used` / `as_conversions` は **deny**(`#[cfg(test)]` 内のみ例外)。
- 各コミット前: `cargo fmt --all` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`(husky pre-commit も強制)。
- コミットメッセージ末尾に空行 + `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` trailer。
- コメントはコードから読めない制約のみ。過去の文脈(「〜から変更」「Task N で」)への言及禁止。
- 各タスク完了後: difit を起動してレビュー依頼。
- タスクは番号順(Task 21 は Task 20 の pure 関数に、Task 22 は Task 21 の `.app` に依存)。
- 勝手に commit しない — 各タスクのコミットは実施してよいが、ブランチのマージ/push はしない。

## Task Overview

| # | 内容 | 主な成果物 |
| --- | --- | --- |
| 19 | 実行時アイコン埋め込み | `gui/assets/icon.png`(コミット済み) + `gui/src/main.rs` の `with_icon` 配線 |
| 20 | xtask bundle **pure 層** | `xtask/src/bundle/{plist,layout,cmd}.rs`(TDD) |
| 21 | xtask bundle **shell 層** | `cargo xtask bundle` → `target/dist/gemelli.app` |
| 22 | xtask dist (dmg + cli tar) | `cargo xtask dist` → `.dmg` + `.tar.gz` |
| 23 | CI 自動リリース | `.github/workflows/release.yml` |
| 24 | 配布ドキュメント | README「Install / Distribution」節 |

アイコンのマスター素材 (`icon.png` 1024²透過 + `icon.icns` + `scripts/gen-icon.sh`) はオーケストレータが事前生成し Task 19 のブランチにコミット済みとして扱う。

---

### Task 19: 実行時ウィンドウ/Dock アイコン埋め込み

**Files:**
- Add: `crates/gui/assets/icon.png`(1024×1024 RGBA、オーケストレータ生成済み)
- Modify: `crates/gui/src/main.rs`

**Interfaces:**
- `eframe::icon_data::from_png_bytes(png: &[u8]) -> Result<egui::IconData, image::ImageError>`(registry source で確認済み。`image`+png feature は eframe 非オプション依存 → 新規依存不要)。
- `ViewportBuilder::with_icon(impl Into<Arc<IconData>>)`。

**Steps:**
- [ ] Step 1 (RED): `main.rs` の `#[cfg(test)] mod tests` に、埋め込み PNG が 1024×1024 かつ `rgba.len() == 1024*1024*4` にデコードされることを検証するテストを追加。pure helper `fn app_icon() -> Result<egui::IconData, image::ImageError> { eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) }` を呼ぶ。helper 未定義でコンパイル RED。
- [ ] Step 2 (GREEN): `app_icon()` を定義し、`main()` で `ViewportBuilder::...with_icon(...)` に配線。`from_png_bytes` の `Result` は `unwrap`/`expect` 禁止 — 失敗時は `eprintln!` してアイコン無しで続行 (`if let Ok(icon) = app_icon() { viewport = viewport.with_icon(icon); }`)。
- [ ] Step 3: `cargo run -p gemelli-gui` で Dock/ウィンドウにアイコン表示を目視 (human checklist)。
- [ ] fmt / clippy / test → commit。

**検証:** `from_png_bytes` が `eframe::icon_data` から pub で見えること (use パス最終確認)。`icon.png` が実在しないと `include_bytes!` がコンパイルエラー → Task 19 前にアイコン素材コミット必須。

---

### Task 20: xtask bundle **pure 層**(TDD)

**Files:**
- Add: `crates/xtask/src/bundle/mod.rs`, `plist.rs`, `layout.rs`, `cmd.rs`
- Modify: `crates/xtask/src/main.rs`(`mod bundle;` 追加のみ、コマンド配線は Task 21)

**pure 関数 (副作用なし、全て `#[cfg(test)]` で網羅):**
- `plist::info_plist_xml(fields: &PlistFields) -> String` — `PlistFields { bundle_name, display_name, identifier, executable, icon_file, short_version, version, min_system_version, camera_usage_description }` から Info.plist の XML 文字列を生成。**exact-string テスト**で全体を pin (Task 4 の render テストと同方式)。`NSCameraUsageDescription` / `NSHighResolutionCapable=true` / `CFBundlePackageType=APPL` を必ず含む。
- `layout::AppBundlePaths::new(dist_dir, app_name)` — `Contents/{MacOS,Frameworks,Resources}` 各パスを算出。
- `layout::tarball_dir_name(version: &str) -> String` → `gemelli-<version>-macos-universal`。
- `cmd::lipo_create_args(inputs: &[PathBuf], output: &Path) -> Vec<OsString>`。
- `cmd::add_rpath_args(rpath: &str, binary: &Path) -> Vec<OsString>` → `["-add_rpath", rpath, binary]`。
- `cmd::cargo_build_target_args(package: &str, target: &str) -> Vec<OsString>` → `["build","--release","-p",package,"--target",target]`。
- `cmd::hdiutil_create_args(volume, srcfolder, output) -> Vec<OsString>`。

**Steps:** 各関数を RED (テスト先) → GREEN。`OsString`/`PathBuf` を使い `as` 変換なし。バージョン値は `env!("CARGO_PKG_VERSION")` を呼び出し側 (Task 21) が渡す — pure 層は文字列を受け取るだけ。

---

### Task 21: xtask bundle **shell 層** — `cargo xtask bundle`

**Files:** Modify `crates/xtask/src/main.rs`(+ `bundle/mod.rs` の shell 関数)

**Behavior:** `cargo xtask bundle` が:
1. `cargo build --release -p gemelli-gui --target aarch64-apple-darwin` と `--target x86_64-apple-darwin` を実行。
2. `lipo -create` で 2 バイナリを `target/dist/gemelli.app/Contents/MacOS/gemelli-gui` に結合。
3. `vendor/Syphon.framework` を `Contents/Frameworks/` に再帰コピー。
4. `install_name_tool -add_rpath @executable_path/../Frameworks` をバイナリに実行。
5. `plist::info_plist_xml` で `Contents/Info.plist` を書く(version = `env!("CARGO_PKG_VERSION")` を gemelli-gui の Cargo から。xtask からは gui の version を `--version` 引数か固定参照で受ける — **実装時に gui version の取得方法を確定**、暫定は workspace 共通 0.1.0)。
6. `crates/gui/assets/icon.icns` を `Contents/Resources/icon.icns` にコピー。
7. `THIRD-PARTY-NOTICES` を `Contents/Resources/` にコピー。

**エラー処理:** 各サブプロセスの非 0 終了・IO 失敗は `XtaskError` の新 variant で伝播 (`unwrap` 禁止)。ビルド前に `target/dist/gemelli.app` を掃除 (冪等)。

**Steps:** shell 関数を実装 → `cargo xtask bundle` 実行 → `open target/dist/gemelli.app` が別マシン相当で起動するか、最低限 `otool -l .../gemelli-gui | grep -A2 LC_RPATH` に `@executable_path/../Frameworks` が入るか、`codesign --verify` ではなく `./Contents/MacOS/gemelli-gui` 直接起動で framework 解決を確認 (human)。

---

### Task 22: `cargo xtask dist` — .dmg + CLI tarball

**Files:** Modify `crates/xtask/src/main.rs`(+ bundle/mod.rs)

**Behavior:** `cargo xtask dist`:
1. `bundle`(Task 21)を実行して `.app` を用意。
2. `hdiutil create -volname gemelli -srcfolder target/dist/gemelli.app -ov -format UDZO target/dist/gemelli-<version>-macos.dmg`。
3. CLI universal2 をビルド(`gemelli-cli`, bin `gemelli`)→ lipo 結合。
4. `target/dist/<tarball_dir>/` に `gemelli` + `Syphon.framework` + `THIRD-PARTY-NOTICES` + `README.txt` を配置し、`install_name_tool -add_rpath @executable_path gemelli`。
5. `tar czf target/dist/<tarball_dir>.tar.gz -C target/dist <tarball_dir>`。

**Steps:** 実装 → `cargo xtask dist` → `.dmg` マウント + `.tar.gz` 展開して起動確認 (human)。README.txt 文言は pure 関数 + exact-string テストで固定。

---

### Task 23: CI 自動リリース `.github/workflows/release.yml`

**Files:** Add `.github/workflows/release.yml`(既存 3 workflow は変更しない)

**内容:**
- `on: release: types: [published]`(release-please のリリース発行で発火)。
- `runs-on: macos-15`、`actions/checkout@v7` submodules recursive。
- Syphon.framework ビルド(ci.yml と同一コマンド)→ `./scripts/fetch-fonts.sh`。
- `rustup target add aarch64-apple-darwin x86_64-apple-darwin`。
- `cargo xtask dist`。
- `gh release upload ${{ github.event.release.tag_name }} target/dist/*.dmg target/dist/*.tar.gz --clobber`(`GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`)。

**Steps:** workflow 追加 → YAML lint(`actionlint` あれば)→ 目視レビュー。実発火は tag 時のため human が後日確認。

---

### Task 24: README「Install / Distribution」節

**Files:** Modify `README.md`

**内容:**
- エンドユーザ向け: `.dmg` から `.app` を Applications へ。**未署名**のため初回のみ右クリック→「開く」(または `xattr -dr com.apple.quarantine /Applications/gemelli.app`)。カメラ権限の許可について一言。
- CLI: `.tar.gz` 展開 → `xattr -dr com.apple.quarantine <dir>` → `./gemelli --help`。
- 開発者向け: `cargo xtask bundle` / `cargo xtask dist` のローカル生成手順、成果物パス `target/dist/`。

**Steps:** 追記 → 目視。

---

## Sequencing & 依存

```
アイコン素材(orchestrator) → 19 ─┐
                                20 → 21 → 22 → 23
                                                24 (docs, 22 完了後いつでも)
```

## 実装時に確定すべき未解決点 (spec §10 再掲)

- `eframe::icon_data` の use パス最終確認。
- `LSMinimumSystemVersion` の値(暫定 11.0、Metal/Syphon 要件で確定)。
- gemelli-gui の version 文字列を xtask がどう取得するか(暫定 workspace 0.1.0 固定、release-please 連動は将来)。
- universal2 クロスビルドで syphon_bridge.mm(cc)が両アーキ通るか(実ビルド確認)。
