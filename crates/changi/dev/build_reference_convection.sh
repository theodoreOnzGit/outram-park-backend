#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the convection FLEXPART reference driver
# (dev/flexpart_reference_convection.f90) at BOTH precisions, writing
#
#   tests/data/flexpart_convection_real4.csv   as FLEXPART ships (default real(4))
#   tests/data/flexpart_convection_real8.csv   -fdefault-real-8 -fdefault-double-8
#
# Routines under test (compiled VERBATIM from upstream): convect43c.f90
# (CONVECT, TLIFT), calcmatrix.f90, redist.f90, convmix.f90, plus sort2.f90
# (Numerical Recipes; called by the driver and by convmix, NOT ported), qvsat.f90
# and ew.f90, the modules com_mod, conv_mod, flux_mod, outg_mod, and
# calcfluxes.f90 (linked because convmix references it; never called, iflux=0).
#
# Two substitutions, neither inside a routine under test:
#   * par_mod.f90 is compiled from a COPY with the single line
#       maxnests=0,nxmaxn=0,nymaxn=0  ->  maxnests=1,nxmaxn=12,nymaxn=12
#     so convmix's nested-grid branch can run. par_mod.f90 is FLEXPART's
#     user-edited configuration file; the substitution is checked below.
#   * random_mod is dev/random_mod_shim_convection.f90, whose ran3 serves a queue the
#     driver fills (NR ran3 is not ported; the Rust port takes draws as input).
#   * class_gribfile is dev/class_gribfile_shim.f90 (the integer parameters
#     only), as in the other harnesses.
#
# Build products go to $FLEXPART_BUILD (default: a mktemp dir), never the repo.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
OUT="$CRATE_DIR/tests/data"
BUILD="${FLEXPART_BUILD:-$(mktemp -d)}"

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }

CONV_FILES=(
  "$SRC/com_mod.f90" "$SRC/conv_mod.f90" "$SRC/flux_mod.f90" "$SRC/outg_mod.f90"
  "$SRC/ew.f90" "$SRC/qvsat.f90" "$SRC/sort2.f90"
  "$SRC/convect43c.f90" "$SRC/calcmatrix.f90" "$SRC/redist.f90"
  "$SRC/calcfluxes.f90" "$SRC/convmix.f90"
)

build_and_run_convection () {
  local tag="$1"; shift
  local extra=("$@")
  local dir="$BUILD/convection_$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  sed -e 's/maxnests=0,nxmaxn=0,nymaxn=0/maxnests=1,nxmaxn=12,nymaxn=12/' \
    "$SRC/par_mod.f90" > "$dir/par_mod.f90"
  grep -q 'maxnests=1,nxmaxn=12,nymaxn=12' "$dir/par_mod.f90"
  [ "$(diff "$SRC/par_mod.f90" "$dir/par_mod.f90" | grep -c '^>')" -eq 1 ]
  gfortran -O2 -mcmodel=medium -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "$dir/par_mod.f90" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" \
    "$CRATE_DIR/dev/random_mod_shim_convection.f90" \
    "${CONV_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_convection.f90" \
    -o "$dir/flexpart_reference_convection"
  "$dir/flexpart_reference_convection"
  rm -rf "$dir"
}

mkdir -p "$OUT"
echo "building real(4) convection reference (as FLEXPART ships)..." >&2
build_and_run_convection real4 > "$OUT/flexpart_convection_real4.csv"
echo "building real(8) convection reference..." >&2
build_and_run_convection real8 -fdefault-real-8 -fdefault-double-8 \
  > "$OUT/flexpart_convection_real8.csv"

for f in "$OUT"/flexpart_convection_real4.csv "$OUT"/flexpart_convection_real8.csv; do
  echo "$(basename "$f"): $(($(grep -c '' "$f") - 4)) rows, $(wc -c < "$f") bytes" >&2
done
