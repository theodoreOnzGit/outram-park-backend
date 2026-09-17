#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the FLEXPART code-to-code reference driver at BOTH precisions,
# writing two committed fixtures under tests/data/.
#
#   tests/data/flexpart_reference_real4.csv   as FLEXPART actually ships
#   tests/data/flexpart_reference_real8.csv   same algorithm, -fdefault-real-8
#
# WHY TWO: FLEXPART's own makefile passes no -fdefault-real-8, so its default
# `real` is single precision (pi stores as 3.14159274). A f64 Rust port diverges
# from that by ~1e-7 no matter how correct the translation. Matching the real8
# build to near machine precision is what proves the TRANSLATION; the real4
# build then measures upstream's own precision rather than confusing the two.
#
# Requires gfortran. The upstream clone is reference-only and gitignored:
#   git clone https://github.com/flexpart/flexpart.git \
#       crates/changi/upstream_source/FLEXPART
#   git -C crates/changi/upstream_source/FLEXPART checkout 3d7eebf
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
BUILD="$CRATE_DIR/dev/build"
OUT="$CRATE_DIR/tests/data"

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }

# Upstream files compiled VERBATIM -- none is edited. The only local file is the
# class_gribfile shim, which supplies the single integer parameter obukhov.f90
# needs without dragging in ecCodes (see its header).
UPSTREAM_FILES=(
  "$SRC/par_mod.f90"
  "$SRC/ew.f90"
  "$SRC/dynamic_viscosity.f90"
  "$SRC/psim.f90"
  "$SRC/psih.f90"
  "$SRC/scalev.f90"
  "$SRC/raerod.f90"
  "$SRC/obukhov.f90"
  "$SRC/erf.f90"
  "$SRC/part0.f90"
)

build_and_run () {
  local tag="$1"; shift
  local extra=("$@")
  local dir="$BUILD/$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  # -std=legacy: FLEXPART is Fortran 90 with older idioms gfortran 13 warns on.
  # -fallow-argument-mismatch: obukhov passes scalars where arrays are declared
  #   in some call paths; upstream builds with the same tolerance.
  gfortran -O2 -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" \
    "${UPSTREAM_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference.f90" \
    -o "$dir/flexpart_reference"
  "$dir/flexpart_reference"
}

mkdir -p "$OUT"
echo "building real(4) reference (as FLEXPART ships)..." >&2
build_and_run real4 > "$OUT/flexpart_reference_real4.csv"
echo "building real(8) reference (-fdefault-real-8)..." >&2
build_and_run real8 -fdefault-real-8 > "$OUT/flexpart_reference_real8.csv"

for f in "$OUT"/flexpart_reference_real4.csv "$OUT"/flexpart_reference_real8.csv; do
  echo "$(basename "$f"): $(($(grep -c '' "$f") - 4)) cases" >&2
done
