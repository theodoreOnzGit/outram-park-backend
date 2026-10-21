#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the FLEXPART stage-`met` code-to-code reference driver
# (dev/flexpart_reference_met.f90) at BOTH precisions, writing
#
#   tests/data/flexpart_met_real4.csv   as FLEXPART ships (default real = real(4))
#   tests/data/flexpart_met_real8.csv   -fdefault-real-8 -fdefault-double-8
#
# Routines under test, all upstream FLEXPART v10.4 @ 3d7eebf, compiled
# VERBATIM: calcpar, calcpar_nests, calcpv, calcpv_nests, interpol_rain,
# interpol_rain_nests, get_wetscav, wetdepo, wetdepokernel,
# wetdepokernel_nest, ohreaction, gethourlyOH, assignland.
#
# NEST CONFIGURATION. The shipped par_mod.f90 has maxnests=0, nxmaxn=0,
# nymaxn=0 (zero-sized nest arrays). par_mod.f90 is FLEXPART's user-edited
# configuration file, so a COPY is compiled from the build directory with only
# that line changed to maxnests=1, nxmaxn=12, nymaxn=12 (checked below).
#
# Build products go to a scratch directory (FLEXPART_MET_BUILD, default under
# $TMPDIR), never into the repository.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
BUILD="${FLEXPART_MET_BUILD:-${TMPDIR:-/tmp}/flexpart_met_build}"
OUT="$CRATE_DIR/tests/data"

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }

MET_FILES=(
  "$SRC/com_mod.f90" "$SRC/point_mod.f90" "$SRC/unc_mod.f90" "$SRC/oh_mod.f90"
  "$SRC/ew.f90" "$SRC/psim.f90" "$SRC/psih.f90" "$SRC/scalev.f90" "$SRC/obukhov.f90"
  "$SRC/raerod.f90" "$SRC/qvsat.f90" "$SRC/richardson.f90" "$SRC/caldate.f90"
  "$SRC/getrb.f90" "$SRC/getrc.f90" "$SRC/partdep.f90"
  "$SRC/getvdep.f90" "$SRC/getvdep_nests.f90"
  "$SRC/calcpv.f90" "$SRC/calcpv_nests.f90" "$SRC/calcpar.f90" "$SRC/calcpar_nests.f90"
  "$SRC/interpol_rain.f90" "$SRC/interpol_rain_nests.f90" "$SRC/get_wetscav.f90"
  "$SRC/wetdepokernel.f90" "$SRC/wetdepokernel_nest.f90" "$SRC/wetdepo.f90"
  "$SRC/zenithangle.f90" "$SRC/photo_O1D.f90" "$SRC/gethourlyOH.f90" "$SRC/ohreaction.f90"
  "$SRC/assignland.f90"
)

build_and_run_met () {
  local tag="$1"; shift
  local extra=("$@")
  local dir="$BUILD/met_$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  sed 's/maxnests=0,nxmaxn=0,nymaxn=0/maxnests=1,nxmaxn=12,nymaxn=12/' \
    "$SRC/par_mod.f90" > "$dir/par_mod.f90"
  grep -q 'maxnests=1,nxmaxn=12,nymaxn=12' "$dir/par_mod.f90" \
    || { echo "par_mod.f90 nest line not found; upstream changed?" >&2; exit 1; }
  # -mcmodel=medium: com_mod's static arrays exceed 2 GB at real(8).
  # -fdefault-double-8 (real8 only): keeps `double precision` at 8 bytes.
  gfortran -O2 -mcmodel=medium -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$dir/par_mod.f90" \
    "${MET_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_met.f90" \
    -o "$dir/flexpart_reference_met"
  "$dir/flexpart_reference_met"
}

mkdir -p "$OUT"
echo "building real(4) met reference (as FLEXPART ships)..." >&2
build_and_run_met real4 > "$OUT/flexpart_met_real4.csv"
echo "building real(8) met reference..." >&2
build_and_run_met real8 -fdefault-real-8 -fdefault-double-8 > "$OUT/flexpart_met_real8.csv"

for f in "$OUT"/flexpart_met_real4.csv "$OUT"/flexpart_met_real8.csv; do
  echo "$(basename "$f"): $(($(grep -c '' "$f") - 4)) rows, $(du -h "$f" | cut -f1)" >&2
done
rm -rf "$BUILD"
