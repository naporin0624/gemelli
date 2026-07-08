# 縦長 UI + widget fidelity(Phase 3.5)— Design

Phase 3 の retheme は color token の移植に留まり、承認済みモックアップが示した
「デザインされた widget」が未実装だった(segmented control ではなく折り返す小さな
selectable label、極小の Start ボタン、平文の大きな見出し等)。ステークホルダー確認の
うえ、縦長レイアウト化と同時にこの fidelity ギャップを解消する。

承認済みモックアップ: <https://claude.ai/code/artifact/574cabb7-0833-4d08-a85c-fa6f8c197deb> §3 案 B

## Decisions(確定済み)

| 論点 | 決定 |
| --- | --- |
| レイアウト | **案 B: CONTROLS TOP** — 操作群が上、プレビューが残り全高、最下部ステータスバー |
| ウィンドウ | 初期 400×860、最小 360×640(main.rs の ViewportBuilder) |
| スコープ | 縦長化 + fidelity 一式を同時に実施 |
| ブランチ | feature/distribution-prep に続けて積む |

## レイアウト(案 B)

```
┌─ gemelli ────────────┐ 400×860
│ DEVICE   [FaceTime HD カメラ ▾][⟳]│ ← 眉ラベル + 全幅フィールド
│ ROTATE   [ 0° │ 90° │ 180° │ 270° ]│ ← segmented(全幅・等分)
│ FLIP     [ none │ H │ V │ H+V ]   │ ← segmented
│ CROP     [ off │ edit… ]          │ ← edit… 選択で数値行を展開
│ SCALE    [ off │ factor │ W×H ]   │ ← 選択に応じ値入力行を展開
│ SERVER   [gemelli________________]│
│ [      START PUBLISHING        ]  │ ← 全幅 solid ACCENT + 暗色文字
├──────────────────────┤
│            preview               │ ← 残り全高。16:9 letterbox +
│      (crop ドラッグは現行維持)      │    クロップ矩形ドラッグ現行機能
├──────────────────────┤
│ ● PUBLISHING  gemelli   896×512  │ ← 状態 + server 名 + 出力解像度
└──────────────────────┘
```

## Widget 仕様(新規 `crates/gui/src/widgets.rs`)

- **眉ラベル** `group_label(ui, "DEVICE")`: uppercase、小サイズ(≒11px)、`TEXT_SUBTLE`。
  egui は letter-spacing 非対応のため大文字 + 小サイズ + subtle 色で代替する。
- **segmented control** `segmented(ui, id, &mut selected, &[labels])`:
  利用可能幅を等分した n セルを一体で描画。外周 2px `BORDER`、セル間 2px 区切り、
  選択セルは `ACCENT` 塗り + `BG_BASE` 文字(反転式)、非選択セルは `BG_PANEL` 塗り +
  `TEXT_MUTED` 文字。クリックで選択切替、`Response` を返す。corner radius 0。
  セル矩形計算(等分 + 端数処理)とクリック位置→セル index の変換は純粋関数にして
  単体テスト。
- **全幅アクションボタン** `action_button(ui, "START PUBLISHING") -> Response`:
  全幅 × 44px(targetComfortable)。solid `ACCENT` 塗り + `BG_BASE` 文字 + 太字。
  publishing 中は "STOP PUBLISHING"。
- **Flip mapping**: 既存の (h, v) bool 対を segmented の 4 値
  `none / H / V / H+V` と相互変換する純粋関数(往復テスト)。

## 既存機能の維持(regression 境界)

- クロップ矩形のプレビュー上ドラッグ編集、device 切替時の refit、fps 表示、
  エラーバナー、Start/Stop のワーカー制御、メニュー(About / Licenses)は挙動不変。
- theme.rs の token・contrast proof は変更しない(消費側のみ変更)。
  `ACCENT_HOVER` はアクションボタンの hover 塗りとしてここで初消費し、
  `allow(dead_code)` を外す。segmented の非選択セル hover は `BG_MUTED`
  (ACCENT_HOVER だと選択セルの ACCENT とほぼ同色になり選択状態の識別性が壊れるため)。

## ステータスバー

`● PUBLISHING`(ACCENT)/`○ stopped`(TEXT_SUBTLE)+ server 名(TEXT_MUTED)+
出力解像度(worker の出力フレーム寸法。未取得時は非表示)+ fps(現行)。

## テスト戦略

widgets.rs のセル分割・hit 判定・flip 変換は純粋関数として TDD。描画・レイアウトは
unit test 対象外(repo 方針どおり)で、目視チェックリストに委ねる。
