# Jangra et al. (2025) Evaluating Style-Personalized Text Generation: Challenges and Directions

arXiv:2508.06374. Columbia / Microsoft / York.
<https://arxiv.org/pdf/2508.06374>

<strong>要旨と序論を読んだ。</strong> 評価の作り方についての研究。

## 分野の名前

<strong>SPTG（Style-Personalized Text Generation）</strong>。「write like me」。特定の書き手の文体で
書かせる課題には、この名前が付いている。

## 何を問題にしているか

<strong>評価指標が標準化されておらず、人の判断とよく相関しない。</strong>

よく使われるのは BLEU のような n-gram の重なり、埋め込み、LLM-as-judge だが、どれも
既知の限界がある。文体転換の分野では <strong>簡単に騙せる</strong>（Krishna et al. 2020）ことも
報告されている。

そもそも LLM は著者固有の文体をうまく写せない（[Bhandarkar 2024](bhandarkar-2024.md)）
のだから、<strong>指標が測りたいものを測れていたのかを疑うべきだ</strong>、という問題意識。

## 結論

<strong>複数の指標を組み合わせた方が、単独の評価器より一貫して良い。</strong>

> employing ensembles of diverse evaluation metrics consistently outperforms
> single-evaluator methods

評価は 8 種の書く課題、3 つの設定（領域判別、著者識別、個人化した生成とそうでない
生成の判別）で行っている。

<strong>低資源の設定</strong>を「参照できる文体テキストが 1,500 語未満」と定義している。

## 押さえておく点

- <strong>単一の評価器では足りない。</strong>[Wang 2025](wang-2025.md) の 4 指標の組み合わせと同じ
  方向を指している
- <strong>単一指標は騙せる。</strong> 文体転換の分野で報告されている。1 つの点数を上げる最適化は
  抜け道を見つける
- <strong>「1,500 語未満が低資源」</strong>という線引きが、この論文で定義されている

## 未読

本文の評価結果と、どの指標の組み合わせが良かったかの詳細は読んでいない。合否判定を
設計するときに読むこと。
