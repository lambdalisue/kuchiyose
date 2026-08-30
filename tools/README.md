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

実験の道具である。検査器ではない。記録は [docs/spike/](../docs/spike/) にある。

| | 何を通すか |
| --- | --- |
| `spike-separation.pl` | 正規化 → 系統 → 距離。分離するか |
| `spike-scale.pl` | 天井と床を仕様の形で作る |
| `spike-calibrate.pl` | 尤度比に変えて合算する。`--check` で目盛りを固定したまま新しい文を測る |
| `spike-rotate.pl` | 相手集合を 10 通り回す |
| `spike-directive.pl` | <strong>指示できる指標</strong>を測る。`--values` は基準を要らない |

<strong>スパイクは仕様に従わせる。</strong> 6 つの欠陥が[このやり方で見つかった](../docs/spike/300-baseline-scene.md#途中でスパイクの欠陥が-6-つ出た)
——直すたびに数字が動いた。<strong>スパイクを緩めて通すのは順序が逆である。</strong>

## 実装

`crates/` は仕様の写しである。試験の名前が仕様の主張になっている——
`約物は日本語の文字ではない`、`文は_node_を跨がない`、`層は系統から引くしかない`、
`取り込み元を間違えたら断る`。

```
nix develop --command cargo test
nix develop --command cargo clippy --all-targets

kakiburi new    <カセット> --scene <場面>
kakiburi add    <カセット> <ファイル...> --source <取り込み元> --as person
kakiburi build  <カセット>
kakiburi review <ファイル> --cassette <カセット>
kakiburi measure <ファイル>
kakiburi metrics
```

<strong>形態素解析は辞書を指したときだけ動く。</strong> 環境変数 `KAKIBURI_UNIDIC` に
[UniDic](https://clrd.ninjal.ac.jp/unidic/) の経路を渡す。指さなければ、それを要る系統と
指標は<strong>測らない</strong>——0 を返さない。

```
KAKIBURI_UNIDIC=/path/to/unidic-mecab-2.1.2_bin kakiburi build <カセット>
```

<strong>実装が仕様の穴を 11 個見つけた</strong>（[経緯](../docs/spike/600-implementation.md)）。
最初の試験で `・` が Katakana ブロックの中にあることが出て、実際の記事に当てたら
無限ループと「補足 18 箇所が 0 になる」が出た。外の解析器を繋いだら、
<strong>辞書の設定が出力の形を決め、知らない語で解析器が死に、長い行が黙って分割された。</strong>
<strong>7 回のレビューと 10 周の監査が見つけられなかったものである。</strong>

### 仕様を機械で守る試験

| 試験 | 何を止めるか |
| --- | --- |
| [`spec_matches.rs`](../crates/kakiburi-metrics/tests/spec_matches.rs) | 登録簿と `docs/spec/metrics/*.md` の食い違い |
| [`no_scale_dependency.rs`](../crates/kakiburi-review/tests/no_scale_dependency.rs) | <strong>検めが目盛りを作る側に依存すること</strong> |
| [`real_articles.rs`](../crates/kakiburi-normalize/tests/real_articles.rs) | 実際の記事が断られること |
| `書き出して読み戻すと同じものになる` | <strong>保存した目盛りが、作ったときと違うものになること</strong> |

<strong>どれも壊して落ちることを確かめてある。</strong> 通っているのが緩んでいるからでないか、
必ず 1 度は壊して見る。
