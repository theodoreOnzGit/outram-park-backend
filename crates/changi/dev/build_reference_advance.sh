#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage 4 (the particle step): build dev/flexpart_reference_advance.f90 at both
# precisions and write tests/data/flexpart_advance_real{4,8}.csv.
# advance, initialize, initialize_cbl_vel, re_initialize_particle,
# get_vdep_prob and everything they call are compiled verbatim. Two local
# files stand in for upstream ones: the nest-configured par_mod copy, and
# dev/random_mod_shim_advance.f90 in place of random_mod.f90 (Numerical
# Recipes, never ported, gh:#410), which returns the draws the driver sets.
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
for tag in real4 real8; do
  dir="$BUILD/advance_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  nest_par_mod "$dir"
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$dir/par_mod.f90" "$SRC/com_mod.f90" \
    "$SRC/point_mod.f90" "$CRATE_DIR/dev/random_mod_shim_advance.f90" \
    "${STEP_FILES[@]}" "$CRATE_DIR/dev/flexpart_reference_advance.f90" -o "$dir/advance"
  "$dir/advance" > "$OUT/flexpart_advance_$tag.csv"
  echo "flexpart_advance_$tag.csv: $(grep -c '' "$OUT/flexpart_advance_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
