#!/bin/bash
# Build and run one GSL reference driver against the static GSL 2.8 built
# from the vendored tree (see README.md), writing its output.
#
# usage (from the repository root):
#   reference-data/gsl/build_driver.sh <driver.c> <out.txt> [libm-shim.a]
#
# With a third argument, that static archive is linked ahead of glibc's -lm.
# `libm_shim.rs` in this directory builds one exporting the `libm` crate's
# `hypot`, the one PETIR uses (recipe in README.md, "Isolating `hypot`").
set -e
G=crates/petir/upstream_source/GSL
driver=$1; out=$2; shim=$3
exe=$(mktemp)
gcc -O2 -I"$G" -o "$exe" "$driver" "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" $shim -lm
"$exe" > "$out"
rm -f "$exe"
