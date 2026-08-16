# Thillainathan et al. (2026) AuthorMix

Modular Authorship Style Transfer via Layer-wise Adapter Mixing. Saarland University.
arXiv:2603.23069.
<https://arxiv.org/pdf/2603.23069>

<strong>全文を読んだ。「その人の文体で書かせる」の最新の到達点である。そして、思っていたより
ずっと低い。</strong>

## 何をしているか

<strong>目標の著者の文体に書き換える。</strong> 文体を再現させるという課題
である。

高資源の著者 10 人それぞれに LoRA アダプタを訓練しておき、新しい目標著者については
<strong>16 文だけの手本</strong>から <strong>層ごとの混合重み</strong> を学習する。

## 評価の設計——3 つを同時に見る

| 指標 | 何を見るか |
| --- | --- |
| Toward | 目標の文体にどれだけ寄ったか |
| MIS | 元の意味がどれだけ保たれたか |
| <strong>Joint</strong> | <strong>両者の幾何平均</strong> |

[文体転換の評価](tst-evaluation.md)の 3 次元がそのまま指標になっている。<strong>幾何平均なので、
片方が 0 に近ければ総合も 0 になる。</strong> 片方を犠牲にして稼げない。

## 結果——<strong>誰も文体を再現できていない</strong>

100 の（元著者 → 目標著者）対で平均した値。

| 手法 | Toward ↑ | MIS ↑ | Joint ↑ |
| --- | --- | --- | --- |
| 中立化しただけ（下限） | 0.01 | 0.79 | 0.05 |
| <strong>GPT-5.1 few-shot</strong> | <strong>0.08</strong> | 0.81 | 0.20 |
| STYLL | 0.07 | 0.68 | 0.16 |
| TinyStyler | 0.16 | 0.75 | 0.31 |
| ASTRAPOP-DPO | 0.17 | 0.63 | 0.29 |
| <strong>AuthorMix（GRPO 層ごと k=4）</strong> | 0.16 | 0.83 | <strong>0.34</strong> |

<strong>Toward の最高が 0.16 である。</strong> 文体の寄り方を見る指標が、どの手法でも 0.2 に届かない。
最先端が <strong>Joint 0.34</strong>。

<strong>few-shot の GPT-5.1 は 0.08。</strong> 中立化しただけの 0.01 よりはましだが、それだけである。

### 人手評価はもっと直接的

| 手法 | 意味保存 (0-2) | <strong>文体の正解率</strong>（偶然 = 0.5） | 流暢さ (0-1) |
| --- | --- | --- | --- |
| <strong>few-shot</strong> | 1.51 | <strong>0.40</strong> | 0.77 |
| STYLL | 1.27 | 0.63 | 0.87 |
| TinyStyler | 1.13 | 0.61 | 0.49 |
| ASTRAPOP-DPO | 0.99 | 0.57 | 0.79 |
| <strong>AuthorMix</strong> | <strong>1.61</strong> | <strong>0.63</strong> | <strong>0.89</strong> |

<strong>例文を見せるだけの few-shot は、文体の正解率 0.40——偶然を下回る。</strong>
人が見て、目標の著者に寄ったとは言えない。[Catch Me](catch-me-2025.md) の結論に、人手の数字が
付いた。

最良でも <strong>0.63</strong> である。

## <strong>本数はやはり 4 前後</strong>

アダプタの本数 k を 0 から 10 まで振っている。

| k | Joint |
| --- | --- |
| 0（素の LLM） | — |
| 1（最も近い 1 人） | 0.17 |
| 2 | 0.23〜0.32 |
| <strong>4</strong> | <strong>0.34</strong>（最高） |
| 6 まで | 競争力を保つ |
| <strong>7 以上</strong> | <strong>劣化する</strong> |

> This aligns with findings by Fisher et al. (2024) who report that <strong>mixing more than five
> adapters hurts grammaticality</strong>

<strong>[StyleRemix](styleremix-2024.md) と独立に、同じ本数に行き着いている。</strong>

> identifying a <strong>good subset of top-k</strong> stylistically similar authors is essential as it reduces
> the search space and ensures that each adapter contributes a meaningful stylistic signal

<strong>「上位 k 本を選ぶ」ことが本質的だと、隠す側と真似る側の両方が言っている。</strong>

## 混ぜる先は「軸」ではなく「他人」である

<strong>ここが StyleRemix と違う。</strong>

| | 混ぜる単位 |
| --- | --- |
| [StyleRemix](styleremix-2024.md) | <strong>文体の軸</strong>（長さ、機能語、丁寧さ……） |
| AuthorMix | <strong>他の著者</strong> |

AuthorMix は目標著者を「似ている 4 人の混合」として表す。<strong>解釈できるのは「誰に似ている
か」であって、「何が違うか」ではない。</strong>

<strong>「A さん 0.3 + B さん 0.5」という表現は、書き手に返しても直しようがない。</strong> 軸で混ぜる
側は、どの軸をどちらへ動かすかを言える。

## 読み取れること

### 1. 再現は未解決である

<strong>Toward 0.16、人手での文体正解率 0.63。</strong> 2026 年の最先端が、モデルを訓練してこれである。

<strong>1 回の生成で寄せきれるという報告は無い。</strong>

### 2. 意味保存で勝てる余地はある

AuthorMix の勝ち筋は Toward ではなく <strong>MIS 0.83</strong> だった。文体を追う手法はどれも意味を
壊している（ASTRAPOP 0.63、STYLL 0.68）。

<strong>「寄せると内容が壊れる」は、どの手法にも共通して出ている。</strong> 文体の一致だけを見る評価
では、この劣化が見えない。

### 3. 本数の上限が、独立に 2 本一致した

[StyleRemix](styleremix-2024.md) は 5 本以上で文法が 16% 落ちるとし、AuthorMix は 7 本
以上で劣化するとした。<strong>軸でも著者でも同じところに落ちる。</strong>

### 4. 手本は 16 文でよい

AuthorMix の目標著者は <strong>16 テキスト</strong>しかない。
[Przystalski](przystalski-2025.md) の「10 文で人と LLM は分かれる」と揃う。

<strong>目標側に要る文章の量は、この桁で足りている。</strong>

### 5. 英語である

Project Gutenberg の 30 著者。<strong>文学作品であり、英語である。</strong>
[金 2014](jin-2014.md) が「文学作品では F1 100、普通の人の日記では 98.4」と示したのと
同じ偏りがここにもある。
