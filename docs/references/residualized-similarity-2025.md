# Residualized Similarity for Faithfully Explainable Authorship Verification (2025)

arXiv:2510.05362.
<https://arxiv.org/pdf/2510.05362>

<strong>要旨を読んだ。</strong> 解釈できる特徴と、ニューラルの精度を、両立させようとしている。

## 問題設定

著者検証を実務で使うには、精度だけでなく <strong>説明</strong> が要る。判断が現実に影響する場面では、
予測が <strong>元の文章まで辿れる解釈可能な特徴</strong> で説明できなければならない。

- ニューラル手法は精度が高いが、表現が解釈できない
- <strong>LLM の説明は忠実でない</strong>

> LLM predictions cannot be explained faithfully – if there is an explanation given for a
> prediction, <strong>it doesn't represent the reasoning process behind the model's prediction</strong>

## 提案

<strong>残差類似度（residualized similarity, RS）</strong>。解釈可能な特徴による判定を主とし、その
<strong>残差</strong>をニューラルネットワークで補う。

解釈可能な部分は元の文章に辿れるまま、精度だけを上げる。

## kakiburi にとって

<strong>1. LLM に説明させてはいけない、という警告。</strong>

kakiburi は「どこが違うか」を出す道具である。それを LLM に判定させたくなるが、この論文
は <strong>LLM の説明は推論過程を表していない</strong>と明言している。
[数える側に LLM を置かない](../spec/010-strategy.md)という判断の裏付けになる。

<strong>2. 構造が近い。</strong>

kakiburi も、解釈できる指標を主に置き、判別の物差しとして強い手法を横に置く構えである。
この論文は両者を 1 つの予測器に統合しているが、発想は同じ——<strong>説明は解釈可能な側が担い、
精度は別の側が担う。</strong>

違いは目的である。向こうは 1 つの判定を出したい。kakiburi は <strong>直し方を出したい</strong>ので、
統合せず分けたまま使う方が合う。

## 未読

手法の詳細、どの解釈可能特徴を使ったか、精度。合否判定を設計するときに読むこと。
