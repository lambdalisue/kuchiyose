# Neurobiber (2025) Fast and Interpretable Stylistic Feature Extraction

arXiv:2502.18590.
<https://arxiv.org/pdf/2502.18590>

<strong>本文を読んだ。「解釈できる指標では精度が出ない」という思い込みを崩す 1 本である。</strong>

## 何をしたか

[Biber の多次元分析](nakamata-multidimensional.md)の文法形式タグを、<strong>ニューラルネットで
直接予測する</strong>。構文解析器を経由しないので速い。

| | 特徴数 | 速度（対 Nini MAT） |
| --- | --- | --- |
| Nini MAT（既存の実装） | 67 | 1x |
| BiberPlus（この論文の Python 実装） | 96 | 2.2x |
| <strong>Neurobiber</strong> | <strong>96</strong> | <strong>56x</strong> |

各特徴が本文中に <strong>あるか無いか</strong>の二値を予測する。macro-F1 0.97、micro-F1 0.98。

## 結果 1——解釈できる 96 次元が、埋め込みとほぼ並ぶ

PAN 2020 の著者検証（small）で比べている。<strong>同人小説なので、同じ著者でも作品世界に
よって書き分ける。題材と register が大きくばらつく課題である。</strong>

| | F1 |
| --- | --- |
| 無作為 | 0.50 |
| RoBERTa bi-encoder（42,000 対で微調整） | <strong>0.78</strong> |
| <strong>Neurobiber の 96 次元 + Random Forest</strong> | <strong>0.77</strong> |

<strong>0.77 対 0.78。</strong> 全データの特徴抽出は約 30 分で終わっている。

> demonstrating that <strong>well-crafted stylistic features can stand alongside more complex
> representations</strong> in tasks like authorship verification

## 結果 2——Biber の第 1 次元が再現される

CORE コーパスの register に対して主成分分析をかけると、
<strong>Involved 対 Informational</strong> の対比が出る。Biber の Dimension 1 と一致する。

一方の端に掲示板、Q&A、対話。反対の端に技術報告、百科事典、研究論文。

[中俣](nakamata-multidimensional.md) の D1（共感的対話 対 客観的伝達、説明力 0.556）と
<strong>同じ軸である。</strong> 英語で 1988 年に見つかり、日本語で独立に見つかり、2025 年に別の
コーパスで再現された。

## 読み取れること

### 1. 解釈できることと精度は、引き換えではない

[Kim & Jurgens](kim-jurgens-2026.md) は「文体埋め込みが古典的な計量文体論を上回る」と
していた。<strong>それは万能の結論ではない。</strong> 特徴を丁寧に設計すれば、<strong>題材がばらつく課題で
埋め込みとほぼ同じところまで来る。</strong>

<strong>解釈できる特徴を選ぶ代償は小さい。</strong> 埋め込みは値の意味を言えないが、この 96 特徴は
どれが何を数えたかを言える。

### 2. 「あるか無いか」でも足りる場合がある

Neurobiber が予測するのは <strong>二値（存在するか）</strong> であって、頻度ではない。それで F1 0.77 が
出ている。

<strong>特徴によっては、頻度を測らなくても存在の有無で足りる。</strong>

### 3. 第 1 軸は言語をまたいで同じ

<strong>共感的対話 対 客観的伝達。</strong> Biber（英語 1988）、中俣（日本語）、Neurobiber（英語
2025）で一致している。<strong>言語とコーパスをまたいで再現している数少ない軸である。</strong>

### 4. そのままは使えない

<strong>英語専用である。</strong> 96 の特徴は英語の文法形式（時制、法助動詞、関係節、受動態、
人称代名詞など）に基づく。日本語の対応物は [中俣](nakamata-multidimensional.md) の
89 の文法形式が担う。

<strong>移せるのは特徴一覧ではなく、手順である。</strong> 文法形式の存否を数え、因子分析で軸を出し、
解釈できる次元として使う。
