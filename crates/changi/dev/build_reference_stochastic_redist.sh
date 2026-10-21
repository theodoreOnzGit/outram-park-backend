#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage 7b (stochastic convective redistribution): build
# dev/flexpart_stochastic_redist.f90 at both precisions and write
# tests/data/flexpart_stochastic_redist_real{4,8}.csv. Everything is upstream
# verbatim INCLUDING random_mod.f90 (compiled as a reference only, never
# ported, gh:#410), on the shipped par_mod.
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
CONV_FILES=(
  "$SRC/com_mod.f90" "$SRC/conv_mod.f90" "$SRC/random_mod.f90"
  "$SRC/ew.f90" "$SRC/qvsat.f90" "$SRC/convect43c.f90" "$SRC/calcmatrix.f90" "$SRC/redist.f90"
)
for tag in real4 real8; do
  dir="$BUILD/stochastic_redist_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$SRC/par_mod.f90" "$CRATE_DIR/dev/class_gribfile_shim.f90" "${CONV_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_stochastic_redist.f90" -o "$dir/redist"
  "$dir/redist" > "$OUT/flexpart_stochastic_redist_$tag.csv"
  echo "flexpart_stochastic_redist_$tag.csv: $(grep -c '' "$OUT/flexpart_stochastic_redist_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
