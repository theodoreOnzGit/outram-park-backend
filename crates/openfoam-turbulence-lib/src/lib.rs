// openfoam-turbulence-lib
//
// Pure-Rust port of the OpenFOAM turbulence model library.
// C++ source reference: src/TurbulenceModels/ in the OpenFOAM source tree.
//
// Planned modules (not yet implemented):
//   laminar          — LaminarModel (no-op; zero turbulent stresses)
//   k_epsilon        — Standard two-equation k-ε (Jones & Launder 1972)
//   k_omega          — Standard two-equation k-ω (Wilcox 1988)
//   k_omega_sst      — Menter's k-ω SST (1994); default for wall-bounded flows
//   spalart_allmaras — One-equation Spalart-Allmaras (1992); aerospace use
//   les_smagorinsky  — LES Smagorinsky sub-grid model
//   wall_functions   — Wall-function boundary conditions (yPlus, nutWallFunction, …)
