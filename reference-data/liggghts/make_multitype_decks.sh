#!/bin/sh
# Generate the two-type decks of gh:#1410 (multi-type property tables for the
# newer contact models). Same pair as in.twotype_oblique (type 1: d=10 mm,
# rho 2500; type 2: d=12 mm, rho 4000; relative spins) with per-type-pair
# coefficient matrices. Usage: ./make_multitype_decks.sh  (writes in.mt_*).
set -eu
cd "$(dirname "$0")"
head_common() { # $1 = comment
cat <<EOT
# $1
atom_style	granular
atom_modify	map array
boundary	f f f
newton		off
communicate	single vel yes
units		si
region		reg block -0.2 0.2 -0.2 0.2 -0.2 0.2 units box
create_box	2 reg
neighbor	0.004 bin
neigh_modify	delay 0
fix 		m1 all property/global youngsModulus peratomtype 1.0e7 3.0e7
fix 		m2 all property/global poissonsRatio peratomtype 0.3 0.2
fix 		m3 all property/global coefficientRestitution peratomtypepair 2 0.9 0.7 0.7 0.5
fix 		m4 all property/global coefficientFriction peratomtypepair 2 0.5 0.3 0.3 0.2
EOT
}
tail_common() { # $1 = name, $2 = extra set options
cat <<EOT
pair_coeff	* *
timestep	1e-6
create_atoms	1 single -0.0060 0.0 0.0 units box
create_atoms	2 single  0.0060 0.0 0.0 units box
set		atom 1 diameter 0.01 density 2500 vx  1.0 vy  0.5 omegaz 40.0
set		atom 2 diameter 0.012 density 4000 vx -1.0 vy -0.5 omegaz -20.0
fix		integr all nve/sphere
thermo		1000
thermo_modify	lost ignore norm no
dump		d1 all custom 10 dump.$1 id x y z vx vy vz omegax omegay omegaz fx fy fz
run		2500
EOT
}
# mk <name> <comment> ; model lines on stdin
mk() { n=$1; c=$2; body=$(cat); { head_common "$c"; printf '%s\n' "$body"; tail_common "$n"; } > "in.$n"; }
mk mt_epsd "two-type EPSD rolling (gh:#1410): per-pair rolling friction and viscous damping." <<'EOT'
fix 		m5 all property/global coefficientRollingFriction peratomtypepair 2 0.1 0.15 0.15 0.2
fix 		m7 all property/global coefficientRollingViscousDamping peratomtypepair 2 0.3 0.4 0.4 0.5
fix 		m6 all property/global characteristicVelocity scalar 2.0
pair_style	gran model hooke tangential history rolling_friction epsd
EOT
mk mt_epsd2 "two-type EPSD2 rolling (gh:#1410)." <<'EOT'
fix 		m5 all property/global coefficientRollingFriction peratomtypepair 2 0.1 0.15 0.15 0.2
fix 		m6 all property/global characteristicVelocity scalar 2.0
pair_style	gran model hooke tangential history rolling_friction epsd2
EOT
mk mt_epsd3 "two-type EPSD3 rolling (gh:#1410)." <<'EOT'
fix 		m5 all property/global coefficientRollingFriction peratomtypepair 2 0.1 0.15 0.15 0.2
fix 		m7 all property/global coefficientRollingViscousDamping peratomtypepair 2 0.3 0.4 0.4 0.5
fix 		m8 all property/global rollingStiffness scalar 5.0
fix 		m6 all property/global characteristicVelocity scalar 2.0
pair_style	gran model hooke tangential history rolling_friction epsd3
EOT
mk mt_luding "two-type luding + tan_luding + rolling luding (gh:#1410): every coefficient a per-pair matrix." <<'EOT'
fix 		p0 all property/global LoadingStiffness peratomtypepair 2 1.0e4 1.5e4 1.5e4 2.0e4
fix 		p1 all property/global UnloadingStiffness peratomtypepair 2 3.0 2.5 2.5 2.0
fix 		p2 all property/global coefficientAdhesionStiffness peratomtypepair 2 0.4 0.3 0.3 0.2
fix 		p3 all property/global coefficientPlasticityDepth peratomtypepair 2 0.05 0.04 0.04 0.03
fix 		p4 all property/global pullOffForce peratomtypepair 2 0.001 0.002 0.002 0.003
fix 		p5 all property/global FluidViscosity peratomtypepair 2 0.8 0.7 0.7 0.6
fix 		p6 all property/global FrictionViscosity peratomtypepair 2 0.6 0.5 0.5 0.4
fix 		p7 all property/global coeffFrictionStiffness peratomtypepair 2 0.5 0.6 0.6 0.7
fix 		p8 all property/global coefficientRollingFriction peratomtypepair 2 0.1 0.15 0.15 0.2
fix 		p9 all property/global coefficientRollingViscousDamping peratomtypepair 2 0.3 0.4 0.4 0.5
fix 		p10 all property/global coeffRollingStiffness peratomtypepair 2 0.5 0.6 0.6 0.7
pair_style	gran model luding tangential tan_luding rolling_friction luding
EOT
mk mt_tn "two-type thornton_ning (gh:#1410): per-pair surface energy, per-type yield ratio." <<'EOT'
fix 		p0 all property/global surfaceEnergy peratomtypepair 2 0.05 0.04 0.04 0.03
fix 		p1 all property/global coefficientYieldRatio peratomtype 0.05 0.08
pair_style	gran model thornton_ning tangential history
EOT
mk mt_edi "two-type edinburgh, nonlinear, fixKc off (gh:#1410): per-pair k2/k1, kc/kn, f_0, surface energy." <<'EOT'
fix 		p0 all property/global UnloadingStiffness peratomtypepair 2 3.0 2.5 2.5 2.0
fix 		p1 all property/global coefficientAdhesionStiffness peratomtypepair 2 0.3 0.25 0.25 0.2
fix 		p2 all property/global adhesionExponent scalar 1.5
fix 		p3 all property/global overlapExponent scalar 1.5
fix 		p4 all property/global pullOffForce peratomtypepair 2 0.0 0.001 0.001 0.002
fix 		p5 all property/global surfaceEnergy peratomtypepair 2 0.05 0.04 0.04 0.03
pair_style	gran model edinburgh tangential history
EOT
mk mt_sjkr "two-type cohesion sjkr (gh:#1410): per-pair cohesion energy density." <<'EOT'
fix 		p0 all property/global cohesionEnergyDensity peratomtypepair 2 2.0e5 3.0e5 3.0e5 4.0e5
pair_style	gran model hertz tangential history cohesion sjkr
EOT
mk mt_sjkr2 "two-type cohesion sjkr2 (gh:#1410)." <<'EOT'
fix 		p0 all property/global cohesionEnergyDensity peratomtypepair 2 2.0e5 3.0e5 3.0e5 4.0e5
pair_style	gran model hertz tangential history cohesion sjkr2
EOT
