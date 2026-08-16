# Sawant (2026) Theory-Grounded Evaluation Exposes the Authorship Gap

in LLM Personalization. arXiv:2604.26460.
<https://arxiv.org/pdf/2604.26460>

<strong>全文を読んだ。この調査でいちばん厳しい結果を出している 1 本である。</strong>
<strong>そして、なぜ他の論文がそれに気づかなかったのかまで示している。</strong>

## 何をしたか

Blog Authorship Corpus から 50 人を選び、4 つの個人化手法で <strong>1,000 件</strong>生成させた。
生成器は Qwen 3 32B。

| 手法 | 中身 |
| --- | --- |
| Non-Personalized | 内容の要約だけ（対照） |
| Few-Shot | その著者の 5 本 |
| Profile Extraction | 文体プロファイルを抽出してから、それだけで生成 |
| Contrastive | その著者 + 他の著者の対照例 + 文体特徴 |

<strong>要点は測り方である。</strong> 著者検証モデル LUAR を使うと、<strong>目盛りに絶対的な意味を持たせ
られる。</strong>

| 基準 | LUAR コサイン類似 |
| --- | --- |
| <strong>天井</strong>——同じ人の別の文章どうし | <strong>0.756</strong> |
| <strong>床</strong>——違う人どうし | <strong>0.626</strong> |

## <strong>結果——4 つとも「床」を下回った</strong>

| 手法 | LUAR |
| --- | --- |
| Non-Personalized | 0.484 |
| <strong>Few-Shot</strong> | <strong>0.508</strong> |
| Profile Extraction | 0.502 |
| Contrastive | 0.494 |
| <strong>床（他人どうし）</strong> | <strong>0.626</strong> |
| 天井（本人どうし） | 0.756 |

> <strong>personalized output is more distant from the target human author than random humans
> are from each other</strong>

<strong>個人化した出力は、赤の他人よりも、その人から遠い。</strong>

しかも <strong>4 手法の差はわずか 0.024</strong>。何をやっても同じである。GLM-4 32B に替えても
床を超えない。

## <strong>なぜそうなるのか——LLM 自身の指紋が消えない</strong>

生成文どうしでは、著者は区別できている。

| 何と何 | LUAR |
| --- | --- |
| Qwen の出力どうし | <strong>0.918</strong>（AUC） |
| Qwen と GLM の出力 | 0.753 |
| <strong>生成文と本物</strong> | <strong>0.45〜0.49</strong> |

> <strong>Each LLM carries its own authorship fingerprint that inference-time personalization does
> not erase</strong>

<strong>個人化は「LLM の文体空間の中で」書き分けているだけで、人間の文体の領域に入っていない。</strong>

[林・相澤](japanese-llm-style.md) がモデル固有表現を 98.2% で識別したのと、同じものを
別の角度から見ている。<strong>推論時の工夫ではモデルの指紋は消えない。</strong>

## <strong>そして、LLM を判定者にすると嘘の勝者が出る</strong>

LLM に「その著者の特徴 5 つ」を挙げさせ、生成文がそれを満たすかを数える指標（TMR）で
測ると、Profile Extraction が明確な勝者になる（効果量 d = 0.58）。<strong>LUAR では差が無い。</strong>

理由は循環である。

| | TMR |
| --- | --- |
| Profile Extraction | <strong>0.542</strong> |
| <strong>本物の著者本人の文章</strong> | <strong>0.427</strong> |

<strong>本人の文章より高い点が出ている。</strong>

> a method that scores higher than ground truth is measuring <strong>instruction-following, not
> authorship fidelity</strong>

判定者の特徴抽出と、手法のプロファイル抽出が、<strong>どちらも LLM に著者の文章を読ませて
特徴を挙げさせている。</strong> 同じ穴を掘って、同じ穴を測っている。

<strong>さらに、その特徴抽出自体が安定しない。</strong> 同じ著者に対して繰り返し抽出すると、
Jaccard 一致は <strong>0.22</strong>。

> <strong>The yardstick itself changes between measurements.</strong>

## 指標どうしは相関しない

| | LUAR | TMR | FuncCos |
| --- | --- | --- | --- |
| LUAR | 1.00 | | |
| TMR | 0.013 | 1.00 | |
| FuncCos | 0.026 | 0.067 | 1.00 |

<strong>3 つとも別のものを測っている。</strong>
[SPTG の評価](sptg-evaluation-2025.md)が「同じ点数で判断が 35% 食い違う」と言っていた
のと同じ現象である。

> without theoretical grounding, <strong>metric choice determines conclusions</strong>

## 機能語について——ここは慎重に読む

<strong>この論文の機能語指標（60 語のコサイン類似）は、著者を分けられなかった。</strong>

| | FuncCos |
| --- | --- |
| 天井 | 0.742 |
| 床 | 0.695 |
| <strong>差</strong> | <strong>0.047</strong> |

生成文は 0.741〜0.761 で、<strong>天井をまたいでいる。</strong>

> LLMs produce <strong>grammatically average function-word distributions regardless of
> personalization</strong>

> [!WARNING]
> <strong>これは「機能語が効かない」を意味しない。</strong> 測り方が違う。
> [Writeprints](writeprints-liwc.md) は 403 語、[Burrows's Delta](burrows-delta.md) は
> z 得点で 150〜1,000 語、[水上ほか](mizukami-2014.md) は日本語の助詞・助動詞・感動詞を
> 置換対象として使っている。<strong>60 語のコサインが弱かった、と読むのが正確である。</strong>
>
> ただし <strong>「LLM の機能語分布は平均的で、個人化しても動かない」</strong> は独立に重要である。
> [高橋ら](japanese-llm-style.md) の「制御できないスタイルがある」の一例と読める。

## 落とし穴——問題文の作り方で 28 ポイント変わる

生成の依頼文を <strong>本文の 1 文目から作る</strong>と、基準値が <strong>28 ポイント</strong>膨らむ。本人の文章の
書き出しがそのまま手掛かりになるからである。

<strong>依頼文は「何について書くか」の中立な要約にする。「どう書くか」を漏らさない。</strong>

## 読み取れること

### 1. <strong>目盛りには天井と床が要る</strong>

| 基準 | 何を測るか |
| --- | --- |
| <strong>天井</strong> | その人の別の文章どうしの一致 |
| <strong>床</strong> | その人と、他の人の一致 |

<strong>0.5 という数字には意味が無い。0.756 と 0.626 の間のどこにいるか、には意味がある。</strong>
較正が無ければ、どの手法がどれだけ届いていないかも言えない。

### 2. LLM に文体を挙げさせて、それで測ってはいけない

<strong>循環する。</strong> 抽出も判定も LLM なら、指示に従えたかを測っているだけになる。
<strong>本人の文章より高い点が出たら、その指標は壊れている。</strong>

<strong>そのまま検査に使える。</strong> 本人の実際の文章を採点して、生成文より低く出る指標は壊れて
いる。<strong>安価で、決定的である。</strong>

### 3. 抽出が安定しないなら、コアにならない

<strong>Jaccard 0.22。</strong> 同じ人から 2 回抽出して、2 割しか一致しない。

<strong>LLM に文体を読み取らせて作ったものは、測るたびに変わる。</strong> 物差しにするなら、
抽出そのものを決定的にするか、再現性を測って記録するしかない。

### 4. 到達点の見積もりが立った

| | LUAR |
| --- | --- |
| 現状の最良 | 0.508 |
| 他人どうし | 0.626 |
| 本人どうし | 0.756 |

<strong>今のところ、どの手法も床を超えていない。</strong>

### 5. 英語、ブログである

Blog Authorship Corpus、Qwen 3 32B と GLM-4 32B。<strong>日本語の検証は無い。</strong>
LUAR に相当する日本語の著者検証モデルも、公開されたものは見当たらない。
<strong>天井と床を作るには、まずそこが要る。</strong>
