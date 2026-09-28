#!/bin/sh
# NJOY2016 ACE library for the five-route ICSBEP study (routes 1 and 3).
#
# Produces, under $WORK/njoy/:
#   293.6K/<name>.ace   RECONR -> BROADR(293.6 K) -> PURR(20 bins, 64 ladders) -> ACER
#   0K/<name>.ace       RECONR -> ACER, the unbroadened companion DBRC needs
#   293.6K/HinH2O.ace   H in H2O S(a,b): RECONR -> BROADR -> THERMR -> ACER (iwt = 1)
#
# REUSE, NOT A NEW DECK. The continuous-energy decks are the two already
# committed beside the Godiva cross-code study
# (`../../openmc_godiva_cross_code/make_ace.sh`, `make_ace_0k.sh`), which are
# also the decks the `reference-data/ace` submodule was built with. The three
# uranium 293.6 K tables and the U-235 0 K table are taken FROM that submodule
# rather than regenerated: same NJOY2016 build (2016.79, ac5adf5), same tapes,
# same deck, so regenerating them would only spend time.
#
# The thermal table goes through OpenMC's own NJOY driver
# (`openmc.data.njoy.make_ace_thermal`), i.e. upstream's deck, with ONE change
# from its default: iwt = 1 (the equiprobable IFENG = 0 form) instead of
# iwt = 2 (continuous). outram-mc's thermal ACE reader refuses IFENG = 2 by
# design (see njoy-outram-park-fork/verification_and_validation/thermal_from_ace/),
# and routes 1 and 3 must read the SAME table, so the form both can read is
# used for both. OpenMC reads IFENG = 0 natively.
#
#   WORK=... NJOY=... ./make_njoy_library.sh
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(git -C "$HERE" rev-parse --show-toplevel)
WORK=${WORK:-$REPO/target/five_route_keff}
NJOY=${NJOY:-$HOME/Documents/research/NJOY2016/build/njoy}
PY=${PY:-$HOME/Documents/research/.venv-openmc/bin/python}
JOBS=${JOBS:-8}
export ENDF_DIR=$REPO/reference-data/endf
export NJOY
DECKS=$REPO/crates/outram-mc-libs/verification_and_validation/openmc_godiva_cross_code
SUB=$REPO/reference-data/ace/reference-njoy/endf-b-viii.0
LIST=$HERE/../nuclides.tsv
OUT=$WORK/njoy
mkdir -p "$OUT/293.6K" "$OUT/0K" "$OUT/build293" "$OUT/build0K"

# ── from the submodule ──────────────────────────────────────────────────────
for n in U234 U235 U238; do
  [ -s "$OUT/293.6K/$n.ace" ] || gunzip -c "$SUB/293.6K/$n.ace.gz" > "$OUT/293.6K/$n.ace"
done
[ -s "$OUT/0K/U235.ace" ] || gunzip -c "$SUB/0K/U235.ace.gz" > "$OUT/0K/U235.ace"

# ── generated here, JOBS at a time ─────────────────────────────────────────
grep -v '^#' "$LIST" | while IFS="$(printf '\t')" read -r name tape mat; do
  case $name in U234|U235|U238) ;; *)
    [ -s "$OUT/293.6K/$name.ace" ] || echo "293 $mat $tape $name" ;;
  esac
  [ "$name" = U235 ] || [ -s "$OUT/0K/$name.ace" ] || echo "0 $mat $tape $name"
done | xargs -r -P "$JOBS" -L 1 sh -c '
  kind=$0; mat=$1; tape=$2; name=$3
  if [ "$kind" = 293 ]; then
    cd "'"$OUT"'/build293" && sh "'"$DECKS"'/make_ace.sh" "$mat" "$tape" "$name" >/dev/null
    cp "'"$OUT"'/build293/$name/tape24" "'"$OUT"'/293.6K/$name.ace"
  else
    cd "'"$OUT"'/build0K" && sh "'"$DECKS"'/make_ace_0k.sh" "$mat" "$tape" "$name" >/dev/null
    cp "'"$OUT"'/build0K/$name/tape24" "'"$OUT"'/0K/$name.ace"
  fi
  echo "  NJOY2016 $kind K  $name  $(wc -c < "'"$OUT"'/$([ "$kind" = 293 ] && echo 293.6K || echo 0K)/$name.ace") bytes"
'

# ── NJOY2016's Type-1 formatter aborts on B-10; rerun those as Type 2 ─────
# NJOY2016 2016.79 stops in `change` (acefc.f90:13942, "Undefined law for dlwh
# block: 0") while formatting ENDF/B-VIII.0 B-10's charged-particle block, and
# leaves a TRUNCATED Type-1 file. The Type-2 branch never calls `change`, so the
# identical deck with itype = 2 (ACER card 2) completes. Detected from NJOY's
# own stdout rather than assumed per nuclide. Both readers downstream take the
# binary: outram-mc's `acer::read::read` sniffs Type 2, and
# openmc_inputs/ace_to_hdf5_route.py reads NJOY's record layout itself.
for kind in 293 0; do
  [ $kind = 293 ] && { b=build293; d=293.6K; } || { b=build0K; d=0K; }
  for dir in "$OUT/$b"/*/; do
    name=$(basename "$dir")
    grep -q '\*\*\*error in change' "$dir/njoy.stdout" 2>/dev/null || continue
    t2="$OUT/${b}_type2/$name"; mkdir -p "$t2"
    if [ ! -s "$t2/tape24" ]; then
      cp "$dir/tape20" "$t2/tape20"
      sed 's|^1 1 1 .00 0/|1 1 2 .00 0/|' "$dir/input" > "$t2/input"
      (cd "$t2" && "$NJOY" < input > njoy.stdout 2>&1)
      ! grep -q '\*\*\*error' "$t2/njoy.stdout" || { echo "NJOY Type-2 rerun of $name FAILED"; exit 1; }
    fi
    cp "$t2/tape24" "$OUT/$d/$name.ace"
    echo "  NJOY2016 $kind K  $name  Type 2 (Type-1 formatter aborted)  $(wc -c < "$OUT/$d/$name.ace") bytes"
  done
done

# ── H in H2O, through OpenMC's NJOY driver, iwt = 1 ────────────────────────
if [ ! -s "$OUT/293.6K/HinH2O.ace" ]; then
  mkdir -p "$OUT/build_thermal"
  "$PY" - "$ENDF_DIR" "$OUT/build_thermal" "$NJOY" <<'PYEOF'
import sys, pathlib
import openmc.data.njoy as nj
endf, out, njoy = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3]
nj.make_ace_thermal(f"{endf}/n-001_H_001-ENDF8.0-Beta6.endf", f"{endf}/tsl-HinH2O.endf",
                    temperatures=[293.6], output_dir=out, iwt=1, table_name="lwtr",
                    njoy_exec=njoy, stdout=False)
PYEOF
  cp "$OUT/build_thermal/ace" "$OUT/293.6K/HinH2O.ace"
fi
echo "NJOY2016 library complete under $OUT"
ls -la "$OUT/293.6K" "$OUT/0K"
