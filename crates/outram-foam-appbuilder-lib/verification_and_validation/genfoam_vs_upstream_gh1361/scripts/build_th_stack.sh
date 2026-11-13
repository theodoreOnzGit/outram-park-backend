#!/bin/bash
# gh:#1361 round 3: build GeN-Foam 652b3da0cc55's libFFSSrc2 and libFFSSolvers2
# (thermalHydraulics) against the ESI v2506 closure libraries in
# OpenFOAM-v2506/platforms/*/lib. Sources untouched. Edits to Make/options:
#  - drop `-g` from FFSSrc2 (debug info only; saves disk)
#  - drop `-lthermoTools` from FFSSolvers2 (not built; the README records whether a symbol needed it)
# Objects are deleted after each link. Time and peak RSS go to build-genfoam-th-*.log.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
SRC=/home/user/upstream-build/GeN-Foam-src/src/classes/thermalHydraulics
buildlib() {
  d=$1; name=$2; edit=$3
  cd "$SRC/$d" || return 1
  [ -n "$edit" ] && sed -i "$edit" Make/options
  wmakeLnInclude .
  /usr/bin/time -v wmake -j2 libso . > /home/user/upstream-build/build-genfoam-th-$name.log 2>&1
  echo "$name exit $? : $(grep -E 'Elapsed|Maximum resident' /home/user/upstream-build/build-genfoam-th-$name.log | tr '\n' ' ')"
  du -sk Make/linux64GccDPInt32Opt | sed "s/^/$name objects (kB): /"
  find Make/linux64GccDPInt32Opt -name '*.o' -delete
  df -h / | tail -1
}
buildlib src ffssrc2 '/^    -g \\$/d'
buildlib solvers ffssolvers2 '/-lthermoTools/d'
ls -l /root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt/lib
