# 文体転換（TST）の評価

Mukherjee et al. (2025) Evaluating Text Style Transfer Evaluation: Are There Any Reliable
Metrics? arXiv:2502.04718.
<https://arxiv.org/pdf/2502.04718> / <https://github.com/souro/tst_evaluation>

<strong>要旨と序論を読んだ。</strong> 評価の設計に効く。

## 評価は 3 次元である

文体転換の評価には、必ずこの 3 つが要る。

| | 何を見るか | よく使われる測り方 |
| --- | --- | --- |
| <strong>文体の強さ</strong> | 狙った文体になっているか | 文体の判別器にかける |
| <strong>内容の保存</strong> | 元の内容が残っているか | BLEU、埋め込みの類似度 |
| <strong>自然さ</strong> | 日本語として読めるか | 言語モデルの困惑度 |

<strong>そして 3 つの間には trade-off がある。</strong> 文体を強く寄せるほど、内容が壊れ、自然さが落ちる。

### 文体だけを見ると何が起きるか

<strong>文体に強く寄せるほど、内容が壊れ、自然さが落ちる。</strong> 文体の一致だけを測っている
評価は、この 2 つの劣化を見落とす。

## LLM を判定に使うことについて

> LLM-based evaluations provide better insights than existing TST metrics. Our oracle
> ensemble approaches show even more potential.

<strong>LLM を判定者にすると、既存の指標より人の判断とよく相関する。</strong> 組み合わせるとさらに
良い。

[Residualized Similarity](residualized-similarity-2025.md) は「LLM の説明は推論過程を
表していない」と警告していた。<strong>矛盾しない。</strong> 別のことを言っている。

| 使い方 | 評価 |
| --- | --- |
| LLM に <strong>判定</strong> させる（合っているか） | 人の判断とよく相関する |
| LLM に <strong>説明</strong> させる（なぜか） | <strong>推論過程を表していない</strong> |

<strong>判定と説明は別の用途である。</strong> 判定に使えることは、説明に使えることを意味しない。

## 人手評価が理想だが高い

> human evaluation is often regarded as the standard for capturing subtle cues in style,
> it is <strong>expensive, time-intensive, and difficult to reproduce at scale</strong>

人手評価に頼る設計が抱える問題である。
自動の指標は人の判断の代理でしかないが、代理が要る理由もはっきりしている。

## 未読

本文の実験結果と、どの指標の組み合わせが最も人と相関したか。使うと決めたときに読むこと。関連して Meta-Evaluation of Style and Attribute Transfer Metrics
（EMNLP 2025 Findings）も未読。
