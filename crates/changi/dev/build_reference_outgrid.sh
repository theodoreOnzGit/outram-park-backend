#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage "outgrid" (output-grid set-up, gross fluxes, initial-condition
# sensitivity): build dev/flexpart_reference_outgrid.f90 at both precisions
# and write tests/data/flexpart_outgrid_real{4,8}.csv.
#
# outgrid_init, outgrid_init_nest, calcfluxes, fluxoutput, initial_cond_calc
# and caldate (called by fluxoutput) are compiled VERBATIM, with the modules
# they use. One local file stands in for an upstream one: a par_mod.f90 copy
# (FLEXPART's user configuration) with exactly three parameter lines changed,
#   maxnests=2,nxmaxn=12,nymaxn=12   maxspec=2   maxageclass=2
# checked below. fluxoutput writes its file into the scratch build directory;
# the driver reads it back and deletes it.
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"

outgrid_par_mod () {
  local dir="$1"
  sed -e 's/maxnests=0,nxmaxn=0,nymaxn=0/maxnests=2,nxmaxn=12,nymaxn=12/' \
      -e 's/:: maxspec=1$/:: maxspec=2/' \
      -e 's/maxageclass=1,nclassunc=1$/maxageclass=2,nclassunc=1/' \
      "$SRC/par_mod.f90" > "$dir/par_mod.f90"
  grep -q 'maxnests=2,nxmaxn=12,nymaxn=12' "$dir/par_mod.f90"
  grep -q ':: maxspec=2' "$dir/par_mod.f90"
  grep -q 'maxageclass=2,nclassunc=1' "$dir/par_mod.f90"
  [ "$(diff "$SRC/par_mod.f90" "$dir/par_mod.f90" | grep -c '^>')" = 3 ] \
    || { echo "outgrid par_mod substitution failed" >&2; exit 1; }
}

OUTGRID_FILES=(
  "$SRC/com_mod.f90" "$SRC/unc_mod.f90" "$SRC/outg_mod.f90" "$SRC/flux_mod.f90"
  "$SRC/oh_mod.f90" "$SRC/caldate.f90"
  "$SRC/outgrid_init.f90" "$SRC/outgrid_init_nest.f90"
  "$SRC/calcfluxes.f90" "$SRC/fluxoutput.f90" "$SRC/initial_cond_calc.f90"
)

for tag in real4 real8; do
  dir="$BUILD/outgrid_$tag"; rm -rf "$dir"; mkdir -p "$dir"
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  outgrid_par_mod "$dir"
  # -cpp: unc_mod.f90 and outgrid_init*.f90 carry #ifdef blocks (upstream's
  # makefile passes -cpp).
  gfortran -cpp "${FFLAGS[@]}" "${extra[@]}" -J"$dir" -I"$dir" \
    "$dir/par_mod.f90" "${OUTGRID_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_outgrid.f90" -o "$dir/outgrid"
  (cd "$dir" && ./outgrid > stdout.log)
  if grep -q 'ERROR' "$dir/stdout.log"; then
    cat "$dir/stdout.log" >&2
    echo "upstream reported an error ($tag)" >&2
    exit 1
  fi
  cp "$dir/outgrid.csv" "$OUT/flexpart_outgrid_$tag.csv"
  echo "flexpart_outgrid_$tag.csv: $(grep -vc '^#\|^function,' "$OUT/flexpart_outgrid_$tag.csv") rows" >&2
  rm -rf "$dir"
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
