#!/usr/bin/env bash
# 基準の池を作る。**プロジェクト側の一度きりの仕事**で、利用者には回らない。
#
# 基準は「機械がどう書くか」であって書き手ごとに変わらない。題材の統制は
# 対ではなく素材全体に効かせるので（docs/spec/200-extract.md）、題材を広く
# 取った池を作っておき、review のときに近い分を選べばよい。
#
# **ここだけ非決定的である。** 生成した時点で素材になり、以降は決定的に回る。
# 収集と同じ扱いで、分離したコマンドにして、出力はファイルに置く。
set -u

OUT=${1:-baselines}
MODEL=${KAKIBURI_BASELINE_MODEL:-sonnet}
# 文体は必ず指定する。 指定しなければ LLM は常体に寄り、敬体で書く人の床が
# 敬体の機械文より下に来る——題材の近い機械の草稿が通ってしまう。
# 同じ題材を両方の文体で持ち、review が本人の文体に合う側を選ぶ。
REGISTER=${KAKIBURI_BASELINE_REGISTER:-dearu}
case "$REGISTER" in
  dearu) STYLE='だ・である調で書く'; SUFFIX='' ;;
  desu) STYLE='です・ます調で書く'; SUFFIX='-desu' ;;
  *) echo "KAKIBURI_BASELINE_REGISTER は dearu か desu" >&2; exit 64 ;;
esac
mkdir -p "$OUT"

# 題材は「登場する物」まで書く。題名だけを渡した LLM は固有名詞をほとんど
# 書かず、本人の記事とは別の語彙になる（docs/spec/010-strategy.md）。
while IFS='|' read -r id title things len; do
  [ -z "${id:-}" ] && continue
  # <strong>長さを散らす。</strong> 池が同じ長さに固まると、長さの範囲の防護柵に当たって
  # 目盛りが作れない——重なり ÷ それぞれの範囲が両方 0.5 以上要る
  # （crates/kakiburi-scale/src/lib.rs の length_range_ok）。
  len=${len:-3,000}
  f="$OUT/$id$SUFFIX.md"
  if [ -s "$f" ]; then
    echo "skip ${id} : 既にある" >&2
    continue
  fi
  # **コードブロックを書かせない。** 地の文だけが計測の対象で、コードは
  # 落とされる（docs/spec/020-document.md#地の文）。入れさせると、字数を
  # 満たしていても測れる語数が下限を割る。
  prompt="次の題材で技術記事を書いてください。

題材: $title
登場するもの: $things
長さ: 地の文だけで $len 字程度
形式: Markdown。見出しと箇条書きは使ってよい

条件:
- コードブロックは書かない。設定やコマンドは文章で説明する
- 前置きや後書きは不要。記事本文だけを出力する
- 日本語の技術ブログ記事として書く
- $STYLE"

  # <strong>標準入力を塞ぐ。</strong> 塞がないと claude が題材表の残りを読み、
  # ループが 1 周で終わる。
  if out=$(claude -p --model "$MODEL" "$prompt" </dev/null 2>/dev/null) && [ -n "$out" ]; then
    printf '%s\n' "$out" >"$f"
    # **全体をコードブロックで包んで返すことがある。** 包まれたままだと地の文が
    # 0 字になって断られるので、外側の囲みだけを外す。
    perl -0pi -e 's/\A```(?:markdown|md)?\n(.*)\n```\s*\z/$1\n/s' "$f"
    n=$(perl -CSD -Mutf8 -e 'local $/; my $t = <>; $t =~ s/\s//g; print length $t' "$f")
    echo "ok   ${id} : ${n} 字" >&2
  else
    # **失敗が計測を巻き込まない。** 足りない分は次に回す。
    echo "fail $id" >&2
  fi
done
