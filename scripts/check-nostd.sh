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
  chem-eng-real-time-process-control-simulator  # gh:#1612
  teh-o-prke                  # gh:#1600
  tuas_boussinesq_solver      # gh:#1601
  outram-park-fork-liggghts   # gh:#1602
  outram-park-fork-pflotran   # gh:#1603
  outram-park-fork-dwsim-libs # gh:#1591
  farrer-park                 # gh:#1632
  outram-foam-appbuilder-lib  # gh:#1633
  boon-lay                    # gh:#1634
  tampines                    # gh:#1606
  outram-foam-mesh            # gh:#1638
  outram-park-fork-cfmesh     # gh:#1639
  nee_soon                    # gh:#1640
  sembawang                   # gh:#1641
  bishan                      # gh:#1642
  pasir-ris                   # gh:#1642
  deck-based-open-source-visualisation-engine-for-reactors-dover  # gh:#1643
  # Already no_std, gated 2026-11-17 (after the gh:#1585 survey). petir,
  # kaki-bukit and redhill are always `no_std` (no `std` feature); bugis is
  # checked without its `egui` default.
  petir
  kaki-bukit
  redhill
  bugis
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
