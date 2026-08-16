# Zeng & Nini (2026) Authorship Impersonation via LLM Prompting does not Evade Authorship Verification Methods

arXiv:2603.29454.
<https://arxiv.org/pdf/2603.29454>

<strong>本文を読んだ。LLM に特定の人物になりすまして書かせ、それが見破れるかを調べている。</strong>
立場は検証する側で、
向こうは「見破れるか」を問うている。

## 何をしたか

GPT-4o に、ある人物の文体を真似た文章を書かせ、法科学で使われる <strong>著者検証（authorship
verification）</strong> システムを欺けるかを試した。

- プロンプトは 4 通り（naive、self-prompting、role-play、tree-of-thought）
- 領域は 3 つ（メール、テキストメッセージ、SNS 投稿）
- 非ニューラル: n-gram tracing、Ranking-Based Impostors、LambdaG
- ニューラル: AdHominem、LUAR、STAR

## 結果

<strong>欺けない。</strong>

> LLM-generated texts <strong>failed to sufficiently replicate authorial individuality</strong> to bypass
> established AV systems

それどころか、<strong>本物の別人の文章より、なりすまし文の方が高い精度で弾かれた</strong>手法もある。

## なぜ失敗するのか——ここが重要

原因が特定されている。<strong>しかも測れる形で。</strong>

> Human-authored texts naturally contain <strong>recurring character or word n-grams that reflect
> an individual's habitual phrasing</strong>. Because LLM-generated texts exhibit <strong>higher entropy
> and greater lexical diversity</strong>, they <strong>fail to produce sufficient repetitions</strong> of these local
> sequences, which makes the impersonations stand out.

<strong>人の文体は繰り返しでできている。</strong> 同じ言い回しを癖として繰り返す。LLM は語彙が多様
すぎて、その繰り返しを作れない。

追試で 3 つの量を比べている。なりすまし文は、対象著者の文章より <strong>いずれも高かった</strong>
（3 コーパスすべてで）。

| 量 | LLM の方が |
| --- | --- |
| 圧縮後の大きさ | 大きい |
| エントロピー | 高い |
| Type-Token 比 | 高い |

ニューラル手法（LUAR、STAR）については、<strong>著者検証ではなく AI 検出をしている可能性</strong>
がある、と考察している。「対象著者の言葉でないから」ではなく「人間の文章の性質から
かけ離れているから」弾いているのではないか、と。

## 読み取れること

### 1. 失敗の中身が 1 つに特定されている

<strong>繰り返しが足りない。</strong> そして測れる——圧縮後の大きさ、エントロピー、Type-Token 比。
どれも計算が軽く、決定的である。

[Stamatatos](stamatatos-2009.md) は語彙の豊富さを「長さ依存なので単独では信頼できない」
としていた。<strong>著者識別の特徴としては弱くても、人か機械かを分ける量としては効く。</strong>

### 2. プロンプト以外の手がある

> decoding parameters such as temperature and frequency penalty were kept at their default
> values. Since these parameters <strong>directly affect lexical diversity and entropy</strong>, future work
> could systematically vary them

<strong>温度と頻度ペナルティを下げれば、繰り返しは増える。</strong> 論文自身が未検証と認めている。
<strong>プロンプト以外に動かせる要素がある</strong>ということである。

### 3. だが、目標を取り違えないこと

> although AV algorithms consistently rejected LLM-generated texts, such texts <strong>may still
> appear plausible to human readers</strong>

<strong>機械は弾くが、人は騙される。</strong>

<strong>著者検証の指標と、人が読んだときの印象は一致しない。</strong> 検証器を満点にするには人間離れ
した反復が要る。どちらを目標にするかで、寄せる先が変わる。

## 限界（論文自身の記載）

- 「入門的な攻撃者」を想定している。適応的な戦略やモデルの微調整は試していない
- AdHominem は部分的に脆弱だった。手法によって差がある
- 温度などの復号パラメータは既定値のまま
