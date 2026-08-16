# 先行研究

kakiburi にあるのは、最初は <strong>やりたいこと</strong> だけである。

> LLM に代筆させたときに、自分が書いたものとして通るようにしたい。書くものの種類は
> 限らない。

これを満たすために何が要るかは、思いつきで決めるものではない。まず何が分かっているか
を調べ、そこから [軸](../spec/000-axis.md) を決め、[戦略](../spec/010-strategy.md)、
戦術と落とす。<strong>独自性は要らない。結果が出ているものはそのまま採る。</strong>

関わる分野は 2 つある。

- <strong>著者識別（authorship attribution）</strong>——文章から書き手を当てる。60 年以上の蓄積
- <strong>SPTG（style-personalized text generation）</strong>——「write like me」。ここ数年の分野

## 読んだもの

### 測る側（著者識別）

| | 何が分かるか |
| --- | --- |
| [Stamatatos (2009) 総説](stamatatos-2009.md) | 特徴量の全体像。<strong>判別は解決済み、説明は未解決</strong> |
| [金 明哲 (2013) 文節パターン](jin-2013.md) | 日本語の実証。<strong>1,100 字で 99%、500 字で 92%</strong> |
| [浅石 (2017) 指標の概観](asaishi-2017.md) | 日本語の指標カタログ。効くもの、効かないもの |
| [Burrows's Delta](burrows-delta.md) | 判別の基準線。実装は数十行 |

### 貸す側（SPTG）

| | 何が分かるか |
| --- | --- |
| [Wang et al. (2025)](wang-2025.md) | <strong>例文を見せるだけでは真似できない。増やしても頭打ち</strong> |
| [Bhandarkar et al. (2024)](bhandarkar-2024.md) | <strong>固定の特徴を指示すると、かえって悪くなる</strong> |
| [Jangra et al. (2025)](jangra-2025.md) | 評価は複数指標の組み合わせで |

## 未読

| | |
| --- | --- |
| [日本語の著者識別（その他）](japanese-attribution-others.md) | とくに <strong>品詞 n-gram の話題頑健性</strong> |

## ここまでで分かったこと

<strong>1. 測る側は解決済みである。</strong>

日本語でも、1,100 字の作文 11 人で 99%、500 字の日記 6 人で 92%
（[金 2013](jin-2013.md)）。使う特徴量ももう挙がっている——機能語の頻度、品詞
n-gram、文字 n-gram、文節パターン、読点の打ち方、単語長と文長の分布。<strong>自前で工夫
する余地はない。借りる。</strong>

効かないと報告されているものもある。係り受け距離の分布、語彙の豊富さ（TTR）。試す前に
落とせる。

<strong>2. 貸す側は未解決で、しかも素朴な手はどちらも失敗している。</strong>

| 素朴な手 | 結果 |
| --- | --- |
| 例文を何本か見せる | 頭打ち。平均的な調子に寄り、AI 生成と検出される（[Wang](wang-2025.md)） |
| 文体の特徴を明示して指示する | <strong>例文だけより悪くなる</strong>（[Bhandarkar](bhandarkar-2024.md)） |

そして [Stamatatos](stamatatos-2009.md) が 2009 年に名指しした未解決——「著者の文体を
高い水準で言い表すことができない」——は、いまも解けていない。

<strong>3. だが、失敗した実験が同じ改善案を指している。</strong>

[Bhandarkar](bhandarkar-2024.md) は、固定の特徴集合が全ての著者に等しく効くという前提
自体を疑い、こう書いている。

> instead of static directed prompting, a more effective approach could involve
> <strong>dynamically prompting LLMs by considering each author's individual linguistic
> preferences</strong>

[Wang](wang-2025.md) の結びも同じ方向を向く。

> Future work should explore <strong>richer personalization signals</strong>

<strong>著者ごとに、その人に効く特徴を選んで渡す。</strong> どちらの論文も、そこに答えがあるかも
しれないと書いて、試していない。

<strong>4. したがって kakiburi の賭けはここにある。</strong>

- 測る側は借りる。工夫しない
- <strong>指標を全部並べて渡すことは、してはいけない。</strong> やった実験は失敗している
- <strong>書き手ごとに効く指標を選び、コーパスから出した数値とともに渡す。</strong> ここが未検証
  の領域であり、この道具が存在してよい理由

## 落とし穴として記録しておくこと

<strong>題材の似た例を選ぶと悪化する。</strong>（[Wang](wang-2025.md)）貸すときに題材の近い例を
選びたくなるが、文体の多様性が減って逆効果になる。

<strong>題材から完全には逃げられない。</strong>（[Stamatatos](stamatatos-2009.md) §5）機能語も文字
n-gram も題材情報を拾う。減らす設計は要るが、消しきれる前提では立てない。

<strong>次元を増やすと題材が混ざる。</strong> 最頻出の数十語は機能語だが、数百語を超えると内容語が
多数派になる。

<strong>評価は単一の点数に頼らない。</strong>（[Jangra](jangra-2025.md)）文体転換の分野では指標が
簡単に騙せることが報告されている。前身でも、語を置換しただけの生成文が 38 点から
94 点になった。

<strong>崩れた文章ほど難しい。</strong>（[Wang](wang-2025.md)）ニュースやメールより、ブログや
フォーラムの方が苦戦する。kakiburi の対象は難しい側にいる。

<strong>先行研究の対象は小説・作文・日記・英語ブログに偏っている。</strong> コードブロックや見出しを
含む記事、断片的なチャットで同じことが成り立つかは確かめられていない。ここは我々が
確かめるしかない。
