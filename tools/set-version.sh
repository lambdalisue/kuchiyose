#!/usr/bin/env bash
# Rewrite the placeholder version in the root Cargo.toml to the given one.
#
# Usage: tools/set-version.sh <version>   (e.g. 0.1.0 or v0.1.0)
#
# The tree holds a placeholder (0.0.0); the release tag is the only record of
# the version. Both the crates.io publish and the release binaries run this, so
# `kuchiyose --version` of a downloaded binary names the same version as the
# crate on crates.io.
set -euo pipefail

version="${1:?usage: tools/set-version.sh <version>}"
version="${version#v}"

# cargo would reject a non-semver version much later, after the build; fail on
# the argument itself instead. Close enough to semver that anything reaching
# cargo is accepted there: no leading zeros, a prerelease before build
# metadata, one of each at most.
printf '%s' "$version" |
  grep -Eq '^(0|[1-9][0-9]*)(\.(0|[1-9][0-9]*)){2}(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$' || {
  echo "${version} is not a semver version" >&2
  exit 1
}

cd "$(dirname "$0")/.."

# The version reaches perl through the environment, not through the program
# text, so a tag can never be read as code. Two shapes of line in the root
# Cargo.toml carry the placeholder, and only those are touched: the
# line-anchored `version =` of [workspace.package], which every crate inherits,
# and the exact `version = "=…"` requirement of each [workspace.dependencies]
# entry, which every crate depending on another member inherits.
VERSION="$version" perl -pi -e '
  s/^version = "[^"]*"$/version = "$ENV{VERSION}"/;
  s/^(kuchiyose-[a-z]+ = \{ path = "[^"]+", version = )"=[^"]*"/$1"=$ENV{VERSION}"/;
' Cargo.toml

# A line that stopped matching the patterns above would otherwise ship a crate
# that still says 0.0.0, or depends on one that does. cargo itself refuses a
# path dependency whose version the requirement does not match, so checking the
# members covers the requirements too.
cargo metadata --no-deps --format-version 1 |
  jq -e --arg v "$version" '[.packages[].version] | all(. == $v)' >/dev/null || {
  echo "not every workspace member reads ${version} after the rewrite" >&2
  exit 1
}

# `--workspace` re-resolves workspace members only — a dependency is touched
# solely if it has gone missing from the lock — so what gets built carries the
# committed dependency set, and --locked afterwards still means the committed
# lock.
cargo update --workspace
