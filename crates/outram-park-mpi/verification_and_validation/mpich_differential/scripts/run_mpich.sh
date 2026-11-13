#!/bin/bash
# Regenerate a refs/*.txt file by running the unmodified MPICH driver (gh:#1435).
#
# usage: run_mpich.sh <out.txt> [VAR=value ...]
#   VAR=value pairs are exported as MPICH CVARs, e.g.
#   MPIR_CVAR_REDUCE_INTRA_ALGORITHM=binomial
#
# Environment (recorded in README.md): Ubuntu 24.04 apt `mpich` 4.2.0-5build3
# (ch4:ucx). That Debian build links the PMIx client, which hydra's mpiexec does
# not serve (every rank comes up as a singleton world of size 1), so the MPICH
# binary is launched with Open MPI's `mpirun`, whose embedded PMIx server it does
# connect to. The MPI library that runs is MPICH's libmpich.so.12 (ldd shows no
# Open MPI library linked into the driver).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$1"
shift
for kv in "$@"; do export "$kv"; done
tmp="$(mktemp -d)"
mpicc.mpich -O1 -o "$tmp/mpich_driver" "$here/../upstream_driver/mpich_driver.c" -lm
: > "$out"
for n in 2 3 4 5 6 7 8; do
  # One launch of the n=4 job hung once at 100 % of a core while the machine was
  # shared with other builds (not reproduced); a 60 s timeout and one retry keep
  # the script from blocking. A retry writes the whole n-block again.
  ok=0
  for attempt in 1 2 3; do
    if timeout 60 mpirun.openmpi --allow-run-as-root --oversubscribe -n "$n" "$tmp/mpich_driver" > "$tmp/out_$n.txt"; then
      ok=1
      break
    fi
  done
  if [ "$ok" != 1 ]; then echo "n=$n failed 3 times" >&2; exit 1; fi
  cat "$tmp/out_$n.txt" >> "$out"
done
rm -rf "$tmp"
