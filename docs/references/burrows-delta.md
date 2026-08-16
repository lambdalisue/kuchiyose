# Burrows's Delta

Evert らが <strong>「文学作品の著者識別で最も確立した文体差の尺度」</strong> と呼ぶもの。ジャンルと
言語をまたいで頑健であることが報告されている（Hoover 2004b、Eder & Rybicki 2013）。

## 出典

<strong>Burrows, J. (2002) 'Delta': A Measure of Stylistic Difference and a Guide to Likely
Authorship.</strong> Literary and Linguistic Computing 17(3), 267-287.
<https://academic.oup.com/dsh/article-abstract/17/3/267/929277>

<strong>Hoover, D. (2004) Testing Burrows's Delta.</strong> Literary and Linguistic Computing 19(4),
453-475. DOI: <https://doi.org/10.1093/llc/19.4.453>
<https://academic.oup.com/dsh/article/19/4/453/943644>（<strong>要旨のみ確認。本文は有料で
入手していない</strong>）

<strong>◎ Evert, S. et al. (2015) Towards a better understanding of Burrows's Delta in
literary authorship attribution.</strong>
<https://aclanthology.org/W15-0709.pdf>（本文を確認）

<strong>○ 実装例</strong> <https://github.com/fastdatascience/faststylometry>

## 手順

Evert らの記述による。

1. コーパス全体で頻度上位 N 語を取る
2. 各語の頻度を、コーパス全体での平均と標準偏差で z 化する
3. 2 つの文章の z 値の <strong>平均絶対差</strong> を距離とする

これだけである。学習も調整もない。

## 語数をいくつにするか

Burrows は 150 語で提案した。Hoover (2004) の要旨が、その検証結果をこう述べている。

> It also shows that <strong>much larger numbers of frequent words are even more accurate than
> the 150 that Burrows tested.</strong>

<strong>150 語より、ずっと多くの頻出語を取ったほうが正確である。</strong> 何語が最良かという具体的な
数値は本文にあり、確認していない。

要旨は他に 3 つ挙げている。

- 詩だけでなく <strong>散文でもほぼ同じくらい働く</strong>
- <strong>人称代名詞</strong>と、<strong>1 つのテキストが出現の大半を占める語</strong>を除くと、精度が大きく上がる
- 著者ごとに <strong>複数のテキストをまとめる</strong>と、著者内のばらつきの影響が減る

そして位置づけを「著者識別の <strong>予備段階</strong> で有用」としている。

> [!NOTE]
> [Stamatatos](stamatatos-2009.md) は、数百語を超えると開いたクラス（内容語）が多数派に
> なり題材固有の語が混ざる、と警告している。語数を増やす方向とは逆を向く。どちらが
> 正しいかは、この 2 本の記述だけでは決まらない。

## 出力の性質

<strong>返るのは 2 つの文章の距離だけである。</strong> z 値の平均絶対差なので、どこがどう違うかは
出てこない。用途は、クラスタリングと、候補集合の中で最も近い著者への帰属である。

日本語に当てるには分かち書きが要る。[Stamatatos](stamatatos-2009.md) は、分かち書きの
難しい言語では文字 n-gram が適した解になるとしている。

## 変種のうち 1 つは、はっきり上回る

Evert らの要旨は、2002 年以降に提案された変種についてこう書いている。

> a recent empirical study showed that <strong>none of the proposed variants constitute a major
> improvement</strong> in terms of authorship attribution performance

<strong>ただし、同じ論文の序論が続きを書いている。</strong> その実証研究（Jannidis et al. 2015、英・独・
仏の 3 コーパス、Delta と 13 の先行版・変種）の結果は、こうである。

> Burrows's Delta remains a strong contender, but <strong>is outperformed quite clearly by Cosine
> Delta</strong> as proposed by Smith and Aldridge (2011)

<strong>素の Delta が最良なのではない。</strong> 要旨の一文だけを読むと逆に取れる。

Evert ら自身の結論も、効いている要素を名指ししている。

> <strong>Vector normalization is revealed as the key factor behind the success of Cosine Delta.</strong>
> It also improves Burrows's Delta and <strong>makes all measures robust wrt. the choice of n<sub>w</sub></strong>

ベクトル正規化を入れると、<strong>語数 n<sub>w</sub> の選び方に対して頑健になる。</strong> 正規化を入れない
場合、語数は「良い方針が無い決定的な要因」として残る、とも書いている。

なぜ Burrows の選択（最頻語・z 得点・マンハッタン距離）が数学的により正当化しやすい
代案より良いのかは、<strong>まだ説明できていない</strong>と論文自身が認めている。

## 特徴選抜は、この実験では過学習しなかった

Evert らは再帰的特徴削減（recursive feature elimination）で 234 語まで絞り、完璧な
分類・クラスタリングを得た。<strong>そこで過学習を疑い、未知データで検証している。</strong>

> Perfect cross-validation and clustering results <strong>suggest that there may be severe
> overfitting.</strong> In order to verify how well the set of selected features performs on unseen
> data, we used two additional evaluation data sets

結果は <strong>否定された</strong>。

| 検証セット | 結果 |
| --- | --- |
| 未知の 19 著者・71 作品 | SVC と MaxEnt がともに <strong>正解率 0.97</strong> |
| 未知の 34 著者・155 作品 | 全特徴より <strong>クラスタリングの質が上がった</strong>（ARI 0.871 対 0.835） |

> indicating that the selected features are <strong>not overfit</strong> to the specific novels in the
> training corpus but <strong>generalize very well</strong> to other works from the same authors

結論も「教師ありの特徴選抜は、著者識別をさらに改善する<strong>見込みのある方法</strong>」としている。
ただし、2 つの検証セットの間でクラスタリング精度に差があり、選ばれた特徴は
<strong>ある程度は著者依存である</strong>とも書いている。

### 選ばれた特徴の中身についての観察

過学習の証拠としてではなく、<strong>興味深い型</strong>として報告されている。

- <strong>機能語に限られない。</strong>「機能語が最良の指標」という通念に反する。ただし内容語の方が
  過学習しやすい<strong>かもしれない</strong>、という但し書きが付く
- 英語・仏語のコーパスでは <strong>ローマ数字</strong>（`XL`、`XXXVVII`）が入った。章数の多い小説の
  特徴であり、<strong>それが特定の著者の特徴でありうる</strong>としている
- 独語のコーパスでは <strong>歴史的正書法</strong>（`Heimath`、`giebt`）が入った。<strong>これについてだけ</strong>、
  著者の文体ではなく <strong>コーパスの作りに由来する可能性が高い</strong>としている
