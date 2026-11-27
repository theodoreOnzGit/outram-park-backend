# 2D Cavity Boussinesq

## Description

This test case portrays the use of the Boussinesq feature. The Boussinesq
approximation is used is buoyancy driven flows and assumes that the effects
of temperature-induced density changes are relevant ONLY in the calculation
of the buoyancy momentum source terms, while in all other terms of the
momentum equation the density is kept constant. By selecting the
Boussinesq equationOfState type in thermophysical properties, and providing
the parameters rho0, beta and T0, the code employs the Boussinesq
approximation as it was originally intended to, i.e. rho = rho0 everywhere
except in the buoyancy term, which is computed as a density gradient with
rho = rho0*(1-beta*(T-T0)). While the Boussinesq equationOfState is a standard
OpenFOAM model, the actual implementation that is consistent with the
approximation (i.e. that uses rho0 in place of rho where needed) is specific
to this solver.

In this case, the domain consists of a square with 10 cm sides and two opposing
walls maintained at 500 K and 1000 K, with the fluid starting at rest and at
500 K. The cooled and heated wall patch normals are perpendicular to gravity,
while gravity acts in the positive X direction. The fluid has thermophysical
properties comparable to those of liquid sodium at 500 K. A steady state is
reached with the establishment of a circular liquid motion so that it rises
against gravity at the heated wall and descends at the cooled wall.
