#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# gh:#1448: build the extracted csunr1/csunr2/unfac/gnrl (NJOY2016.80, gfortran -O2, no -march)
# with driver.f90, run it on cases.in and write upstream_output.txt.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
up=${NJOY_UPSTREAM_SOURCE:-$here/../../../upstream_source}
src=$up/NJOY2016/src
build=$(mktemp -d); trap 'rm -rf "$build"' EXIT
"$here/extract.sh" "$build/extracted.inc"
gfortran --version | head -1
cp "$build/extracted.inc" "$here/extracted.inc"
(cd "$build" && gfortran -O2 -ffree-line-length-none -I"$here" -o drv \
  "$src/locale.f90" "$src/mainio.f90" "$src/util.f90" "$src/phys.f90" "$src/endf.f90" "$here/driver.f90")
rm -f "$here/extracted.inc"
"$build/drv" < "$here/cases.in" > "$here/upstream_output.txt"
echo "wrote upstream_output.txt ($(wc -l < "$here/upstream_output.txt") lines)"
