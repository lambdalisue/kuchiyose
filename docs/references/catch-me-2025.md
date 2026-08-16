# Wang et al. (2025) Catch Me If You Can? Not Yet

LLMs Still Struggle to Imitate the Implicit Writing Styles of Everyday Authors.
Findings of ACL: EMNLP 2025, pp.10040-10055. arXiv:2509.14543.
Stony Brook / Penn State / Bosch.
<https://aclanthology.org/2025.findings-emnlp.532.pdf>

<strong>全文を読んだ。対象は「普通の人」で、素材はブログと掲示板である。</strong>

## 何をしたか

<strong>400 人以上の実在の書き手</strong>を 4 領域から集め、6 つの LLM に真似させた。手本として
渡すのは、その書き手の文章と、書く内容の要約である。

| 素材 | 書き手 | 文章 | 平均語数 |
| --- | --- | --- | --- |
| Enron（社内メール） | 150 | 3,884 | 309 |
| <strong>Blog</strong> | 100 | 25,224 | 319 |
| CCAT50（記者の記事） | 50 | 2,500 | 584 |
| <strong>Reddit</strong> | 100 | 8,451 | 333 |

モデルは GPT-4o、GPT-4o-mini、Gemini-2.0-Flash、Gemma-3-27B、Llama-4-Maverick、
DeepSeek-V3。<strong>1 モデルあたり 4 万件以上の生成。</strong>

## <strong>結果——形の決まった文章はできる。崩れた文章はできない</strong>

生成文が目標の書き手のものと判定される率。

| 素材 | 著者検証 | 著者推定（上位 5） |
| --- | --- | --- |
| 記事 | 95〜97% | 86〜93% |
| メール | 95〜97% | 56〜62% |
| <strong>ブログ</strong> | <strong>19〜66%</strong> | <strong>39〜44%</strong> |
| <strong>掲示板</strong> | <strong>19〜66%</strong> | <strong>27〜36%</strong> |

> while LLMs can approximate user styles in structured formats like news and email, they
> <strong>struggle with nuanced, informal writing in blogs and forums</strong>

<strong>形式が決まっているほど「できているように見える」。</strong> 記事の 95% は、記事という型を
守れているだけかもしれない。個人の文体が最も出る崩れた文章で、最も失敗する。

## <strong>結果 2——そもそも人間の文章に見えない</strong>

生成文が「人が書いた」と判定された割合。

| モデル | 人と判定された率 |
| --- | --- |
| <strong>GPT 系</strong> | <strong>ほぼ 0%</strong> |
| その他 | 最大 54% |

<strong>文体が寄っているかどうか以前に、AI が書いたと分かる。</strong>
[Zeng & Nini](zeng-nini-2026.md) の「繰り返しが足りない」「エントロピーが高い」が、
別の実験で再現されている。

## <strong>結果 3——増やしても、似た題材を選んでも、良くならない</strong>

手本を 2, 4, 6, 8, 10 本と変えている。

> including more writing examples in the prompt <strong>affects the four metrics very little</strong>

そして題材で選ぶと <strong>下がる</strong>。

> "+Sim ctrl" (selecting examples by topic) <strong>surprisingly reduces attribution performance</strong>,
> especially in Enron, Reddit, and Blog

原因の推測も同じである。<strong>題材を絞ると文体の多様性が減る。</strong>

長さを揃えると推定は少し上がるが、文体モデルの精度は下がった。<strong>片方を上げると片方が
下がる。</strong>

> <strong>exemplar selection is far from trivial</strong>: strategies optimized for content or length do not
> always enhance stylistic imitation, and <strong>no single configuration consistently excels</strong>
> across all metrics

## 評価は 4 つを組み合わせている

1. 著者識別（authorship attribution）
2. 著者検証（authorship verification）
3. 文体の一致（style matching）
4. <strong>AI 検出</strong>——AI が書いたと判定されないか

4 つ目があるので、文体が合っていても AI くさければ低く出る。

<strong>4 つとも計算による判定であり、人が読んだ判断ではない。</strong> 論文自身が限界として
挙げている。

## 冒頭 50 語を渡すと「人間らしく」は見える

書き出しの 50 語を手本に入れると、<strong>人間らしさの判定は最も上がる</strong>。だが著者推定の結果は
まちまちだった。

<strong>人間らしく見えることと、その人らしいことは別である。</strong>

## 読み取れること

### 1. 領域によって結果が大きく違う

<strong>19〜66% 対 95〜97%。</strong> 同じ手法でも、対象の領域で成立したりしなかったりする。

> [!IMPORTANT]
> 記事側の 95% を根拠にしてはいけない。<strong>型を守れているだけかもしれない。</strong>
> [Wegmann](wegmann-2022.md) の「著者を当てられることは文体を表している証明にならない」
> と同じ罠である。

### 2. 手本の選び方は、まだ誰も解いていない

<strong>増やしても効かない。題材で選ぶと下がる。長さで揃えると片方が下がる。</strong>

論文自身が「単一の設定でどの指標も良くなることは無い」と書いている。<strong>手本の選び方は
未解決のまま残っている。</strong>

### 3. 文体の一致と、人間らしさは別である

<strong>GPT 系はほぼ 0% が人間と判定された。</strong> 文体が寄っていても、機械が書いたと分かる。
<strong>後者は前者から自動的には出てこない。</strong> 寄せる先の量は
[繰り返し・エントロピー・圧縮後の大きさ](zeng-nini-2026.md)にある。

### 4. 推奨されている方向

> Future work should explore <strong>richer personalization signals</strong> and <strong>hybrid prompting
> and/or finetuning</strong> strategies

<strong>何を渡せばよいかは、この論文の時点で分かっていない。</strong>

## 注意

- <strong>日本語ではない。</strong> 英語
- 評価は計算による判定であり、人が読んだ判断ではない
