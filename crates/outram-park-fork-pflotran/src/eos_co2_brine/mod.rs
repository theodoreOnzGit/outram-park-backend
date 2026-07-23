//! Real-fluid equations of state for **supercritical/gaseous CO2** and **NaCl
//! brine** — the fluid pair needed for GENERAL-mode, geologic-CO2-sequestration
//! (GCS) simulations (bead op-1y6).
//!
//! This module extends [`crate::eos_real`] (IAPWS-IF97 pure liquid water) toward
//! the two-fluid, two-component system a CO2-storage model needs: a CO2-rich
//! non-wetting phase and an NaCl-brine aqueous phase. It provides density and
//! viscosity for each, as plain-`f64` SI (or documented per-method) inputs.
//!
//! # ⚠️ ACCURACY WARNING — READ THIS FIRST ⚠️
//!
//! **The CO2 density here uses the Redlich-Kwong (RK) cubic EOS, which is only a
//! rough, low-order approximation to the Span & Wagner (1996) reference EOS that
//! NIST/REFPROP and the real PFLOTRAN CO2 modes use.** In the dense
//! near-critical / supercritical region that matters most for GCS, **RK
//! under-predicts CO2 density by ~10-15%** (measured: ~-14% at 40 °C, 10 MPa,
//! see the tests). Do **not** treat RK CO2 density as quantitatively reliable
//! for storage-capacity, buoyancy, or plume-migration estimates. It is a
//! placeholder with the correct qualitative behaviour (monotonic in P and T,
//! ideal-gas limit) until a Span-Wagner Helmholtz-EOS port lands.
//!
//! **The CO2 viscosity is the Fenghour-Wakeham-Vesovic (1998) *zero-density*
//! (dilute-gas) term only** — it depends on temperature alone and **omits the
//! excess (density-dependent) contribution entirely**, so it materially
//! under-predicts the viscosity of dense supercritical CO2. Use it only near the
//! dilute-gas limit; flag every dense-phase use as approximate.
//!
//! The brine density/viscosity use the Batzle & Wang (1992) correlations, which
//! are empirical fits valid over the stated ranges (below) and are the same
//! order of fidelity used in exploration geophysics — reasonable, but still an
//! empirical correlation, not a reference EOS.
//!
//! Per the workspace `RESPONSIBLE_USE.md`, **everything here is untrusted
//! AI-generated draft** until a human reviews it and validates it against
//! published CO2-brine reference data. No human V&V has been performed.
//!
//! # Units at the API boundary
//!
//! Following the PFLOTRAN-fork convention, the public methods take and return
//! plain `f64`, but the **unit differs per fluid** to match each correlation's
//! natural units — read each signature:
//!
//! - [`Co2Properties`] takes **temperature in kelvin (K)** and **pressure in
//!   pascal (Pa)**; returns density in kg/m^3, viscosity in Pa.s, `Z`
//!   dimensionless.
//! - [`Brine`] takes **temperature in degrees Celsius (°C)** and **pressure in
//!   megapascal (MPa)** — the native units of the Batzle-Wang correlation —
//!   and returns density in kg/m^3, viscosity in Pa.s.
//!
//! # Valid ranges (inputs outside these are rejected, not extrapolated)
//!
//! - **CO2 (RK):** `T > 0 K`, `0 < P <= 500 MPa`, both finite. RK is a global
//!   cubic so it *returns* a value anywhere in that box, but its accuracy is
//!   only "order-of-magnitude to ~15%" as warned above, worst near the critical
//!   point (Tc = 304.13 K, Pc = 7.3773 MPa).
//! - **Brine (Batzle-Wang):** `0 °C <= T <= 350 °C`, `0 < P <= 100 MPa`, NaCl
//!   mass fraction `0 <= w <= 0.26` (0.26 ≈ halite saturation near ambient).
//!   The correlation was fit over roughly 20-350 °C, up to ~100 MPa, and NaCl
//!   up to saturation.
//!
//! # Provenance / references
//!
//! - Redlich, O. & Kwong, J.N.S. (1949). *On the Thermodynamics of Solutions.
//!   V. An Equation of State. Fugacities of Gaseous Solutions.* Chem. Rev.
//!   44(1), 233-244. — the cubic EOS used for CO2 density.
//! - Span, R. & Wagner, W. (1996). *A New Equation of State for Carbon Dioxide
//!   Covering the Fluid Region from the Triple-Point Temperature to 1100 K at
//!   Pressures up to 800 MPa.* J. Phys. Chem. Ref. Data 25(6), 1509-1596. —
//!   the **reference** CO2 EOS that RK here only *approximates* (RK is not this).
//! - Fenghour, A., Wakeham, W.A. & Vesovic, V. (1998). *The Viscosity of Carbon
//!   Dioxide.* J. Phys. Chem. Ref. Data 27(1), 31-44. — CO2 viscosity; only the
//!   zero-density term is used here.
//! - Batzle, M. & Wang, Z. (1992). *Seismic properties of pore fluids.*
//!   Geophysics 57(11), 1396-1408. — brine density and brine viscosity
//!   correlations (Eqs. 27a, 27b and Eq. 29 of that paper).
//!
//! Higher-fidelity brine viscosity alternatives (Kestin et al. 1981; Mao & Duan
//! 2009) and CO2 solubility in brine (Duan & Sun 2003) are noted as follow-up
//! work (see the module TODO in [`co2`]/[`brine`]); they are **not** implemented
//! here.

pub mod brine;
pub mod co2;

pub use brine::Brine;
pub use co2::Co2Properties;
