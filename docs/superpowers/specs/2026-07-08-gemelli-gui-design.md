# gemelli GUI (Phase 2) 設計

日付: 2026-07-08
ステータス: ステークホルダー承認済み(会話内で承認)
前提: Phase 1 (core + CLI) 完了・main merge 済み(`docs/superpowers/specs/2026-07-07-webcam-sharedtexture-design.md` / plan `2026-07-07-core-cli-implementation.md`)

## 目的

gemelli-core を使う egui GUI(crates/gui = gemelli-gui)。webcam プレビューを見ながら
crop / rotate / flip / scale を調整し、Syphon publish を制御する。

## 決定事項(ステークホルダー確認済み)

- レイアウト: **サイドバー型**(左: 操作パネル、右: 大プレビュー + ステータスバー)
- プレビュー: **変換後のみ**(= Syphon 出力と同一)。ただし crop 編集モード中のみ生フレーム + 矩形オーバーレイ
- crop UI: **プレビュー上ドラッグ編集** + W/H/X/Y 数値欄(相互同期)
- UI 言語: **英語**(egui デフォルトフォントで完結)
- スレッドモデル: **capture 専用スレッド + latest-frame 方式**(Phase 1 spec のギャップ解消)

## アーキテクチャ

```
[capture thread]                          [GUI thread (eframe/egui)]
 loop:                                     60fps repaint:
   NokhwaSource::next_frame()                latest_preview を読む
   config snapshot (ArcSwap)                 → BGRA→RGBA swizzle → TextureHandle 更新
   transform::apply()                        sidebar で config 編集
   SyphonPublisher::publish()                → ArcSwap<TransformConfig> store
   latest_preview へ store ──────────→       start/stop publish、device 切替
   stop AtomicBool チェック                   → capture thread 停止→再起動
   error → mpsc channel ────────────→       エラーバナー表示(消費は GUI の 1 箇所のみ)
```

- publish は capture スレッド側(カメラ fps がペース。SyphonPublisher は Send 済み)
- config: `arc_swap::ArcSwap<TransformConfig>`、フレーム: `Mutex<Option<Frame>>`(latest のみ、フレーム落ち許容)
- gemelli-core は原則無変更。GUI 用 capture ループ(preview 出力付き)は gemelli-gui 内に実装
- crop 編集モード: 生フレーム表示 + 矩形ドラッグ/リサイズ。crop はチェーン先頭のため矩形は生フレーム座標に 1:1 対応。数値欄と双方向同期。矩形は境界チェック(CropRect の検証と同じ制約)をドラッグ中に clamp で満たす

## UI 仕様

```
┌──────────────┬────────────────────────┐
│ Device       │                        │
│ [FaceTime ▾] │      preview           │
│ Rotate       │   (変換適用後 /        │
│ (0)(90)      │    crop 編集中は生+矩形)│
│ (180)(270)   │                        │
│ Flip [h][v]  │                        │
│ Crop [Edit]  │                        │
│  W H X Y     ├────────────────────────┤
│ Scale …      │ 1920x1080→960x540 60fps│
│ server:      │ ● publishing    [Stop] │
│ [gemelli   ] │                        │
└──────────────┴────────────────────────┘
```

- Rotate: 4 値のセグメント選択。Flip: h / v 独立トグル(両方 ON = Both)。Scale: 倍率スライダー + WxH 直接入力の切替
- server name 変更・device 切替は capture thread 再起動を伴う(publish 中なら再 publish)
- ステータスバー: 入力→出力解像度、実測 fps(1 秒窓)、publish 状態
- **色設計**: WCAG 2.1 AA 準拠の color token(通常テキスト コントラスト比 ≥4.5:1、UI コンポーネント/状態表示 ≥3:1)を定数モジュールとして定義し egui `Visuals` に適用。publishing 状態は色 + 文言(● publishing / ○ stopped)の冗長表現
- エラー(カメラ喪失・publish 失敗など): 上部バナーに表示、Dismiss 可能。expected failure で panic しない

## テスト計画(t-wada TDD)

GUI の egui ウィジェット自体は目視、ロジックは全て純関数に切り出して TDD:
1. **プレビュー変換**: BGRA→RGBA swizzle 純関数(画素検証)
2. **crop ドラッグ**: 画面座標↔フレーム座標変換、矩形の clamp(境界・最小サイズ)純関数
3. **fps 計測**: 1 秒窓カウンタ純関数
4. **capture ループ**: FakeSource/CollectingPublisher(Phase 1 の double)で latest-frame 更新・stop・エラー channel 送出を検証
5. **色 token**: コントラスト比計算のテスト(4.5:1 / 3:1 を満たすことを数値で assert)
6. 手動: 実カメラ + Syphon Recorder での E2E チェックリスト

## スコープ外(Phase 3+)

設定永続化 / Spout (Windows) / GPU 変換 / 複数サーバー / 録画

## 依存追加

gemelli-gui: eframe/egui、arc-swap。build.rs(`DEP_SYPHON_BRIDGE_RPATH` 読み、Phase 1 の cli 実装と同型)
