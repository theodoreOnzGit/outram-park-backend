#!/bin/sh
# Writes the three square planes in.htr10_conus_massflow counts through (gh:#1416).
#   ./make_htr10_massflow_planes.sh          (writes beside this script)
# name -> z [m], half-width [m]: a (z = -0.02, over the conus), b (z = -0.10, in the
# conus), c (z = -0.50, across the 0.25 m discharge tube). Two facets each.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
plane() { # name z half
  cat > "$here/$1.stl" <<EOT
solid $1
  facet normal 0 0 1
    outer loop
      vertex -$3 -$3 $2
      vertex $3 -$3 $2
      vertex $3 $3 $2
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex -$3 -$3 $2
      vertex $3 $3 $2
      vertex -$3 $3 $2
    endloop
  endfacet
endsolid $1
EOT
}
plane htr10_plane_a -0.02 1.0
plane htr10_plane_b -0.10 1.0
plane htr10_plane_c -0.50 0.3
