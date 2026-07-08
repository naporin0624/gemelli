## やりたいこと

- webcam -> Spout/Syphon の変換を行う小さいツールを作成する
  - webcam の rotate, flip, crop, scale などの処理を引数で指定できるようにする
- 実装が終わったら lint, test 実行すること
- husky で lint, test を担保すること
- cli, gui 両方の実装を行うこと
- core module を作り、cli, gui は core module を利用すること


## project rules

- t-wada の TDD で実装すること
- PoC を作って細かく検証して進めること
- node の依存が必要な場合は pnpm を使って setup すること
- version 管理は mise に任せたい
  - mise と uv の連携を調査してから
- 実装をする前にライブラリについて知らないことがある時は context7, web で調査してから進めること。
- husky で commit 時に lint, typecheck を実行すること
- 勝手に commit しないこと
- 実装は小さいタスクに分けて実装すること。実装が終わったら difit を起動して私に review 依頼すること
- review で繰り返し受けた内容は rules, skills にすることで永続化して
  - review の内容はまず memory に記憶して繰り返し指摘されるものは skills にすること
- 実装は subagent に分けること。sonnet5 を利用して
 - sonnet5 が意図しない動作を行った時それを /superpower:writing-skills で修正するための skills を作ること


## coding rules

- あなたは実装計画、ステークホルダーである私に対して要件のブレがなくなるまで AskUserQuestion で質問することに努め、実装は subagent に任せること
- 関数は単一責任で実装すること
- 同時に命令が複数来た時は Task で優先順位をつけて subagent に実装を任せること
- コメントはコードを見てもわからないことに関して書くこと。過去の文脈に対するコメントを書くことを禁止する。



## ui rules

- WCAG2.1 AA 基準を満たすように color token を設計すること
- UI は文脈に沿った内容にすること
    - 機械的なUIの利用は徹底的に避けること
    - 伝えたい情報はどんなものでその情報に適切な UI を常に考察、模索すること
    - ASCII ダイアグラムで提案すること
    - AskUserQuestion であなたが考えたパターンを私に提示してどれがいいか提案すること


## ref repository

ここに書かれているリポジトリには gh コマンドで参照し、既存実装を参照する前に ref repository の内容を先に探すこと
issue, .claude/rules, skills やコードが参考になる。

- Spout/Syphon の rust 実装に関しては https://github.com/naporin0624/electron-texture-bridge が参考になるのでよく見ること
- webcam の映像を回転させる cli option については https://github.com/naporin0624/ravioli を参考にすること


## resources rules
- mockup 用の画像が必要な時は Codex CLI の組み込み画像生成スキル `$imagegen` を使うこと
- 使い方:
    - headless（推奨）: `codex exec "<生成したい画像の説明> $imagegen"`
    - 対話: `codex "<説明> $imagegen"`
    - 参照画像を渡す: `codex -i ref.png "<説明> $imagegen"` / `codex --image a.png,b.jpg "<説明>"`
- モデルは `gpt-image-2`。生成画像は `~/.codex/generated_images/`（`$CODEX_HOME/generated_images/`）に保存される
- 出力先パス・サイズ・品質・透過・枚数は プロンプト内に自然言語で指定する（`--out`/`--size` 等のフラグは不要）
- 用途: アイコン・バナー・イラスト・スプライト・プレースホルダ等のモックアップ素材
