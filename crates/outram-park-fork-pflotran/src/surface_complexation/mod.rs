//! Electrostatic surface-complexation sorption models (bead op-gg7).
//!
//! This module extends the equilibrium sorption family in [`crate::sorption`]
//! (Kd / Langmuir / Freundlich isotherms and Gaines–Thomas ion exchange) with
//! **pH-dependent surface complexation**. Mineral surfaces such as **hydrous
//! ferric oxide (HFO)** expose amphoteric hydroxyl sites `>SOH` that protonate,
//! deprotonate, and bind aqueous metal cations. Because the reactions consume or
//! release `H+`, the amount of metal sorbed is a strong function of pH — the
//! characteristic sigmoidal *sorption edge* that isotherms cannot reproduce.
//!
//! ## Mass-action core (non-electrostatic model, NEM)
//!
//! Three surface reactions act on a single amphoteric site type. Square brackets
//! denote **surface-species concentrations in mol per litre of solution**; `{H+}`
//! and `{M}` are aqueous **activities** (here taken equal to molarity, `gamma =
//! 1`, matching the v1 [`crate::geochemistry`] core):
//!
//! ```text
//!   protonation    >SOH + H+  <=> >SOH2+          K_a1 = [>SOH2+] / ([>SOH]·{H+})
//!   deprotonation  >SOH       <=> >SO- + H+        K_a2 = [>SO-]·{H+} / [>SOH]
//!   metal binding  >SOH + M^z+ <=> >SOM^(z-1) + H+ K_M  = [>SOM]·{H+} / ([>SOH]·{M})
//! ```
//!
//! with the **site balance** (total site density conserved)
//!
//! ```text
//!   T = [>SOH] + [>SOH2+] + [>SO-] + [>SOM].
//! ```
//!
//! Every surface species is proportional to `[>SOH]`, so for the NEM the balance
//! is a single *linear* equation with the closed form
//!
//! ```text
//!   [>SOH] = T / (1 + K_a1·{H+} + K_a2/{H+} + K_M·{M}/{H+}),
//! ```
//! and no iteration is needed.
//!
//! ## Electrostatic correction (CCM and DLM)
//!
//! When surface charge develops, moving an ion of charge `z_i` through the
//! near-surface potential `psi` (volts) costs coulombic work, so each intrinsic
//! constant is corrected by a **Boltzmann factor**. Writing the change in the
//! *surface species'* charge across a reaction as `Δz`, the apparent
//! (conditional) constant is `K_app = K_int · exp(-Δz·F·psi/(R·T))`. Introducing
//! the master factor
//!
//! ```text
//!   P = exp(-F·psi/(R·T)),
//! ```
//! the three surface-species ratios (relative to `[>SOH]`) become
//!
//! ```text
//!   [>SOH2+]/[>SOH] = K_a1·{H+}·P^(+1)      (Δz = +1)
//!   [>SO-]  /[>SOH] = K_a2/{H+}·P^(-1)      (Δz = -1)
//!   [>SOM]  /[>SOH] = K_M·{M}/{H+}·P^(z-1)  (Δz = z-1)
//! ```
//!
//! The **surface charge density** `sigma` (C/m^2) follows from the net surface
//! charge per litre and the surface area per litre `A·Cs`:
//!
//! ```text
//!   Q      = [>SOH2+] - [>SO-] + (z-1)·[>SOM]        (mol charge / L)
//!   sigma  = Q·F / (A·Cs)                            (C/m^2)
//! ```
//! where `A` is the specific surface area (m^2/g) and `Cs` the solid
//! concentration (g/L), so `A·Cs` has units m^2 per litre of solution.
//!
//! Two electrostatic closures relate `sigma` and `psi`, solved self-consistently:
//!
//! - **Constant-capacitance model (CCM):** `sigma = C · psi`, i.e. `psi =
//!   sigma/C`, with the capacitance `C` in F/m^2. Appropriate for high, constant
//!   ionic strength.
//! - **Diffuse-layer model (DLM):** the Gouy–Chapman relation, in the form used
//!   by Dzombak & Morel (1990) for a symmetric 1:1 electrolyte at 25 °C,
//!   ```text
//!     sigma = 0.1174 · sqrt(I) · sinh(F·psi/(2·R·T)),
//!   ```
//!   where `I` is the ionic strength in mol/L. The prefactor `0.1174` C/m^2 is
//!   `sqrt(8·R·T·epsilon·epsilon0·1000)` evaluated at `T = 298.15 K` with the
//!   permittivity of water, so it is only valid at 25 °C.
//!
//! ## Solver and convergence handling
//!
//! For the NEM the speciation is the closed form above (`psi = 0`). For CCM and
//! DLM the residual
//!
//! ```text
//!   f(psi) = sigma(psi) - closure(psi)
//! ```
//! is **strictly monotone decreasing** in `psi`: `sigma(psi)` decreases as `psi`
//! rises (a more positive potential drives off positive charge), while both
//! closures (`C·psi` and the `sinh` term) increase. A monotone residual has a
//! unique root, which we find by **bracketing then bisection** — expand a small
//! symmetric interval about `psi = 0` until the residual changes sign, then
//! bisect. Bisection cannot diverge once a bracket exists, so it is used in
//! preference to a Newton step that could overshoot the stiff exponential. If a
//! bracket cannot be found inside a physical bound (±5 V, far outside any real
//! surface potential), a [`PflotranError::Convergence`] is returned rather than a
//! panic. The search bound also keeps every exponent finite, so no `NaN`/`inf`
//! escapes.
//!
//! ## Point of zero charge
//!
//! Ignoring metal, the surface is neutral when `[>SOH2+] = [>SO-]`, i.e.
//! `K_a1·{H+} = K_a2/{H+}`, giving `{H+} = sqrt(K_a2/K_a1)` and
//!
//! ```text
//!   pH_pzc = 0.5·(log10 K_a1 - log10 K_a2).
//! ```
//! With the sign convention here (`log_k_a2` is the deprotonation constant, a
//! *negative* number for an acidic site) this equals the midpoint of the two
//! `pKa`-style magnitudes. See [`SurfaceSite::point_of_zero_charge_ph`].
//!
//! ## Units summary
//!
//! | Symbol   | Meaning                              | Unit           |
//! |----------|--------------------------------------|----------------|
//! | `{H+}`   | proton activity (`10^-pH`)           | mol/L          |
//! | `{M}`    | aqueous metal activity               | mol/L          |
//! | `[>S…]`  | surface-species concentration        | mol/L          |
//! | `T`      | total site density                   | mol/L          |
//! | `K_a1`   | protonation constant                 | L/mol          |
//! | `K_a2`   | deprotonation constant               | mol/L          |
//! | `K_M`    | metal-binding constant               | dimensionless* |
//! | `A`      | specific surface area                | m^2/g          |
//! | `Cs`     | solid concentration                  | g/L            |
//! | `C`      | CCM capacitance                      | F/m^2          |
//! | `I`      | ionic strength (DLM)                 | mol/L          |
//! | `psi`    | surface potential                    | V              |
//! | `sigma`  | surface charge density               | C/m^2          |
//!
//! (*`K_M` as written groups `{M}` and `{H+}` so it is dimensionless for a
//! `1:1` proton exchange; the exact dimensionality depends on the reaction
//! stoichiometry chosen.)
//!
//! ## Scope and human-review flags
//!
//! - **Single site type, single metal.** One amphoteric `>SOH` site and one
//!   metal complex are modelled. Multi-site (strong/weak HFO) and competitive
//!   multi-metal systems are **not** implemented here.
//! - **25 °C only.** The Faraday/`RT` factor and the DLM `0.1174` prefactor are
//!   evaluated at `T = 298.15 K`. No temperature dependence.
//! - **Equilibrium, ideal activities.** Local instantaneous equilibrium;
//!   concentrations used directly as activities (`gamma = 1`). No triple-layer
//!   model, no basic-Stern layer.
//! - **Untrusted AI-generated draft** until a human reviews it, per the
//!   workspace `RESPONSIBLE_USE.md`. The tests below check physical *limits*
//!   (charge sign vs pH, edge shape, mass conservation), **not** published
//!   PFLOTRAN / FITEQL reference numbers — this is verification, not validation.
//!
//! ## Provenance
//!
//! - D. A. Dzombak & F. M. M. Morel, *Surface Complexation Modeling: Hydrous
//!   Ferric Oxide*, Wiley-Interscience (1990) — the diffuse-layer model,
//!   the `0.1174` C/m^2 prefactor, and the HFO reaction set.
//! - J. A. Davis & D. B. Kent, "Surface complexation modeling in aqueous
//!   geochemistry," *Reviews in Mineralogy* 23 (1990) 177 — CCM/DLM/TLM overview.
//! - G. Sposito, *The Surface Chemistry of Natural Particles*, Oxford (2004).

use crate::error::PflotranError;

/// Faraday constant `F`, in C/mol (CODATA).
const FARADAY: f64 = 96485.33212;
/// Molar gas constant `R`, in J/(mol·K) (CODATA).
const GAS_CONSTANT: f64 = 8.314462618;
/// Reference temperature, 25 °C, in kelvin.
const TEMPERATURE_K: f64 = 298.15;
/// `F/(R·T)` at 25 °C, in V^-1 (≈ 38.92). Multiplies a potential (volts) to
/// give the dimensionless Boltzmann exponent.
const F_OVER_RT: f64 = FARADAY / (GAS_CONSTANT * TEMPERATURE_K);
/// Diffuse-layer (Gouy–Chapman) prefactor `sqrt(8·R·T·eps·eps0·1000)` at 25 °C,
/// in C/m^2, with the ionic strength `I` supplied in mol/L (Dzombak & Morel 1990).
const DLM_PREFACTOR: f64 = 0.1174;
/// Largest surface potential (in volts, either sign) the bracket search will
/// explore. Physical surface potentials never approach this; the bound also
/// keeps every `exp` argument finite so no `NaN`/`inf` can escape the solver.
const PSI_BOUND: f64 = 5.0;

/// Choice of electrostatic closure for the surface-complexation solve.
///
/// Enum dispatch (no trait objects) per the workspace design rules. The physics
/// difference is only in how surface charge density `sigma` relates to surface
/// potential `psi`; the mass-action core is shared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceComplexationModel {
    /// Non-electrostatic model — no coulombic correction (`psi = 0`, `P = 1`).
    ///
    /// The intrinsic constants act directly; the site balance is the linear
    /// closed form. Appropriate as a first approximation or at conditions where
    /// electrostatics are negligible.
    NonElectrostatic,

    /// Constant-capacitance model: `sigma = C · psi`.
    ///
    /// Suited to high, roughly constant ionic strength. `capacitance` is `C` in
    /// **F/m^2** and must be strictly positive.
    ConstantCapacitance {
        /// Surface capacitance `C`, in F/m^2 (`> 0`).
        capacitance: f64,
    },

    /// Diffuse-layer (Gouy–Chapman) model:
    /// `sigma = 0.1174·sqrt(I)·sinh(F·psi/(2·R·T))` at 25 °C.
    ///
    /// `ionic_strength` is `I` in **mol/L** and must be strictly positive.
    DiffuseLayer {
        /// Bulk ionic strength `I`, in mol/L (`> 0`).
        ionic_strength: f64,
    },
}

impl SurfaceComplexationModel {
    /// Validate the model parameters.
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if the capacitance (CCM) or ionic
    /// strength (DLM) is non-finite or not strictly positive.
    pub fn validate(&self) -> Result<(), PflotranError> {
        match *self {
            SurfaceComplexationModel::NonElectrostatic => Ok(()),
            SurfaceComplexationModel::ConstantCapacitance { capacitance } => {
                if !capacitance.is_finite() || capacitance <= 0.0 {
                    return Err(PflotranError::InvalidInput(format!(
                        "CCM capacitance must be finite and > 0 (F/m^2), got {capacitance}"
                    )));
                }
                Ok(())
            }
            SurfaceComplexationModel::DiffuseLayer { ionic_strength } => {
                if !ionic_strength.is_finite() || ionic_strength <= 0.0 {
                    return Err(PflotranError::InvalidInput(format!(
                        "DLM ionic strength must be finite and > 0 (mol/L), got {ionic_strength}"
                    )));
                }
                Ok(())
            }
        }
    }
}

/// A single amphoteric surface-site type with its protonation constants,
/// site density, and the surface metrics needed for the electrostatic models.
///
/// All fields are public so a caller can build one literally; the validating
/// constructor [`SurfaceSite::new`] is preferred as it rejects non-physical
/// values, and [`SurfaceSite::speciate`] re-validates defensively.
///
/// # Units and assumptions
/// - `log_k_a1`, `log_k_a2` are base-10 logs of the intrinsic protonation /
///   deprotonation constants. `log_k_a2` is typically **negative** (acidic site).
/// - `site_density` is the **total** `>SOH`-equivalent site concentration in
///   **mol per litre of solution** (`> 0`).
/// - `specific_surface_area` (m^2/g) and `solid_concentration` (g/L) enter only
///   through their product `A·Cs` (m^2/L), used to convert net surface charge to
///   `sigma`. They are required (`> 0`) for CCM/DLM but ignored for the NEM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSite {
    /// `log10 K_a1` for `>SOH + H+ = >SOH2+` (protonation; usually positive).
    pub log_k_a1: f64,
    /// `log10 K_a2` for `>SOH = >SO- + H+` (deprotonation; usually negative).
    pub log_k_a2: f64,
    /// Total site density `T`, in mol sites per litre of solution (`> 0`).
    pub site_density: f64,
    /// Specific surface area `A`, in m^2/g (used for `sigma`; see module docs).
    pub specific_surface_area: f64,
    /// Solid concentration `Cs`, in g/L.
    pub solid_concentration: f64,
}

/// A surface metal complex `>SOH + M^z+ = >SOM^(z-1) + H+`.
///
/// `log_k` is `log10 K_M` of the intrinsic binding constant, and `metal_charge`
/// is the aqueous cation charge `z` (`> 0`), which sets both the product complex
/// charge `z-1` and the coulombic exponent in the electrostatic models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceComplex {
    /// `log10 K_M` of the intrinsic metal-binding constant.
    pub log_k: f64,
    /// Aqueous metal cation charge `z` (`> 0`, e.g. `2.0` for `Zn^2+`).
    pub metal_charge: f64,
}

impl SurfaceComplex {
    /// Construct and validate a [`SurfaceComplex`].
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if `log_k` is non-finite, or
    /// `metal_charge` is non-finite or not strictly positive.
    pub fn new(log_k: f64, metal_charge: f64) -> Result<Self, PflotranError> {
        if !log_k.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "surface-complex log_k must be finite, got {log_k}"
            )));
        }
        if !metal_charge.is_finite() || metal_charge <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "surface-complex metal_charge must be finite and > 0, got {metal_charge}"
            )));
        }
        Ok(SurfaceComplex {
            log_k,
            metal_charge,
        })
    }

    /// Validate an already-built complex (used defensively by `speciate`).
    fn validate(&self) -> Result<(), PflotranError> {
        SurfaceComplex::new(self.log_k, self.metal_charge).map(|_| ())
    }
}

/// The equilibrium surface speciation at a given pH and aqueous metal activity.
///
/// All four species concentrations are in **mol per litre of solution** and, by
/// construction, sum to the site density `T` (mass conservation). The surface
/// potential is in volts (`0` for the non-electrostatic model).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSpeciation {
    /// Neutral site `[>SOH]`, mol/L.
    pub soh: f64,
    /// Protonated site `[>SOH2+]`, mol/L (positive charge).
    pub soh2: f64,
    /// Deprotonated site `[>SO-]`, mol/L (negative charge).
    pub so_minus: f64,
    /// Metal-bound site `[>SOM^(z-1)]`, mol/L.
    pub som: f64,
    /// Self-consistent surface potential `psi`, in volts (`0` for the NEM).
    pub surface_potential: f64,
    /// Fraction of total metal (`aqueous + sorbed`) held on the surface,
    /// `[>SOM] / ({M} + [>SOM])`, dimensionless in `[0, 1]`.
    pub fraction_metal_sorbed: f64,
}

impl SurfaceSpeciation {
    /// Net surface charge, in **mol of charge per litre**:
    /// `[>SOH2+] - [>SO-] + (z-1)·[>SOM]`.
    ///
    /// `metal_charge` is the aqueous cation charge `z` used to weight the
    /// metal-bound site's residual charge `z-1`. Positive below the point of
    /// zero charge, negative above it.
    pub fn net_surface_charge(&self, metal_charge: f64) -> f64 {
        self.soh2 - self.so_minus + (metal_charge - 1.0) * self.som
    }
}

impl SurfaceSite {
    /// Construct and validate a [`SurfaceSite`].
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] if any log-K is non-finite, `site_density`
    /// is non-finite or `<= 0`, or the surface-area / solid-concentration inputs
    /// are non-finite or negative.
    pub fn new(
        log_k_a1: f64,
        log_k_a2: f64,
        site_density: f64,
        specific_surface_area: f64,
        solid_concentration: f64,
    ) -> Result<Self, PflotranError> {
        let site = SurfaceSite {
            log_k_a1,
            log_k_a2,
            site_density,
            specific_surface_area,
            solid_concentration,
        };
        site.validate()?;
        Ok(site)
    }

    /// Validate the intrinsic (model-independent) site parameters.
    fn validate(&self) -> Result<(), PflotranError> {
        if !self.log_k_a1.is_finite() || !self.log_k_a2.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "surface-site log_k_a1/log_k_a2 must be finite, got {}/{}",
                self.log_k_a1, self.log_k_a2
            )));
        }
        if !self.site_density.is_finite() || self.site_density <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "site_density must be finite and > 0 (mol/L), got {}",
                self.site_density
            )));
        }
        if !self.specific_surface_area.is_finite() || self.specific_surface_area < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "specific_surface_area must be finite and >= 0 (m^2/g), got {}",
                self.specific_surface_area
            )));
        }
        if !self.solid_concentration.is_finite() || self.solid_concentration < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "solid_concentration must be finite and >= 0 (g/L), got {}",
                self.solid_concentration
            )));
        }
        Ok(())
    }

    /// Analytical point of zero charge (pH), `0.5·(log_k_a1 - log_k_a2)`.
    ///
    /// The pH at which `[>SOH2+] = [>SO-]` so the bare surface (no metal) carries
    /// zero net charge. See the module-level derivation.
    pub fn point_of_zero_charge_ph(&self) -> f64 {
        0.5 * (self.log_k_a1 - self.log_k_a2)
    }

    /// Boltzmann master factor `P = exp(-F·psi/(R·T))`.
    fn boltzmann_p(psi: f64) -> f64 {
        (-F_OVER_RT * psi).exp()
    }

    /// Surface species `([>SOH], [>SOH2+], [>SO-], [>SOM])` (mol/L) at a given
    /// proton activity `h`, metal activity `m`, complex, and potential `psi`.
    ///
    /// Uses the ratio form (every species proportional to `[>SOH]`) so the site
    /// balance is enforced exactly: the four returned values sum to
    /// `site_density` up to floating-point rounding.
    fn species_at(
        &self,
        h: f64,
        m: f64,
        complex: &SurfaceComplex,
        psi: f64,
    ) -> (f64, f64, f64, f64) {
        let p = Self::boltzmann_p(psi);
        let k_a1 = 10.0_f64.powf(self.log_k_a1);
        let k_a2 = 10.0_f64.powf(self.log_k_a2);
        let k_m = 10.0_f64.powf(complex.log_k);
        let z = complex.metal_charge;

        // Ratios of each species to [>SOH].
        let r_soh2 = k_a1 * h * p; // Δz = +1
        let r_so = k_a2 / h / p; //   Δz = -1
        let r_som = if m > 0.0 {
            k_m * m / h * p.powf(z - 1.0) // Δz = z-1
        } else {
            0.0
        };

        let denom = 1.0 + r_soh2 + r_so + r_som;
        let soh = self.site_density / denom;
        (soh, soh * r_soh2, soh * r_so, soh * r_som)
    }

    /// Surface charge density `sigma` (C/m^2) at potential `psi`.
    ///
    /// `sigma = Q·F/(A·Cs)` with `Q` the net surface charge per litre. Requires
    /// `A·Cs > 0`; callers ensure that before invoking (electrostatic path only).
    fn surface_charge_density(&self, h: f64, m: f64, complex: &SurfaceComplex, psi: f64) -> f64 {
        let (_, soh2, so_minus, som) = self.species_at(h, m, complex, psi);
        let q = soh2 - so_minus + (complex.metal_charge - 1.0) * som; // mol charge / L
        let area_per_litre = self.specific_surface_area * self.solid_concentration; // m^2/L
        q * FARADAY / area_per_litre
    }

    /// Solve the self-consistent surface potential for an electrostatic model.
    ///
    /// Returns the unique root of the monotone-decreasing residual
    /// `f(psi) = sigma(psi) - closure(psi)`, found by bracket-then-bisection.
    ///
    /// # Errors
    /// [`PflotranError::Convergence`] if no sign change is found within
    /// `±PSI_BOUND` volts (not expected for any physical input).
    fn solve_potential(
        &self,
        h: f64,
        m: f64,
        complex: &SurfaceComplex,
        model: SurfaceComplexationModel,
    ) -> Result<f64, PflotranError> {
        let closure = |psi: f64| -> f64 {
            match model {
                SurfaceComplexationModel::ConstantCapacitance { capacitance } => capacitance * psi,
                SurfaceComplexationModel::DiffuseLayer { ionic_strength } => {
                    DLM_PREFACTOR * ionic_strength.sqrt() * (0.5 * F_OVER_RT * psi).sinh()
                }
                // NEM never reaches the solver.
                SurfaceComplexationModel::NonElectrostatic => 0.0,
            }
        };
        let residual =
            |psi: f64| -> f64 { self.surface_charge_density(h, m, complex, psi) - closure(psi) };

        // Bracket: f is strictly decreasing, so we want lo (f > 0) and hi (f < 0).
        let mut lo = -0.02_f64;
        let mut hi = 0.02_f64;
        let mut f_lo = residual(lo);
        let mut f_hi = residual(hi);
        let mut expand = 0;
        while f_lo * f_hi > 0.0 {
            lo = (lo * 1.6).max(-PSI_BOUND);
            hi = (hi * 1.6).min(PSI_BOUND);
            f_lo = residual(lo);
            f_hi = residual(hi);
            expand += 1;
            if (lo <= -PSI_BOUND && hi >= PSI_BOUND) || expand > 80 {
                if f_lo * f_hi > 0.0 {
                    return Err(PflotranError::Convergence(format!(
                        "surface potential could not be bracketed within ±{PSI_BOUND} V \
                         (f({lo})={f_lo}, f({hi})={f_hi})"
                    )));
                }
                break;
            }
        }

        // Bisection on the monotone-decreasing residual: f(lo) > 0, f(hi) < 0.
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let f_mid = residual(mid);
            if f_mid.abs() < 1.0e-15 || (hi - lo) < 1.0e-13 {
                return Ok(mid);
            }
            if f_mid > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let mid = 0.5 * (lo + hi);
        if residual(mid).abs() < 1.0e-6 {
            Ok(mid)
        } else {
            Err(PflotranError::Convergence(format!(
                "surface-potential bisection did not converge (residual {})",
                residual(mid)
            )))
        }
    }

    /// Solve the equilibrium surface speciation at a given pH and aqueous metal
    /// molarity for the chosen model and metal complex.
    ///
    /// # Parameters
    /// - `ph`: solution pH; the proton activity is `{H+} = 10^-pH`.
    /// - `metal_molarity`: aqueous metal activity `{M}`, mol/L (`>= 0`; `0` means
    ///   no metal, giving `fraction_metal_sorbed = 0`).
    /// - `model`: electrostatic closure ([`SurfaceComplexationModel`]).
    /// - `complex`: the metal surface complex ([`SurfaceComplex`]).
    ///
    /// # Returns
    /// A [`SurfaceSpeciation`] whose four species sum to `site_density`.
    ///
    /// # Errors
    /// - [`PflotranError::InvalidInput`] for non-physical site / model / complex
    ///   parameters, a non-finite pH, a negative or non-finite metal molarity,
    ///   or (for CCM/DLM) a zero surface area / solid concentration.
    /// - [`PflotranError::Convergence`] if the electrostatic potential solve
    ///   fails to bracket a root (not expected for physical input).
    pub fn speciate(
        &self,
        ph: f64,
        metal_molarity: f64,
        model: SurfaceComplexationModel,
        complex: &SurfaceComplex,
    ) -> Result<SurfaceSpeciation, PflotranError> {
        self.validate()?;
        model.validate()?;
        complex.validate()?;
        if !ph.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "pH must be finite, got {ph}"
            )));
        }
        if !metal_molarity.is_finite() || metal_molarity < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "metal_molarity must be finite and >= 0 (mol/L), got {metal_molarity}"
            )));
        }

        let h = 10.0_f64.powf(-ph);
        let m = metal_molarity;

        let psi = match model {
            SurfaceComplexationModel::NonElectrostatic => 0.0,
            _ => {
                let area_per_litre = self.specific_surface_area * self.solid_concentration;
                if !(area_per_litre > 0.0) {
                    return Err(PflotranError::InvalidInput(
                        "electrostatic models need specific_surface_area > 0 and \
                         solid_concentration > 0 to define a surface charge density"
                            .to_string(),
                    ));
                }
                self.solve_potential(h, m, complex, model)?
            }
        };

        let (soh, soh2, so_minus, som) = self.species_at(h, m, complex, psi);
        if !(soh.is_finite() && soh2.is_finite() && so_minus.is_finite() && som.is_finite()) {
            return Err(PflotranError::Convergence(
                "surface speciation produced a non-finite concentration".to_string(),
            ));
        }

        let total_metal = m + som;
        let fraction_metal_sorbed = if total_metal > 0.0 {
            som / total_metal
        } else {
            0.0
        };

        Ok(SurfaceSpeciation {
            soh,
            soh2,
            so_minus,
            som,
            surface_potential: psi,
            fraction_metal_sorbed,
        })
    }

    /// Compute a **sorption edge**: the fraction of metal sorbed at each pH in
    /// `ph_values`, holding the aqueous metal molarity fixed.
    ///
    /// For cation sorption this is the classic sigmoidal S-curve that rises with
    /// pH. Returns `(pH, fraction)` pairs in the input order.
    ///
    /// # Errors
    /// Propagates any error from [`SurfaceSite::speciate`] (the first failing pH
    /// aborts the sweep).
    pub fn sorption_edge(
        &self,
        ph_values: &[f64],
        metal_molarity: f64,
        model: SurfaceComplexationModel,
        complex: &SurfaceComplex,
    ) -> Result<Vec<(f64, f64)>, PflotranError> {
        let mut out = Vec::with_capacity(ph_values.len());
        for &ph in ph_values {
            let sp = self.speciate(ph, metal_molarity, model, complex)?;
            out.push((ph, sp.fraction_metal_sorbed));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative HFO-like site: strongly protonating (log_k_a1 = 7),
    /// weakly acidic deprotonation (log_k_a2 = -9) → PZC at pH 8.
    fn hfo_site() -> SurfaceSite {
        SurfaceSite::new(7.0, -9.0, 1.0e-3, 600.0, 1.0).unwrap()
    }

    /// A divalent metal complex placing the NEM edge in the mid-pH range.
    fn metal_complex() -> SurfaceComplex {
        SurfaceComplex::new(-3.0, 2.0).unwrap()
    }

    #[test]
    fn protonation_charge_sign_flips_across_pzc() {
        let site = hfo_site();
        let cx = metal_complex();
        let nem = SurfaceComplexationModel::NonElectrostatic;

        // No metal — bare amphoteric surface.
        let low = site.speciate(3.0, 0.0, nem, &cx).unwrap();
        assert!(
            low.soh2 > 1.0e6 * low.so_minus,
            "at low pH the surface should be dominated by >SOH2+ (positive): \
             soh2={}, so_minus={}",
            low.soh2,
            low.so_minus
        );
        assert!(low.net_surface_charge(cx.metal_charge) > 0.0);

        let high = site.speciate(13.0, 0.0, nem, &cx).unwrap();
        assert!(
            high.so_minus > 1.0e6 * high.soh2,
            "at high pH the surface should be dominated by >SO- (negative): \
             soh2={}, so_minus={}",
            high.soh2,
            high.so_minus
        );
        assert!(high.net_surface_charge(cx.metal_charge) < 0.0);

        // Net charge ~ 0 at the analytical PZC (= 0.5*(7 - (-9)) = 8).
        let pzc = site.point_of_zero_charge_ph();
        assert!((pzc - 8.0).abs() < 1e-12, "pzc = {pzc}");
        let at_pzc = site.speciate(pzc, 0.0, nem, &cx).unwrap();
        assert!(
            (at_pzc.soh2 - at_pzc.so_minus).abs() < 1e-9 * site.site_density,
            "net charge should vanish at the PZC: soh2={}, so_minus={}",
            at_pzc.soh2,
            at_pzc.so_minus
        );
    }

    #[test]
    fn sorption_edge_is_rising_sigmoid_crossing_half() {
        let site = hfo_site();
        let cx = metal_complex();
        let nem = SurfaceComplexationModel::NonElectrostatic;
        let phs: Vec<f64> = (3..=10).map(|p| p as f64).collect();
        let edge = site.sorption_edge(&phs, 1.0e-6, nem, &cx).unwrap();

        // Monotone increasing in pH.
        for w in edge.windows(2) {
            assert!(
                w[1].1 >= w[0].1 - 1e-12,
                "edge must be non-decreasing: pH {} -> {}, frac {} -> {}",
                w[0].0,
                w[1].0,
                w[0].1,
                w[1].1
            );
        }
        // Low-pH tail near 0, high-pH plateau near 1, and it crosses 0.5.
        assert!(
            edge.first().unwrap().1 < 0.05,
            "low-pH tail {}",
            edge.first().unwrap().1
        );
        assert!(
            edge.last().unwrap().1 > 0.9,
            "high-pH plateau {}",
            edge.last().unwrap().1
        );
        let crosses = edge.iter().any(|&(_, f)| f > 0.5) && edge.iter().any(|&(_, f)| f < 0.5);
        assert!(
            crosses,
            "edge must cross 0.5 at an intermediate pH: {edge:?}"
        );
    }

    #[test]
    fn site_balance_conserved_all_models() {
        let site = hfo_site();
        let cx = metal_complex();
        let models = [
            SurfaceComplexationModel::NonElectrostatic,
            SurfaceComplexationModel::ConstantCapacitance { capacitance: 1.0 },
            SurfaceComplexationModel::DiffuseLayer {
                ionic_strength: 0.1,
            },
        ];
        for model in models {
            for &ph in &[3.0, 5.0, 7.0, 9.0, 11.0] {
                let sp = site.speciate(ph, 1.0e-5, model, &cx).unwrap();
                let sum = sp.soh + sp.soh2 + sp.so_minus + sp.som;
                assert!(
                    (sum - site.site_density).abs() < 1e-12 * site.site_density,
                    "site balance violated for {model:?} at pH {ph}: sum={sum}, T={}",
                    site.site_density
                );
                // Every species non-negative.
                assert!(sp.soh >= 0.0 && sp.soh2 >= 0.0 && sp.so_minus >= 0.0 && sp.som >= 0.0);
                assert!((0.0..=1.0).contains(&sp.fraction_metal_sorbed));
            }
        }
    }

    #[test]
    fn electrostatics_shift_the_edge_and_both_converge() {
        let site = hfo_site();
        let cx = metal_complex();
        let phs: Vec<f64> = (3..=11).map(|p| p as f64).collect();
        let nem = site
            .sorption_edge(
                &phs,
                1.0e-6,
                SurfaceComplexationModel::NonElectrostatic,
                &cx,
            )
            .unwrap();
        let ccm = site
            .sorption_edge(
                &phs,
                1.0e-6,
                SurfaceComplexationModel::ConstantCapacitance { capacitance: 1.0 },
                &cx,
            )
            .unwrap();

        // Both edges are valid monotone sigmoids (both converged).
        for edge in [&nem, &ccm] {
            for w in edge.windows(2) {
                assert!(w[1].1 >= w[0].1 - 1e-9, "edge must be non-decreasing");
            }
        }

        // Electrostatics change the edge measurably. For a divalent cation the
        // positive surface charge below the PZC opposes cation uptake via the
        // Boltzmann term, so the CCM sorbs less than the NEM in the rising
        // region (edge shifted to higher pH). Assert a detectable, signed shift.
        let max_abs_diff = nem
            .iter()
            .zip(ccm.iter())
            .map(|(a, b)| (a.1 - b.1).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_abs_diff > 1e-3,
            "CCM should shift the edge measurably vs NEM (max |Δfrac| = {max_abs_diff})"
        );
        // Suppression check at pH 7 (below the PZC of 8, in the rising region).
        let f_nem = site
            .speciate(7.0, 1.0e-6, SurfaceComplexationModel::NonElectrostatic, &cx)
            .unwrap()
            .fraction_metal_sorbed;
        let f_ccm = site
            .speciate(
                7.0,
                1.0e-6,
                SurfaceComplexationModel::ConstantCapacitance { capacitance: 1.0 },
                &cx,
            )
            .unwrap()
            .fraction_metal_sorbed;
        assert!(
            f_ccm < f_nem,
            "CCM (positive surface, z=2) should suppress cation sorption below the PZC: \
             f_ccm={f_ccm}, f_nem={f_nem}"
        );
    }

    #[test]
    fn dlm_potential_negative_at_high_ph() {
        // This test isolates the *protonation* electrostatics, so it uses the
        // BARE amphoteric surface (metal_molarity = 0). With a metal present the
        // net surface charge is protonation charge PLUS the charge of the sorbed
        // >SOM^(z-1) complex; for the divalent test metal (z=2, log_k=-3) the
        // positively charged >SOM+ that forms at high pH offsets the deprotonated
        // >SO-, driving the net charge (and psi) back toward zero — a correct
        // model result, but one that masks the sign of the protonation charge the
        // diffuse layer responds to here. On the bare surface the sign is clean:
        // above the PZC the surface is negatively charged, so psi < 0; below it,
        // psi > 0.
        let site = hfo_site();
        let cx = metal_complex();
        let sp = site
            .speciate(
                11.0,
                0.0,
                SurfaceComplexationModel::DiffuseLayer {
                    ionic_strength: 0.01,
                },
                &cx,
            )
            .unwrap();
        assert!(
            sp.surface_potential < 0.0,
            "DLM potential should be negative above the PZC, got {}",
            sp.surface_potential
        );
        // And positive below the PZC.
        let sp_low = site
            .speciate(
                5.0,
                0.0,
                SurfaceComplexationModel::DiffuseLayer {
                    ionic_strength: 0.01,
                },
                &cx,
            )
            .unwrap();
        assert!(
            sp_low.surface_potential > 0.0,
            "DLM potential should be positive below the PZC, got {}",
            sp_low.surface_potential
        );
    }

    #[test]
    fn dilute_metal_fraction_is_independent_of_concentration() {
        // In the linear (Henry) regime the sorbed fraction depends only on pH,
        // not on the (small) total metal, because [>SOM] ∝ {M} and the sites are
        // not appreciably occupied.
        let site = hfo_site();
        let cx = metal_complex();
        let nem = SurfaceComplexationModel::NonElectrostatic;
        let f1 = site
            .speciate(7.0, 1.0e-9, nem, &cx)
            .unwrap()
            .fraction_metal_sorbed;
        let f2 = site
            .speciate(7.0, 1.0e-10, nem, &cx)
            .unwrap()
            .fraction_metal_sorbed;
        assert!(
            (f1 - f2).abs() < 1e-4,
            "dilute-limit fraction should be concentration-independent: {f1} vs {f2}"
        );
        assert!(
            f1 > 0.0 && f1 < 1.0,
            "fraction should be a genuine partial: {f1}"
        );
    }

    #[test]
    fn input_validation_rejects_bad_parameters() {
        let cx = metal_complex();
        let nem = SurfaceComplexationModel::NonElectrostatic;

        // Negative / zero site density.
        assert!(SurfaceSite::new(7.0, -9.0, -1.0e-3, 600.0, 1.0).is_err());
        assert!(SurfaceSite::new(7.0, -9.0, 0.0, 600.0, 1.0).is_err());
        // Negative surface metrics.
        assert!(SurfaceSite::new(7.0, -9.0, 1.0e-3, -600.0, 1.0).is_err());
        assert!(SurfaceSite::new(7.0, -9.0, 1.0e-3, 600.0, -1.0).is_err());
        // Non-finite log-K.
        assert!(SurfaceSite::new(f64::NAN, -9.0, 1.0e-3, 600.0, 1.0).is_err());

        // Bad model parameters.
        assert!(
            SurfaceComplexationModel::ConstantCapacitance { capacitance: -1.0 }
                .validate()
                .is_err()
        );
        assert!(
            SurfaceComplexationModel::ConstantCapacitance { capacitance: 0.0 }
                .validate()
                .is_err()
        );
        assert!(SurfaceComplexationModel::DiffuseLayer {
            ionic_strength: -0.1
        }
        .validate()
        .is_err());

        // Bad complex.
        assert!(SurfaceComplex::new(-3.0, -2.0).is_err()); // non-positive charge
        assert!(SurfaceComplex::new(f64::INFINITY, 2.0).is_err());

        // Bad speciate arguments.
        let site = hfo_site();
        assert!(site.speciate(f64::NAN, 1.0e-6, nem, &cx).is_err()); // non-finite pH
        assert!(site.speciate(7.0, -1.0e-6, nem, &cx).is_err()); // negative metal
        let ccm = SurfaceComplexationModel::ConstantCapacitance { capacitance: -5.0 };
        assert!(site.speciate(7.0, 1.0e-6, ccm, &cx).is_err()); // bad capacitance

        // Electrostatic model with zero surface area → InvalidInput, not a panic.
        let no_area = SurfaceSite::new(7.0, -9.0, 1.0e-3, 0.0, 1.0).unwrap();
        let dlm = SurfaceComplexationModel::DiffuseLayer {
            ionic_strength: 0.1,
        };
        assert!(no_area.speciate(7.0, 1.0e-6, dlm, &cx).is_err());
    }

    #[test]
    fn zero_metal_gives_zero_sorbed_fraction() {
        let site = hfo_site();
        let cx = metal_complex();
        let nem = SurfaceComplexationModel::NonElectrostatic;
        let sp = site.speciate(7.0, 0.0, nem, &cx).unwrap();
        assert_eq!(sp.som, 0.0);
        assert_eq!(sp.fraction_metal_sorbed, 0.0);
    }

    #[test]
    fn extreme_conditions_do_not_panic_or_diverge() {
        // Very acidic / very alkaline with small capacitance still solves.
        let site = hfo_site();
        let cx = SurfaceComplex::new(-2.0, 3.0).unwrap();
        for &ph in &[0.0, 1.0, 13.0, 14.0] {
            let sp = site
                .speciate(
                    ph,
                    1.0e-4,
                    SurfaceComplexationModel::ConstantCapacitance { capacitance: 0.1 },
                    &cx,
                )
                .expect("solver should converge, not panic");
            assert!(sp.surface_potential.is_finite());
            let sum = sp.soh + sp.soh2 + sp.so_minus + sp.som;
            assert!((sum - site.site_density).abs() < 1e-11 * site.site_density);
        }
    }
}
