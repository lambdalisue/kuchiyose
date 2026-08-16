# Bhandarkar et al. (2024) Emulating Author Style: A Feasibility Study of Prompt-enabled Text Stylization with Off-the-Shelf LLMs

1st Workshop on Personalization of Generative AI Systems (PERSONALIZE 2024), ACL.
University of Florida.
<https://aclanthology.org/2024.personalize-1.6.pdf>

<strong>全文を読んだ。文体特徴を明示的に指示すると成績が下がる、という否定的な結果がある。</strong>

## 何をしたか

既製の LLM 12 種に、ある著者の文体で書かせられるかを試した。プロンプトの与え方を
2 通りで比較している。

| | 内容 |
| --- | --- |
| simple prompting | 著者の文章の例だけを見せる |
| <strong>directed prompting</strong> | 例に加えて、<strong>言語的特徴を明示して指示する</strong> |

評価は 5 種類の著者識別アルゴリズム（BertAA、Contra-X ほか）。生成文が、元の著者の
文章と同じくらい著者識別で当てられるかを見る。

## 結果

<strong>明示的な特徴を指示した方が、ほとんどの LLM で成績が下がった。</strong>

> most LLMs exhibit reduced performance while transitioning from simple to directed
> prompting

そして全体として、<strong>最良でも元の著者の文章の 3 分の 2 程度の識別性能</strong>しか出ない。
「plug-and-play での著者文体模倣には現状かなり限界がある」と結論している。

## なぜ下がったのか（論文の考察）

<strong>1. LLM 自身の理解を邪魔している。</strong> LLM は例文から著者の文体を暗黙に理解している
可能性があり、指示で特定の特徴に注目させると、その理解に干渉して性能を落とす。

<strong>2. 特徴の選び方が悪い。</strong> 使った特徴は LIWC と Writeprints の部分集合である。

> the assumption that a fixed set of linguistic features can universally influence all
> authors equally might be flawed. Therefore, instead of static directed prompting, a more
> effective approach could involve <strong>dynamically prompting LLMs by considering each
> author's individual linguistic preferences</strong>

<strong>固定の特徴集合が全ての著者に等しく効くという前提が誤りかもしれない。静的な指示では
なく、著者ごとにその人の言語的な傾向を考えて動的に指示すべきだ</strong>——と書いてある。

<strong>3. LLM がその特徴を扱えない。</strong> 長い文章では指示した特徴によく従う。とくに調子、
真正性、分析的な面、語彙の豊かさを反映する特徴には従いやすい。

## 読み取れること

<strong>固定の特徴集合を並べて指示する作りは、例文だけより悪い。</strong> 実測されている。

そして論文が挙げる改善案——<strong>著者ごとにその人の傾向を見て動的に指示する</strong>——は、
<strong>提案されただけで試されていない。</strong>

<strong>特徴を選ばずに渡すくらいなら、渡さないほうがよい。</strong> これがこの論文の実測から言える
最も強い主張である。

## 注意

- <strong>日本語ではない。</strong> 英語での結果
- 評価は著者識別アルゴリズムによるもので、人が読んで判断してはいない
- 「著者ごとに動的に」は提案されているだけで、試されていない
