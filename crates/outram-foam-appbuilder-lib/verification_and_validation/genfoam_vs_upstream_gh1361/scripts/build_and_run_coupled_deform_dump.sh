#!/bin/bash
# gh:#1852 / #1854 round 30: build the coupledDeformDump driver (libfiniteVolume/libsampling/libmeshTools of OpenFOAM-ESI v2506 only) and run
# the GeN-Foam time-step sequence (map all fields, deformMesh of neutroRegion, fvm::ddt on the moving mesh) on the 3D_SmallESFR meshes.
# usage: build_and_run_coupled_deform_dump.sh <out.txt>      Scratch: /home/user/upstream-build/gh1852 (delete after comparing).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1852
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
mkdir -p $W
rm -rf $W/driver; mkdir $W/driver
cp -r $HERE/upstream_driver/coupled_deform_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_mesh_deform_case.sh $W/case >/dev/null
# the ddt consumer needs an Euler ddtScheme on neutroRegion (the staged copy is the mechanics steadyState one)
cat > $W/case/system/neutroRegion/fvSchemes <<'EOT'
FoamFile { version 2.0; format ascii; class dictionary; object fvSchemes; }
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
EOT
S0=$(date +%s)
coupledDeformDump -case $W/case "$1" > $W/case/log.coupledDeformDump 2>&1
echo "coupledDeformDump exit $? in $(( $(date +%s)-S0 )) s"
tail -5 $W/case/log.coupledDeformDump
