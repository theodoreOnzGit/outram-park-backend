#!/usr/bin/env bash
# Acceptance check for the NJOY port and Outram MC (GitHub #859): the four
# ICSBEP cases of the five-route study, at REDUCED statistics, each judged
# against the recorded OpenMC + NJOY2016 k of the same case with a 200 pcm
# allowance. Runs the command-line path a user runs: ENDF/B-VIII.0 tapes from
# reference-data/endf/ through the Rust NJOY port into Outram MC.
#
#   scripts/acceptance-keff.sh [record.csv] [case ...]
#
# The record (default target/acceptance/keff_acceptance.csv) gets one row per
# case. Exit status is non-zero if any case fails or cannot resolve the
# allowance. The check itself and its reasoning are in
# crates/outram-mc-libs/examples/icsbep_five_route_keff.rs ("--accept"); the
# results by environment are in
# crates/outram-mc-libs/verification_and_validation/acceptance_testing/.
#
# Every case runs at one size, 5000 histories x [40 inactive + 120 active].
# This is acceptance (does it run, and give a sane k, on this platform), not
# validation, so the lattice no longer runs the campaign's
# 10 000 x [250 + 400] (CHANGED 2026-10-13, maintainer direction): that size
# made it two-thirds of the transport time on its own.
#
# Every case runs 32 seeds (maintainer direction, 2026-10-15: "32 seeds
# across the board"), the five-route campaign's per-case count. Seed-to-seed
# sd of one run at this size: Godiva 183, Jemima 152, HST-009 181 pcm from the
# campaign, and the lattice ~190 pcm predicted (74 pcm at its campaign size,
# scaled by the square root of active histories). Three standard errors then
# take 81-101 pcm of the 200 pcm allowance. The floor is about 8 seeds
# (6 for Jemima): below it, three standard errors exceed 200 pcm and the run
# is UNRESOLVED. Before 2026-10-15 the counts were 16, 24, 16 and 16.
#
# LCT-008 here is a SMOKE TEST on the retired 11-nuclide tier
# (--cheap-nuclides; maintainer direction 2026-10-14). The lattice's model is
# the full nuclide list everywhere else; the 11-nuclide tier is kept to check
# that the code runs on each platform, judged against its own recorded
# route-1 k. It is not the lattice's k.
set -euo pipefail
cd "$(dirname "$0")/.."

record="${1:-target/acceptance/keff_acceptance.csv}"
shift || true
cases=("$@")
[ ${#cases[@]} -eq 0 ] && cases=(godiva jemima hst009 lct008)

threads="${ACCEPTANCE_THREADS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"
commit="$(git rev-parse --short=10 HEAD 2>/dev/null || echo unknown)"
summary=crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/data/summary_keff.csv
mkdir -p "$(dirname "$record")"
rm -f "$record"

# Extra flags per case (see the LCT-008 smoke-test note at the top).
extra_for() {
  case "$1" in
    lct008) echo --cheap-nuclides ;;
    *) echo ;;
  esac
}

seeds_for() {
  case "$1" in
    godiva) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32 ;;
    jemima) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32 ;;
    hst009) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32 ;;
    lct008) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32 ;;
    *) echo "unknown case $1" >&2; exit 2 ;;
  esac
}

cargo build --release -p outram-mc-libs --features endf-pebble-cases --example icsbep_five_route_keff

status=0
for c in "${cases[@]}"; do
  per_seed="$(dirname "$record")/per_seed_$c.csv"
  rm -f "$per_seed"
  cargo run --release -q -p outram-mc-libs --features endf-pebble-cases --example icsbep_five_route_keff -- \
    --case "$c" --route endf --label route4 --seeds "$(seeds_for "$c")" --threads "$threads" \
    --particles 5000 --inactive 40 --active 120 $(extra_for "$c") \
    --csv "$per_seed" --commit "$commit" \
    --accept "$summary" --allowance-pcm 200 --accept-record "$record" || status=1
done

echo
echo "== acceptance record: $record =="
cat "$record"
exit $status
