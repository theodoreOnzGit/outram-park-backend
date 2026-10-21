#!/bin/bash
# Build PETIR's hypot shim and the MPFR driver, and write the reference.
# usage (from the repository root, after `cargo build --release -p petir`):
#   reference-data/core-math/build_and_run.sh <core-math checkout> <N> <out.txt>
# <core-math checkout> provides src/binary64/hypot/hypot.wc and
# src/binary32/hypot/hypotf.wc (CORE-MATH's worst-case inputs).
set -e
cm=$1; n=$2; out=$3
deps=target/release/deps
petir=$(ls -t "$deps"/libpetir-*.rlib | head -1)
libm=$(ls -t "$deps"/liblibm-*.rlib | head -1)
tmp=$(mktemp -d)
rustc --edition 2021 --crate-type staticlib -C opt-level=3 -C panic=abort \
    --extern petir="$petir" --extern libm="$libm" -L "$deps" \
    reference-data/core-math/petir_hypot_shim.rs -o "$tmp/shim.a"
gcc -O2 -o "$tmp/driver" reference-data/core-math/hypot_mpfr_driver.c "$tmp/shim.a" -lmpfr -lgmp -lm
"$tmp/driver" "$cm/src/binary64/hypot/hypot.wc" "$cm/src/binary32/hypot/hypotf.wc" "$n" > "$out"
rm -rf "$tmp"
