# Burrows's Delta

著者識別の標準的な基準線。<strong>実装が数十行で済み、標本が小さくても壊れない。</strong>

## 出典

<strong>Burrows, J. (2002) 'Delta': A Measure of Stylistic Difference and a Guide to Likely
Authorship.</strong> Literary and Linguistic Computing 17(3), 267-287.
<https://academic.oup.com/dsh/article-abstract/17/3/267/929277>

<strong>◎ Hoover, D. (2004) Testing Burrows's Delta.</strong> LLC 19(4).
<https://mimno.infosci.cornell.edu/info3350/readings/delta.pdf>（本文を確認）

<strong>○ Evert, S. et al. (2015) Towards a better understanding of Burrows's Delta in
literary authorship attribution.</strong>
<https://aclanthology.org/W15-0709.pdf>（未読）

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

kakiburi の対象（技術記事）はこれを満たす。

## kakiburi にとって

<strong>判別の基準線として、そのまま採る。</strong>

用途は 2 つある。

<strong>1. どれだけ「らしさ」が測れるかの上限を知る。</strong> Delta が分離できないコーパスなら、
解釈できる指標で分離できるはずがない。逆に Delta が分離するのに我々の指標が分離しない
なら、<strong>まだ捉えられていない癖がある</strong>ということになる。

<strong>2. 次の指標の候補を出す。</strong> Delta に強く効いている語を見れば、そこに癖がある。人が
思いつく必要がない。

日本語に当てるには形態素解析を前置きする。語の代わりに文字 n-gram を使う手もあり、
[Stamatatos](stamatatos-2009.md) は分かち書きの難しい言語には文字 n-gram が適すると
している。

<strong>製品には使えない。</strong> z 値の平均絶対差は解釈できないので、貸すことも直し方を示すことも
できない。あくまで物差しである。
