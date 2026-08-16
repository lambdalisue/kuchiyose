# Masks and Mimicry (2025) 著者検証への攻撃

Strategic Obfuscation and Impersonation Attacks on Authorship Verification.
NLP4DH 2025. arXiv:2503.19099.
<https://aclanthology.org/2025.nlp4dh-1.10.pdf>

<strong>全文を読んだ。「なりすまし」を成功として測った、数少ない 1 本である。</strong>

## 2 つの攻撃

| | 何をするか | 難易度 |
| --- | --- | --- |
| 隠す（obfuscation） | 著者を <strong>誰でもいいから別人</strong> に見せる | 易 |
| <strong>なりすます（impersonation）</strong> | 著者を <strong>特定の 1 人</strong> に見せる | <strong>難</strong> |

> in obfuscation, we are going from one author to any other author, whereas in
> impersonation, we are going from any other author to one particular author, <strong>making this
> a much harder problem</strong>

## 結果 1——隠すのは簡単

| 手法 | ASR（Twitter） | ASR（同人小説） |
| --- | --- | --- |
| DIPPER（言い換え専用モデル） | 0.54 | 0.80 |
| Mistral そのまま | 0.90 | 0.23 |
| <strong>Mistral zero-shot</strong> | <strong>0.92</strong> | <strong>0.83</strong> |

言い換えるだけで著者検証は 9 割騙せる。<strong>ただし DIPPER が意味保存では常に最良だった。</strong>
攻撃の強さと意味の保存は別物である。

<strong>50〜60% を言い換えるまで検証器は崩れない。</strong> 部分的な書き換えには頑健である。

## 結果 2——なりすましは <strong>書き手によって桁が違う</strong>

同人小説の最も多作な 5 人を目標にした攻撃成功率。

| 目標著者 | STRAP | Mistral + RAG |
| --- | --- | --- |
| A | 0.50 | 0.54 |
| B | 0.30 | 0.35 |
| C | 0.52 | <strong>0.75</strong> |
| <strong>D</strong> | <strong>0.11</strong> | 0.48 |
| <strong>E</strong> | <strong>0.77</strong> | 0.42 |

<strong>0.11 から 0.77 まで開く。しかも手法によって順位が入れ替わる</strong>（E は STRAP が最良、
D は最悪）。

平均は Mistral-7B で 50%、Mixtral-8x7B で 55%。

<strong>「その人ごとに効く手が違う」ことが、成功率の形で出ている。</strong>
[Bhandarkar](bhandarkar-2024.md) の「固定の特徴集合が全著者に等しく効くという前提を
疑う」に、直接の証拠が付いた。

## <strong>結果 3——手本は 3 本が最良。増やすと下がる</strong>

手本の本数を変えたときの成功率。

| 目標著者 | 1 本 | <strong>3 本</strong> | 全部 |
| --- | --- | --- | --- |
| A | 0.50 | <strong>0.56</strong> | 0.54 |
| B | 0.37 | <strong>0.65</strong> | 0.35 |
| C | 0.52 | <strong>0.79</strong> | 0.75 |
| D | 0.11 | 0.46 | <strong>0.48</strong> |
| E | <strong>0.78</strong> | 0.59 | 0.42 |

> <strong>four out of the five authors perform better with three stories than using more stories</strong>
> ... These results suggest that <strong>less data is needed for optimal performance</strong>

<strong>5 人中 4 人が、全部渡すより 3 本の方が良い。</strong> B は 0.65 が 0.35 に、C は 0.79 が 0.75 に、
E は 0.59 が 0.42 に落ちる。

[Catch Me](catch-me-2025.md) は「増やしても頭打ち」と言っていた。<strong>頭打ちではなく、下がる。</strong>

## 読み取れること

### 1. 手本は少ないほうがよい

<strong>3 本前後。</strong> [AuthorMix](authormix-2026.md) の 16 文、
[Przystalski](przystalski-2025.md) の 10 文と桁が揃う。

<strong>手本を増やすことは、なりすましの成功率を上げない。</strong> そして E のように 1 本が最良の
書き手もいる。<strong>本数自体が書き手ごとに違う。</strong>

### 2. 「書き手ごとに違う」の証拠が、また 1 つ増えた

| 側 | 書き手ごとに違うことの証拠 |
| --- | --- |
| 測る | [Writeprints](writeprints-liwc.md) が著者ごとの特徴量セットを使う |
| 隠す | [StyleRemix](styleremix-2024.md) の軸選択が無作為より 6% 良い |
| <strong>なりすます</strong> | <strong>成功率が 0.11〜0.77 に開く。最良の手法も入れ替わる</strong> |

### 3. 検証器を騙せることは、本人が納得することではない

<strong>この論文が測っているのは攻撃成功率であって、読んだ人の判断ではない。</strong>
成功例を見ると分かる。

> Original: He sighed with relief. Those papers weren't important.
>
> Mistral + RAG: "Whew! Finally got some time to breathe." Nah, those papers were just
> fine. Holy crap, Chix's wings were so big and powerful...

<strong>三人称が一人称になり、内容が変わっている。</strong> 検証器は騙せているが、元の文章の代わり
にはならない。

[文体転換の評価](tst-evaluation.md)が 3 次元——文体・内容・自然さ——を要求する理由が、
具体例として出ている。
