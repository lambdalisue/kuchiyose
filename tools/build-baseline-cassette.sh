#!/usr/bin/env bash
# 同梱の基準カセットを作り直す。baselines/ の文書から cassette build で作り、
# 実行ファイルに埋め込むファイルへ書く（docs/design/100-cassette.md#同梱の基準カセット）。
#
# 指標の定義・解析器・測り方の設定・正規化を変えたら、ここを走らせる。
# 走らせ忘れると、埋め込んだカセットと baselines/ から作り直したカセットが
# 食い違い、試験（crates/kakiburi-cli/src/cassettes.rs）が落ちる。
#
# 一時の置き場へ新しく作ってから差し替える。 在るところへ build すると作り直しに
# なって世代が 1 つ進み、同じ文書から同じバイト列が出なくなる。同梱の基準には
# 調整が無いので、差し替えて失うものは無い。
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/crates/kakiburi-cli/assets/baseline.kb"
# 場面の名前。試験（cassettes.rs の BUNDLED_SCENE）と同じ名前を使う。
SCENE='同梱の基準'

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cargo run --release --quiet --manifest-path "$ROOT/Cargo.toml" -p kakiburi-cli -- \
  cassette build "$ROOT/baselines" -o "$TMP/baseline.kb" --scene "$SCENE"
mv "$TMP/baseline.kb" "$OUT"
