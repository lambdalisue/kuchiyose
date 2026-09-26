# 文書の検査

<strong>手で掃くのをやめるために置いた。</strong>

仕様のレビューを 9 周した結果、<strong>毎周おなじ失敗をしていた</strong>——見つけた場所を直して、
同じ記述のほかの出現を掃いていない。9 周目の監査はこう書いた。

> 撤回した文字列を grep してから「直した」と言い、札を位置表に照らして検める。
> この 2 つで、この周の指摘のほとんどは安く捕まっていた。

<strong>その 2 つがこれである。</strong>

## checkspec.pl

```
perl tools/checkspec.pl docs
```

<strong>見るものを並べる。</strong>

| | |
| --- | --- |
| <strong>撤回した記述が残っていないか</strong> | 一覧はスクリプトの中にある |
| 札が[位置表](../docs/spec/100-metrics.md#定義に落とす)どおりか | 種別ごとに欄の数が違う |
| 札の系統・分類・向きが実在するか | 綴りが違えば層が引けない |
| <strong>実装に足りる形か</strong> | 数え方が無ければ書けない。照合は次元、指示は直し方が必須 |

<strong>実装に足りる形かは、種別で分ける。</strong> 指示できる指標はスカラーなので次元を持たず、
照合の系統は[直し方が書けない](../docs/spec/100-metrics.md#指標の種類)。
<strong>一律に要求すると、正しい定義が落ちる。</strong>

<strong>記述を撤回したら、一覧に足す。</strong> それが「二度と戻ってこない」ことの担保になる。
足さなければ、次に書き直したときに黙って復活する。

<strong>論文の原文を引用している箇所は、撤回の対象にしない。</strong> 引用は事実の記録であって、
我々の主張ではない。一覧に入れる前に、そこが引用でないことを確かめる。

## checklinks.pl

```
perl tools/checklinks.pl .
```

相対リンクと見出しアンカーを全部たどる。<strong>アンカーは GitHub の規則で作る</strong>——約物を
落とし、空白を `-` にする。節の名前を変えたら、ここが落ちる。

Rust のソースのコメントに書いた文書へのリンクも同じ規則でたどる。 コードのコメントも
仕様の節を指しているので、節の名前を変えると黙って腐る。

<strong>経路は両側を同じ関数で正す。</strong> `canonpath` は先頭の `./` を落とすので、片側だけに
通すと <strong>`..` を含むリンクが全部壊れて見える</strong>。実際にそうなっていて、
<strong>1,162 本のうち 478 本を誤って壊れと報告していた</strong>——ディレクトリを跨ぐリンクを
1 本も検査できていなかったことになる。

<strong>検査器そのものを検める。</strong> 実在する見出しを 1 つ壊して、落ちることを確かめる。
落ちなければ、通っているのは緩んでいるからである。

## 何を見ないか

<strong>正しさは見ない。</strong> 数字が論文と合っているか、手続きが実装できるか、設計が当たって
いるかは、どれも読まないと分からない。<strong>この 2 本が見るのは、直したはずのものが直って
いるかだけである。</strong>

## スパイク

実験の道具である。検査器ではない。

| | 何を通すか |
| --- | --- |
| `spike-separation.pl` | 正規化 → 系統 → 距離。分離するか |
| `spike-scale.pl` | 天井と床を仕様の形で作る |
| `spike-calibrate.pl` | 尤度比に変えて合算する。`--check` で目盛りを固定したまま新しい文を測る |
| `spike-rotate.pl` | 相手集合を 10 通り回す |
| `spike-directive.pl` | <strong>指示できる指標</strong>を測る。`--values` は基準を要らない |

<strong>スパイクは仕様に従わせる。</strong> 6 つの欠陥がこのやり方で見つかった
——直すたびに数字が動いた。<strong>スパイクを緩めて通すのは順序が逆である。</strong>

## 実装

`crates/` は仕様の写しである。試験の名前が仕様の主張になっている——
`約物は日本語の文字ではない`、`文は_node_を跨がない`、`層は系統から引くしかない`、
`対応表に無い_directive_は断る`。

```
nix develop --command cargo test
nix develop --command cargo clippy --all-targets

kuchiyose build  <フォルダ> [-o <形代>] [--scene <場面>] [--no-persona] [--agent <道具>] [--print]
kuchiyose write  [<要約>] [-o <草稿>] [--katashiro <形代>] [--rounds <数>] [--no-polish] [--print]
kuchiyose polish <ファイル> [-o <ファイル>] [--katashiro <形代>] [--rounds <数>] [--print]

kuchiyose katashiro build <フォルダ> -o <形代> [--scene <場面>]
kuchiyose katashiro show  <形代>
kuchiyose katashiro diff  <本人の形代> <基準の形代>
kuchiyose katashiro list  <形代> [--kind <種類>] [--state on|off]
kuchiyose katashiro mute|unmute <形代> <名前または ID>... | --kind <種類>
kuchiyose katashiro first-person <形代> <一人称>|auto
kuchiyose katashiro register <形代> polite|plain|auto
kuchiyose katashiro edit  <形代>
kuchiyose katashiro persona <形代> [<ファイル> [--material <フォルダ>] | --remove]
kuchiyose review <草稿>... [--katashiro <形代>] [--baseline <形代>] [--values] [--json]
```

コマンドの体系は[設計 200-command](../docs/design/200-command.md)、LLM の道具の起動は
[設計 400-agent](../docs/design/400-agent.md) にある。

同梱の基準形代は `baselines/` の文書から `katashiro build` で作り、素材のフォルダの経路を
外して実行ファイルに埋め込む（`tools/build-baseline-katashiro.sh`）。`baselines/` を直したら
作り直す。作り直したものと埋め込んだものが一致することは試験が確かめる。

取り込み元は拡張子から決まる。`.md` と `.markdown` は Markdown、`.html` と `.htm` は HTML で、
ほかの拡張子は断る（フォルダの中なら読まずに飛ばす）。

形態素解析器と辞書は実行ファイルに同梱してある（Lindera と UniDic 2.1.2）。用意するものは無い。

<strong>実装が仕様の穴を 11 個見つけた</strong>。
最初の試験で `・` が Katakana ブロックの中にあることが出て、実際の記事に当てたら
無限ループと「補足 18 箇所が 0 になる」が出た。外の解析器を繋いだら、
<strong>辞書の設定が出力の形を決め、知らない語で解析器が死に、長い行が黙って分割された。</strong>
<strong>7 回のレビューと 10 周の監査が見つけられなかったものである。</strong>

### 仕様を機械で守る試験

| 試験 | 何を止めるか |
| --- | --- |
| [`spec_matches.rs`](../crates/kuchiyose-metrics/tests/spec_matches.rs) | 登録簿と `docs/spec/metrics/*.md` の食い違い |
| [`no_scale_dependency.rs`](../crates/kuchiyose-review/tests/no_scale_dependency.rs) | <strong>検めが目盛りを作る側に依存すること</strong> |
| [`real_articles.rs`](../crates/kuchiyose-normalize/tests/real_articles.rs) | 実際の記事が断られること |
| `書き出して読み戻すと同じものになる` | <strong>保存した目盛りが、作ったときと違うものになること</strong> |

<strong>どれも壊して落ちることを確かめてある。</strong> 通っているのが緩んでいるからでないか、
必ず 1 度は壊して見る。
