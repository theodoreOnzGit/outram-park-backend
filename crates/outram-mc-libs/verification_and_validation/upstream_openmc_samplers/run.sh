#!/usr/bin/env bash
# Build OpenMC's real sampler sources (unmodified, SHA-checked by extract.sh)
# plus the verbatim extracts, against stubs/ (in-memory HDF5, fmt, pugixml,
# version.h, error.h), with g++ -O2 -ffp-contract=off, and write reference.txt.
# Set OPENMC_SRC to the pinned clone (default ../../upstream_source/OpenMC).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
up="${OPENMC_SRC:-$here/../../upstream_source/OpenMC}"
bash "$here/extract.sh"
build="${TMPDIR:-/tmp}/outram_openmc_samplers_build"; mkdir -p "$build"
flags="-O2 -std=c++17 -ffp-contract=off -include cstdint -include openmc/error.h -I$here/stubs -I$here/generated -I$up/include"
objs=""
for f in random_lcg random_dist distribution_energy distribution_angle secondary_uncorrelated \
         secondary_kalbach secondary_correlated secondary_nbody secondary_thermal position; do
  g++ $flags -c "$up/src/$f.cpp" -o "$build/$f.o"
  objs="$objs $build/$f.o"
done
g++ $flags "$here/driver.cpp" $objs -o "$build/driver"
"$build/driver" > "$here/reference.txt"
# fission-neutron differential (gh:#1444 d): same objects, own stubs
fflags="-O2 -std=c++17 -ffp-contract=off -include cstdint -include openmc/error.h -I$here/stubs -I$here/generated -I$up/include"
g++ $fflags "$here/fission_driver.cpp" $objs -o "$build/fission_driver"
"$build/fission_driver" > "$here/reference_fission.txt"
echo "wrote $here/reference_fission.txt ($(wc -l < "$here/reference_fission.txt") lines)"
rm -rf "$build"
echo "wrote $here/reference.txt ($(wc -l < "$here/reference.txt") lines)"
