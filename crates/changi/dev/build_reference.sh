#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the FLEXPART code-to-code reference driver at BOTH precisions,
# writing two committed fixtures under tests/data/.
#
#   tests/data/flexpart_reference_real4.csv   as FLEXPART actually ships
#   tests/data/flexpart_reference_real8.csv   same algorithm, -fdefault-real-8
#   tests/data/flexpart_physics_real4.csv     stage-1 physics, as shipped
#   tests/data/flexpart_physics_real8.csv     stage-1 physics, real(8)
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

# ---------------------------------------------------------------------------
# Stage-1 physics driver (dev/flexpart_reference_physics.f90): turbulence, CBL,
# dry deposition, PBL diagnostics, solar geometry, calendar, geodesy.
#
# These routines `use com_mod` / `hanna_mod`, so the modules are compiled too
# (verbatim) and the driver writes synthetic fields into them. Two extra flags:
#   -mcmodel=medium      com_mod's static arrays exceed 2 GB at real(8);
#   -fdefault-double-8   keeps `double precision` (the Julian dates the driver
#                        passes) at 8 bytes under -fdefault-real-8, which would
#                        otherwise promote it to real(16) and break the
#                        interface with upstream's real(kind=dp).
PHYS_FILES=(
  "$SRC/par_mod.f90" "$SRC/com_mod.f90" "$SRC/hanna_mod.f90"
  "$SRC/ew.f90" "$SRC/dynamic_viscosity.f90" "$SRC/psim.f90" "$SRC/psih.f90"
  "$SRC/raerod.f90" "$SRC/hanna.f90" "$SRC/hanna1.f90" "$SRC/hanna_short.f90"
  "$SRC/cbl.f90" "$SRC/getrb.f90" "$SRC/getrc.f90" "$SRC/partdep.f90"
  "$SRC/caldate.f90" "$SRC/juldate.f90" "$SRC/getvdep.f90"
  "$SRC/get_settling.f90" "$SRC/pbl_profile.f90" "$SRC/qvsat.f90"
  "$SRC/richardson.f90" "$SRC/windalign.f90" "$SRC/zenithangle.f90"
  "$SRC/photo_O1D.f90" "$SRC/distance.f90" "$SRC/distance2.f90"
)

build_and_run_physics () {
  local tag="$1"; shift
  local extra=("$@")
  local dir="$BUILD/physics_$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  gfortran -O2 -mcmodel=medium -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" \
    "${PHYS_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_physics.f90" \
    -o "$dir/flexpart_reference_physics"
  "$dir/flexpart_reference_physics"
}

echo "building real(4) physics reference..." >&2
build_and_run_physics real4 > "$OUT/flexpart_physics_real4.csv"
echo "building real(8) physics reference..." >&2
build_and_run_physics real8 -fdefault-real-8 -fdefault-double-8 \
  > "$OUT/flexpart_physics_real8.csv"

for f in "$OUT"/flexpart_physics_real4.csv "$OUT"/flexpart_physics_real8.csv; do
  echo "$(basename "$f"): $(($(grep -c '' "$f") - 4)) rows" >&2
done

# ---------------------------------------------------------------------------
# Later stages, each with its own script (see the script headers for what is
# compiled verbatim and what configuration copy or shim each needs).
for stage in interp met advance cmapf output convection stochastic stochastic_redist; do
  script="$CRATE_DIR/dev/build_reference_$stage.sh"
  if [ -f "$script" ]; then
    echo "stage $stage..." >&2
    bash "$script"
  fi
done
