//! Incompressible-fluid properties — the CoolProp `INCOMP` backend.
//!
//! The evaluation engine (this module) is real and verified against T66
//! (Therminol 66, a pure heat-transfer oil) — the first fluid ported.
//! **Only one fluid's coefficient data is ported so far** (bead op-kbc.15
//! follow-up: the other 125 CoolProp incompressible fluids, via a codegen
//! script mirroring `dev/gen_fluid.py`).
//!
//! # What this models
//!
//! CoolProp's incompressible backend is **not** a Helmholtz EOS. Each fluid's
//! properties (density, isobaric heat capacity, thermal conductivity, dynamic
//! viscosity) are **direct fits** in temperature `T` and, for solutions, a
//! composition variable `x` (mass or volume fraction) — see
//! [`PropertyForm`] for the fit forms this port supports. Pressure is (almost)
//! a free variable — the fluid is incompressible — so the natural inputs are
//! `(T, p)` and, for brines, `(T, p, x)`.
//!
//! Three CoolProp sub-kinds (see [`IncompressibleKind`]):
//! - **pure** incompressible liquids (`T`-only fits),
//! - **mass-based** solutions/brines (`(T, x)` fits, `x` = mass fraction),
//! - **volume-based** solutions/brines (`(T, x)` fits, `x` = volume fraction).
//!
//! # Fit forms ([`PropertyForm`])
//!
//! CoolProp's `IncompressibleFluid` supports five fit forms
//! (`IncompressibleFluid.cpp`); this port implements the two most common
//! (`polynomial`, `exppolynomial`, `exponential` — used by T66's `density`/
//! `specific_heat`/`conductivity` and `viscosity` respectively) and returns
//! [`IncompressibleError::UnsupportedFitForm`] for `logexponential` and
//! `polyoffset` (present in some CoolProp fluids' viscosity fits, not yet
//! ported) — never a wrong number.
//!
//! # Units
//!
//! Mass-based SI: `T` \[K\], `p` \[Pa\], density \[kg/m³\], `c` \[J/(kg·K)\],
//! `λ` \[W/(m·K)\], `μ` \[Pa·s\].

pub mod fluids;

/// The set of incompressible fluids ported from CoolProp's `INCOMP` backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incompressible {
    /// Therminol 66 — a pure synthetic heat-transfer oil (`IncompressibleKind::Pure`).
    T66,
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

/// The fit form of one property (CoolProp `IncompressibleData::type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyForm {
    /// `Σ_ij c_ij·(T−T_base)^i·(x−x_base)^j`. CoolProp `polynomial`.
    Polynomial,
    /// `exp(Σ_ij c_ij·(T−T_base)^i·(x−x_base)^j)`. CoolProp `exppolynomial`.
    ExpPolynomial,
    /// `exp(c₀/((T−T_base)+c₁) − c₂)` — a 3-coefficient hyperbolic-in-`T` fit,
    /// `x`-independent (CoolProp `exponential`; the `coeffs` field carries
    /// exactly one row of 3 values `[c₀, c₁, c₂]`).
    Exponential,
}

/// One property's fit: form plus its coefficient table.
///
/// For [`PropertyForm::Polynomial`]/[`PropertyForm::ExpPolynomial`],
/// `coeffs[i][j]` is the coefficient of `(T−T_base)^i·(x−x_base)^j` (row `i` =
/// `T`-power, column `j` = `x`-power; for [`IncompressibleKind::Pure`] every
/// row has exactly one column). For [`PropertyForm::Exponential`], `coeffs` is
/// a single row of exactly 3 values.
#[derive(Debug, Clone, Copy)]
pub struct PropertyFit {
    /// Fit form.
    pub form: PropertyForm,
    /// Coefficient table — see the form-specific layout above.
    pub coeffs: &'static [&'static [f64]],
}

/// The polynomial-fit coefficient block for one incompressible fluid.
///
/// All `&'static`, so fluid definitions are `const` — same hardcoded-data
/// discipline as [`crate::fluids`].
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
    /// Temperature centering `T_base` \[K\] (CoolProp `Tbase`) — fits are in
    /// powers of `(T − T_base)`, not `T` directly.
    pub t_base: f64,
    /// Composition centering `x_base` \[-\] (CoolProp `xbase`).
    pub x_base: f64,
    /// Density fit `ρ(T, x)` \[kg/m³\].
    pub density: PropertyFit,
    /// Isobaric heat-capacity fit `c(T, x)` \[J/(kg·K)\].
    pub heat_capacity: PropertyFit,
    /// Thermal-conductivity fit `λ(T, x)` \[W/(m·K)\].
    pub conductivity: PropertyFit,
    /// Dynamic-viscosity fit `μ(T, x)` \[Pa·s\] (often
    /// [`PropertyForm::Exponential`], not `ln μ` as a polynomial — see CoolProp
    /// `IncompressibleFluid::visc`).
    pub viscosity: PropertyFit,
}

/// Failure modes of an incompressible-property query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompressibleError {
    /// `T` or `x` outside the fluid's fitted range.
    OutOfRange,
    /// A composition `x` was supplied for a [`IncompressibleKind::Pure`] fluid,
    /// or omitted for a solution.
    CompositionMismatch,
    /// The property's [`PropertyForm`] is not implemented yet (`logexponential`,
    /// `polyoffset`) — never a wrong number.
    UnsupportedFitForm,
}

impl Incompressible {
    /// The hardcoded coefficient block for this fluid.
    pub fn data(self) -> &'static IncompressibleFluid {
        match self {
            Incompressible::T66 => &fluids::T66_INCOMP,
        }
    }

    /// Validate `(t, x)` against the fluid's range and composition kind.
    fn check_inputs(self, t: f64, x: f64) -> Result<(), IncompressibleError> {
        let data = self.data();
        if !(t.is_finite() && t >= data.t_range.0 && t <= data.t_range.1) {
            return Err(IncompressibleError::OutOfRange);
        }
        match data.kind {
            IncompressibleKind::Pure => {
                if x != 0.0 {
                    return Err(IncompressibleError::CompositionMismatch);
                }
            }
            IncompressibleKind::MassBased | IncompressibleKind::VolumeBased => {
                if !(x.is_finite() && x >= data.x_range.0 && x <= data.x_range.1) {
                    return Err(IncompressibleError::OutOfRange);
                }
            }
        }
        Ok(())
    }

    /// Mass density \[kg/m³\] at temperature `t` \[K\] and composition `x`
    /// \[-\] (must be `0.0` for pure fluids). `p` is accepted for API
    /// symmetry with the rest of the crate but unused — incompressible
    /// properties do not depend on pressure in this fit family.
    pub fn density(self, t: f64, _p: f64, x: f64) -> Result<f64, IncompressibleError> {
        self.check_inputs(t, x)?;
        let data = self.data();
        eval_fit(&data.density, t, x, data.t_base, data.x_base)
    }

    /// Isobaric specific heat `c_p` \[J/(kg·K)\].
    pub fn heat_capacity(self, t: f64, _p: f64, x: f64) -> Result<f64, IncompressibleError> {
        self.check_inputs(t, x)?;
        let data = self.data();
        eval_fit(&data.heat_capacity, t, x, data.t_base, data.x_base)
    }

    /// Thermal conductivity `λ` \[W/(m·K)\].
    pub fn conductivity(self, t: f64, _p: f64, x: f64) -> Result<f64, IncompressibleError> {
        self.check_inputs(t, x)?;
        let data = self.data();
        eval_fit(&data.conductivity, t, x, data.t_base, data.x_base)
    }

    /// Dynamic viscosity `μ` \[Pa·s\].
    pub fn viscosity(self, t: f64, _p: f64, x: f64) -> Result<f64, IncompressibleError> {
        self.check_inputs(t, x)?;
        let data = self.data();
        eval_fit(&data.viscosity, t, x, data.t_base, data.x_base)
    }

    /// Specific enthalpy \[J/kg\], `h(T) = ∫_{T_base}^{T} c_p(T', x) dT'` (zero
    /// at `T = T_base`, `x` fixed — an internally-consistent reference, not
    /// necessarily CoolProp's absolute convention). Only implemented for a
    /// [`PropertyForm::Polynomial`] heat-capacity fit (the common case,
    /// including T66); other forms return
    /// [`IncompressibleError::UnsupportedFitForm`].
    pub fn enthalpy(self, t: f64, _p: f64, x: f64) -> Result<f64, IncompressibleError> {
        self.check_inputs(t, x)?;
        let data = self.data();
        if data.heat_capacity.form != PropertyForm::Polynomial {
            return Err(IncompressibleError::UnsupportedFitForm);
        }
        Ok(poly2d_integral_dt(data.heat_capacity.coeffs, t - data.t_base, x - data.x_base))
    }

    /// Temperature \[K\] from specific enthalpy `h` \[J/kg\] — the inverse of
    /// [`Self::enthalpy`], via Newton on `c_p = dh/dT`.
    pub fn temperature_from_enthalpy(self, h: f64, p: f64, x: f64) -> Result<f64, IncompressibleError> {
        let data = self.data();
        if data.heat_capacity.form != PropertyForm::Polynomial {
            return Err(IncompressibleError::UnsupportedFitForm);
        }
        let mut t = 0.5 * (data.t_range.0 + data.t_range.1);
        for _ in 0..50 {
            let h_t = self.enthalpy(t, p, x)?;
            let cp = self.heat_capacity(t, p, x)?;
            if cp <= 0.0 {
                return Err(IncompressibleError::OutOfRange);
            }
            let step = (h_t - h) / cp;
            t -= step;
            if step.abs() < 1e-9 * t.abs().max(1.0) {
                return Ok(t);
            }
        }
        Err(IncompressibleError::OutOfRange)
    }
}

/// Antiderivative in `T` of the 2-D polynomial `poly2d` (see its doc),
/// evaluated from `0` to `dt` at fixed `dx` — i.e.
/// `∫_0^{dt} Σ_ij c_ij·t'^i·dx^j dt' = Σ_ij c_ij/(i+1)·dt^{i+1}·dx^j`.
fn poly2d_integral_dt(coeffs: &[&[f64]], dt: f64, dx: f64) -> f64 {
    fn horner(row: &[f64], dx: f64) -> f64 {
        row.iter().rev().fold(0.0, |acc, &c| acc * dx + c)
    }
    coeffs
        .iter()
        .enumerate()
        .map(|(i, row)| horner(row, dx) / (i as f64 + 1.0) * dt.powi(i as i32 + 1))
        .sum()
}

/// Evaluate a 2-D property polynomial `Σ_ij c_ij·(T−T_base)^i·(x−x_base)^j`
/// (or its `exp`/hyperbolic variants, see [`PropertyForm`]) at `(t, x)`.
fn eval_fit(fit: &PropertyFit, t: f64, x: f64, t_base: f64, x_base: f64) -> Result<f64, IncompressibleError> {
    match fit.form {
        PropertyForm::Polynomial => Ok(poly2d(fit.coeffs, t - t_base, x - x_base)),
        PropertyForm::ExpPolynomial => Ok(poly2d(fit.coeffs, t - t_base, x - x_base).exp()),
        PropertyForm::Exponential => {
            // exp(c0/(T+c1) - c2); CoolProp::IncompressibleFluid::baseExponential,
            // called with ybase=0.0 (NOT Tbase) for this fit form -- confirmed
            // against IncompressibleFluid.cpp's visc()/rho()/cond() dispatch
            // (`baseExponential(data, T, 0.0)`), unlike the Polynomial/
            // ExpPolynomial forms, which do centre on Tbase. The near-zero-
            // denominator linearisation guard is omitted -- no ported fluid's
            // fitted range crosses that singularity.
            let row = fit.coeffs[0];
            let (c0, c1, c2) = (row[0], row[1], row[2]);
            Ok((c0 / (t + c1) - c2).exp())
        }
    }
}

/// Horner evaluation of `Σ_ij c_ij·dt^i·dx^j`, `dt = T-T_base`, `dx = x-x_base`
/// — outer Horner over rows (`T`-power) of the inner row's Horner-in-`dx`
/// value, matching CoolProp `Polynomial2DFrac::evaluate` with `x_exp=y_exp=0`.
fn poly2d(coeffs: &[&[f64]], dt: f64, dx: f64) -> f64 {
    fn horner(row: &[f64], dx: f64) -> f64 {
        row.iter().rev().fold(0.0, |acc, &c| acc * dx + c)
    }
    coeffs.iter().rev().fold(0.0, |acc, row| acc * dt + horner(row, dx))
}
