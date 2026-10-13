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
# Seeds per case are sized so three standard errors sit well inside 200 pcm
# (seed-to-seed sd of one run, from the campaign: Godiva 183, Jemima 152,
# HST-009 181, lattice 74 pcm). Run sizes are the campaign's own.
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

seeds_for() {
  case "$1" in
    godiva) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 ;;
    jemima) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24 ;;
    hst009) echo 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 ;;
    lct008) echo 1,2,3,4,5,6 ;;
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
    --csv "$per_seed" --commit "$commit" \
    --accept "$summary" --allowance-pcm 200 --accept-record "$record" || status=1
done

echo
echo "== acceptance record: $record =="
cat "$record"
exit $status
