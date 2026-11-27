#!/bin/bash
# Build the minimal OpenFOAM ESI library chain the gh:#1859 AMI driver links:
# OSspecific/POSIX, dummy Pstream, OpenFOAM, fileFormats, surfMesh, meshTools
# (the first stage of upstream's src/Allwmake), serial, double, 32-bit labels.
#
# Usage: build-upstream.sh <ESI source tree at 6e2595746a396d688edd9890f4aebaaa7ff4ecfd>
# The tree needs at least: wmake etc bin META-INFO src/OpenFOAM src/OSspecific
# src/Pstream src/fileFormats src/surfMesh src/meshTools (a sparse checkout is
# enough). Needs g++ and flex. Took 20 min with 3 jobs on 4 cores (2026-11-27).
set -e
cd "$1"
set +e
# shellcheck disable=SC1091
source etc/bashrc WM_MPLIB=dummy WM_COMPILER=Gcc WM_LABEL_SIZE=32 2>/dev/null
set -e
export WM_NCOMPPROCS=3
( cd wmake/src && make )
( cd src && wmakeLnInclude -u OpenFOAM && wmakeLnInclude -u OSspecific/POSIX \
    && (OpenFOAM/Alltouch -check 2>/dev/null || true) )
( cd src && OSspecific/POSIX/Allwmake )
( cd src && Pstream/Allwmake )
for d in src/OpenFOAM src/fileFormats src/surfMesh src/meshTools; do
  ( cd "$d" && wmake -j3 libso )
done
