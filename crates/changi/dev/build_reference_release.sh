#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage `release` (particle release and domain filling): build
# dev/flexpart_reference_release.f90 at both precisions and write
# tests/data/flexpart_release_real{4,8}.csv.
# releaseparticles, init_domainfill, boundcond_domainfill, juldate and caldate
# are compiled verbatim. Two local files stand in for upstream ones: a
# par_mod copy with three configuration lines changed (nests, maxspec=2,
# nclassunc=5; the diff is checked), and dev/random_mod_shim_release.f90 in
# place of random_mod.f90 (Numerical Recipes, never ported, gh:#410), which
# returns the draws the driver sets. init_domainfill's list-directed progress
# lines (they start with a blank) are dropped from the fixture.
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"
for tag in real4 real8; do
  dir="$BUILD/release_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  nest_par_mod "$dir"
  sed -i -e 's/maxspec=1$/maxspec=2/' \
    -e 's/maxageclass=1,nclassunc=1$/maxageclass=1,nclassunc=5/' "$dir/par_mod.f90"
  [ "$(diff "$SRC/par_mod.f90" "$dir/par_mod.f90" | grep -c '^[<>]')" = 6 ] \
    || { echo "release par_mod substitution failed" >&2; exit 1; }
  gfortran "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$CRATE_DIR/dev/class_gribfile_shim.f90" "$dir/par_mod.f90" "$SRC/com_mod.f90" \
    "$SRC/point_mod.f90" "$SRC/xmass_mod.f90" "$CRATE_DIR/dev/random_mod_shim_release.f90" \
    "$SRC/juldate.f90" "$SRC/caldate.f90" "$SRC/releaseparticles.f90" \
    "$SRC/init_domainfill.f90" "$SRC/boundcond_domainfill.f90" \
    "$CRATE_DIR/dev/flexpart_reference_release.f90" -o "$dir/release"
  "$dir/release" | grep -v '^ ' > "$OUT/flexpart_release_$tag.csv"
  echo "flexpart_release_$tag.csv: $(grep -c '' "$OUT/flexpart_release_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
