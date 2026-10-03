#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage 7 (stochastic comparison): build dev/flexpart_stochastic_advance.f90 at
# both precisions and write tests/data/flexpart_stochastic_real{4,8}.csv.
# Everything is upstream verbatim, INCLUDING random_mod.f90: FLEXPART draws
# from its own generators here, compiled as a reference only (never ported or
# re-implemented, gh:#410). The shipped par_mod is used (no nests).
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
for tag in real4 real8; do
  dir="$BUILD/stochastic_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$SRC/par_mod.f90" "$SRC/com_mod.f90" \
    "$SRC/point_mod.f90" "$SRC/random_mod.f90" \
    "${STEP_FILES[@]}" "$CRATE_DIR/dev/flexpart_stochastic_advance.f90" -o "$dir/stochastic"
  "$dir/stochastic" > "$OUT/flexpart_stochastic_$tag.csv"
  echo "flexpart_stochastic_$tag.csv: $(grep -c '' "$OUT/flexpart_stochastic_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
