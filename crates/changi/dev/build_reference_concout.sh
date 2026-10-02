#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Stage "concout": build dev/flexpart_reference_concout.f90 at both precisions
# and write tests/data/flexpart_concout_real{4,8}.csv.
#
# Compiled VERBATIM from upstream: par_mod (configuration copy, see below),
# com_mod, unc_mod, outg_mod, point_mod, mean_mod, caldate, concoutput,
# concoutput_nest, concoutput_surf. caldate.f90 (Numerical Recipes lineage) is
# only compiled here to name the output files, never ported.
#
# The timemanager.f90 bookkeeping is inline upstream; the driver carries
# byte-for-byte copies of the cited lines, and this script CHECKS that they
# still match upstream before building (blocks between `! >>> ... verbatim`
# and `! <<< end verbatim`, and every line tagged `!tm:<line>`).
#
# par_mod.f90 copies (FLEXPART's user configuration; checked diff counts):
#   V0  as shipped                     (real(8) build: + dep_prec=dp)
#   V1  maxspec=2, maxageclass=2, nclassunc=4         (real(8): + dep_prec=dp)
#   V2  as V1 + lparticlecountoutput=.true.           (real(8): + dep_prec=dp)
# dep_prec=dp is REQUIRED in the real(8) build: with -fdefault-real-8 and the
# shipped dep_prec=sp, concoutput*.f90 do not compile (no specific of the
# generic `mean` for a real(4) sample and real(8) results).
source "$(dirname "${BASH_SOURCE[0]}")/build_stage_common.sh"

DRIVER="$CRATE_DIR/dev/flexpart_reference_concout.f90"
TM="$SRC/timemanager.f90"

# --- the extracted timemanager lines must still be upstream's -------------
python3 - "$DRIVER" "$TM" <<'EOF'
import re, sys
drv = open(sys.argv[1]).read().split('\n')
up = open(sys.argv[2]).read().split('\n')
nblocks = 0
for i, line in enumerate(drv):
    m = re.match(r'! >>> timemanager\.f90:(\d+)-(\d+) verbatim$', line)
    if m:
        a, b = int(m.group(1)), int(m.group(2))
        j = drv.index('! <<< end verbatim', i)
        if drv[i+1:j] != up[a-1:b]:
            sys.exit(f'extracted block timemanager.f90:{a}-{b} differs from upstream')
        nblocks += 1
ntag = 0
for line in drv:
    m = re.match(r'^(.*) !tm:(\d+)$', line)
    if m:
        if m.group(1) != up[int(m.group(2))-1]:
            sys.exit(f'line tagged !tm:{m.group(2)} differs from upstream')
        ntag += 1
if nblocks != 4 or ntag != 37:
    sys.exit(f'expected 4 verbatim blocks and 37 tagged lines, found {nblocks}, {ntag}')
print(f'timemanager extracts verified: {nblocks} blocks, {ntag} tagged lines', file=sys.stderr)
EOF

# make_par_mod <variant> <tag> <dest>
make_par_mod () {
  local variant="$1" tag="$2" dest="$3" want=0
  cp "$SRC/par_mod.f90" "$dest"
  if [ "$variant" != V0 ]; then
    sed -i -e 's/maxageclass=1,nclassunc=1$/maxageclass=2,nclassunc=4/' \
           -e 's/:: maxspec=1$/:: maxspec=2/' "$dest"
    grep -q 'maxageclass=2,nclassunc=4' "$dest"
    grep -q ':: maxspec=2$' "$dest"
    want=$((want + 2))
  fi
  if [ "$variant" = V2 ]; then
    sed -i -e 's/lparticlecountoutput=\.false\./lparticlecountoutput=.true./' "$dest"
    grep -q 'lparticlecountoutput=.true.' "$dest"
    want=$((want + 1))
  fi
  if [ "$tag" = real8 ]; then
    sed -i -e 's/:: dep_prec=sp$/:: dep_prec=dp/' "$dest"
    grep -q ':: dep_prec=dp$' "$dest"
    want=$((want + 1))
  fi
  [ "$(diff "$SRC/par_mod.f90" "$dest" | grep -c '^>')" -eq "$want" ] \
    || { echo "par_mod $variant/$tag: unexpected diff" >&2; exit 1; }
}

FILES=(
  "$SRC/com_mod.f90" "$SRC/unc_mod.f90" "$SRC/outg_mod.f90" "$SRC/point_mod.f90"
  "$SRC/mean_mod.f90" "$SRC/caldate.f90"
  "$SRC/concoutput.f90" "$SRC/concoutput_nest.f90" "$SRC/concoutput_surf.f90"
)

for tag in real4 real8; do
  extra=(); [ "$tag" = real8 ] && extra=("${REAL8[@]}")
  {
    for variant in V0 V1 V2; do
      dir="$BUILD/concout_${variant}_$tag"; rm -rf "$dir"; mkdir -p "$dir"
      make_par_mod "$variant" "$tag" "$dir/par_mod.f90"
      # -cpp: unc_mod.f90 carries #ifdef blocks (upstream's makefile passes -cpp).
      gfortran "${FFLAGS[@]}" -cpp "${extra[@]}" -J"$dir" -I"$dir" \
        "$dir/par_mod.f90" "${FILES[@]}" "$DRIVER" -o "$dir/concout"
      # Run in the build dir: the routines write their files to path(2) = './'.
      if [ "$variant" = V0 ]; then
        (cd "$dir" && ./concout)
      else
        (cd "$dir" && ./concout) | grep -v '^#\|^function,'
      fi
      rm -rf "$dir"
    done
  } > "$OUT/flexpart_concout_$tag.csv"
  echo "flexpart_concout_$tag.csv: $(grep -c '' "$OUT/flexpart_concout_$tag.csv") lines" >&2
done
[ -n "${FLEXPART_STAGE_BUILD:-}" ] || rm -rf "$BUILD"
