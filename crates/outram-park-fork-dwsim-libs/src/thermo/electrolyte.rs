//! Electrolyte (aqueous-ionic) activity-coefficient tier — DWSIM port.
//!
//! GPLv3 provenance (upstream: **DWSIM**, GPL-3.0, commit `1abf72d`):
//!
//! - `DWSIM.Thermodynamics/PropertyPackages/ElectrolyteBase.vb` — the
//!   electrolyte property-package substrate (ion speciation, phase glue).
//! - `DWSIM.Thermodynamics/PropertyPackages/ElectrolyteIdeal.vb` — the simplest
//!   aqueous-ionic model (molality-scale ideal + a Debye-Hückel mean-ionic term,
//!   `Models/ElectrolyteProperties.vb::MIAC`).
//! - `DWSIM.Thermodynamics/PropertyPackages/LIQUAC2PropertyPackage.vb` and its
//!   activity kernel `PropertyPackages/Models/LIQUAC2.vb::GAMMA_MR` — LIQUAC =
//!   Debye-Hückel **long-range** + a **middle-range** ionic virial term +
//!   UNIQUAC **short-range** for strong electrolytes.
//! - `DWSIM.Thermodynamics/FlashAlgorithms/ElectrolyteSVLE.vb` — the aqueous
//!   solid-vapour-liquid ionic equilibrium / speciation flash.
//! - Osmotic coefficient / pH / freezing-point functions:
//!   `PropertyPackages/Models/ElectrolyteProperties.vb`.
//!
//! This is a GPL-3.0 derivative of those files. Copyright of the original
//! algorithm: Daniel Wagner O. de Medeiros (DWSIM). LIQUAC reference:
//! Li, Polka & Gmehling, *Fluid Phase Equilibria* (1994) / the ACS paper cited
//! in `LIQUAC2.vb` (`https://pubs.acs.org/doi/10.1021/ie0510122`).
//!
//! > **⚠️ Untrusted draft, pending human V&V.** Early-stage translation, no human
//! > review. Independent OUTRAM PARK fork, **not** the official DWSIM. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions (workspace `RESPONSIBLE_USE.md`). The verification tests below
//! > check the Debye-Hückel limiting law and electroneutrality analytically;
//! > nothing here is *validated* against experimental electrolyte data.
//!
//! # What this computes, and in what units
//!
//! For an aqueous mixture of neutral solvent(s) and ions given as **mole
//! fractions** `x` (dimensionless, should sum to 1) at temperature `T` (kelvin),
//! this module computes:
//!
//! - **Molality** `m_i = x_i / w` where `w = Σ_solvent x_s M_s` is the solvent
//!   mass per mole of mixture [kg]. Molality is in **mol/kg (solvent)**.
//!   (`LIQUAC2.vb:263-283`, `ElectrolyteProperties.vb:86-103`.)
//! - **Ionic strength** `I = ½ Σ_i z_i² m_i` [mol/kg] (`LIQUAC2.vb:319-324`).
//! - **Electroneutrality residual** `Σ_i z_i m_i` [mol/kg], which must be `0`.
//! - **Activity coefficients** `γ_i` (dimensionless, `> 0`). For ions these are
//!   on the **molality scale, unsymmetric (McMillan-Mayer) convention**
//!   (`γ_i → 1` as `I → 0`); for the solvent on the mole-fraction scale.
//!
//! # Debye-Hückel constant and convention (read carefully)
//!
//! The long-range term uses the **natural-log** Debye-Hückel form
//!
//! ```text
//! ln γ_i^LR = -A z_i² √I / (1 + b √I)          (LIQUAC2.vb:382)
//! ```
//!
//! with, following `LIQUAC2.vb:326-327`,
//!
//! ```text
//! A = 132775.7 · √ρ_solv / (ε_solv · T)^{3/2}   [kg^{1/2} mol^{-1/2}]
//! b =   6.359696 · √ρ_solv / (ε_solv · T)^{1/2}  [kg^{1/2} mol^{-1/2}]
//! ```
//!
//! where `ρ_solv` is the solvent mass density [kg/m³] and `ε_solv` its (static,
//! dimensionless) relative permittivity. For **water at 25 °C**
//! (`ρ = 997.05 kg/m³`, `ε = 78.25`) this gives **`A ≈ 1.1766 kg^{1/2}
//! mol^{-1/2}`** and `b ≈ 1.3147` — see [`DebyeHuckel::water_25c`]. The value
//! `A ≈ 1.174` on the natural-log scale is the textbook water-25 °C constant
//! (`= 2.303 × 0.5108`, the log₁₀ constant `A_10 ≈ 0.511`). Everything in this
//! module is on the **ln (natural-log)** scale.
//!
//! In the **limiting law** (`I → 0`, drop the `1 + b√I` denominator):
//!
//! ```text
//! ln γ_i^LR → -A z_i² √I ,     ln γ_± = -A |z_+ z_-| √I
//! ```
//!
//! which the analytic V&V test below reproduces to machine precision at low `I`.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch ([`ElectrolyteModel`]); no `dyn`, no `Box<T>`, no lifetimes.
//! Species are held **by value** and referenced **by index** (`usize`). Raw
//! `f64` in the inner arithmetic loops with every quantity's unit documented.
//! The UNIQUAC short-range term **reuses** [`crate::thermo::activity::UniquacParams`]
//! (symmetric convention) rather than re-deriving the combinatorial/residual
//! math — see [`LiquacModel`] for the documented reference-state caveat.
//!
//! # Honest scope — what is and is not ported
//!
//! **Done, and analytically verified:** molality / ionic-strength /
//! electroneutrality bookkeeping; the Debye-Hückel long-range term (LIQUAC form)
//! and its limiting law; the mean ionic activity coefficient; the osmotic
//! coefficient and pH helpers; strong-electrolyte (complete-dissociation)
//! speciation — the kernel the `ElectrolyteSVLE` flash iterates on.
//!
//! **Lean / partial (documented):** the LIQUAC **middle-range** ionic-virial
//! term is implemented structurally but its interaction coefficients `B_ij`
//! (`LIQUAC2_IP.txt`, an extensive tabulated database) are **caller-supplied and
//! default to zero** — this port does not inline that database. The UNIQUAC
//! short-range reuses the symmetric kernel; DWSIM's ion **reference-state
//! normalization** (`LIQUAC2.vb:607-614`) is *not* applied, so ion short-range
//! coefficients are approximate. The full `ElectrolyteSVLE` flash (reaction-set
//! chemical equilibria solved with an IPOPT extent optimiser, coupled to a VLE
//! nested-loops flash — `ElectrolyteSVLE.vb:241-540`) is **not** ported; only
//! the strong-electrolyte dissociation speciation is.
//!
//! The **data tables are deliberately lean**: a tiny built-in Na⁺ / Cl⁻ / H₂O
//! set for tests and examples (see [`presets`]). Any real electrolyte study
//! needs the full DWSIM `LIQUAC2_RiQi.txt` / `LIQUAC2_IP.txt` /
//! `dielectricconstants.txt` parameter databases, which are **not** bundled here.

use crate::thermo::activity::UniquacParams;

/// LIQUAC Debye-Hückel long-range **`A`-constant** numerator
/// (`LIQUAC2.vb:326`): `A = A_CONST · √ρ / (ε T)^{3/2}`. Dimensioned so that, with
/// `ρ` in kg/m³, `ε` dimensionless and `T` in K, `A` comes out in
/// `kg^{1/2} mol^{-1/2}` on the natural-log scale.
pub const A_CONST: f64 = 132_775.7;

/// LIQUAC Debye-Hückel **`b`-constant** numerator (`LIQUAC2.vb:327`):
/// `b = B_CONST · √ρ / (ε T)^{1/2}`, giving `b` in `kg^{1/2} mol^{-1/2}`.
pub const B_CONST: f64 = 6.359_696;

/// Molar mass of water `M_{H₂O} = 0.018015 kg/mol`. Used as the default solvent
/// mass basis for molality.
pub const M_WATER: f64 = 0.018_015;

/// Relative static permittivity (dielectric constant) of liquid water as a
/// function of temperature `t` [K], DWSIM's polynomial
/// (`ElectrolyteProperties.vb:42`):
///
/// ```text
/// ε(T) = 289.82 - 1.148 T + 0.0017843 T² - 1.053e-6 T³   [-]
/// ```
///
/// Valid roughly `273–373 K`. At `T = 298.15 K` this returns `ε ≈ 78.26`.
///
/// # Units
/// - `t` — temperature [K]
/// - returns — relative permittivity `ε_r` [dimensionless]
#[must_use]
pub fn water_dielectric_constant(t: f64) -> f64 {
    289.82 - 1.148 * t + 0.001_784_3 * t * t - 1.053e-6 * t * t * t
}

/// A single aqueous species: a neutral solvent, a neutral solute, or an ion.
///
/// Held **by value**; a mixture is an ordered `Vec<AqueousSpecies>` and every
/// composition / coefficient vector is indexed to match (species `i` ↔ `x[i]`).
///
/// # Units
/// - `molar_mass` — molar mass `M` [kg/mol], must be `> 0`
/// - `charge` — signed ionic charge number `z` [-] (`0` for neutral species)
#[derive(Debug, Clone, PartialEq)]
pub struct AqueousSpecies {
    /// Human-readable species name (identification only; e.g. `"Water"`,
    /// `"Na+"`, `"Cl-"`).
    pub name: String,
    /// Signed charge number `z_i` [-]. `0` for neutral molecules; `+1` for Na⁺,
    /// `-1` for Cl⁻, `-2` for SO₄²⁻, etc.
    pub charge: i32,
    /// Molar mass `M_i` [kg/mol]. Must be finite and `> 0`.
    pub molar_mass: f64,
    /// Whether this species counts as **solvent** for the molality mass basis
    /// (`w = Σ_solvent x_s M_s`). DWSIM treats Water (and Methanol) as solvent
    /// (`LIQUAC2.vb:267`, `ElectrolyteProperties.vb:90`). Neutral solutes and
    /// ions are *not* solvent.
    pub is_solvent: bool,
}

impl AqueousSpecies {
    /// Construct a neutral **solvent** species (e.g. water) of molar mass
    /// `molar_mass` [kg/mol].
    #[must_use]
    pub fn solvent(name: impl Into<String>, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge: 0,
            molar_mass,
            is_solvent: true,
        }
    }

    /// Construct an **ion** of signed charge `charge` [-] and molar mass
    /// `molar_mass` [kg/mol].
    #[must_use]
    pub fn ion(name: impl Into<String>, charge: i32, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge,
            molar_mass,
            is_solvent: false,
        }
    }

    /// `true` if this species carries a non-zero charge.
    #[must_use]
    pub fn is_ion(&self) -> bool {
        self.charge != 0
    }
}

/// An ordered aqueous mixture: the species list plus the molality / ionic-
/// strength / electroneutrality bookkeeping every electrolyte model needs.
///
/// Composition is supplied per call as a mole-fraction slice `x` whose length
/// must equal [`Self::n`]. All quantities follow the module-level unit
/// conventions (molality in mol/kg, ionic strength in mol/kg).
#[derive(Debug, Clone, PartialEq)]
pub struct AqueousSystem {
    /// The species, in index order.
    pub species: Vec<AqueousSpecies>,
}

impl AqueousSystem {
    /// Build a system from an ordered species list. Panics if empty.
    #[must_use]
    pub fn new(species: Vec<AqueousSpecies>) -> Self {
        assert!(!species.is_empty(), "AqueousSystem needs ≥ 1 species");
        Self { species }
    }

    /// Number of species `n`.
    #[must_use]
    pub fn n(&self) -> usize {
        self.species.len()
    }

    /// Solvent mass per mole of mixture `w = Σ_solvent x_s M_s` [kg], the
    /// molality denominator (`LIQUAC2.vb:263-271`). Panics if `x.len() != n`.
    #[must_use]
    pub fn solvent_mass(&self, x: &[f64]) -> f64 {
        assert_eq!(x.len(), self.n(), "x length must equal species count");
        let mut w = 0.0;
        for (i, s) in self.species.iter().enumerate() {
            if s.is_solvent {
                w += x[i] * s.molar_mass;
            }
        }
        w
    }

    /// Molalities `m_i = x_i / w` [mol/kg] for every species (`LIQUAC2.vb:277`).
    ///
    /// `w` is the solvent mass ([`Self::solvent_mass`]); if there is no solvent
    /// (`w = 0`) this returns all-zero (mirrors DWSIM's guard against a
    /// solvent-free basis). Panics if `x.len() != n`.
    #[must_use]
    pub fn molalities(&self, x: &[f64]) -> Vec<f64> {
        let w = self.solvent_mass(x);
        if w <= 0.0 {
            return vec![0.0; self.n()];
        }
        x.iter().map(|&xi| xi / w).collect()
    }

    /// Ionic strength `I = ½ Σ_i z_i² m_i` [mol/kg] on the molality scale
    /// (`LIQUAC2.vb:319-324`). Panics if `x.len() != n`.
    #[must_use]
    pub fn ionic_strength(&self, x: &[f64]) -> f64 {
        let m = self.molalities(x);
        let mut im = 0.0;
        for (i, s) in self.species.iter().enumerate() {
            let z = f64::from(s.charge);
            im += 0.5 * z * z * m[i];
        }
        im
    }

    /// Electroneutrality residual `Σ_i z_i m_i` [mol/kg]. A physically valid
    /// electrolyte solution must have this equal to `0` (charge balance). Panics
    /// if `x.len() != n`.
    #[must_use]
    pub fn net_charge_molality(&self, x: &[f64]) -> f64 {
        let m = self.molalities(x);
        let mut q = 0.0;
        for (i, s) in self.species.iter().enumerate() {
            q += f64::from(s.charge) * m[i];
        }
        q
    }

    /// `true` if the composition is electroneutral to within `tol` [mol/kg]
    /// (`|Σ z_i m_i| ≤ tol`). Panics if `x.len() != n`.
    #[must_use]
    pub fn is_electroneutral(&self, x: &[f64], tol: f64) -> bool {
        self.net_charge_molality(x).abs() <= tol
    }
}

/// Debye-Hückel long-range term with the LIQUAC constants.
///
/// Stores the two solvent-dependent coefficients `A` and `b` (both in
/// `kg^{1/2} mol^{-1/2}`, natural-log scale) so a model can evaluate the
/// long-range activity contribution of any ion at a given ionic strength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebyeHuckel {
    /// Debye-Hückel `A`-coefficient [kg^{1/2} mol^{-1/2}], natural-log scale
    /// (`= A_CONST √ρ / (ε T)^{3/2}`).
    pub a: f64,
    /// Debye-Hückel `b`-coefficient [kg^{1/2} mol^{-1/2}] in the `1 + b√I`
    /// denominator (`= B_CONST √ρ / (ε T)^{1/2}`).
    pub b: f64,
}

impl DebyeHuckel {
    /// Build the coefficients from solvent density and permittivity at `T`,
    /// exactly as `LIQUAC2.vb:326-327`.
    ///
    /// # Units
    /// - `density` — solvent mass density `ρ` [kg/m³]
    /// - `dielectric` — solvent relative permittivity `ε_r` [-]
    /// - `t` — temperature [K]
    #[must_use]
    pub fn from_solvent(density: f64, dielectric: f64, t: f64) -> Self {
        let et = dielectric * t;
        let a = A_CONST * density.sqrt() / et.powf(1.5);
        let b = B_CONST * density.sqrt() / et.sqrt();
        Self { a, b }
    }

    /// Water at 25 °C: `ρ = 997.05 kg/m³`, `ε` from
    /// [`water_dielectric_constant`] at `298.15 K`.
    ///
    /// **Measured (2026-08-03, this port):** `A = 1.17656 kg^{1/2} mol^{-1/2}`,
    /// `b = 1.31474 kg^{1/2} mol^{-1/2}` — `A` matching the textbook natural-log
    /// water-25 °C Debye-Hückel constant `≈ 1.174` to `< 0.3 %`.
    #[must_use]
    pub fn water_25c() -> Self {
        let t = 298.15;
        Self::from_solvent(997.05, water_dielectric_constant(t), t)
    }

    /// Long-range `ln γ` of an ion of charge `z` at ionic strength `im` [mol/kg]
    /// (`LIQUAC2.vb:382`):
    ///
    /// ```text
    /// ln γ^LR = -A z² √I / (1 + b √I)
    /// ```
    ///
    /// Returns `0` for a neutral species (`z = 0`). Dimensionless result.
    #[must_use]
    pub fn ln_gamma_ion(&self, z: i32, im: f64) -> f64 {
        if z == 0 {
            return 0.0;
        }
        let zf = f64::from(z);
        let sqrt_i = im.sqrt();
        -self.a * zf * zf * sqrt_i / (1.0 + self.b * sqrt_i)
    }

    /// Debye-Hückel **limiting law** `ln γ = -A z² √I` (the `I → 0` limit of
    /// [`Self::ln_gamma_ion`], dropping the `1 + b√I` denominator). Provided for
    /// the analytic verification test and low-molality reference use.
    #[must_use]
    pub fn ln_gamma_ion_limiting(&self, z: i32, im: f64) -> f64 {
        if z == 0 {
            return 0.0;
        }
        let zf = f64::from(z);
        -self.a * zf * zf * im.sqrt()
    }
}

/// Mean ionic activity coefficient (natural log) of a single salt
/// `C_{ν+} A_{ν-}` from its individual-ion long-range coefficients:
///
/// ```text
/// ln γ_± = (ν_+ ln γ_+ + ν_- ln γ_-) / (ν_+ + ν_-)
/// ```
///
/// Combined with the limiting law and electroneutrality (`ν_+ z_+ = ν_- |z_-|`)
/// this collapses to `ln γ_± = -A |z_+ z_-| √I`, the form the V&V test checks.
///
/// # Arguments (all dimensionless)
/// - `nu_plus`, `nu_minus` — stoichiometric coefficients `ν_+`, `ν_-`
/// - `ln_gamma_plus`, `ln_gamma_minus` — the cation / anion `ln γ`
#[must_use]
pub fn mean_ionic_ln_gamma(
    nu_plus: f64,
    nu_minus: f64,
    ln_gamma_plus: f64,
    ln_gamma_minus: f64,
) -> f64 {
    (nu_plus * ln_gamma_plus + nu_minus * ln_gamma_minus) / (nu_plus + nu_minus)
}

/// Aqueous-ionic activity-coefficient model (enum dispatch, no `dyn`).
///
/// Given an [`AqueousSystem`], a mole-fraction composition `x`, and temperature
/// `t` [K], [`Self::activity_coefficients`] returns `γ_i` (dimensionless, `> 0`)
/// — molality-scale unsymmetric for ions, mole-fraction-scale for the solvent.
#[derive(Debug, Clone, PartialEq)]
pub enum ElectrolyteModel {
    /// **Ideal electrolyte** (`ElectrolyteIdeal.vb`): ions carry only the
    /// Debye-Hückel long-range term; neutral species are ideal (`γ = 1`).
    Ideal(IdealElectrolyte),
    /// **LIQUAC** (`LIQUAC2.vb`): long-range (Debye-Hückel) + middle-range
    /// (ionic virial) + short-range (UNIQUAC).
    Liquac(LiquacModel),
}

impl ElectrolyteModel {
    /// Activity coefficients `γ_i` at composition `x` (mole fractions) and
    /// temperature `t` [K]. Panics if `x.len()` differs from the model's
    /// species count.
    #[must_use]
    pub fn activity_coefficients(&self, system: &AqueousSystem, x: &[f64], t: f64) -> Vec<f64> {
        match self {
            ElectrolyteModel::Ideal(m) => m.activity_coefficients(system, x),
            ElectrolyteModel::Liquac(m) => m.activity_coefficients(system, x, t),
        }
    }
}

/// Ideal aqueous-ionic model — the simplest electrolyte tier
/// (`ElectrolyteIdeal.vb`). Ions get the Debye-Hückel long-range term only;
/// neutral solvents/solutes are ideal (`γ = 1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdealElectrolyte {
    /// The Debye-Hückel long-range coefficients (solvent + temperature fixed at
    /// construction; e.g. [`DebyeHuckel::water_25c`]).
    pub dh: DebyeHuckel,
}

impl IdealElectrolyte {
    /// Construct from a [`DebyeHuckel`] coefficient set.
    #[must_use]
    pub fn new(dh: DebyeHuckel) -> Self {
        Self { dh }
    }

    /// `γ_i`: ions get `exp(ln γ^LR)` at the composition's ionic strength;
    /// neutral species get `1`. Panics if `x.len() != system.n()`.
    #[must_use]
    pub fn activity_coefficients(&self, system: &AqueousSystem, x: &[f64]) -> Vec<f64> {
        let im = system.ionic_strength(x);
        system
            .species
            .iter()
            .map(|s| {
                if s.is_ion() {
                    self.dh.ln_gamma_ion(s.charge, im).exp()
                } else {
                    1.0
                }
            })
            .collect()
    }
}

/// LIQUAC model: Debye-Hückel long-range + (lean) middle-range + UNIQUAC
/// short-range (`LIQUAC2.vb::GAMMA_MR`).
///
/// ```text
/// ln γ_i = ln γ_i^LR + ln γ_i^MR + ln γ_i^SR
/// ```
///
/// - **Long-range** `LR` — [`DebyeHuckel::ln_gamma_ion`] for ions; `0` for the
///   solvent (DWSIM's solvent-`LR` `σ` expression, `LIQUAC2.vb:383-387`, is a
///   documented omission — it is small and the reference itself carries a
///   commented-out alternative form).
/// - **Middle-range** `MR` — ionic virial term (`LIQUAC2.vb:414-531`). Requires
///   the `B_ij` interaction database (`LIQUAC2_IP.txt`). **Not inlined here**:
///   `middle_range` is caller-supplied and, when `None`, the MR term is `0`.
/// - **Short-range** `SR` — **reuses** [`UniquacParams`] (workspace UNIQUAC
///   kernel) in the *symmetric* convention. DWSIM additionally applies an ion
///   **reference-state normalization** (`LIQUAC2.vb:607-614`) to make the ion SR
///   term unsymmetric; that normalization is **not** applied here, so ion SR
///   coefficients are approximate. When `short_range` is `None`, SR is `0`.
///
/// The long-range term is the analytically-verified core; MR and SR are the
/// documented lean/partial parts.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquacModel {
    /// Debye-Hückel long-range coefficients.
    pub dh: DebyeHuckel,
    /// Optional UNIQUAC short-range parameters (symmetric convention, reused
    /// from [`crate::thermo::activity`]). `r`/`q` and interaction energies must
    /// be `n × n` for the same `n` as the [`AqueousSystem`].
    pub short_range: Option<UniquacParams>,
    /// Optional lean middle-range interaction matrix `B_ij` [kg/mol], `n × n`.
    /// When present, contributes an ion-molality virial term (see
    /// [`Self::activity_coefficients`]). `None` ⇒ MR term is `0`.
    pub middle_range: Option<Vec<Vec<f64>>>,
}

impl LiquacModel {
    /// Long-range-only LIQUAC (no MR, no SR) — the analytically-grounded core.
    #[must_use]
    pub fn long_range_only(dh: DebyeHuckel) -> Self {
        Self {
            dh,
            short_range: None,
            middle_range: None,
        }
    }

    /// Attach a UNIQUAC short-range term (reused workspace kernel).
    #[must_use]
    pub fn with_short_range(mut self, uniquac: UniquacParams) -> Self {
        self.short_range = Some(uniquac);
        self
    }

    /// Attach a lean middle-range `B_ij` [kg/mol] matrix (`n × n`).
    #[must_use]
    pub fn with_middle_range(mut self, b: Vec<Vec<f64>>) -> Self {
        self.middle_range = Some(b);
        self
    }

    /// `γ_i = exp(ln γ_i^LR + ln γ_i^MR + ln γ_i^SR)`. Panics if `x.len()` (or
    /// any attached parameter matrix) is not sized to `system.n()`.
    #[must_use]
    pub fn activity_coefficients(&self, system: &AqueousSystem, x: &[f64], t: f64) -> Vec<f64> {
        let n = system.n();
        assert_eq!(x.len(), n, "x length must equal species count");
        let im = system.ionic_strength(x);
        let m = system.molalities(x);

        // Long-range (Debye-Hückel) per ion; 0 for solvent/neutral.
        let mut ln_gamma: Vec<f64> = system
            .species
            .iter()
            .map(|s| self.dh.ln_gamma_ion(s.charge, im))
            .collect();

        // Lean middle-range: symmetric ion-ion / ion-solvent virial
        // ln γ_i^MR = Σ_j B_ij m_j   (a reduced form of LIQUAC2.vb:521-531,
        // with the ionic-strength-derivative and reference terms omitted — see
        // the struct docs; contributes only when B is supplied).
        if let Some(b) = &self.middle_range {
            assert_eq!(b.len(), n, "middle_range must be n×n");
            for (i, ln_gi) in ln_gamma.iter_mut().enumerate() {
                assert_eq!(b[i].len(), n, "middle_range row {i} must have length n");
                let mut s = 0.0;
                for (j, &mj) in m.iter().enumerate() {
                    s += b[i][j] * mj;
                }
                *ln_gi += s;
            }
        }

        // Short-range: reuse the symmetric UNIQUAC kernel.
        if let Some(uq) = &self.short_range {
            let g_sr = uq.activity_coefficients(x, t);
            for (ln_gi, &g) in ln_gamma.iter_mut().zip(g_sr.iter()) {
                *ln_gi += g.ln();
            }
        }

        ln_gamma.into_iter().map(f64::exp).collect()
    }
}

/// Osmotic coefficient of the solvent (`ElectrolyteProperties.vb::OsmoticCoeff`,
/// lines 76-108):
///
/// ```text
/// Φ = -ln(x_w · γ_w) / (M_w · Σ_ion m_i)
/// ```
///
/// where `x_w`, `γ_w` are the solvent mole fraction and activity coefficient,
/// `M_w` its molar mass [kg/mol], and `Σ_ion m_i` the total ion molality
/// [mol/kg]. Dimensionless.
///
/// # Arguments
/// - `system` — the aqueous mixture
/// - `x` — mole fractions [-]
/// - `gamma` — activity coefficients [-] (same order/length as `x`)
/// - `solvent_index` — index of the reference solvent (e.g. water)
///
/// Returns `f64::NAN` if there are no ions (undefined osmotic coefficient).
/// Panics if lengths disagree.
#[must_use]
pub fn osmotic_coefficient(
    system: &AqueousSystem,
    x: &[f64],
    gamma: &[f64],
    solvent_index: usize,
) -> f64 {
    assert_eq!(x.len(), system.n());
    assert_eq!(gamma.len(), system.n());
    let m = system.molalities(x);
    let sum_ion: f64 = system
        .species
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_ion())
        .map(|(i, _)| m[i])
        .sum();
    if sum_ion <= 0.0 {
        return f64::NAN;
    }
    let mw = system.species[solvent_index].molar_mass;
    -(x[solvent_index] * gamma[solvent_index]).ln() / (mw * sum_ion)
}

/// pH from the hydrogen-ion molality and its activity coefficient
/// (`ElectrolyteProperties.vb::pH`, lines 111-148, molality form):
///
/// ```text
/// pH = -log₁₀(m_{H⁺} · γ_{H⁺})
/// ```
///
/// # Note on DWSIM's density factor
/// DWSIM multiplies `m_{H⁺}` by `ρ_liq/1000` to approximate a molarity
/// (mol/L) basis before taking `-log₁₀`. This port uses the **molality**
/// (mol/kg) basis directly (the `ρ/1000` factor is `≈ 1` for dilute aqueous
/// solutions and requires a liquid-density model not ported here). Documented
/// simplification.
///
/// # Arguments
/// - `system`, `x` — mixture and mole fractions
/// - `gamma` — activity coefficients [-]
/// - `hydrogen_index` — index of the H⁺ (`Hydron`) species
///
/// Panics if lengths disagree.
#[must_use]
pub fn ph_from_hydrogen(
    system: &AqueousSystem,
    x: &[f64],
    gamma: &[f64],
    hydrogen_index: usize,
) -> f64 {
    assert_eq!(x.len(), system.n());
    assert_eq!(gamma.len(), system.n());
    let m = system.molalities(x);
    -(m[hydrogen_index] * gamma[hydrogen_index]).log10()
}

/// Strong-electrolyte **complete-dissociation speciation** — the kernel the
/// `ElectrolyteSVLE` flash iterates on for a strong salt (`ElectrolyteSVLE.vb`:
/// its `SolveChemicalEquilibria` drives every strong reaction to completion).
///
/// Given a salt `C_{ν+} A_{ν-}` dissolved at molality `m_salt` [mol/kg], returns
/// the resulting `(cation_molality, anion_molality)` [mol/kg] assuming the salt
/// fully dissociates (the strong-electrolyte limit):
///
/// ```text
/// m_+ = ν_+ · m_salt ,   m_- = ν_- · m_salt
/// ```
///
/// The result is charge-balanced by construction when `ν_+ z_+ + ν_- z_- = 0`;
/// [`is_dissociation_neutral`] checks that stoichiometry.
///
/// This is the *strong* speciation only. DWSIM's full flash also solves weak
/// (finite-`K_eq`) equilibria via an extent optimiser — **not** ported here.
#[must_use]
pub fn dissociate_strong_salt(m_salt: f64, nu_plus: f64, nu_minus: f64) -> (f64, f64) {
    (nu_plus * m_salt, nu_minus * m_salt)
}

/// Check that a salt's dissociation stoichiometry is charge-neutral:
/// `ν_+ z_+ + ν_- z_- = 0` (`z_-` is negative). Returns `true` when balanced.
#[must_use]
pub fn is_dissociation_neutral(nu_plus: f64, z_plus: i32, nu_minus: f64, z_minus: i32) -> bool {
    (nu_plus * f64::from(z_plus) + nu_minus * f64::from(z_minus)).abs() < 1e-12
}

/// Lean built-in species presets — a **tiny** Na⁺ / Cl⁻ / H₂O set for tests and
/// examples. **Not** a parameter database: any real study needs DWSIM's full
/// `LIQUAC2_RiQi.txt` / `LIQUAC2_IP.txt` / `dielectricconstants.txt`, which are
/// not bundled in this fork.
///
/// Molar masses are standard public IUPAC atomic-weight values.
pub mod presets {
    use super::AqueousSpecies;

    /// Water H₂O (solvent). `M = 0.018015 kg/mol`.
    #[must_use]
    pub fn water() -> AqueousSpecies {
        AqueousSpecies::solvent("Water", 0.018_015)
    }

    /// Sodium ion Na⁺ (`z = +1`). `M = 0.022990 kg/mol`.
    #[must_use]
    pub fn sodium_ion() -> AqueousSpecies {
        AqueousSpecies::ion("Na+", 1, 0.022_990)
    }

    /// Chloride ion Cl⁻ (`z = -1`). `M = 0.035453 kg/mol`.
    #[must_use]
    pub fn chloride_ion() -> AqueousSpecies {
        AqueousSpecies::ion("Cl-", -1, 0.035_453)
    }

    /// Hydrogen ion (hydron) H⁺ (`z = +1`). `M = 0.001008 kg/mol`.
    #[must_use]
    pub fn hydrogen_ion() -> AqueousSpecies {
        AqueousSpecies::ion("Hydron", 1, 0.001_008)
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the DWSIM electrolyte port
    //!
    //! **Methodology (shared).** These are *verification* checks against
    //! **analytical** properties of the model equations — the Debye-Hückel
    //! limiting law, charge balance, and the closed-form mean-ionic relation —
    //! **not** validation against experimental electrolyte data. Reference
    //! expressions are the DWSIM `LIQUAC2.vb` / `ElectrolyteProperties.vb`
    //! kernels. Numbers were measured on 2026-08-03 (this port).
    //!
    //! **Scope (honesty).** No parameter set here is claimed experimentally
    //! accurate; nothing is cleared for nuclear / safety-critical use.

    use super::*;
    use approx::assert_relative_eq;

    /// NaCl / water system at the given salt molality, as mole fractions.
    /// Species order: [Water, Na+, Cl-].
    fn nacl_system() -> AqueousSystem {
        AqueousSystem::new(vec![
            presets::water(),
            presets::sodium_ion(),
            presets::chloride_ion(),
        ])
    }

    /// Mole fractions for `m` mol/kg of NaCl in water (1 kg water basis).
    fn nacl_mole_fractions(m: f64) -> Vec<f64> {
        let n_water = 1.0 / M_WATER; // moles of water in 1 kg
        let total = n_water + 2.0 * m;
        vec![n_water / total, m / total, m / total]
    }

    /// **Methodology.** The Debye-Hückel `A`-constant from the LIQUAC formula
    /// (`A = 132775.7 √ρ / (εT)^{3/2}`) for water at 25 °C must reproduce the
    /// textbook natural-log constant `A ≈ 1.174 kg^{1/2} mol^{-1/2}`.
    /// **Result (2026-08-03).** `A = 1.17656`, within `0.3 %` of `1.174`;
    /// `b = 1.31474`.
    #[test]
    fn debye_huckel_constant_matches_textbook() {
        let dh = DebyeHuckel::water_25c();
        assert_relative_eq!(dh.a, 1.174, epsilon = 3e-3);
        assert!(
            (dh.a - 1.176_56).abs() < 1e-4,
            "A = {} (expected ≈ 1.17656)",
            dh.a
        );
        assert!(
            (dh.b - 1.314_74).abs() < 1e-4,
            "b = {} (expected ≈ 1.31474)",
            dh.b
        );
    }

    /// **Methodology (mandated analytic check).** At **low** ionic strength the
    /// long-range term must match the Debye-Hückel **limiting law**
    /// `ln γ_± = -A |z_+ z_-| √I`. For NaCl (`z_+ = +1`, `z_- = -1`,
    /// `ν_+ = ν_- = 1`), `|z_+ z_-| = 1`, so `ln γ_± = -A √I`. We evaluate the
    /// full LIQUAC long-range `ln γ_i = -A z² √I/(1+b√I)` at `m = 0.01 mol/kg`
    /// (`I = 0.01`), combine into the mean-ionic value, and compare to the
    /// limiting law; the `1 + b√I` denominator makes them agree to `O(b√I) ≈
    /// 1.3 %`, and the limiting-law helper agrees to machine precision.
    /// **Result (2026-08-03).** At `I = 0.01`: limiting-law `ln γ_± =
    /// -0.117656` (`γ_± = 0.88900`); full LR `ln γ_± = -0.103985`
    /// (`γ_± = 0.90124`); the full/limiting `ln γ` ratio is
    /// `1/(1+b√I) = 0.8838`, i.e. within the expected `≈ b√I` spread. The
    /// limiting-law path reproduces `-A √I` to `< 1e-12`.
    #[test]
    fn debye_huckel_limiting_law_low_molality() {
        let dh = DebyeHuckel::water_25c();
        let system = nacl_system();
        let m = 0.01; // mol/kg
        let x = nacl_mole_fractions(m);
        let im = system.ionic_strength(&x);
        // I ≈ 0.01 mol/kg (the 1 kg-water basis makes molality ≈ m).
        assert_relative_eq!(im, m, epsilon = 1e-4);

        // Analytic limiting law for the mean ionic coefficient.
        let expected_ln_gpm = -dh.a * (im).sqrt(); // |z+ z-| = 1
        // From individual-ion limiting coefficients + the mean-ionic formula.
        let ln_gp = dh.ln_gamma_ion_limiting(1, im);
        let ln_gm = dh.ln_gamma_ion_limiting(-1, im);
        let ln_gpm = mean_ionic_ln_gamma(1.0, 1.0, ln_gp, ln_gm);
        assert_relative_eq!(ln_gpm, expected_ln_gpm, epsilon = 1e-12);

        // Full LR term is close (denominator 1/(1+b√I)) and converges to the
        // limiting law as I → 0.
        let ln_gp_full = dh.ln_gamma_ion(1, im);
        let ln_gm_full = dh.ln_gamma_ion(-1, im);
        let ln_gpm_full = mean_ionic_ln_gamma(1.0, 1.0, ln_gp_full, ln_gm_full);
        assert_relative_eq!(
            ln_gpm_full,
            expected_ln_gpm / (1.0 + dh.b * im.sqrt()),
            epsilon = 1e-12
        );
    }

    /// **Methodology.** Convergence of the full LR term to the limiting law as
    /// `I → 0`: the ratio `ln γ^full / ln γ^limiting = 1/(1+b√I) → 1`. Checked
    /// at `I = 1e-6`.
    /// **Result (2026-08-03).** Ratio `= 0.99868` at `I = 1e-6`, `→ 1` to
    /// `< 1.4e-3`; the two forms are indistinguishable in the dilute limit.
    #[test]
    fn full_lr_converges_to_limiting_law() {
        let dh = DebyeHuckel::water_25c();
        let im = 1e-6;
        let full = dh.ln_gamma_ion(1, im);
        let lim = dh.ln_gamma_ion_limiting(1, im);
        assert_relative_eq!(full / lim, 1.0, epsilon = 2e-3);
    }

    /// **Methodology.** Electroneutrality `Σ z_i m_i = 0` must hold for a
    /// dissolved 1:1 salt at any molality (equal cation/anion molality). Checked
    /// at `m = 0.01` and `m = 1.0`.
    /// **Result (2026-08-03).** Residual `= 0` exactly (equal-and-opposite
    /// molalities); `is_electroneutral` true at `tol = 1e-12`.
    #[test]
    fn electroneutrality_enforced() {
        let system = nacl_system();
        for &m in &[0.01, 1.0] {
            let x = nacl_mole_fractions(m);
            assert_relative_eq!(system.net_charge_molality(&x), 0.0, epsilon = 1e-12);
            assert!(system.is_electroneutral(&x, 1e-12));
        }
    }

    /// **Methodology.** A charge-imbalanced composition (extra cation, no
    /// balancing anion) must be flagged non-neutral.
    /// **Result (2026-08-03).** `Σ z_i m_i > 0`; `is_electroneutral` false.
    #[test]
    fn electroneutrality_detects_imbalance() {
        let system = nacl_system();
        // Water + Na+ only, no Cl-: net positive charge.
        let n_water = 1.0 / M_WATER;
        let total = n_water + 0.05;
        let x = vec![n_water / total, 0.05 / total, 0.0];
        assert!(system.net_charge_molality(&x) > 0.0);
        assert!(!system.is_electroneutral(&x, 1e-9));
    }

    /// **Methodology.** Ionic strength of a `z:z` electrolyte equals the salt
    /// molality: `I = ½(z_+² m_+ + z_-² m_-) = ½(1·m + 1·m) = m` for 1:1.
    /// **Result (2026-08-03).** `I = m` to `< 1e-4` (basis rounding) at
    /// `m = 0.1`.
    #[test]
    fn ionic_strength_of_symmetric_salt() {
        let system = nacl_system();
        let m = 0.1;
        let x = nacl_mole_fractions(m);
        assert_relative_eq!(system.ionic_strength(&x), m, epsilon = 1e-3);
    }

    /// **Methodology.** The [`ElectrolyteModel::Ideal`] dispatch must give
    /// ions `γ = exp(ln γ^LR)` and the solvent `γ = 1`. Cross-checked against a
    /// direct Debye-Hückel evaluation at `m = 0.01`.
    /// **Result (2026-08-03).** Solvent `γ = 1.0`; Na⁺ and Cl⁻
    /// `γ = 0.90126`, matching the direct LR exponential to `< 1e-12`.
    #[test]
    fn ideal_model_ion_gamma_is_debye_huckel() {
        let system = nacl_system();
        let m = 0.01;
        let x = nacl_mole_fractions(m);
        let dh = DebyeHuckel::water_25c();
        let model = ElectrolyteModel::Ideal(IdealElectrolyte::new(dh));
        let g = model.activity_coefficients(&system, &x, 298.15);
        assert_relative_eq!(g[0], 1.0, epsilon = 1e-12); // water
        let im = system.ionic_strength(&x);
        assert_relative_eq!(g[1], dh.ln_gamma_ion(1, im).exp(), epsilon = 1e-12);
        assert_relative_eq!(g[2], dh.ln_gamma_ion(-1, im).exp(), epsilon = 1e-12);
        // Physically: activity coefficient < 1 for the dilute strong electrolyte.
        assert!(g[1] < 1.0 && g[1] > 0.85);
    }

    /// **Methodology.** LIQUAC long-range-only must equal the Ideal model (both
    /// are pure Debye-Hückel) when no MR/SR term is attached.
    /// **Result (2026-08-03).** Identical to `< 1e-15`.
    #[test]
    fn liquac_long_range_only_equals_ideal() {
        let system = nacl_system();
        let x = nacl_mole_fractions(0.05);
        let dh = DebyeHuckel::water_25c();
        let ideal = ElectrolyteModel::Ideal(IdealElectrolyte::new(dh));
        let liquac = ElectrolyteModel::Liquac(LiquacModel::long_range_only(dh));
        let gi = ideal.activity_coefficients(&system, &x, 298.15);
        let gl = liquac.activity_coefficients(&system, &x, 298.15);
        for (a, b) in gi.iter().zip(gl.iter()) {
            assert_relative_eq!(*a, *b, epsilon = 1e-15);
        }
    }

    /// **Methodology.** The short-range term must **reuse** the workspace
    /// UNIQUAC kernel: attaching a [`UniquacParams`] adds `ln γ^UNIQUAC` to the
    /// long-range `ln γ`. Verified by comparing the LIQUAC(LR+SR) result to the
    /// product `γ^LR · γ^UNIQUAC` computed independently.
    /// **Result (2026-08-03).** Match to `< 1e-12` for all three species.
    #[test]
    fn liquac_short_range_reuses_uniquac() {
        let system = nacl_system();
        let x = nacl_mole_fractions(0.05);
        let t = 298.15;
        let dh = DebyeHuckel::water_25c();
        // A representative (illustrative, NOT validated) 3-species UNIQUAC set.
        let r = vec![0.92, 1.0, 1.0];
        let q = vec![1.40, 1.0, 1.0];
        let a = vec![
            vec![0.0, 50.0, -30.0],
            vec![50.0, 0.0, 10.0],
            vec![-30.0, 10.0, 0.0],
        ];
        let uq = UniquacParams::from_energies(r, q, a);
        let liquac = ElectrolyteModel::Liquac(
            LiquacModel::long_range_only(dh).with_short_range(uq.clone()),
        );
        let g = liquac.activity_coefficients(&system, &x, t);

        let im = system.ionic_strength(&x);
        let g_uq = uq.activity_coefficients(&x, t);
        for (i, s) in system.species.iter().enumerate() {
            let ln_lr = dh.ln_gamma_ion(s.charge, im);
            let expected = (ln_lr + g_uq[i].ln()).exp();
            assert_relative_eq!(g[i], expected, epsilon = 1e-12);
        }
    }

    /// **Methodology.** The lean middle-range term `ln γ_i^MR = Σ_j B_ij m_j`
    /// must add exactly when a `B` matrix is supplied and vanish when it is
    /// zero. Checked with a single nonzero ion-solvent coefficient.
    /// **Result (2026-08-03).** Zero `B` ⇒ identical to LR-only; nonzero `B`
    /// shifts `ln γ` by the expected `Σ_j B_ij m_j` to `< 1e-12`.
    #[test]
    fn liquac_middle_range_adds_virial_term() {
        let system = nacl_system();
        let x = nacl_mole_fractions(0.1);
        let m = system.molalities(&x);
        let dh = DebyeHuckel::water_25c();

        let zero_b = vec![vec![0.0; 3]; 3];
        let lr_only = LiquacModel::long_range_only(dh);
        let with_zero =
            LiquacModel::long_range_only(dh).with_middle_range(zero_b);
        let g0 = lr_only.activity_coefficients(&system, &x, 298.15);
        let gz = with_zero.activity_coefficients(&system, &x, 298.15);
        for (a, b) in g0.iter().zip(gz.iter()) {
            assert_relative_eq!(*a, *b, epsilon = 1e-15);
        }

        // Nonzero B: give Na+ (index 1) a coefficient with water (index 0).
        let mut b = vec![vec![0.0; 3]; 3];
        b[1][0] = 0.3; // kg/mol
        let with_b = LiquacModel::long_range_only(dh).with_middle_range(b);
        let gb = with_b.activity_coefficients(&system, &x, 298.15);
        let im = system.ionic_strength(&x);
        let expected_na = (dh.ln_gamma_ion(1, im) + 0.3 * m[0]).exp();
        assert_relative_eq!(gb[1], expected_na, epsilon = 1e-12);
    }

    /// **Methodology.** Strong-salt dissociation speciation: 0.5 mol/kg NaCl must
    /// yield 0.5 mol/kg Na⁺ and 0.5 mol/kg Cl⁻, and the stoichiometry must be
    /// charge-neutral (`1·(+1) + 1·(-1) = 0`).
    /// **Result (2026-08-03).** `(0.5, 0.5)`; `is_dissociation_neutral` true.
    #[test]
    fn strong_salt_dissociation() {
        let (mp, mm) = dissociate_strong_salt(0.5, 1.0, 1.0);
        assert_relative_eq!(mp, 0.5, epsilon = 1e-15);
        assert_relative_eq!(mm, 0.5, epsilon = 1e-15);
        assert!(is_dissociation_neutral(1.0, 1, 1.0, -1));
        // A 2:1 salt like Na2SO4 (2 Na+, 1 SO4^2-) is also neutral.
        assert!(is_dissociation_neutral(2.0, 1, 1.0, -2));
        // A wrong stoichiometry is flagged.
        assert!(!is_dissociation_neutral(1.0, 1, 1.0, -2));
    }

    /// **Methodology.** Osmotic coefficient sign/finiteness: for a dilute strong
    /// electrolyte with a slightly non-ideal solvent activity, `Φ` must be
    /// finite and positive; with no ions it must be `NaN` (undefined).
    /// **Result (2026-08-03).** `Φ` finite and `> 0` for the NaCl case;
    /// `NaN` for pure water.
    #[test]
    fn osmotic_coefficient_defined_only_with_ions() {
        let system = nacl_system();
        let x = nacl_mole_fractions(0.1);
        // Solvent activity slightly < 1 (γ_w·x_w), ions γ from DH.
        let dh = DebyeHuckel::water_25c();
        let model = ElectrolyteModel::Ideal(IdealElectrolyte::new(dh));
        let g = model.activity_coefficients(&system, &x, 298.15);
        let phi = osmotic_coefficient(&system, &x, &g, 0);
        assert!(phi.is_finite() && phi > 0.0, "Φ = {phi}");

        // Pure water: no ions ⇒ NaN.
        let x_pure = vec![1.0, 0.0, 0.0];
        let g_pure = vec![1.0, 1.0, 1.0];
        assert!(osmotic_coefficient(&system, &x_pure, &g_pure, 0).is_nan());
    }

    /// **Methodology.** pH from hydrogen-ion molality: a system with H⁺ at a
    /// known molality and `γ = 1` must give `pH = -log₁₀(m_{H⁺})`.
    /// **Result (2026-08-03).** For `m_{H⁺} = 1e-3 mol/kg`, `pH = 3.0` to
    /// `< 1e-6`.
    #[test]
    fn ph_from_hydrogen_molality() {
        // Water + H+ + Cl- (HCl), with H+ at ~1e-3 mol/kg.
        let system = AqueousSystem::new(vec![
            presets::water(),
            presets::hydrogen_ion(),
            presets::chloride_ion(),
        ]);
        let m_h = 1e-3;
        let n_water = 1.0 / M_WATER;
        let total = n_water + 2.0 * m_h;
        let x = vec![n_water / total, m_h / total, m_h / total];
        let gamma = vec![1.0, 1.0, 1.0];
        let ph = ph_from_hydrogen(&system, &x, &gamma, 1);
        assert_relative_eq!(ph, 3.0, epsilon = 1e-6);
    }
}
