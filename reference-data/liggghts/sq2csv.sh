#!/bin/sh
# Concatenate LIGGGHTS superquadric frame dumps dump.F<step>
# (id x y z vx vy vz omegax omegay omegaz quat1 quat2 quat3 quat4) into one CSV:
# step,id,x,y,z,vx,vy,vz,omegax,omegay,omegaz,quat1,quat2,quat3,quat4. Pure
# reshape, fields copied as printed (%.17g from the output-precision patch).
#   ./sq2csv.sh <out.csv> dump.F1 dump.F500 ...
set -eu
out=$1; shift
echo "step,id,x,y,z,vx,vy,vz,omegax,omegay,omegaz,quat1,quat2,quat3,quat4" > "$out"
for f in "$@"; do
  awk '/^ITEM: TIMESTEP/{m="s";next} /^ITEM: ATOMS/{m="a";next} /^ITEM:/{m="";next} m=="s"{s=$1;m="";next} m=="a"{printf "%s",s; for(i=1;i<=NF;i++) printf ",%s",$i; printf "\n"}' "$f" >> "$out"
done
