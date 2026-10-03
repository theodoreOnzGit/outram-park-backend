#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the stage "output" FLEXPART reference driver
# (dev/flexpart_reference_output.f90) at BOTH precisions, writing
#
#   tests/data/flexpart_output_real4.csv   as FLEXPART ships (default real(4))
#   tests/data/flexpart_output_real8.csv   -fdefault-real-8 -fdefault-double-8
#
# Routines under test (compiled VERBATIM from upstream): conccalc,
# drydepokernel, drydepokernel_nest, centerofmass, clustering, plumetraj,
# mean_mod, partpos_average (+ distance, distance2 and the modules).
#
# par_mod.f90 VARIANTS. par_mod.f90 is FLEXPART's user-edited configuration
# file. Its compile-time parameters lusekerneloutput, lparticlecountoutput,
# maxspec, maxageclass and nclassunc gate branches of the routines under test,
# so each precision is built three times and the outputs concatenated:
#   V0  par_mod.f90 verbatim (as shipped)
#   V1  copy with maxspec=2, maxageclass=3, nclassunc=2
#   V2  as V1, plus lusekerneloutput=.false., lparticlecountoutput=.true.
#   V3  as V1, plus lparticlecountoutput=.true. only (count mode, kernel on)
# Only those parameter lines are changed (sed, checked below); every other
# upstream file is compiled unmodified.
#
# Build products go to $FLEXPART_BUILD (default: a mktemp dir), never the repo.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
OUT="$CRATE_DIR/tests/data"
BUILD="${FLEXPART_BUILD:-$(mktemp -d)}"

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }

OUTPUT_FILES=(
  "$SRC/com_mod.f90" "$SRC/unc_mod.f90" "$SRC/outg_mod.f90" "$SRC/point_mod.f90"
  "$SRC/mean_mod.f90" "$SRC/distance.f90" "$SRC/distance2.f90"
  "$SRC/conccalc.f90" "$SRC/drydepokernel.f90" "$SRC/drydepokernel_nest.f90"
  "$SRC/centerofmass.f90" "$SRC/clustering.f90" "$SRC/plumetraj.f90"
  "$SRC/partpos_average.f90"
)

# make_par_mod <variant> <dest>: verbatim copy (V0) or the parameter-edited copy.
make_par_mod () {
  local variant="$1" dest="$2"
  case "$variant" in
    V0) cp "$SRC/par_mod.f90" "$dest" ;;
    V1|V2|V3)
      sed -e 's/maxageclass=1,nclassunc=1$/maxageclass=3,nclassunc=2/' \
          -e 's/:: maxspec=1$/:: maxspec=2/' "$SRC/par_mod.f90" > "$dest"
      if [ "$variant" = V2 ]; then
        sed -i -e 's/lusekerneloutput=\.true\./lusekerneloutput=.false./' "$dest"
      fi
      if [ "$variant" = V2 ] || [ "$variant" = V3 ]; then
        sed -i -e 's/lparticlecountoutput=\.false\./lparticlecountoutput=.true./' "$dest"
      fi
      # Fail loudly if upstream's lines moved and a substitution missed.
      grep -q 'maxageclass=3,nclassunc=2' "$dest"
      grep -q ':: maxspec=2' "$dest"
      case "$variant" in
        V1) want=2 ;;
        V2) want=4; grep -q 'lusekerneloutput=.false.' "$dest"; grep -q 'lparticlecountoutput=.true.' "$dest" ;;
        V3) want=3; grep -q 'lparticlecountoutput=.true.' "$dest" ;;
      esac
      [ "$(diff "$SRC/par_mod.f90" "$dest" | grep -c '^>')" -eq "$want" ]
      ;;
  esac
}

build_and_run_output () {
  local variant="$1" tag="$2"; shift 2
  local extra=("$@")
  local dir="$BUILD/output_${variant}_$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  make_par_mod "$variant" "$dir/par_mod.f90"
  # -cpp: unc_mod.f90 carries #ifdef blocks (upstream's makefile passes -cpp).
  gfortran -O2 -cpp -mcmodel=medium -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "$dir/par_mod.f90" "${OUTPUT_FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_output.f90" \
    -o "$dir/flexpart_reference_output"
  (cd "$dir" && ./flexpart_reference_output)
  rm -rf "$dir"
}

mkdir -p "$OUT"
for tag in real4 real8; do
  if [ "$tag" = real4 ]; then flags=(); else flags=(-fdefault-real-8 -fdefault-double-8); fi
  echo "building $tag output reference (V0, V1, V2, V3)..." >&2
  {
    build_and_run_output V0 "$tag" "${flags[@]}"
    build_and_run_output V1 "$tag" "${flags[@]}" | grep -v '^#\|^function,'
    build_and_run_output V2 "$tag" "${flags[@]}" | grep -v '^#\|^function,'
    build_and_run_output V3 "$tag" "${flags[@]}" | grep -v '^#\|^function,'
  } > "$OUT/flexpart_output_$tag.csv"
  echo "flexpart_output_$tag.csv: $(grep -vc '^#\|^function,' "$OUT/flexpart_output_$tag.csv") rows" >&2
done
