#!/usr/bin/env bash
# Cases for check-image-version.sh. Run: bash .github/scripts/check-image-version.test.sh
set -uo pipefail

script="$(cd "$(dirname "$0")" && pwd)/check-image-version.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf '[package]\nname = "x"\nversion = "2.1.0"\n\n[dependencies]\nfoo = { version = "9.9.9" }\n' >"$tmp/plain.toml"
printf '[package]\nname = "x"\nversion.workspace = true\n\n[dependencies]\nversion = "9.9.9"\n' >"$tmp/workspace.toml"
printf '[package]\nname = "x"\nversion_note = "a"\nversion = "1.2.3" # comment\n' >"$tmp/tricky.toml"
printf '[package]\nname = "x"\nversion = "2.1.0-rc1"\n' >"$tmp/prerelease.toml"
printf '[workspace]\nversion = "5.0.0"\n\n[package]\nname = "x"\nversion = "1.0.0"\n' >"$tmp/order.toml"

pass=0 fail=0
expect() {
  local want=$1 name=$2
  shift 2
  bash "$script" "$@" >/dev/null 2>&1
  local got=$?
  if [ "$got" = "$want" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "FAIL $name: want exit $want, got $got"
  fi
}

V2=refs/heads/v2_0
expect 0 "matching version" "$tmp/plain.toml" 2.1.0 false refs/heads/feature
expect 1 "mismatched version" "$tmp/plain.toml" 2.0.99 false "$V2"
expect 1 "empty version" "$tmp/plain.toml" "" false "$V2"
expect 1 "latest" "$tmp/plain.toml" latest false "$V2"
expect 1 "path traversal" "$tmp/plain.toml" ../x false "$V2"
expect 1 "leading v" "$tmp/plain.toml" v2.1.0 false "$V2"
expect 1 "trailing space" "$tmp/plain.toml" "2.1.0 " false "$V2"
expect 1 "pre-release" "$tmp/plain.toml" 2.1.0-rc1 false "$V2"
expect 1 "dependency version is not the crate version" "$tmp/plain.toml" 9.9.9 false "$V2"
expect 0 "latest from v2_0" "$tmp/plain.toml" 2.1.0 true "$V2"
expect 1 "latest from a branch" "$tmp/plain.toml" 2.1.0 true refs/heads/feature
expect 1 "workspace-inherited version" "$tmp/workspace.toml" 9.9.9 false "$V2"
expect 0 "version_* keys and trailing comment" "$tmp/tricky.toml" 1.2.3 false "$V2"
expect 0 "only [package] counts" "$tmp/order.toml" 1.0.0 false "$V2"
expect 1 "not [workspace]" "$tmp/order.toml" 5.0.0 false "$V2"
expect 1 "pre-release crate version is not a plain tag" "$tmp/prerelease.toml" 2.1.0-rc1 false "$V2"
expect 1 "missing argument" "$tmp/plain.toml" 2.1.0 false

echo "SUMMARY pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
