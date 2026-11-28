#!/bin/bash
# round 85 (gh:#1946): the real-binary single-solve dumps of the S1 (FFTF p_rgh) and S2 (TStruct) variants.
# usage: run_gamg_features_real_r85.sh <out dir> [variants...]   (a1..a8, b1..b7)
VV=$(cd "$(dirname "$0")/.." && pwd)
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$1; shift
W=/home/user/upstream-build/r85gamg/runs
export GAMGDUMP_LIB=/home/user/upstream-build/r85gamg/lib
mkdir -p "$OUT" "$W"
for v in "$@"; do
  case $v in
    a*)
      unset GAMGDUMP_TARGET; export GAMGDUMP_MAXSOLVES=1
      case $v in
        a1) S="smoother FDIC;";;
        a2) S="smoother nonBlockingGaussSeidel;";;
        a3) S="smoother DIC; interpolateCorrection true;";;
        a4) S="smoother DIC; directSolveCoarsest true;";;
        a5) S="smoother DIC; mergeLevels 2;";;
        a6) S="smoother DIC; agglomerator algebraicPair;";;
        a7) S="smoother DIC; updateInterval 10;";;
        a8) S="smoother DIC; cacheAgglomeration false;"; export GAMGDUMP_TARGET=1; export GAMGDUMP_MAXSOLVES=2;;
      esac
      GAMG_SMOOTHER="$S" bash $VV/scripts/run_fftf_gamg_real.sh $W/$v tight 1 dump
      D=$W/$v/gamgdump
      ;;
    b*)
      C=$W/$v/case
      rm -rf $W/$v; mkdir -p $W/$v
      SHIPPED=1 bash $VV/scripts/make_legacy_tm_case.sh $C > /dev/null
      python3 -I $HERE/edit_tstruct_r85.py $C/system/thermoMechanicalRegion/fvSolution $v
      sed -i "/^application/i libs (\"$GAMGDUMP_LIB/libGAMGdump.so\");" $C/system/controlDict
      mkdir -p $C/gamgdump
      ( . /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
        export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
        GAMGDUMP_DIR=$C/gamgdump GAMGDUMP_MAXSOLVES=1 LEGACY_TM_DISP_ONLY=1 legacyTmDump -case $C $W/$v/out.txt 1 > $C/log.legacyTmDump 2>&1
        echo "$v legacyTmDump exit $?" )
      D=$C/gamgdump
      ;;
  esac
  for f in matrix agglom cycles solution; do
    [ -f $D/$f.txt ] && gzip -9 -c $D/$f.txt > $OUT/gamg_r85_${v}_$f.ref.txt.gz
  done
  ls -la $OUT/gamg_r85_${v}_* 2>/dev/null | awk '{print $5, $9}'
  [ -f $W/$v/log.GeN-Foam ] && grep -m3 -i "fatal\|error" $W/$v/log.GeN-Foam
  [ -f $W/$v/case/log.legacyTmDump ] && grep -m3 -i "fatal\|error" $W/$v/case/log.legacyTmDump
  rm -rf $W/$v
done
echo ALLDONE
