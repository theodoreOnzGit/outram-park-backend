#!/usr/bin/env bash
# Extract `ascle`, `ascll` (acefc.f90) and `mtname` (acecm.f90) from NJOY2016.80
# unchanged into $OUT, checking the SHA-256 of both sources first, build the
# differential driver and write reference.txt next to this script.
# Usage: NJOY2016_ROOT=<path to NJOY2016> OUT=<scratch dir> ./extract.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src="${NJOY2016_ROOT:?set NJOY2016_ROOT}/src"
out="${OUT:?set OUT to a scratch directory}"
mkdir -p "$out"
check() { echo "$2  $1" | sha256sum -c - >/dev/null || { echo "SHA-256 mismatch for $1" >&2; exit 1; }; }
check "$src/acefc.f90" 66ccec86db2fe1acbc9bb8e3713aecc21c9f14255b9ac622eb7eb66ed4f7c3f5
check "$src/acecm.f90" 504c5ac484f0911126de1921f1d3a3089196b7d544df56f9a016e4951ddfa973
a=$(grep -n '^   subroutine ascle' "$src/acefc.f90" | cut -d: -f1)
b=$(grep -n '^   end subroutine ascll' "$src/acefc.f90" | cut -d: -f1)
c=$(grep -n '^   subroutine mtname' "$src/acecm.f90" | cut -d: -f1)
d=$(grep -n '^   end subroutine mtname' "$src/acecm.f90" | cut -d: -f1)
{
  echo "module scale_mod"
  echo "  implicit none"
  echo "  integer,parameter::kr=kind(1.d0)"
  echo "contains"
  sed -n "${a},${b}p" "$src/acefc.f90"
  echo "end module scale_mod"
} > "$out/scale_mod.f90"
{
  echo "module endf"
  echo "  implicit none"
  echo "  integer,public::iverf=-1"
  echo "end module endf"
  echo "module acecm"
  echo "  implicit none"
  echo "  integer,parameter::kr=kind(1.d0)"
  echo "contains"
  sed -n "${c},${d}p" "$src/acecm.f90"
  echo "end module acecm"
} > "$out/acecm_mod.f90"
cd "$out"
gfortran -O3 -c scale_mod.f90 acecm_mod.f90
gfortran -O3 -o scale_driver "$here/scale_driver.f90" scale_mod.o acecm_mod.o
./scale_driver > "$here/reference.txt"
wc -l "$here/reference.txt"
