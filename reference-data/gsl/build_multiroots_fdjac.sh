#!/bin/bash
# Build and run multiroots_fdjac_reference_driver.c against the GSL 2.8 tree,
# linking GSL's own multiroots/test_funcs.c (unmodified) for the problems.
#
# usage: reference-data/gsl/build_multiroots_fdjac.sh <out.txt> [GSL-dir]
#   GSL-dir defaults to crates/petir/upstream_source/GSL (relative to cwd).
set -e
out=$1
G=${2:-crates/petir/upstream_source/GSL}
here=$(cd "$(dirname "$0")" && pwd)
exe=$(mktemp)
gcc -O2 -I"$G" -I"$G/multiroots" -DHAVE_CONFIG_H -o "$exe" \
  "$here/multiroots_fdjac_reference_driver.c" "$G/multiroots/test_funcs.c" \
  "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" -lm
"$exe" > "$out"
rm -f "$exe"
