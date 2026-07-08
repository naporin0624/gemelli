# 配布準備(licenses / Cannelloni retheme / appmenu About)— Design

GUI を配布物として仕上げるための 3 本柱:

1. サードパーティライセンスの自動収集・アプリ内表示・配布同梱
2. UI 全体を Cannelloni の design system(neo-brutalist / "terminal-print")に retheme
3. macOS メニューバー(appmenu)に About を追加 — app name / version / author / build id

## Goals

- Rust crate 依存(数百件)+ 非 crate 依存(Syphon Framework, LINE Seed JP)のライセンスを
  網羅して、アプリ内ウィンドウと `THIRD-PARTY-NOTICES` テキストの両方に単一ソースから出力する。
- copyleft / 未知ライセンスの混入を CI で機械的に検出して fail させる(permissive のみ許可)。
- gemelli GUI の見た目を Cannelloni palette に全面移行し、WCAG 2.1 AA をテストで再証明する。
- macOS ネイティブメニュー(gemelli ▸ About / Help ▸ Open Source Licenses…)を muda で提供する。

## Non-Goals

- Windows / Spout GUI 対応(muda 自体はクロスプラットフォームなので将来の下地にはなる)。
- ライセンス全文の crate ごと個別ファイル配布(構造化 JSON + 結合テキストを採用)。
- ビルド時のライセンス自動生成(手動コマンド + コミットを採用 — Cannelloni と同判断)。
- About の自前 egui ウィンドウ化(ネイティブ About パネルを採用)。

## Decisions(確定済み)

| 論点 | 決定 |
| --- | --- |
| retheme 範囲 | GUI 全体を Cannelloni palette へ移行(新規画面だけでなく既存 UI も) |
| appmenu | muda でネイティブメニューバー(macOS: `init_for_nsapp()`) |
| About 表示面 | muda `PredefinedMenuItem::about` + `AboutMetadata`(macOS 標準 About パネル) |
| licenses 表示 | アプリ内専用ウィンドウ(egui deferred viewport)+ `THIRD-PARTY-NOTICES` 同梱 |
| license 収集 | `cargo-bundle-licenses --format json` + 手書き appendix JSON を merge |
| ポリシー検査 | cargo-deny(allowlist 方式)。CI hard fail + 手動コマンド。コミットフックには入れない |
| build id | vergen 9 系(`vergen-gix`)で git short SHA + build date を build.rs から埋め込み |
| 生成タイミング | 手動 `cargo xtask gen-licenses` → 生成物をコミット。CI で鮮度検査(`--check`) |

## 1. Cannelloni retheme(`crates/gui/src/theme.rs`)

Cannelloni `panda.config.ts` の primitive tokens(oklch)を sRGB `Color32` に換算して置き換える。
換算値とコントラスト実測値(本リポジトリの `contrast_ratio` と同式で算出済み):

| gemelli token | 新値 | oklch ソース | コントラスト証明(実測) |
| --- | --- | --- | --- |
| `BG_BASE` | `#121212` | dark.canvas 0.180 0 0 | — |
| `BG_PANEL` | `#1c1c1c` | dark.subtle 0.225 0 0 | — |
| `BG_MUTED`(新設) | `#262626` | dark.muted 0.270 0 0 | 展開行の背景など |
| `TEXT_PRIMARY` | `#eeeff2` | gray.1 0.952 0.004 265 | 16.29 / base, 14.82 / panel(≥4.5) |
| `TEXT_MUTED` | `#c9ccd1` | gray.6 0.845 0.008 265 | 11.63 / base, 10.58 / panel(≥4.5) |
| `TEXT_SUBTLE`(旧 `ACCENT_IDLE`) | `#9a9ea7` | gray.8 0.700 0.013 265 | 6.98 / base, 6.35 / panel(≥4.5、"○ stopped" ラベル) |
| `ACCENT`(旧 `ACCENT_PUBLISH`) | `#3996ff` | neon.blue 0.700 0.235 260 | 6.23 / base, 5.66 / panel(≥4.5) |
| `ACCENT_HOVER`(新設) | `#2785ff` | neon.blueHover 0.650 0.235 260 | hover 塗りにのみ使用 |
| `ACCENT_ALT`(新設・slider fill) | `#34dde5` | neon.cyan 0.820 0.130 200 | 11.26 / base(≥3.0、1.4.11) |
| `SELECTION_BG` | `#3996ff` | accent.solid | 反転式: 上に載る文字は `BG_BASE` 色(fg.onSolid)。6.23(≥4.5) |
| `DANGER` | `#ff2939` | red.text **L 0.650→0.660 補正** | 5.02 / base, 4.57 / panel(≥4.5) |
| `BORDER`(新設) | `#696969` | dark.border 0.520 0 0 | 3.41 / base, 3.10 / panel(≥3.0、1.4.11) |
| `BORDER_SUBTLE`(新設) | `#424242` | dark.borderSubtle 0.380 0 0 | 区切り線(非情報伝達)用 |
| `CROP_OVERLAY` | 白 + 黒縁(変更なし) | — | 映像上のため二重ストローク方式を継続 |

**Cannelloni からの意図的な逸脱は DANGER の 1 点のみ**: 原典 `oklch(0.650 0.250 25)` は
`BG_PANEL` 上で 4.497:1 と AA(4.5:1)を僅差で下回る(Cannelloni は error 表示を canvas 上に
置くため成立している)。L を 0.660 に上げて panel 上でも 4.57:1 を確保する。

スタイル規則(`apply_theme` で `Visuals` / `Style` に反映):

- **sharp corners**: すべての `corner_radius = 0`(widgets / window / menu)。
- **2px ink border**: interactive widget の `bg_stroke` を `BORDER` 2.0px に。
- **選択状態は反転式**: `selection.bg_fill = ACCENT`、選択中の文字・ストロークは `BG_BASE` 色。
  現行の「深緑塗り + 白文字」を置き換える(egui 0.35 は `selection.stroke` が選択中の
  fg_stroke になる — 既知の観測 29278 に従う)。
- publish 状態表示は green を廃止し `ACCENT`(neon-blue)+ 「● publishing」ラベル併記を維持
  (WCAG 1.4.1)。
- フォントは LINE Seed JP のままで Cannelloni と一致済み(変更なし)。

既存の contrast proof テストは新 token で全面書き直し(上表の全ペアをアサート)。
承認済みモックアップ: <https://claude.ai/code/artifact/8ef9d12a-3cd3-4c80-9b69-9fdd4d2dcbc1>

## 2. appmenu + About(新規 `crates/gui/src/menu.rs`)

- muda(0.17 系)でメニューを構築し、macOS では `Menu::init_for_nsapp()` を
  eframe 起動後(`GemelliApp::new` 内、`cfg(target_os = "macos")`)に呼ぶ。
- メニュー構成:
  - **gemelli ▸** `About gemelli` / separator / `Quit gemelli ⌘Q`(いずれも `PredefinedMenuItem`)
  - **Help ▸** `Open Source Licenses…`(カスタム `MenuItem`)
- About は `PredefinedMenuItem::about(None, Some(metadata))`。`AboutMetadata` の内容:
  - name: `gemelli`
  - version: `CARGO_PKG_VERSION`(0.1.0)
  - short_version / build id: `VERGEN_GIT_SHA`(short)
  - authors: `naporitan`
  - copyright: `© 2026 naporitan`
  - website: `https://napochaan.com`
- `AboutMetadata` を組み立てる関数は純粋関数(`about_metadata() -> AboutMetadata` 相当)にして
  単体テストで内容を検証する。
- イベント処理: `MenuEvent::receiver()`(crossbeam channel)を `update()` 冒頭で
  `try_recv` ループし、`MenuId` → `MenuAction` enum(`OpenLicenses` のみ。About/Quit は
  ネイティブ側で完結)へ変換する純粋関数を挟んでテスト可能にする。
- build id の供給: `crates/gui/build.rs` に vergen-gix の emit を追加
  (既存の Syphon rpath 処理と共存)。git 情報が取れない環境(ターボール等)では
  vergen の fallback(`VERGEN_GIT_SHA` idempotent 出力)に任せ、ビルドは失敗させない。

## 3. Licenses ウィンドウ(新規 `crates/gui/src/licenses.rs`)

Cannelloni の colophon(third-party-licenses-window spec)の情報設計を egui に移植する。

- **表示面**: egui deferred viewport(独立ウィンドウ)。`MenuAction::OpenLicenses` で
  open フラグを立て、既に開いていれば focus。
- **データ**: `include_str!("../assets/third-party-licenses.json")` を初回オープン時に
  serde で parse。失敗時は panic せずウィンドウ内にエラーメッセージを表示
  (workspace lint が unwrap/expect を deny しているため必然的に Result 経路)。
- **スキーマ**(Cannelloni と同一):

  ```rust
  enum LicenseCategory { Library, Font, Native }
  struct LicenseEntry {
      name: String,
      version: Option<String>, // font / native は None
      license: String,         // SPDX 表記
      text: String,            // 全文
      homepage: Option<String>,
      category: LicenseCategory,
  }
  ```

- **UI 構成**: 上部に検索 `TextEdit` + カテゴリフィルタ(All / Library / Font / Native の
  選択式トグル)。本体は `ScrollArea` のスタックドロウ(hairline divider のみ、先頭マーカーなし)。
  行クリックで全文をその場に展開(展開行の背景は `BG_MUTED`)+ homepage リンク。
  appendix 由来(font / native)は license badge で区別。
- 検索・フィルタの絞り込みは純粋関数(`filter_entries`)にして単体テスト。

## 4. 生成パイプライン(新規 `crates/xtask`)

```text
cargo bundle-licenses --format json      licenses/appendix.json(手書き 2 件:
     (crate 依存・全文入り)               Syphon BSD-3 / LINE Seed JP OFL-1.1)
            │                                   │
            ▼                                   │
   cargo xtask gen-licenses  ◀──────────────────┘
      merge(同名は appendix 優先)→ category → name 安定ソート → 2 出力:
   ① crates/gui/assets/third-party-licenses.json(embed 用・コミット対象)
   ② THIRD-PARTY-NOTICES(配布同梱テキスト・コミット対象)
```

- `crates/xtask` は workspace member 外の慣例ではなく member に追加し、
  `.cargo/config.toml` に `xtask = "run --package xtask --"` alias を置く(標準 xtask パターン)。
- シェル層(subprocess 実行・ファイル IO)と純粋関数層(normalize / merge / sort / render)を
  分離し、純粋関数層を TDD する — Cannelloni の generate-licenses.ts と同じ責務分割。
- `cargo xtask gen-licenses --check`: 一時ディレクトリに再生成してコミット済み生成物と diff。
  相違があれば非ゼロ exit(「依存追加したが生成物未更新」の検出)。
- 既存の手書き `THIRD-PARTY-NOTICES`(Syphon / LINE Seed JP の 2 件)は appendix JSON に
  移設し、ファイル自体は生成物に置き換わる。
- 配布時は `.app` 同梱(bundle 手順は本設計のスコープ外だが、生成物がリポジトリルートに
  あることを前提とする)。

## 5. ポリシー検査 + CI

- `deny.toml`(リポジトリルート): `[licenses]` allowlist —
  許可は `MIT / Apache-2.0 / BSD-2-Clause / BSD-3-Clause / ISC / Zlib / Unicode-3.0` を起点に
  実依存を見て確定する。weak copyleft(MPL-2.0)以上・不明ライセンスは allowlist に載せず
  hard fail(必要になったら個別レビューのうえ明示追加)。
- `.github/workflows/license-check.yml`(この repo 初の CI workflow):
  `cargo-deny-action` + `cargo xtask gen-licenses --check` を `push` / `pull_request` で実行。
- 手動コマンド: `cargo deny check licenses`。husky コミットフックには追加しない
  (毎コミットには重い — Cannelloni と同判断)。

## エラーハンドリング方針

- 埋め込み JSON の parse 失敗 → licenses ウィンドウ内にエラー表示(アプリは落とさない)。
- vergen の git 情報取得失敗 → ビルドは継続、build id はフォールバック値。
- xtask: subprocess 失敗・JSON 不正は即エラー終了(生成は開発者操作なので fail fast)。
- muda のメニュー構築失敗(`Result` を返す API)→ 起動継続、メニューなしで動作
  (GUI 本体の機能はメニューに依存しない)。エラーは stderr へ。

## テスト戦略(t-wada TDD)

| 対象 | テスト |
| --- | --- |
| theme.rs | 新 token 全ペアの contrast proof(上表の値)、`apply_theme` の Visuals/Style 反映、corner_radius=0 / stroke 2px |
| menu.rs | `AboutMetadata` 組み立ての内容検証、`MenuId → MenuAction` 変換 |
| licenses.rs | JSON parse(正常 / 不正)、`filter_entries`(検索・カテゴリ・複合) |
| xtask 純粋関数層 | normalize / merge(appendix 優先)/ sort(安定)/ render(NOTICES 書式) |
| 統合 | コミット済み `third-party-licenses.json` が parse でき、appendix 2 件を含むことのアサート |

実装は小さいタスクに分割し、subagent(sonnet5)に委譲。各タスク完了後に lint / test /
difit review を回す(既存の開発フロー通り)。

## 参考

- Cannelloni: `docs/superpowers/specs/2026-06-25-third-party-licenses-window-design.md`、
  `src/main/menu/about-dialog/`、`panda.config.ts`(design tokens)
- cargo-bundle-licenses: <https://github.com/sstadick/cargo-bundle-licenses>
- muda: <https://docs.rs/muda>(`init_for_nsapp`, `PredefinedMenuItem::about`)
- vergen: <https://docs.rs/vergen-gix>
