# Kim & Jurgens (2026) Interpreting Style Representations via Style-Eliciting Prompts

arXiv:2606.05716. University of Michigan.
<https://arxiv.org/pdf/2606.05716>

<strong>要旨・序論・関連研究を読んだ。</strong> [Stamatatos](stamatatos-2009.md) が 2009 年に未解決と
名指しした「文体を高い水準で言い表す」に、正面から取り組んでいる。<strong>採れるものと、採れ
ないものが、はっきり分かれる。</strong>

## 前提となっている技術

<strong>文体埋め込み（style representation）</strong>。同じ著者の文章が近くに来るように学習した
ベクトル表現（Rivera-Soto et al. 2021、Wegmann et al. 2022、Patel et al. 2025）。

> These dense vector representations have resulted in significant performance gains for
> many tasks like authorship attribution <strong>over older, more interpretable methods from
> stylometry</strong>

<strong>著者識別のような課題では、古い解釈できる計量文体論の手法より大きく性能が上がった</strong>
というのが、この論文が前提として置いている認識である。

> [!NOTE]
> [Neurobiber](neurobiber-2025.md) は、題材のばらつく著者検証で、解釈できる 96 特徴
> + Random Forest が F1 0.77、微調整した RoBERTa が 0.78 という結果を出している。
> <strong>差がほとんど無い場合もある。</strong>

問題は、ベクトルが何を捉えているか分からないこと。

## 「制御できることを説明とみなす」

この論文の中心的な着想である。

これまでの解釈の試みは、LLM に対象の文章を見せて自然言語で説明させるものだった。だが
それは <strong>説明的であって機能的でない</strong>——その説明を使って文体を再現・操作・転写できる
保証がない。しかも LLM の偏りや幻覚が混ざる。

代わりに、<strong>その説明を使って生成させたときに同じ文体になるか</strong> を基準にする。

> we adopt a complementary perspective that emphasizes <strong>control as an explanation</strong>

## 何をしたか

1. 26 の文体次元（語彙選択、統語構造、調子、修辞戦略など）にわたる <strong>1,010 個の文体
   特徴</strong>を用意する
2. それを指示として LLM に <strong>180 万件</strong>の文章を生成させる
3. 生成文の文体埋め込みから、元の指示を復元する <strong>デコーダ</strong>を学習する

これで、埋め込み → 自然言語の指示、という変換が得られる。

## 結果

3 つの課題すべてで、<strong>対象の文章を直接プロンプトに入れる方法を上回った</strong>。

| 課題 | 改善 |
| --- | --- |
| 指示の復元 | ROUGE-1 76.0%、LaBSE 21.7%、LLM-as-judge 42.8% |
| 同じ文体で生成 | L2 で 12.9% |
| <strong>人が書いた文章の文体に寄せる</strong> | L2 で 26.1% |

## 読み取れること

### 説明の正しさを、機械で測る基準がある

<strong>「制御できることを説明とみなす」。</strong> 出した説明で書かせて、元の文章の文体に寄ったかを
測る。<strong>人手評価を介さずに回せる。</strong>

### 出力は自然言語の指示であって、数値ではない

そのため、この手法だけでは次の 3 つができない。

- <strong>検める</strong>——書かれたものが合っているかを決定的に測れない
- <strong>直し方を示す</strong>——どこがどれだけ外れているかが出ない
- <strong>根拠を示す</strong>——なぜそう言えるのかがコーパスに紐づかない

### 実務上の壁

- <strong>日本語の文体埋め込みモデルが要る。</strong> 挙がっているのは英語のもの
- 180 万件の生成とデコーダの学習が要る。個人の手には重い
- 決定的でない

## 読み残し

実験の詳細と限界の節。
