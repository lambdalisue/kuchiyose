# Ghatpande, Tsuge, Ishihara, Zaitsu, Inaba (2026) 日本語での尤度比による著者比較

Fusing Stylometric and Embedding Systems to Estimate Authorship Likelihood Ratios in
Japanese. arXiv:2606.13991.
<https://arxiv.org/pdf/2606.13991>

<strong>全文を読んだ。[Sawant](authorship-gap-2026.md) が英語で作った「天井と床」に相当する
枠組みを、日本語で、法科学の作法で作っている。</strong>

## 素材——日本語のブログ、2,287 人

> Identify authors with at least two posts of <strong>1,000 characters or more</strong>

<strong>2,287 人 × 2 本 = 4,574 本。</strong> 1 本 1,000 字以上のブログ記事。
<strong>技術記事とほぼ同じ長さと素材である。</strong>

## 使った特徴——5 つ

| 番号 | 特徴 |
| --- | --- |
| 1 | <strong>文字 bigram</strong> |
| 2 | <strong>機能語 unigram</strong> |
| 3 | <strong>品詞 bigram</strong> |
| 4 | <strong>読点と、その直後の文字の bigram</strong> |
| 5 | <strong>文字種</strong> |

<strong>[金 2014](jin-2014.md) が挙げた 4 種とほぼ同じである。</strong> 独立した研究が同じ特徴に
落ち着いている。

そして <strong>4 番が読点の打ち方である。</strong> 日本語の著者分析で検証済みの特徴として、
<strong>読点の直後に何が来るか</strong> が独立の系統として立っている。

これに埋め込みが 2 つ加わる。<strong>6 = 単語埋め込み、7 = 文字埋め込み。</strong>

## 測り方——Cllr（対数尤度比コスト）

<strong>0 に近いほど良い。1 は偶然と同じで、何の情報も無い。</strong>

<strong>これが目盛りである。</strong> 「精度 90%」のような、候補者数に依存する数字ではない。
<strong>証拠としての強さを絶対的に表す。</strong>

## 結果 1——単独では、埋め込みが計量文体論に勝つ

| 系統 | Cllr |
| --- | --- |
| <strong>単語埋め込み</strong> | <strong>0.38757</strong> |
| 文字 bigram（計量文体論で最良） | 0.47115 |
| 文字埋め込み | 0.53951 |
| 品詞 bigram | 0.54215 |

[Kim & Jurgens](kim-jurgens-2026.md) の「文体埋め込みが古典的な計量文体論を上回る」が、
<strong>日本語で確認された。</strong>

## <strong>結果 2——混ぜると、どちらの単独よりも良くなる</strong>

| 混ぜ方 | Cllr | EER |
| --- | --- | --- |
| 計量文体論どうし（1+3+4） | 0.43964 | 0.1184 |
| 埋め込みどうし（6+7） | 0.36186 | 0.1045 |
| <strong>文字 bigram + 単語埋め込み（1+6）</strong> | <strong>0.34603</strong> | 0.0972 |
| 1+6+7 | 0.32907 | 0.0934 |
| <strong>1+4+5+6+7（最良）</strong> | <strong>0.32484</strong> | <strong>0.0929</strong> |
| 1+2+4+5+6+7 | 0.32579 | 0.0934 |
| 全 7 系統 | 0.32855 | 0.0929 |

<strong>2 つ混ぜるだけで、埋め込み単独（0.38757）より良い。</strong> 種類の違うものを混ぜることが
効いている。

そして <strong>5 つが頂点で、6 つ・7 つは悪くなる。</strong> 差はわずかだが、
[StyleRemix](styleremix-2024.md) と [AuthorMix](authormix-2026.md) の「増やすと劣化する」
と同じ形が、測る側でも出ている。

<strong>最良の組み合わせが、文字 bigram・読点 bigram・文字種・単語埋め込み・文字埋め込み
であることに注目する。機能語 unigram と品詞 bigram は落ちている。</strong>

## 読み取れること

### 1. <strong>日本語に、較正された目盛りがある</strong>

[Sawant](authorship-gap-2026.md) は「LUAR に相当する日本語モデルが無い」という穴を
指摘していた。<strong>この論文がその穴を埋めている。</strong>

- 素材は <strong>1,000 字の日本語ブログ</strong>
- <strong>2,287 人</strong>で較正されている
- <strong>Cllr という絶対的な目盛り</strong>がある
- 特徴はすべて再実装できるものである

### 2. 混ぜると効く。ただし 5 つまで

<strong>種類の違うものを混ぜると、どちらの単独よりも良くなる。</strong>
[SPTG の評価](sptg-evaluation-2025.md) の「違う原理の指標を組み合わせる」と、日本語・
法科学の側から一致している。

<strong>そして 6 つ目からは悪くなる。</strong>

### 3. 読点の直後の文字は、独立の系統として立つ

[金 2014](jin-2014.md) は「読点の打ち方は文字 bigram の部分集合」と位置づけていた。
<strong>この論文では、最良の組み合わせに文字 bigram と読点 bigram の両方が入っている。</strong>
（機能語と品詞 bigram は落ちた。）

<strong>部分集合であっても、独立に持つ価値がある。</strong> しかも低次元で、何を数えたかを言える。

### 4. 文字種は効いている

<strong>文字種</strong>（ひらがな・カタカナ・漢字・英数字・記号の比率）が最良の組み合わせに
残っている。日本語固有の、安く数えられて解釈できる特徴である。

> [!NOTE]
> [金 2014](jin-2014.md) は「漢字の使用率」を効かない特徴に挙げていた。<strong>矛盾しない。</strong>
> 単独の比率としては弱く、<strong>文字種の分布として、他の系統と混ぜたときに効く</strong>という
> 読み方になる。

### 5. 目盛りの現在地

<strong>1,000 字、2,287 人、Cllr 0.32484、EER 9.3%。</strong> これが日本語ブログでの到達点である。
