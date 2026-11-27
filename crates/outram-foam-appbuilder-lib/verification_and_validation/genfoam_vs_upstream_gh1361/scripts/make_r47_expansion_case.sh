#!/bin/bash
# gh:#1898 (README round 47, item 2c): run upstream's own XS class (upstream_driver/dump_xs_exp, `dumpXSexp`) on the df_slab mesh with
# a closed-form `disp` and the axExp / radExp feedback data cases/df_slab/nuclearData.x0.
#   make_r47_expansion_case.sh <name> <A|B> [axialOrientation, default "0 0 1"]
# disp (cell centres x = y = 0.05 m, z = the mean of the two bounding z planes; + - * / only):
#   A: (0.004 x (1+z), 0.003 y (1 - z/2), 0.008 z + 0.004 z z)
#   B: (0.002 (y - x), 0.001 x z, 0.01 (z + 1.0985)/2.197)
# The probe dumps the displacement it READ, so the port's inputs are upstream's own values (the awk centres need not equal ESI's).
# Output (scratch, deleted after extraction): /home/user/upstream-build/gh1887/runs/<name>/xsexp/.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
NAME=$1; V=$2; AX=${3:-0 0 1}
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
W=/home/user/upstream-build/gh1887/runs/$NAME
rm -rf "$W"; mkdir -p "$W"
cp -r /home/user/upstream-build/GeN-Foam-src/Tutorials/guidedCases/2_reactorSlabReflected_1D_1Gr_neutronicDiffusion/. "$W/"
cd "$W" || exit 1
cp -r 0.orig 0
cp "$HERE/cases/df_slab/nuclearData.x0" constant/neutroRegion/nuclearData
if [ "$AX" != "0 0 1" ]; then
  sed -i "s/^fastNeutrons .*/fastNeutrons    true;\naxialOrientation ($AX);/" constant/neutroRegion/nuclearData
fi
zcat "$HERE/cases/df_slab/cellZones.gz" > constant/neutroRegion/polyMesh/cellZones
for f in points faces owner neighbour boundary; do
  zcat "$HERE/cases/guided2/polyMesh/$f.gz" > constant/neutroRegion/polyMesh/$f
done
zcat "$HERE/cases/guided2/polyMesh/points.gz" | awk 'NR>19 && /^\(/{gsub(/[()]/,""); if($1==0&&$2==0)print $3}' > zplanes.txt
N=$(( $(wc -l < zplanes.txt) - 1 ))
rm -f 0/neutroRegion/disp
{
  echo 'FoamFile { version 2.0; format ascii; class volVectorField; object disp; }'
  echo 'dimensions [0 1 0 0 0 0 0];'
  echo "internalField nonuniform List<vector> $N"
  echo '('
  awk -v v="$V" '{z[NR-1]=$1} END{for(c=0;c<NR-1;c++){zc=(z[c]+z[c+1])/2; x=0.05; y=0.05;
     if(v=="A") printf "(%.17g %.17g %.17g)\n", 0.004*x*(1+zc), 0.003*y*(1-zc/2), 0.008*zc+0.004*zc*zc;
     else printf "(%.17g %.17g %.17g)\n", 0.002*(y-x), 0.001*x*zc, 0.01*(zc+1.0985)/2.197}}' zplanes.txt
  echo ')'
  echo ';'
  echo 'boundaryField { walls { type zeroGradient; } top { type zeroGradient; } bottom { type zeroGradient; } }'
} > 0/neutroRegion/disp
dumpXSexp > log.dump 2>&1
tail -3 log.dump
