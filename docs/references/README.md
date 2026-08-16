# 先行研究

kakiburi にあるのは、最初は <strong>やりたいこと</strong> だけである。

> LLM に代筆させたときに、自分が書いたものとして通るようにしたい。書くものの種類は
> 限らない。

これを満たすために何が要るかは、思いつきで決めない。まず何が分かっているかを調べ、
そこから [軸](../spec/000-axis.md) を決め、[戦略](../spec/010-strategy.md)、戦術と落とす。
<strong>独自性は要らない。結果が出ているものはそのまま採る。</strong>

関わる分野は 2 つある。

- <strong>著者識別（authorship attribution）</strong>——文章から書き手を当てる。60 年以上の蓄積
- <strong>SPTG（style-personalized text generation）</strong>——「write like me」。ここ数年の分野

## 読んだもの

### 測る側

| | 何が分かるか |
| --- | --- |
| [Stamatatos (2009) 総説](stamatatos-2009.md) | 特徴量の全体像。文体分類は話題分類と作法が逆 |
| [金 明哲 (2013) 文節パターン](jin-2013.md) | 日本語の実証。<strong>1,100 字で 99%、500 字で 92%</strong> |
| [浅石 (2017) 指標の概観](asaishi-2017.md) | 日本語の指標カタログ。効くもの、効かないもの |
| [Burrows's Delta](burrows-delta.md) | 判別の基準線。改良版は追わなくてよい。<strong>選抜は過学習する</strong> |

### 貸す側

| | 何が分かるか |
| --- | --- |
| [Wang et al. (2025)](wang-2025.md) | <strong>例文を見せるだけでは真似できない。増やしても頭打ち</strong> |
| [Bhandarkar et al. (2024)](bhandarkar-2024.md) | <strong>固定の特徴を指示すると、かえって悪くなる</strong> |
| [Kim & Jurgens (2026)](kim-jurgens-2026.md) | <strong>制御できることを説明とみなす。</strong> 埋め込み → 指示のデコーダ |
| [Jangra et al. (2025)](jangra-2025.md) | 評価は複数指標の組み合わせで |

### 日本語の資源

| | 何が分かるか |
| --- | --- |
| [馬場 (2022) 語の文体値データ](baba-2022.md) | <strong>語彙素ごとに 5 つの解釈できる値。既に公開されている</strong> |

## 未読

| | |
| --- | --- |
| [日本語の著者識別（その他）](japanese-attribution-others.md) | とくに <strong>品詞 n-gram の話題頑健性</strong> |

## 分かったこと

### 1. 測る側は解決済み。借りる

日本語でも、1,100 字の作文 11 人で 99%、500 字の日記 6 人で 92%
（[金 2013](jin-2013.md)）。使う特徴量ももう挙がっている——機能語の頻度、品詞 n-gram、
文字 n-gram、文節パターン、読点の打ち方、単語長と文長の分布。

<strong>自前で工夫する余地はない。</strong> それどころか、文体埋め込みが古典的な計量文体論を判別性能
で上回ることも明記されている（[Kim & Jurgens](kim-jurgens-2026.md)）。手作りの特徴量で
判別性能を競う道は、既に終わっている。

効かないと報告されているものもある。係り受け距離の分布、語彙の豊富さ（TTR）。試す前に
落とせる。

### 2. 貸す側は、素朴な手が両方とも失敗している

| 素朴な手 | 結果 |
| --- | --- |
| 例文を何本か見せる | 頭打ち。平均的な調子に寄り、AI 生成と検出される（[Wang](wang-2025.md)） |
| 文体の特徴を明示して指示する | <strong>例文だけより悪くなる</strong>（[Bhandarkar](bhandarkar-2024.md)） |

<strong>2 つ目が kakiburi の前提を直撃する。</strong> 指標を並べて渡す作りは、やってはいけない。

### 3. だが、失敗した実験が改善案を指している

[Bhandarkar](bhandarkar-2024.md) は、固定の特徴集合が全ての著者に等しく効くという前提
自体を疑っている。

> instead of static directed prompting, a more effective approach could involve
> <strong>dynamically prompting LLMs by considering each author's individual linguistic
> preferences</strong>

[Wang](wang-2025.md) の結びも同じ方向を向く（`richer personalization signals`）。
<strong>著者ごとに、その人に効く特徴を選んで渡す。</strong> どちらも、そこに答えがあるかもしれないと
書いて、試していない。

### 4. 貸す側にも既に手法がある。ただし別の道

[Kim & Jurgens (2026)](kim-jurgens-2026.md) は、文体埋め込みから自然言語の指示を復元
するデコーダを学習し、対象の文章を直接プロンプトに入れる方法を上回った。

<strong>「貸す側は未開拓」ではない。</strong> ただし出てくるのは自然言語の指示であって数値ではない。
したがって次ができない。

- <strong>検める</strong>——書かれたものが合っているかを決定的に測れない
- <strong>直し方を示す</strong>——どこがどれだけ外れているかが出ない
- 日本語で使えるモデルが要る。学習も要る。決定的でない

### 5. 日本語には、解釈できる資源が既にある

[馬場 (2022)](baba-2022.md) の『語の文体値データ』は、語彙素ごとに <strong>専門度・客観度・
硬度・くだけ度・語りかけ性度</strong> の 5 値を持つ。文章の側は語の値を平均するだけで出る。

[Stamatatos](stamatatos-2009.md) が「無い」と言った高い水準での言い表しが、日本語には
資源として存在していた。解釈でき、計算が軽く、決定的である。

### 6. 合否の判定は借りられる

[Kim & Jurgens](kim-jurgens-2026.md) の <strong>「制御できることを説明とみなす」</strong>。説明が
正しいかを、その説明で書かせて同じ文体になるかで測る。

kakiburi はこれまで合否を「本人が読んで受け入れる」に置いていた。主観的で、高くつき、
回数を稼げない。この基準なら機械で回せる。

そして [Jangra](jangra-2025.md) と [Wang](wang-2025.md) から、<strong>単一の点数に頼らない</strong>。
著者識別、著者検証、文体の一致、AI 検出の組み合わせで見る。

## kakiburi の立ち位置

判別で勝つ道具ではない。埋め込みに負ける。

生成の質だけを競う道具でもない。[Kim & Jurgens](kim-jurgens-2026.md) の方が強い。

<strong>残るのは、書かせて、検めて、直すループを回せることである。</strong> それには決定的で解釈
できる数値が要る。数値であることが、そのまま次の 3 つを可能にする。

- 貸すときの指示になる
- 検めるときの物差しになる
- <strong>両者が同じものを見ていることを保証できる</strong>

そして賭けているのは、[Bhandarkar](bhandarkar-2024.md) が提案して試さなかった
<strong>書き手ごとの選択</strong> である。指標を全部渡すのは害だと分かっている。選んで渡すことは、
まだ誰も試していない。

## 落とし穴として記録しておくこと

<strong>指標を並べて渡すと悪化する。</strong>（[Bhandarkar](bhandarkar-2024.md)）

<strong>題材の似た例を選ぶと悪化する。</strong>（[Wang](wang-2025.md)）貸すときに題材の近い例を選び
たくなるが、文体の多様性が減って逆効果になる。

<strong>特徴選抜は過学習する。</strong>（[Evert](burrows-delta.md)）コーパスの癖を著者の文体として
拾った実例がある。選抜して精度が上がっても信じてはいけない。

<strong>題材から完全には逃げられない。</strong>（[Stamatatos](stamatatos-2009.md) §5）機能語も文字
n-gram も題材情報を拾う。減らす設計は要るが、消しきれる前提では立てない。

<strong>単一の点数は騙せる。</strong>（[Jangra](jangra-2025.md)）前身でも、語を置換しただけの生成文
が 38 点から 94 点になった。

<strong>崩れた文章ほど難しい。</strong>（[Wang](wang-2025.md)）ニュースやメールより、ブログや
フォーラムの方が苦戦する。kakiburi の対象は難しい側にいる。

<strong>先行研究の対象は偏っている。</strong> 小説、作文、日記、英語のブログ。コードブロックや見出し
を含む記事、断片的なチャットで同じことが成り立つかは確かめられていない。<strong>ここは我々が
確かめるしかない。</strong>
