# 標準的な特徴量セット——Writeprints と LIWC

英語圏で既製品として使われている特徴量の束。[Bhandarkar](bhandarkar-2024.md) が
「指示に載せた特徴」の出どころでもある。

## Writeprints

Abbasi & Chen (2008) Writeprints: A stylometric approach to identity-level identification
and similarity detection in cyberspace. ACM TOIS 26(2), Article 7.
DOI: <https://doi.org/10.1145/1344411.1344413>

<strong>要旨のみ確認した。</strong> 本文は有料で、入手していない。以下はすべて要旨に書かれている
範囲である。

特徴を 5 種に分けている。

> a rich set of stylistic features, including <strong>lexical, syntactic, structural,
> content-specific, and idiosyncratic</strong> attributes

語彙的・統語的・<strong>構造的</strong>・内容固有・<strong>特異</strong>。各種の中身は本文にあり、確認していない。

評価は 4 つの領域（メール、インスタントメッセージ、フィードバックコメント、
プログラムコード）で行われ、<strong>100 人の識別で最高 94%</strong>。SVM、Ensemble SVM、PCA、
標準的な KL 変換を上回ったとしている。

### 個人ごとの特徴量セットを使っている

> Writeprints is a Karhunen-Loeve transforms-based technique that uses a sliding window and
> pattern disruption algorithm with <strong>individual author-level feature sets</strong>

そして、それが効いたことも要旨に書かれている。

> Furthermore, <strong>individual-author-level feature sets generally outperformed use of a single
> group of attributes.</strong>

<strong>2008 年の時点で、著者ごとに違う特徴量セットを使う設計が採られ、単一の属性群を使うより
良かったと報告されている。</strong>

[Bhandarkar](bhandarkar-2024.md) が生成の側で提案して試さなかった「著者ごとに動的に」と
同じ考え方が、測る側では 2008 年に報告されている。<strong>再現させる側で試された報告は
見当たらない。</strong>

## Writeprints-static

実装しやすいように固定次元にした版。<https://literary-materials.github.io/writeprints-static/>

<strong>合計 560 次元。</strong>

| | 次元 | 内容 |
| --- | --- | --- |
| 語彙的 | 127 | 語数、平均語長、短語数、文字数、数字率、大文字率、特殊文字 21、英字 26、数字 10、文字 2-gram 39、文字 3-gram 20、hapax と dis legomena 2 |
| 統語的 | 433 | <strong>機能語 403</strong>、品詞タグ 22、句読点 8 |

<strong>560 次元のうち 403 が機能語である。</strong> 機能語が中心という [Stamatatos](stamatatos-2009.md)
の記述が、そのまま構成に出ている。

## LIWC

Linguistic Inquiry and Word Count (Boyd et al. 2022, LIWC-22)。語を心理的・社会的な
カテゴリに割り当てる辞書。感情、認知過程、社会性など。

[Bhandarkar](bhandarkar-2024.md) が指示に使った言語的特徴は、LIWC と Writeprints の
部分集合だった。<strong>それを明示的に指示したら成績が下がった</strong>のがあの論文の結果である。

## 言語

<strong>どちらも英語向けである。</strong> Writeprints-static の 560 次元には大文字率と英字 26 が含まれ、
LIWC の辞書も英語の語彙で作られている。日本語版の整備状況は、この 2 本の範囲外である。
