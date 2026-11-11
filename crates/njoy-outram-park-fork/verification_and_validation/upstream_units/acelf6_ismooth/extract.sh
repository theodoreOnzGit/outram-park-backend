#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# gh:#1368: extract the two `ismooth` sqrt(E) extension branches of NJOY2016 `acelf6`
# (acefc.f90) VERBATIM, from the comment "extend low histogram bins" to the line before
# `xss(nexd)=lep+10*nd`, inside subroutine acelf6 only (the same text recurs in another
# routine). The block is wrapped in `subroutine smooth` with the variables it uses. The only
# line added is `eout1=scr(7)/emev` BEFORE the block, which both versions get so that the
# unguarded 2016.79 text compiles with the same declarations; 2016.79's block never reads it,
# 2016.80's reads it exactly as acelf6 does (acefc.f90:7547 in 2016.80).
# Usage: extract.sh 80|79 outdir   (set NJOY_UPSTREAM_SOURCE to the upstream_source folder
# holding NJOY2016/ and NJOY2016-2016.79/ if they are not under this crate)
# NJOY2016 is modified-BSD-3 (LANL); see crates/njoy-outram-park-fork/LICENSE.njoy.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
up=${NJOY_UPSTREAM_SOURCE:-$here/../../../upstream_source}
case "$1" in
  80) src=$up/NJOY2016/src/acefc.f90
      sum=66ccec86db2fe1acbc9bb8e3713aecc21c9f14255b9ac622eb7eb66ed4f7c3f5 ;;
  79) src=$up/NJOY2016-2016.79/src/acefc.f90
      sum=979d41de609a8717722b3d9c2965c9416ef1d5a2402cca0188d1f7f70151b3c5 ;;
  *) echo "usage: extract.sh 80|79 outdir"; exit 2 ;;
esac
out=$2; mkdir -p "$out"
echo "$sum  $src" | sha256sum -c -
srcdir=$(dirname "$src")
{
  echo "! EXTRACTED by extract.sh from NJOY2016 acefc.f90 acelf6 (version $1)"
  echo "module smoothx"
  echo "   use locale"
  echo "   use util, only: sigfig"
  echo "   implicit none"
  echo "contains"
  echo "   subroutine smooth(scr,nx,n,nd,ncyc,na,law,lang,lep,zap,ismooth,mt,ee,jscr)"
  echo "   integer::nx,n,nd,ncyc,na,law,lang,lep,ismooth,mt,jscr"
  echo "   real(kr)::scr(*),zap,ee"
  echo "   integer::ix,ii,nn,nsyso"
  echo "   real(kr)::fx,ex,cx,cxx,val,e1,p1,e2,p2,eout1"
  echo "   real(kr),parameter::emev=1.e6_kr"
  echo "   nsyso=6"
  echo "   eout1=scr(7)/emev"
  awk '/^   subroutine acelf6\(/{s=1} s&&/extend low histogram bins/{p=1} p&&/xss\(nexd\)=lep\+10\*nd/{exit} p{print}' "$src"
  echo "   end subroutine smooth"
  echo "end module smoothx"
} > "$out/ismooth_extract_v$1.f90"
cp "$srcdir/locale.f90" "$srcdir/mainio.f90" "$srcdir/util.f90" "$srcdir/phys.f90" "$out/"
echo "extracted $out/ismooth_extract_v$1.f90"
