# Stamatatos (2009) A Survey of Modern Authorship Attribution Methods

Journal of the American Society for Information Science and Technology 60(3).
<https://icsdweb.aegean.gr/stamatatos/papers/survey.pdf>

<strong>全文を読んだ。</strong> この分野の標準的な総説である。

## 特徴量の分類

計算に必要な道具の重さで 5 つに分けている。

| 種類 | 例 | 要る道具 |
| --- | --- | --- |
| 語彙的 | 語長、文長、語彙の豊富さ、語の頻度、語 n-gram | 分かち書き器 |
| 文字的 | 文字種の数、文字 n-gram、圧縮 | <strong>ほぼ不要</strong> |
| 統語的 | 品詞、チャンク、句構造、書き換え規則 | 品詞タグ付け器、構文解析器 |
| 意味的 | 同義語、意味依存関係 | 辞書、意味解析器 |
| 応用固有 | 構造、内容固有、言語固有 | HTML 解析器、専用辞書 |

## 主要な知見

### 機能語がなぜ効くのか

> function words are used in a largely unconscious manner by the authors and they are
> topic-independent

<strong>無意識であること、題材に依存しないこと。</strong> 機能語がこの 2 つを備えている点が、
文体の担い手とされる理由である。

### 文体分類は話題分類と逆である

<strong>文体では最頻出の特徴がそのまま最良の特徴になる。</strong> 話題分類では判別力で特徴を選ぶが、
文体ではその必要がない。

<strong>「特徴を判別力で選抜する」のは話題分類の作法である。</strong> 文体分析では頻度順に取れば
よい。

次元も低くて済む。話題分類が数千語を要するのに対し、文体分類は数百語で足りる。

### 頻出語をどこまで取るか

| 研究 | 語数 |
| --- | --- |
| Burrows (1987, 1992) | 100 以下 |
| Koppel et al. (2007) | 250 |
| Stamatatos (2006a) | 1,000 |
| Madigan et al. (2005) | 2 回以上出る語すべて |

<strong>注意点。</strong> 最頻出の数十語は閉じたクラス（冠詞・前置詞）が占めるが、数百語を超える
と開いたクラス（名詞・動詞）が多数派になる。<strong>次元を増やすと題材固有の語が混ざる。</strong>

### 文字 n-gram

語彙的特徴より効くという報告が複数ある（Forsyth & Holmes 1996、Peng et al. 2003、
Keselj et al. 2003、Grieve 2007）。2004 年の著者識別コンペで最上位のひとつも文字
n-gram だった。

利点は 3 つ。語彙情報も文脈も句読点も大文字小文字も拾える。誤字に強い（`simplistic`
と `simpilstc` は多くの trigram を共有する）。<strong>道具がほぼ要らない。</strong>

そして <strong>分かち書きが難しい東洋の言語では、文字 n-gram が適した解になる</strong>
（Matsuura & Kanada 2000）。日本語がまさにこれである。

n の選び方は言語依存。大きくすると題材情報も拾ってしまう。

### 語 n-gram はあまり良くない

個々の語より精度が高いとは限らず、疎になり、<strong>文体ではなく内容を捉えてしまいやすい</strong>。

### 統語的・意味的特徴は単独では足りない

> More elaborate features, capturing syntactic or semantic information are not yet able
> to represent adequately the stylistic choices of texts. Hence, they can only be used as
> complement in other more powerful features

解析器が持ちこむ雑音が原因かもしれない、としている。

### 語彙の豊富さは当てにならない

TTR も hapax も文章の長さに強く依存する。長さに対して安定させる指標（Yule の K、
Honoré の R）も提案されたが結果は疑わしく、<strong>単独で使うのは信頼できない</strong>とされる。

## 評価コーパスの作り方

> Any good evaluation corpus for authorship attribution should be controlled for genre
> and topic.

理想は <strong>全員が同じ題材で書いたもの</strong>。実例として、Chaski (2001) は 92 人に共通の
10 題、Baayen et al. (2002) は 8 人に 3 ジャンル × 3 題材で 72 編を書かせている。

さらに年齢・教育・国籍も揃え、<strong>同じ時期のもの</strong>を使う（Can & Patton 2004、文体は
時とともに変わる）。

<strong>統制すべきは、ジャンル、題材、時期、書き手の属性である。</strong>

## この分野が解けていないこと

> An important obstacle is that it is not yet possible to explain the differences between
> the authors' style. It is possible to estimate the significance of certain (usually
> character or lexical) features for specific authors. <strong>But what we need is a higher level
> abstract description of the authorial style.</strong>

<strong>判別はできる。説明はできない。</strong> これが 2009 年時点の到達点であり、大きくは変わって
いない。

<strong>判別と説明は別の問題であり、後者だけが未解決のまま残っている。</strong> 文体を再現させるに
は、何がその人らしさかを言葉にする必要がある。そこが埋まっていない。

判別のための特徴量を新たに工夫しても、既存手法を超えることはまず無い。

## 未解決として挙げられていること

- <strong>文章の長さの下限が決まっていない。</strong> 1,000 語未満でも良い結果の報告はあるが、
  閾値は定義できていない
- <strong>著者・ジャンル・題材を分離できていない。</strong> 機能語も文字 n-gram も、題材情報を
  拾ってしまうことが分かっている（Clement & Sharp 2003、Mikros & Argiri 2007）。
  「文体情報と題材情報の組み合わせだからこそ強いのではないか」とまで書かれている

## 統語的特徴（§2.3）

<strong>著者は統語の型を無意識に使うので、語彙情報より信頼できる指紋だと考えられている。</strong>
機能語が効くこと自体、統語情報が有用である証拠でもある（機能語は特定の統語構造に
現れるため）。

だが解析器が要る。解析器は誤るので <strong>データに雑音が入る</strong>。言語依存でもある。

| 研究 | 使ったもの | 結果 |
| --- | --- | --- |
| Baayen et al. (1996) | 書き換え規則の頻度 | 語彙的特徴と語彙の豊富さより良い |
| Gamon (2004) | 同上 | <strong>単独では語彙的特徴より悪い。組み合わせると改善</strong> |
| Stamatatos et al. (2000, 2001) | チャンク（句）の数と長さ | 高精度で自動抽出できる |
| Hirst & Feiguina (2007) | 統語ラベル列の bigram | <strong>200 語程度の非常に短い文章で有効</strong> |
| van Halteren (2007) | 形態統語タグの n-gram と書き換え規則 | 約 90 万次元 |

<strong>品詞タグの n-gram</strong> は簡便な選択肢として広く使われている。

Karlgren & Eriksson (2007) は、頻度ではなく <strong>連続する文にわたる出現の型</strong>（分布的性質）
を見ようとしており、「有望」と評されている。

## 応用固有の特徴（§2.5）

電子メールやフォーラムでは <strong>構造的な特徴</strong> が定義できる。挨拶、結び、署名の型、字下げ、
段落長など。

> such features ... are particular important in <strong>very short texts</strong> where the stylistic
> properties of the textual content cannot be adequately represented

<strong>非常に短い文章ほど、構造的な特徴が効く。</strong>

言語固有の特徴もある。現代ギリシャ語の二言語併用（形式的な語尾と口語的な語尾）が例に
挙がっている。<strong>日本語なら敬体と常体がこれに当たる。</strong>

## 特徴の選び方（§2.6）——ここが重要

<strong>選抜の基準は頻度である。判別力ではない。</strong>

> The most important criterion for selecting features in authorship attribution tasks is
> <strong>their frequency</strong>. In general, the more frequent a feature, the more stylistic variation
> it captures.

| 比較 | 結果 |
| --- | --- |
| 頻度 vs 情報利得（Houvardas & Stamatatos 2006） | <strong>4,000 次元までは頻度が上</strong> |
| 頻度 vs オッズ比（Koppel et al. 2006） | <strong>頻度が上。組み合わせるとさらに良い</strong> |

判別力で選ぶことには危険がある。

> the best features may strongly correlate with one of the authors due to <strong>content-specific
> rather than stylistic choices</strong>

政治の記事を書く著者とスポーツの記事を書く著者がいれば、選抜は話題語を選ぶ。<strong>選抜され
た特徴はコーパス依存になり、一般には使えない。</strong>[Evert](burrows-delta.md) の過学習の話と
同じである。

### 不安定性という基準

Koppel et al. (2006) が提案した、もう 1 つの基準。

<strong>不安定性とは、その言語的特徴に「言い換え」が存在するかである。</strong>

- `and` や `the` は <strong>安定</strong>——代わりが無い。だから文体の選択ではない
- `benefit` や `over` は <strong>不安定</strong>——`gain` や `above` に置き換えられる。だから <strong>文体の選択
  を表しやすい</strong>

言い換えの生成には、機械翻訳で往復させる方法を使っている。

実験では、不安定性だけで選ぶと頻度で選ぶより劣った。だが <strong>頻度と不安定性を組み合わせる
と大きく良くなった</strong>。

<strong>選択肢があるところに文体があり、選択肢が無いところには無い。</strong>

## profile 型と instance 型（§3.4）

| | profile 型 | instance 型 |
| --- | --- | --- |
| 表現 | 著者の全文書を連結して 1 つ | 文書ごとに 1 つ |
| 特徴の組み合わせ | 難しい | <strong>容易</strong> |
| 文書単位の特徴（挨拶、署名） | <strong>使えない</strong> | 使える |
| 短い文章 | <strong>連結する方が安定する</strong> | 個別だと不安定 |
| 学習 | 不要 | 必要 |

<strong>文書ごとの値の分布が要る場合、異種の特徴を混ぜる場合、構造的な特徴を使う場合は
instance 型になる。</strong>

ただし <strong>短い文章では profile 型の方が安定する</strong>。短文を何本か束ねて 1 単位にする設計は
これに沿う。
