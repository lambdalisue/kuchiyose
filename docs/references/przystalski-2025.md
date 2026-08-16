# Przystalski et al. (2025) Stylometry recognizes human and LLM-generated texts in short samples

Expert Systems with Applications. arXiv:2507.00838.
<https://arxiv.org/pdf/2507.00838>

<strong>本文を読んだ。人の文章と LLM の出力がどこまで分離するかを扱っている。</strong>

## 何をしたか

Wikipedia の人が書いた説明文と、LLM が生成・要約・言い換えした文章を、<strong>古典的な計量
文体論の特徴量で判別できるか</strong>を試した。

- 生成側: GPT-3.5/4、LLaMa 2/3、Orca、Falcon
- 要約: T5、BART、Gensim、Sumy。言い換え: Dipper、T5
- <strong>1 件は 10 文</strong>
- 特徴量: StyloMetrix（人が設計した 196 種）と n-gram 頻度
- 分類器: 決定木と LightGBM

## 結果

<strong>短い文章でも、少ない特徴量で、はっきり分かれる。</strong>

| | |
| --- | --- |
| 2 値分類 | 精度 <strong>0.79〜1.0</strong> |
| Wikipedia 対 GPT-4 | <strong>0.98</strong>（均衡データ） |
| 7 クラス | Matthews 相関係数 0.87 |

> LLMs can be easily distinguished from the man-made texts and from each other with a
> boosted tree classifier <strong>even with very few features</strong> (196 for StyloMetrix in English)
> <strong>and even for extremely short texts (10 sentences)</strong>

## 効いた特徴量（重要度順）

1. <strong>機能語の異なり数</strong>
2. 叙述文中の語数
3. 語彙素の type-token 比
4. 名詞句のあいだの統計量
5. 前置（fronting）
6. 語数と文数の差
7. <strong>句点</strong>
8. <strong>句読点</strong>
9. <strong>読点</strong>
10. 数詞

<strong>句読点が上位に 3 つ入っている。</strong> 単純な記号の数が、人と機械を分ける上位の特徴に
なっている。

## LLM の文章の性質

<strong>LLM は標準的すぎる。</strong>

> the LLM favours certain individual words and is <strong>more standardised</strong> than Wikipedia in
> terms of grammatical structures

これが基準として使える理由である。書き手の個性は、標準的なものからの <strong>ずれ</strong> として現れる。

細かい観察も面白い。LLaMa 2 の空白は二重空白や行頭の空白だった。句点の数が効くのは
<strong>LLM が文の途中で生成を止めることがある</strong>ためではないか、としている。

## 落とし穴

<strong>LLM ごとに効く特徴が違う。</strong>

> models do not have single strongly recognisable features, but their style is <strong>more
> dispersed among many quantified features</strong>. Moreover, the explanations are not general,
> but may vary depending on the model

<strong>基準は、使う LLM に依存する。</strong> モデルを変えれば基準も
変わる。「LLM が既定で書いたもの」と一口に言えない。どのモデルで作った基準かを記録
する必要がある。

<strong>StyloMetrix は日本語に対応していない。</strong> ポーランド語、英語、ドイツ語、ウクライナ語、
ロシア語のみ。頻度特徴の側も spaCy に依存する。<strong>日本語では自前で用意することになる。</strong>

Wikipedia は多人数が書いて多人数が編集しているので、そもそも個人の文体ではない、という
限界も自ら挙げている。

## 押さえておく点

- <strong>人の文章と LLM の既定出力は分離する。</strong> 実証されている
- <strong>10 文でも分かれる。</strong> 短い文章でも成立する
- <strong>句読点が効いている。</strong> 単純な指標が上位に来る
- <strong>分離の基準はモデルごとに違う</strong>
- 日本語向けの特徴量セットは、この研究には無い
