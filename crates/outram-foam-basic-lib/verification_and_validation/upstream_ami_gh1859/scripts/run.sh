#!/bin/bash
# Build and run the gh:#1859 AMI differential driver against an OpenFOAM ESI
# tree at commit 6e2595746a396d688edd9890f4aebaaa7ff4ecfd in which
# OSspecific/POSIX, Pstream (dummy), OpenFOAM, fileFormats, surfMesh and
# meshTools are built (the minimal chain upstream's src/Allwmake builds first).
#
# Usage: run.sh <path to the ESI source tree>
# Writes ../reference/ami_esi_6e25957.txt.
# Research, education and V&V only (RESPONSIBLE_USE.md).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
esi="$(cd "$1" && pwd)"
set +eu
# shellcheck disable=SC1091
source "$esi/etc/bashrc" WM_MPLIB=dummy WM_COMPILER=Gcc WM_LABEL_SIZE=32
set -eu
work="$(mktemp -d)"
cp -r "$here/../driver/." "$work/"
(cd "$work" && wmake)
"$FOAM_USER_APPBIN/amiDump" "$here/../reference/ami_esi_6e25957.txt"
echo "wrote $here/../reference/ami_esi_6e25957.txt"
