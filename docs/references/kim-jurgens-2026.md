# Kim & Jurgens (2026) Interpreting Style Representations via Style-Eliciting Prompts

arXiv:2606.05716. University of Michigan.
<https://arxiv.org/pdf/2606.05716>

<strong>要旨・序論・関連研究を読んだ。</strong> [Stamatatos](stamatatos-2009.md) が 2009 年に未解決と
名指しした「文体を高い水準で言い表す」に、正面から取り組んでいる。<strong>kakiburi の貸す側と
真っ向から競合する。</strong>

## 前提となっている技術

<strong>文体埋め込み（style representation）</strong>。同じ著者の文章が近くに来るように学習した
ベクトル表現（Rivera-Soto et al. 2021、Wegmann et al. 2022、Patel et al. 2025）。

> These dense vector representations have resulted in significant performance gains for
> many tasks like authorship attribution <strong>over older, more interpretable methods from
> stylometry</strong>

<strong>古典的な計量文体論より精度が高い。</strong> つまり [Burrows's Delta](burrows-delta.md) や
特徴量の手作りは、判別性能では既に古い。

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

3 つ目は kakiburi のやりたいことそのものである。

## kakiburi にとって

<strong>厳しい話と、採るべき話が両方ある。</strong>

### 採るべきもの

<strong>「制御できることを説明とみなす」は、そのまま合否判定に使える。</strong>

kakiburi はこれまで合否を「本人が読んで受け入れる」に置いていた。主観的で、高くつき、
回数を稼げない。この論文の基準なら <strong>機械で回せる</strong>——出した指示で書かせて、元の文章の
文体に寄ったかを測ればよい。

そしてこれは kakiburi の構造とよく合う。貸すと検めるが同じものを見る、という設計は、
まさに「指示で書かせて、同じ物差しで測り直す」ことだった。

### 厳しい話

<strong>判別のための特徴を手で作る道は、性能では終わっている。</strong> 文体埋め込みが古典的な計量
文体論を上回ると明記されている。

<strong>そして貸す側でも、既に手法がある。</strong> kakiburi が「未検証の領域」と考えていた場所に、
別のアプローチが先に立っている。

### それでも kakiburi に残るもの

この手法が出すのは <strong>自然言語の指示</strong> であって、数値ではない。したがって次ができない。

- <strong>検める</strong>——書かれたものが合っているかを決定的に測れない
- <strong>直し方を示す</strong>——どこがどれだけ外れているかが出ない
- <strong>本人が納得する</strong>——なぜそう言えるのかの根拠がコーパスに紐づかない

kakiburi は「代筆させて、検めて、直す」ループを回す道具である。生成の質だけを競うなら
この論文の方が強いが、<strong>ループには測れる数値が要る</strong>。

### 実務上の壁

- <strong>日本語の文体埋め込みモデルが要る。</strong> 挙がっているのは英語のもの
- 180 万件の生成とデコーダの学習が要る。個人の手には重い
- 決定的でない

## 読み残し

実験の詳細と限界の節。kakiburi の合否判定を設計するときに読むこと。
