# Fisher, Hallinan et al. (2024) StyleRemix

Interpretable Authorship Obfuscation via Distillation and Perturbation of Style Elements.
EMNLP 2024, 4172-4206. University of Washington / AI2.
<https://aclanthology.org/2024.emnlp-main.241.pdf> /
<https://github.com/jfisher52/StyleRemix>

<strong>全文を読んだ。書き手ごとに軸を選んで文体を動かす手法が、実装され評価されている。</strong>
<strong>ただし向きは「隠す」側である。</strong>

## 何をしているか

著者を <strong>隠す</strong>（obfuscation）ための書き換えである。

1. 文体を <strong>軸</strong> に分解する
2. 各軸の各方向に <strong>LoRA アダプタ</strong> を 1 つずつ訓練しておく
3. <strong>その著者が平均から最も離れている軸を選ぶ</strong>
4. 離れている度合いに応じた重みでアダプタを混ぜ、生成を <strong>その方向から遠ざける</strong>

<strong>4 を「近づける」に変えれば、再現の手順になる。</strong> 論文はその向きを試していない。

## 7 つの軸

| 軸 | 測り方 |
| --- | --- |
| 長さ | 1 文あたりの語数 |
| <strong>機能語</strong> | 機能語の数 |
| 学年水準 | Flesch-Kincaid / Linsear Write / Gunning Fog の平均 |
| 丁寧さ | 分類器の出力 |
| 皮肉 | 分類器 |
| 態 | 能動 / 受動 |
| 書き方 | 説得 / 描写 / 語り / 説明の 4 択 |

各軸に高低 2 方向（書き方だけ 4 択）で、<strong>16 の要素</strong>。

> our goal is to identify <strong>"author invariants"</strong>, which are text properties that are unique
> to a specific author

## <strong>ここが本命——書き手ごとの軸選択</strong>

<strong>手順が完全に書かれている。</strong>

1. 各著者について、7 軸の値からなる <strong>著者ベクトル</strong> `x_i ∈ R^7` を作る
2. 全著者で正規化する
3. 平均との差 `x̄_i = x_i − mean` を取る
4. <strong>絶対値 `|x̄_i|` の大きい順に上位 k 軸を選ぶ</strong>
5. 重みは、平均から何標準偏差離れているかで決める

| 標準偏差 | アダプタの重み |
| --- | --- |
| ≤ 1 | 0.7 |
| 1〜2 | 0.9 |
| 2〜3 | 1.2 |
| > 3 | 1.5 |

### 効果は測られている

> choosing based on difference between the average style vector and the author vector
> <strong>improves obfuscation on average by 6% over random selection</strong> of the same number of
> weights

<strong>同じ本数を無作為に選ぶより 6% 良い。文法と内容の保存は変わらない。</strong>

[Bhandarkar](bhandarkar-2024.md) が「試していない」と書いた <strong>書き手ごとの動的な特徴選択</strong>
は、隠す側では実装され、効果が測られている。

## <strong>もう 1 つの本命——軸を増やすと壊れる</strong>

軸の本数を 1 から 7 まで変えて総合点を測っている。

| 領域 | 1 | 2 | 3 | 4 | <strong>5</strong> | 6 | 7 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 演説 | 17.0 | 17.7 | <strong>21.2</strong> | 19.2 | <strong>6.0</strong> | 17.0 | 11.4 |
| ブログ | 13.1 | 16.5 | <strong>19.6</strong> | 18.9 | 12.1 | 10.5 | 6.4 |
| 小説 | 8.6 | 11.2 | 13.0 | 14.4 | 16.3 | 11.2 | <strong>21.8</strong> |
| 学術 | 1.1 | 1.8 | 2.3 | 3.4 | 0.8 | 6.0 | 16.9 |

> using <strong>5+ style adapters leads to an average of ∼16% decrease in grammar</strong> and a ∼5%
> decrease in overall score

<strong>軸は 3〜4 本が最良で、5 本を超えると文法が壊れる。</strong>

[Bhandarkar](bhandarkar-2024.md) の「特徴を並べて指示すると悪化する」に、<strong>本数という
形が付いた。</strong> 悪化するのは「指示すること」ではなく「<strong>多すぎること</strong>」かもしれない。

## 学術文書だけ、ほとんど動かない

演説・ブログ・小説では総合点が 10〜20 出るのに、<strong>学術文書は 1〜3 である</strong>（7 軸まで
増やしてやっと 16.9）。

<strong>ジャンルの制約が強い文書は、文体を動かす余地が小さい。</strong>
[柳・金](yanagi-jin-2022.md) の「ジャンルは器、個人文体は流体」がそのまま出ている。

崩れた文章ほど文体が出るという [Wang](wang-2025.md) の指摘とも整合する。<strong>形式の
決まった文書ほど、動かせる幅が小さい。</strong>

## 読み取れること

### 1. 書き手ごとの選択は、2 つの向きで実績がある

| 向き | 書き手ごとの選択 | 出典 |
| --- | --- | --- |
| 測る | <strong>実績あり</strong>（2008 年から） | [Writeprints](writeprints-liwc.md) |
| <strong>隠す</strong> | <strong>実績あり。無作為より 6% 良い</strong> | <strong>StyleRemix</strong> |
| 真似る | <strong>未検証</strong> | [Bhandarkar](bhandarkar-2024.md) が提案のみ |

<strong>未検証で残っているのは「近づける向き」だけである。</strong>

### 2. 選び方の式が明示されている

<strong>平均からの差の絶対値で上位 k 本。標準偏差で重み付け。</strong> 再実装できる形で書かれている。

ここでの「平均」は他の著者の平均である。基準を別のものに置けば、差の取り方も変わる。

### 3. 渡す軸は 3〜4 本

<strong>本数を振った実測がある。</strong> 5 本以上で文法が 16% 落ちる。

### 4. 効きを事前に検証している

<strong>アダプタが実際にその軸を動かせるかを、使う前に確かめている。</strong>
[高橋ら](japanese-llm-style.md) の「制御できる軸とできない軸がある」と合わせると、
<strong>軸ごとの効きは、使う前に測っておく類のものである。</strong>

## 公開されている資源

<strong>AuthorMix</strong>——14 著者、4 領域（大統領演説、1900 年代初頭の小説、学術論文、日記風
ブログ）、3 万段落超。

<strong>DiSC</strong>——7 軸 16 方向の対訳コーパス。1,500 テキスト、総計 2.4 万。

<strong>どちらも英語である。</strong> DiSC に相当する日本語のデータは見当たらない。
