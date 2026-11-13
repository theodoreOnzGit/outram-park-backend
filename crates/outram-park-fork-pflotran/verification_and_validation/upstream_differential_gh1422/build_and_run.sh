#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
#
# Build the REAL PFLOTRAN at commit 669fbce204af39428020c6b9d74beae363376689
# against conda-forge PETSc 3.24.6 (route 2 of gh:#1422: there is no conda-forge
# pflotran package), then link driver.F90 against its objects and run it.
#
#   UP   = clone of bitbucket.org/pflotran/pflotran checked out at 669fbce
#   ENV  = micromamba env with: petsc=3.24.*=real*, mpich, hdf5=*=mpi_mpich*,
#          gfortran_linux-64, make   (conda-forge, 2026-11-13)
#   OUT  = directory for outputs
# PFLOTRAN needs PETSc >= 3.24.3 (pflotran_constants.F90 #error otherwise).
set -euo pipefail
UP=${1:?clone dir}; ENV=${2:?conda env dir}; OUT=${3:?output dir}
export PATH=$ENV/bin:$PATH PETSC_DIR=$ENV PETSC_ARCH=
here=$(cd "$(dirname "$0")" && pwd)
git -C "$UP" rev-parse HEAD | grep -q '^669fbce204af39428020c6b9d74beae363376689$'
src=$UP/src/pflotran
cd "$src"
HDF=(-L"$ENV/lib" -lhdf5_hl_fortran -lhdf5_fortran -lhdf5_hl -lhdf5)
make -j2 pflotran LIBS="${HDF[*]}"
# link the driver against every object except the two main programs
objs=$(ls *.o | grep -v -E '^(pflotran|pflotran_rxn|driver_gh1422)\.o$')
mpifort -O2 -fno-tree-vectorize -ffree-line-length-none -I"$ENV/include" -I. \
  -c "$here/driver.F90" -o driver_gh1422.o
flags="-L$ENV/lib -Wl,-rpath,$ENV/lib -Wl,-rpath-link,$ENV/lib -fopenmp -lpetsc ${HDF[*]} -llapack -lblas"
mpifort -o "$OUT/gh1422_driver" driver_gh1422.o $objs $flags -Wl,--allow-shlib-undefined
"$OUT/gh1422_driver" > "$OUT/upstream_output.txt"
wc -l "$OUT/upstream_output.txt"
