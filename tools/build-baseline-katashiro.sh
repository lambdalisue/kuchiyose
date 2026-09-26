#!/usr/bin/env bash
# 同梱の基準形代を作り直す。baselines/ の文書から katashiro build で作り、
# 実行ファイルに埋め込むファイルへ書く（docs/design/100-katashiro.md#同梱の基準形代）。
#
# 指標の定義・解析器・測り方の設定・正規化を変えたら、ここを走らせる。
# 走らせ忘れると、埋め込んだ形代と baselines/ から作り直した形代が
# 食い違い、試験（crates/kuchiyose-cli/src/katashiros.rs）が落ちる。
#
# 一時の置き場へ新しく作ってから差し替える。 在るところへ build すると作り直しに
# なって世代が 1 つ進み、同じ文書から同じバイト列が出なくなる。同梱の基準には
# 調整が無いので、差し替えて失うものは無い。
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/crates/kuchiyose-cli/assets/baseline.katashiro"
# 場面の名前。試験（katashiros.rs の BUNDLED_SCENE）と同じ名前を使う。
SCENE='同梱の基準'

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cargo run --release --quiet --manifest-path "$ROOT/Cargo.toml" -p kuchiyose -- \
  katashiro build "$ROOT/baselines" -o "$TMP/baseline.katashiro" --scene "$SCENE"
# 素材のフォルダの経路は手元の事情なので外す。 残せば、作った人の経路が実行ファイルに
# 入り、どこで作り直したかによってバイト列が変わる。
cargo run --release --quiet --manifest-path "$ROOT/Cargo.toml" -p kuchiyose-katashiro \
  --example forget_material -- "$TMP/baseline.katashiro"
mv "$TMP/baseline.katashiro" "$OUT"
