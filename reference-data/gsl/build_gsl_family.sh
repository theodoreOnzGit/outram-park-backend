#!/bin/bash
# Build and run one GSL reference driver against the per-directory static
# archives of GSL 2.8 (gh:#1340/#1341 drivers), writing its output.
#
# One-off setup of the GSL tree (ampl/gsl@cf180cd7, cloned to
# crates/petir/upstream_source/GSL, see its README.md):
#   ./configure --disable-shared --enable-static
#   make -C gsl            # links the public headers into gsl/
#   for d in utils sys err integration interpolation linalg vector block \
#            matrix blas cblas permutation specfunc complex poly; do
#     make -C $d libgsl$d.la; done      # libutils.la for utils
#
# usage (from the repository root):
#   reference-data/gsl/build_gsl_family.sh <driver.c> <out.txt> [extra gcc args]
set -e
G=crates/petir/upstream_source/GSL
driver=$1; out=$2; shift 2
exe=$(mktemp)
libs=$(ls $G/*/.libs/*.a)
gcc -O2 -I"$G" -o "$exe" "$driver" "$@" -Wl,--start-group $libs -Wl,--end-group -lm
"$exe" > "$out"
rm -f "$exe"
