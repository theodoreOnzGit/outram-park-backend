#!/bin/bash
# usage: run_arm.sh ARM UF_BIN LU_BIN OUTDIR   (gh:#1635 item 3; one MC process at a time)
ARM=$1; UF=$2; LU=$3; OUT=$4
ulimit -v 12582912
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf
export THREADS=3
# control: explicit ablation, no law bound, expected bit-identical old vs new
GRAPHITE_SAB=0 PARTICLES=500 INACTIVE=5 ACTIVE=10 P_HIST=20000 SEED=777 $UF > $OUT/uf_${ARM}_ablate.out 2> $OUT/uf_${ARM}_ablate.err
GRAPHITE_SAB=0 PARTICLES=500 INACTIVE=5 ACTIVE=10 SEED=778 RADII=0.3 CONTROL=0 $LU > $OUT/lu_${ARM}_ablate.out 2> $OUT/lu_${ARM}_ablate.err
for i in 1 2 3 4 5 6; do
  s=$((20261100+i))
  PARTICLES=2000 INACTIVE=15 ACTIVE=40 P_HIST=100000 SEED=$s $UF > $OUT/uf_${ARM}_seed$s.out 2> $OUT/uf_${ARM}_seed$s.err
  PARTICLES=2000 INACTIVE=15 ACTIVE=40 SEED=$s RADII=0.3 CONTROL=0 $LU > $OUT/lu_${ARM}_seed$s.out 2> $OUT/lu_${ARM}_seed$s.err
done
echo done > $OUT/${ARM}_DONE
