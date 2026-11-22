#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Build NJOY2016 mathm.f90 (unmodified; identical in 2016.79 and 2016.80, SHA below)
# with the driver and write upstream_output.txt (gh:#1366). gfortran -O2, no -march.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src=$here/../../../upstream_source/NJOY2016/src
echo "138488ea54bb90b5899977dedd7c1c77551f643fe1234f0778e39ea23d2bf7ad  $src/mathm.f90" | sha256sum -c -
build=$(mktemp -d); trap 'rm -rf "$build"' EXIT
gfortran --version | head -1
gfortran -O2 -ffree-line-length-none -J "$build" -o "$build/drv" \
  "$src/locale.f90" "$src/mainio.f90" "$src/util.f90" "$src/mathm.f90" "$here/driver.f90"
"$build/drv" > "$here/upstream_output.txt"
echo "wrote upstream_output.txt ($(wc -l < "$here/upstream_output.txt") lines)"
