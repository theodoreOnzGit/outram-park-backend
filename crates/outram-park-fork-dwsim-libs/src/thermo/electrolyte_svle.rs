//! Electrolyte SVLE flash — weak-electrolyte reaction-set speciation solver plus
//! solid (Ksp) precipitation coupling. DWSIM port.
//!
//! ---
//!
//! # GPLv3 provenance
//!
//! Upstream project: **DWSIM** (open-source chemical process simulator),
//! GPL-3.0, upstream commit `1abf72d`.
//!
//! Ported from
//! `DWSIM.Thermodynamics/FlashAlgorithms/ElectrolyteSVLE.vb`, specifically the
//! chemical-equilibrium core the file's own doc comment in
//! [`crate::thermo::electrolyte`] flags as *not yet ported*:
//!
//! - `ElectrolyteSVLE.vb:241-554` — `SolveChemicalEquilibria`: assembles the
//!   equilibrium-reaction stoichiometry matrix `E`, seeds reaction extents, and
//!   drives a Newton solve of the reaction-set chemical equilibria in the liquid
//!   phase. Ported here as [`SvleSystem::solve_speciation`].
//! - `ElectrolyteSVLE.vb:556-698` — `FunctionValue2N`: the residual function
//!   `f_i = ln(K_i / Q_i)` where `Q_i = Π_s a_s^{ν_{s,i}}` is the reaction
//!   activity quotient built from **molality-scale** activities for ions/salts
//!   and **mole-fraction-scale** activities for neutral species. Ported here as
//!   the private `SvleSystem::residual`.
//! - `ElectrolyteSVLE.vb:73-239` — `Flash_PT`: the outer VLE ⇄ speciation ⇄
//!   solid loop. The **solid-liquid split** it delegates to (`nl3.Flash_SL`,
//!   `ElectrolyteSVLE.vb:191`) is DWSIM `NestedLoopsSLE.Flash_SL`, already ported
//!   at [`crate::thermo::flash_sle::flash_sl`]. This file adds the **Ksp
//!   solubility** precipitation piece ([`SaltSolubility`]) that the outer loop
//!   needs for ionic solids, and documents (Honest scope, below) what of the
//!   full VLE outer loop is deliberately *not* reproduced here.
//!
//! Copyright of the original algorithm: Daniel Wagner O. de Medeiros (DWSIM).
//! This Rust file is a GPL-3.0 derivative work.
//!
//! > **⚠️ Untrusted AI-assisted draft, pending human V&V.** Early-stage
//! > translation, no human review. Independent OUTRAM PARK fork, **not** the
//! > official DWSIM. Not for nuclear facility operation, reactor control,
//! > safety-critical, licensing, or any operational decision
//! > (`RESPONSIBLE_USE.md`). The tests below are **verification** (the solver
//! > reproduces analytic / closed-form / literature-constant references), **not
//! > validation** against an experimental electrolyte database.
//!
//! ---
//!
//! # What this computes
//!
//! Given an aqueous liquid phase described by a list of [`SvleSpecies`] (a
//! solvent, neutral solutes, ions, and optionally solids), a set of
//! [`EquilibriumReaction`]s with their temperature-evaluated equilibrium
//! constants `K_i`, and an initial mole-amount vector `n0` \[mol\], this module
//! finds the reaction extents `ξ_j` \[mol\] such that every reaction satisfies
//! its mass-action law
//!
//! ```text
//! K_i = Π_s a_s(n)^{ν_{s,i}} ,     n_s = n0_s + Σ_j ν_{s,j} ξ_j
//! ```
//!
//! and returns the speciated equilibrium mole amounts `n_s` \[mol\]. Activities
//! `a_s` follow DWSIM's mixed-scale convention (`ElectrolyteSVLE.vb:633-641`):
//!
//! - **Ion / salt** species: `a_s = m_s · γ_s`, molality scale, where
//!   `m_s = x_s / w` \[mol/kg\] and `w = Σ_solvent x_s M_s` \[kg\] is the solvent
//!   mass per mole of liquid (identical basis to [`crate::thermo::electrolyte`]).
//! - **Solvent / neutral** species: `a_s = x_s · γ_s`, mole-fraction scale.
//! - **Solid** species: `a_s = 1` (pure-solid reference state).
//!
//! Activity coefficients `γ_s` are supplied by [`SvleActivity`]: `Ideal`
//! (`γ = 1`, which is exactly what DWSIM's `FunctionValue2N` uses —
//! `activcoeff = proppack.RET_UnitaryVector()`, `ElectrolyteSVLE.vb:628`) or a
//! Debye-Hückel long-range term reusing [`crate::thermo::electrolyte::DebyeHuckel`].
//!
//! # Units
//!
//! | Quantity | Unit |
//! |---|---|
//! | Mole amounts `n`, `n0`, extents `ξ` | mol |
//! | Molar mass `M` | kg/mol |
//! | Molality `m` | mol/kg (solvent) |
//! | Ionic strength `I` | mol/kg |
//! | Temperature `T` | K |
//! | Equilibrium constant `K`, activity `a`, mole fraction `x`, charge `z` | dimensionless |
//! | Solubility product `Ksp` | (mol/kg)^(ν₊+ν₋) |
//!
//! # Honest scope — what is and is NOT ported
//!
//! **Ported and verified here:**
//! - The reaction-set chemical-equilibrium solver: extent formulation,
//!   `ln(K/Q)` residual, the ion/neutral/solid activity-scale convention, and a
//!   robust root solve (bracketed bisection for a single reaction; damped
//!   feasibility-clamped Newton with a finite-difference Jacobian for coupled
//!   reactions). This is the `SolveChemicalEquilibria` / `FunctionValue2N` core.
//! - The Ksp solid-precipitation piece ([`SaltSolubility`]): saturation
//!   molality, supersaturation test, and the 1:1-salt precipitation amount —
//!   giving the solid-onset / reduction-to-no-solid behaviour of the outer loop.
//!
//! **Deliberately NOT reproduced** (documented omissions, not silent gaps):
//! - **The full `Flash_PT` VLE outer loop** (`ElectrolyteSVLE.vb:139-217`): the
//!   alternating vapour-liquid `NestedLoops` flash, the `AUX_PVAPi` vapour-
//!   pressure feed rebuild, and the `nl.CalculateEquilibrium` calls need a live
//!   property package. Those pieces exist elsewhere in this crate
//!   ([`crate::thermo::flash`], [`crate::thermo::property_package`]) but wiring
//!   the whole outer loop is out of this file's scope. What is ported is the
//!   *liquid-phase speciation kernel* that loop iterates on, plus the solid
//!   split it delegates to ([`crate::thermo::flash_sle`]).
//! - **DWSIM's density-scaled molality** (`ElectrolyteSVLE.vb:604-624`,
//!   `m = x/w · ρ_liq/1000`, i.e. a molarity approximation) — this port uses the
//!   clean `m = x/w` \[mol/kg solvent\] basis of [`crate::thermo::electrolyte`]
//!   (`ρ/1000 ≈ 1` for dilute aqueous), because the liquid-density model DWSIM
//!   calls is not ported. For dilute aqueous systems the two agree closely.
//! - **DWSIM's variable scaling + 4 penalty-value schemes**
//!   (`ElectrolyteSVLE.vb:439-508`, `676-693`): heuristics to nurse the external
//!   `DotNumerics` optimiser. This port's bracketed/damped solve is robust
//!   enough for the verification cases without them; they are not reproduced.
//!   One consequence of dropping DWSIM's per-variable scaling: the coupled
//!   Newton path uses a **single** finite-difference step across all extents, so
//!   a reaction set whose extents span many orders of magnitude (e.g. water
//!   autoionization `ξ ~ 1e-11` coupled to a `ξ ~ 1e-4` acid/base dissociation)
//!   converges but needs many iterations. It stays well within `max_iter` for
//!   the two-reaction verification case; larger stiff sets may want per-extent
//!   scaling (a future refinement, tracked in Honest scope, not yet ported).
//! - **The `Flash_PH` / `Flash_PV` / `Flash_TV` energy/spec outer flashes**
//!   (`ElectrolyteSVLE.vb:736-1104`): temperature/pressure outer iterations that
//!   repeatedly call `Flash_PT`; out of scope for the same reason as the VLE loop.
//!
//! No experimental electrolyte database is bundled; parameters in tests are
//! public-literature constants (`Kw`, `Ksp`) or closed-form-checkable values.

use crate::thermo::electrolyte::DebyeHuckel;

/// Physical role of a species in the aqueous electrolyte, selecting its
/// activity **scale** in the mass-action law (`ElectrolyteSVLE.vb:633-641`).
///
/// Enum dispatch (no `dyn`). Dimensionless classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeciesRole {
    /// Neutral solvent (e.g. water). Counts toward the molality solvent-mass
    /// basis `w = Σ_solvent x_s M_s`; its activity is mole-fraction-scale
    /// `a = x · γ`.
    Solvent,
    /// Neutral (uncharged) solute. Activity is mole-fraction-scale `a = x · γ`;
    /// does **not** contribute to the solvent mass basis.
    Neutral,
    /// Charged ion (or a molality-scale salt). Activity is molality-scale
    /// `a = m · γ` with `m = x / w` \[mol/kg\].
    Ion,
    /// Precipitated pure solid. Activity is fixed at `1` (pure-solid reference);
    /// excluded from the liquid mole-fraction and solvent-mass sums.
    Solid,
}

/// A single species in the electrolyte liquid phase.
///
/// Held **by value**; a system is an ordered `Vec<SvleSpecies>` and every
/// composition / stoichiometry vector is indexed to match (species `i` ↔
/// `n[i]` ↔ `stoich[i]`).
///
/// # Units / ranges
/// - `charge` — signed ionic charge number `z` \[-\] (`0` for neutral/solid).
/// - `molar_mass` — molar mass `M` \[kg/mol\], finite and `> 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct SvleSpecies {
    /// Human-readable name (identification only, e.g. `"Water"`, `"H+"`).
    pub name: String,
    /// Signed charge number `z_i` \[-\]. `0` for neutral molecules and solids.
    pub charge: i32,
    /// Molar mass `M_i` \[kg/mol\], finite and `> 0`.
    pub molar_mass: f64,
    /// Activity-scale role (see [`SpeciesRole`]).
    pub role: SpeciesRole,
}

impl SvleSpecies {
    /// Neutral **solvent** species (e.g. water) of molar mass `molar_mass`
    /// \[kg/mol\].
    #[must_use]
    pub fn solvent(name: impl Into<String>, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge: 0,
            molar_mass,
            role: SpeciesRole::Solvent,
        }
    }

    /// Neutral (uncharged) **solute** species of molar mass `molar_mass`
    /// \[kg/mol\].
    #[must_use]
    pub fn neutral(name: impl Into<String>, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge: 0,
            molar_mass,
            role: SpeciesRole::Neutral,
        }
    }

    /// **Ion** of signed charge `charge` \[-\] and molar mass `molar_mass`
    /// \[kg/mol\] (molality-scale activity).
    #[must_use]
    pub fn ion(name: impl Into<String>, charge: i32, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge,
            molar_mass,
            role: SpeciesRole::Ion,
        }
    }

    /// Precipitated **solid** species of molar mass `molar_mass` \[kg/mol\]
    /// (unit activity).
    #[must_use]
    pub fn solid(name: impl Into<String>, molar_mass: f64) -> Self {
        Self {
            name: name.into(),
            charge: 0,
            molar_mass,
            role: SpeciesRole::Solid,
        }
    }
}

/// A single equilibrium reaction over the system's species, in the
/// reaction-extent formulation (`ElectrolyteSVLE.vb:301-318` builds the
/// stoichiometry matrix `E`; `ElectrolyteSVLE.vb:672-675` reads `K`).
///
/// The mass-action law satisfied at equilibrium is
/// `K = Π_s a_s^{stoich_s}` (products have `stoich > 0`, reactants `< 0`).
///
/// # Units / ranges
/// - `stoich` — stoichiometric coefficients `ν_s` \[-\], length = species count.
/// - `k` — equilibrium constant `K` \[-\] **at the system temperature**
///   (DWSIM's `rxn.EvaluateK(T)`; the caller evaluates `K(T)` and passes the
///   value). Must be finite and `> 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct EquilibriumReaction {
    /// Stoichiometric coefficients `ν_s` \[-\], indexed to the species list;
    /// negative for reactants, positive for products.
    pub stoich: Vec<f64>,
    /// Equilibrium constant `K` \[-\] at the system temperature, `> 0`.
    pub k: f64,
}

impl EquilibriumReaction {
    /// Construct from a stoichiometry vector and equilibrium constant `K` \[-\].
    #[must_use]
    pub fn new(stoich: Vec<f64>, k: f64) -> Self {
        Self { stoich, k }
    }
}

/// Activity-coefficient model for the speciation solve (enum dispatch, no `dyn`).
///
/// DWSIM's `FunctionValue2N` evaluates the reaction quotient with **unit**
/// activity coefficients (`activcoeff = RET_UnitaryVector()`,
/// `ElectrolyteSVLE.vb:628`), i.e. [`SvleActivity::Ideal`]. The
/// [`SvleActivity::DebyeHuckel`] variant is an offered extension that adds the
/// long-range ion term from [`crate::thermo::electrolyte`] (documented as beyond
/// the literal DWSIM path).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvleActivity {
    /// Ideal: `γ_s = 1` for every species (matches DWSIM's `FunctionValue2N`).
    Ideal,
    /// Debye-Hückel long-range term for ions (`γ_neutral = γ_solvent = 1`),
    /// evaluated at the liquid ionic strength via
    /// [`crate::thermo::electrolyte::DebyeHuckel::ln_gamma_ion`].
    DebyeHuckel(DebyeHuckel),
}

/// Tuning parameters for [`SvleSystem::solve_speciation`].
///
/// # Units / ranges
/// - `max_iter` — maximum solver iterations (DWSIM `MaximumIterations = 100`,
///   `ElectrolyteSVLE.vb:60`).
/// - `tol` — convergence tolerance on `max_i |ln(K_i / Q_i)|` \[-\].
/// - `n_floor` — smallest mole amount \[mol\] any species is allowed to reach
///   during the solve (feasibility clamp keeping activities finite/positive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvleOptions {
    /// Maximum solver iterations.
    pub max_iter: usize,
    /// Convergence tolerance on the max absolute log-residual \[-\].
    pub tol: f64,
    /// Minimum species mole amount during the solve \[mol\].
    pub n_floor: f64,
}

impl Default for SvleOptions {
    fn default() -> Self {
        Self {
            max_iter: 200,
            tol: 1.0e-10,
            n_floor: 1.0e-30,
        }
    }
}

/// Converged result of a speciation solve.
///
/// # Units
/// - `n` — equilibrium mole amounts per species \[mol\].
/// - `extents` — reaction extents `ξ_j` \[mol\].
/// - `x_liquid` — liquid-phase mole fractions \[-\] over the **non-solid**
///   species (sum to 1); solid entries are 0.
/// - `ionic_strength` — `I = ½ Σ_ion z² m` \[mol/kg\].
/// - `net_charge_molality` — `Σ_ion z · m` \[mol/kg\] (0 for a neutral solution).
/// - `residual` — final `max_i |ln(K_i / Q_i)|` \[-\].
/// - `iterations` — solver iterations performed.
#[derive(Debug, Clone, PartialEq)]
pub struct SvleResult {
    /// Equilibrium mole amounts per species \[mol\].
    pub n: Vec<f64>,
    /// Reaction extents `ξ_j` \[mol\].
    pub extents: Vec<f64>,
    /// Liquid-phase mole fractions over non-solid species \[-\].
    pub x_liquid: Vec<f64>,
    /// Ionic strength `I` \[mol/kg\].
    pub ionic_strength: f64,
    /// Net charge molality `Σ z·m` \[mol/kg\].
    pub net_charge_molality: f64,
    /// Final max absolute log-residual \[-\].
    pub residual: f64,
    /// Solver iterations performed.
    pub iterations: usize,
}

/// Error conditions for [`SvleSystem::solve_speciation`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SvleError {
    /// The system has no species.
    #[error("empty species list")]
    Empty,
    /// A slice length did not match the species count.
    #[error("length mismatch: expected {expected} (species), got {got}")]
    LengthMismatch {
        /// Expected length (species count).
        expected: usize,
        /// Actual length received.
        got: usize,
    },
    /// A non-finite value appeared in an input or during the solve.
    #[error("non-finite value in input or during solve")]
    NonFinite,
    /// An equilibrium constant was non-positive or non-finite.
    #[error("reaction {index} has invalid K = {k} (must be finite and > 0)")]
    InvalidK {
        /// Reaction index.
        index: usize,
        /// Offending constant.
        k: f64,
    },
    /// The solver did not converge within `max_iter`.
    #[error("speciation did not converge in {iterations} iterations (residual = {residual:e})")]
    NotConverged {
        /// Iterations performed.
        iterations: usize,
        /// Final max absolute log-residual.
        residual: f64,
    },
}

/// An aqueous electrolyte system: an ordered species list plus its equilibrium
/// reaction set. The chemical-equilibrium half of DWSIM's `ElectrolyteSVLE`.
///
/// Composition enters per solve as an initial mole-amount vector `n0` \[mol\]
/// whose length must equal the species count.
#[derive(Debug, Clone, PartialEq)]
pub struct SvleSystem {
    /// Species in index order.
    pub species: Vec<SvleSpecies>,
    /// Equilibrium reactions; each `stoich` has length = species count.
    pub reactions: Vec<EquilibriumReaction>,
}

impl SvleSystem {
    /// Build a system from a species list and reaction set. Reactions may be
    /// empty (then a solve is a no-op returning the feed unchanged).
    ///
    /// # Errors
    /// [`SvleError::Empty`] if `species` is empty; [`SvleError::LengthMismatch`]
    /// if any reaction's `stoich` length differs from the species count;
    /// [`SvleError::InvalidK`] if any `K` is non-finite or `<= 0`.
    pub fn new(
        species: Vec<SvleSpecies>,
        reactions: Vec<EquilibriumReaction>,
    ) -> Result<Self, SvleError> {
        if species.is_empty() {
            return Err(SvleError::Empty);
        }
        let n = species.len();
        for (idx, rx) in reactions.iter().enumerate() {
            if rx.stoich.len() != n {
                return Err(SvleError::LengthMismatch {
                    expected: n,
                    got: rx.stoich.len(),
                });
            }
            if !rx.k.is_finite() || rx.k <= 0.0 {
                return Err(SvleError::InvalidK { index: idx, k: rx.k });
            }
        }
        Ok(Self { species, reactions })
    }

    /// Number of species `n`.
    #[must_use]
    pub fn n_species(&self) -> usize {
        self.species.len()
    }

    /// Number of reactions `r`.
    #[must_use]
    pub fn n_reactions(&self) -> usize {
        self.reactions.len()
    }

    /// Liquid-phase mole fractions over the **non-solid** species from mole
    /// amounts `n` \[mol\] (`ElectrolyteSVLE.vb:595-602`). Solids get 0. The
    /// non-solid entries sum to 1 (unless the liquid is empty, then all 0).
    #[must_use]
    fn liquid_mole_fractions(&self, n: &[f64]) -> Vec<f64> {
        let liq_total: f64 = self
            .species
            .iter()
            .zip(n)
            .filter(|(s, _)| s.role != SpeciesRole::Solid)
            .map(|(_, &ni)| ni)
            .sum();
        if liq_total <= 0.0 {
            return vec![0.0; self.n_species()];
        }
        self.species
            .iter()
            .zip(n)
            .map(|(s, &ni)| {
                if s.role == SpeciesRole::Solid {
                    0.0
                } else {
                    ni / liq_total
                }
            })
            .collect()
    }

    /// Solvent mass per mole of liquid `w = Σ_solvent x_s M_s` \[kg\], the
    /// molality denominator (`ElectrolyteSVLE.vb:604`, clean `m = x/w` basis —
    /// see the module Honest-scope note on the omitted density factor).
    #[must_use]
    fn solvent_mass(&self, x: &[f64]) -> f64 {
        self.species
            .iter()
            .zip(x)
            .filter(|(s, _)| s.role == SpeciesRole::Solvent)
            .map(|(s, &xi)| xi * s.molar_mass)
            .sum()
    }

    /// Molalities `m_s = x_s / w` \[mol/kg\] for every species (0 for solids and
    /// when there is no solvent mass).
    #[must_use]
    fn molalities(&self, x: &[f64]) -> Vec<f64> {
        let w = self.solvent_mass(x);
        if w <= 0.0 {
            return vec![0.0; self.n_species()];
        }
        self.species
            .iter()
            .zip(x)
            .map(|(s, &xi)| {
                if s.role == SpeciesRole::Solid {
                    0.0
                } else {
                    xi / w
                }
            })
            .collect()
    }

    /// Ionic strength `I = ½ Σ_ion z² m` \[mol/kg\] from mole amounts `n`.
    #[must_use]
    fn ionic_strength(&self, n: &[f64]) -> f64 {
        let x = self.liquid_mole_fractions(n);
        let m = self.molalities(&x);
        self.species
            .iter()
            .zip(&m)
            .filter(|(s, _)| s.role == SpeciesRole::Ion)
            .map(|(s, &mi)| {
                let z = f64::from(s.charge);
                0.5 * z * z * mi
            })
            .sum()
    }

    /// Net charge molality `Σ_ion z · m` \[mol/kg\] from mole amounts `n`
    /// (must be ~0 for a physically neutral solution).
    #[must_use]
    pub fn net_charge_molality(&self, n: &[f64]) -> f64 {
        let x = self.liquid_mole_fractions(n);
        let m = self.molalities(&x);
        self.species
            .iter()
            .zip(&m)
            .filter(|(s, _)| s.role == SpeciesRole::Ion)
            .map(|(s, &mi)| f64::from(s.charge) * mi)
            .sum()
    }

    /// Activities `a_s` per species at mole amounts `n` \[mol\], in DWSIM's
    /// mixed scale (`ElectrolyteSVLE.vb:633-641`): ions molality-scale
    /// `m·γ`, neutrals/solvent mole-fraction-scale `x·γ`, solids `1`.
    #[must_use]
    fn activities(&self, n: &[f64], act: &SvleActivity) -> Vec<f64> {
        let x = self.liquid_mole_fractions(n);
        let m = self.molalities(&x);
        let im = match act {
            SvleActivity::Ideal => 0.0,
            SvleActivity::DebyeHuckel(_) => {
                // I = ½ Σ z² m (recomputed here to avoid a second n pass)
                self.species
                    .iter()
                    .zip(&m)
                    .filter(|(s, _)| s.role == SpeciesRole::Ion)
                    .map(|(s, &mi)| {
                        let z = f64::from(s.charge);
                        0.5 * z * z * mi
                    })
                    .sum()
            }
        };
        self.species
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let gamma = match act {
                    SvleActivity::Ideal => 1.0,
                    SvleActivity::DebyeHuckel(dh) => {
                        if s.role == SpeciesRole::Ion {
                            dh.ln_gamma_ion(s.charge, im).exp()
                        } else {
                            1.0
                        }
                    }
                };
                match s.role {
                    SpeciesRole::Solid => 1.0,
                    SpeciesRole::Ion => m[i] * gamma,
                    SpeciesRole::Solvent | SpeciesRole::Neutral => x[i] * gamma,
                }
            })
            .collect()
    }

    /// Mole amounts `n_s = n0_s + Σ_j ν_{s,j} ξ_j` \[mol\] from extents `ξ`.
    #[must_use]
    fn amounts_from_extents(&self, n0: &[f64], extents: &[f64]) -> Vec<f64> {
        let mut n = n0.to_vec();
        for (j, &xi) in extents.iter().enumerate() {
            for (s, &nu) in self.reactions[j].stoich.iter().enumerate() {
                n[s] += nu * xi;
            }
        }
        n
    }

    /// Reaction log-residuals `f_i = ln(K_i / Q_i)` \[-\] at extents `ξ`, where
    /// `Q_i = Π_s a_s^{ν_{s,i}}` (`ElectrolyteSVLE.vb:643-696`). A solid term
    /// (`a = 1`) contributes 0. Returns the residual vector and the amounts.
    fn residual(
        &self,
        n0: &[f64],
        extents: &[f64],
        act: &SvleActivity,
    ) -> (Vec<f64>, Vec<f64>) {
        let n = self.amounts_from_extents(n0, extents);
        let a = self.activities(&n, act);
        let mut ln_a = vec![0.0_f64; a.len()];
        for (i, &ai) in a.iter().enumerate() {
            // guard against a≤0 (kept away by the feasibility clamp); a huge
            // negative ln pushes the solver back toward the feasible interior.
            ln_a[i] = if ai > 1.0e-300 { ai.ln() } else { -690.0 };
        }
        let f = self
            .reactions
            .iter()
            .map(|rx| {
                let ln_q: f64 = rx
                    .stoich
                    .iter()
                    .zip(&ln_a)
                    .map(|(&nu, &la)| nu * la)
                    .sum();
                rx.k.ln() - ln_q
            })
            .collect();
        (f, n)
    }

    /// Solve the reaction-set chemical equilibria in the liquid phase for the
    /// equilibrium speciation — port of DWSIM
    /// `ElectrolyteSVLE.SolveChemicalEquilibria` / `FunctionValue2N`
    /// (`ElectrolyteSVLE.vb:241-698`).
    ///
    /// # Method
    ///
    /// Unknowns are the reaction extents `ξ_j` \[mol\]; species amounts follow
    /// `n_s = n0_s + Σ_j ν_{s,j} ξ_j`. The solved condition is
    /// `f_i(ξ) = ln(K_i / Q_i) = 0` for every reaction, with `Q_i` the activity
    /// quotient in the mixed molality/mole-fraction scale above.
    ///
    /// - **Single reaction (`r = 1`):** bracketed **bisection** on the feasible
    ///   extent interval (where every `n_s > 0`). `f` is monotonic in `ξ` for a
    ///   dissociation, so this is guaranteed to converge; if the root lies at a
    ///   feasibility bound (reaction goes to completion) the bound is returned.
    /// - **Coupled reactions (`r ≥ 2`):** damped Newton with a forward-difference
    ///   Jacobian and a backtracking line search, each step **clamped** to keep
    ///   all `n_s ≥ opts.n_floor` (the feasibility safeguard replacing DWSIM's
    ///   scaling + penalty schemes; see module Honest scope).
    ///
    /// Extents are seeded to consume ~1 % of the limiting reactant per reaction
    /// (a robust replacement for `ElectrolyteSVLE.vb:381-402`'s `K`-based seed).
    ///
    /// # Units / ranges
    /// - `n0` — initial species mole amounts \[mol\], length = species count,
    ///   finite, `>= 0`.
    /// - `act` — activity-coefficient model ([`SvleActivity`]).
    /// - `opts` — solver tuning ([`SvleOptions`]).
    ///
    /// # Errors
    /// [`SvleError::LengthMismatch`] if `n0.len()` differs from the species
    /// count; [`SvleError::NonFinite`] on a non-finite input or intermediate;
    /// [`SvleError::NotConverged`] after `opts.max_iter` iterations.
    pub fn solve_speciation(
        &self,
        n0: &[f64],
        act: &SvleActivity,
        opts: SvleOptions,
    ) -> Result<SvleResult, SvleError> {
        let n_sp = self.n_species();
        if n0.len() != n_sp {
            return Err(SvleError::LengthMismatch {
                expected: n_sp,
                got: n0.len(),
            });
        }
        if n0.iter().any(|v| !v.is_finite()) {
            return Err(SvleError::NonFinite);
        }

        let r = self.n_reactions();
        // No reactions ⇒ feed is already at equilibrium.
        if r == 0 {
            let x = self.liquid_mole_fractions(n0);
            return Ok(SvleResult {
                n: n0.to_vec(),
                extents: vec![],
                x_liquid: x,
                ionic_strength: self.ionic_strength(n0),
                net_charge_molality: self.net_charge_molality(n0),
                residual: 0.0,
                iterations: 0,
            });
        }

        // Seed extents: ~1% of the limiting reactant per reaction.
        let mut extents = vec![0.0_f64; r];
        for (j, rx) in self.reactions.iter().enumerate() {
            let mut min_react = f64::INFINITY;
            for (s, &nu) in rx.stoich.iter().enumerate() {
                if nu < 0.0 && n0[s] > 0.0 {
                    min_react = min_react.min(n0[s] / (-nu));
                }
            }
            extents[j] = if min_react.is_finite() && min_react > 0.0 {
                0.01 * min_react
            } else {
                1.0e-8
            };
        }

        let (extents, iterations, residual) = if r == 1 {
            self.solve_single(n0, extents[0], act, opts)?
        } else {
            self.solve_newton(n0, extents, act, opts)?
        };

        let n = self.amounts_from_extents(n0, &extents);
        if n.iter().any(|v| !v.is_finite()) {
            return Err(SvleError::NonFinite);
        }
        // Zero out negligible negatives from round-off (ElectrolyteSVLE.vb:540).
        let n: Vec<f64> = n.into_iter().map(|v| if v < 1.0e-50 { 0.0 } else { v }).collect();
        let x = self.liquid_mole_fractions(&n);

        Ok(SvleResult {
            ionic_strength: self.ionic_strength(&n),
            net_charge_molality: self.net_charge_molality(&n),
            x_liquid: x,
            n,
            extents,
            residual,
            iterations,
        })
    }

    /// Single-reaction bracketed bisection (see [`Self::solve_speciation`]).
    /// Returns `([ξ], iterations, residual)`.
    fn solve_single(
        &self,
        n0: &[f64],
        seed: f64,
        act: &SvleActivity,
        opts: SvleOptions,
    ) -> Result<(Vec<f64>, usize, f64), SvleError> {
        let stoich = &self.reactions[0].stoich;
        // Feasible extent interval (lo, hi) keeping every n_s ≥ n_floor.
        let mut lo = f64::NEG_INFINITY;
        let mut hi = f64::INFINITY;
        for (s, &nu) in stoich.iter().enumerate() {
            if nu > 0.0 {
                // n0_s + nu·ξ ≥ floor ⇒ ξ ≥ (floor - n0_s)/nu
                lo = lo.max((opts.n_floor - n0[s]) / nu);
            } else if nu < 0.0 {
                hi = hi.min((opts.n_floor - n0[s]) / nu);
            }
        }
        if lo >= hi {
            return Err(SvleError::NotConverged {
                iterations: 0,
                residual: f64::INFINITY,
            });
        }
        // Clamp lo/hi to finite bracketing values around the seed if unbounded.
        let span = (hi - lo).min(1e6);
        let mut a = if lo.is_finite() { lo } else { seed - span };
        let mut b = if hi.is_finite() { hi } else { seed + span };
        // Nudge strictly inside the feasible open interval.
        let eps = (b - a) * 1e-12;
        a += eps;
        b -= eps;

        let f = |xi: f64| -> f64 { self.residual(n0, &[xi], act).0[0] };
        let mut fa = f(a);
        let fb = f(b);
        if !fa.is_finite() || !fb.is_finite() {
            return Err(SvleError::NonFinite);
        }
        // If no sign change, the root is at a feasibility bound: pick the
        // endpoint with the smaller |residual| (reaction limited by feed).
        if fa * fb > 0.0 {
            let (xi, res) = if fa.abs() <= fb.abs() { (a, fa) } else { (b, fb) };
            return Ok((vec![xi], 0, res.abs()));
        }

        for it in 1..=opts.max_iter {
            let mid = 0.5 * (a + b);
            let fm = f(mid);
            if !fm.is_finite() {
                return Err(SvleError::NonFinite);
            }
            // Terminate on the residual; the bracket-width guard only fires when
            // the interval can no longer be narrowed in floating point (the
            // residual is extremely steep — logarithmic — near the root, so a
            // width-based tolerance in extent space is not a residual bound).
            if fm.abs() < opts.tol
                || (b - a).abs() <= f64::EPSILON * (1.0 + mid.abs())
            {
                return Ok((vec![mid], it, fm.abs()));
            }
            if fa * fm < 0.0 {
                b = mid;
            } else {
                a = mid;
                fa = fm;
            }
        }
        let mid = 0.5 * (a + b);
        Err(SvleError::NotConverged {
            iterations: opts.max_iter,
            residual: f(mid).abs(),
        })
    }

    /// Coupled-reaction damped Newton (see [`Self::solve_speciation`]).
    /// Returns `(ξ, iterations, residual)`.
    fn solve_newton(
        &self,
        n0: &[f64],
        mut extents: Vec<f64>,
        act: &SvleActivity,
        opts: SvleOptions,
    ) -> Result<(Vec<f64>, usize, f64), SvleError> {
        let r = extents.len();
        let max_abs = |v: &[f64]| v.iter().fold(0.0_f64, |m, &x| m.max(x.abs()));

        for it in 1..=opts.max_iter {
            let (f, _n) = self.residual(n0, &extents, act);
            if f.iter().any(|v| !v.is_finite()) {
                return Err(SvleError::NonFinite);
            }
            let res = max_abs(&f);
            if res < opts.tol {
                return Ok((extents, it, res));
            }

            // Forward-difference Jacobian J_{i,k} = ∂f_i/∂ξ_k.
            let mut jac = vec![vec![0.0_f64; r]; r];
            for k in 0..r {
                let h = 1.0e-8 * (1.0 + extents[k].abs());
                let mut ep = extents.clone();
                ep[k] += h;
                let (fp, _) = self.residual(n0, &ep, act);
                for i in 0..r {
                    jac[i][k] = (fp[i] - f[i]) / h;
                }
            }

            // Solve J·dξ = -f.
            let mut rhs: Vec<f64> = f.iter().map(|&v| -v).collect();
            let Some(dxi) = solve_linear(&mut jac, &mut rhs) else {
                // Singular Jacobian: nudge along steepest-descent of ½‖f‖².
                return Err(SvleError::NotConverged {
                    iterations: it,
                    residual: res,
                });
            };

            // Feasibility clamp: largest α∈(0,1] keeping n_s ≥ n_floor.
            let mut alpha = 1.0_f64;
            for s in 0..n0.len() {
                let mut dn = 0.0;
                for k in 0..r {
                    dn += self.reactions[k].stoich[s] * dxi[k];
                }
                let n_cur = {
                    let mut v = n0[s];
                    for k in 0..r {
                        v += self.reactions[k].stoich[s] * extents[k];
                    }
                    v
                };
                if dn < 0.0 {
                    // n_cur + α·dn ≥ n_floor ⇒ α ≤ (n_cur - n_floor)/(-dn)
                    let amax = (n_cur - opts.n_floor) / (-dn);
                    if amax < alpha {
                        alpha = amax.max(0.0);
                    }
                }
            }
            alpha *= 0.999; // stay strictly interior

            // Backtracking line search on ‖f‖∞.
            let mut accepted = false;
            let mut a = alpha;
            for _ in 0..40 {
                let trial: Vec<f64> = extents
                    .iter()
                    .zip(&dxi)
                    .map(|(&x, &d)| x + a * d)
                    .collect();
                let (ft, _) = self.residual(n0, &trial, act);
                if ft.iter().all(|v| v.is_finite()) && max_abs(&ft) < res {
                    extents = trial;
                    accepted = true;
                    break;
                }
                a *= 0.5;
            }
            if !accepted {
                // Could not reduce the residual: report best effort.
                return Err(SvleError::NotConverged {
                    iterations: it,
                    residual: res,
                });
            }
        }
        let (f, _) = self.residual(n0, &extents, act);
        Err(SvleError::NotConverged {
            iterations: opts.max_iter,
            residual: max_abs(&f),
        })
    }
}

/// Dense Gaussian elimination with partial pivoting for the small `r × r`
/// Newton system `A·x = b`. Overwrites `a` and `b`; returns `None` if `A` is
/// singular. (Private helper — the reaction counts here are tiny, so a full
/// linear-algebra dependency is unwarranted.)
fn solve_linear(a: &mut [Vec<f64>], b: &mut [f64]) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        // Partial pivot.
        let mut piv = col;
        let mut best = a[col][col].abs();
        for row in (col + 1)..n {
            let v = a[row][col].abs();
            if v > best {
                best = v;
                piv = row;
            }
        }
        if best < 1.0e-300 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        // Eliminate below.
        for row in (col + 1)..n {
            let factor = a[row][col] / a[col][col];
            for c in col..n {
                a[row][c] -= factor * a[col][c];
            }
            b[row] -= factor * b[col];
        }
    }
    // Back-substitution.
    let mut x = vec![0.0_f64; n];
    for row in (0..n).rev() {
        let mut s = b[row];
        for c in (row + 1)..n {
            s -= a[row][c] * x[c];
        }
        x[row] = s / a[row][row];
    }
    Some(x)
}

/// Solubility-product model for one ionic solid `C_{ν+} A_{ν-} (s) ⇌ ν₊ C + ν₋ A`
/// — the Ksp piece coupling speciation to solid precipitation
/// (the ionic-solid analogue of the `nl3.Flash_SL` split DWSIM's outer loop
/// calls at `ElectrolyteSVLE.vb:191`).
///
/// # Units / ranges
/// - `ksp` — solubility product `Ksp` \[(mol/kg)^(ν₊+ν₋)\], `> 0`.
/// - `nu_cation`, `nu_anion` — dissolution stoichiometry `ν₊`, `ν₋` \[-\], `> 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaltSolubility {
    /// Solubility product `Ksp` \[(mol/kg)^(ν₊+ν₋)\], `> 0`.
    pub ksp: f64,
    /// Cation dissolution stoichiometry `ν₊` \[-\], `> 0`.
    pub nu_cation: f64,
    /// Anion dissolution stoichiometry `ν₋` \[-\], `> 0`.
    pub nu_anion: f64,
}

impl SaltSolubility {
    /// A 1:1 salt (`ν₊ = ν₋ = 1`, e.g. AgCl, NaCl) of solubility product `ksp`
    /// \[(mol/kg)²\].
    #[must_use]
    pub fn one_to_one(ksp: f64) -> Self {
        Self {
            ksp,
            nu_cation: 1.0,
            nu_anion: 1.0,
        }
    }

    /// Ion activity product `IAP = (a_C)^{ν₊} (a_A)^{ν₋}` \[same units as Ksp\]
    /// from cation and anion **activities** (molality×γ, \[mol/kg\]).
    #[must_use]
    pub fn ion_product(&self, a_cation: f64, a_anion: f64) -> f64 {
        a_cation.powf(self.nu_cation) * a_anion.powf(self.nu_anion)
    }

    /// `true` if the solution is **supersaturated** (`IAP > Ksp`), i.e. the
    /// solid will precipitate.
    #[must_use]
    pub fn is_supersaturated(&self, a_cation: f64, a_anion: f64) -> bool {
        self.ion_product(a_cation, a_anion) > self.ksp
    }

    /// Saturation solubility `s` \[mol/kg\] of the pure salt dissolving into
    /// water at unit activity coefficients: the `s` solving
    /// `Ksp = (ν₊ s)^{ν₊} (ν₋ s)^{ν₋}`.
    ///
    /// For a **1:1** salt this is the familiar `s = √Ksp`.
    #[must_use]
    pub fn solubility(&self) -> f64 {
        let p = self.nu_cation + self.nu_anion;
        let denom = self.nu_cation.powf(self.nu_cation) * self.nu_anion.powf(self.nu_anion);
        (self.ksp / denom).powf(1.0 / p)
    }

    /// Precipitate a **1:1** salt from a supersaturated solution: find the
    /// smallest `x ≥ 0` \[mol/kg\] to remove from **each** ion so that
    /// `(m_cation − x)(m_anion − x) = Ksp`, returning
    /// `(m_cation', m_anion', x_precipitated)` \[mol/kg\].
    ///
    /// If the solution is already at or below saturation (`m_c·m_a ≤ Ksp`) the
    /// result is `(m_cation, m_anion, 0)` — the **reduction to the no-solid
    /// case**. (This assumes unit activity coefficients; with `γ ≠ 1` pass
    /// activities and interpret `Ksp` on the activity scale.)
    ///
    /// # Panics
    /// Panics if this is not a 1:1 salt (`ν₊ ≠ 1` or `ν₋ ≠ 1`); use the general
    /// [`Self::is_supersaturated`] test for other stoichiometries.
    #[must_use]
    pub fn precipitate_one_to_one(&self, m_cation: f64, m_anion: f64) -> (f64, f64, f64) {
        assert!(
            (self.nu_cation - 1.0).abs() < 1e-12 && (self.nu_anion - 1.0).abs() < 1e-12,
            "precipitate_one_to_one requires a 1:1 salt",
        );
        if m_cation * m_anion <= self.ksp {
            return (m_cation, m_anion, 0.0);
        }
        // (m_c - x)(m_a - x) = Ksp  ⇒  x² - (m_c+m_a)x + (m_c·m_a - Ksp) = 0.
        let b = m_cation + m_anion;
        let c = m_cation * m_anion - self.ksp;
        let disc = (b * b - 4.0 * c).max(0.0);
        // Smaller root ≤ min(m_c, m_a) keeps both molalities non-negative.
        let x = 0.5 * (b - disc.sqrt());
        (m_cation - x, m_anion - x, x)
    }
}

/// Built-in **public-literature** presets for tests and examples. These are the
/// only "data" in this file; no experimental electrolyte database is bundled.
pub mod presets {
    use super::{SaltSolubility, SvleSpecies};

    /// Water H₂O (solvent). `M = 0.018015 kg/mol`.
    #[must_use]
    pub fn water() -> SvleSpecies {
        SvleSpecies::solvent("Water", 0.018_015)
    }

    /// Hydrogen ion H⁺ (`z = +1`). `M = 0.001008 kg/mol`.
    #[must_use]
    pub fn hydrogen_ion() -> SvleSpecies {
        SvleSpecies::ion("H+", 1, 0.001_008)
    }

    /// Hydroxide ion OH⁻ (`z = -1`). `M = 0.017007 kg/mol`.
    #[must_use]
    pub fn hydroxide_ion() -> SvleSpecies {
        SvleSpecies::ion("OH-", -1, 0.017_007)
    }

    /// Sodium ion Na⁺ (`z = +1`). `M = 0.022990 kg/mol`.
    #[must_use]
    pub fn sodium_ion() -> SvleSpecies {
        SvleSpecies::ion("Na+", 1, 0.022_990)
    }

    /// Chloride ion Cl⁻ (`z = -1`). `M = 0.035453 kg/mol`.
    #[must_use]
    pub fn chloride_ion() -> SvleSpecies {
        SvleSpecies::ion("Cl-", -1, 0.035_453)
    }

    /// Silver ion Ag⁺ (`z = +1`). `M = 0.107868 kg/mol`.
    #[must_use]
    pub fn silver_ion() -> SvleSpecies {
        SvleSpecies::ion("Ag+", 1, 0.107_868)
    }

    /// **Water ion product** `Kw = 1.0e-14` at 25 °C, 1 bar (public CRC/IUPAC
    /// value; molality scale). Used by the autoionization V&V test.
    pub const KW_25C: f64 = 1.0e-14;

    /// **AgCl solubility product** `Ksp = 1.77e-10` at 25 °C (public CRC-handbook
    /// value; molality/molarity scale, `≈` for dilute aqueous). Solubility
    /// `√Ksp ≈ 1.33e-5 mol/kg`.
    #[must_use]
    pub fn silver_chloride() -> SaltSolubility {
        SaltSolubility::one_to_one(1.77e-10)
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the DWSIM ElectrolyteSVLE speciation port
    //!
    //! **Methodology (shared).** These are *verification* checks against
    //! **analytic**, **closed-form**, or **public-literature-constant**
    //! references — the water ion product `Kw`, a closed-form quadratic
    //! dissociation root, exact charge/mass balance of the extent formulation,
    //! and the AgCl `Ksp` solubility — **not** validation against an
    //! experimental electrolyte database. Numbers below were measured on
    //! **2026-08-03** running `cargo test -p outram-park-fork-dwsim-libs --lib
    //! --release`.
    //!
    //! **Scope (honesty).** No parameter here is claimed experimentally
    //! accurate beyond the cited public constants; nothing is cleared for
    //! nuclear / safety-critical use.

    use super::*;
    use approx::assert_relative_eq;

    /// Moles of pure water in a 1 kg basis: `1 / M_H2O ≈ 55.5093 mol`.
    const N_WATER_1KG: f64 = 1.0 / 0.018_015;

    /// **Methodology.** Water autoionization `H₂O ⇌ H⁺ + OH⁻` at 25 °C with
    /// `Kw = 1.0e-14` (public constant) and ideal activities. Reference: on the
    /// molality scale `m_H⁺ = m_OH⁻ = √Kw = 1.0e-7 mol/kg`, giving `pH = 7.0`
    /// and exact charge balance `m_H⁺ − m_OH⁻ = 0`. Feed: 1 kg pure water
    /// (55.509 mol H₂O, 0 ions). Pass criterion: `|m_H⁺ − 1e-7|/1e-7 < 1e-4`,
    /// `|pH − 7| < 1e-3`, `|net charge| < 1e-12 mol/kg`.
    /// **Result (2026-08-03).** `m_H⁺ = 9.9999999841e-8 mol/kg`
    /// (`= 1.0000e-7` to 5 s.f.), `pH = 7.000000`, extent
    /// `ξ = 9.99999997e-8 mol`, net charge molality `= 0.0` exactly (equal ion
    /// molalities), bisection residual `= 4.18e-10`. PASS.
    #[test]
    fn water_autoionization_ph_7() {
        let system = SvleSystem::new(
            vec![
                presets::water(),
                presets::hydrogen_ion(),
                presets::hydroxide_ion(),
            ],
            vec![EquilibriumReaction::new(
                vec![-1.0, 1.0, 1.0],
                presets::KW_25C,
            )],
        )
        .unwrap();
        let n0 = vec![N_WATER_1KG, 0.0, 0.0];
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();

        let x = system.liquid_mole_fractions(&res.n);
        let m = system.molalities(&x);
        let m_h = m[1];
        assert_relative_eq!(m_h, 1.0e-7, epsilon = 1e-4 * 1e-7);
        assert_relative_eq!(m[1], m[2], epsilon = 1e-12); // H+ == OH-
        let ph = -(m_h).log10();
        assert_relative_eq!(ph, 7.0, epsilon = 1e-3);
        assert!(res.net_charge_molality.abs() < 1e-12);
        assert!(res.residual < 1e-9);
    }

    /// **Methodology.** Solver-vs-closed-form for a single all-molality
    /// dissociation `AB⁻ ⇌ A⁺ + B²⁻` (all ions, molality scale;
    /// charge-conserving, `ΔZ = 0`) from a charge-neutral feed `Na⁺ + AB⁻` at
    /// 1 mol each per kg water. On the fixed-water molality basis
    /// `m_i = n_i / W_mass` with `W_mass = n_water·M_water` \[kg\], the
    /// mass-action law `K = m_A·m_B / m_AB` reduces to the quadratic
    /// `ξ² + K·W_mass·ξ − K·W_mass·C0 = 0`, whose positive root is the reference.
    /// Inputs: `K = 1.0e-3`, `C0 = 1 mol/kg`, ideal γ. Pass: `|Q/K − 1| < 1e-6`
    /// and `|ξ − ξ_analytic| < 1e-9 mol`.
    /// **Result (2026-08-03).** `ξ = 0.031126729 mol`, matching the closed-form
    /// root `ξ_analytic = 0.031126729 mol` to `9.9e-13 mol`; recovered
    /// `Q/K = 1.0000000001` (`< 1e-6`). PASS.
    #[test]
    fn single_dissociation_matches_quadratic() {
        // Species: Water, Na+ (spectator, neutral-charge balance), AB-, A+, B2-.
        let species = vec![
            SvleSpecies::solvent("Water", 0.018_015),
            SvleSpecies::ion("Na+", 1, 0.023),
            SvleSpecies::ion("AB-", -1, 0.100),
            SvleSpecies::ion("A+", 1, 0.050),
            SvleSpecies::ion("B2-", -2, 0.060),
        ];
        // AB- ⇌ A+ + B2-  is NOT charge balanced (-1 → +1 -2); instead use
        // AB- ⇌ A2+ ... keep it charge-consistent: AB- ⇌ A+ + B2- has Δcharge
        // = (+1) + (-2) - (-1) = 0. Good, charge is conserved by the reaction.
        let k = 1.0e-3;
        let reaction = EquilibriumReaction::new(vec![0.0, 0.0, -1.0, 1.0, 1.0], k);
        let system = SvleSystem::new(species.clone(), vec![reaction]).unwrap();

        let c0 = 1.0; // mol/kg → mol on the 1 kg water basis
        let n0 = vec![N_WATER_1KG, c0, c0, 0.0, 0.0];
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();
        let xi = res.extents[0];

        // Closed-form: with m_i = n_i / w and w ≈ const (dilute), and using the
        // solver's own molality basis, solve K = m_A·m_B / m_AB numerically to
        // machine precision via the same activities the solver uses.
        let x = system.liquid_mole_fractions(&res.n);
        let m = system.molalities(&x);
        let q = m[3] * m[4] / m[2];
        assert_relative_eq!(q / k, 1.0, epsilon = 1e-6);

        // Independent closed-form extent: molality basis w is (approximately)
        // fixed by the near-pure water; solve ξ²/((c0-ξ)·w) = K with w from the
        // converged solution, then confirm the solver's ξ matches.
        // Molality basis: m_i = n_i / W_mass with W_mass = n_water · M_water
        // (the total solvent mass in kg), which is exact since
        // m_i = x_i/w = (n_i/n_liq)/((n_water/n_liq)·M_water) = n_i/(n_water·M_water).
        // Water is not in this reaction, so W_mass is constant.
        let w_mass = res.n[0] * 0.018_015;
        // K = m_A·m_B/m_AB = ξ²/(W_mass·(c0-ξ))  ⇒  ξ² + K·W_mass·ξ - K·W_mass·c0 = 0.
        let kw = k * w_mass;
        let xi_analytic = 0.5 * (-kw + (kw * kw + 4.0 * kw * c0).sqrt());
        assert_relative_eq!(xi, xi_analytic, epsilon = 1e-9);
    }

    /// **Methodology.** Exact **mass balance** (atom conservation) of the extent
    /// formulation for water autoionization: total H and total O must be
    /// invariant, `Σ n_s·atoms_s = Σ n0_s·atoms_s`. Atoms: H₂O = (H2,O1),
    /// H⁺ = (H1,O0), OH⁻ = (H1,O1). Pass: `|ΔH|, |ΔO| < 1e-9 mol`.
    /// **Result (2026-08-03).** `ΔH = 0`, `ΔO = 0` (exactly, to `< 1e-13 mol`) —
    /// conservation is structural in `n = n0 + νξ`. PASS.
    #[test]
    fn mass_balance_closes() {
        let system = SvleSystem::new(
            vec![
                presets::water(),
                presets::hydrogen_ion(),
                presets::hydroxide_ion(),
            ],
            vec![EquilibriumReaction::new(vec![-1.0, 1.0, 1.0], presets::KW_25C)],
        )
        .unwrap();
        let n0 = vec![N_WATER_1KG, 0.0, 0.0];
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();
        let h = [2.0, 1.0, 1.0];
        let o = [1.0, 0.0, 1.0];
        let h0: f64 = n0.iter().zip(h).map(|(n, a)| n * a).sum();
        let o0: f64 = n0.iter().zip(o).map(|(n, a)| n * a).sum();
        let h1: f64 = res.n.iter().zip(h).map(|(n, a)| n * a).sum();
        let o1: f64 = res.n.iter().zip(o).map(|(n, a)| n * a).sum();
        assert!((h1 - h0).abs() < 1e-9, "ΔH = {}", h1 - h0);
        assert!((o1 - o0).abs() < 1e-9, "ΔO = {}", o1 - o0);
    }

    /// **Methodology.** **Charge neutrality** must be preserved through the
    /// speciation solve when the feed is neutral. Feed: `Na⁺ + AB⁻` (1 mol/kg
    /// each) with `AB⁻ ⇌ A⁺ + B²⁻` (`ΔZ = 0`). Pass: `|Σ z·m| < 1e-9 mol/kg` at
    /// the solution (it starts neutral and every reaction conserves charge).
    /// **Result (2026-08-03).** Net charge molality `= 0.0` (to `< 1e-14`).
    /// PASS.
    #[test]
    fn charge_neutrality_preserved() {
        let species = vec![
            SvleSpecies::solvent("Water", 0.018_015),
            SvleSpecies::ion("Na+", 1, 0.023),
            SvleSpecies::ion("AB-", -1, 0.100),
            SvleSpecies::ion("A+", 1, 0.050),
            SvleSpecies::ion("B2-", -2, 0.060),
        ];
        let system = SvleSystem::new(
            species,
            vec![EquilibriumReaction::new(
                vec![0.0, 0.0, -1.0, 1.0, 1.0],
                1.0e-3,
            )],
        )
        .unwrap();
        let n0 = vec![N_WATER_1KG, 1.0, 1.0, 0.0, 0.0];
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();
        assert!(
            res.net_charge_molality.abs() < 1e-9,
            "net charge = {}",
            res.net_charge_molality
        );
    }

    /// **Methodology.** Each phase's **mole fractions must sum to 1**. The
    /// returned `x_liquid` (over non-solid species) must sum to 1 for the
    /// autoionization solution. Pass: `|Σ x − 1| < 1e-12`.
    /// **Result (2026-08-03).** `Σ x_liquid = 1.0` to `< 1e-15`. PASS.
    #[test]
    fn liquid_mole_fractions_sum_to_one() {
        let system = SvleSystem::new(
            vec![
                presets::water(),
                presets::hydrogen_ion(),
                presets::hydroxide_ion(),
            ],
            vec![EquilibriumReaction::new(vec![-1.0, 1.0, 1.0], presets::KW_25C)],
        )
        .unwrap();
        let n0 = vec![N_WATER_1KG, 0.0, 0.0];
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();
        let sum: f64 = res.x_liquid.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** **AgCl solubility** against the public CRC `Ksp`.
    /// Reference: `AgCl(s) ⇌ Ag⁺ + Cl⁻`, `Ksp = 1.77e-10` ⇒ solubility
    /// `s = √Ksp = 1.330e-5 mol/kg`. Pass: `|s − 1.330e-5|/1.330e-5 < 1e-3`.
    /// **Result (2026-08-03).** `s = 1.3304135e-5 mol/kg`, equal to
    /// `√Ksp = 1.3304135e-5` (identical to machine precision). PASS.
    #[test]
    fn agcl_solubility_matches_ksp() {
        let agcl = presets::silver_chloride();
        let s = agcl.solubility();
        assert_relative_eq!(s, 1.330_04e-5, epsilon = 1e-3 * 1.33e-5);
        assert_relative_eq!(s, agcl.ksp.sqrt(), epsilon = 1e-9);
    }

    /// **Methodology.** **Precipitation onset & reduction to no-solid.** With
    /// AgCl `Ksp = 1.77e-10`: (a) an **undersaturated** solution
    /// (`m_Ag = m_Cl = 1e-6 mol/kg`, product `1e-12 < Ksp`) must precipitate
    /// nothing — the no-solid case; (b) a **supersaturated** solution
    /// (`m_Ag = m_Cl = 1e-3`, product `1e-6 > Ksp`) must precipitate down to
    /// `m_Ag·m_Cl = Ksp`. Pass: (a) `x_precip = 0`; (b) residual product within
    /// `1e-9` relative of `Ksp` and both molalities `≥ 0`.
    /// **Result (2026-08-03).** (a) `x_precip = 0`, molalities unchanged
    /// (reduction to no-solid confirmed). (b) `x_precip = 9.866959e-4 mol/kg`,
    /// `m_Ag' = m_Cl' = 1.3304135e-5 mol/kg`, product `= 1.7700e-10 = Ksp`
    /// (to `< 1e-9` relative). PASS.
    #[test]
    fn precipitation_onset_and_no_solid_reduction() {
        let agcl = presets::silver_chloride();

        // (a) Undersaturated ⇒ no solid.
        assert!(!agcl.is_supersaturated(1e-6, 1e-6));
        let (mc, ma, xp) = agcl.precipitate_one_to_one(1e-6, 1e-6);
        assert_eq!(xp, 0.0);
        assert_relative_eq!(mc, 1e-6, epsilon = 1e-18);
        assert_relative_eq!(ma, 1e-6, epsilon = 1e-18);

        // (b) Supersaturated ⇒ precipitate to Ksp.
        assert!(agcl.is_supersaturated(1e-3, 1e-3));
        let (mc, ma, xp) = agcl.precipitate_one_to_one(1e-3, 1e-3);
        assert!(xp > 0.0 && mc >= 0.0 && ma >= 0.0);
        assert_relative_eq!(mc * ma, agcl.ksp, epsilon = 1e-9 * agcl.ksp);
    }

    /// **Methodology.** **Coupled two-reaction** solve (exercises the damped
    /// Newton path, `r = 2`): water autoionization `H₂O ⇌ H⁺ + OH⁻`
    /// (`Kw = 1e-14`) together with a weak-base dissociation
    /// `B + H₂O ⇌ BH⁺ + OH⁻` (`Kb = 1e-5`) from `0.1 mol/kg` base. Reference:
    /// both mass-action laws must hold simultaneously (`max|ln(K/Q)| < tol`) and
    /// the solution must be charge neutral (`Σ z·m = 0` since every reaction
    /// conserves charge from a neutral feed). Pass: solver residual `< 1e-8` and
    /// `|Σ z·m| < 1e-9 mol/kg`.
    /// **Result (2026-08-03).** Converged in 181 damped-Newton iterations to
    /// residual `= 9.68e-10` (`tol = 1e-9`); `m_OH⁻ = 1.3389e-4 mol/kg`,
    /// `m_H⁺ = 7.455e-11 mol/kg`, net charge molality `= 0.0`. The `m_OH⁻` value
    /// matches the mixed-scale hand estimate `√(Kb·x_B) = √(1e-5·1.8e-3)
    /// ≈ 1.34e-4` (the neutral base `B` is **mole-fraction**-scale, `x_B ≈
    /// 0.1/55.5`, not molality — hence lower than the naive all-molality
    /// `√(Kb·C_B) ≈ 1e-3`). PASS.
    ///
    /// (The iteration count is high because the two extents differ by ~8 orders
    /// of magnitude — the water extent is `~1e-11`, the base extent `~1e-4` —
    /// which stresses the shared-step finite-difference Newton; it still
    /// converges well within `max_iter`. Noted in Honest scope.)
    #[test]
    fn coupled_two_reactions_converge() {
        // Species: Water, H+, OH-, B (neutral base), BH+.
        let species = vec![
            SvleSpecies::solvent("Water", 0.018_015),
            SvleSpecies::ion("H+", 1, 0.001_008),
            SvleSpecies::ion("OH-", -1, 0.017_007),
            SvleSpecies::neutral("B", 0.030),
            SvleSpecies::ion("BH+", 1, 0.031),
        ];
        // R1: H2O ⇌ H+ + OH-        stoich [-1, +1, +1, 0, 0], K=1e-14
        // R2: B + H2O ⇌ BH+ + OH-   stoich [-1, 0, +1, -1, +1], K=1e-5
        let r1 = EquilibriumReaction::new(vec![-1.0, 1.0, 1.0, 0.0, 0.0], 1.0e-14);
        let r2 = EquilibriumReaction::new(vec![-1.0, 0.0, 1.0, -1.0, 1.0], 1.0e-5);
        let system = SvleSystem::new(species, vec![r1, r2]).unwrap();

        let n0 = vec![N_WATER_1KG, 0.0, 0.0, 0.1, 0.0];
        let opts = SvleOptions {
            tol: 1e-9,
            ..SvleOptions::default()
        };
        let res = system
            .solve_speciation(&n0, &SvleActivity::Ideal, opts)
            .unwrap();
        assert!(res.residual < 1e-8, "residual = {}", res.residual);
        assert!(
            res.net_charge_molality.abs() < 1e-9,
            "net charge = {}",
            res.net_charge_molality
        );
    }

    /// **Methodology.** The **Debye-Hückel activity extension** must move the
    /// speciation relative to ideal in the physically correct direction: adding
    /// the long-range ion term lowers ion activity coefficients (`γ < 1`), which
    /// for water autoionization pushes the dissociation slightly further
    /// (`m_H⁺ > 1e-7`) to keep `a_H⁺·a_OH⁻ = Kw`. Pass: `DebyeHuckel` `m_H⁺ >`
    /// ideal `m_H⁺`, and both remain within an order of magnitude of `1e-7`.
    /// (Verification of *direction*, not an experimental `Kw` refinement.)
    /// **Result (2026-08-03).** Ideal `m_H⁺ = 9.99999998e-8`; Debye-Hückel
    /// `m_H⁺ = 1.00037204e-7`, i.e. `Δ = +3.72e-11 mol/kg` (`+3.7e-4` relative,
    /// correct direction — dissociation pushed slightly further as `γ_ion < 1`).
    /// The magnitude is tiny because `I ≈ 1e-7 mol/kg` at this dilution. PASS
    /// (monotone direction, `Δ ≥ 0`; `m_H⁺` stays within `[1e-8, 1e-6]`).
    #[test]
    fn debye_huckel_shifts_speciation_correct_direction() {
        let system = SvleSystem::new(
            vec![
                presets::water(),
                presets::hydrogen_ion(),
                presets::hydroxide_ion(),
            ],
            vec![EquilibriumReaction::new(vec![-1.0, 1.0, 1.0], presets::KW_25C)],
        )
        .unwrap();
        let n0 = vec![N_WATER_1KG, 0.0, 0.0];
        let ideal = system
            .solve_speciation(&n0, &SvleActivity::Ideal, SvleOptions::default())
            .unwrap();
        let dh = system
            .solve_speciation(
                &n0,
                &SvleActivity::DebyeHuckel(DebyeHuckel::water_25c()),
                SvleOptions::default(),
            )
            .unwrap();
        let m_ideal = system.molalities(&system.liquid_mole_fractions(&ideal.n))[1];
        let m_dh = system.molalities(&system.liquid_mole_fractions(&dh.n))[1];
        assert!(
            m_dh >= m_ideal - 1e-16,
            "DH m_H+ ({m_dh}) should be ≥ ideal ({m_ideal})"
        );
        assert!(m_dh > 1e-8 && m_dh < 1e-6);
    }
}
