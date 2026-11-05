# cclens-multi

Claude Code・Codex・OpenCode・pi・OMP のローカルログを共通 SQLite に集計する Rust CLI。コマンド名は `cclens-multi` で、既存の `cclens` と共存する。

本家 [lambdalisue/cclens](https://github.com/lambdalisue/cclens) の公開 fork で、Git 履歴と [MIT LICENSE](LICENSE) を保持する。本家の説明は [README.md](README.md)、拡張の契約は [multi-agent spec](docs/specs/multi-agent.md) にある。

## 導入・別 PC への移動

1. `git clone https://github.com/harunonsystem/cclens-multi.git` を実行する。実ログと集計 DB のコピーは不要。
2. clone 先で Rust stable と Cargo を用意し、`cargo install --path . --locked` を実行する。この PC では Rust 1.99.0 で検証した。
3. `~/.cargo/bin` を PATH に入れ、ソース外で `cclens-multi agent-context` を実行する。`CARGO_INSTALL_ROOT` を指定した場合はその `bin/` を PATH に入れる。
4. `cclens-multi analyze --dry-run --json` で入力先を確認し、`cclens-multi doctor --json` を実行する。

インストール時には Cargo の依存取得が必要になる場合がある。既存レポートとローカル監査にはネットワーク・API キーを使わない。任意の AI 診断だけは明示許可を受け、選択した CLI の認証と provider を使う。各 PC のログを各 PC の DB に集計する。複数 PC の DB を合算する機能は含めない。

## ワークフロー改善

`cargo install --path . --locked` で入る同じバイナリに `workflow` を追加している。
監査 → 根拠付き提案 → 1変更の試行 → 比較結果を、後から再開できる私有ディレクトリに保存する。
今回の変更は未公開。公開後は Git URL からも Cargo install できるが、crates.io 公開は行わない。

1. 実ログの記録と一致する project パスでローカル監査を作る。

   ```sh
   cclens-multi workflow audit --project /tmp/example --out /tmp/example-workflow
   cclens-multi workflow show /tmp/example-workflow --json
   ```

   既定は直近14日・新しい親5セッション・候補40イベント。完了済みタスク数とは同一視しない。
   元の本文は既定で読まず、`--include-text` で短い抜粋を含める。
   現在の設定との照合には `--context /path/to/current/SKILL.md` を明示する。秘密情報は渡さない。

2. `workflow brief RUN --json` で入力を確認し、既存エージェントが返した JSON を取り込む。

   ```sh
   cclens-multi workflow advise /tmp/example-workflow --response /tmp/advice.json
   ```

   認証済みの pi / Claude CLI に直接診断させる場合だけ、provider 入力を許可する。

   ```sh
   cclens-multi workflow advise /tmp/example-workflow \
     --advisor pi --advisor-model provider/model --allow-model-input --json
   ```

   Claude は `--advisor claude` と対応モデル名を指定する。
   本文・明示 context・パスが選択した provider に送られる。tools・拡張・周辺 context の自動読み込みを無効にする。
   不明な根拠参照や形式外の応答は保存しない。設定は自動編集しない。

3. 現在も必要な変更と確認してから、試行を1件記録する。

   ```sh
   cclens-multi workflow trial start /tmp/example-workflow --finding 0 \
     --baseline /tmp/before.json --change "use the existing verification step" \
     --confirm-current
   ```

   対策追加済み・解消済みの提案は新規試行にしない。編集は既存の SSOT・権限ガードに従って行う。

4. 同種作業の観測を比較する。

   ```sh
   cclens-multi workflow trial evaluate /tmp/example-workflow --after /tmp/after.json
   ```

   before / after は次の JSON。未取得は 0 でなく null にする。

   ```json
   {"workload":"same-fixed-tasks","model":"provider/model","tasks":3,
    "quality_passed":true,"human_interventions":2,"rework":0,"elapsed_seconds":600}
   ```

   結果は adopt / revert / hold。品質悪化は速度改善に優先し、モデル差・欠測・比較不足は保留する。
   観測値はユーザー入力であり、CLI が独立検証した測定値ではない。設定変更や撤回も自動では行わない。

提案形式、benchmark の出典、比較条件、制限は [workflow spec](docs/specs/workflow.md) にある。
本文を含む私有の監査 artifact は Git に追加しない。途中の記録は上書きせず、`workflow show RUN` で再開する。

## 使い方

```sh
cclens-multi analyze --json
cclens-multi usage --by month --since 2026-09-01 --until 2026-10-01 --json
cclens-multi failures --agent codex --min-count 2 --frozen --json
cclens-multi stuck --project /absolute/project/path --frozen --json
cclens-multi prompts --agent pi --frozen --json
cclens-multi inventory --frozen --json
cclens-multi sources --agent opencode --frozen --json
cclens-multi get '<example.id>' --agent codex --frozen --json
```

レポートは通常、増分分析してから出力する。`--frozen` は既存 DB の読み取りだけを行う。既定の上限は 50 行。`total_rows`・`truncated` を確認し、フィルタか `--limit`（最大 10,000）で絞る。`doctor` は利用量・失敗・編集反復・ユーザー介入をまとめる。Agent 向け手順は [docs/agent/SKILL.md](docs/agent/SKILL.md)、引数一覧は `--help`。

`--json` は stdout に単一の JSON を出力し、診断は stderr に出す。指定しない場合も整形済み JSON。stdin を使わず対話待ちは発生しない。終了コードは 0 が成功（取得範囲に欠損がある場合を含む）、1 が入力・DB・実行前提のエラー、2 が CLI の構文・引数のエラー。欠損は `sources` と各指標の取得件数で判定する。

## 入力と保存先

| Agent | 既定の入力 | 上書き |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/` | `--claude PATH`、`CLAUDE_CONFIG_DIR` |
| Codex | `~/.codex/sessions/` | `--codex PATH`、`CODEX_HOME` |
| OpenCode | `~/.local/share/opencode/opencode.db` | `--opencode PATH`、`XDG_DATA_HOME` |
| pi | `~/.pi/agent/sessions/` | `--pi PATH` |
| OMP | `~/.omp/agent/sessions/` | `--omp PATH` |

`--home PATH` は別の home 配下を使う。明示指定した入力ルートは DB に記録され、次の増分実行で再利用する。入力フラグはディレクトリに加えて単一 JSONL も受け付ける。OpenCode は SQLite とその WAL を読み取り専用のトランザクションで読む。

DB は `${XDG_STATE_HOME:-~/.local/state}/cclens-multi/analysis.db`。`--db /absolute/path/analysis.db` で変更できる。入力先への DB 書き込みは拒否する。Unix の保存ディレクトリと DB は所有者のみアクセス可能。消えた入力から抽出済みの履歴は保持し、ソースの不在を表示する。

## 指標の読み方

- トークンは入力に cache read/write、出力に reasoning を含む共通形式。cache と reasoning は内数なので重ねて加算しない。欠損は `null`、合計は取得できた値だけの合計。`known_*_events` と `usage_events` を比較する。
- Codex の旧累積カウンタは差分化し、重複スナップショットを除く。応答単位ログがある区間はそちらを優先する。旧区間の応答同一性は推定のため `estimated_usage_events` に入る。初期累積値の前史が不明な場合は直近応答だけを数え、警告する。
- 失敗の発生はログの error/終了コードを根拠にし、失敗カテゴリ、編集反復による修正ループ、指示・修正・継続・質問の分類は推定。編集反復だけでは作業の失敗を断定しない。`examples` → `get` で元の JSONL 行番号または SQLite の table/row ID を辿れる。
- `reported_api_equivalent_usd` はログに記録された API 換算額。モデル料金表から新しく計算しない。`actual_payment_usd` は常に `null`。定額契約の実際の支払額や OpenCode の provider 固有の単位と同一視しない。
- 設定・skill の存在と Markdown の概算トークン量を収集する。`static_tokens` は定義全体、`startup_tokens` は起動時、`on_demand_tokens` は必要時に読む本文の概算。Claude は既存 parser の `load_mode` を保持し、他の Agent のロード方式・起動時コストは推測せず `null` にする。外部 plugin の全設定や実際のロード状態は網羅しない。Claude 以外の設定 JSON/TOML はファイルのメタデータだけを読む。skill の明示呼び出し・読み取りは区別し、トレースがなければ `null` / `unavailable`。取得できない使用回数をゼロや未使用と扱わない。

変更確認はJSONLを固定サイズのバッファでハッシュし、OpenCodeは読み取りトランザクション内で生の行をハッシュする。サイズ・更新時刻だけでスキップせず、同サイズの書き換えやWAL内の更新も検出する。usage・failures・promptsはSQLiteから逐次集計する。`--limit` は表示件数だけを制限し、総数や合計の計算対象は減らさない。編集バーストの検出は引き続き対象の編集履歴をメモリに保持する。

Codex の単独 `rg` 検索で終了コード 1・空出力の記録は「該当なし」として failure から除く。実行エラー・複合コマンド・欠損フィールドは除外しない。既存 DB には次の refresh で反映され、`--frozen` は保存済みの結果をそのまま返す。

未対応のイベント・壊れた行・欠損カウンタは `sources --json` と stderr に出る。保存形式の全バージョン互換は保証せず、解析可能な根拠の範囲を出力する。pi / OMP の既知の observation-pack 補助ledgerは除外件数をsource警告に残し、モデル利用量へ加算しない。未認識の補助ログは引き続き取得範囲の欠損として報告する。

## 本家の Claude 分析

`cclens-multi claude <本家のコマンド>` で既存の設定分析を使える。DB は共通だが、互換コマンドは本家の Claude 用テーブルを別途増分更新する。

```sh
cclens-multi claude analyze --json
cclens-multi claude inventory --frozen --json
cclens-multi claude overhead --frozen --json
```

互換側の `--scope`・`--by`・`--format` は本家の仕様を保つ。`--json` は `--format json` の別名。`optimize` は `--print` を付けた提案文の出力だけを許可する。設定の自動変更は行わない。

## 開発・検証

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --release --locked
```

テストの入力は合成データだけ。実ログ、集計 DB、認証情報、実設定の本文を Git に追加しない。入力は読み取り専用で、`multi_*` テーブルにはカウンタ・分類・パス・イベント ID・根拠位置を保存し、本文を保存しない。互換側の本家テーブルは既存仕様に従って短いエラー抜粋も保持する。ローカル DB とレポートのパス・モデル名も私的な情報として扱う。

管理先は [harunonsystem/cclens-multi](https://github.com/harunonsystem/cclens-multi)。既定ブランチは `main`。公開 repo のため、commit 前に README.md の手順で privacy hook（betterleaks・lefthook）を入れる。規則は [session-data-privacy](.claude/rules/session-data-privacy.md)、CI の `privacy` job が同じ検査を必須で行う。

crates.io・Homebrew への公開は行わないため、本家の release workflow と配布用の CI job（クロスビルド、`cargo publish --dry-run`）は削除した。
