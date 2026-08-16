# 標準的な特徴量セット——Writeprints と LIWC

英語圏で既製品として使われている特徴量の束。[Bhandarkar](bhandarkar-2024.md) が
「指示に載せた特徴」の出どころでもある。

## Writeprints

Abbasi & Chen (2008) Writeprints: A stylometric approach to identity-level identification
and similarity detection in cyberspace. ACM TOIS 26(2).
<https://www.scss.tcd.ie/Khurshid.Ahmad/Research/Sentiments/K_Teams_Buchraest/a7-abbasi.pdf>

特徴を 5 つに分ける。

| 種類 | 内容 |
| --- | --- |
| 語彙的 | 文字と語。大文字の分布、特殊文字、平均語長、1 文あたりの語数 |
| 構造的 | 文章の組み立て。段落数、文数、それぞれの平均長、<strong>挨拶や結びの有無</strong> |
| 統語的 | <strong>機能語</strong>、句読点の型 |
| 内容固有 | その領域に固有の語 |
| <strong>特異</strong> | 綴りの誤りなど、その人だけの癖 |

100 人の識別で 94% と報告されている。

### 個人ごとの特徴量セットを使っている

> Writeprints is a Karhunen-Loeve transforms-based technique that uses a sliding window and
> pattern disruption algorithm with <strong>individual author-level feature sets</strong>

<strong>これは効く。</strong> 2008 年の時点で、<strong>著者ごとに違う特徴量セットを使う</strong>という設計が採られ、
成果を出している。

[Bhandarkar](bhandarkar-2024.md) が生成の側で提案して試さなかった「著者ごとに動的に」は、
<strong>測る側では既に実績がある。</strong> kakiburi の賭けは、測る側で確立された考え方を貸す側に
持ち込むことになる。

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

## 日本語では

<strong>そのままは使えない。</strong> 大文字・小文字の区別が無く、機能語の一覧も違い、LIWC の日本語版
の整備状況も別問題である。

対応するものを日本語で組むなら、[金 2014](jin-2014.md) の 4 種になる。

| Writeprints | 日本語で対応するもの |
| --- | --- |
| 機能語 403 | <strong>助詞・助動詞</strong>の頻度、[語の文体値](baba-2022.md) |
| 品詞タグ 22 | <strong>品詞の unigram / bigram</strong> |
| 句読点 8 | <strong>読点の打ち方</strong>、記号の頻度 |
| 文字 n-gram | <strong>文字 bigram</strong>（3 以上は内容を拾う） |
| 構造的 | 見出し、段落、箇条書き。<strong>短い文章ほど効く</strong> |
| 特異 | 表記ゆれ、誤変換 |
