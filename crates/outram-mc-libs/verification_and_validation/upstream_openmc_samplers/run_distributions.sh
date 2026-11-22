#!/usr/bin/env bash
# Build OpenMC's real source-distribution code (verbatim extracts, SHA-checked by
# extract_distributions.sh) against stubs_dist/ with g++ -O2 -ffp-contract=off and
# write reference_distributions.txt (gh:#1388). OPENMC_SRC = the pinned clone.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
up="${OPENMC_SRC:-$here/../../upstream_source/OpenMC}"
bash "$here/extract_distributions.sh"
build="${TMPDIR:-/tmp}/outram_openmc_dist_build"; mkdir -p "$build"
flags="-O2 -std=c++17 -ffp-contract=off -include cstdint -include openmc/error.h -I$here/stubs_dist -I$here/stubs -I$here/generated -I$up/include"
objs=""
for f in random_lcg random_dist position; do
  g++ $flags -c "$up/src/$f.cpp" -o "$build/$f.o"
  objs="$objs $build/$f.o"
done
g++ $flags "$here/distributions_driver.cpp" $objs -o "$build/dist_driver"
"$build/dist_driver" > "$here/reference_distributions.txt"
rm -rf "$build"
echo "wrote $here/reference_distributions.txt ($(wc -l < "$here/reference_distributions.txt") lines)"
