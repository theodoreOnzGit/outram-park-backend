#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors.
#
# Build upstream EGSnrc's egs_alias_table.cpp together with harness.cpp and
# run it. usage: build_and_run.sh OUTDIR
# EGSNRC defaults to the workspace's gitignored vendor/ clone
# (commit bd49a47aae3938f49f4100b8e018091861408b19).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
EGSNRC="${EGSNRC:-$here/../../../../vendor/egsnrc}"
out="${1:?usage: build_and_run.sh OUTDIR}"
mkdir -p "$out"
g++ -O2 -ffp-contract=off -std=c++17 \
    -I "$here/stub" -I "$EGSNRC/HEN_HOUSE/egs++" \
    "$here/harness.cpp" "$EGSNRC/HEN_HOUSE/egs++/egs_alias_table.cpp" \
    -o "$out/egs_alias_harness"
"$out/egs_alias_harness" "$out"
