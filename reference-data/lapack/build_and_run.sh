#!/bin/bash
# Build and run gebal_reference_driver.c twice (gh:#998):
#   1. against the system Reference-LAPACK/BLAS (-llapack -lblas), which is
#      what GNU Octave links here; its output is the committed reference;
#   2. against DGEBAL, DGEBAK and their callees compiled with gfortran from
#      a Reference-LAPACK source checkout, to show the system library is
#      that source.
# usage (from the repository root):
#   reference-data/lapack/build_and_run.sh <lapack-v3.12.1 checkout> <out.txt>
set -e
src=$1; out=$2
tmp=$(mktemp -d)
gcc -O2 -o "$tmp/sys" reference-data/lapack/gebal_reference_driver.c -llapack -lblas -lm
"$tmp/sys" reference-data/octave/expm-octave-11.3.0-reference.txt > "$out"
( cd "$tmp" && gfortran -O2 -c \
    "$src/SRC/dgebal.f" "$src/SRC/dgebak.f" "$src/SRC/disnan.f" "$src/SRC/dlaisnan.f" \
    "$src/INSTALL/dlamch.f" "$src/BLAS/SRC/lsame.f" "$src/BLAS/SRC/xerbla.f" \
    "$src/BLAS/SRC/idamax.f" "$src/BLAS/SRC/dscal.f" "$src/BLAS/SRC/dswap.f" \
    "$src/BLAS/SRC/dnrm2.f90" )
gcc -O2 -o "$tmp/src" reference-data/lapack/gebal_reference_driver.c "$tmp"/*.o -lgfortran -lm
"$tmp/src" reference-data/octave/expm-octave-11.3.0-reference.txt > "$tmp/src.txt"
if cmp -s "$out" "$tmp/src.txt"; then
  echo "system liblapack and the compiled source agree byte for byte"
else
  echo "system liblapack and the compiled source DIFFER"; exit 1
fi
rm -rf "$tmp"
