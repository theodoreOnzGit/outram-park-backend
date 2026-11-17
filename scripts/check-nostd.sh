#!/usr/bin/env bash
#
# check-nostd.sh -- the `no_std` + `alloc` gate (epic gh:#1585).
#
# Each crate below has a default `std` feature; with it off the crate is
# `no_std` + `alloc`. This checks that, with `--no-default-features`, the
# library type-checks for `x86_64-unknown-none`: a bare-metal target that has
# `core` and `alloc` and NO `std`. A plain `--no-default-features` check on
# the host would pass even if a dependency quietly pulled `std` in; this one
# cannot.
#
# Compile-only, like the wasm and Android gates: it says the types line up
# without an operating system, not that anything has run on bare metal.
# The tests run with `std` (the crate is `std` in test builds).
#
# A crate is added here in the change that converts it (one child issue of
# gh:#1585 each), never before, and never removed to make the gate green.
#
#   scripts/check-nostd.sh            # every converted crate
#
# Release, like every other cargo invocation in this workspace.
set -euo pipefail

target="x86_64-unknown-none"
crates=(
  outram-foam-basic-lib       # gh:#1586
  outram-park-fork-coolprop   # gh:#1598
  tampines-steam-tables       # gh:#1599
  changi                      # gh:#1587
  bedok                       # gh:#1588
  outram-park-fork-onix       # gh:#1589
  outram-park-fork-thermochimica  # gh:#1590
  buangkok                    # gh:#1593
  outram-foam-multiphase      # gh:#1594
  outram-foam-turbulence-lib  # gh:#1595
  outram-park-fork-offbeat    # gh:#1596
  outram-park-fork-moltres    # gh:#1597
  njoy-outram-park-fork       # gh:#1592
  raffles                     # gh:#1610
  outram-blender              # gh:#1604
  outram-mc-libs              # gh:#1605
)

rustup target add "$target" >/dev/null
fail=0

# `no_std` maps are `hashbrown` through `crate::hash`, never a `BTreeMap`
# stand-in (docs/claude-md/rust-design-rules.md, maintainer 2026-11-16).
if grep -rnE 'BTree(Map|Set) as Hash(Map|Set)' --include='*.rs' crates; then
  echo "FAILED: a BTreeMap/BTreeSet stands in for HashMap/HashSet; use crate::hash (hashbrown)" >&2
  fail=1
fi
for c in "${crates[@]}"; do
  echo "== $c (no_std + alloc, $target)"
  if ! cargo check --release -p "$c" --lib --no-default-features --target "$target"; then
    echo "FAILED: $c does not build without std" >&2
    fail=1
  fi
done
exit "$fail"
