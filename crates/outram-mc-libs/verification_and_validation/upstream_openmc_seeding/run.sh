#!/usr/bin/env bash
# Build OpenMC random_lcg.cpp unmodified + the verbatim simulation.cpp extract
# with g++ -O2 and write reference.txt. Set OPENMC_SRC to the pinned clone
# (default ../../upstream_source/OpenMC).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
up="${OPENMC_SRC:-$here/../../upstream_source/OpenMC}"
bash "$here/extract.sh"
build="${TMPDIR:-/tmp}/outram_openmc_seeding_build"; mkdir -p "$build"
g++ -O2 -std=c++17 -ffp-contract=off -Wall -Wextra -I"$here/../upstream_openmc_samplers/stubs" -I"$up/include" -I"$here/generated" -include cstdint -include openmc/error.h \
  "$here/driver.cpp" "$up/src/random_lcg.cpp" "$up/src/random_dist.cpp" -o "$build/driver"
"$build/driver" > "$here/reference.txt"
rm -rf "$build"
echo "wrote $here/reference.txt ($(wc -l < "$here/reference.txt") lines)"
