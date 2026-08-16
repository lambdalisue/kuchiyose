# Can You Make It Sound Like You? (2026)

Post-Editing LLM-Generated Text for Personal Style. arXiv:2604.24444.
<https://arxiv.org/html/2604.24444v1>

<strong>全文を読んだ。LLM の草稿を本人が自分らしく直す、という作業を 81 人でやった実験で
ある。</strong>

## 何をしたか

<strong>81 人</strong>に、文体が大事な場面の文章を書かせた。結婚式の誓い、謝罪文、弔辞、お悔やみなど
8 種から 6 つを選ばせている。

| 条件 | 手順 |
| --- | --- |
| 対照 | 内容を計画 → <strong>自分で書く</strong> |
| 処置 | 内容を計画 → GPT-4o mini の草稿を受け取る → <strong>自分らしくなるよう直す</strong> |

文体は LUAR 埋め込みで測り、AI 検出は Pangram、主観は 5 段階で訊いている。

## 結果 1——直すと、たしかに寄る

| | 効果量 |
| --- | --- |
| 自分の文章との類似が上がった | g = 0.55 |
| LLM の文章との類似が下がった | g = −0.41 |
| AI 検出のスコアが下がった | g = −0.45 |

<strong>直す作業には効果がある。</strong>

## <strong>結果 2——それでも、自分の文章より LLM の文章に近いまま</strong>

> post-edited text remained <strong>significantly more stylistically similar to LLM-generated text
> than to their unassisted control text</strong> (p = .0002, <strong>g = −1.43</strong>)

<strong>効果量 1.43。</strong> 寄せた分（0.55）よりはるかに大きい差が残っている。

<strong>人が手で直しても、LLM の文体は落ちない。</strong>

## <strong>結果 3——本人はそれに気づかない</strong>

> Participants perceived post-edited text as <strong>just as representative of their personal style as
> their control writing</strong> (p = .9062, g = 0.01)

<strong>主観では、自分で書いたものと区別が付いていない。</strong>

LUAR の測定値と、本人が感じる「自分らしさ」の相関は <strong>r = 0.244</strong>。ほとんど無い。

論文の解釈は 2 つ挙がっている。<strong>人は特定の語（`delve` など）や記号（em dash）だけを見て
判断していて、分布としての文体を見ていない。</strong> あるいは埋め込みが、統計的には目立つが
主観的には無関係なものを拾っている。

> <strong>User intuitions about personal style may be unreliable guides</strong> for AI personalization
> systems

## 人が実際にやった直し

| 直し | 例 |
| --- | --- |
| <strong>LLM 臭い記号・語を消す</strong> | em dash を削る、`delve` を消す |
| 短縮形を足す | `it is` → `it's` |
| 飾りを削る | 大げさな言い回しを平らにする |
| 地域・文化に合わせる | 英国綴り、弔辞の宗教的表現 |
| 内容の修正 | <strong>処置群の約 31% で事実の直しが入った</strong> |

そして、<strong>びっしり直しても、まばらに直しても、埋め込み上の類似は変わらなかった。</strong>

## 読み取れること

### 1. <strong>本人は自分の文体を判定できない</strong>

主観では自分の文章と区別が付かず、埋め込みとの相関は r = 0.244 しかない。

[SPTG の評価](sptg-evaluation-2025.md)は評価者間の一致が 0.641 だと示している。
<strong>この論文は、本人ですら判定できないことを示した。</strong>

### 2. 「直す」段は必要だが、それだけでは足りない

<strong>g = 0.55 寄せて、g = 1.43 残る。</strong> 直しは効くが、片付かない。

[AuthorMix](authormix-2026.md) の「1 回の生成では寄らない」と合わせると、
<strong>生成でも直しでも 1 回では足りない</strong>ということになる。

### 3. 人が気づいて直すのは、表層の層である

<strong>記号（em dash）、短縮形、飾りの多さ。</strong> 人が自分で気づいて直したのは、
<strong>表層の表記と語彙の癖</strong> だった。

[Wegmann](wegmann-2022.md) の凝集クラスタが、末尾の句読点・アポストロフィの字種・
改行で分かれたのと同じ場所である。<strong>日本語なら、三点リーダの字数、感嘆符の全角半角、
括弧の種類、和欧間スペース、読点の打ち方。</strong>

<strong>本人が気づける層と、気づけない層がある。</strong> 表層の表記には気づく。分布としての文体に
は気づかない。

### 4. 直すと文体が均される

処置群の文章は、対照群より <strong>互いに似通っていた</strong>（g = 1.42）。LLM の生の出力ほどでは
ない（g = −0.69）が、人が自由に書いたときより多様性が減る。

<strong>「直す」経路は、書き手の間の差を潰す方向に働く。</strong>

### 5. 英語である

参加者は英語話者、素材は誓いの言葉や弔辞である。日本語や他の素材での再現は無い。
