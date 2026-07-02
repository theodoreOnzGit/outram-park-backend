#!/usr/bin/env bash
# Run the njoy-outram-park-fork test suite under a hard ~12 GB memory cap.
#
# WHY: the ACER / thermal S(alpha,beta) import paths build large per-energy
# emission tables. A malformed record (a cursor that fails to advance, an
# unbounded grid) turns into runaway allocation that can freeze the whole
# machine. This wrapper caps the address space so a runaway aborts the test
# process cleanly instead of taking the box down.
#
# HARD RULE (see CLAUDE.md): njoy unit tests must run under this cap. Use this
# script — do not invoke `cargo test -p njoy-outram-park-fork` bare.
#
# Usage:
#   crates/njoy-outram-park-fork/scripts/test.sh [extra cargo test args]
#
# Examples:
#   crates/njoy-outram-park-fork/scripts/test.sh
#   crates/njoy-outram-park-fork/scripts/test.sh thermal -- --nocapture
set -euo pipefail

# 12 GiB, expressed in KiB for `ulimit -v` (virtual address space).
MEM_CAP_KIB=$((12 * 1024 * 1024))

ulimit -v "${MEM_CAP_KIB}"

# --release is mandatory workspace-wide; --lib --tests skips the examples.
exec cargo test -p njoy-outram-park-fork --lib --tests --release "$@"
