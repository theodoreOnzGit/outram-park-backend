#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0
#
# Build and run the cmapf/coordtrafo code-to-code driver
# (dev/flexpart_reference_cmapf.f90) at BOTH precisions, writing
#
#   tests/data/flexpart_cmapf_real4.csv   as FLEXPART ships (default real = 4)
#   tests/data/flexpart_cmapf_real8.csv   -fdefault-real-8 -fdefault-double-8
#
# Upstream files are compiled VERBATIM. The one generated file is
# cmapf_open_mod.f90: cmapf_mod.f90 with the module renamed and its single
# `  private` statement deleted, so the driver can reach the thirteen routines
# upstream keeps private. The script asserts that this is the ENTIRE change.
# The verbatim cmapf_mod is linked as well and the driver checks the two
# copies agree on every public routine (see the driver header).
#
# Build products go to $CMAPF_BUILD (default: a mktemp dir), never the repo.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$CRATE_DIR/upstream_source/FLEXPART/src"
OUT="$CRATE_DIR/tests/data"
BUILD="${CMAPF_BUILD:-$(mktemp -d)}"

if [ ! -d "$SRC" ]; then
  echo "upstream FLEXPART clone not found at $SRC" >&2
  echo "  git clone https://github.com/flexpart/flexpart.git $CRATE_DIR/upstream_source/FLEXPART" >&2
  echo "  git -C $CRATE_DIR/upstream_source/FLEXPART checkout 3d7eebf" >&2
  exit 1
fi
command -v gfortran >/dev/null || { echo "gfortran not installed" >&2; exit 1; }

mkdir -p "$BUILD"
OPEN="$BUILD/cmapf_open_mod.f90"
sed -e 's/^module cmapf_mod$/module cmapf_open_mod/' \
    -e 's/^end module cmapf_mod$/end module cmapf_open_mod/' \
    -e '/^  private$/d' \
    "$SRC/cmapf_mod.f90" > "$OPEN"
# The diff must be exactly: module line, `private` deleted, end-module line.
changed=$(diff "$SRC/cmapf_mod.f90" "$OPEN" | grep -c '^[<>]' || true)
if [ "$changed" != "5" ]; then
  echo "cmapf_open_mod.f90 differs from cmapf_mod.f90 by more than the visibility change:" >&2
  diff "$SRC/cmapf_mod.f90" "$OPEN" >&2 || true
  exit 1
fi

FILES=(
  "$SRC/par_mod.f90" "$SRC/com_mod.f90" "$SRC/point_mod.f90"
  "$SRC/cmapf_mod.f90" "$OPEN" "$SRC/coordtrafo.f90"
)

build_and_run_cmapf () {
  local tag="$1"; shift
  local extra=("$@")
  local dir="$BUILD/cmapf_$tag"
  rm -rf "$dir"; mkdir -p "$dir"
  gfortran -O2 -mcmodel=medium -std=legacy -fallow-argument-mismatch -w \
    -J"$dir" -I"$dir" "${extra[@]}" \
    "${FILES[@]}" \
    "$CRATE_DIR/dev/flexpart_reference_cmapf.f90" \
    -o "$dir/flexpart_reference_cmapf"
  local out="$OUT/flexpart_cmapf_$tag.csv"
  # coordtrafo's NOTICE lines (list-directed, leading blank) are not rows.
  "$dir/flexpart_reference_cmapf" | grep -v '^ ' > "$out"
  # coordtrafo's all-points-removed path ends in `stop`, so it runs in its
  # own process. Keep its input rows; record the stop only if upstream printed
  # its error and did NOT return (no coordtrafo_n row).
  local stoplog="$dir/stop.log"
  "$dir/flexpart_reference_cmapf" stop > "$stoplog" || true
  if grep -q 'NO PARTICLE RELEASES ARE DEFINED' "$stoplog" \
     && ! grep -q '^coordtrafo_n,' "$stoplog"; then
    grep -v '^ ' "$stoplog" >> "$out"
    echo "coordtrafo_stop,9,1" >> "$out"
  else
    echo "coordtrafo stop scenario did not stop as expected ($tag):" >&2
    cat "$stoplog" >&2
    exit 1
  fi
  echo "$(basename "$out"): $(grep -vc '^#\|^function,' "$out") rows, $(wc -c < "$out") bytes" >&2
}

mkdir -p "$OUT"
echo "building real(4) cmapf reference (as FLEXPART ships)..." >&2
build_and_run_cmapf real4
echo "building real(8) cmapf reference..." >&2
build_and_run_cmapf real8 -fdefault-real-8 -fdefault-double-8

if [ -z "${CMAPF_BUILD:-}" ]; then rm -rf "$BUILD"; fi
