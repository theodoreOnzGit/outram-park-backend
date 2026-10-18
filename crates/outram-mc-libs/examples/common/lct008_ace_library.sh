#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build, once, the ACE library that `tutorial_lct008 --ace-dir <dir>` reads:
# LEU-COMP-THERM-008's 38 tapes (`five_route_keff/nuclides.tsv`: the 35 the
# case-1 cards name, plus F-19, U-236 and Na-23, which other cases use), each
# as RECONR -> BROADR -> HEATR -> PURR -> ACER at 293.6 K and RECONR -> HEATR
# -> ACER at 0 K (DBRC's companion), and H in H2O from the LEAPR deck, all by
# `njoy-outram-park-fork`'s `write_ace_library` example (the generator the
# five-route campaign's route 5 uses). A table that exists is skipped, so an
# interrupted build resumes.
#
#   crates/outram-mc-libs/examples/common/lct008_ace_library.sh [OUT_DIR] [JOBS]
#
# OUT_DIR defaults to target/tutorials/lct008/ace (gitignored); JOBS (default
# 8) nuclides are processed at a time, one thread each. Run it from the
# repository root. Needs `reference-data/endf/`.
set -euo pipefail

OUT="${1:-target/tutorials/lct008/ace}"
JOBS="${2:-8}"
TSV=crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/nuclides.tsv

cargo build --release -p njoy-outram-park-fork --example write_ace_library
WRITER=target/release/examples/write_ace_library
mkdir -p "$OUT"

# One writer process per nuclide, JOBS at a time.
grep -v '^#' "$TSV" | awk -F'\t' '{print $1":"$2":"$3}' |
  xargs -P "$JOBS" -I{} "$WRITER" --out "$OUT" --temp 293.6 --nuclide {}

# The thermal table (no nuclide to process on this call).
"$WRITER" --out "$OUT" --temp 293.6 \
  --thermal-leapr HinH2O:HinH2O:n-001_H_001-ENDF8.0-Beta6.endf:lwtr

du -sh "$OUT"
