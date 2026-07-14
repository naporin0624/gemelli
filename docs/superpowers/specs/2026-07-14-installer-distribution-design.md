# インストーラー配布 (bucatini 方式) Design Spec

**Status:** Approved (AskUserQuestion, 2026-07-14)
**Date:** 2026-07-14
**Reference:** [naporin0624/bucatini](https://github.com/naporin0624/bucatini) の release.yml / packaging/
**Depends on:** 2026-07-08 distribution-artifacts (macOS `.dmg` + `.tar.gz` は `cargo xtask dist` で実装済み)

## 1. 目的

gemelli のリリースを bucatini と同じ形に揃える:

1. **Windows インストーラー** — Inno Setup 製 `gemelli-<ver>-windows-x64-setup.exe` + 素の `.zip` を GitHub Release に添付する。
2. **リリース自動起動** — release-please とビルドジョブを単一 workflow に統合し、リリース作成時にビルドが自動で走るようにする (現状は GITHUB_TOKEN の cascade 問題で手動 dispatch が必要)。
3. **README の Install (prebuilt) セクション** — Releases からのインストール手順を案内する。

確定済みステークホルダー決定 (AskUserQuestion, 2026-07-14):

| 項目 | 決定 |
| --- | --- |
| スコープ | 上記 3 点すべて |
| Windows パッケージング実装場所 | `cargo xtask dist` の拡張 (bucatini の workflow 内 PowerShell インラインではなく)。`.iss` は宣言的設定として `packaging/windows/` に置く |
| `.ico` | ローカルで一度生成してコミット (`icon.icns` と同じ生成物コミット方針)。CI に Python/Pillow 依存を持ち込まない |

## 2. bucatini との差分 (なぜ完全コピーではないか)

- gemelli は macOS パッケージングを `cargo xtask dist` (Rust、ユニットテスト付き) で実装済み。Windows も同じ場所に載せることで、パッケージングロジックの二重構造 (Rust + PowerShell) を避ける。
- gemelli の Spout backend は SpoutDX/SpoutGL を**静的にコンパイル**しており、bucatini の NDI Runtime のような外部ランタイムのインストール案内は不要。`.iss` と README がその分シンプルになる。
- アイコンは既存の `scripts/gen-icon.py` (Pillow 幾何描画、決定的) 系統に `.ico` 出力を足し、生成物をコミットする。

## 3. Windows パッケージング — `cargo xtask dist` 拡張

`Commands::Dist` を `#[cfg(target_os = "macos")]` / `#[cfg(windows)]` で OS ディスパッチする。

### macOS (既存のまま、変更なし)

`.app` → `target/dist/gemelli-<ver>-macos-universal.dmg` + CLI `.tar.gz`。

### Windows (新規)

1. **staging** — `target/dist/gemelli-<ver>-windows-x64/` に以下を集約:
   - `target/release/gemelli.exe` (CLI)
   - `target/release/gemelli-gui.exe` (GUI)
   - `crates/gui/assets/icon.ico`
   - `README.md`, `THIRD-PARTY-NOTICES`
2. **zip** — staging ディレクトリを `target/dist/gemelli-<ver>-windows-x64.zip` に圧縮。外部コマンド (`tar.exe -a -c -f`、Windows 10+ 標準) を使い、zip クレート依存を増やさない。
3. **installer** — `ISCC.exe` を defines 付きで呼び出す:
   ```
   ISCC.exe /DMyAppVersion=<ver> /DSourceDir=<staging絶対パス> /DOutputDir=<target/dist絶対パス> packaging/windows/gemelli.iss
   ```
   ISCC は相対パスを `.iss` のディレクトリ基準で解決するため、**絶対パスを渡す** (bucatini で確認済みの罠)。ISCC の場所は `%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe` を既定とし、環境変数 `ISCC_PATH` で上書き可能にする。
   出力: `target/dist/gemelli-<ver>-windows-x64-setup.exe`

バージョンは既存の `cargo metadata` 経由 (`layout::gui_package_version`) を再利用する。

### `packaging/windows/gemelli.iss` (新規)

bucatini の `bucatini.iss` を踏襲し、以下を変更:

- `AppId` — 新規 GUID を採番 (アップグレード/アンインストールの同一性キー)
- `MyAppName` = `Gemelli`, `MyAppPublisher` = `naporin0624`, `MyAppExeName` = `gemelli-gui.exe`
- `[Files]` — `gemelli.exe` / `gemelli-gui.exe` / `icon.ico` / `README.md` / `THIRD-PARTY-NOTICES` (README.ja.md は存在しないため入れない)
- `[Languages]` 英語 + 日本語、`[Tasks]` オプションのデスクトップアイコン、`[Icons]` Start Menu + Uninstall、`[Run]` インストール後起動 (skipifsilent)
- `OutputBaseFilename=Gemelli-{#MyAppVersion}-windows-x64-setup` ではなく **`gemelli-` 小文字始まり**にする (既存 macOS 成果物 `gemelli-<ver>-macos-universal.dmg` と命名を揃える)

### `crates/gui/assets/icon.ico` (新規、コミット)

`scripts/gen-icon.py` に `.ico` 出力を追加 (Pillow は複数サイズ埋め込み `.ico` を直接書ける: 16/24/32/48/64/128/256px)。ローカルで一度実行して生成物をコミットする。

### テスト (t-wada TDD)

純関数として切り出してユニットテストする (macOS 上でも実行可能):

- staging レイアウト (コピー元→コピー先のペア列挙)
- 成果物命名 (`windows_zip_name(ver)`, `windows_setup_name(ver)`)
- ISCC 引数組み立て (`iscc_args(ver, source_dir, output_dir, iss_path)`)

プロセス起動・ファイルコピーは既存 `cmd.rs` / `run_checked` パターンに従う。

## 4. リリース workflow 統合

`release-please.yml` と `release.yml` を単一の `release.yml` に統合する (旧 `release-please.yml` は削除):

```
push to main
  └─ release-please job
       ├─ outputs: release_created, tag_name
       ├─(release_created == 'true')─▶ build-macos   (macos-15)
       └─(release_created == 'true')─▶ build-windows (windows-latest)
```

- **release-please job** — 既存 release-please.yml の中身をそのまま移設 (`googleapis/release-please-action@v5`、`config-file` / `manifest-file` 指定)。`outputs` に `release_created` / `tag_name` を追加。
- **build-macos job** — 既存 release.yml のステップをそのまま移設 (checkout + submodules → rust-toolchain@1.96.1 → universal2 targets → rust-cache → Syphon.framework ビルド → fetch-fonts → `cargo xtask dist` → `gh release upload`)。
- **build-windows job** (新規) — checkout (submodules 込み) → rust-toolchain@1.96.1 → rust-cache → `scripts/fetch-fonts.sh` → `scripts/fetch-spout.sh` → `cargo build --release --workspace` → `choco install innosetup` → `cargo xtask dist` → `gh release upload` で `.zip` + `-setup.exe` を添付。
- **手動再実行** — `workflow_dispatch` (tag 入力) を残す。ビルドジョブの `if` は `release_created == 'true' || github.event_name == 'workflow_dispatch'`、`TAG` は現行 release.yml と同じ三項パターンで解決する。workflow_dispatch 時は release-please job の成功に依存しないよう `needs` の結果を `!cancelled()` 系で緩めるのではなく、release-please job 自体は常に走る (push トリガーでなくとも action は no-op で成功する) ため `needs: [release-please]` のままでよい。
- **課金** — Windows runner はリリース作成時 (release PR マージ時) と手動 dispatch 時のみ消費。PR CI の label-gate 運用 (ci-windows) はそのまま。

## 5. README 更新

`README.md` に **Install (prebuilt)** セクションを追加 (Setup/Build セクションより前):

- **macOS** — `gemelli-<ver>-macos-universal.dmg` をダウンロードし `gemelli.app` を Applications にドラッグ。未署名のため初回は右クリック → 開く → 開く。
- **Windows** — `gemelli-<ver>-windows-x64-setup.exe` を実行 (GUI + CLI + Start Menu ショートカット)。未署名のため SmartScreen は「詳細情報 → 実行」で通す。
- インストール不要派向けに `.tar.gz` / `.zip` も添付されている旨を記載。
- 外部ランタイム要件なし (Spout/Syphon とも同梱・静的リンク) — bucatini の NDI Runtime 節に相当するものは書かない。

## 6. 検証計画

1. ユニットテスト: `cargo test --workspace` (命名・引数組み立て・レイアウトの純関数)。
2. lint: `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings`。
3. マージ後、`workflow_dispatch` で既存タグに対して release workflow を手動実行し、Release に `dmg` / `tar.gz` / `zip` / `-setup.exe` の 4 点が揃うことを確認。
4. Windows 実機 (または手元の VM) で `-setup.exe` を実行し、Start Menu から GUI が起動すること・アンインストールが機能することを確認 (ユーザー側タスク)。

## 7. スコープ外

- コード署名 (macOS notarization / Windows Authenticode) — 未署名配布のまま。
- winget / Homebrew cask などのパッケージマネージャ配布。
- 自動アップデート機構。
