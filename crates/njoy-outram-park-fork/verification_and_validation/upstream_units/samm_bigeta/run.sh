#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Build the extracted upstream routines for 2016.79 and 2016.80 with gfortran
# and write upstream_output_v{79,80}.txt (gh:#1366). -O2, no -march (no FMA),
# same flags for both versions so the only difference is upstream's.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
build=$(mktemp -d); trap 'rm -rf "$build"' EXIT
gfortran --version | head -1
for v in 79 80; do
  "$here/extract.sh" $v "$build/$v" >/dev/null
  (cd "$build/$v" && gfortran -O2 -ffree-line-length-none -o drv locale.f90 mainio.f90 util.f90 phys.f90 \
     "samm_extract_v$v.f90" "$here/driver.f90")
  "$build/$v/drv" > "$here/upstream_output_v$v.txt"
  echo "v$v: $(wc -l < "$here/upstream_output_v$v.txt") lines"
done
