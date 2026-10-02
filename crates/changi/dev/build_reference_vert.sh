#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage `vert` (vertical transformation): build dev/flexpart_reference_vert.f90
# at both precisions and write tests/data/flexpart_vert_real{4,8}.csv.
# verttransform_ecmwf, verttransform_gfs, verttransform_nests, shift_field,
# shift_field_0 and what they call are compiled verbatim. One local file
# stands in for an upstream one: a par_mod copy with the nest line changed to
# maxnests=2, nxmaxn=12, nymaxn=12 (checked to be the only difference).
# The driver runs once per process set (p1, p2, shift) because the routines
# keep SAVEd state; the three outputs are concatenated.
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
VERT_FILES=(
  "$SRC/cmapf_mod.f90" "$SRC/ew.f90" "$SRC/qvsat.f90"
  "$SRC/verttransform_ecmwf.f90" "$SRC/verttransform_gfs.f90"
  "$SRC/verttransform_nests.f90" "$SRC/shift_field.f90" "$SRC/shift_field_0.f90"
)
for tag in real4 real8; do
  dir="$BUILD/vert_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  sed 's/maxnests=0,nxmaxn=0,nymaxn=0/maxnests=2,nxmaxn=12,nymaxn=12/' \
    "$SRC/par_mod.f90" > "$dir/par_mod.f90"
  [ "$(diff "$SRC/par_mod.f90" "$dir/par_mod.f90" | grep -c '^[<>]')" = 2 ] \
    || { echo "vert par_mod substitution failed" >&2; exit 1; }
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$dir/par_mod.f90" "$SRC/com_mod.f90" \
    "${VERT_FILES[@]}" "$CRATE_DIR/dev/flexpart_reference_vert.f90" -o "$dir/vert"
  out="$OUT/flexpart_vert_$tag.csv"
  : > "$out"
  for set in p1 p2 shift; do
    "$dir/vert" "$set" "$dir/$set.csv" > /dev/null
    cat "$dir/$set.csv" >> "$out"
  done
  echo "flexpart_vert_$tag.csv: $(grep -c '' "$out") lines, $(wc -c < "$out") bytes" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
