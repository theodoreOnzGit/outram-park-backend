#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# gh:#1368: build the extracted acelf6 ismooth block for 2016.79 and 2016.80 (gfortran -O2, same flags)
# and write upstream_output_v{79,80}.txt. Set NJOY_UPSTREAM_SOURCE if the clones are elsewhere.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
build=$(mktemp -d); trap 'rm -rf "$build"' EXIT
gfortran --version | head -1
for v in 79 80; do
  "$here/extract.sh" $v "$build/$v" >/dev/null
  (cd "$build/$v" && gfortran -O2 -ffree-line-length-none -o drv locale.f90 mainio.f90 util.f90 phys.f90 "ismooth_extract_v$v.f90" "$here/driver.f90")
  "$build/$v/drv" | grep '^[IDRE]' > "$here/upstream_output_v$v.txt"
  echo "v$v: $(grep -c ^R "$here/upstream_output_v$v.txt") cases"
done
