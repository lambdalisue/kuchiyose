# 先行研究

<strong>調べた問い。</strong>

> ある人の書きぶりを機械で測れるか。測れるとして、それを使って別の書き手（人でも LLM
> でも）にその文体を再現させられるか。日本語で成り立つか。

<strong>ここは調査の結果だけを書く。</strong> 事実と、その出どころと、確かめられていないこと。
何をどう作るかの判断はここではしない（[仕様](../spec/)がする）。

分野は 3 つにまたがる。

- <strong>著者識別（authorship attribution）</strong>——文章から書き手を当てる。60 年以上の蓄積
- <strong>SPTG（style-personalized text generation）</strong>——「write like me」。ここ数年の分野
- <strong>機械生成文の検出</strong>——人が書いたものと機械が書いたものを分ける

## 読んだもの

### 測定・著者識別

| | 何が分かるか |
| --- | --- |
| [Stamatatos (2009) 総説](stamatatos-2009.md) | <strong>特徴は頻度で選ぶ。判別力で選ぶのは話題分類の作法</strong> |
| [財津・金 (2018) 判定手続きの標準化](zaitsu-2018.md) | <strong>「判定不能」を置けば感度 100%。日本語ブログ 100 人</strong> |
| [尤度比による著者比較 (2026)](fusing-lr-2026.md) | <strong>日本語ブログ 2,287 人の目盛り。混ぜると効く。Cllr 0.325</strong> |
| [日本語の著者推定 近年 2 本](japanese-attribution-recent.md) | <strong>単独で弱い指標が混ぜると効く。60 字だと崩れる</strong> |
| [金 明哲 (2014) 統合的分類](jin-2014.md) | <strong>採るべき 4 種。文長・漢字率・語彙の豊富さは効かない</strong> |
| [金 明哲 (2013) 文節パターン](jin-2013.md) | 日本語の実証。<strong>1,100 字で 99%、500 字で 92%</strong> |
| [柳・金 (2023) 核文節](yanagi-jin-2023.md) | <strong>文節パターン B 型の定義。前処理の規則</strong> |
| [柳・金 (2022) 異ジャンル混在](yanagi-jin-2022.md) | <strong>場面をまたぐと何が残るか。ジャンルは器、個人文体は流体</strong> |
| [浅石 (2017) 指標の概観](asaishi-2017.md) | 日本語の指標カタログ。効くもの、効かないもの |
| [Burrows's Delta](burrows-delta.md) | 最も確立した基準線。<strong>Cosine Delta が明確に上回る。効いているのはベクトル正規化</strong> |
| [Writeprints と LIWC](writeprints-liwc.md) | 既製の特徴量セット。<strong>2008 年から著者ごとの特徴量セット</strong> |
| [Wegmann et al. (2022)](wegmann-2022.md) | <strong>著者を当てられることは、文体を表している証明にならない</strong> |
| [Neurobiber (2025)](neurobiber-2025.md) | <strong>解釈できる 96 次元が、埋め込みとほぼ並ぶ</strong> |
| [日本語の系譜と読み残し](japanese-attribution-others.md) | <strong>読点は 3 層。規範／個人／強調</strong>。ほか未入手の 1 本 |

### 文体の再現

| | 何が分かるか |
| --- | --- |
| [Sawant (2026) 著者性の隔たり](authorship-gap-2026.md) | <strong>個人化した出力は、赤の他人より遠い。天井と床で測る</strong> |
| [Wang ら (2025) Catch Me If You Can](catch-me-2025.md) | <strong>普通の人のブログ・掲示板では 19〜66%。記事は 95%。例文を増やしても効かない</strong> |
| [AuthorMix (2026)](authormix-2026.md) | <strong>最先端でも文体は寄らない。軸は 4 本が頂点</strong> |
| [Post-Editing (2026)](post-editing-2026.md) | <strong>人が手で直しても LLM 文体は落ちない。本人は気づかない</strong> |
| [StyleRemix (2024)](styleremix-2024.md) | <strong>書き手ごとの軸選択は実装済み。無作為より 6% 良い</strong> |
| [SICO (2024)](sico-2024.md) | <strong>「人らしく」なら 40 本で解ける。差から作った記述を散文で渡す</strong> |
| [Masks and Mimicry (2025)](masks-mimicry-2025.md) | <strong>手本は 3 本が最良。成功率は書き手で 0.11〜0.77</strong> |
| [Bhandarkar et al. (2024)](bhandarkar-2024.md) | <strong>固定の特徴を指示すると、かえって悪くなる</strong> |
| [Zeng & Nini (2026)](zeng-nini-2026.md) | <strong>失敗の原因は繰り返しの不足。しかも測れる</strong> |
| [Kim & Jurgens (2026)](kim-jurgens-2026.md) | <strong>制御できることを説明とみなす</strong> |
| [高橋ら (2025) 日本語スタイル制御](japanese-llm-style.md)（日本語 × LLM の 1 本目） | <strong>制御できるスタイルと、できないスタイルがある</strong> |
| [Residualized Similarity (2025)](residualized-similarity-2025.md) | LLM の説明は推論過程を表していない |

### 評価

| | 何が分かるか |
| --- | --- |
| [Jangra ら (2025) SPTG の評価](sptg-evaluation-2025.md) | <strong>分野の名前。単独の指標は信用できない。組み合わせる</strong> |
| [文体転換の評価 (2025)](tst-evaluation.md) | <strong>評価は 3 次元。文体・内容・自然さは trade-off</strong> |

### 機械生成文の検出

| | 何が分かるか |
| --- | --- |
| [Przystalski et al. (2025)](przystalski-2025.md) | <strong>10 文でも人と LLM は分かれる。基準はモデルごとに違う</strong> |
| [林・相澤 (2026) モデル固有表現](japanese-llm-style.md)（同ファイルの 2 本目。別の論文） | <strong>日本語でも当たる。バージョン差だけで 93.6%</strong> |

### 日本語の資源

| | 何が分かるか |
| --- | --- |
| [中俣 多次元的抽出](nakamata-multidimensional.md) | <strong>客観的に導いた 4 つの文体軸。ジャンルのラベルを使っていない</strong> |
| [中俣 (2020) 類義副詞](nakamata-2020-adverbs.md) | <strong>30 組中 29 組が異なる文体グループ。言い換えは文体を持つ</strong> |
| [水上ほか (2013, 2014)](mizukami-2014.md) | <strong>個人性は機能語に宿る。場面の寄与は人の寄与と同じ大きさ</strong> |
| [馬場 語の文体値データ](baba-2022.md) | 13 万語の表。<strong>2 次元。向きが名前と逆</strong> |
| [有馬ら (2018) 文末表現辞書](arima-2018.md) | 文末表現の抽出規則。敬体/常体/会話体/ネット文体 |
| [mStyleDistance (2025)](mstyledistance-2025.md) | 日本語を含む文体埋め込み。ただし日本語部分は翻訳由来 |

## 分かったこと——測定

### 日本語の書き手識別は実証されている

日本語でも、1,100 字の作文と 500 字の日記で、<strong>2 人ずつの総当たり</strong>なら 99% / 92%
（[金 2013](jin-2013.md)）。閉集合では 78% / 74% に下がる。有効な特徴量も列挙されている。文体埋め込みが古典的な計量
文体論を上回るという報告もある（[Kim & Jurgens](kim-jurgens-2026.md)）。

<strong>特徴の選び方には、逆を向く 2 本がある。</strong>[Stamatatos §2.6](stamatatos-2009.md) は
<strong>頻度で選ぶ</strong>とし、判別力での選抜は話題分類の作法で、選抜された特徴はコーパス依存に
なるとする。[Evert ら (2015)](burrows-delta.md) は <strong>教師ありの選抜</strong>を未知データで検証し、
過学習していないと報告している。<strong>決着はしていない</strong>（[後述](#交絡と報告されている失敗)）。

効かないと報告されているものもある。係り受け距離の分布、語彙の豊富さ（TTR）。

<strong>ただし精度の数字をそのまま根拠にはできない。</strong>（[Wegmann](wegmann-2022.md)）著者識別は
<strong>題材を手掛かりに当てられてしまう</strong>。統制無しで訓練した文体表現は、自分の条件で AUC .79
出るのに、題材を揃えると .58 まで落ちる。<strong>その .79 は題材の精度だった。</strong>

そして解釈できる指標でも精度は大きく落ちない。Biber の 96 特徴 + Random Forest が、題材の
ばらつく著者検証で <strong>F1 0.77</strong>。微調整した RoBERTa が 0.78
（[Neurobiber](neurobiber-2025.md)）。

### 日本語には目盛りがある

[尤度比による著者比較](fusing-lr-2026.md)が、<strong>日本語ブログ 1,000 字、2,287 人</strong>で
較正されている。

| 系統 | Cllr（0 が最良、1 が偶然） |
| --- | --- |
| 単語埋め込み単独 | 0.38757 |
| 文字 bigram 単独 | 0.47115 |
| 埋め込みどうしを混ぜる | 0.36186 |
| <strong>計量文体論 + 埋め込みを混ぜる（最良、5 系統）</strong> | <strong>0.32484</strong>（EER 9.3%） |

<strong>種類の違うものを混ぜると、どちらの単独よりも良くなる。そして 6 系統以上は悪くなる。</strong>

特徴は <strong>文字 bigram / 機能語 unigram / 品詞 bigram / 読点とその直前の文字 / 文字種</strong>。
[金 2014](jin-2014.md) の 4 種と、独立に、ほぼ同じところに落ちている。

### 判定不能を置くと、残りの精度が上がる

[財津・金](zaitsu-2018.md) が、日本語ブログ 100 人で <strong>疑問／対照／無関係</strong> の 3 種を
比べる枠組みを立て、<strong>16 分析の得点を合算</strong>して判定している。

| | 平均得点 |
| --- | --- |
| 同一人 | <strong>+9.49</strong> |
| 別人 | <strong>−7.44</strong> |

<strong>絶対値 4 以下は「判定不能」に逃がす。</strong> 逃がした先で、感度 <strong>100%</strong>、特異度 <strong>95.1%</strong>。
判定不能率は 14〜19%。

<strong>2 割弱を「分からない」と言えることが、100% を可能にしている。</strong> 判定は 2 値ではなく
<strong>3 値</strong>——同一人、別人、判定不能——で返されている。

そして <strong>読点の打ち方</strong> を、独立した特徴として立てている研究が 3 本ある
（[財津・金](zaitsu-2018.md), [尤度比](fusing-lr-2026.md),
[柳・金 2022](yanagi-jin-2022.md)）。

<strong>4 本目の[金 2014](jin-2014.md) だけは、独立に立てていない。</strong> 読点の打ち方は
<strong>文字・記号 bigram の部分集合にすぎない</strong>ので、教師ありなら文字 bigram を使うほうが
有効だとしている。ただし次元がはるかに低いので <strong>教師なしの解析には向く</strong>、とも書いて
いる。<strong>落としたのではなく、包含されている。</strong>

<strong>1,000 字が目安である。</strong> 財津・金も尤度比も[金 2013](jin-2013.md) も、その長さで
やっている。年齢・性別による差は無い。

### 1 つの指標の中に、規範と個人が混ざっている

読点の打ち方について、石黒 (2011) の整理を[岩崎](japanese-attribution-others.md) が
引いている。

| 層 | 何が決めるか | 個人差が出るか |
| --- | --- | --- |
| 誰もが打つ | 意味・長さ・構造 | <strong>いいえ</strong> |
| <strong>人により異なる</strong> | <strong>表記・音調・リズム</strong> | <strong>はい</strong> |
| <strong>一部の人が打つ</strong> | <strong>強調</strong> | <strong>はい</strong> |

<strong>「読点の密度」という 1 つの数字にすると、3 層が混ざって規範に薄められる。</strong>
接続詞の語彙素ごと、位置ごとに分けて初めて個人が出る。

そして場面だけで大きく動く。<strong>接続詞の直後に読点を打つ割合は、新聞 65.6%、
Yahoo! ブログ 39.5%。</strong> 個人を見る前に 26 ポイントの幅がある。

<strong>規範の層には個人差が出ない。</strong> 岩崎は、文頭の接続詞の後に読点が打たれやすい理由として
公用文の書き方の書籍を挙げている。磯崎 (2010) は「文頭の接続詞の後に『、』を打つのも、
常識である」と書いている。

### 単独で弱い指標が、組み合わせでは効く

[統合アンサンブル](japanese-attribution-recent.md)で、文節パターンは <strong>単独 F1 0.704 で
最下位</strong>なのに、<strong>最良の組み合わせには必ず入っている</strong>。

逆に[尤度比](fusing-lr-2026.md)では、機能語 unigramが最良の組み合わせ
から落ちた。

<strong>指標を選ぶ基準は、単独の性能ではなく、他と違うものを見ているかである。</strong>

## 分かったこと——再現

### 素朴な手は両方とも失敗している

| 素朴な手 | 結果 |
| --- | --- |
| 例文を何本か見せる | 頭打ち。平均的な調子に寄り、AI 生成と検出される（[Catch Me](catch-me-2025.md)） |
| 文体の特徴を並べて指示する | <strong>例文だけより悪くなる</strong>（[Bhandarkar](bhandarkar-2024.md)） |

<strong>2 つ目は「特徴を渡せば寄る」という素朴な期待を否定している。</strong>

### そして最先端でも、文体はほとんど寄らない

[AuthorMix](authormix-2026.md) が 100 の（元著者 → 目標著者）対で測った結果。

| 手法 | Toward（文体の寄り） | Joint |
| --- | --- | --- |
| GPT-5.1 few-shot | <strong>0.08</strong> | 0.20 |
| TinyStyler | 0.16 | 0.31 |
| <strong>AuthorMix（最先端）</strong> | <strong>0.16</strong> | <strong>0.34</strong> |

人手評価では、<strong>few-shot の文体正解率は 0.40——偶然の 0.5 を下回る</strong>。最良でも 0.63。

### 「寄らない」の程度が、目盛り付きで測られた

[Sawant](authorship-gap-2026.md) が、著者検証モデル LUAR に <strong>天井と床</strong>を置いて測って
いる。

| | LUAR |
| --- | --- |
| 天井——<strong>同じ人</strong>の別の文章どうし | <strong>0.756</strong> |
| 床——<strong>違う人</strong>どうし | <strong>0.626</strong> |
| 4 つの個人化手法すべて | <strong>0.484〜0.508</strong> |

> personalized output is <strong>more distant from the target human author than random humans
> are from each other</strong>

<strong>個人化した出力は、赤の他人よりもその人から遠い。</strong> 手法を変えても差は 0.024 しか無い。

理由も測られている。<strong>LLM 自身の指紋が消えない。</strong> 生成文どうしでは著者を 0.918 で
区別できるのに、生成文と本物の間は 0.45〜0.49。<strong>LLM の文体空間の中で書き分けているだけ
で、人間の領域に入っていない。</strong>

そして人が手で直しても落ちない（[Post-Editing](post-editing-2026.md)）。自分の文章に
寄せる効果はある（g = 0.55）が、<strong>残っている LLM 寄りの差は g = 1.43</strong>。

<strong>1 回の生成で寄せきれることを示した報告は無い。</strong> そして到達すべき水準として、
他人どうしの一致（0.626）がまず参照点になる。

### 失敗の原因が特定されていて、しかも測れる

[Zeng & Nini](zeng-nini-2026.md) が、なりすましがなぜ見破られるかを突き止めている。

> Human-authored texts naturally contain <strong>recurring ... n-grams that reflect an
> individual's habitual phrasing</strong>. Because LLM-generated texts exhibit <strong>higher entropy and
> greater lexical diversity</strong>, they fail to produce sufficient repetitions

<strong>人の文体は繰り返しでできている。LLM は語彙が散りすぎて繰り返せない。</strong>

圧縮後の大きさ、エントロピー、Type-Token 比。3 つとも、なりすまし文の方が高かった。
<strong>寄せるべき方向がはっきりしている。</strong>

そして温度と頻度ペナルティは既定値のままだったと論文自身が認めている。<strong>プロンプト以外
に手がある。</strong>

### 失敗した実験が、同じ改善案を指している

[Bhandarkar](bhandarkar-2024.md) は、固定の特徴集合が全ての著者に等しく効くという前提
自体を疑っている。

> instead of static directed prompting, a more effective approach could involve
> <strong>dynamically prompting LLMs by considering each author's individual linguistic
> preferences</strong>

[Catch Me](catch-me-2025.md) の結びも同じ方向を向く（`richer personalization signals`）。
<strong>著者ごとに、その人に効く特徴を選んで渡す。</strong> どちらも、そこに答えがあるかもしれないと
書いて、試していない。

そして測る側では、これは既に実績のある設計である。Writeprints は 2008 年の時点で
<strong>著者ごとに違う特徴量セット</strong>を使っている（[Writeprints](writeprints-liwc.md)）。

<strong>隠す側でも実装され、効果が測られていた。</strong>（[StyleRemix](styleremix-2024.md)）著者
ベクトルと平均の差の絶対値で上位 k 軸を選び、標準偏差で重みを付ける。<strong>同じ本数を無作為
に選ぶより 6% 良い。</strong>

| 側 | 書き手ごとの選択 |
| --- | --- |
| 測る | <strong>実績あり</strong>（[Writeprints](writeprints-liwc.md), 2008） |
| 隠す | <strong>実績あり。無作為より 6%</strong>（[StyleRemix](styleremix-2024.md), 2024） |
| なりすます | <strong>成功率が 0.11〜0.77 に開く</strong>（[Masks and Mimicry](masks-mimicry-2025.md)） |
| <strong>真似る</strong> | <strong>未検証</strong>（[Bhandarkar](bhandarkar-2024.md) が提案のみ） |

<strong>報告が見当たらないのは「近づける向き」だけである。</strong>

### 渡す軸は 3〜4 本で頂点になる

独立した 2 本が同じところに行き着いている。

| | 最良 | それを超えると |
| --- | --- | --- |
| [StyleRemix](styleremix-2024.md)（軸を混ぜる） | <strong>3〜4</strong> | <strong>5 本以上で文法が 16% 落ちる</strong> |
| [AuthorMix](authormix-2026.md)（他人を混ぜる） | <strong>4</strong> | <strong>7 本以上で劣化</strong> |

[Bhandarkar](bhandarkar-2024.md) の「並べて指示すると悪化する」に、<strong>本数という形が
付いた。</strong> 悪化の原因は指示することではなく、<strong>多すぎること</strong>かもしれない。

### 手本は 3 本前後。増やすと下がる

| | 手本の量 |
| --- | --- |
| [Masks and Mimicry](masks-mimicry-2025.md) | <strong>5 人中 4 人が、全部より 3 本の方が良い</strong> |
| [SICO](sico-2024.md) | 人の文章 <strong>40 本</strong>（「人らしく」の側） |
| [AuthorMix](authormix-2026.md) | 目標著者あたり <strong>16 文</strong> |
| [Przystalski](przystalski-2025.md) | <strong>10 文</strong>で人と LLM は分かれる |

<strong>コーパスを増やす価値があるのは測るときだけである。</strong> 再現させるときに増やすと下がる。この 2 つを
混同すると「たくさん集めたのに悪くなった」が起きる。

### 際立ちだけでは選べない——動かせない軸がある

その書き手で際立っているか。それに加えて、<strong>指示して LLM が動かせるか</strong>。

[高橋ら](japanese-llm-style.md) が、日本語のスタイル制御で <strong>制御可能なスタイルと制御困難な
スタイルが存在する</strong>ことを示している。モデルの種類には依存しない。
[Bhandarkar](bhandarkar-2024.md) が挙げた原因の 1 つ——LLM が指示された特徴を取り込む
能力を欠く——の裏付けになる。

高橋らは、制御できるスタイルとできないスタイルを <strong>部分空間とノルムの分布分析で
定量的に区別できる</strong>としている。<strong>どちらであるかは、事後に測って分かる性質である。</strong>

## 分かったこと——日本語の資源

### 解釈できる軸は、日本語に既にある

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

そして [Neurobiber](neurobiber-2025.md) が 2025 年に別コーパス（CORE）で <strong>Involved 対
Informational</strong> を再現した。

> [!NOTE]
> <strong>Neurobiber は独立な導出ではない。</strong> 使っているのは BiberPlus の 96 特徴、つまり
> Biber (1988) 自身の特徴族である。<strong>別のコーパスでの再現</strong>であって、別の出発点から
> 同じ軸に着いたわけではない。
>
> 独立なのは <strong>中俣</strong> のほうである。日本語の 89 形式を自分で選び、ジャンルのラベルを
> 使わずに因子分析して、Biber の D1・D2 と一致した。<strong>第 1 軸について言えるのは、
> 独立な導出が 2 つ（英語 1988 年、日本語）と、別コーパスでの再現が 1 つ、である。</strong>

### 日本語の個人性は機能語に宿る

[水上ほか](mizukami-2014.md) が、4,720 万組の言い換えデータベースと、助詞・助動詞・
感動詞・フィラーを集めた <strong>1,338 組の辞書</strong> を比べている。

<strong>1,338 組の方が「その話者らしい」と評価された。</strong> 大きい方は機能語を 11% しか覆えて
いなかった。前身の実験では <strong>フィラーと感動詞だけで正解率 51.7%</strong>（偶然 20%）。

[Writeprints](writeprints-liwc.md) の 560 次元中 403 が機能語だったのと同じ結論が、
日本語側から出ている。<strong>覆う範囲ではなく、覆う場所が効く。</strong>

## 分かったこと——評価

### 説明の正しさを、制御できるかで測る方法がある

[Kim & Jurgens](kim-jurgens-2026.md) の <strong>「制御できることを説明とみなす」</strong>。説明が
正しいかを、その説明で書かせて同じ文体になるかで測る。

人手による合否判定は主観的で、高くつき、回数を稼げない。この基準なら機械で回せる。

そして評価の側からは、<strong>単一の点数に頼らないこと</strong>と、その組み合わせ方が出ている——
<strong>違う原理の指標を、性能で重み付けて投票させる</strong>
（[SPTG の評価](sptg-evaluation-2025.md)。組み合わせ 0.821 対 単独最良 0.815）。
[Catch Me](catch-me-2025.md) も、著者識別・著者検証・文体の一致・AI 検出の 4 つを
並べて評価している。

### 本人の判断は、思っていたほど当てにならない

| 誰が判断するか | 一致度 |
| --- | --- |
| 複数の評価者、内容の好み | 0.779 |
| 複数の評価者、<strong>文体</strong>の好み | <strong>0.641</strong> |
| 複数の評価者、推敲の好み | <strong>0.400</strong> |
| <strong>本人</strong>、自分の文章と直した文章 | <strong>区別が付かない</strong> |

[Post-Editing](post-editing-2026.md) で、81 人が LLM の草稿を自分らしく直した。
<strong>本人は「自分で書いたのと同じくらい自分らしい」と評価した</strong>（p = .9062）。
測ると g = 1.43 の差が残っていた。埋め込みとの相関は r = 0.244。

<strong>本人の主観より、測った値のほうが実態に近い場面がある。</strong>

### LLM に挙げさせた特徴で採点すると循環する

<strong>循環する。</strong>（[Sawant](authorship-gap-2026.md)）

LLM に著者の特徴を挙げさせ、生成文がそれを満たすかで測ると、
<strong>本人の文章（0.427）より個人化した生成文（0.542）の方が高く出る。</strong>
抽出も判定も LLM だからで、測っているのは指示に従えたかである。

さらに、その特徴抽出自体が安定しない。<strong>同じ著者から 2 回抽出して、一致は Jaccard 0.22。</strong>

<strong>論文はこれを検査として提示している。</strong> 本人の実際の文章を採点して、生成文より低く出る
指標は、著者性ではなく指示追従を測っている。

### 場面ごとの見込みが、日本語で立った

| 場面 | 素材 | 規模 | 到達点 |
| --- | --- | --- | --- |
| 技術記事 | [ブログ 1,000 字](fusing-lr-2026.md) | 2,287 人 | <strong>EER 9.3%</strong> |
| 文学 | [800 字](japanese-attribution-recent.md) | 10 人 | F1 0.96〜1.00 |
| 作文 | [1,100 字](jin-2013.md) | 11 人 | 99%（2 人総当たり）/ 78%（閉集合） |
| 日記 | [500 字](jin-2014.md) | 6 人 | F1 98.4 |
| <strong>チャット</strong> | [商品レビュー 60 字](japanese-attribution-recent.md) | 100 人 | <strong>88%</strong>（1,000 人で 51%） |

<strong>短くなるほど落ちる。</strong> そして短い非形式文では、<strong>定型文・情報不足・話題語彙</strong> の 3 つが
邪魔をする。

## 2 つの条件は別物である

<strong>「その人が書いたものとして通る」は 2 つに分かれる。</strong>

| 条件 | 状況 |
| --- | --- |
| <strong>その人らしい</strong> | <strong>誰も届いていない</strong>（[Sawant](authorship-gap-2026.md), [AuthorMix](authormix-2026.md)） |
| <strong>人が書いたものに見える</strong> | <strong>解かれた例がある</strong>（[SICO](sico-2024.md)） |

<strong>2 つ目は 1 つ目から出てこない。</strong> GPT 系の生成文が人と判定される割合はほぼ 0%
（[Catch Me](catch-me-2025.md)）。文体がどれだけ寄っても、機械が書いたと分かる文章は
別の条件で落ちる。

2 つ目を測る量も分かっている。<strong>繰り返しの多さ、エントロピー、圧縮後の大きさ、
Type-Token 比</strong>（[Zeng & Nini](zeng-nini-2026.md), [水上ほか](mizukami-2014.md)）。

> [!NOTE]
> SICO の手順の中核は <strong>検出器のスコアに対する最適化</strong>である。検出器を持たない
> 場面にそのまま移せるかは示されていない。

## 交絡と、報告されている失敗

<strong>指標を並べて渡すと悪化する。5 本を超えると文法が壊れる。</strong>
（[Bhandarkar](bhandarkar-2024.md), [StyleRemix](styleremix-2024.md),
[AuthorMix](authormix-2026.md)）

<strong>題材の似た例を選ぶと悪化する。</strong>（[Catch Me](catch-me-2025.md)）題材の近い例を選びたくなるが、
文体の多様性が減って逆効果になる。

<strong>手本を増やすと下がる。</strong>（[Masks and Mimicry](masks-mimicry-2025.md)）5 人中 4 人で、
全部渡すより 3 本の方が良かった。<strong>頭打ちではなく、下がる。</strong>

<strong>著者識別の精度は、文体の精度ではない。</strong>（[Wegmann](wegmann-2022.md)）題材を揃えると
AUC が .79 から .58 に落ちた例がある。<strong>先行研究の精度を引用するときは、題材が統制されて
いたかを見る。</strong>

<strong>ジャンルの制約が強い文書ほど、文体を動かせない。</strong>（[StyleRemix](styleremix-2024.md)）
学術論文では、演説・ブログの 5 分の 1 から 10 分の 1 しか動かなかった。<strong>技術記事はそちら
側に近い。</strong>

<strong>場面を固定しないと、測っているものの半分は場面である。</strong>（[水上ほか](mizukami-2014.md)）
クロスエントロピーで測ると、同じ人でも役割が変われば +0.77、違う人でも役割が同じなら
+0.85。<strong>ほぼ同じ大きさである。</strong>

<strong>目盛りに天井と床が無い数字は読めない。</strong>（[Sawant](authorship-gap-2026.md)）0.5 という
値には意味が無い。<strong>本人どうしの一致と、他人との一致を測って、その間のどこかを示す。</strong>

<strong>LLM に文体を挙げさせて、それで採点すると循環する。</strong>（[Sawant](authorship-gap-2026.md)）
本人の文章より生成文が高く出たら、その指標は壊れている。

<strong>依頼文に「どう書くか」を漏らすと 28 ポイント膨らむ。</strong>（[Sawant](authorship-gap-2026.md)）
本文の 1 文目から依頼文を作ると、書き出しの癖がそのまま手掛かりになる。
<strong>依頼文は「何について書くか」の中立な要約にする。</strong>

<strong>文体埋め込みは、同じ依頼から作った 2 つを比べる場面で崩れる。</strong>
（[SPTG の評価](sptg-evaluation-2025.md)）内容が同じなので区別できない。落ち込みは分類
ごとに −38.3% から −11.3% まで幅がある。

<strong>構造化した出力を求めると悪化するという報告がある。</strong>（[SPTG の評価](sptg-evaluation-2025.md)）
<strong>ただしモデル 1 つ・64 件で、両条件とも偶然を下回る比較である。</strong> 指示側で同じことが
起きるかも調べられていない。

<strong>直すと書き手どうしの差が均される。</strong>（[Post-Editing](post-editing-2026.md)）人が直した
文章は、自由に書いた文章より互いに似通う（g = 1.42）。<strong>差を出す道具にとっては逆向きの
力である。</strong>

<strong>特徴選抜については、2 本が逆を向いている。</strong>[Stamatatos §2.6](stamatatos-2009.md) は
判別力で選抜すると <strong>話題語が選ばれ、コーパス依存になる</strong>とし、頻度で選ぶほうが上だと
する。一方 [Evert](burrows-delta.md) は、教師ありの再帰的特徴削減で 234 語まで絞り、
<strong>未知の著者・作品で検証して過学習していないことを確かめた</strong>（正解率 0.97、
クラスタリングは全特徴より良い ARI 0.871 対 0.835）。結論も <strong>「見込みのある方法」</strong>
としている。

<strong>どちらが正しいかは、この 2 本では決まらない。</strong> 対象が違う（英仏独の小説と、著者識別
一般）。Evert 自身も、選ばれた特徴の中に <strong>独語の歴史的正書法</strong>（`Heimath`、`giebt`）
という <strong>コーパス由来の可能性が高いもの</strong>が混ざったことは書いている。

<strong>単独の性能で指標を落とすと、組み合わせが弱くなる。</strong>
（[統合アンサンブル](japanese-attribution-recent.md)）文節パターンは単独最下位（0.704）で
最良の組み合わせの常連。<strong>単独の順位は、組み合わせでの寄与を予測しない。</strong>

<strong>定型文が著者間の類似を押し上げる。</strong>（[日本語レビュー](japanese-attribution-recent.md)）
「発送が早かった」に相当するものは、どの場面にもある。技術記事なら見出しの定型と引用、
チャットなら挨拶と相槌。

<strong>基準は使う LLM ごとに違う。しかもバージョンごとに違う。</strong>
（[Przystalski](przystalski-2025.md), [林・相澤](japanese-llm-style.md)）日本語では、
同一系列のバージョン差だけで 93.6% 識別できる。<strong>モデル名では足りない。版と推論設定まで
記録する。</strong>

<strong>寄せると内容が壊れる。</strong>（[文体転換の評価](tst-evaluation.md)）文体・内容・自然さの間に
trade-off がある。書きぶりだけを見ていると、題材が保存されているかを見落とす。

<strong>文長・単語長・漢字率・語彙の豊富さは効かない。</strong>（[金 2014](jin-2014.md)）古典的で
数えやすいので手が伸びるが、<strong>普通の人が書いた現代文では弱い</strong>と名指しされている。

<strong>文字 n-gram は bigram まで。</strong>（[金 2014](jin-2014.md)）3 以上は内容とジャンルの雑音を
拾う。

<strong>解析器と対象がずれると精度が落ちる。</strong>（[柳・金](yanagi-jin-2023.md)）新聞で訓練した
解析器を旧字体の小説に当てると壊れる。

<strong>題材から完全には逃げられない。</strong>（[Stamatatos §5](stamatatos-2009.md)）機能語も文字
n-gram も題材情報を拾う。減らす設計は要るが、消しきれる前提では立てない。

<strong>単一の点数は騙せる。</strong>（[SPTG の評価](sptg-evaluation-2025.md)）文体転換の分野で
報告されている（Krishna et al. 2020）。

<strong>崩れた文章ほど難しい。</strong>（[Catch Me](catch-me-2025.md)）
記事とメールでは生成文が本人のものと判定される率が 95〜97% なのに、<strong>ブログと掲示板では
19〜66%</strong>。

そして <strong>記事側の 95% を根拠にしてはいけない。</strong> 型を守れているだけかもしれない
（[Wegmann](wegmann-2022.md) と同じ罠）。

<strong>先行研究の対象は、思ったほど偏っていなかった。だが穴は残っている。</strong>
日本語の 1,000 字ブログ（[2,287 人](fusing-lr-2026.md),
[100 人](zaitsu-2018.md)）と 60 字のレビュー（[100〜1,000 人](japanese-attribution-recent.md)）
は押さえられている。<strong>残る穴は、コードブロックや見出しを含む記事の構造と、
日本語の LLM 生成文を本人の文章と比べた実験である。</strong>

## 資源

### 日本語

<strong>『語の文体値データ』</strong>（馬場 2022, CC BY-NC-ND 3.0）
DOI: <https://doi.org/10.15084/00003532>

<strong>『BCCWJ 図書館サブコーパスの文体情報』</strong>（柏野 2015）
DOI: <https://doi.org/10.15084/00003109>

### 英語

いずれも英語である。

| | 何か |
| --- | --- |
| [DiSC](styleremix-2024.md) | 7 軸 16 方向の対訳コーパス。1,500 テキスト。<strong>日本語に相当するものは見当たらない</strong> |
| [AuthorMix データ](styleremix-2024.md) | 14 著者 4 領域 3 万段落 |
| [BiberPlus / Neurobiber](neurobiber-2025.md) | 96 の文法形式タグ |
| [STEL / Style-Embeddings](wegmann-2022.md) | 題材を統制した評価の枠組み。<https://github.com/nlpsoc/Style-Embeddings> |
