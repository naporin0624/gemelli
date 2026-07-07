# web-cam-sharedtexture 設計

日付: 2026-07-07
ステータス: ステークホルダー承認済み(会話内で承認)

## 目的

webcam の映像を Spout/Syphon の shared texture として publish する小さいツール。
rotate / flip / crop / scale の変換を CLI 引数(および GUI 操作)で指定できる。

## 決定事項(ステークホルダー確認済み)

- スタック: **Pure Rust** の cargo workspace(core / cli / gui の 3 crate)。node は husky 等の dev tooling のみ
- GUI: **egui**(リアルタイムプレビューと wgpu/Metal 親和性を優先)
- プラットフォーム: **macOS(Syphon)先行**。Spout(Windows)はトレイトで抽象化だけ行い実装は後回し
- CLI 引数は個別オプション方式。適用順は **crop → rotate → flip → scale** で固定
- skills: TS 汎用 5 個 + ravioli の Python 2 個(three-word-naming / early-return-guards)の計 7 個を Rust 版に変換して `.claude/skills/` に配置
- t-wada TDD で実装。commit は Claude からは行わない(husky が lint / typecheck / test を担保)

## アーキテクチャ

```
web-cam-sharedtexture/
├── crates/
│   ├── core/    # capture → transform → publish パイプライン(lib)
│   ├── cli/     # clap ベース CLI(bin: webcam-sharedtexture)
│   └── gui/     # egui GUI(bin)
├── .claude/skills/   # Rust 版 coding skills ×7
├── mise.toml         # rust / node / pnpm
└── package.json      # husky のみ
```

### core のデータフロー

```
[capture thread (nokhwa)]
   │  RGBA/BGRA frame + timestamp
   ▼
[transform: crop → rotate → flip → scale]   # CPU 処理(ravioli と同方式)
   │  変換済み frame
   ▼
[TexturePublisher trait]
   ├── SyphonPublisher (macOS / Metal)  # electron-texture-bridge の Rust 実装を参考に移植
   └── SpoutPublisher  (Windows)       # トレイト定義のみ、実装は後続フェーズ
```

- capture は専用スレッド。最新フレームを保持する構造(latest-frame 方式)で
  レンダ/publish 側と分離し、フレーム落ちよりレイテンシを優先する
- 変換は capture 直後に適用(ravioli の camera-rotation 設計を踏襲)。
  縦向きカメラでは publish 解像度が縦長になる

### 型設計(precise-type-modeling 準拠)

```rust
enum Rotation { R0, R90, R180, R270 }      // 時計回り
enum Flip { None, Horizontal, Vertical, Both }
struct CropRect { width: u32, height: u32, x: u32, y: u32 }
enum ScaleSpec { Exact { width: u32, height: u32 }, Factor(f64) }
struct TransformConfig { crop: Option<CropRect>, rotation: Rotation, flip: Flip, scale: Option<ScaleSpec> }
```

不正値(45° 回転など)は CLI/GUI の境界でパースエラーとして reject し、
core 内は型で静的に保証する。

### エラー処理(chaining-result-combinators 準拠)

- crate ごとに `thiserror` の error enum(`CoreError` は capture / transform / publish の variant を持つ)
- core は expected failure で panic / print / exit しない。`Result` を返すのみ
- 境界での消費は各サーフェスで一度だけ:
  - CLI: `run() -> Result<(), CliError>` を `main` で一度 match → stderr + exit code
  - GUI: エラーを UI 表示に変換する箇所で一度 match

## CLI 仕様

```
webcam-sharedtexture [DEVICE_INDEX] [OPTIONS]

  --list-devices            webcam を列挙して終了
  --rotate <0|90|180|270>   時計回り回転(デフォルト 0)
  --flip <h|v|hv>           ミラー反転(デフォルトなし)
  --crop <WxH+X+Y>          クロップ(例: 1280x720+320+180)
  --scale <WxH | factor>    リサイズ(例: 960x540 または 0.5)
  --server-name <NAME>      Syphon サーバー名(デフォルト: webcam-sharedtexture)
  --fps <N>                 カメラへ要求する fps(ベストエフォート)
```

- DEVICE_INDEX 省略時: TTY なら対話選択、非 TTY ならエラーで exit code 1(ravioli 踏襲)
- 不正な引数値は clap の値パースで即 reject(exit code 2)

## GUI 仕様(egui)

- webcam プレビュー(変換適用後の映像)
- デバイス選択ドロップダウン、rotate / flip の切り替え、crop / scale の数値入力
- Syphon publish の開始/停止とサーバー名設定
- core の `TransformConfig` をそのまま共有(CLI と同一のセマンティクス)

## テスト計画(t-wada TDD)

1. **transform 単体**: 非対称な小画像(2×3px、全画素一意)で crop/rotate/flip/scale の
   全画素位置を厳密検証。rotate 90°×4 = identity 等の性質テスト
2. **パイプライン統合**: fake capture device で横長フレームを流し、変換後の寸法・内容を検証。
   fake publisher で publish されたフレームを検証
3. **CLI パース**: `--rotate 90` → config 反映、`--rotate 45` → エラー exit、`--crop` 形式パース
4. Syphon 実出力は手動検証(Syphon Recorder / Simple Client を受信側に使用)

## ツーリング

- mise: rust / node / pnpm のバージョン固定
- clippy(workspace lints): `unwrap_used` / `expect_used` / `as_conversions` を deny
  (skills の機械的に検査可能な部分は lint で担保し、skills は判断が必要な部分を担う)
- husky pre-commit: `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test`

## スコープ外

- Spout(Windows)の実装(トレイト定義のみ行う)
- GPU での変換処理(CPU で開始し、性能不足が実測されたら検討)
- カメラ向きの自動検出、設定ファイル、per-camera 設定の永続化
- 音声

## 参考

- Syphon Rust 実装: naporin0624/electron-texture-bridge(packages/native)
- CLI 変換オプション設計: naporin0624/ravioli(docs/superpowers/specs/2026-07-06-camera-rotation-design.md)
