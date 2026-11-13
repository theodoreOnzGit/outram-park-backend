#!/bin/sh
# Generates reference-data/liggghts/in.massflow_plate (gh:#1416).
out=$1
cat > "$out" <<'EOF'
# fix massflow/mesh (gh:#1416): 12 spheres (no mutual contact) cross the 0.3 m square
# plate_square.stl (z = 0, normal +-z) while bouncing on a floor wall at z = -0.12.
# mfu counts upward crossings (count multiple), mfd downward ones (count once).
atom_style	granular
atom_modify	map array
boundary	f f f
newton		off
communicate	single vel yes
units		si
region		reg block -0.4 0.4 -0.4 0.4 -0.2 0.4 units box
create_box	1 reg
neighbor	0.003 bin
neigh_modify	delay 0
fix 		m1 all property/global youngsModulus peratomtype 1.0e7
fix 		m2 all property/global poissonsRatio peratomtype 0.3
fix 		m3 all property/global coefficientRestitution peratomtypepair 1 0.9
fix 		m4 all property/global coefficientFriction peratomtypepair 1 0.5
pair_style	gran model hertz tangential history
pair_coeff	* *
timestep	1e-5
fix		plate all mesh/surface file plate_square.stl type 1
fix		floor all wall/gran model hertz tangential history primitive type 1 zplane -0.12
EOF
n=0
for z in 0.06 0.03 -0.05; do
  for x in -0.2 -0.08 0.04 0.16; do
    n=$((n+1))
    y=$(awk -v n=$n 'BEGIN{printf "%.3f", -0.05+0.01*n}')
    echo "create_atoms	1 single $x $y $z units box" >> "$out"
  done
done
n=0
for z in 0.06 0.03 -0.05; do
  for x in -0.2 -0.08 0.04 0.16; do
    n=$((n+1))
    vz=$(awk -v n=$n -v z=$z 'BEGIN{ if (z<0) printf "%.2f", 1.2+0.1*n; else printf "%.2f", -0.4-0.1*n }')
    vx=$(awk -v n=$n 'BEGIN{printf "%.2f", 0.05*(n%3)-0.05}')
    echo "set		atom $n diameter 0.01 density 2500 vx $vx vz $vz" >> "$out"
  done
done
cat >> "$out" <<'EOF'
fix		grav all gravity 9.81 vector 0 0 -1
fix		integr all nve/sphere
fix		mfu all massflow/mesh mesh plate vec_side 0 0 1 count multiple
fix		mfd all massflow/mesh mesh plate vec_side 0 0 -1 count once
thermo_style	custom step f_mfu[1] f_mfu[2] f_mfu[3] f_mfu[4] f_mfd[1] f_mfd[2] f_mfd[3] f_mfd[4]
thermo_modify	lost ignore norm no format float %.17g
thermo		500
dump		d1 all custom 500 dump.massflow_plate id x y z vx vy vz omegax omegay omegaz fx fy fz
run		60000
EOF
