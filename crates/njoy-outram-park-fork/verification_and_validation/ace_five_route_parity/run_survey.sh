#!/bin/sh
# Every five-route nuclide, ours against NJOY2016, word for word (GitHub #872).
#
#   NJOY=<path to NJOY2016 njoy> WORK=<scratch dir> \
#     crates/njoy-outram-park-fork/verification_and_validation/ace_five_route_parity/run_survey.sh
#
# 1. Builds the NJOY2016 library with the five-route campaign's own script
#    (RECONR -> BROADR -> PURR -> ACER at 293.6 K, RECONR -> ACER at 0 K; the
#    uranium 293.6 K tables and U-235 0 K come from the reference-data/ace
#    submodule). Its last step (H in H2O through OpenMC's Python driver) is
#    not needed here and may fail without OpenMC; the CE tables are built
#    before it.
# 2. Runs examples/ace_word_parity for each nuclide and temperature: ours
#    from process::cross_sections (no HEATR, as the NJOY deck), compared by
#    acer::compare with no tolerance. One line per table.
set -u
REPO=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
WORK=${WORK:?set WORK to a scratch directory}
NJOY=${NJOY:?set NJOY to the NJOY2016 executable}
cd "$REPO"
WORK=$WORK NJOY=$NJOY sh crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/scripts/make_njoy_library.sh \
  > "$WORK/njoy_library.log" 2>&1 || echo "(make_njoy_library.sh ended non-zero; see $WORK/njoy_library.log)"
cargo build --release -p njoy-outram-park-fork --example ace_word_parity >/dev/null 2>&1 || { echo "build failed"; exit 1; }
L=$WORK/njoy
grep -v '^#' crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/nuclides.tsv |
while IFS="$(printf '\t')" read -r name tape mat; do
  for T in 293.6 0; do
    d=$([ "$T" = 0 ] && echo 0K || echo 293.6K)
    [ -s "$L/$d/$name.ace" ] || { echo "$name $T K: no NJOY2016 table"; continue; }
    out=$(target/release/examples/ace_word_parity "$tape" "$T" "$L/$d/$name.ace" 2>&1); rc=$?
    echo "$name $T K exit $rc: $(echo "$out" | grep VERDICT | sed 's/.*VERDICT: //')"
    [ $rc = 0 ] || echo "$out" | grep -E "DIFFERENT|MISSING|ERROR" | grep -v '^ *JXS' | head -5
  done
done
