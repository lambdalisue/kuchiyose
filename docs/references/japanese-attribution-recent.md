# 日本語の著者推定——近年の 2 本

[尤度比の枠組み](fusing-lr-2026.md)とは別に、日本語の著者推定で押さえておく 2 本。
<strong>片方は文学、片方は短い商品レビュー。長さの両端にあたる。</strong>

## 神田・金ほか (2025) BERT と特徴量の統合アンサンブル

Integrated ensemble of BERT- and feature-based models for authorship attribution in
Japanese literary works. Frontiers in Artificial Intelligence. arXiv:2504.08527.
<https://arxiv.org/html/2504.08527v1>

### 素材

| コーパス | 著者 | 作品 | 長さ |
| --- | --- | --- | --- |
| A | 古典 10 人（夏目漱石、太宰治、森鷗外ほか） | 各 20 | <strong>先頭 510 形態素（約 800 字）</strong> |
| B | <strong>現代 10 人</strong>（村上春樹、東野圭吾ほか） | 各 20 | 同 |

### 結果

| 手法 | A の F1 | <strong>B の F1</strong> |
| --- | --- | --- |
| BERT 単独（最良） | 0.970 | 0.823 |
| 特徴量 + 分類器（最良） | 0.810 | 0.800 |
| BERT のアンサンブル | 0.990 | 0.902 |
| 特徴量のアンサンブル | 0.912 | 0.889 |
| <strong>統合アンサンブル</strong> | <strong>1.000</strong> | <strong>0.960</strong> |

<strong>現代の作家で 14 ポイント上がっている。</strong> 古典（事前学習済みのデータに入っている）で
1.000 が出るのは、その分を差し引いて読む。

### <strong>ここが効く——単独で弱い特徴が、混ぜると効く</strong>

> Phrase-patterns scored lowest individually (<strong>0.704</strong>) but <strong>play a significant role in top
> ensemble combinations</strong>

<strong>文節パターンは単独では最下位なのに、最良の組み合わせには必ず入っている。</strong>

> [!IMPORTANT]
> <strong>単独の性能で指標を落としてはいけない。</strong> 他と違うものを見ている指標は、
> 単独で弱くても組み合わせに効く。
>
> [Stamatatos](stamatatos-2009.md) の「特徴は頻度で選ぶ、判別力で選抜するな」と
> 同じ方向を、別の理由から指している。そして
> [尤度比の論文](fusing-lr-2026.md)で機能語 unigram（単独では中位）が最良の組み合わせ
> から落ちたのも、同じ現象の裏面である。<strong>効くのは組み合わせであって、単独の順位では
> ない。</strong>

使った特徴は <strong>文字 bigram（44 次元）、トークン unigram（3,300 次元）、
文節パターン（804 次元）</strong>。分類器は Random Forest と AdaBoost。
[金 2014](jin-2014.md) の「RF を必ず含める」がここでも守られている。

## Japanese Web Reviews (2026) 短い日本語の非形式文章

Foundational Study on Authorship Attribution of Japanese Web Reviews for Actor Analysis.
arXiv:2604.16376.
<https://arxiv.org/html/2604.16376>

### 素材——<strong>中央値 60 文字</strong>

楽天市場のレビュー。上位 100 人で 68,222 件、上位 1,000 人で 294,443 件。
<strong>1 件の中央値は 60 文字、平均 118 文字。</strong>

### 結果

| 候補者数 | TF-IDF + LR | BERT 微調整 |
| --- | --- | --- |
| 100 人 | 0.8622 | <strong>0.8827</strong> |
| <strong>1,000 人</strong> | <strong>0.510</strong> | <strong>0.501</strong> |

<strong>候補が 10 倍になると精度は半分になる。</strong> ただし上位 10 位までに入る率は 1,000 人でも
0.745 で、絞り込みには使える。

<strong>速さの差が大きい。</strong> TF-IDF + ロジスティック回帰は 36.72 秒、BERT 微調整は 1,063.99 秒。
<strong>約 29 倍。</strong> 精度差は 2 ポイント。

### <strong>短い日本語で何が邪魔をするか</strong>

論文が 3 つ挙げている。

| 障害 | 中身 |
| --- | --- |
| <strong>定型文</strong> | 「発送が早かった」「梱包が丁寧」——<strong>著者間の類似が上がる</strong> |
| <strong>情報不足</strong> | 「よかったです」「満足」——<strong>個人が出る余地が無い</strong> |
| <strong>話題語彙の混入</strong> | 「スマホケース」——<strong>文体ではなく内容で当ててしまう</strong> |

3 つ目は [Wegmann](wegmann-2022.md) の指摘そのものである。論文自身が
「identification based on content rather than style」と書いている。

## 読み取れること

### 1. 素材の長さと候補者数で、到達点が大きく違う

| 素材 | 研究 | 到達点 |
| --- | --- | --- |
| 技術記事（1,000 字） | [尤度比](fusing-lr-2026.md) 2,287 人 | <strong>EER 9.3%</strong> |
| 文学（800 字） | 統合アンサンブル 10 人 | F1 0.96〜1.00 |
| 作文（1,100 字） | [金 2013](jin-2013.md) 11 人 | 99% |
| 日記（500 字） | [金 2014](jin-2014.md) 6 人 | F1 98.4 |
| <strong>チャット（60 字）</strong> | 商品レビュー 100 人 | <strong>88%</strong>、1,000 人で <strong>51%</strong> |

<strong>短くなるほど落ちる。そして候補が増えるほど落ちる。</strong>

<strong>60 字で定型文が多い素材では、そもそも測る材料が足りない。</strong>

### 2. 定型文を落とす前処理が要る

<strong>「発送が早かった」に相当するものが、どの場面にもある。</strong> 技術記事なら見出しの定型、
コードブロック、引用。チャットなら挨拶と相槌。

[柳・金 2023](yanagi-jin-2023.md) が会話文を落としたのと同じ判断が、素材ごとに要る。

### 3. 単独で弱い指標が、組み合わせでは効く

文節パターンは単独 F1 0.704 で最下位なのに、最良の組み合わせには必ず入る。
<strong>単独の順位は、組み合わせでの寄与を予測しない。</strong>

### 4. 重い手法を使う理由は薄い

<strong>29 倍の時間をかけて 2 ポイント。</strong> BERT の微調整と TF-IDF + ロジスティック回帰の差は
その程度である。
