#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors.
#
# Build EGSnrc's unmodified egs_interpolator.cpp with egs_interp_harness.cpp
# and regenerate reference.txt (gh:#1289). Needs g++ and the gitignored
# EGSnrc clone at <repo>/vendor/egsnrc (commit bd49a47aae39...).
#
#   ./build_and_run.sh [OUT_DIR]
#
# Writes OUT_DIR/reference.txt (copy it over the committed one only on
# purpose), OUT_DIR/full_dump.txt (every value, for
# `EGS_INTERP_FULL_DUMP=... cargo test --release -p petir --test
# egs_interpolator_vs_egsnrc -- --ignored`), and the same two files for the
# two sensitivity builds (-march=native, -ffast-math).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
# EGSNRC_DIR overrides the clone's location (e.g. from a git worktree, which
# has no vendor/ of its own).
egspp="${EGSNRC_DIR:-$repo/vendor/egsnrc}/HEN_HOUSE/egs++"
out="${1:-$here/build}"
mkdir -p "$out"
[ -f "$egspp/egs_interpolator.cpp" ] || { echo "missing $egspp (clone EGSnrc into vendor/egsnrc)"; exit 1; }

build() {   # name, flags...
    local name="$1"; shift
    g++ -std=c++17 "$@" -I"$here/stub" -I"$egspp" \
        "$here/egs_interp_harness.cpp" "$egspp/egs_interpolator.cpp" \
        -o "$out/harness_$name"
    "$out/harness_$name" "$out/full_dump_$name.txt" > "$out/reference_$name.txt"
}

# The reference: EGSnrc's egspp_gcc64_example.conf `opt = -O3`, baseline
# x86-64 (no FMA instructions, so no contraction can happen).
build o3 -O3
# Sensitivity builds, NOT the reference: FMA contraction (GCC's default
# -ffp-contract=fast once -march allows FMA) and -ffast-math (used by some of
# EGSnrc's example configs).
build native -O3 -march=native
build fastmath -O3 -ffast-math

cp -f "$out/reference_o3.txt" "$out/reference.txt"
cp -f "$out/full_dump_o3.txt" "$out/full_dump.txt"
for v in native fastmath; do
    if cmp -s "$out/full_dump_o3.txt" "$out/full_dump_$v.txt"; then
        echo "$v: identical to -O3"
    else
        echo "$v: $(diff "$out/full_dump_o3.txt" "$out/full_dump_$v.txt" | grep -c '^<') lines differ from -O3"
    fi
done
