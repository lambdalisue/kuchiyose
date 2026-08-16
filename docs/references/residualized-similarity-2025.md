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

## 読み取れること

<strong>1. LLM の説明は、その LLM の推論過程を表していない。</strong> 論文が明言している。「なぜ
そう判定したか」を LLM に語らせても、判定の根拠にはならない。

<strong>2. 説明と精度を別の担い手に割り当てる構えである。</strong> 解釈できる特徴が説明を担い、
強い手法が精度を担う。この論文は両者を 1 つの予測器に統合しているが、役割は分けている。

## 未読

手法の詳細、どの解釈可能特徴を使ったか、精度。合否判定を設計するときに読むこと。
