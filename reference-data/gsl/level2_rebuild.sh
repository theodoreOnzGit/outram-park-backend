#!/bin/bash
# Rebuild selected GSL 2.8 linalg files with CROSSOVER (linalg/recurse.h)
# raised to 100000, so GSL's recursive Level-3 blocking never triggers and it
# runs its Level-2 code at every size; link them ahead of the static libgsl.a
# and run a reference driver against the result.
#
# Why: PETIR ports GSL's Level-2 routines only (gh:#993). Above n = 24 stock
# GSL blocks, so its rounding differs from PETIR's. Comparing PETIR against
# this rebuild isolates that one cause: if PETIR is bit-identical to GSL-at-
# Level-2, the whole stock-GSL difference is the blocking and nothing else.
#
# usage (from the repository root, after building the static GSL as in
# README.md):
#   reference-data/gsl/level2_rebuild.sh <driver.c> <out.txt> <file.c>...
# e.g.
#   reference-data/gsl/level2_rebuild.sh \
#     reference-data/gsl/cholesky_reference_driver.c \
#     reference-data/gsl/cholesky-gsl-2.8-level2-reference.txt \
#     linalg/cholesky.c linalg/invtri.c linalg/trimult.c
set -e
G=crates/petir/upstream_source/GSL
S=$(mktemp -d)
driver=$1; out=$2; shift 2
srcs=()
for f in "$@"; do cp "$G/$f" "$S/"; srcs+=("$S/$(basename "$f")"); done
sed 's/#define CROSSOVER              24/#define CROSSOVER              100000/' \
    "$G/linalg/recurse.h" > "$S/recurse.h"
grep -q "CROSSOVER              100000" "$S/recurse.h"
gcc -O2 -I"$G" -I"$S" -DHAVE_CONFIG_H -o "$S/driver" "$driver" "${srcs[@]}" \
    "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" -lm
"$S/driver" > "$out"
rm -rf "$S"
