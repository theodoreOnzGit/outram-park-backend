#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors.
#
# Build and run the EGSnrc RNG harness (gh:#1289).
#
#   build.sh <EGSnrc checkout> <work dir>
#
# The EGSnrc checkout is read only. Everything is built in <work dir>:
#   1. Mortran3 from HEN_HOUSE/mortran3/mortran3.f (+ a stub egs_fdate, which
#      upstream gets from the configure-generated machine.f), and its macro
#      table mortran3.dat from mornew77.raw, as HEN_HOUSE/mortran3/Makefile does.
#   2. ranlux.f: prelude (the two type macros of src/egsnrc.macros:69-70) +
#      src/ranlux.macros + src/ranlux.mortran through Mortran3, the file order
#      of specs/all_common.spec (SOURCES).
#   3. gfortran with EGSnrc's shipped gfortran flags (specs/gfortran-suse11.4.conf:
#      FCFLAGS = -fPIC, FOPT = -O3 -ffast-math), and g++ -O3 (egspp_gcc64 conf)
#      on harness.cpp, which #includes egs++/egs_rndm.cpp unmodified.
#   4. Runs the harness twice. getGaussian calls the platform log, and on
#      x86-64 glibc picks an FMA-contracted log when the CPU has FMA, so
#      upstream's Gaussian stream depends on the machine:
#        <work dir>/            glibc's FMA variants masked (the portable
#                               build; this reference.txt is the committed one)
#        <work dir>/fma_variant/ glibc's default choice on this CPU
#      Every other stream is the same in both runs.
#
# Tested 2026-11-05 with GCC/gfortran 15 on x86-64 Linux.
set -euo pipefail
EGS="$1"
WORK="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
HH="$EGS/HEN_HOUSE"
mkdir -p "$WORK"
cd "$WORK"

printf '      subroutine egs_fdate(ounit)\n      integer ounit\n      write(ounit,*) "egs_rndm harness"\n      end\n' > stub_fdate.f
gfortran -O2 -std=legacy -o mortran3.exe "$HH/mortran3/mortran3.f" stub_fdate.f
./mortran3.exe -s -d "$HH/mortran3/mornew77.raw" -o7 mortran3.dat -o8 mornew77.lst

printf '%%C80\n%%Q1\nREPLACE {$REAL}    WITH {;real*8}\nREPLACE {$INTEGER} WITH {;integer*4}\n' > prelude.macros
cat prelude.macros "$HH/src/ranlux.macros" "$HH/src/ranlux.mortran" > ranlux_in.mortran
./mortran3.exe -s -d mortran3.dat -f ranlux_in.mortran -o7 ranlux.f -o8 ranlux.mortlst

FOPT="${FOPT:--O3 -ffast-math}"
gfortran -fPIC $FOPT -c ranlux.f -o ranlux.o
g++ -O3 -std=c++14 -I"$HERE/include" -I"$HH/egs++" -c "$HERE/harness.cpp" -o harness.o
g++ -O3 -I"$HERE/include" -I"$HH/egs++" -c "$HH/egs++/egs_input.cpp" -o egs_input.o
g++ -o harness harness.o egs_input.o ranlux.o -lgfortran
GLIBC_TUNABLES=glibc.cpu.hwcaps=-FMA,-AVX2,-FMA4 ./harness "$WORK" \
    "glibc FMA variants masked (GLIBC_TUNABLES=glibc.cpu.hwcaps=-FMA,-AVX2,-FMA4); FOPT=$FOPT" \
    > harness_stdout.txt
mkdir -p fma_variant
./harness "$WORK/fma_variant" "glibc default log variant on this CPU; FOPT=$FOPT" \
    > fma_variant/harness_stdout.txt
echo "wrote $WORK/reference.txt and $WORK/fma_variant/reference.txt"
