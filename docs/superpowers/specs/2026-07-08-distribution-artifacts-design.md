# 配布成果物 (distribution artifacts) + アプリアイコン Design Spec

**Status:** Draft (承認待ち)
**Date:** 2026-07-08
**Phase:** 4 (distribution-prep ブランチ継続、base = Phase 3.5 の HEAD)
**Depends on:** Phase 3 (licenses / About / retheme) 完了済み。THIRD-PARTY-NOTICES と `crates/gui/assets/third-party-licenses.json` は生成済み。

## 1. 目的

gemelli の CLI (`gemelli`) と GUI (`gemelli-gui`) を **他マシンでそのまま動く配布物**にする。あわせて双子/ミラーをモチーフにしたアプリアイコンを作成し、`.app` と実行時ウィンドウ/Dock の両方に埋め込む。

確定済みステークホルダー決定 (AskUserQuestion, 2026-07-08):

| 項目 | 決定 |
| --- | --- |
| 作業ブランチ | `feature/distribution-prep` で継続 (worktree なし、既存 WIP 保持) |
| GUI 配布形式 | `.app` を同梱 framework 付きで生成 → `.dmg`。**未署名** (Gatekeeper は右クリック→開く手順を README に記載) |
| CLI 配布形式 | GitHub Release に `tar.gz` (バイナリ + Syphon.framework 同梱) |
| アイコン | 双子/ミラー + レンズ (blue #3996FF × cyan #34DDE5, charcoal #121212) |
| 自動化 | ローカル `cargo xtask` + GitHub Actions 自動リリース (release-please のタグ連携) |
| アーキ | Universal2 (arm64 + x86_64, lipo 結合) |
| bundle 実装 | xtask 自作 (外部バンドラ不使用) |

## 2. 配布の技術的制約 (なぜ単純なバイナリ配布では動かないか)

- GUI/CLI は Apple の `Syphon.framework` に**動的リンク**している (`crates/syphon/build.rs`: `cargo:rustc-link-lib=framework=Syphon`)。framework の install name は `@rpath/Syphon.framework/Versions/A/Syphon` (otool -D で確認済み)。
- 現状の rpath は `@loader_path/../../vendor` 等、リポジトリ内の `vendor/Syphon.framework` を指す (build.rs が焼き込み)。配布物には vendor/ が無いので、**framework を同梱し rpath を配布レイアウトに合わせて後付け**する必要がある。
- 後付けは `install_name_tool -add_rpath` で行う (build.rs は一切変更しない)。framework の install name が `@rpath/...` なので、rpath を「framework を格納するディレクトリ」に向ければ解決する。
- `vendor/Syphon.framework/Versions/A/Syphon` は既に universal fat (x86_64 + arm64, lipo -info で確認済み)。rustup ターゲット `aarch64-apple-darwin` / `x86_64-apple-darwin` は両方インストール済み。

## 3. アイコン設計

### 3.1 コンセプト (ステークホルダー承認: Cannelloni 視覚言語の双子ディスク)
gemelli は Cannelloni アプリ (同 author / 同 Spout・Syphon ドメイン) の双子。アイコンも同じ視覚言語を継ぐ: 青みのある深いクールブラックの squircle タイル + 柔らかく発光する角丸要素 (参照: `naporin0624/Cannelloni` の `resources/icon.png` — 2×2 パッドの1つが blue 発光)。gemelli では **分離した2つの塗り潰し円** (左 blue #3996FF = webcam in / 右 cyan #34DDE5 = shared texture out) を**重ねずに**並べ、各々に自色の soft glow を付ける。2つが双子/ミラーを象徴。円は重ねない (ステークホルダー明示)。小サイズ (Dock/menubar) でも視認性を保つ。

### 3.2 生成と成果物 (PIL 直描画、AI 不使用)
- マスター PNG は `scripts/gen-icon.py` (Pillow) が theme.rs のトークンで幾何描画 → 4x supersample → 1024×1024 に縮小 → `crates/gui/assets/icon.png`。決定的でパレットが GUI と常に一致。
- `.icns` は `scripts/gen-icon.sh` が `sips` で 16〜512px (@1x/@2x) の iconset を作り `iconutil -c icns` で生成 → `crates/gui/assets/icon.icns`。
- 両生成物はコミット (licenses JSON と同じ生成物コミット方針)。`gen-icon.sh` は Pillow + macOS `sips`/`iconutil` を要求。

### 3.3 埋め込み先 (2箇所)
1. **`.app` タイル / Finder / Dock (静的)**: `Info.plist` の `CFBundleIconFile = icon` + `Contents/Resources/icon.icns`。
2. **実行時ウィンドウ / Dock (ランタイム)**: `gui/src/main.rs` の `ViewportBuilder::with_icon(...)`。`eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))` を使用。`image`(png feature) は eframe の非オプション依存なので**新規依存不要**。CLI にアイコンは不要。

## 4. `.app` バンドルレイアウト

```
gemelli.app/
  Contents/
    Info.plist                # CFBundle* + NSCameraUsageDescription + LSMinimumSystemVersion
    MacOS/
      gemelli-gui             # universal2 バイナリ (lipo 結合済み)
    Frameworks/
      Syphon.framework/       # vendor からコピー (universal fat)
    Resources/
      icon.icns
      THIRD-PARTY-NOTICES
```

- バイナリへ `install_name_tool -add_rpath @executable_path/../Frameworks gemelli-gui` を実行。
- **`NSCameraUsageDescription` 必須**: webcam アクセス時に macOS が要求。未設定だとカメラ拒否/クラッシュ。文言例: "gemelli shares your camera feed as a Syphon texture."
- `Info.plist` の主キー: `CFBundleName=gemelli`, `CFBundleDisplayName=gemelli`, `CFBundleIdentifier=com.naporin0624.gemelli`, `CFBundleExecutable=gemelli-gui`, `CFBundleIconFile=icon`, `CFBundleShortVersionString` (= Cargo version), `CFBundleVersion` (= 同左 or build id), `CFBundlePackageType=APPL`, `LSMinimumSystemVersion` (要確認、暫定 11.0), `NSHighResolutionCapable=true`, `NSCameraUsageDescription`。

## 5. CLI tarball レイアウト

```
gemelli-<version>-macos-universal/
  gemelli                 # universal2 バイナリ
  Syphon.framework/       # vendor からコピー
  THIRD-PARTY-NOTICES
  README.txt              # 起動手順 + Gatekeeper 注意
```

- `install_name_tool -add_rpath @executable_path gemelli` (framework がバイナリと同階層なので rpath = 実行ファイルのディレクトリ)。
- `tar czf gemelli-<version>-macos-universal.tar.gz ...`。

## 6. xtask コマンド設計

`cargo xtask` に以下を追加 (既存 `gen-licenses` と同じ crate、pure-function + shell 層分離、no unwrap/expect/as を維持):

| コマンド | 役割 |
| --- | --- |
| `cargo xtask bundle` | universal2 の gui バイナリをビルド → `.app` を `target/dist/gemelli.app` に生成 |
| `cargo xtask dist` | `bundle` を実行し `.dmg` を生成、さらに CLI universal2 を `.tar.gz` 化。すべて `target/dist/` へ |

- **pure 層** (TDD 対象、副作用なし): Info.plist XML 文字列生成 (`PlistFields` → `String`)、バンドル/tarball のパス計算、`install_name_tool` / `lipo` / `hdiutil` の引数ベクタ構築、tarball ディレクトリ名生成。
- **shell 層**: `cargo build --target ...` の起動、`lipo -create`、ファイルコピー、`install_name_tool`、`sips`/`iconutil` は不要 (icon は事前生成コミット済み)、`hdiutil create`、`tar`。
- universal2 ビルド: `cargo build --release -p gemelli-gui --target aarch64-apple-darwin` と `--target x86_64-apple-darwin` を実行し `lipo -create -output` で結合。CLI も同様 (`-p gemelli-cli`, bin 名 `gemelli`)。

## 7. CI 自動リリース

`.github/workflows/release.yml` を新規追加:
- トリガ: `on: release: types: [published]` (release-please がリリース発行時に発火)。
- runner: `macos-15` (ci.yml と同じ)。submodule recursive checkout → Syphon.framework ビルド (ci.yml と同じ手順) → `./scripts/fetch-fonts.sh` → `cargo xtask dist`。
- `target/dist/*.dmg` と `target/dist/*.tar.gz` を `gh release upload ${{ github.event.release.tag_name }}` でアップロード。
- 既存 `ci.yml` / `license-check.yml` / `release-please.yml` は変更しない。

## 8. ドキュメント

README に「Install / 配布物」節を追加:
- `.dmg` から `.app` を Applications へ。未署名のため初回は右クリック→開く (または `xattr -dr com.apple.quarantine`)。
- CLI: tarball 展開後 `./gemelli --help`。同じく quarantine 解除手順。
- `cargo xtask dist` によるローカルビルド手順。

## 9. 非目標 (この Phase では扱わない)

- コード署名 / 公証 (Apple Developer ID)。将来 `xtask` に `--sign` オプションを足せる余地は残すが本 Phase では未署名。
- Homebrew formula / cask。
- Windows (Spout) 配布 — 現状 macOS/Syphon のみ。
- crates.io publish (framework ローカルビルド依存のため不適)。

## 10. 未確定 → 実装時に検証すること

- `eframe::icon_data` モジュールと `from_png_bytes` の公開可視性 (registry source で pub 確認済み、実装時に use パスを最終確認)。
- `LSMinimumSystemVersion` の適正値 (Syphon.framework / Metal 要件から確定)。
- `.dmg` を `hdiutil create -srcfolder` で作る際のボリューム名と Applications シンボリックリンク要否 (最小構成では srcfolder 方式で可)。
- universal2 クロスビルドで `cc` (syphon_bridge.mm) が両アーキを正しくコンパイルするか (両ターゲット導入済みなので想定内、実ビルドで確認)。
