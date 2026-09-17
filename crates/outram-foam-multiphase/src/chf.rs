// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// Unlike most files in this crate, `chf.rs` is NOT a port of OpenFOAM source.
// The critical-heat-flux correlations implemented here (Biasi 1967, Westinghouse
// W-3 / Tong 1967, Bowring 1972) and the Groeneveld look-up-table framework are
// standard PUBLIC-LITERATURE models; each item below cites the paper/report it
// implements. No OpenFOAM code was used or consulted for this file — only the
// open published correlations. The file is licensed GPL-3.0-only for workspace
// consistency; the correlations themselves are public-domain scientific results
// used only for education, research, and verification/validation (see the
// workspace `RESPONSIBLE_USE.md` / `DATA_POLICY.md`).
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Stage 4 — **Critical-heat-flux (CHF / DNB / dryout) correlations**
//! (bead `op-2kk.4`).
//!
//! Point (0-D) engineering correlations that predict the **critical heat flux**
//! `q''_c` `[W/m²]` — the wall heat flux at which the boiling-heat-transfer
//! mode degrades (departure from nucleate boiling at low quality, dryout at
//! high quality) and the wall temperature excursion begins — as a function of
//! local flow conditions. These are the closures a wall-boiling or two-fluid
//! solver queries to detect the CHF limit; they are *not* CFD themselves.
//!
//! ## Models implemented (all from the open literature)
//!
//! | Correlation | Regime | Source |
//! |---|---|---|
//! | [`Biasi`]  | round tube, local quality | Biasi et al. (1967) |
//! | [`W3`]     | PWR rod bundle / tube DNB, low quality + subcooling | Tong (1967) |
//! | [`Bowring`]| round tube, uniform flux, dryout | Bowring (1972) |
//! | [`GroeneveldLut`] | tabulated CHF(P, G, x) for an 8 mm tube | Groeneveld et al. (2007) |
//!
//! Runtime model selection uses the [`ChfCorrelation`] **enum** (no `dyn`
//! dispatch, per the workspace design rules), which forwards the [`ChfModel`]
//! contract to the selected concrete correlation.
//!
//! ## Units and the API boundary
//!
//! Every public entry point takes and returns `uom`-typed physical quantities so
//! the dimensions are checked at the boundary:
//!
//! - pressure `P` — [`Pressure`] `[Pa]`
//! - mass flux `G` — [`MassFlux`] `[kg·m⁻²·s⁻¹]`
//! - thermodynamic equilibrium quality `x` — `f64`, dimensionless `[-]`
//! - (heated / hydraulic) diameter `D` — [`Length`] `[m]`
//! - result CHF `q''_c` — [`HeatFluxDensity`] `[W/m²]`
//!
//! Two correlations need one extra scalar that the four-argument [`ChfModel`]
//! signature does not carry, so it is stored on the model struct at
//! construction (also `uom`-typed):
//!
//! - [`W3`] needs the **inlet subcooling enthalpy** `Δh_in = h_f − h_in`
//!   `[J/kg]` ([`AvailableEnergy`]) for its subcooling factor.
//! - [`Bowring`] needs the **latent heat of vaporisation** `h_fg` `[J/kg]`
//!   ([`AvailableEnergy`]) at the system pressure (from steam tables — public
//!   literature data).
//!
//! ## Honest scope — verification, not validation
//!
//! The inline tests below are **verification** checks: each correlation is
//! evaluated against a **hand-computed reference point** (the full arithmetic is
//! written out in the test doc comment) to confirm the algebra is implemented
//! correctly, and the look-up-table interpolation is checked for node-exactness,
//! midpoint correctness, and CSV round-trip. **No benchmark validation against
//! experimental CHF databases has been performed here** — that is a later,
//! human-run V&V step. Do not read these correlations as validated for any
//! design or safety purpose (see the workspace `RESPONSIBLE_USE.md`: AI-assisted
//! output is untrusted draft material until human-reviewed).
//!
//! The stated pressure / mass-flux / quality / diameter validity range of each
//! correlation is documented on the model and exposed via
//! [`ChfModel::in_valid_range`]; evaluating **outside** that range extrapolates
//! (it is not auto-clamped) and the caller is responsible for checking. Clearly
//! non-physical inputs (`P ≤ 0`, `G ≤ 0`, `D ≤ 0`, `x > 1`) return
//! [`MultiphaseError::InvalidInput`]. The look-up table instead **clamps** an
//! out-of-range query to its axis bounds (documented on [`GroeneveldLut`]).

use crate::MultiphaseError;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, HeatFluxDensity, Length, MassFlux, Pressure};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::length::meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;

// ─────────────────────────────────────────────────────────────────────────────
// Contracts (traits) + runtime-selection enum
// ─────────────────────────────────────────────────────────────────────────────

/// Compiler-enforced contract every critical-heat-flux correlation satisfies.
///
/// A `ChfModel` maps the local flow state `(P, G, x, D)` to the critical heat
/// flux `q''_c`. It is used purely as a **contract** (the compiler checks each
/// concrete correlation implements it); runtime dispatch goes through the
/// [`ChfCorrelation`] enum, not a trait object, per the workspace no-`dyn` rule.
///
/// # Units
/// - `pressure` — system pressure `P` `[Pa]`.
/// - `mass_flux` — mass flux `G` `[kg·m⁻²·s⁻¹]`.
/// - `quality` — thermodynamic equilibrium quality `x` `[-]` (may be negative
///   for subcooled conditions; must be `≤ 1`).
/// - `diameter` — heated / hydraulic diameter `D` `[m]`.
/// - returns critical heat flux `q''_c` `[W/m²]`.
pub trait ChfModel {
    /// Critical heat flux `q''_c` `[W/m²]` at the given local flow state.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] for non-physical inputs (`P ≤ 0`,
    /// `G ≤ 0`, `D ≤ 0`, or `x > 1`), or a model-specific failure.
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError>;

    /// Whether `(P, G, x, D)` lies inside the correlation's documented validity
    /// range. `false` means [`critical_heat_flux`](Self::critical_heat_flux)
    /// will *extrapolate* (it still returns a value for physical inputs); the
    /// caller decides whether to trust it.
    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> bool;
}

/// Compiler-enforced contract for correlations that additionally account for
/// **inlet subcooling** explicitly at call time.
///
/// Most CHF correlations here are functions of the *local* state only; the
/// Westinghouse [`W3`] correlation carries an inlet-subcooling correction
/// factor. This trait exposes that dependence as an explicit call-time argument
/// (the subcooling enthalpy `Δh_in = h_f − h_in` `[J/kg]`), so a caller can
/// sweep subcooling without rebuilding the model.
pub trait ChfSubCoolModel {
    /// Critical heat flux `q''_c` `[W/m²]` with an explicit inlet-subcooling
    /// enthalpy `subcooling = h_f − h_in` `[J/kg]` ([`AvailableEnergy`]).
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] for non-physical inputs.
    fn critical_heat_flux_subcooled(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
        subcooling: AvailableEnergy,
    ) -> Result<HeatFluxDensity, MultiphaseError>;
}

/// Runtime-selectable critical-heat-flux correlation (enum dispatch, no `dyn`).
///
/// Wraps the four concrete models so a solver can choose a correlation at run
/// time while keeping the exhaustiveness, zero-allocation, and
/// rust-analyzer-navigability benefits of an enum over a trait object. Forwards
/// the [`ChfModel`] contract to the selected variant.
///
/// # Variants
/// - [`Biasi`](Self::Biasi) — Biasi et al. (1967) round-tube correlation.
/// - [`W3`](Self::W3) — Westinghouse W-3 / Tong (1967) DNB correlation.
/// - [`Bowring`](Self::Bowring) — Bowring (1972) round-tube dryout correlation.
/// - [`GroeneveldLut`](Self::GroeneveldLut) — Groeneveld 2006 look-up-table
///   framework (interpolated CHF for an 8 mm reference tube).
pub enum ChfCorrelation {
    /// Biasi et al. (1967) correlation. See [`Biasi`].
    Biasi(Biasi),
    /// Westinghouse W-3 / Tong (1967) DNB correlation. See [`W3`].
    W3(W3),
    /// Bowring (1972) round-tube dryout correlation. See [`Bowring`].
    Bowring(Bowring),
    /// Groeneveld look-up-table framework. See [`GroeneveldLut`].
    GroeneveldLut(GroeneveldLut),
}

impl ChfModel for ChfCorrelation {
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        match self {
            Self::Biasi(m) => m.critical_heat_flux(pressure, mass_flux, quality, diameter),
            Self::W3(m) => m.critical_heat_flux(pressure, mass_flux, quality, diameter),
            Self::Bowring(m) => m.critical_heat_flux(pressure, mass_flux, quality, diameter),
            Self::GroeneveldLut(m) => m.critical_heat_flux(pressure, mass_flux, quality, diameter),
        }
    }

    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> bool {
        match self {
            Self::Biasi(m) => m.in_valid_range(pressure, mass_flux, quality, diameter),
            Self::W3(m) => m.in_valid_range(pressure, mass_flux, quality, diameter),
            Self::Bowring(m) => m.in_valid_range(pressure, mass_flux, quality, diameter),
            Self::GroeneveldLut(m) => m.in_valid_range(pressure, mass_flux, quality, diameter),
        }
    }
}

/// Guard shared by the algebraic correlations: reject clearly non-physical
/// inputs. Returns `(P[Pa], G[kg/m²s], D[m])` as plain `f64` for the formulas.
fn unpack_physical(
    pressure: Pressure,
    mass_flux: MassFlux,
    quality: f64,
    diameter: Length,
) -> Result<(f64, f64, f64), MultiphaseError> {
    let p = pressure.get::<pascal>();
    let g = mass_flux.get::<kilogram_per_square_meter_second>();
    let d = diameter.get::<meter>();
    if !(p > 0.0) {
        return Err(MultiphaseError::InvalidInput(format!(
            "pressure must be > 0 Pa (got {p})"
        )));
    }
    if !(g > 0.0) {
        return Err(MultiphaseError::InvalidInput(format!(
            "mass flux must be > 0 kg/m^2/s (got {g})"
        )));
    }
    if !(d > 0.0) {
        return Err(MultiphaseError::InvalidInput(format!(
            "diameter must be > 0 m (got {d})"
        )));
    }
    if quality > 1.0 {
        return Err(MultiphaseError::InvalidInput(format!(
            "quality must be <= 1 (got {quality})"
        )));
    }
    Ok((p, g, d))
}

// ─────────────────────────────────────────────────────────────────────────────
// Biasi et al. (1967)
// ─────────────────────────────────────────────────────────────────────────────

/// **Biasi et al. (1967)** critical-heat-flux correlation for upward flow of
/// water in uniformly heated round tubes.
///
/// Source: L. Biasi, G.C. Clerici, S. Garribba, R. Sala, A. Tozzi (1967),
/// *"Studies on burnout — Part 3: A new correlation for round ducts and uniform
/// heating and its comparison with world data"*, Energia Nucleare **14**(9),
/// 530–536. Form and constants as reproduced in N.E. Todreas & M.S. Kazimi,
/// *Nuclear Systems Volume I: Thermal Hydraulic Fundamentals* (2nd ed., 2012),
/// and S.M. Ghiaasiaan, *Two-Phase Flow, Boiling and Condensation* (2nd ed.,
/// 2017).
///
/// ## Correlation
///
/// Two candidate heat fluxes are formed (`p` in **bar**, `D` in **cm**,
/// `G` in `kg·m⁻²·s⁻¹`, `x` dimensionless, `q''` in `W/m²`):
///
/// low-quality / high-flux branch
/// `q''_1 = (1.883e7 / (D^n · G^{1/6})) · ( f_p / G^{1/6} − x )`
///
/// high-quality / low-flux branch
/// `q''_2 = (3.78e7 · h_p / (D^n · G^{0.6})) · ( 1 − x )`
///
/// with the pressure functions
/// `f_p = 0.7249 + 0.099·p·exp(−0.032·p)` and
/// `h_p = −1.159 + 0.149·p·exp(−0.019·p) + 8.99·p / (10 + p²)`,
/// and diameter exponent `n = 0.4` for `D ≥ 1 cm`, `n = 0.6` for `D < 1 cm`.
///
/// Selection rule: for `G ≥ 300 kg·m⁻²·s⁻¹`, `q''_c = max(q''_1, q''_2)`;
/// for `G < 300`, only the high-quality branch is used, `q''_c = q''_2`.
///
/// ## Validity range (from the source)
/// - pressure `P`: 2.7 – 140 bar (0.27 – 14.0 MPa)
/// - mass flux `G`: 100 – 6000 kg·m⁻²·s⁻¹
/// - diameter `D`: 0.3 – 3.75 cm
/// - quality `x`: up to the value making the selected branch non-negative
///
/// This model is stateless (no stored parameters).
#[derive(Debug, Clone, Copy, Default)]
pub struct Biasi;

impl Biasi {
    /// Construct the (stateless) Biasi correlation.
    pub fn new() -> Self {
        Biasi
    }
}

impl ChfModel for Biasi {
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        let (p_pa, g, d_m) = unpack_physical(pressure, mass_flux, quality, diameter)?;
        let p_bar = p_pa / 1.0e5; // Pa → bar
        let d_cm = d_m * 100.0; // m → cm
        let n = if d_cm >= 1.0 { 0.4 } else { 0.6 };

        let f_p = 0.7249 + 0.099 * p_bar * (-0.032 * p_bar).exp();
        let h_p =
            -1.159 + 0.149 * p_bar * (-0.019 * p_bar).exp() + 8.99 * p_bar / (10.0 + p_bar * p_bar);

        let d_n = d_cm.powf(n);
        let g16 = g.powf(1.0 / 6.0);
        let q1 = (1.883e7 / (d_n * g16)) * (f_p / g16 - quality);
        let q2 = (3.78e7 * h_p / (d_n * g.powf(0.6))) * (1.0 - quality);

        let q = if g >= 300.0 { q1.max(q2) } else { q2 };
        Ok(HeatFluxDensity::new::<watt_per_square_meter>(q))
    }

    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        _quality: f64,
        diameter: Length,
    ) -> bool {
        let p_bar = pressure.get::<pascal>() / 1.0e5;
        let g = mass_flux.get::<kilogram_per_square_meter_second>();
        let d_cm = diameter.get::<meter>() * 100.0;
        (2.7..=140.0).contains(&p_bar)
            && (100.0..=6000.0).contains(&g)
            && (0.3..=3.75).contains(&d_cm)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Westinghouse W-3 (Tong, 1967)
// ─────────────────────────────────────────────────────────────────────────────

/// **Westinghouse W-3** (Tong, 1967) departure-from-nucleate-boiling (DNB)
/// correlation for pressurised-water conditions.
///
/// Source: L.S. Tong (1967), *"Prediction of departure from nucleate boiling for
/// an axially non-uniform heat flux distribution"*, J. Nuclear Energy **21**(3),
/// 241–248; uniform-flux form as tabulated in Todreas & Kazimi, *Nuclear
/// Systems Volume I* (2nd ed., 2012), and Tong & Weisman, *Thermal Analysis of
/// Pressurized Water Reactors* (3rd ed., 1996).
///
/// ## Correlation (original mixed English units, converted internally)
///
/// The W-3 uniform-flux DNB heat flux `q''_{DNB,EU}` in `BTU·hr⁻¹·ft⁻²` is
/// (`P` in psia, `G` in `lbm·hr⁻¹·ft⁻²`, `D_e` in inch, `x` `[-]`,
/// `Δh_in = h_f − h_in` in `BTU·lbm⁻¹`):
///
/// ```text
/// q''/1e6 = { (2.022 − 0.0004302 P) + (0.1722 − 0.0000984 P)·exp[(18.177 − 0.004129 P)·x] }
///          × [ (0.1484 − 1.596 x + 0.1729 x|x|)·G/1e6 + 1.037 ]·(1.157 − 0.869 x)
///          × [ 0.2664 + 0.8357·exp(−3.151 D_e) ]·[ 0.8258 + 0.000794·Δh_in ]
/// ```
///
/// The implementation converts SI inputs → these units, evaluates, and converts
/// the result `BTU·hr⁻¹·ft⁻²` → `W/m²` (`× 3.154591`). The inlet subcooling
/// `Δh_in = h_f − h_in` `[J/kg]` is stored on the struct (see [`W3::new`]); a
/// call-time override is available via [`ChfSubCoolModel`].
///
/// ## Validity range (from the source)
/// - pressure `P`: 1000 – 2300 psia (6.9 – 15.9 MPa)
/// - mass flux `G`: 1.0e6 – 5.0e6 lbm·hr⁻¹·ft⁻² (≈ 1356 – 6800 kg·m⁻²·s⁻¹)
/// - equivalent diameter `D_e`: 0.2 – 0.7 inch (5.1 – 17.8 mm)
/// - quality `x`: −0.15 – +0.15 (subcooled to low quality)
/// - inlet enthalpy `h_in ≥ 400 BTU·lbm⁻¹`
pub struct W3 {
    /// Inlet subcooling enthalpy `Δh_in = h_f − h_in` `[J/kg]` (stored in SI).
    subcooling_j_per_kg: f64,
}

/// pascal per psi — `P[psia] = P[Pa] / 6894.757`.
const PA_PER_PSI: f64 = 6894.757;
/// `G[lbm·hr⁻¹·ft⁻²] = G[kg·m⁻²·s⁻¹] / 1.3562299e-3`.
const KG_M2S_PER_LBM_HR_FT2: f64 = 1.3562299e-3;
/// metre per inch.
const M_PER_INCH: f64 = 0.0254;
/// `W/m²` per `BTU·hr⁻¹·ft⁻²`.
const WM2_PER_BTU_HR_FT2: f64 = 3.154591;
/// `J·kg⁻¹` per `BTU·lbm⁻¹`.
const J_PER_KG_PER_BTU_LBM: f64 = 2326.0;

impl W3 {
    /// Construct a W-3 model with a fixed inlet subcooling enthalpy.
    ///
    /// # Parameters
    /// - `subcooling` — `Δh_in = h_f − h_in` `[J/kg]` ([`AvailableEnergy`]), the
    ///   inlet subcooling used by the W-3 subcooling factor
    ///   `0.8258 + 0.000794·Δh_in` (with `Δh_in` internally in `BTU·lbm⁻¹`).
    ///   Use `AvailableEnergy::new::<joule_per_kilogram>(0.0)` for a saturated
    ///   inlet (factor `0.8258`).
    pub fn new(subcooling: AvailableEnergy) -> Self {
        W3 {
            subcooling_j_per_kg: subcooling.get::<joule_per_kilogram>(),
        }
    }

    /// The stored inlet subcooling enthalpy `Δh_in` `[J/kg]`.
    pub fn subcooling(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.subcooling_j_per_kg)
    }

    /// Core W-3 evaluation in native units, given SI inputs and an explicit
    /// subcooling enthalpy `[J/kg]`. Returns `q''_c` `[W/m²]`.
    fn evaluate(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
        subcooling_j_per_kg: f64,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        let (p_pa, g_si, d_m) = unpack_physical(pressure, mass_flux, quality, diameter)?;
        let p = p_pa / PA_PER_PSI; // psia
        let g6 = (g_si / KG_M2S_PER_LBM_HR_FT2) / 1.0e6; // (lbm/hr/ft²)/1e6
        let de = d_m / M_PER_INCH; // inch
        let x = quality;
        let dh_in = subcooling_j_per_kg / J_PER_KG_PER_BTU_LBM; // BTU/lbm

        let term1 = (2.022 - 0.0004302 * p)
            + (0.1722 - 0.0000984 * p) * ((18.177 - 0.004129 * p) * x).exp();
        let term2 = (0.1484 - 1.596 * x + 0.1729 * x * x.abs()) * g6 + 1.037;
        let term3 = 1.157 - 0.869 * x;
        let term4 = 0.2664 + 0.8357 * (-3.151 * de).exp();
        let term5 = 0.8258 + 0.000794 * dh_in;

        let q_btu = (term1 * term2 * term3 * term4 * term5) * 1.0e6; // BTU/hr/ft²
        Ok(HeatFluxDensity::new::<watt_per_square_meter>(
            q_btu * WM2_PER_BTU_HR_FT2,
        ))
    }
}

impl ChfModel for W3 {
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        self.evaluate(
            pressure,
            mass_flux,
            quality,
            diameter,
            self.subcooling_j_per_kg,
        )
    }

    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> bool {
        let p = pressure.get::<pascal>() / PA_PER_PSI;
        let g6 =
            (mass_flux.get::<kilogram_per_square_meter_second>() / KG_M2S_PER_LBM_HR_FT2) / 1.0e6;
        let de = diameter.get::<meter>() / M_PER_INCH;
        (1000.0..=2300.0).contains(&p)
            && (1.0..=5.0).contains(&g6)
            && (0.2..=0.7).contains(&de)
            && (-0.15..=0.15).contains(&quality)
    }
}

impl ChfSubCoolModel for W3 {
    fn critical_heat_flux_subcooled(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
        subcooling: AvailableEnergy,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        self.evaluate(
            pressure,
            mass_flux,
            quality,
            diameter,
            subcooling.get::<joule_per_kilogram>(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bowring (1972)
// ─────────────────────────────────────────────────────────────────────────────

/// **Bowring (1972)** round-tube, uniform-heat-flux dryout correlation for
/// water over 0.2 – 19 MPa.
///
/// Source: R.W. Bowring (1972), *"A simple but accurate round tube, uniform heat
/// flux, dryout correlation over the pressure range 0.7 to 17 MN/m²"*,
/// report AEEW-R789, UK Atomic Energy Authority, Winfrith. Form as reproduced in
/// Todreas & Kazimi, *Nuclear Systems Volume I* (2nd ed., 2012) and Collier &
/// Thome, *Convective Boiling and Condensation* (3rd ed., 1994).
///
/// ## Correlation (SI throughout)
///
/// `q''_c = ( A − (1/4)·D·G·h_fg·x ) / C`  `[W/m²]`, with (`D` in m, `G` in
/// `kg·m⁻²·s⁻¹`, `h_fg` in `J/kg`, `q''` in `W/m²`)
///
/// ```text
/// A = 2.317·(h_fg·D·G/4)·F1 / (1 + 0.0143·F2·D^{1/2}·G)
/// C = 0.077·F3·D·G / (1 + 0.347·F4·(G/1356)^n)
/// n = 2.0 − 0.5·p_R
/// ```
///
/// where `p_R = 0.145·P` with `P` in **MPa** (so `p_R` is the pressure in units
/// of 1000 psi; `p_R = 1` at `P ≈ 6.895 MPa`). The pressure functions, for
/// `p_R ≤ 1`:
///
/// ```text
/// F1     = ( p_R^{18.942}·exp[20.89(1−p_R)] + 0.917 ) / 1.917
/// F1/F2  = ( p_R^{1.316}·exp[2.444(1−p_R)] + 0.309 ) / 1.309
/// F3     = ( p_R^{17.023}·exp[16.658(1−p_R)] + 0.667 ) / 1.667
/// F4/F3  = p_R^{1.649}
/// ```
///
/// and for `p_R > 1`:
///
/// ```text
/// F1     = p_R^{-0.368}·exp[0.648(1−p_R)]
/// F1/F2  = p_R^{-0.448}·exp[0.245(1−p_R)]
/// F3     = p_R^{0.219}
/// F4/F3  = p_R^{1.649}
/// ```
///
/// with `F2 = F1 / (F1/F2)` and `F4 = F3 · (F4/F3)`.
///
/// The latent heat `h_fg` at the system pressure is a required input, stored on
/// the struct at construction (see [`Bowring::new`]) from steam tables (public
/// literature data).
///
/// ## Validity range (from the source)
/// - pressure `P`: 0.2 – 19 MPa (original fit 0.7 – 17 MN/m²)
/// - diameter `D`: 2 – 45 mm (0.002 – 0.045 m)
/// - mass flux `G`: 136 – 18 600 kg·m⁻²·s⁻¹
pub struct Bowring {
    /// Latent heat of vaporisation `h_fg` `[J/kg]` at the system pressure.
    h_fg_j_per_kg: f64,
}

impl Bowring {
    /// Construct a Bowring model with the latent heat at the system pressure.
    ///
    /// # Parameters
    /// - `h_fg` — latent heat of vaporisation `[J/kg]` ([`AvailableEnergy`]) at
    ///   the operating pressure, taken from steam tables. Must be `> 0`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if `h_fg ≤ 0`.
    pub fn new(h_fg: AvailableEnergy) -> Result<Self, MultiphaseError> {
        let h = h_fg.get::<joule_per_kilogram>();
        if !(h > 0.0) {
            return Err(MultiphaseError::InvalidInput(format!(
                "h_fg must be > 0 J/kg (got {h})"
            )));
        }
        Ok(Bowring { h_fg_j_per_kg: h })
    }

    /// The stored latent heat `h_fg` `[J/kg]`.
    pub fn h_fg(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.h_fg_j_per_kg)
    }
}

/// Bowring pressure functions `(F1, F2, F3, F4)` for reduced pressure `p_R`.
fn bowring_f(p_r: f64) -> (f64, f64, f64, f64) {
    let (f1, f1_over_f2, f3);
    if p_r <= 1.0 {
        f1 = (p_r.powf(18.942) * (20.89 * (1.0 - p_r)).exp() + 0.917) / 1.917;
        f1_over_f2 = (p_r.powf(1.316) * (2.444 * (1.0 - p_r)).exp() + 0.309) / 1.309;
        f3 = (p_r.powf(17.023) * (16.658 * (1.0 - p_r)).exp() + 0.667) / 1.667;
    } else {
        f1 = p_r.powf(-0.368) * (0.648 * (1.0 - p_r)).exp();
        f1_over_f2 = p_r.powf(-0.448) * (0.245 * (1.0 - p_r)).exp();
        f3 = p_r.powf(0.219);
    }
    let f2 = f1 / f1_over_f2;
    let f4 = f3 * p_r.powf(1.649);
    (f1, f2, f3, f4)
}

impl ChfModel for Bowring {
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        let (p_pa, g, d) = unpack_physical(pressure, mass_flux, quality, diameter)?;
        let h_fg = self.h_fg_j_per_kg;
        let p_mpa = p_pa / 1.0e6;
        let p_r = 0.145 * p_mpa; // pressure in kpsi

        let (f1, f2, f3, f4) = bowring_f(p_r);
        let n = 2.0 - 0.5 * p_r;

        let a = 2.317 * (h_fg * d * g / 4.0) * f1 / (1.0 + 0.0143 * f2 * d.sqrt() * g);
        let c = 0.077 * f3 * d * g / (1.0 + 0.347 * f4 * (g / 1356.0).powf(n));

        let q = (a - 0.25 * d * g * h_fg * quality) / c;
        Ok(HeatFluxDensity::new::<watt_per_square_meter>(q))
    }

    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        _quality: f64,
        diameter: Length,
    ) -> bool {
        let p_mpa = pressure.get::<pascal>() / 1.0e6;
        let g = mass_flux.get::<kilogram_per_square_meter_second>();
        let d = diameter.get::<meter>();
        (0.2..=19.0).contains(&p_mpa)
            && (0.002..=0.045).contains(&d)
            && (136.0..=18600.0).contains(&g)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Groeneveld look-up-table framework
// ─────────────────────────────────────────────────────────────────────────────

/// **Groeneveld 2006 CHF look-up-table** framework — tabulated critical heat
/// flux `q''_c(P, G, x)` for a vertical 8 mm water-cooled tube, with trilinear
/// interpolation and CSV import/export.
///
/// Source of the method: D.C. Groeneveld, J.Q. Shan, A.Z. Vasić, L.K.H. Leung,
/// A. Durmayaz, J. Yang, S.C. Cheng, A. Tanase (2007), *"The 2006 CHF look-up
/// table"*, Nuclear Engineering and Design **237**(15–17), 1909–1922. The LUT
/// gives CHF at discrete nodes over the axes
/// (pressure `P`, mass flux `G`, quality `x`) for a reference diameter of 8 mm;
/// values between nodes are obtained by **trilinear interpolation**, and a
/// diameter-correction factor scales the 8 mm value to other tube sizes.
///
/// ## Data scope — honest note
///
/// The **full** published 2006 table is a large dataset (≈24 pressures × 20 mass
/// fluxes × 23 qualities). It is **not embedded here**: importing the complete
/// public LUT is a deferred data-acquisition step (respecting the workspace
/// `DATA_POLICY` — only properly sourced public literature data, with provenance
/// recorded). This type implements the table structure, the interpolation, and a
/// CSV loader/writer; the tests exercise them against a **small, synthetic,
/// illustrative** sample grid (round made-up numbers — *not* the copyrighted
/// Groeneveld values). Load the real table via [`from_csv`](Self::from_csv) once
/// it has been obtained and its provenance documented.
///
/// ## Axes and storage
///
/// The three axis vectors must be **strictly ascending**. Table values are
/// stored row-major with index `[ip·(ng·nx) + ig·nx + ix]`, i.e. quality varies
/// fastest, then mass flux, then pressure. Units: `P` `[Pa]`, `G`
/// `[kg·m⁻²·s⁻¹]`, `x` `[-]`, stored CHF `[W/m²]`.
///
/// ## Out-of-range behaviour
///
/// A query `(P, G, x)` outside the axis bounds is **clamped** to the nearest
/// bound on each axis before interpolation (documented, deterministic). This is
/// the boundary-value (no-extrapolation) convention;
/// [`in_valid_range`](ChfModel::in_valid_range) reports whether clamping
/// occurred.
///
/// [`from_csv`]: Self::from_csv
pub struct GroeneveldLut {
    /// Pressure axis `[Pa]`, strictly ascending.
    pressures: Vec<f64>,
    /// Mass-flux axis `[kg·m⁻²·s⁻¹]`, strictly ascending.
    mass_fluxes: Vec<f64>,
    /// Quality axis `[-]`, strictly ascending.
    qualities: Vec<f64>,
    /// CHF values `[W/m²]`, row-major `[ip·(ng·nx) + ig·nx + ix]`.
    chf: Vec<f64>,
    /// Reference tube diameter of the table `[m]` (8 mm for the Groeneveld LUT).
    reference_diameter_m: f64,
}

/// Return `true` iff `v` is strictly ascending with at least 2 entries.
fn strictly_ascending(v: &[f64]) -> bool {
    v.len() >= 2 && v.windows(2).all(|w| w[0] < w[1])
}

impl GroeneveldLut {
    /// Build a look-up table from explicit axes and a row-major value array.
    ///
    /// # Parameters
    /// - `pressures` — pressure axis `[Pa]`, strictly ascending, `≥ 2` nodes.
    /// - `mass_fluxes` — mass-flux axis `[kg·m⁻²·s⁻¹]`, strictly ascending, `≥ 2`.
    /// - `qualities` — quality axis `[-]`, strictly ascending, `≥ 2`.
    /// - `chf` — CHF values `[W/m²]`, length `np·ng·nx`, row-major with quality
    ///   fastest (`[ip·ng·nx + ig·nx + ix]`).
    /// - `reference_diameter` — the table's reference tube diameter `[m]`
    ///   (8 mm for the Groeneveld LUT). Must be `> 0`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if any axis is not strictly ascending
    /// or too short, if `chf.len() != np·ng·nx`, or if
    /// `reference_diameter ≤ 0`.
    pub fn new(
        pressures: Vec<f64>,
        mass_fluxes: Vec<f64>,
        qualities: Vec<f64>,
        chf: Vec<f64>,
        reference_diameter: Length,
    ) -> Result<Self, MultiphaseError> {
        for (name, ax) in [
            ("pressure", &pressures),
            ("mass_flux", &mass_fluxes),
            ("quality", &qualities),
        ] {
            if !strictly_ascending(ax) {
                return Err(MultiphaseError::InvalidInput(format!(
                    "{name} axis must be strictly ascending with >= 2 nodes"
                )));
            }
        }
        let expected = pressures.len() * mass_fluxes.len() * qualities.len();
        if chf.len() != expected {
            return Err(MultiphaseError::InvalidInput(format!(
                "chf has {} values, expected np*ng*nx = {expected}",
                chf.len()
            )));
        }
        let d_ref = reference_diameter.get::<meter>();
        if !(d_ref > 0.0) {
            return Err(MultiphaseError::InvalidInput(format!(
                "reference diameter must be > 0 m (got {d_ref})"
            )));
        }
        Ok(GroeneveldLut {
            pressures,
            mass_fluxes,
            qualities,
            chf,
            reference_diameter_m: d_ref,
        })
    }

    /// Number of `(pressure, mass_flux, quality)` nodes in the table.
    pub fn n_nodes(&self) -> usize {
        self.chf.len()
    }

    /// The table's reference tube diameter `[m]`.
    pub fn reference_diameter(&self) -> Length {
        Length::new::<meter>(self.reference_diameter_m)
    }

    /// Flat index into `chf` for node `(ip, ig, ix)`.
    fn idx(&self, ip: usize, ig: usize, ix: usize) -> usize {
        let ng = self.mass_fluxes.len();
        let nx = self.qualities.len();
        ip * (ng * nx) + ig * nx + ix
    }

    /// Bracketing lower index and interpolation weight on an ascending axis.
    ///
    /// Returns `(i, t)` such that the value lies between `axis[i]` and
    /// `axis[i+1]` with fraction `t ∈ [0,1]`; the query is clamped to the axis
    /// bounds first (so `t = 0` at/below the low bound, `t = 1` at/above the
    /// high bound, landing exactly on a node gives `t = 0` there).
    fn bracket(axis: &[f64], v: f64) -> (usize, f64) {
        let last = axis.len() - 1;
        if v <= axis[0] {
            return (0, 0.0);
        }
        if v >= axis[last] {
            return (last - 1, 1.0);
        }
        // axis[0] < v < axis[last]; find i with axis[i] <= v < axis[i+1].
        let mut i = 0;
        while i + 1 < axis.len() && axis[i + 1] <= v {
            i += 1;
        }
        let t = (v - axis[i]) / (axis[i + 1] - axis[i]);
        (i, t)
    }

    /// Raw trilinear-interpolated CHF `[W/m²]` for the **reference** 8 mm tube,
    /// at `(P[Pa], G[kg·m⁻²·s⁻¹], x[-])`, clamped to the axis bounds.
    ///
    /// Exact at table nodes; a linear blend of the 8 surrounding corners
    /// elsewhere. No diameter correction is applied (see
    /// [`critical_heat_flux`](ChfModel::critical_heat_flux) for the
    /// diameter-scaled result).
    pub fn lookup(&self, pressure_pa: f64, mass_flux: f64, quality: f64) -> f64 {
        let (ip, tp) = Self::bracket(&self.pressures, pressure_pa);
        let (ig, tg) = Self::bracket(&self.mass_fluxes, mass_flux);
        let (ix, tx) = Self::bracket(&self.qualities, quality);

        let c = |dp: usize, dg: usize, dx: usize| self.chf[self.idx(ip + dp, ig + dg, ix + dx)];
        // Trilinear blend of the 8 corners.
        let mut acc = 0.0;
        for (dp, wp) in [(0usize, 1.0 - tp), (1, tp)] {
            for (dg, wg) in [(0usize, 1.0 - tg), (1, tg)] {
                for (dx, wx) in [(0usize, 1.0 - tx), (1, tx)] {
                    acc += wp * wg * wx * c(dp, dg, dx);
                }
            }
        }
        acc
    }

    /// Groeneveld cylindrical-tube **diameter-correction factor** `K1`, scaling
    /// the reference CHF to a tube of diameter `D` `[m]`.
    ///
    /// `K1 = (D_ref / D)^{1/2}` for `0.002 m ≤ D ≤ 0.016 m`; for `D > 0.016 m`
    /// the factor is held at its value at 16 mm, and for `D < 0.002 m` at its
    /// value at 2 mm (the recommended clamps from Groeneveld et al., 2007). With
    /// the 8 mm reference this gives `K1 = 1` at `D = 8 mm`.
    pub fn diameter_factor(&self, diameter_m: f64) -> f64 {
        let d = diameter_m.clamp(0.002, 0.016);
        (self.reference_diameter_m / d).sqrt()
    }
}

impl ChfModel for GroeneveldLut {
    /// Diameter-corrected CHF: `q''_c = K1(D)·lookup(P, G, x)`.
    ///
    /// The `(P, G, x)` query is clamped to the table axis bounds (see
    /// [`GroeneveldLut`]); `D` enters through the
    /// [`diameter_factor`](GroeneveldLut::diameter_factor). Non-physical inputs
    /// (`P ≤ 0`, `G ≤ 0`, `D ≤ 0`, `x > 1`) return
    /// [`MultiphaseError::InvalidInput`].
    fn critical_heat_flux(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        diameter: Length,
    ) -> Result<HeatFluxDensity, MultiphaseError> {
        let (p, g, d) = unpack_physical(pressure, mass_flux, quality, diameter)?;
        let base = self.lookup(p, g, quality);
        Ok(HeatFluxDensity::new::<watt_per_square_meter>(
            self.diameter_factor(d) * base,
        ))
    }

    fn in_valid_range(
        &self,
        pressure: Pressure,
        mass_flux: MassFlux,
        quality: f64,
        _diameter: Length,
    ) -> bool {
        let p = pressure.get::<pascal>();
        let g = mass_flux.get::<kilogram_per_square_meter_second>();
        let pl = *self.pressures.first().unwrap();
        let ph = *self.pressures.last().unwrap();
        let gl = *self.mass_fluxes.first().unwrap();
        let gh = *self.mass_fluxes.last().unwrap();
        let xl = *self.qualities.first().unwrap();
        let xh = *self.qualities.last().unwrap();
        (pl..=ph).contains(&p) && (gl..=gh).contains(&g) && (xl..=xh).contains(&quality)
    }
}

// ── CSV import / export ──────────────────────────────────────────────────────

impl GroeneveldLut {
    /// Serialise the table to CSV text (tidy / long format).
    ///
    /// The output is a leading comment recording the reference diameter, one
    /// header line `pressure_pa,mass_flux_kg_m2_s,quality,chf_w_m2`, then one
    /// row per node — all `np·ng·nx` combinations, row-major (quality fastest).
    /// Numbers use Rust's shortest round-trip `f64` formatting so
    /// [`from_csv`](Self::from_csv) reconstructs the exact same table.
    pub fn to_csv(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "# groeneveld_lut reference_diameter_m={}\n",
            self.reference_diameter_m
        ));
        s.push_str("pressure_pa,mass_flux_kg_m2_s,quality,chf_w_m2\n");
        for (ip, &p) in self.pressures.iter().enumerate() {
            for (ig, &g) in self.mass_fluxes.iter().enumerate() {
                for (ix, &x) in self.qualities.iter().enumerate() {
                    let v = self.chf[self.idx(ip, ig, ix)];
                    s.push_str(&format!("{p},{g},{x},{v}\n"));
                }
            }
        }
        s
    }

    /// Parse a table from CSV text produced by [`to_csv`](Self::to_csv) (tidy
    /// format: header line, then `pressure_pa,mass_flux,quality,chf` rows).
    ///
    /// The distinct pressure / mass-flux / quality values seen in the rows form
    /// the three axes (sorted ascending); every combination must appear exactly
    /// once. A leading `# ... reference_diameter_m=<v>` comment sets the
    /// reference diameter (default 0.008 m if absent). Blank lines and other
    /// `#` comments are ignored.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] on a malformed row, a non-numeric
    /// field, a missing/duplicated `(P, G, x)` combination, or fewer than 2
    /// distinct values on any axis.
    pub fn from_csv(text: &str) -> Result<Self, MultiphaseError> {
        let mut ref_d = 0.008_f64;
        let mut rows: Vec<(f64, f64, f64, f64)> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix('#') {
                if let Some(pos) = rest.find("reference_diameter_m=") {
                    let tail = &rest[pos + "reference_diameter_m=".len()..];
                    let tok = tail.split_whitespace().next().unwrap_or("");
                    if let Ok(v) = tok.parse::<f64>() {
                        ref_d = v;
                    }
                }
                continue;
            }
            if line.starts_with("pressure_pa") {
                continue; // header
            }
            let f: Vec<&str> = line.split(',').map(|t| t.trim()).collect();
            if f.len() != 4 {
                return Err(MultiphaseError::InvalidInput(format!(
                    "CSV row must have 4 fields, got {}: {line:?}",
                    f.len()
                )));
            }
            let parse = |s: &str| -> Result<f64, MultiphaseError> {
                s.parse::<f64>()
                    .map_err(|e| MultiphaseError::InvalidInput(format!("bad number {s:?}: {e}")))
            };
            rows.push((parse(f[0])?, parse(f[1])?, parse(f[2])?, parse(f[3])?));
        }

        // Distinct sorted axes.
        let axis = |sel: fn(&(f64, f64, f64, f64)) -> f64| -> Vec<f64> {
            let mut v: Vec<f64> = rows.iter().map(sel).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup_by(|a, b| (*a - *b).abs() == 0.0);
            v
        };
        let pressures = axis(|r| r.0);
        let mass_fluxes = axis(|r| r.1);
        let qualities = axis(|r| r.2);
        let (np, ng, nx) = (pressures.len(), mass_fluxes.len(), qualities.len());
        if np < 2 || ng < 2 || nx < 2 {
            return Err(MultiphaseError::InvalidInput(format!(
                "each axis needs >= 2 distinct values (got np={np}, ng={ng}, nx={nx})"
            )));
        }
        if rows.len() != np * ng * nx {
            return Err(MultiphaseError::InvalidInput(format!(
                "expected {} rows (np*ng*nx), got {}",
                np * ng * nx,
                rows.len()
            )));
        }

        let find = |axis: &[f64], v: f64| -> usize { axis.iter().position(|&a| a == v).unwrap() };
        let mut chf = vec![f64::NAN; np * ng * nx];
        for (p, g, x, val) in rows {
            let ip = find(&pressures, p);
            let ig = find(&mass_fluxes, g);
            let ix = find(&qualities, x);
            let flat = ip * (ng * nx) + ig * nx + ix;
            if !chf[flat].is_nan() {
                return Err(MultiphaseError::InvalidInput(format!(
                    "duplicated (P,G,x) = ({p},{g},{x}) in CSV"
                )));
            }
            chf[flat] = val;
        }
        if chf.iter().any(|v| v.is_nan()) {
            return Err(MultiphaseError::InvalidInput(
                "CSV is missing one or more (P,G,x) combinations".to_string(),
            ));
        }
        GroeneveldLut::new(
            pressures,
            mass_fluxes,
            qualities,
            chf,
            Length::new::<meter>(ref_d),
        )
    }

    /// A **small synthetic sample** LUT for demos/tests — **NOT** the real
    /// Groeneveld data.
    ///
    /// A 3×3×3 grid over `P ∈ {5, 10, 15} MPa`, `G ∈ {500, 2000, 4000}
    /// kg·m⁻²·s⁻¹`, `x ∈ {−0.1, 0.1, 0.3}`, with round, made-up CHF values that
    /// decrease with quality and vary mildly with pressure/flux. Its only role
    /// is to exercise the interpolation and CSV paths; do not use its numbers
    /// for any physical purpose. Reference diameter 8 mm.
    pub fn sample() -> Self {
        let pressures = vec![5.0e6, 10.0e6, 15.0e6];
        let mass_fluxes = vec![500.0, 2000.0, 4000.0];
        let qualities = vec![-0.1, 0.1, 0.3];
        // Synthetic model: base 3.0 MW/m² + pressure/flux terms − quality term.
        let mut chf = Vec::with_capacity(27);
        for &p in &pressures {
            for &g in &mass_fluxes {
                for &x in &qualities {
                    let v = 3.0e6 + 0.02 * p + 200.0 * g - 4.0e6 * x;
                    chf.push(v);
                }
            }
        }
        GroeneveldLut::new(
            pressures,
            mass_fluxes,
            qualities,
            chf,
            Length::new::<meter>(0.008),
        )
        .expect("sample LUT is well-formed")
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — VERIFICATION (formula/interpolation exactness against hand-computed
// reference points). NOT validation: no experimental-CHF-database comparison.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn p_pa(v: f64) -> Pressure {
        Pressure::new::<pascal>(v)
    }
    fn g(v: f64) -> MassFlux {
        MassFlux::new::<kilogram_per_square_meter_second>(v)
    }
    fn d_m(v: f64) -> Length {
        Length::new::<meter>(v)
    }
    fn hfg(v: f64) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(v)
    }
    fn wm2(q: HeatFluxDensity) -> f64 {
        q.get::<watt_per_square_meter>()
    }

    // ── Biasi (1967) ─────────────────────────────────────────────────────────

    /// Methodology: hand-evaluate the Biasi correlation (form/constants from
    /// Todreas & Kazimi, *Nuclear Systems I*, 2nd ed.) at P = 70 bar
    /// (7.0e6 Pa), G = 2000 kg/m²s, x = 0.2, D = 1 cm (0.01 m → n = 0.4,
    /// D_cm^n = 1). Steps:
    ///   f_p = 0.7249 + 0.099·70·exp(−0.032·70) = 0.7249 + 6.93·0.106459 = 1.462760
    ///   h_p = −1.159 + 0.149·70·exp(−0.019·70) + 8.99·70/(10+4900)
    ///       = −1.159 + 10.43·0.264477 + 0.128167 = 1.727664
    ///   G^{1/6} = 2000^{1/6} = 3.549601
    ///   q1 = (1.883e7/3.549601)·(1.462760/3.549601 − 0.2)
    ///      = 5.304822e6·0.212079 = 1.12504e6 W/m²
    ///   q2 = (3.78e7·1.727664/95.6516)·0.8 = 5.463e5 W/m²  (2000^0.6 = 95.6516)
    ///   G ≥ 300 ⇒ CHF = max(q1,q2) = q1 ≈ 1.125e6 W/m².
    /// Result (2026-07-23): code returns ≈ 1.12504e6 W/m² (rel err < 1e-3 vs the
    /// hand value 1.125e6). PASS. This verifies the algebra, not a benchmark.
    #[test]
    fn biasi_reference_point() {
        let q = Biasi::new()
            .critical_heat_flux(p_pa(7.0e6), g(2000.0), 0.2, d_m(0.01))
            .unwrap();
        assert_relative_eq!(wm2(q), 1.125e6, max_relative = 1e-3);
    }

    /// Biasi is in range at the reference point; out on pressure otherwise.
    /// Result (2026-07-23): in_valid_range true at 70 bar/2000/1cm; false at
    /// 200 bar. PASS.
    #[test]
    fn biasi_range_flag() {
        let b = Biasi::new();
        assert!(b.in_valid_range(p_pa(7.0e6), g(2000.0), 0.2, d_m(0.01)));
        assert!(!b.in_valid_range(p_pa(2.0e7), g(2000.0), 0.2, d_m(0.01)));
    }

    /// Non-physical inputs error (verification of the input guard).
    /// Result (2026-07-23): P≤0, G≤0, D≤0, x>1 each → Err. PASS.
    #[test]
    fn biasi_rejects_nonphysical() {
        let b = Biasi::new();
        assert!(b
            .critical_heat_flux(p_pa(-1.0), g(2000.0), 0.2, d_m(0.01))
            .is_err());
        assert!(b
            .critical_heat_flux(p_pa(7.0e6), g(0.0), 0.2, d_m(0.01))
            .is_err());
        assert!(b
            .critical_heat_flux(p_pa(7.0e6), g(2000.0), 0.2, d_m(0.0))
            .is_err());
        assert!(b
            .critical_heat_flux(p_pa(7.0e6), g(2000.0), 1.5, d_m(0.01))
            .is_err());
    }

    // ── W-3 / Tong (1967) ────────────────────────────────────────────────────

    /// Methodology: hand-evaluate the W-3 uniform-flux DNB correlation (Tong
    /// 1967; form from Todreas & Kazimi, *Nuclear Systems I*) at P = 2000 psia
    /// (1.3789514e7 Pa), G = 2.5e6 lbm/hr/ft² (3390.6 kg/m²s), D_e = 0.5 in
    /// (0.0127 m), x = 0, Δh_in = 0 (saturated inlet). Factors:
    ///   T1 = (2.022−0.0004302·2000) + (0.1722−0.0000984·2000)·exp(0)
    ///      = 1.1616 + (−0.0246) = 1.1370
    ///   T2 = (0.1484·2.5) + 1.037 = 1.408
    ///   T3 = 1.157
    ///   T4 = 0.2664 + 0.8357·exp(−3.151·0.5) = 0.2664 + 0.8357·0.206890 = 0.439299
    ///   T5 = 0.8258
    ///   q''/1e6 = 1.1370·1.408·1.157·0.439299·0.8258 = 0.672143 (×1e6 BTU/hr/ft²)
    ///   → 0.672143e6 · 3.154591 = 2.1203e6 W/m².
    /// Result (2026-07-23): code returns ≈ 2.1203e6 W/m² (rel err < 5e-3 vs hand
    /// value 2.120e6). PASS. Verifies the algebra + unit conversions.
    #[test]
    fn w3_reference_point() {
        let m = W3::new(hfg(0.0)); // saturated inlet: Δh_in = 0
        let p = p_pa(2000.0 * PA_PER_PSI);
        let gg = g(2.5e6 * KG_M2S_PER_LBM_HR_FT2);
        let dd = d_m(0.5 * M_PER_INCH);
        let q = m.critical_heat_flux(p, gg, 0.0, dd).unwrap();
        assert_relative_eq!(wm2(q), 2.120e6, max_relative = 5e-3);
    }

    /// The explicit-subcooling interface raises CHF above the saturated case,
    /// and equals the stored-subcooling result given the same value.
    /// Methodology: with Δh_in = 100 BTU/lbm (2.326e5 J/kg), the subcooling
    /// factor becomes 0.8258 + 0.000794·100 = 0.9052, i.e. ×(0.9052/0.8258).
    /// Result (2026-07-23): subcooled/saturated ratio = 0.9052/0.8258 = 1.09615
    /// (rel err < 1e-4); a W3 built with that stored subcooling matches the
    /// ChfSubCoolModel call (rel err < 1e-12). PASS.
    #[test]
    fn w3_subcooling_factor() {
        let sat = W3::new(hfg(0.0));
        let p = p_pa(2000.0 * PA_PER_PSI);
        let gg = g(2.5e6 * KG_M2S_PER_LBM_HR_FT2);
        let dd = d_m(0.5 * M_PER_INCH);
        let q_sat = wm2(sat.critical_heat_flux(p, gg, 0.0, dd).unwrap());
        let dh = hfg(100.0 * J_PER_KG_PER_BTU_LBM);
        let q_sub = wm2(sat
            .critical_heat_flux_subcooled(p, gg, 0.0, dd, dh)
            .unwrap());
        assert_relative_eq!(q_sub / q_sat, 0.9052 / 0.8258, max_relative = 1e-4);
        let stored = W3::new(dh);
        let q_stored = wm2(stored.critical_heat_flux(p, gg, 0.0, dd).unwrap());
        assert_relative_eq!(q_stored, q_sub, max_relative = 1e-12);
    }

    // ── Bowring (1972) ───────────────────────────────────────────────────────

    /// Methodology: hand-evaluate the Bowring dryout correlation (AEEW-R789;
    /// form from Todreas & Kazimi, *Nuclear Systems I*) at p_R = 1.0 exactly
    /// (P = 1/0.145 MPa = 6.8965517 MPa), where F1=F2=F3=F4=1 and n=1.5, with
    /// D = 0.01 m, G = 2000 kg/m²s, x = 0.2, h_fg = 1.505e6 J/kg. Steps:
    ///   A = 2.317·(1.505e6·0.01·2000/4)/(1+0.0143·0.1·2000)
    ///     = 2.317·7.525e6 / 3.86 = 1.7435425e7/3.86 = 4.517468e6
    ///   C = 0.077·0.01·2000 / (1 + 0.347·(2000/1356)^{1.5})
    ///     = 1.54 / (1 + 0.347·1.790895) = 1.54/1.621441 = 0.949775
    ///   num = A − 0.25·0.01·2000·1.505e6·0.2 = 4.517468e6 − 1.505e6 = 3.012468e6
    ///   q''_c = 3.012468e6 / 0.949775 = 3.17178e6 W/m².
    /// Result (2026-07-23): code returns ≈ 3.17178e6 W/m² (rel err < 1e-3 vs
    /// hand value 3.172e6). PASS. Verifies the algebra, not validation.
    #[test]
    fn bowring_reference_point() {
        let m = Bowring::new(hfg(1.505e6)).unwrap();
        let p = p_pa(6.896551724e6); // p_R = 0.145 · 6.8965517 = 1.0
        let q = m.critical_heat_flux(p, g(2000.0), 0.2, d_m(0.01)).unwrap();
        assert_relative_eq!(wm2(q), 3.172e6, max_relative = 1e-3);
    }

    /// Bowring rejects h_fg ≤ 0 at construction; valid-range flag behaves.
    /// Result (2026-07-23): new(0) → Err; in range at 6.9 MPa/2000/1cm, out at
    /// 25 MPa. PASS.
    #[test]
    fn bowring_construction_and_range() {
        assert!(Bowring::new(hfg(0.0)).is_err());
        let m = Bowring::new(hfg(1.505e6)).unwrap();
        assert!(m.in_valid_range(p_pa(6.9e6), g(2000.0), 0.2, d_m(0.01)));
        assert!(!m.in_valid_range(p_pa(2.5e7), g(2000.0), 0.2, d_m(0.01)));
    }

    // ── Groeneveld LUT: interpolation, diameter factor, CSV ──────────────────

    /// A LUT that is exactly linear in each axis, so trilinear interpolation is
    /// exact everywhere and its values are hand-predictable:
    /// value(P,G,x) = 1e6 + 0.1·P + 100·G + 1e6·x, reference D = 8 mm.
    fn linear_lut() -> GroeneveldLut {
        let pressures = vec![1.0e6, 2.0e6, 3.0e6];
        let mass_fluxes = vec![1000.0, 2000.0, 3000.0];
        let qualities = vec![0.0, 0.2, 0.4];
        let mut chf = Vec::new();
        for &p in &pressures {
            for &gg in &mass_fluxes {
                for &x in &qualities {
                    chf.push(1.0e6 + 0.1 * p + 100.0 * gg + 1.0e6 * x);
                }
            }
        }
        GroeneveldLut::new(pressures, mass_fluxes, qualities, chf, d_m(0.008)).unwrap()
    }

    /// Methodology: on the linear LUT the interpolation must be **exact at every
    /// node**. Check corner (P=1e6,G=1000,x=0) and interior node
    /// (P=2e6,G=2000,x=0.2):
    ///   node(1e6,1000,0)   = 1e6 + 1e5 + 1e5 + 0    = 1.2e6
    ///   node(2e6,2000,0.2) = 1e6 + 2e5 + 2e5 + 2e5  = 1.6e6
    /// Result (2026-07-23): lookup returns 1.2e6 and 1.6e6 exactly (rel err
    /// < 1e-12). PASS.
    #[test]
    fn lut_exact_at_nodes() {
        let lut = linear_lut();
        assert_relative_eq!(lut.lookup(1.0e6, 1000.0, 0.0), 1.2e6, max_relative = 1e-12);
        assert_relative_eq!(lut.lookup(2.0e6, 2000.0, 0.2), 1.6e6, max_relative = 1e-12);
    }

    /// Methodology: at the centre of the middle cell (P=1.5e6, G=1500, x=0.1),
    /// the linear function gives 1e6 + 0.1·1.5e6 + 100·1500 + 1e6·0.1
    /// = 1e6 + 1.5e5 + 1.5e5 + 1e5 = 1.4e6, and trilinear reproduces it exactly.
    /// Result (2026-07-23): lookup = 1.4e6 (rel err < 1e-12). PASS.
    #[test]
    fn lut_midpoint_is_exact_for_linear_data() {
        let lut = linear_lut();
        assert_relative_eq!(lut.lookup(1.5e6, 1500.0, 0.1), 1.4e6, max_relative = 1e-12);
    }

    /// Methodology: an out-of-range query clamps to axis bounds. Below all lows
    /// (P=0.5e6<1e6, G=500<1000, x=−0.5<0) must equal the low corner
    /// node(1e6,1000,0) = 1.2e6; above all highs equals node(3e6,3000,0.4)
    /// = 1e6 + 3e5 + 3e5 + 4e5 = 2.0e6.
    /// Result (2026-07-23): clamps give 1.2e6 and 2.0e6 (rel err < 1e-12);
    /// in_valid_range false for the out-of-range query. PASS.
    #[test]
    fn lut_clamps_out_of_range() {
        let lut = linear_lut();
        assert_relative_eq!(lut.lookup(0.5e6, 500.0, -0.5), 1.2e6, max_relative = 1e-12);
        assert_relative_eq!(lut.lookup(9.9e9, 9.9e6, 0.9), 2.0e6, max_relative = 1e-12);
        assert!(!lut.in_valid_range(p_pa(0.5e6), g(500.0), -0.5, d_m(0.008)));
    }

    /// Methodology: the diameter factor is K1 = (8mm/D)^{1/2}. At D = 8 mm,
    /// K1 = 1 (CHF unchanged); at D = 2 mm, K1 = (0.008/0.002)^{1/2} = 2. Apply
    /// via critical_heat_flux at an interior point and check the ratio to the
    /// raw 8 mm lookup.
    /// Result (2026-07-23): K1(8mm)=1.0, K1(2mm)=2.0 (rel err < 1e-12);
    /// CHF(2mm)/lookup = 2.0. PASS.
    #[test]
    fn lut_diameter_factor() {
        let lut = linear_lut();
        assert_relative_eq!(lut.diameter_factor(0.008), 1.0, max_relative = 1e-12);
        assert_relative_eq!(lut.diameter_factor(0.002), 2.0, max_relative = 1e-12);
        let base = lut.lookup(2.0e6, 2000.0, 0.2);
        let q = wm2(lut
            .critical_heat_flux(p_pa(2.0e6), g(2000.0), 0.2, d_m(0.002))
            .unwrap());
        assert_relative_eq!(q, 2.0 * base, max_relative = 1e-12);
    }

    /// Methodology (CSV round-trip): export the linear LUT to CSV, re-import,
    /// and confirm every node value and the reference diameter survive exactly.
    /// Result (2026-07-23): 27/27 nodes reproduced bit-for-bit (f64 shortest
    /// round-trip formatting); reference diameter 0.008 m preserved. PASS.
    #[test]
    fn lut_csv_round_trip() {
        let lut = linear_lut();
        let csv = lut.to_csv();
        let back = GroeneveldLut::from_csv(&csv).unwrap();
        assert_eq!(back.n_nodes(), lut.n_nodes());
        assert_relative_eq!(
            back.reference_diameter().get::<meter>(),
            0.008,
            max_relative = 1e-12
        );
        for &p in &[1.0e6, 2.0e6, 3.0e6] {
            for &gg in &[1000.0, 2000.0, 3000.0] {
                for &x in &[0.0, 0.2, 0.4] {
                    assert_eq!(back.lookup(p, gg, x), lut.lookup(p, gg, x));
                }
            }
        }
    }

    /// Malformed CSV is rejected (verification of the loader guards).
    /// Result (2026-07-23): wrong field count, non-numeric field, and an
    /// incomplete grid each → Err. PASS.
    #[test]
    fn lut_csv_rejects_malformed() {
        // Wrong number of fields.
        assert!(GroeneveldLut::from_csv("pressure_pa,a,b\n1,2,3\n").is_err());
        // Non-numeric field.
        let bad = "pressure_pa,mass_flux_kg_m2_s,quality,chf_w_m2\n1e6,1000,oops,2e6\n";
        assert!(GroeneveldLut::from_csv(bad).is_err());
        // Incomplete grid (2×2×2 needs 8 rows).
        let incomplete = "pressure_pa,mass_flux_kg_m2_s,quality,chf_w_m2\n\
             1e6,1000,0.0,1.0\n1e6,1000,0.2,2.0\n2e6,2000,0.0,3.0\n";
        assert!(GroeneveldLut::from_csv(incomplete).is_err());
    }

    /// The synthetic sample LUT is well-formed and usable through the enum.
    /// Result (2026-07-23): sample() builds 27 nodes; ChfCorrelation dispatch
    /// returns a finite positive CHF at an interior point. PASS.
    #[test]
    fn sample_lut_via_enum() {
        let corr = ChfCorrelation::GroeneveldLut(GroeneveldLut::sample());
        let q = corr
            .critical_heat_flux(p_pa(10.0e6), g(2000.0), 0.1, d_m(0.008))
            .unwrap();
        assert!(wm2(q).is_finite() && wm2(q) > 0.0);
    }

    /// Enum dispatch reaches each algebraic variant and matches the concrete
    /// model (verification that ChfCorrelation forwards correctly).
    /// Result (2026-07-23): Biasi and Bowring via the enum equal their direct
    /// calls (rel err < 1e-12). PASS.
    #[test]
    fn enum_dispatch_matches_concrete() {
        let gg = g(2000.0);
        let dd = d_m(0.01);

        let biasi = Biasi::new();
        let e = ChfCorrelation::Biasi(Biasi::new());
        assert_relative_eq!(
            wm2(e.critical_heat_flux(p_pa(7.0e6), gg, 0.1, dd).unwrap()),
            wm2(biasi.critical_heat_flux(p_pa(7.0e6), gg, 0.1, dd).unwrap()),
            max_relative = 1e-12
        );

        let bow = Bowring::new(hfg(1.5e6)).unwrap();
        let e = ChfCorrelation::Bowring(Bowring::new(hfg(1.5e6)).unwrap());
        assert_relative_eq!(
            wm2(e.critical_heat_flux(p_pa(6.9e6), gg, 0.1, dd).unwrap()),
            wm2(bow.critical_heat_flux(p_pa(6.9e6), gg, 0.1, dd).unwrap()),
            max_relative = 1e-12
        );
    }
}
