#!/usr/bin/env bash
# Differential harness for OpenMC `scatter()`'s reaction walk (physics.cpp, 8f79d9d0):
# extracts lines 732-775 VERBATIM (commit + SHA-256 checked) into a wrapper whose
# elastic_scatter / sab_scatter / inelastic_scatter are recorders, then prints,
# for fixed xi from the real OpenMC LCG, which reaction the walk chose.
# Output: reference.txt (hex bits of cutoff, chosen index). gh:#1386 / #1444.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
up="${OPENMC_SRC:-$here/../../upstream_source/OpenMC}"
want="8f79d9d02483c90e4134f03a9df4b75ce6b062bf"
[ "$(cd "$up" && git rev-parse HEAD)" = "$want" ] || { echo "wrong upstream commit" >&2; exit 1; }
[ "$(sha256sum "$up/src/physics.cpp" | cut -d' ' -f1)" = "6a9e54382e63cca6dc0aa2540801132feea42187e10b0dc9b0f9d933c04fc7b9" ] || { echo "physics.cpp SHA mismatch" >&2; exit 1; }
[ "$(sha256sum "$up/src/random_lcg.cpp" | cut -d' ' -f1)" = "0aa9193434b5b0d4b85db4a399d5f44adab8bbf7c500fa11609d16c7a33e0182" ] || { echo "random_lcg.cpp SHA mismatch" >&2; exit 1; }
[ "$(sed -n 732p "$up/src/physics.cpp")" = "  double prob = micro.elastic - micro.thermal;" ] || { echo "line 732 moved" >&2; exit 1; }
[ "$(sed -n 775p "$up/src/physics.cpp")" = "  }" ] || { echo "line 775 moved" >&2; exit 1; }
build="${TMPDIR:-/tmp}/outram_reaction_walk_build"; mkdir -p "$build"
sed -n 732,775p "$up/src/physics.cpp" > "$build/walk_extract.inc"
flags="-O2 -std=c++17 -ffp-contract=off -include cstdint -I$up/include"
g++ $flags -c "$up/src/random_lcg.cpp" -o "$build/random_lcg.o"
g++ $flags -I"$build" "$here/driver.cpp" "$build/random_lcg.o" -o "$build/driver"
"$build/driver" > "$here/reference.txt"
rm -rf "$build"
echo "wrote $here/reference.txt ($(wc -l < "$here/reference.txt") lines)"
