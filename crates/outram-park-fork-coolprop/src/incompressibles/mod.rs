//! Incompressible-fluid properties — the CoolProp `INCOMP` backend.
//!
//! **Scaffold only (bead op-kbc.15).** Types and signatures are laid out; the
//! bodies are `todo!()`. Nothing here is wired into the crate's public API yet.
//!
//! # What this models
//!
//! CoolProp's incompressible backend is **not** a Helmholtz EOS. Each fluid's
//! properties (density, isobaric heat capacity, thermal conductivity, dynamic
//! viscosity, and the saturation pressure) are **direct polynomial fits** in
//! temperature `T` and, for solutions, a composition variable `x` (mass or
//! volume fraction). Pressure is (almost) a free variable — the fluid is
//! incompressible — so the natural inputs are `(T, p)` and, for brines,
//! `(T, p, x)`.
//!
//! Three CoolProp sub-kinds (see [`IncompressibleKind`]):
//! - **pure** incompressible liquids (`T`-only polynomials),
//! - **mass-based** solutions/brines (`(T, x)` polynomials, `x` = mass frac),
//! - **volume-based** solutions/brines (`(T, x)` polynomials, `x` = vol frac).
//!
//! # Units
//!
//! Mass-based SI: `T` \[K\], `p` \[Pa\], density \[kg/m³\], `c` \[J/(kg·K)\],
//! `λ` \[W/(m·K)\], `μ` \[Pa·s\], enthalpy/entropy relative to each fluid's
//! reference state.
#![allow(dead_code, unused_variables, unused_imports)] // TODO(op-kbc.15): drop once implemented

pub mod fluids;

/// The set of incompressible fluids ported from CoolProp's `INCOMP` backend.
///
/// Populated by the codegen follow-up (bead op-kbc.15); this scaffold pins two
/// representative variants — a pure liquid and a brine — so the dispatch shape
/// is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incompressible {
    /// Downtherm-Q-style pure heat-transfer oil (pure, `T`-only). Placeholder.
    ExampleThermalOil,
    /// Aqueous ethylene-glycol brine (mass-based solution, `(T, x)`). Placeholder.
    ExampleEthyleneGlycol,
}

/// Which polynomial family a fluid uses — fixes the meaning of the `x` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompressibleKind {
    /// Pure liquid: coefficients are `T`-only (`x` is ignored / must be 0).
    Pure,
    /// Solution keyed by **mass** fraction `x` \[kg solute / kg solution\].
    MassBased,
    /// Solution keyed by **volume** fraction `x` \[-\].
    VolumeBased,
}

/// The polynomial-fit coefficient block for one incompressible fluid.
///
/// Each property is a 2-D polynomial `Σ c_ij · T^i · x^j` (for [`IncompressibleKind::Pure`]
/// only the `j = 0` column is present). All `&'static`, so fluid definitions are
/// `const` — same hardcoded-data discipline as [`crate::fluids`].
#[derive(Debug, Clone, Copy)]
pub struct IncompressibleFluid {
    /// Fluid name (as in CoolProp).
    pub name: &'static str,
    /// Polynomial family / meaning of `x`.
    pub kind: IncompressibleKind,
    /// Valid temperature range \[K\] (`t_min`, `t_max`).
    pub t_range: (f64, f64),
    /// Valid composition range \[-\] (`x_min`, `x_max`); `(0.0, 0.0)` when pure.
    pub x_range: (f64, f64),
    /// Density coefficients `ρ(T, x)` \[kg/m³\], row-major `c_ij`.
    pub density: &'static [&'static [f64]],
    /// Isobaric heat-capacity coefficients `c(T, x)` \[J/(kg·K)\].
    pub heat_capacity: &'static [&'static [f64]],
    /// Thermal-conductivity coefficients `λ(T, x)` \[W/(m·K)\].
    pub conductivity: &'static [&'static [f64]],
    /// Dynamic-viscosity coefficients — CoolProp fits `ln(μ)` \[ln Pa·s\].
    pub log_viscosity: &'static [&'static [f64]],
    /// Saturation-pressure coefficients `ln(p_sat(T, x))` \[ln Pa\] (may be empty).
    pub log_psat: &'static [&'static [f64]],
}

/// Failure modes of an incompressible-property query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompressibleError {
    /// `T` or `x` outside the fluid's fitted range.
    OutOfRange,
    /// A composition `x` was supplied for a [`IncompressibleKind::Pure`] fluid,
    /// or omitted for a solution.
    CompositionMismatch,
    /// An inverse solve (`T` from `h`/`s`) did not converge.
    NonConvergent,
}

impl Incompressible {
    /// The hardcoded coefficient block for this fluid.
    pub fn data(self) -> &'static IncompressibleFluid {
        // TODO(op-kbc.15): match self -> &fluids::<NAME>_INCOMP (codegen).
        todo!("op-kbc.15: incompressible fluid data dispatch")
    }

    /// Mass density \[kg/m³\] at temperature `t` \[K\], pressure `p` \[Pa\] and
    /// composition `x` \[-\] (ignored for pure fluids).
    pub fn density(self, t: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: ρ(T, x) polynomial eval")
    }

    /// Isobaric specific heat `c_p` \[J/(kg·K)\].
    pub fn heat_capacity(self, t: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: c(T, x) polynomial eval")
    }

    /// Thermal conductivity `λ` \[W/(m·K)\].
    pub fn conductivity(self, t: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: λ(T, x) polynomial eval")
    }

    /// Dynamic viscosity `μ` \[Pa·s\] (CoolProp fits `ln μ`).
    pub fn viscosity(self, t: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: exp(ln μ(T, x)) polynomial eval")
    }

    /// Specific enthalpy \[J/kg\] relative to the fluid reference state — the
    /// `∫ c_p dT` integral of the heat-capacity polynomial (plus the `∫ v dp`
    /// term).
    pub fn enthalpy(self, t: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: h = ∫c dT + ∫v dp")
    }

    /// Temperature \[K\] from specific enthalpy `h` \[J/kg\] — the inverse of
    /// [`Self::enthalpy`] (Newton on `c_p`).
    pub fn temperature_from_enthalpy(
        self,
        h: f64,
        p: f64,
        x: f64,
    ) -> Result<f64, IncompressibleError> {
        todo!("op-kbc.15: invert h(T) via Newton on c_p")
    }
}

/// Evaluate a 2-D property polynomial `Σ_ij c_ij · T^i · x^j` at `(t, x)`.
///
/// The shared kernel every property getter calls; `coeffs[i][j] = c_ij`.
fn poly2d(coeffs: &[&[f64]], t: f64, x: f64) -> f64 {
    // TODO(op-kbc.15): Horner in T for each x-column, then Horner in x. Match
    // CoolProp's base-value shift (fits are usually centred on a T/x offset).
    todo!("op-kbc.15: 2-D polynomial evaluation")
}
