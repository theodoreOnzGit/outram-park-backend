#!/bin/bash
# Build the blockMesh primal cases for the polyDualMesh differential (gh:#1472).
. /home/user/upstream-build/scripts/of-env.sh
R=${POLYDUAL_WORK:?set POLYDUAL_WORK to a scratch directory}
CAV=$WM_PROJECT_DIR/tutorials/incompressible/icoFoam/cavity/cavity
mk() { # name
  rm -rf $R/$1; mkdir -p $R/$1/system $R/$1/constant
  cp $CAV/system/controlDict $CAV/system/fvSchemes $CAV/system/fvSolution $R/$1/system/
  sed -i '/^writePrecision/d' $R/$1/system/controlDict
  sed -i 's/^writeCompression.*$/writePrecision 17;\nwriteCompression off;/' $R/$1/system/controlDict
}
HDR='FoamFile { version 2.0; format ascii; class dictionary; object blockMeshDict; }'
mk cavity; cp $CAV/system/blockMeshDict $R/cavity/system/
mk annulus; cat > $R/annulus/system/blockMeshDict <<EOF
$HDR
scale 1;
vertices ( (1 0 0) (2 0 0) (1.4142135623730951 1.4142135623730951 0) (0.7071067811865476 0.7071067811865476 0)
           (1 0 0.3) (2 0 0.3) (1.4142135623730951 1.4142135623730951 0.3) (0.7071067811865476 0.7071067811865476 0.3) );
blocks ( hex (0 1 2 3 4 5 6 7) (10 12 2) simpleGrading (1 1 1) );
edges ( arc 1 2 (1.8477590650225735 0.7653668647301796 0)
        arc 0 3 (0.9238795325112867 0.3826834323650898 0)
        arc 5 6 (1.8477590650225735 0.7653668647301796 0.3)
        arc 4 7 (0.9238795325112867 0.3826834323650898 0.3) );
boundary ( inner { type wall; faces ((0 4 7 3)); } outer { type wall; faces ((1 2 6 5)); } );
EOF
mk warped; cat > $R/warped/system/blockMeshDict <<EOF
$HDR
scale 1;
vertices ( (0 0 0) (1 0 0) (1 1 0) (0 1 0) (0.1 -0.1 1) (1.2 0 1.15) (1.1 1.2 0.9) (0 1 1.3) );
blocks ( hex (0 1 2 3 4 5 6 7) (6 5 4) simpleGrading (1 1 1) );
edges ( );
boundary ( );
EOF
mk lshape; cat > $R/lshape/system/blockMeshDict <<EOF
$HDR
scale 1;
vertices ( (0 0 0) (1 0 0) (2 0 0) (0 1 0) (1 1 0) (2 1 0) (0 2 0) (1 2 0)
           (0 0 .5) (1 0 .5) (2 0 .5) (0 1 .5) (1 1 .5) (2 1 .5) (0 2 .5) (1 2 .5) );
blocks ( hex (0 1 4 3 8 9 12 11) (4 4 2) simpleGrading (1 1 1)
         hex (1 2 5 4 9 10 13 12) (4 4 2) simpleGrading (1 1 1)
         hex (3 4 7 6 11 12 15 14) (4 4 2) simpleGrading (1 1 1) );
edges ( );
boundary ( );
EOF
for c in cavity annulus warped lshape; do
  cd $R/$c
  blockMesh > log.blockMesh 2>&1
  echo "$c: $(grep -c . log.blockMesh) log lines; $(ls constant/polyMesh | tr '\n' ' ')"
done
