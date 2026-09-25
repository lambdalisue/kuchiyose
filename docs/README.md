# 文書

<strong>仕様・設計・先行研究に分かれている。</strong> 読む順は目的で決まる。

| 層 | 何が書いてあるか |
| --- | --- |
| [用語集](glossary.md) | <strong>言葉の意味。</strong> カセット・照合値・帯など、自前の用語 |
| [仕様](#仕様) | <strong>なぜそうなっているか。</strong> 判断とその理由 |
| [設計](#設計) | <strong>どう作られているか。</strong> クレート・保存・コマンド・テスト |
| [先行研究](#先行研究) | <strong>何を根拠にしているか。</strong> 借りたものと借りなかったもの |

<strong>仕様と設計を混ぜない。</strong> 仕様は「なぜ」を書き、設計は「どう」を書く。
実装の都合で仕様が変わることはあるが、そのときは仕様のほうを先に直す。

## どこから読むか

| 知りたいこと | 行き先 |
| --- | --- |
| この言葉は何を指すのか | [用語集](glossary.md) |
| この道具は何に賭けているのか | [仕様 000-axis](spec/000-axis.md) |
| 使い方 | [README](../README.md) |
| なぜ判定が 3 値なのか | [仕様 010-strategy](spec/010-strategy.md) |
| どの指標を、なぜ測るのか | [仕様 100-metrics](spec/100-metrics.md)、[指標の一覧](spec/metrics/README.md) |
| 目盛りはどう作られるか | [仕様 200-extract](spec/200-extract.md) |
| コードのどこに何があるか | [設計 000-architecture](design/000-architecture.md) |
| カセットの中身 | [設計 100-cassette](design/100-cassette.md) |

<strong>いちばん短い道は [仕様 000-axis](spec/000-axis.md) である。</strong> 目的・仮説・
反証のしかた・確かめられていないことが 1 枚に収まっている。

## 仕様

<strong>なぜそうなっているかを書く。</strong> 番号は読む順で、前の文書の判断を後ろが受ける。

| | |
| --- | --- |
| [000-axis](spec/000-axis.md) | 軸。目的と仮説と、その反証のしかた |
| [010-strategy](spec/010-strategy.md) | 戦略。3 値・3 段・場面という形がどこから出るか |
| [020-document](spec/020-document.md) | 文書の形。地の文とは何か、日本語の文字とは何か |
| [030-normalize](spec/030-normalize.md) | 入力を正規形にする。何を潰し、何を残すか |
| [100-metrics](spec/100-metrics.md) | 指標を決める。根拠の層と、粗い括りに丸めない規則 |
| [200-extract](spec/200-extract.md) | 評価して、その人の値と目盛りにする |
| [300-revise](spec/300-revise.md) | 検めて、直す |

[指標の定義](spec/metrics/README.md)は 1 指標 1 ファイル。意味・出どころ・数え方・次元・
除外・直し方が同じ形で並ぶ。<strong>ここにあるのは定義だけで、どう使うかは仕様の本体にある。</strong>

## 設計

<strong>どう作られているかを書く。</strong>

| | |
| --- | --- |
| [000-architecture](design/000-architecture.md) | 全体の構造とクレートの分割 |
| [100-cassette](design/100-cassette.md) | カセットの構造。何を収め、何を捨ててよいか |
| [200-command](design/200-command.md) | コマンドの体系 |
| [300-test](design/300-test.md) | テストの体系 |

## 先行研究

[一覧と読み方](references/README.md)。<strong>借りたものと借りなかったものを、両方書く。</strong>
「kakiburi で使える」とは書かない——食い違いは食い違いとして残す。

日本語の書き手識別で確かめられているものだけを層 1 とし、別の目的の研究から取るものは
層 2 として、<strong>効くかの判定を通るまで同じ重さで扱わない</strong>
（[100-metrics](spec/100-metrics.md)）。


## 書くときの約束

<strong>増減する数を本文に書かない。</strong> 「3 つの指標」と書けば、4 つ目を足した日に
文書が嘘になる。数える代わりに名前を挙げる。

<strong>暫定値はそう名乗る。</strong> 骨格が通るまで閾値を手で決めない。決めたなら、
どこから来た値かと、いつ導き直すかを書く。

<strong>リンクは `tools/checklinks.pl` が検める。</strong> 見出しを変えたら、指しているほうも直す。
