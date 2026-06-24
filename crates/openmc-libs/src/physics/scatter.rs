// Elastic and inelastic neutron scattering.
//
// C++ source: `src/physics_common.cpp`, `src/physics.cpp`.
//
// Elastic scattering (MT=2):
//   - Centre-of-mass frame rotation by polar angle θ_cm
//   - Azimuthal angle φ sampled uniformly in [0, 2π)
//   - Energy transfer: E' = E · [(A·cos(θ_cm) + 1)² + sin²(θ_cm)] / (A+1)²
//   where A = atomic weight ratio.
//
// Inelastic scattering (MT=51–90):
//   - Secondary energy/angle from tabulated distributions (ENDF law 4/44/61)
//
// TODO: port both from `physics_common.cpp`.
