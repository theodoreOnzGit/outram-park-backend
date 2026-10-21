#!/bin/sh
# Generate the HTR-10 bottom conus + fuel discharge tube as an ASCII STL, with
# INWARD-facing normals (the pebbles are inside).
#
# Published geometry, from docs/reactor-scoping/htr10-neutronics.md:
#   core radius            90 cm      (cone inlet)
#   height of conus        36.946 cm
#   discharge tube radius  25 cm
# The tube is extended TUBE_LEN below the cone to hold extracted pebbles; its
# real length (610 cm) is irrelevant to a defuelling simulation, which removes
# pebbles at the top of it. 25 cm holds ~250 pebbles, far more than one batch.
#
# Winding: Triangle{a,b,c} has normal normalize((b-a) x (c-a)) (mesh_wall.rs),
# so vertices are ordered to make that point toward the axis.
set -eu
OUT=${1:-htr10_discharge.stl}
N=${2:-120}          # circumferential segments
R_CORE=0.90
R_TUBE=0.25
H_CONE=0.36946
TUBE_LEN=0.25

awk -v n="$N" -v rc="$R_CORE" -v rt="$R_TUBE" -v hc="$H_CONE" -v tl="$TUBE_LEN" '
BEGIN {
  PI = 3.14159265358979323846
  print "solid htr10_discharge"
  # band 1: the conus, z = 0 (r = rc) down to z = -hc (r = rt)
  emit_band(0.0, rc, -hc, rt)
  # band 2: the discharge tube, constant radius
  emit_band(-hc, rt, -hc - tl, rt)
  print "endsolid htr10_discharge"
}
function emit_band(z0, r0, z1, r1,   i, a0, a1, p0x,p0y, p1x,p1y, q0x,q0y, q1x,q1y) {
  for (i = 0; i < n; i++) {
    a0 = 2*PI*i/n; a1 = 2*PI*(i+1)/n
    p0x = r0*cos(a0); p0y = r0*sin(a0)
    p1x = r0*cos(a1); p1y = r0*sin(a1)
    q0x = r1*cos(a0); q0y = r1*sin(a0)
    q1x = r1*cos(a1); q1y = r1*sin(a1)
    # both windings chosen so (b-a) x (c-a) points toward the axis
    tri(p0x,p0y,z0,  q1x,q1y,z1,  q0x,q0y,z1)
    tri(p0x,p0y,z0,  p1x,p1y,z0,  q1x,q1y,z1)
  }
}
function tri(ax,ay,az, bx,by,bz, cx,cy,cz) {
  printf "  facet normal 0 0 0\n    outer loop\n"
  printf "      vertex %.9g %.9g %.9g\n", ax, ay, az
  printf "      vertex %.9g %.9g %.9g\n", bx, by, bz
  printf "      vertex %.9g %.9g %.9g\n", cx, cy, cz
  printf "    endloop\n  endfacet\n"
}' > "$OUT"
echo "$OUT: $(grep -c 'facet normal' "$OUT") facets"
