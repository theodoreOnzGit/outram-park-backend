#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Extract NJOY2016 samm.f90 `getfg`, `bigeta`, `getps` VERBATIM (by name, not
# line number) for one upstream version, gh:#1366. Usage:
#   extract.sh 80|79 <outdir>
# NJOY2016 is modified-BSD-3 (LANL); see crates/njoy-outram-park-fork/LICENSE.njoy.
# Edits, all mechanical and listed here: (1) the three routines are wrapped in
# `module sammx` (public) because upstream's `samm` module is `private`;
# (1b) since 2026-11-10 also `coulx`, `asymp1`, `asymp2`, `taylor`, `xsigll`, `end1` (gh:#1368); (2) `kseries` is `bigeta` cut at the "Now have I_0(Z)" comment, with ONE
# added line storing the four Bessel values in a module array so K0/K1 can be
# printed. Nothing inside a series loop is touched.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
up=${NJOY_UPSTREAM_SOURCE:-$here/../../../upstream_source}   # override to point at another worktree's clones
case "$1" in
  80) src=$up/NJOY2016/src
      sum=1d37aad5628d79f57d32119c4a4a9c871e44bf288591c927e736f2552e416582 ;;
  79) src=$up/NJOY2016-2016.79/src
      sum=0f6c50575f5ba2e6fb88bb6cd32b38ab959265619d279c3055d27f981625d056 ;;
  *) echo "usage: extract.sh 80|79 outdir"; exit 2 ;;
esac
out=$2; mkdir -p "$out"
echo "$sum  $src/samm.f90" | sha256sum -c -
{
  echo "! EXTRACTED by extract.sh from NJOY2016 samm.f90 (version $1)"
  echo "module sammx"
  echo "   use locale"
  echo "   implicit none"
  echo "   real(kr)::kout(4)"
  echo "contains"
  awk '/^   subroutine getfg\(/{p=1} p{print} /^   end subroutine getfg/{p=0}' "$src/samm.f90"
  awk '/^   subroutine bigeta\(/{p=1} p{print} /^   end subroutine bigeta/{p=0}' "$src/samm.f90"
  awk '/^   subroutine getps /{p=1} p{print} /^   end subroutine getps/{p=0}' "$src/samm.f90"
  # gh:#1368 (coulx finite-eta branch): the routines `coulx` calls, verbatim.
  for r in coulx asymp1 asymp2 taylor xsigll end1; do
    awk -v r="$r" '$0 ~ "^   subroutine " r "\\(" {p=1} p{print} $0 ~ "^   end subroutine " r {p=0}' "$src/samm.f90"
  done
  awk '/^   subroutine bigeta\(/{p=1; sub(/subroutine bigeta/,"subroutine kseries")}
       p && /--Now have I_0/ {print "   kout=(/ai0,ak0,ai1,ak1/)"; print "   return"; print "   end subroutine kseries"; p=0; next}
       p{print}' "$src/samm.f90"
  echo "end module sammx"
} > "$out/samm_extract_v$1.f90"
cp "$src/locale.f90" "$src/mainio.f90" "$src/util.f90" "$src/phys.f90" "$out/"
echo "extracted $out/samm_extract_v$1.f90"
