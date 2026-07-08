# webcam キャプチャ・スループット改善 設計

日付: 2026-07-09
ステータス: 会話内で方針承認済み(計測に基づく)
ブランチ: `perf/webcam-pipeline`(base: `feat/spout-publisher`)

## 背景・計測結果

CLI で publish すると 1〜5fps しか出ない(mac/Syphon は 30fps)。この機(Windows +
Logitech StreamCam)で CLI に一時計装を入れて 1 フレームの内訳を実測した:

```
交渉フォーマット: 2304x1296 YUYV(非圧縮)  ← AbsoluteHighestResolution が選択
  camera.frame()   ~3.6ms   受信は速い(ボトルネックでない)
  YUYV→RGB decode  ~592ms   ← 支配的(nokhwa の YUYV デコードが激遅)
  RGB→BGRA 変換     ~222ms   ← 2番目(自前 rgb_to_bgra の素朴ループ)
  transform         ~7ms
  publish(SendImage) ~2ms   ← GPU アップロードは激安(unified/shared memory 非対称は無関係)
  → ~1.2fps
```

MJPEG 1920x1080 を強制する PoC を実測したところ:

```
  decode  592ms → ~38ms   (mozjpeg 経由で 15 倍速)
  rgb2bgra ~157ms          ← MJPEG 化後は「ここ」が最大のボトルネック(1080p=200万画素の素朴ループ)
  → ~5fps
```

### 結論(データに基づく)

- 真のボトルネックは **CPU の色処理**。GPU アップロード(publish 2ms)でもカメラ受信でもない。
- **効く修正は2つ**: ① カメラフォーマットを MJPEG に(decode 592→38ms)、② 色変換の高速化
  (rgb_to_bgra 157ms → 目標数 ms)。
- **マルチスレッド化は本タスクの対象外**。①② 未対応では律速が変わらないことを実証済み。
  ①② 完了後の上積み/レイテンシ改善として別タスクにする。

### 目標

- CLI で **30fps 以上**(mac/Syphon 相当)。1080p を基本とし、高 fps を優先。
- gemelli-core の変更のみ。CLI/GUI/spout/syphon のインターフェースは不変(両者は core 経由で恩恵)。

## 決定事項(承認済み)

- **高 fps 優先**。フォーマット選択のデフォルトは MJPEG + 高フレームレート。
- 本タスクのスコープは ①フォーマット選択 と ②色変換高速化 のみ。マルチスレッドは含めない。
- `gemelli-core` のパブリック API(`NokhwaSource::open`, `CaptureSource::next_frame`, `Frame`)は
  シグネチャ不変。内部実装のみ変更。

## アーキテクチャ / 変更点(すべて crates/core)

### ① フォーマット選択の刷新(`capture.rs`)

現状 `format_candidates` は `AbsoluteHighestResolution`(→ YUYV 巨大モード)を選ぶ。これを
**MJPEG 優先・高 fps 優先** に変える。

- nokhwa の `RequestedFormatType::Closest` は距離が近ければ非 MJPEG(YUYV)を選びうるため、
  **カメラの対応フォーマットを列挙して MJPEG を明示選択**する方式にする:
  1. `Camera::compatible_camera_formats()`(または開けた後に列挙)で候補を取得。
  2. `FrameFormat::MJPEG` のみに絞り、**(frame_rate 降順, resolution 降順)** で最良を選ぶ。
     ただし解像度は上限(既定 1920×1080)を超えない範囲で最大 fps を優先。
  3. その CameraFormat を `RequestedFormatType::Exact` で要求。
- フォールバック順(いずれかが失敗/列挙不可のとき):
  1. `Closest(CameraFormat::new(1920x1080, MJPEG, 60))`
  2. `Closest(CameraFormat::new(1280x720,  MJPEG, 60))`
  3. `AbsoluteHighestFrameRate`(フォーマット不問・最高 fps)
  4. `AbsoluteHighestResolution`(現行=最終手段)
- `requested_fps: Some(fps)` が渡された場合は MJPEG + その fps を優先(既存 `--fps` 経路を尊重)。
- 列挙・選択ロジックは純粋関数に切り出してユニットテスト可能にする
  (例 `select_mjpeg_format(formats: &[CameraFormat], fps: Option<u32>, max_res) -> Option<CameraFormat>`)。
- **注意**: nokhwa の `compatible_camera_formats` 等の正確な API は実装前に context7/ソースで確認する
  (バージョン 0.10.11)。列挙 API が使えない場合はフォールバックのみで実装し、その旨コメントする。

### ② 色変換の高速化(`capture.rs::rgb_to_bgra`)

現状は `for pixel in rgb.chunks_exact(3) { bgra.extend_from_slice(&[b,g,r,255]); }`。1 画素ごとに
4 バイト配列を作って push しており ~75ns/画素。

- 出力バッファを `vec![0u8; pixel_count*4]` で確保し、`chunks_exact(3)` の入力と
  `chunks_exact_mut(4)` の出力を `zip` して**直接インデックス書き込み**する
  (境界チェックが消え LLVM が自動ベクトル化しやすい)。alpha は 255 固定。
- 公開挙動(RGB→BGRA + 不透明 alpha)は不変。既存テスト
  `rgb_to_bgra_swizzles_channels_and_adds_opaque_alpha` を満たしつつ、
  端数・サイズ 0・大サイズのテストを追加。
- 効果はマイクロベンチ(`#[ignore]` の計測テスト、または criterion 不使用の簡易計測)で
  before/after を記録。目標: 1080p で 157ms → 一桁 ms。

## データフロー(不変)

```
NokhwaSource::next_frame():
  camera.frame()  → MJPEG buffer        (① で MJPEG になる)
  decode_image::<RgbFormat>() → RGB     (mozjpeg 高速)
  rgb_to_bgra() → BGRA                  (② で高速化)
  Frame::new(w, h, bgra)
→ pipeline: transform::apply → publisher.publish
```

## テスト方針 (t-wada TDD)

- **② 色変換**(host 非依存・純粋関数): 既存の swizzle テストに加え、複数画素・0 サイズ・
  非正方サイズで RGB→BGRA + alpha=255 を検証。実装前に失敗するテストを追加 → 実装 → green。
- **① フォーマット選択ロジック**(純粋関数): `select_mjpeg_format` に対し、
  - MJPEG が複数あるとき (fps 降順→resolution 降順) で最良を選ぶ
  - MJPEG が無いとき None(→ フォールバックへ)
  - fps 指定時はその fps に最も近い MJPEG を選ぶ
  - 解像度上限を超える MJPEG は除外
  をユニットテスト(CameraFormat を直接構築、カメラ不要)。
- **実機計測**(`#[ignore]`/手動): この機 + StreamCam で CLI を起動し、
  - 交渉フォーマットが MJPEG になること
  - 1 フレーム内訳(decode/convert)と実 fps が目標(≥30fps)に達すること
  を再計測。Spout 受信アプリで映像・色・向きが正しいことも確認。

## 検証(実機 E2E, Windows + MSVC2019)

1. `cargo test -p gemelli-core`(②①のユニット)green。
2. `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --check`。
3. worktree に `scripts/fetch-spout.sh` / `fetch-fonts.sh` を流して full build。
4. `gemelli 0 --server-name gemelli` を起動 → 交渉フォーマット MJPEG・fps≥30 を確認、
   Spout 受信で映像確認。改善前(YUYV/~5fps)との比較を記録。

## スコープ外(別タスク)

- capture/publish のマルチスレッド分離 + drain-to-latest(スループット上積み・レイテンシ/鮮度改善)。
- GPU 側での色変換/転送、ゼロコピー経路。
- `--resolution`/`--format` などの CLI/GUI 露出(将来の拡張余地として設計に含めるが実装しない)。

## 影響範囲

- 変更: `crates/core/src/capture.rs`(`format_candidates`/`open` のフォーマット選択、`rgb_to_bgra`、
  新規純粋関数 `select_mjpeg_format`)。
- 不変: `frame.rs` / `pipeline.rs` / `transform/*` / CLI / GUI / spout / syphon。
