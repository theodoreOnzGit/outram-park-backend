#!/usr/bin/env bash
# Build the driver against OpenMC's extracted routines (+ the real position.cpp
# and quartic_solver.cpp, compiled unmodified) and write reference.txt.
# gh:#1386 prio-1 rows 1-2. -ffp-contract=off so no FMA is formed (Rust never
# contracts either); -O2 as the maintainer's brief says.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
up="${OPENMC_SRC:-$here/../../upstream_source/OpenMC}"
build="${TMPDIR:-/tmp}/outram_mc_surfaces_build"
mkdir -p "$build"
"$here/extract.sh"
g++ -O2 -std=c++17 -ffp-contract=off -Wall -Wno-unused-variable \
  -I"$here/stubs" -I"$here/generated" -I"$up/include" \
  "$here/driver.cpp" "$up/src/position.cpp" "$up/src/external/quartic_solver.cpp" \
  -o "$build/driver"
"$build/driver" > "$here/reference.txt"
echo "wrote $here/reference.txt ($(wc -l < "$here/reference.txt") lines)"
rm -rf "$build"
