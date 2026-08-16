# mStyleDistance (2025) Multilingual Style Embeddings and their Evaluation

arXiv:2502.15168.
<https://arxiv.org/html/2502.15168>

<strong>要旨とデータ構築の節を読んだ。日本語に対応した文体埋め込みが存在する。</strong>

## 何か

文体埋め込みは英語のものしか公開されていなかった。それを <strong>9 言語</strong>に広げたもの。

> Arabic, German, Spanish, French, Hindi, <strong>Japanese</strong>, Korean, Russian, and Chinese

合成データと対照学習で訓練し、多言語版の STEL-or-Content ベンチマーク（Wegmann et al.
2022）で評価している。著者検証タスクでも使われている。モデルは公開されている。

## 40 の文体特徴

Patel et al. (2024b) の 40 特徴を土台にしている。<strong>解釈できる分類になっている。</strong>

| 種類 | 例 |
| --- | --- |
| 統語的 | 能動/受動、短縮形、<strong>機能語の多用</strong> |
| 感情・認知 | 感情を示す語、認知過程を示す語 |
| 文体・美的 | 比喩、<strong>形式ばった調子</strong> |
| 社会・対人 | <strong>丁寧な調子</strong>、攻撃的な調子 |
| 図像・デジタル | 大文字化、<strong>絵文字</strong>、数字 |
| 時間・相 | 現在への焦点、未来への焦点 |

<strong>言語ごとに使えない特徴は落としている。</strong> 冠詞は中国語と日本語に無いので外された。

## 日本語についての注意

<strong>日本語のデータは英語からの機械翻訳である。</strong>

> The direct approach was selected for all languages in L <strong>except for Japanese and Hindi</strong>

各言語について「直接生成」と「英語で生成してから翻訳」を比べ、流暢さと特徴の再現度で
良い方を選んでいる。<strong>日本語は翻訳の方が選ばれた。</strong>

これは注意が要る。翻訳された日本語には翻訳特有の癖がある。<strong>そこで学習した埋め込みが、
日本語らしい文体をどこまで捉えているかは不明である。</strong>

全体の特徴再現度は 0.79、流暢さは 0.93（言語別の内訳は付録）。

## kakiburi にとって

<strong>判別の物差しとして、学習なしに使える可能性がある。</strong> 日本語対応の文体埋め込みが公開
されているなら、[Burrows's Delta](burrows-delta.md) より強い基準線になる。

そして 40 特徴の分類は、<strong>解釈できる指標の候補として使える。</strong> 丁寧さ、形式ばり、絵文字
——日本語でも意味を持つものが多い。[馬場の語の文体値](baba-2022.md)の 5 軸と重なる部分
もある。

<strong>ただし日本語部分が翻訳由来である点は、実際に当てて確かめる必要がある。</strong>

## 未読

本文の評価結果、モデルの入手先、日本語の内訳（付録 D）。使うと決めたら読むこと。
