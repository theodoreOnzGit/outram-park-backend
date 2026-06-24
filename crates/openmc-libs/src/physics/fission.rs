// Fission neutron production.
//
// C++ source: `src/physics.cpp` — `fission()`, `create_fission_sites()`.
//
// For each fission collision:
//   1. Sample number of secondary neutrons: ν from a `NuDistribution`
//      (integer part deterministic; fractional part by comparison with ξ)
//   2. For each secondary neutron:
//      a. Sample energy from the fission spectrum (Watt or tabulated)
//      b. Sample direction (isotropic in lab frame for prompt fission)
//   3. Push secondary sites to the fission bank for the next generation
//
// Delayed neutrons:
//   - OpenMC tracks delayed group fractions (β_i) and decay constants (λ_i)
//   - For eigenvalue problems, delayed neutrons are combined with prompt ones
//     and all treated as prompt (using the total ν)
//
// TODO: port from `physics.cpp::fission()` and `create_fission_sites()`.
