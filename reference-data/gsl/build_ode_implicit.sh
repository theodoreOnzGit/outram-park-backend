#!/bin/bash
# Build and run ode_implicit_reference_driver.c against the GSL 2.8 tree.
# The driver #includes ode-initval2/rk1imp.c (file-static state), so it needs
# the GSL source dir on -I as well as the built static libs.
#
# usage: reference-data/gsl/build_ode_implicit.sh <out.txt> [GSL-dir]
#   GSL-dir defaults to crates/petir/upstream_source/GSL (relative to cwd).
set -e
out=$1
G=${2:-crates/petir/upstream_source/GSL}
here=$(cd "$(dirname "$0")" && pwd)
exe=$(mktemp)
gcc -O2 -I"$G" -I"$G/ode-initval2" -DHAVE_CONFIG_H -o "$exe" \
  "$here/ode_implicit_reference_driver.c" \
  "$G/.libs/libgsl.a" "$G/cblas/.libs/libgslcblas.a" -lm
"$exe" > "$out"
rm -f "$exe"
