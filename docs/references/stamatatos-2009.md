# Stamatatos (2009) A Survey of Modern Authorship Attribution Methods

Journal of the American Society for Information Science and Technology 60(3).
<https://icsdweb.aegean.gr/stamatatos/papers/survey.pdf>

<strong>全文を読んだ。</strong> この分野の標準的な総説。kakiburi の位置づけを決める上でいちばん
効く 1 本。

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

<strong>kakiburi の[軸](../spec/000-axis.md)は、この一文から性質 1 と性質 3 を採った。</strong>
定義を先に立てたのではなく、先行研究が示した性質を定義に採っている。

### 文体分類は話題分類と逆である

<strong>文体では最頻出の特徴がそのまま最良の特徴になる。</strong> 話題分類では判別力で特徴を選ぶが、
文体ではその必要がない。

これは kakiburi にとって重要な警告になる。「指標を分散比で選抜する」という発想は
<strong>話題分類の作法</strong> であって、文体分析の作法ではない。頻度順に取ればよい。

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

この作法から、kakiburi は場面を固定すること、時期の偏りを見ることを採る。ただし
「同じ題材」までは揃えられない。実在の記事を使うためである。

## この分野が解けていないこと

ここが kakiburi にとって決定的である。

> An important obstacle is that it is not yet possible to explain the differences between
> the authors' style. It is possible to estimate the significance of certain (usually
> character or lexical) features for specific authors. <strong>But what we need is a higher level
> abstract description of the authorial style.</strong>

<strong>判別はできる。説明はできない。</strong> これが 2009 年時点の到達点であり、大きくは変わって
いない。

kakiburi の軸は「本人の外で使えるようにする」——つまり貸すことである。貸すには説明が
要る。<strong>つまり kakiburi がやろうとしていることのうち、検める側は解決済みの技術で、
貸す側が未解決の側にある。</strong>

これは戦略に直接効く。判別のための特徴量を自前で工夫しても、既存手法を超えることは
まず無い。労力を割くべきは説明の層である。

## 未解決として挙げられていること

- <strong>文章の長さの下限が決まっていない。</strong> 1,000 語未満でも良い結果の報告はあるが、
  閾値は定義できていない
- <strong>著者・ジャンル・題材を分離できていない。</strong> 機能語も文字 n-gram も、題材情報を
  拾ってしまうことが分かっている（Clement & Sharp 2003、Mikros & Argiri 2007）。
  「文体情報と題材情報の組み合わせだからこそ強いのではないか」とまで書かれている
