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

### kakiburi にとっての意味

<strong>これは仕様に書いていなかった危険である。</strong>

kakiburi は「書きぶりに寄せる」ことだけを見ようとしていた。だが指標に寄せるよう強く
指示すれば、<strong>書くべき内容が壊れるか、日本語として不自然になる</strong>。

[軸](../spec/000-axis.md)は「題材と場面を決めたあとに残る差」を扱うと定めている。つまり
<strong>題材は保存されていることが前提</strong> である。保存されているかを見ないなら、その前提が
成り立っているか分からない。

<strong>検めるときは 3 つを見る。</strong> 書きぶりが寄ったか、題材が保たれたか、読める日本語か。

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

kakiburi にとって。<strong>説明は数値が担う。判定の一部に LLM を混ぜるのはありうる。</strong>
ただし[数える側に LLM を置かない](../spec/010-strategy.md)という決定は、決定性のための
ものなので変わらない。合否の判定は決定的である必要が無い場面もある、という整理になる。

## 人手評価が理想だが高い

> human evaluation is often regarded as the standard for capturing subtle cues in style,
> it is <strong>expensive, time-intensive, and difficult to reproduce at scale</strong>

kakiburi が合否を「本人が読んで受け入れる」に置いていたときの問題そのものである。
自動の指標は人の判断の代理でしかないが、代理が要る理由もはっきりしている。

## 未読

本文の実験結果と、どの指標の組み合わせが最も人と相関したか。合否判定を設計するときに
読むこと。関連して Meta-Evaluation of Style and Attribute Transfer Metrics
（EMNLP 2025 Findings）も未読。
