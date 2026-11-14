#!/bin/bash
# run_upstream.sh <workdir>: blockMesh the three cases and decompose each with ESI v2506
# decomposePar for every variant below. Needs the ESI environment
# (scripts/upstream/build-esi-v2506-and-genfoam.sh decompose; run through with-of.sh):
#   with-of.sh crates/outram-foam-mesh/verification_and_validation/decompose_par_gh1381/run_upstream.sh /path/to/workdir
set -e
here=$(cd "$(dirname "$0")" && pwd)
work=${1:?workdir}
mkdir -p "$work"
# case  variant-name  method  nProcs  n  order  delta   ('-' = unset)
variants=(
 "cavity simple_2_2_1 simple 4 (2_2_1) - -"
 "cavity simple_4_1_1 simple 4 (4_1_1) - -"
 "cavity simple_3_2_1_d0 simple 6 (3_2_1) - 0"
 "cavity hier_2_2_1_xyz hierarchical 4 (2_2_1) xyz -"
 "cavity hier_3_2_1_yxz hierarchical 6 (3_2_1) yxz -"
 "cavity hier_2_3_1_d0 hierarchical 6 (2_3_1) xyz 0"
 "box3d simple_2_2_2 simple 8 (2_2_2) - -"
 "box3d simple_3_2_2 simple 12 (3_2_2) - -"
 "box3d simple_4_1_2_d0 simple 8 (4_1_2) - 0"
 "box3d hier_2_2_2_zyx hierarchical 8 (2_2_2) zyx -"
 "box3d hier_3_2_2_xyz hierarchical 12 (3_2_2) xyz -"
 "box3d hier_2_2_3_yzx hierarchical 12 (2_2_3) yzx -"
 "box3d hier_4_1_2_d0 hierarchical 8 (4_1_2) xyz 0"
 "lshape simple_2_2_1 simple 4 (2_2_1) - -"
 "lshape simple_3_1_2_d0 simple 6 (3_1_2) - 0"
 "lshape hier_2_2_2_xyz hierarchical 8 (2_2_2) xyz -"
 "lshape hier_3_2_1_yxz hierarchical 6 (3_2_1) yxz -"
)
for v in "${variants[@]}"; do
  read -r c name method np n order delta <<<"$v"
  n=${n//_/ }
  d=$work/${c}_$name
  rm -rf "$d"; mkdir -p "$d/system" "$d/constant"
  cp "$here/cases/$c.blockMeshDict" "$d/system/blockMeshDict"
  cat >"$d/system/controlDict" <<EOT
FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }
application blockMesh; startFrom startTime; startTime 0; stopAt endTime; endTime 1;
deltaT 1; writeControl timeStep; writeInterval 1; writeFormat ascii; writePrecision 17;
writeCompression off; timeFormat general; timePrecision 6;
EOT
  extra=""
  [ "$order" = "-" ] || extra="$extra order $order;"
  [ "$delta" = "-" ] || extra="$extra delta $delta;"
  cat >"$d/system/decomposeParDict" <<EOT
FoamFile { version 2.0; format ascii; class dictionary; object decomposeParDict; }
numberOfSubdomains $np;
method $method;
${method}Coeffs { n ($n); $extra }
EOT
  (cd "$d" && blockMesh >log.blockMesh 2>&1 && decomposePar -force >log.decomposePar 2>&1) || { echo "FAIL $d"; exit 1; }
  echo "ok $c $name"
done
