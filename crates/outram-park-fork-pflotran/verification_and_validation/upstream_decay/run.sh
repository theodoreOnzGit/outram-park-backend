#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.
#
# Compile the extracted upstream routines with driver.F90 and regenerate the
# reference outputs (gh:#1000). Two builds, because the flags change the bits:
#
#   upstream_output.txt            -O2 -fno-tree-vectorize: every `exp` is a
#                                  scalar call to glibc's `exp`.
#   upstream_output_O2_vector.txt  plain -O2: gfortran (glibc pre-includes
#                                  math-vector-fortran.h) vectorises the array
#                                  statement `solution = solution*exp(-rhs)` of
#                                  the IMPLICIT_SOLUTION path into libmvec's
#                                  `_ZGVbN2v_exp`, which is not glibc's scalar
#                                  `exp` bit for bit.
#
# Neither build uses -march, so neither contracts to FMA. The two
# zero-inventory probes run with a timeout; their outcome is recorded in
# README.md, not in the output files.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
gfortran --version | head -1
gfortran -O2 -fno-tree-vectorize -ffree-line-length-none -J "$build" \
  "$here/driver.F90" -o "$build/driver_scalar"
gfortran -O2 -ffree-line-length-none -J "$build" \
  "$here/driver.F90" -o "$build/driver_vector"
echo "vector exp symbols in the -O2 build:"
nm -D "$build/driver_vector" | grep -i '_ZGV' || echo "  (none)"
"$build/driver_scalar" > "$here/upstream_output.txt"
"$build/driver_vector" > "$here/upstream_output_O2_vector.txt"
echo "wrote upstream_output.txt ($(wc -l < "$here/upstream_output.txt") lines)"
echo "wrote upstream_output_O2_vector.txt ($(wc -l < "$here/upstream_output_O2_vector.txt") lines)"
echo "--- probe: zero parentless isotope (upstream aborts in LUDecomposition)"
timeout 10 "$build/driver_scalar" zeroparent || echo "exit status $?"
echo "--- probe: zero daughter (upstream Newton never converges; timeout 10 s)"
timeout 10 "$build/driver_scalar" zero || echo "exit status $? (124 = timed out)"
