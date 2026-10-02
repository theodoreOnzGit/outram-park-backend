#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage 2 (meteorological interpolation): build dev/flexpart_reference_interp.f90
# at both precisions and write tests/data/flexpart_interp_real{4,8}.csv.
# The ten interpol_* routines are compiled verbatim; par_mod is the
# nest-configured copy (see build_stage_common.sh).
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
for tag in real4 real8; do
  dir="$BUILD/interp_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  nest_par_mod "$dir"
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$dir/par_mod.f90" "$SRC/com_mod.f90" \
    "${INTERP_FILES[@]}" "$CRATE_DIR/dev/flexpart_reference_interp.f90" -o "$dir/interp"
  "$dir/interp" > "$OUT/flexpart_interp_$tag.csv"
  echo "flexpart_interp_$tag.csv: $(grep -c '' "$OUT/flexpart_interp_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
