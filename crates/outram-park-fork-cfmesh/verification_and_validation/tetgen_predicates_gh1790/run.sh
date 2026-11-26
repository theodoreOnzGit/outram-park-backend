#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
#
# gh:#1790: regenerate the TetGen predicate vectors from UPSTREAM predicates.cxx.
#
#   run.sh <tetgen-upstream-dir> [build-dir]
#
# <tetgen-upstream-dir> is a clone of https://github.com/libigl/tetgen at
# e05aca7df74e (TetGen 1.6.0). It is only read. Builds:
#   gen_O2    upstream predicates.cxx, g++ -O2 -ffp-contract=off  -> the vectors
#   gen_O0    the same at -O0 (upstream's own makefile builds predicates at -O0)
#   gen_count instrument.awk's copy with stage counters, -O2 -ffp-contract=off
# and checks that all three write the same file, byte for byte. Then gzips it
# (-n, -9) into test_data/tetgen_predicates/ and prints the stage counts.
set -euo pipefail
up="$(cd "$1" && pwd)"
here="$(cd "$(dirname "$0")" && pwd)"
build="${2:-$(mktemp -d)}"
mkdir -p "$build/count"
test "$(git -C "$up" rev-parse --short=12 HEAD)" = "e05aca7df74e" || { echo "upstream is not e05aca7df74e"; exit 1; }

flags=(-std=c++17 -ffp-contract=off -fno-fast-math)
g++ -O2 "${flags[@]}" -I"$up" "$here/gen_vectors.cxx" -o "$build/gen_O2"
g++ -O0 "${flags[@]}" -I"$up" "$here/gen_vectors.cxx" -o "$build/gen_O0"
awk -f "$here/instrument.awk" "$up/predicates.cxx" > "$build/count/predicates.cxx"
g++ -O2 "${flags[@]}" -DCOUNT_STAGES -I"$build/count" -I"$up" "$here/gen_vectors.cxx" -o "$build/gen_count"

"$build/gen_O2" "$build/vectors_O2.bin"
"$build/gen_O0" "$build/vectors_O0.bin"
"$build/gen_count" "$build/vectors_count.bin" | tee "$build/stage_counts.txt"
cmp "$build/vectors_O2.bin" "$build/vectors_O0.bin"
cmp "$build/vectors_O2.bin" "$build/vectors_count.bin"
echo "O2, O0 and counted builds agree: $(stat -c %s "$build/vectors_O2.bin") bytes, sha256 $(sha256sum < "$build/vectors_O2.bin" | cut -c1-16)"

out="$here/../../test_data/tetgen_predicates/predicates_vectors.bin.gz"
gzip -n -9 -c "$build/vectors_O2.bin" > "$out"
echo "wrote $out ($(stat -c %s "$out") bytes)"
