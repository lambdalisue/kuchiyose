# Evaluating Style-Personalized Text Generation (2025)

Challenges and Directions. arXiv:2508.06374.
<https://arxiv.org/abs/2508.06374>

<strong>全文を読んだ。この分野の評価がどう行われているかを整理した 1 本である。</strong>

## 分野の名前

> style-personalized text generation——<strong>"write like me"</strong>——has become a rapidly growing
> area

<strong>SPTG。</strong>「LLM に特定の書き手の文体で書かせる」課題には、この名前が付いている。

## 評価は 2 択にする

評価を「どれくらい似ているか」ではなく、<strong>「2 つのうちどちらが参照文に近いか」</strong>で測る。

> reduces the problem to determining <strong>if the metrics can measure style accuracy, rather
> than to what extent</strong>

3 つの設定で測る。

| 設定 | 何を区別させるか |
| --- | --- |
| DD | 同じ著者、違う領域 |
| AA | 同じ領域、違う著者 |
| <strong>LLM</strong> | <strong>同じモデルの、個人化した出力と、していない出力</strong> |

## 結果——単独で信頼できる指標は無い

| 指標 | 平均正解率 |
| --- | --- |
| BLEU | 0.733 |
| StyleDistance（文体埋め込み） | 0.722 |
| <strong>gpt-4.1 を判定者に</strong> | <strong>0.815</strong> |
| <strong>組み合わせ（性能加重投票）</strong> | <strong>0.821</strong> |

> <strong>Ensembles of SPTG metrics are more effective than any SPTG metric alone.</strong>

最良の組み合わせは <strong>BLEU + ROUGE-1 + StyleDistance + gpt-4.1</strong>。
<strong>同じ系統の指標を足しても意味が無く、違う原理のものを混ぜる。</strong>

### 同じ点数でも、違うものを測っている

> ROUGE-1 and StyleDistance achieve the same overall performance score of 0.722;
> their <strong>disagreement score is 0.35</strong>

<strong>正解率が同じで、判断が 35% 食い違う。</strong> 総合点だけ見て指標を選ぶと、何を測っているか
分からないまま採ることになる。

## 効かないもの

<strong>文体埋め込みは、LLM の出力を判定する場面で崩れる。</strong>

> Performance dropped <strong>38.3%</strong> from domain discrimination to LLM settings

理由もはっきりしている。<strong>2 つの候補が同じ依頼から生成されているので、内容が同じであり、
内容に頼っている埋め込みは区別できない。</strong>
[Wegmann](wegmann-2022.md) の「文体表現は題材を優先している」が、実務上の失敗として
出た形である。

<strong>LLM を判定者にする場合、小さいモデルは使えない。</strong> Ministral-3B と Llama-3.1-8B は
ほぼ偶然の水準。

<strong>そして構造化出力を求めると 24% 悪化した。</strong>

> Structured output prompts performed <strong>24% worse</strong> than open-ended approaches

## 人間も一致しない

| 何についての好み | 評価者間の一致 |
| --- | --- |
| 内容 | 0.779 |
| <strong>文体</strong> | <strong>0.641</strong> |
| 推敲 | <strong>0.400</strong> |

> Annotators <strong>had difficulty distinguishing between the personalized and non-personalized
> responses</strong>, often labeling "Both"

<strong>人が読んでも、個人化してあるかどうか分からないことが多い。</strong>

## 読み取れること

### 1. 判定は組み合わせで行う。1 つの数字にしない

[Jangra](jangra-2025.md) の「単一の点数は騙せる」に、<strong>組み合わせ方の具体案</strong>が付いた。
<strong>違う原理の指標を、性能で重みを付けて投票させる。</strong>

<strong>ただし埋め込みは、同じ依頼から作った 2 つを比べる場面では効かない。</strong> 内容が同じなので
区別できず、性能が 38.3% 落ちる。

### 2. 「本人が読んで判断する」は当てにできない

<strong>文体の一致度 0.641、推敲の一致度 0.400。</strong> 人の判断自体がばらつく。

<strong>人手評価を基準に据える設計は、この一致度の上に建つことになる。</strong>

### 3. 構造化した形を求めると成績が落ちる

<strong>開かれた形より 24% 悪い。</strong> 判定側の実測である。

<strong>[Bhandarkar](bhandarkar-2024.md) の「特徴を並べると悪化する」と合わせると、
数値を表で並べるより散文で渡すほうがよい、という方向を指す。</strong>

### 4. 既存の基準は、この課題を測っていない

LaMP と LongLaMP は ROUGE と METEOR で測っている。<strong>n-gram の重なりは騙しやすく、
人の判断と相関しない。</strong>

> Prior approaches focused on <strong>sentence-level style transfer, which does not align with
> contemporary SPTG in long-form, low-resource scenarios</strong>

<strong>長文で、素材が少ない設定</strong>が、既存の基準では覆われていない。
