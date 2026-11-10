#!/usr/bin/env bash
# Build OpenMC's src/random_lcg.cpp unmodified with g++ -O2 and run the driver,
# writing reference.txt beside this script. Gh:#1338 (case 6).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
up="$here/../../upstream_source/openmc"
want="8f79d9d02483c90e4134f03a9df4b75ce6b062bf"
build="${TMPDIR:-/tmp}/petir_openmc_lcg_build"
mkdir -p "$build"

have="$(git -C "$up" rev-parse HEAD)"
if [ "$have" != "$want" ]; then
  echo "upstream at $have, expected $want" >&2
  exit 1
fi

g++ -O2 -std=c++17 -ffp-contract=off -Wall -Wextra \
  -I"$up/include" \
  "$here/driver.cpp" "$up/src/random_lcg.cpp" \
  -o "$build/driver"

"$build/driver" > "$here/reference.txt"
echo "wrote $here/reference.txt ($(wc -l < "$here/reference.txt") lines)"
