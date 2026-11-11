#!/usr/bin/env bash
# Build the VIEWR/graph differential driver against NJOY2016.80 and write reference.txt.
#  * locale.f90, phys.f90, mainio.f90, util.f90 and graph.f90 are compiled UNMODIFIED.
#  * `rget`, `iget`, `stripv` (private in graph.f90) and `ascalv` (private in viewr.f90) are
#    copied out by line range into small modules, after checking the SHA-256 of both files.
# Usage: NJOY2016_ROOT=<path to NJOY2016> OUT=<scratch dir> ./extract.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src="${NJOY2016_ROOT:?set NJOY2016_ROOT}/src"
out="${OUT:?set OUT to a scratch directory}"
mkdir -p "$out"
check() { echo "$2  $1" | sha256sum -c - >/dev/null || { echo "SHA-256 mismatch for $1" >&2; exit 1; }; }
check "$src/locale.f90" a763f2618fe52ee2190adba1448506334f3475e4be090047a625a3ca85d530c5
check "$src/phys.f90"   8b30cfe6ddf4d749827ea5ab52da5d1d384c124156164257904f9683cff39223
check "$src/mainio.f90" f505d251592346c7b335f483d577ef3b748cd81a1ab201e4114f8ff554c311e4
check "$src/util.f90"   7f58621219b54b1dd1fd9f93746548b0a5f1ca01734a128c9e10e60e5e0b42ea
check "$src/graph.f90"  70bd1cd22273aa2ec245f95975115f0ed0d05c1ca8e5065e349ce35085f59bd0
check "$src/viewr.f90"  89971eac0d0fb03d5f0ac2dd13b14e7ef5678d17b9e80bcc40e6915635f1fb4b
ln() { grep -n "$1" "$2" | head -1 | cut -d: -f1; }
a=$(ln '^   integer function iget' "$src/graph.f90")
b=$(ln '^   end function rget' "$src/graph.f90")
c=$(ln '^   subroutine stripv' "$src/graph.f90")
d=$(ln '^   end subroutine stripv' "$src/graph.f90")
e=$(ln '^   subroutine ascalv' "$src/viewr.f90")
f=$(ln '^   end subroutine ascalv' "$src/viewr.f90")
{ echo "module gutil"; echo "  implicit none"; echo "  integer,parameter::kr=kind(1.d0)"; echo "contains"
  sed -n "${a},${b}p" "$src/graph.f90"; sed -n "${c},${d}p" "$src/graph.f90"; echo "end module gutil"; } > "$out/gutil.f90"
{ echo "module ascmod"; echo "  implicit none"; echo "  integer,parameter::kr=kind(1.d0)"; echo "contains"
  sed -n "${e},${f}p" "$src/viewr.f90"; echo "end module ascmod"; } > "$out/ascmod.f90"
cd "$out"
rm -f tape6?
gfortran -O3 -c "$src/locale.f90" "$src/phys.f90" "$src/mainio.f90" "$src/util.f90" "$src/graph.f90" gutil.f90 ascmod.f90
gfortran -O3 -o viewr_driver "$here/viewr_driver.f90" locale.o phys.o mainio.o util.o graph.o gutil.o ascmod.o
./viewr_driver > numbers.txt
{ cat numbers.txt
  for n in 1 2 3 4 5 6; do echo "=== scene $n"; cat "tape6$n"; done; } > "$here/reference.txt"
echo "wrote $here/reference.txt: $(wc -l < "$here/reference.txt") lines"
