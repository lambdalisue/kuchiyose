# 参考文献

「文章から書き手を定量的に識別する」は <strong>計量文体論 / 著者識別（stylometry,
authorship attribution）</strong> として蓄積がある。kakiburi はこの領域の結果を、できる
かぎりそのまま採る。

<strong>確度の表記。</strong> ◎ は本文または要旨を実際に読んで確かめたもの。○ は検索結果の
要約から拾ったもので、書誌情報と数値は未確認。採用前に原典に当たること。

## 総説

<strong>○ Stamatatos, E. (2009) A Survey of Modern Authorship Attribution Methods.</strong>
JASIST 60(3).
<https://icsdweb.aegean.gr/stamatatos/papers/survey.pdf>

この分野の標準的な総説。kakiburi にとって重要な記述が 2 つある。

- <strong>機能語は「書き手が概ね無意識に使い、題材に依存しない」</strong>。これは
  [軸](spec/000-axis.md)の性質 1（本人が意識していない）と性質 3（題材で動かない）
  と同じことを、先行研究の側から言っている
- <strong>文字 n-gram が語彙的特徴より有効</strong>。複数のタスクで最も効果的とされる

<strong>○ 浅石 卓真「テキストの特徴を計量する指標の概観」</strong> 日本図書館情報学会誌 63(3),
p.159-.
<https://www.jstage.jst.go.jp/article/jslis/63/3/63_159/_pdf>

日本語テキストの計量指標を文字・語・文・文書の単位で概観したもの。指標の棚卸しに
使える。<strong>未読</strong>（HTML 版が無く、PDF からテキストを取り出せなかった）。

## 距離・判別の手法

<strong>○ Burrows, J. (2002) 'Delta': A Measure of Stylistic Difference and a Guide to
Likely Authorship.</strong> Literary and Linguistic Computing 17(3), 267-287.
<https://academic.oup.com/dsh/article-abstract/17/3/267/929277>

著者識別の標準的な基準線。手順は単純である。

1. コーパス全体で頻度上位 N 語を取る（Burrows は 150 語。100〜5,000 でも機能する）
2. 各語の頻度を、コーパス全体の平均と標準偏差で z 化する
3. z 値の平均絶対差を距離とする

<strong>必要な長さ</strong>: 1,500 語を超える文章で有効。100 語程度でも候補の絞り込みには使える。
標本が小さくても壊れないのが利点で、記事 10〜50 本という kakiburi の条件に合う。

<strong>○ Hoover, D. (2004) Testing Burrows's Delta.</strong>
<https://mimno.infosci.cornell.edu/info3350/readings/delta.pdf>

Burrows が試した 150 語より <strong>多くの語を使う方が精度が上がる</strong>と報告。

<strong>○ Evert, S. et al. Towards a better understanding of Burrows's Delta /
Understanding and explaining Delta measures for authorship attribution.</strong>
<https://aclanthology.org/W15-0709.pdf>

Delta がなぜ効くのかの分析。距離尺度の選択について。

<strong>○ faststylometry</strong>（実装）
<https://github.com/fastdatascience/faststylometry>

Burrows's Delta の実装と解説ノート。手順の確認に使える。日本語には形態素解析を
前置きする必要がある。

## 日本語の著者識別

<strong>○ 松浦・金田 (2000)</strong> 文字 n-gram の分布で近代小説家 8 人を識別。
<https://cir.nii.ac.jp/crid/1520009410173965056>

<strong>○ 金 明哲 (2002)</strong> 助詞の n-gram モデルに基づいた書き手の識別。

<strong>◎ 金 明哲 (2013) 文節パターンに基づいた文章の書き手の識別.</strong> 行動計量学 40(1),
17-28.
<https://www.jstage.jst.go.jp/article/jbhmk/40/1/40_17/_pdf>

<strong>○ 品詞 n-gram を用いた著者推定手法 — 話題に対する頑健性の評価</strong>
<https://jglobal.jst.go.jp/detail?JGLOBAL_ID=201302291822438406>
（関連: 話題に依存しない頑健性の評価
<https://jglobal.jst.go.jp/detail?JGLOBAL_ID=201202292975845425>）

<strong>kakiburi にとって最も重要な 1 本。</strong> 品詞 n-gram が <strong>話題の違う文章でも高い精度で
著者を識別でき、既存手法より頑健である</strong>と報告している。これは
[条件 2（題材で動かない）](spec/100-metrics.md)に真正面から答える先行研究であり、
自前で確かめるより、この特徴量を採る方が早い。

<strong>○ 金川ら (2016)</strong> 係り受け解析の部分木による文体類似度。34 人の作家を対象。

<strong>○ 小泉・菅原 (2017)</strong> 部分木、Subset Tree、係り受けエッジ。10 名、青空文庫。
「どれを用いても同一著者の文どうしの類似度が最大になる比率が 0.5 未満」と報告。

<strong>◎ 著者識別における核文節関連情報を用いた文体特徴量の提案</strong>
<https://www.jstage.jst.go.jp/article/bdajcs/12/1/12_33/_html/-char/ja>

核文節関連情報（NBS）による特徴量。ランダムフォレストで <strong>2 群判別 98.61%、
10 群判別 94.75%</strong>。上に挙げた日本語研究の系譜が整理されているので、入口として
読むとよい。

## ここから採れる特徴量

先行研究で繰り返し使われている日本語の文体特徴量。

| 特徴量 | 出典 | kakiburi での位置づけ |
| --- | --- | --- |
| 文字 n-gram | 松浦・金田 2000、Stamatatos 2009 | 判別用の基準線 |
| 助詞の n-gram | 金 2002 | 機能語。題材に依存しない |
| 品詞 n-gram | 話題頑健性の評価 | <strong>題材への頑健性が確認済み</strong> |
| 文節パターン | 金 2013 | |
| 係り受けの部分木 | 金川 2016、小泉・菅原 2017 | 解析器が要る |
| 核文節関連情報 (NBS) | 上記 | 高精度だが実装は重い |
| 読点の打ち方 | 金（読点と文章の分類） | 出典を要確認 |
| 文字種の使用率（漢字・かな・カナ） | 複数 | |
| 文長分布 | 複数 | |

## 未確認のまま残っている点

- 上記のうち ◎ 以外は書誌情報と数値が未確認。原典に当たること
- <strong>対象が小説・文学作品に偏っている。</strong> kakiburi の対象は技術記事で、コードブロックや
  見出しを含む。同じ特徴量がそのまま効くかは確かめる必要がある
- 「AI が生成した文章」を基準に置く使い方の先行研究は、まだ調べていない
