# Burrows's Delta

著者識別の標準的な基準線。<strong>実装が数十行で済み、標本が小さくても壊れない。</strong>

## 出典

<strong>Burrows, J. (2002) 'Delta': A Measure of Stylistic Difference and a Guide to Likely
Authorship.</strong> Literary and Linguistic Computing 17(3), 267-287.
<https://academic.oup.com/dsh/article-abstract/17/3/267/929277>

<strong>◎ Hoover, D. (2004) Testing Burrows's Delta.</strong> LLC 19(4).
<https://mimno.infosci.cornell.edu/info3350/readings/delta.pdf>（本文を確認）

<strong>◎ Evert, S. et al. (2015) Towards a better understanding of Burrows's Delta in
literary authorship attribution.</strong>
<https://aclanthology.org/W15-0709.pdf>（要旨と結論を確認）

<strong>○ 実装例</strong> <https://github.com/fastdatascience/faststylometry>

## 手順

1. コーパス全体で頻度上位 N 語を取る
2. 各語の頻度を、コーパス全体での平均と標準偏差で z 化する
3. 2 つの文章の z 値の <strong>平均絶対差</strong> を距離とする

これだけである。学習も調整もない。

## 語数をいくつにするか

Burrows は 150 語で説明したが、Hoover が検証している。

- 150 語から 40 語に減らすと <strong>精度が落ちる</strong>
- 800 語の方が 150 語より <strong>たいてい精度が高い</strong>
- 20〜800 語で試したところ、<strong>少なくとも 600 語までは精度が上がり続ける</strong>

Hoover の実験は詩と散文の両方で、約 54 万語のコーパス、25 人の著者。

<strong>したがって数百語を取るのが妥当。</strong> ただし [Stamatatos](stamatatos-2009.md) が警告する
とおり、数百語を超えると開いたクラス（内容語）が多数派になり、題材固有の語が混ざる。

## 必要な文章の長さ

<strong>1,500 語を超える文章で有効。</strong> 100 語程度でも候補の絞り込みには使える。

## 基準線としての性質

<strong>Delta が分離できないコーパスは、そもそも書き手の差が薄い。</strong> 逆に Delta が分離する
場合、そこには何らかの癖がある。<strong>強く効いている語を見れば、それが何かが分かる。</strong>

日本語に当てるには形態素解析を前置きする。語の代わりに文字 n-gram を使う手もあり、
[Stamatatos](stamatatos-2009.md) は分かち書きの難しい言語には文字 n-gram が適すると
している。

<strong>出力は解釈できない。</strong> z 値の平均絶対差なので、どこがどう違うかは言えない。分離する
かどうかを見る量である。

## 改良版を追う必要はない

Evert らによれば、2002 年以降に提案された Delta の変種は数多いが、

> a recent empirical study showed that <strong>none of the proposed variants constitute a major
> improvement</strong> in terms of authorship attribution performance

<strong>素の Delta で足りる。</strong> 変種の比較に時間を使わない。

## 特徴選抜は過学習する

Evert らは、再帰的な特徴削減で完璧な分類・クラスタリングを達成できたが、そのとき
選ばれた特徴を調べて警告している。

- <strong>内容語が選ばれる。</strong>「機能語が最良の指標」という通念に反するが、内容語の方が
  過学習しやすいのではないかとしている
- <strong>コーパスの癖が選ばれる。</strong> ローマ数字（`XL`、`XXXVVII`）や歴史的仮名遣い
  （`Heimath`、`giebt`）が特徴として拾われた。これらは著者の文体ではなく <strong>コーパスの
  作りに由来する</strong>

未知データで検証すると精度が落ちた。<strong>「選抜したら精度が上がった」は、そのままでは
信用できない。</strong>
