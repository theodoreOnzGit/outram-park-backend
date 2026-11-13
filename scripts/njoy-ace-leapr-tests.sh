#!/usr/bin/env bash
# The NJOY port's ACE and LEAPR comparison tests, as CI runs them (gh #862).
#
# Runs every integration test file `tests/acer*.rs`, `tests/ace_*.rs` and
# `tests/leapr*.rs` of njoy-outram-park-fork, in release, LEAVING OUT by name
# the tests that skip (print SKIP and pass without checking anything) because
# their NJOY2016 tables or fixtures are not in the repository. Then it FAILS if
# any test that did run printed SKIP, so a test that starts skipping cannot pass
# unnoticed. The one exception is `missing_data_skips_cleanly`, whose purpose is
# to check skipping (its notes name a `/nonexistent/` folder).
#
# Needs the `reference-data/ace` submodule:
#   git submodule update --init --depth 1 reference-data/ace
#
# Usage: bash scripts/njoy-ace-leapr-tests.sh [log file]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
log="${1:-target/njoy-ace-leapr-tests.log}"
mkdir -p "$(dirname "$log")"

crate=crates/njoy-outram-park-fork
tests=()
for f in "$crate"/tests/acer*.rs "$crate"/tests/ace_*.rs "$crate"/tests/leapr*.rs; do
  name="$(basename "$f" .rs)"
  # Every test in this file needs OUTRAM_ACE_LAW_DIR (gh #862).
  [[ "$name" == ace_dlw_law_family_vs_njoy2016 ]] && continue
  tests+=(--test "$name")
done

# Tests that skip inside files that otherwise run (gh #862 lists why).
skips=(
  na23_carries_the_mftype_13_form_and_c12_a_non_isotropic_entry
  h2s_single_entry_is_the_6_25_mev_capture_line
  type1_and_type2_give_the_same_table
  read_sniffs_the_container
  every_obtainable_class_reads
  type2_write_reproduces_njoys_bytes
  type1_write_round_trips_through_values
)
skip_args=()
for s in "${skips[@]}"; do skip_args+=(--skip "$s"); done

# The crate's memory cap (its CLAUDE.md: ~12 GB), where the shell can set it.
if [[ "$(uname -s)" == Linux ]]; then ulimit -v 12582912 || true; fi

echo "running $(( ${#tests[@]} / 2 )) test files, leaving out ${#skips[@]} named tests"
set +e
# --no-fail-fast: one failing test file must not stop the rest (maintainer,
# 2026-10-29: the merge-to-main tests run with no fail-fast).
cargo test --release --no-fail-fast -p njoy-outram-park-fork "${tests[@]}" -- --exact "${skip_args[@]}" --nocapture 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
set -e
[[ $status -eq 0 ]] || { echo "tests failed (exit $status)" >&2; exit "$status"; }

bad="$(grep -E 'SKIP|OUTRAM_[A-Z_]+ unset|not generated' "$log" | grep -v '/nonexistent/' || true)"
if [[ -n "$bad" ]]; then
  echo "a test that CI runs skipped instead of checking; make its data available or add it to the left-out list with a reason:" >&2
  echo "$bad" >&2
  exit 1
fi
echo "every test that ran checked something; none skipped"
grep -E '^test result' "$log" | awk '{p+=$4; f+=$6; i+=$8} END {print p" passed, "f" failed, "i" ignored"}'
