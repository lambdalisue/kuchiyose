# 先行研究

kakiburi にあるのは、最初は <strong>やりたいこと</strong> だけである。

> LLM に代筆させたときに、自分が書いたものとして通るようにしたい。書くものの種類は
> 限らない。

これを満たすために何が要るかは、思いつきで決めない。まず何が分かっているかを調べ、
そこから [軸](../spec/000-axis.md) を決め、[戦略](../spec/010-strategy.md)、戦術と落とす。

<strong>kakiburi は研究ではない。</strong> 新しさは要らない。<strong>使えるものを使う。</strong>

関わる分野は 3 つある。

- <strong>著者識別（authorship attribution）</strong>——文章から書き手を当てる。60 年以上の蓄積
- <strong>SPTG（style-personalized text generation）</strong>——「write like me」。ここ数年の分野
- <strong>機械生成文の検出</strong>——基準を LLM の出力に置くことの裏付け

## 読んだもの

### 測る

| | 何が分かるか |
| --- | --- |
| [Stamatatos (2009) 総説](stamatatos-2009.md) | <strong>特徴は頻度で選ぶ。判別力で選ぶのは話題分類の作法</strong> |
| [金 明哲 (2013) 文節パターン](jin-2013.md) | 日本語の実証。<strong>1,100 字で 99%、500 字で 92%</strong> |
| [柳・金 (2023) 核文節](yanagi-jin-2023.md) | <strong>文節パターン B 型の定義。前処理の規則</strong> |
| [浅石 (2017) 指標の概観](asaishi-2017.md) | 日本語の指標カタログ。効くもの、効かないもの |
| [Burrows's Delta](burrows-delta.md) | 基準線。改良版は追わなくてよい。<strong>選抜は過学習する</strong> |
| [井上・山名ほか（未読）](japanese-attribution-others.md) | 品詞 n-gram の話題頑健性ほか |

### 貸す

| | 何が分かるか |
| --- | --- |
| [Wang et al. (2025)](wang-2025.md) | <strong>例文を見せるだけでは真似できない。増やしても頭打ち</strong> |
| [Bhandarkar et al. (2024)](bhandarkar-2024.md) | <strong>固定の特徴を指示すると、かえって悪くなる</strong> |
| [Zeng & Nini (2026)](zeng-nini-2026.md) | <strong>失敗の原因は繰り返しの不足。しかも測れる</strong> |
| [Kim & Jurgens (2026)](kim-jurgens-2026.md) | <strong>制御できることを説明とみなす</strong> |
| [Jangra et al. (2025)](jangra-2025.md) | 評価は複数指標の組み合わせで |
| [Residualized Similarity (2025)](residualized-similarity-2025.md) | LLM の説明は推論過程を表していない |

### 基準に使う

| | 何が分かるか |
| --- | --- |
| [Przystalski et al. (2025)](przystalski-2025.md) | <strong>10 文でも人と LLM は分かれる。基準はモデルごとに違う</strong> |

### 日本語の資源

| | 何が分かるか |
| --- | --- |
| [中俣 多次元的抽出](nakamata-multidimensional.md) | <strong>客観的に導いた 4 つの文体軸。解釈できる指標の本命</strong> |
| [馬場 語の文体値データ](baba-2022.md) | 13 万語の表。<strong>2 次元。向きが名前と逆</strong> |
| [有馬ら (2018) 文末表現辞書](arima-2018.md) | 文末表現の抽出規則。敬体/常体/会話体/ネット文体 |
| [mStyleDistance (2025)](mstyledistance-2025.md) | 日本語を含む文体埋め込み。ただし日本語部分は翻訳由来 |

## 分かったこと

### 1. 測る側は借りる

日本語でも、1,100 字の作文 11 人で 99%、500 字の日記 6 人で 92%
（[金 2013](jin-2013.md)）。使う特徴量ももう挙がっている。手作りの特徴量で判別の精度を
追う理由は無い。文体埋め込みが古典的な計量文体論を上回ることも明記されている
（[Kim & Jurgens](kim-jurgens-2026.md)）。

<strong>特徴は頻度で選ぶ。</strong>（[Stamatatos §2.6](stamatatos-2009.md)）判別力で選抜するのは話題
分類の作法であり、文体分析では劣る。しかも選抜された特徴はコーパス依存になる。

効かないと報告されているものもある。係り受け距離の分布、語彙の豊富さ（TTR）。試す前に
落とせる。

### 2. 貸す側は、素朴な手が両方とも失敗している

| 素朴な手 | 結果 |
| --- | --- |
| 例文を何本か見せる | 頭打ち。平均的な調子に寄り、AI 生成と検出される（[Wang](wang-2025.md)） |
| 文体の特徴を並べて指示する | <strong>例文だけより悪くなる</strong>（[Bhandarkar](bhandarkar-2024.md)） |

<strong>2 つ目が kakiburi の前提を直撃する。</strong> 指標を全部並べて渡す作りは、やってはいけない。

### 3. 失敗の原因が特定されていて、しかも測れる

[Zeng & Nini](zeng-nini-2026.md) が、なりすましがなぜ見破られるかを突き止めている。

> Human-authored texts naturally contain <strong>recurring ... n-grams that reflect an
> individual's habitual phrasing</strong>. Because LLM-generated texts exhibit <strong>higher entropy and
> greater lexical diversity</strong>, they fail to produce sufficient repetitions

<strong>人の文体は繰り返しでできている。LLM は語彙が散りすぎて繰り返せない。</strong>

圧縮後の大きさ、エントロピー、Type-Token 比。3 つとも、なりすまし文の方が高かった。
<strong>寄せるべき方向がはっきりしている。</strong>

そして温度と頻度ペナルティは既定値のままだったと論文自身が認めている。<strong>プロンプト以外
に手がある。</strong>

### 4. 失敗した実験が、同じ改善案を指している

[Bhandarkar](bhandarkar-2024.md) は、固定の特徴集合が全ての著者に等しく効くという前提
自体を疑っている。

> instead of static directed prompting, a more effective approach could involve
> <strong>dynamically prompting LLMs by considering each author's individual linguistic
> preferences</strong>

[Wang](wang-2025.md) の結びも同じ方向を向く（`richer personalization signals`）。
<strong>著者ごとに、その人に効く特徴を選んで渡す。</strong> どちらも、そこに答えがあるかもしれないと
書いて、試していない。

### 5. 解釈できる軸は、日本語に既にある

[中俣](nakamata-multidimensional.md) が Biber 流の因子分析を日本語に適用し、880 サンプル
× 89 の文法形式から 4 因子を取り出している。

| | 軸 | 説明力 |
| --- | --- | --- |
| D1 | 共感的対話 対 客観的伝達 | 0.556 |
| D2 | 語り 対 非語り | 0.184 |
| D3 | 新情報の解説 | 0.161 |
| D4 | 聞き手への配慮 | 0.099 |

<strong>ジャンルのラベルを使わずに導いている。</strong> D1・D2 は Biber の英語の次元と一致しており、
言語をまたいだ頑健性がある。

[馬場のデータ](baba-2022.md)も、実データで相関を測ったところ <strong>2 次元</strong>だった（硬さと
語り性）。<strong>主観から作った表と、客観的な因子分析が、上位 2 軸で一致している。</strong>

### 6. 合否の判定も借りられる

[Kim & Jurgens](kim-jurgens-2026.md) の <strong>「制御できることを説明とみなす」</strong>。説明が
正しいかを、その説明で書かせて同じ文体になるかで測る。

kakiburi はこれまで合否を「本人が読んで受け入れる」に置いていた。主観的で、高くつき、
回数を稼げない。この基準なら機械で回せる。

そして [Jangra](jangra-2025.md) と [Wang](wang-2025.md) から、<strong>単一の点数に頼らない</strong>。

## kakiburi が作るもの

判別の精度を追う道具ではない。そこは既にあるものを借りる。

<strong>作るのは、書かせて、検めて、直すループである。</strong> それには決定的で解釈できる数値が要る。
数値であることが、そのまま 3 つを可能にする。

- 貸すときの指示になる
- 検めるときの物差しになる
- <strong>両者が同じものを見ていることを保証できる</strong>

材料は揃っている。[中俣の 4 軸](nakamata-multidimensional.md)、
[馬場の 2 軸](baba-2022.md)、[文節パターン B 型](yanagi-jin-2023.md)、
[文末表現](arima-2018.md)、機能語と句読点。

そして賭けているのは、[Bhandarkar](bhandarkar-2024.md) が提案して試さなかった
<strong>書き手ごとの選択</strong> である。全部渡すのは害だと分かっている。選んで渡すことは、まだ誰も
試していない。

## 落とし穴として記録しておくこと

<strong>指標を並べて渡すと悪化する。</strong>（[Bhandarkar](bhandarkar-2024.md)）

<strong>題材の似た例を選ぶと悪化する。</strong>（[Wang](wang-2025.md)）題材の近い例を選びたくなるが、
文体の多様性が減って逆効果になる。

<strong>特徴選抜は過学習する。</strong>（[Evert](burrows-delta.md)）コーパスの癖（ローマ数字、歴史的
仮名遣い）を著者の文体として拾った実例がある。

<strong>基準は使う LLM ごとに違う。</strong>（[Przystalski](przystalski-2025.md)）モデルによって効く
特徴が違う。どのモデルで作った基準かを記録する。

<strong>解析器と対象がずれると精度が落ちる。</strong>（[柳・金](yanagi-jin-2023.md)）新聞で訓練した
解析器を旧字体の小説に当てると壊れる。技術記事もチャットも新聞ではない。

<strong>題材から完全には逃げられない。</strong>（[Stamatatos §5](stamatatos-2009.md)）機能語も文字
n-gram も題材情報を拾う。減らす設計は要るが、消しきれる前提では立てない。

<strong>単一の点数は騙せる。</strong>（[Jangra](jangra-2025.md)）前身でも、語を置換しただけの生成文
が 38 点から 94 点になった。

<strong>崩れた文章ほど難しい。</strong>（[Wang](wang-2025.md)）ニュースやメールより、ブログや
フォーラムの方が苦戦する。kakiburi の対象は難しい側にいる。

<strong>先行研究の対象は偏っている。</strong> 小説、作文、日記、英語のブログ。コードブロックや見出し
を含む記事、断片的なチャットで同じことが成り立つかは確かめられていない。<strong>ここは我々が
確かめるしかない。</strong>

## 使わせてもらう資源

<strong>『語の文体値データ』</strong>（馬場 2022, CC BY-NC-ND 3.0）
DOI: <https://doi.org/10.15084/00003532>

<strong>『BCCWJ 図書館サブコーパスの文体情報』</strong>（柏野 2015）
DOI: <https://doi.org/10.15084/00003109>
