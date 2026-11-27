#!/bin/bash
# gh:#1235 round 24: the `0/thermoMechanicalRegion/disp` files of the GFTractionDisplacement differential cases.
# usage: make_traction_disp.sh <t0|t1> <out file>
#   t0  the tutorial's OWN file, byte for byte (zeroDispl fixedValue; GFTractionDisplacement, traction 0, pressure 0)
#   t1  the same patches, loaded: nonzero traction and pressure, per-face lists on `bottom` (traction) and `topCR` (pressure)
SRC=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver/0/thermoMechanicalRegion/disp
case "$1" in
t0) cp "$SRC" "$2" ;;
t1)
  {
  cat <<'EOT'
FoamFile
{
    version     2.0;
    format      ascii;
    class       volVectorField;
    object      disp;
}

dimensions      [0 1 0 0 0 0 0];

internalField   uniform (0 0 0);

boundaryField
{
    "zeroDispl"
    {
        type            fixedValue;
        value           uniform (0 0 0);
    }
    "defaultFaces"
    {
        type            GFTractionDisplacement;
        traction        uniform (2e4 0 0);
        pressure        uniform 2e5;
        value           uniform (0 0 0);
    }
    "bottom"
    {
        type            GFTractionDisplacement;
EOT
  echo "        traction        nonuniform List<vector> 900"
  echo "("
  awk 'BEGIN{for(i=0;i<900;i++) printf "(%.17g 5e4 -5e4)\n", 1.0e5*(1+0.001*i)}'
  echo ");"
  cat <<'EOT'
        pressure        uniform 1e4;
        value           uniform (0 0 0);
    }
    "top"
    {
        type            GFTractionDisplacement;
        traction        uniform (1e5 -2e5 3e5);
        pressure        uniform 0;
        value           uniform (0 0 0);
    }
    "topCR"
    {
        type            GFTractionDisplacement;
        traction        uniform (0 0 0);
EOT
  echo "        pressure        nonuniform List<scalar> 108"
  echo "("
  awk 'BEGIN{for(i=0;i<108;i++) printf "%.17g\n", 5.0e4*(1+0.01*i)}'
  echo ");"
  cat <<'EOT'
        value           uniform (0 0 0);
    }
    "bottomCR"
    {
        type            GFTractionDisplacement;
        traction        uniform (5e4 0 0);
        pressure        uniform 0;
        value           uniform (0 0 0);
    }
}
EOT
  } > "$2" ;;
*) echo "usage: $0 <t0|t1> <out file>" >&2; exit 1 ;;
esac
