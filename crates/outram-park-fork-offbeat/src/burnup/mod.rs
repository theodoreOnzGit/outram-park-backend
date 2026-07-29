// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
//
// Ported from, and cross-checked against, these upstream files:
//   offbeatLib/burnup/burnup.H / .C                     (the `none` model)
//   offbeatLib/burnup/constantBurnup.H / .C             (the `constant` model,
//                                                        and `nextDeltaT`)
//   offbeatLib/burnup/burnupFromPower.H / .C            (the `fromPower` model)
//   offbeatLib/fastFlux/fastFlux.H / .C                 (the `none` model)
//   offbeatLib/fastFlux/constantFastFlux.H / .C         (fluence integration)
//   offbeatLib/fastFlux/timeDependentAxialProfile.H/.C  (tabulated flux history)
// with unit conventions cross-read from:
//   offbeatLib/burnup/burnupLassmann.C                  (0.8815 HM fraction,
//                                                        10960 kg/m3 UO2 TD)
//   offbeatLib/materials/.../conductivityMatproUO2.C    (Bu/1000/0.881)
//   offbeatLib/physicsSubSolvers/.../fissionProductsDiffusionSolver.C (%FIMA)
//   offbeatLib/fissionGasRelease/fgrSCIANTIX.C          (312.0e-13 J/fission)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Burnup accumulation and fast-neutron fluence accumulation — the
//! **irradiation-history bookkeeping** of a fuel-performance run.
//!
//! # What this module is for
//!
//! A fuel rod's material properties do not depend only on where it is and how
//! hot it is; they depend on *how much irradiation it has already seen*. Two
//! scalars carry almost all of that memory:
//!
//! - **Burnup** — the thermal energy extracted from the fuel per unit mass of
//!   the heavy metal (uranium + plutonium) it started life with. It is the
//!   standard measure of "how far through its life" a piece of fuel is, and it
//!   drives fuel conductivity degradation, solid and gaseous swelling,
//!   densification, relocation, and fission-gas release.
//! - **Fast fluence** — the time integral of the fast-neutron flux
//!   (conventionally neutrons with energy above 1 MeV). Fast neutrons displace
//!   atoms from lattice sites; the accumulated damage drives irradiation
//!   hardening, irradiation creep and irradiation growth in the cladding, and
//!   anisotropic dimensional change in TRISO pyrolytic-carbon layers.
//!
//! Both are *monotonically accumulating* quantities: this module owns the small
//! amount of state needed to advance them through a timestep, and nothing else.
//!
//! # Units — read this before using anything here
//!
//! Burnup is the classic unit trap in fuel performance, because four different
//! quantities are all called "burnup" and differ by factors of 1000 and by
//! whether the denominator is the *heavy metal* or the whole *oxide*:
//!
//! | Unit | Meaning | Typical LWR discharge |
//! |---|---|---|
//! | MWd/kgHM | MW-days per kg of **initial heavy metal** | ~40-60 |
//! | MWd/tHM (= GWd/tHM x 1000) | same, per **tonne** of heavy metal | ~40 000-60 000 |
//! | MWd/t(oxide) | per tonne of **UO2**, i.e. HM *and* its oxygen | ~35 000-53 000 |
//! | %FIMA | percent of initial heavy-metal atoms fissioned | ~4-6 |
//!
//! **This crate's canonical unit is MWd/kgHM**, matching
//! [`MaterialState::burnup`](crate::materials::MaterialState::burnup).
//!
//! Upstream OFFBEAT is different, and the difference is worth stating precisely
//! because it is easy to mis-port. Upstream's `Bu` field is stored in
//! **MWd/t(oxide)** (see `burnupFromPower.C`, whose update is
//! `Bu += Q*dt_days/rho/1000` with `rho` the *fuel* density and the class
//! documentation reading "burnup in MWd/MT_oxide"), and every use site converts
//! locally — `Bu/1000/0.881` appears throughout the material correlations, which
//! is MWd/t(oxide) -> MWd/kg(oxide) -> MWd/kgHM. Because that conversion is
//! repeated at a dozen call sites upstream, it is exactly the kind of thing that
//! drifts. **This port converts once, here, at the boundary**, and everything
//! downstream receives MWd/kgHM.
//!
//! The heavy-metal mass fraction that appears in that conversion is *not*
//! hard-coded silently: it lives in [`HeavyMetalBasis`], which the caller
//! constructs explicitly. See [`UO2_HEAVY_METAL_MASS_FRACTION`] for why upstream
//! uses two slightly different numbers (`0.881` and `0.8815`) for it.
//!
//! Fluence is in **n/m²** here (matching
//! [`MaterialState::fast_fluence`](crate::materials::MaterialState::fast_fluence)),
//! whereas upstream's `fastFlux`/`fastFluence` fields are in n/cm²/s and n/cm².
//! Convert with [`FLUENCE_PER_CM2_TO_PER_M2`].
//!
//! # What is *not* here
//!
//! Upstream also ships `burnupLassmann` and `burnupLassmannFBR`: a TUBRNP-style
//! radial depletion module that solves a reduced Bateman chain for ~14 nuclides
//! and rebuilds the radial power profile from the resulting fissile-nuclide
//! distribution. That is a neutronics model, not bookkeeping; it needs a flux
//! solution and an axial slice mapper, and it is **not ported here**. Callers
//! needing a radial burnup profile must supply the profile themselves and drive
//! one [`BurnupAccumulator`] per radial ring.
//!
//! Likewise the *axial shape* machinery of upstream's
//! `timeDependentAxialProfile` (which needs a mesh, a pin direction and the
//! `profiles/` library) is out of scope: [`FastFluxModel`] gives the rod-average
//! flux history `phi(t)`, and the caller multiplies by the normalised axial
//! shape `g(z, t)` themselves — exactly the product upstream forms, just with
//! the mesh half left to the mesh layer.
//!
//! # Status
//!
//! Scaffold. No human verification or validation. The tests below are
//! self-consistency and code-equivalence checks against the upstream C++
//! expressions; none of them is a validation against experiment.

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

// ---------------------------------------------------------------------------
// Physical constants and unit-conversion factors
// ---------------------------------------------------------------------------

/// Seconds in a day \[s/d\] — exactly 86 400.
///
/// Named because the burnup update is the one place in a fuel-performance code
/// where a seconds/days mix-up produces a plausible-looking wrong answer rather
/// than an obvious blow-up.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// Joules in one megawatt-day \[J/(MW·d)\] — exactly `1e6 * 86400 = 8.64e10`.
///
/// This is the whole of the burnup unit conversion: energy per unit heavy-metal
/// mass in J/kgHM, divided by this constant, is burnup in MWd/kgHM.
pub const JOULES_PER_MEGAWATT_DAY: f64 = 8.64e10;

/// Theoretical (pore-free) density of stoichiometric UO2 at room temperature
/// \[kg/m³\].
///
/// Value 10 960 kg/m³, taken from upstream `burnupLassmann.C` (line ~625,
/// `rho_hm = 10960*densityFractionAverage*0.8815`). Real fabricated pellets are
/// 94–97 % of this; multiply by the fraction of theoretical density — which is
/// what [`HeavyMetalBasis::uo2`] does.
pub const UO2_THEORETICAL_DENSITY: f64 = 10_960.0;

/// Heavy-metal mass fraction of stoichiometric UO2 \[kg-HM / kg-UO2\].
///
/// # Where the number comes from
///
/// `M(U) / (M(U) + 2 M(O)) = 238.029 / (238.029 + 2 x 15.999) = 0.88150`.
///
/// # Why upstream has two of them
///
/// OFFBEAT is not internally consistent about this constant, and a port that
/// silently picks one hides a real (small) inconsistency in the original:
///
/// - `0.8815` in `burnupLassmann.C` (lines 436–437, 625, 645, 800–812) and in
///   the bundled SCIANTIX (`GlobalVariables.C`, `U_UO2 = 0.8815`);
/// - `0.881` in every material correlation that converts burnup, e.g.
///   `conductivityMatproUO2.C:175`, `swellingFRAPCON.C:132`,
///   `densificationFRAPCON.C:128`, `relocationFRAPCON.C:338`.
///
/// The two differ by 0.057 %, which is far inside the scatter of any of those
/// correlations, so neither is "wrong" — but they *are* different, so this port
/// exposes the fraction as data rather than baking it in. This constant is the
/// more accurate `0.8815`; pass `0.881` to [`HeavyMetalBasis::new`] if you are
/// reproducing an upstream correlation number exactly.
///
/// # For anything that is not UO2
///
/// MOX, U-Pu-Zr metal fuel, UN, UC and TRISO kernels all have different
/// heavy-metal fractions. Do not reuse this value for them — compute the
/// fraction from the actual stoichiometry and pass it to
/// [`HeavyMetalBasis::new`].
pub const UO2_HEAVY_METAL_MASS_FRACTION: f64 = 0.8815;

/// Burnup, in MWd/kgHM, corresponding to 1 % FIMA \[MWd/kgHM per %FIMA\].
///
/// FIMA = "fissions per initial metal atom". Value 9.3706 is upstream's, from
/// `fissionProductsDiffusionSolver.C:119`
/// (`b = Bu*1e-3/0.881/9.3706`, which takes MWd/t-oxide to %FIMA).
///
/// Physically it is the energy released by fissioning 1 % of the initial
/// heavy-metal atoms in a kilogram of uranium. It depends weakly on the isotopic
/// mix and on the energy released per fission, so treat it as a ~1 %-accurate
/// engineering conversion, not an exact identity.
pub const MWD_PER_KGHM_PER_PERCENT_FIMA: f64 = 9.3706;

/// Recoverable energy released per fission \[J\].
///
/// Value `3.12e-11 J` (= 194.7 MeV), which is upstream's `312.0e-13` — see
/// `fgrSCIANTIX.C:649-650` (`Fissionrate = Q/312.0e-13`) and
/// `MatproCreepModel.C:229` (`F = heatSource/312e-13`).
///
/// This is the *recoverable* energy (fission fragments, prompt and delayed
/// neutrons and gammas, beta decay), not the ~202 MeV total including
/// antineutrinos, and not the ~168 MeV fragment kinetic energy alone. It varies
/// by a few percent between fissioning nuclides; upstream uses one number for
/// all of them, and so does this port.
pub const ENERGY_PER_FISSION_J: f64 = 3.12e-11;

/// Multiply a fluence in n/cm² by this to get n/m² \[-\]; likewise a flux in
/// n/cm²/s to get n/m²/s. Exactly `1e4`.
///
/// Upstream stores `fastFlux` in n/cm²/s and `fastFluence` in n/cm²
/// (`constantFastFlux.H`). This crate stores n/m²/s and n/m², matching
/// [`MaterialState::fast_fluence`](crate::materials::MaterialState::fast_fluence).
pub const FLUENCE_PER_CM2_TO_PER_M2: f64 = 1.0e4;

/// Upstream OpenFOAM's `SMALL` \[-\], used as the denominator guard in the
/// adaptive-timestep expressions so a zero increment gives a huge — rather than
/// infinite — next timestep. Value `1e-15`, matching OpenFOAM's `scalar SMALL`.
const OPENFOAM_SMALL: f64 = 1.0e-15;

/// Fission-rate density \[fissions/(m³·s)\] from volumetric heat generation
/// \[W/m³\].
///
/// Divides by [`ENERGY_PER_FISSION_J`]. This is the bridge between the thermal
/// solve (which knows power) and every fission-product model (which needs a
/// fission rate) — fission-gas production in [`crate::fgr`] is driven from it,
/// as is upstream's MATPRO irradiation-creep law.
///
/// # Inputs and range
///
/// `power_density` in W/m³, must be finite and non-negative. A representative
/// LWR pellet-average value is 3–4 x 10⁸ W/m³ (20 kW/m over an 8.2 mm pellet),
/// giving about 1.2 x 10¹⁹ fissions/(m³·s) = 1.2 x 10¹³ fissions/(cm³·s).
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if `power_density` is negative or not finite.
///
/// ```
/// use outram_park_fork_offbeat::burnup::fission_rate_density;
///
/// let f = fission_rate_density(3.79e8).unwrap();
/// assert!(f > 1.0e19 && f < 1.3e19);
/// ```
pub fn fission_rate_density(power_density: f64) -> Result<f64> {
    if !power_density.is_finite() || power_density < 0.0 {
        return Err(OffbeatError::Unphysical {
            quantity: "volumetric heat generation",
            value: power_density,
            unit: "W/m^3",
            reason: "must be finite and non-negative",
        });
    }
    Ok(power_density / ENERGY_PER_FISSION_J)
}

// ---------------------------------------------------------------------------
// HeavyMetalBasis
// ---------------------------------------------------------------------------

/// The mass basis burnup is measured against: how many kilograms of **initial
/// heavy metal** sit in a cubic metre of fuel.
///
/// # Why this is a separate type
///
/// Burnup is energy per unit *initial heavy-metal* mass, but a thermal solver
/// only knows energy per unit *volume*. Converting between them needs two
/// numbers — the fuel's bulk density and the heavy-metal fraction of that
/// density — and getting either wrong scales every burnup-dependent correlation
/// in the run by a constant factor that is easy to miss and hard to find.
/// Upstream spreads these two numbers across a density-field lookup and a
/// literal `0.881` (or `0.8815`) repeated at a dozen call sites. Here they are
/// one explicitly-constructed value.
///
/// # Fields, units and ranges
///
/// - bulk fuel density \[kg/m³\] — the density of the fuel **including** its
///   as-fabricated porosity, i.e. theoretical density x fraction of theoretical
///   density. Must be > 0. Typical LWR UO2: ~10 400 kg/m³.
/// - heavy-metal mass fraction \[-\] — kg of U + Pu per kg of fuel. Must be in
///   `(0, 1]`. UO2: 0.8815 ([`UO2_HEAVY_METAL_MASS_FRACTION`]); metal fuel: 1.0
///   for pure U, ~0.9 for U-10Zr.
///
/// # Assumptions
///
/// The basis is the **as-fabricated** one and never changes during irradiation.
/// That is correct by definition — burnup is per *initial* heavy metal, so the
/// denominator is deliberately frozen even though the fuel's actual heavy-metal
/// content falls as it is fissioned, and its bulk density changes with
/// densification, swelling and thermal expansion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeavyMetalBasis {
    fuel_density: f64,
    heavy_metal_fraction: f64,
}

impl HeavyMetalBasis {
    /// Build a basis from an explicit bulk fuel density \[kg/m³\] and
    /// heavy-metal mass fraction \[-\].
    ///
    /// Use this for anything that is not UO2 — MOX, metal fuel, carbide,
    /// nitride, a TRISO kernel — computing the fraction from the actual
    /// stoichiometry.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if `fuel_density` is not finite and strictly
    /// positive, or if `heavy_metal_fraction` is not in `(0, 1]`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::burnup::HeavyMetalBasis;
    ///
    /// // Explicitly reproducing upstream's material-correlation constant 0.881.
    /// let basis = HeavyMetalBasis::new(10_412.0, 0.881).unwrap();
    /// assert!((basis.heavy_metal_density() - 9172.972).abs() < 1e-2);
    /// ```
    pub fn new(fuel_density: f64, heavy_metal_fraction: f64) -> Result<Self> {
        if !fuel_density.is_finite() || fuel_density <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fuel bulk density",
                value: fuel_density,
                unit: "kg/m^3",
                reason: "must be finite and strictly positive",
            });
        }
        if !heavy_metal_fraction.is_finite()
            || heavy_metal_fraction <= 0.0
            || heavy_metal_fraction > 1.0
        {
            return Err(OffbeatError::Unphysical {
                quantity: "heavy-metal mass fraction",
                value: heavy_metal_fraction,
                unit: "-",
                reason: "must be finite and within (0, 1]",
            });
        }
        Ok(Self {
            fuel_density,
            heavy_metal_fraction,
        })
    }

    /// Basis for stoichiometric UO2 at a given fraction of theoretical density.
    ///
    /// Uses [`UO2_THEORETICAL_DENSITY`] (10 960 kg/m³) and
    /// [`UO2_HEAVY_METAL_MASS_FRACTION`] (0.8815), the pair upstream's
    /// `burnupLassmann.C:625` combines as `10960*densityFraction*0.8815`.
    ///
    /// # Inputs
    ///
    /// `density_fraction` \[-\] is the fraction of theoretical density of the
    /// as-fabricated pellet, in `(0, 1]`. LWR pellets are 0.94–0.97.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if `density_fraction` is not in `(0, 1]`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::burnup::HeavyMetalBasis;
    ///
    /// let basis = HeavyMetalBasis::uo2(0.95).unwrap();
    /// // 10960 * 0.95 * 0.8815
    /// assert!((basis.heavy_metal_density() - 9178.2).abs() < 0.1);
    /// ```
    pub fn uo2(density_fraction: f64) -> Result<Self> {
        if !density_fraction.is_finite() || density_fraction <= 0.0 || density_fraction > 1.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "UO2 fraction of theoretical density",
                value: density_fraction,
                unit: "-",
                reason: "must be finite and within (0, 1]",
            });
        }
        Self::new(
            UO2_THEORETICAL_DENSITY * density_fraction,
            UO2_HEAVY_METAL_MASS_FRACTION,
        )
    }

    /// Bulk fuel density \[kg/m³\], including as-fabricated porosity.
    #[must_use]
    pub fn fuel_density(&self) -> f64 {
        self.fuel_density
    }

    /// Heavy-metal mass fraction \[-\], kg of U + Pu per kg of fuel.
    #[must_use]
    pub fn heavy_metal_fraction(&self) -> f64 {
        self.heavy_metal_fraction
    }

    /// Initial heavy-metal density \[kg-HM/m³\] — the product of the two fields.
    ///
    /// This is the denominator of the burnup update: a volumetric energy
    /// deposition \[J/m³\] divided by it is J/kgHM.
    #[must_use]
    pub fn heavy_metal_density(&self) -> f64 {
        self.fuel_density * self.heavy_metal_fraction
    }
}

// ---------------------------------------------------------------------------
// BurnupAccumulator
// ---------------------------------------------------------------------------

/// Accumulates local burnup and fast-neutron fluence through an irradiation
/// history.
///
/// # What it represents
///
/// One **cell's worth** of irradiation memory: the total energy extracted per kg
/// of initial heavy metal, and the total fast-neutron fluence, since beginning
/// of life. It is deliberately tiny and owns its data by value, so a mesh-wide
/// field of them is a plain `Vec<BurnupAccumulator>` with no lifetimes and no
/// shared borrows.
///
/// # The two updates
///
/// Per timestep of length `dt` \[s\]:
///
/// - burnup: `Bu += Q · dt / (rho_fuel · f_HM) / 8.64e10` \[MWd/kgHM\], with `Q`
///   the volumetric heat generation \[W/m³\]. This is upstream
///   `burnupFromPower.C`'s `Bu += Q*dt_days/rho/1000`, with the
///   oxide-to-heavy-metal conversion (which upstream defers to each use site)
///   folded in here.
/// - fluence: `Phi += phi · dt` \[n/m²\], upstream
///   `constantFastFlux::advanceFluence`.
///
/// Both are explicit (forward-Euler) integrations using the *end-of-step* power
/// and flux, exactly as upstream does. That is first-order accurate in `dt`;
/// over a slow irradiation with smoothly varying power the error is negligible,
/// but during a fast ramp the timestep must be small enough that power is nearly
/// constant across it — which is what [`Self::next_time_step`] is for.
///
/// # Units
///
/// State is carried in the crate's canonical units — burnup MWd/kgHM, fluence
/// n/m² — so [`Self::apply_to`] can write straight into a
/// [`MaterialState`](crate::materials::MaterialState). Every accessor names its
/// unit; nothing here is "just a number".
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::burnup::{BurnupAccumulator, HeavyMetalBasis};
///
/// let basis = HeavyMetalBasis::uo2(0.95).unwrap();
/// let mut acc = BurnupAccumulator::new(basis);
///
/// // 379 MW/m^3 (about 20 kW/m in an 8.2 mm pellet) for 1000 days,
/// // with a fast flux of 1e18 n/m^2/s (= 1e14 n/cm^2/s).
/// acc.advance(3.79e8, 1.0e18, 1000.0 * 86_400.0).unwrap();
///
/// assert!((acc.burnup_mwd_per_kg_hm() - 41.3).abs() < 0.1);
/// assert!((acc.fast_fluence_per_m2() - 8.64e25).abs() < 1e20);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnupAccumulator {
    /// Burnup \[MWd/kgHM\].
    burnup: f64,
    /// Fast fluence \[n/m²\], E > 1 MeV.
    fast_fluence: f64,
    /// Mass basis the burnup is measured against.
    basis: HeavyMetalBasis,
    /// Burnup added by the most recent `advance_burnup` \[MWd/kgHM\].
    last_burnup_increment: f64,
    /// Total irradiation time integrated so far \[s\].
    elapsed_time: f64,
}

impl BurnupAccumulator {
    /// A fresh, unirradiated accumulator: zero burnup, zero fluence, zero
    /// elapsed time, on the supplied mass `basis`.
    ///
    /// This is the beginning-of-life state.
    #[must_use]
    pub fn new(basis: HeavyMetalBasis) -> Self {
        Self {
            burnup: 0.0,
            fast_fluence: 0.0,
            basis,
            last_burnup_increment: 0.0,
            elapsed_time: 0.0,
        }
    }

    /// An accumulator restarted from a known irradiation state.
    ///
    /// Use this when restarting a case from a previous run, or when burnup and
    /// fluence come from an external neutronics code — which is exactly what
    /// upstream's `constant` burnup model is for: it reads `Bu` from the start
    /// time directory and never evolves it.
    ///
    /// # Inputs
    ///
    /// - `burnup` \[MWd/kgHM\], finite and >= 0.
    /// - `fast_fluence` \[n/m²\], finite and >= 0.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if either is negative or not finite.
    pub fn restart(basis: HeavyMetalBasis, burnup: f64, fast_fluence: f64) -> Result<Self> {
        if !burnup.is_finite() || burnup < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "initial burnup",
                value: burnup,
                unit: "MWd/kgHM",
                reason: "must be finite and non-negative",
            });
        }
        if !fast_fluence.is_finite() || fast_fluence < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "initial fast fluence",
                value: fast_fluence,
                unit: "n/m^2",
                reason: "must be finite and non-negative",
            });
        }
        Ok(Self {
            burnup,
            fast_fluence,
            basis,
            last_burnup_increment: 0.0,
            elapsed_time: 0.0,
        })
    }

    /// Advance both burnup and fluence over one timestep.
    ///
    /// Equivalent to [`Self::advance_burnup`] followed by
    /// [`Self::advance_fluence`], and the normal way to drive this type.
    ///
    /// # Inputs
    ///
    /// - `power_density` \[W/m³\] — local volumetric heat generation, >= 0.
    ///   Upstream's `Q` field.
    /// - `fast_flux` \[n/(m²·s)\] — local fast-neutron flux, E > 1 MeV, >= 0.
    ///   Note the unit: upstream's field is n/(cm²·s), a factor
    ///   [`FLUENCE_PER_CM2_TO_PER_M2`] smaller.
    /// - `dt` \[s\] — timestep, >= 0.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite input.
    pub fn advance(&mut self, power_density: f64, fast_flux: f64, dt: f64) -> Result<()> {
        self.advance_burnup(power_density, dt)?;
        self.advance_fluence(fast_flux, dt)
    }

    /// Advance burnup only, from local volumetric power over `dt`.
    ///
    /// `Bu += Q · dt / (rho_HM · 8.64e10)` \[MWd/kgHM\], where `rho_HM` is
    /// [`HeavyMetalBasis::heavy_metal_density`].
    ///
    /// # Inputs
    ///
    /// - `power_density` \[W/m³\], finite and >= 0. Zero is legal and common — a
    ///   shutdown period accrues no burnup.
    /// - `dt` \[s\], finite and >= 0.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite input.
    pub fn advance_burnup(&mut self, power_density: f64, dt: f64) -> Result<()> {
        if !power_density.is_finite() || power_density < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "volumetric heat generation",
                value: power_density,
                unit: "W/m^3",
                reason: "must be finite and non-negative",
            });
        }
        if !dt.is_finite() || dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must be finite and non-negative",
            });
        }

        // J/m^3 -> J/kgHM -> MWd/kgHM. One conversion, at the boundary.
        let increment =
            power_density * dt / self.basis.heavy_metal_density() / JOULES_PER_MEGAWATT_DAY;

        self.burnup += increment;
        self.last_burnup_increment = increment;
        self.elapsed_time += dt;
        Ok(())
    }

    /// Advance fast fluence only, from local fast flux over `dt`.
    ///
    /// `Phi += phi · dt`, the port of upstream
    /// `constantFastFlux::advanceFluence()`.
    ///
    /// # Inputs
    ///
    /// - `fast_flux` \[n/(m²·s)\], finite and >= 0, conventionally E > 1 MeV. An
    ///   LWR cladding sees ~10¹⁷–10¹⁸ n/(m²·s).
    /// - `dt` \[s\], finite and >= 0.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite input.
    ///
    /// # Note on elapsed time
    ///
    /// This method does **not** advance [`Self::elapsed_time`] — only
    /// [`Self::advance_burnup`] does — so that [`Self::advance`], which calls
    /// both, counts the step once rather than twice.
    pub fn advance_fluence(&mut self, fast_flux: f64, dt: f64) -> Result<()> {
        if !fast_flux.is_finite() || fast_flux < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fast-neutron flux",
                value: fast_flux,
                unit: "n/(m^2 s)",
                reason: "must be finite and non-negative",
            });
        }
        if !dt.is_finite() || dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must be finite and non-negative",
            });
        }
        self.fast_fluence += fast_flux * dt;
        Ok(())
    }

    /// Burnup \[MWd/kgHM\] — the crate's canonical unit, and the one
    /// [`MaterialState::burnup`](crate::materials::MaterialState::burnup) wants.
    #[must_use]
    pub fn burnup_mwd_per_kg_hm(&self) -> f64 {
        self.burnup
    }

    /// Burnup \[MWd/tHM\], i.e. per *tonne* of initial heavy metal.
    ///
    /// Numerically 1000 x the GWd/tHM figure. This is the unit most
    /// fuel-performance literature and most benchmark specifications quote.
    #[must_use]
    pub fn burnup_mwd_per_tonne_hm(&self) -> f64 {
        self.burnup * 1000.0
    }

    /// Burnup \[MWd/t(oxide)\] — **upstream OFFBEAT's own `Bu` field unit**.
    ///
    /// Provided so a port result can be compared against an OFFBEAT case output
    /// directly, without the reader having to remember which of the four burnup
    /// units the file is in. It is the heavy-metal-basis value multiplied by the
    /// heavy-metal fraction — the same energy spread over the oxide mass instead
    /// of the metal mass — so it is *smaller* than
    /// [`Self::burnup_mwd_per_tonne_hm`].
    #[must_use]
    pub fn burnup_mwd_per_tonne_oxide(&self) -> f64 {
        self.burnup * 1000.0 * self.basis.heavy_metal_fraction()
    }

    /// Burnup \[J/kgHM\] — energy per unit initial heavy-metal mass, in strict
    /// SI.
    ///
    /// Rarely what a correlation wants, but it is the unambiguous form: no
    /// megawatt-days, no tonnes, no oxide/metal ambiguity.
    #[must_use]
    pub fn burnup_joules_per_kg_hm(&self) -> f64 {
        self.burnup * JOULES_PER_MEGAWATT_DAY
    }

    /// Burnup as \[%FIMA\] — percent of the initial heavy-metal atoms fissioned.
    ///
    /// Divides by [`MWD_PER_KGHM_PER_PERCENT_FIMA`] (9.3706), upstream's factor.
    /// This is an engineering conversion accurate to roughly 1 %: the exact
    /// factor depends on the fissioning isotopic mix and on the energy released
    /// per fission. Typical LWR discharge is 4–6 %FIMA.
    #[must_use]
    pub fn burnup_percent_fima(&self) -> f64 {
        self.burnup / MWD_PER_KGHM_PER_PERCENT_FIMA
    }

    /// Fast fluence \[n/m²\], E > 1 MeV — the crate's canonical unit.
    #[must_use]
    pub fn fast_fluence_per_m2(&self) -> f64 {
        self.fast_fluence
    }

    /// Fast fluence \[n/cm²\], E > 1 MeV — **upstream OFFBEAT's field unit**,
    /// and the unit most cladding irradiation-growth and hardening correlations
    /// are published in. LWR cladding end-of-life is ~10²² n/cm².
    #[must_use]
    pub fn fast_fluence_per_cm2(&self) -> f64 {
        self.fast_fluence / FLUENCE_PER_CM2_TO_PER_M2
    }

    /// The mass basis this accumulator measures burnup against.
    #[must_use]
    pub fn basis(&self) -> HeavyMetalBasis {
        self.basis
    }

    /// Total irradiation time integrated so far \[s\].
    ///
    /// Advanced by [`Self::advance_burnup`] (and hence by [`Self::advance`]),
    /// not by [`Self::advance_fluence`].
    #[must_use]
    pub fn elapsed_time(&self) -> f64 {
        self.elapsed_time
    }

    /// Burnup added by the most recent [`Self::advance_burnup`] call
    /// \[MWd/kgHM\]. Zero for a freshly constructed accumulator.
    ///
    /// This is the quantity the adaptive-timestep criterion is written against;
    /// see [`Self::next_time_step`].
    #[must_use]
    pub fn last_burnup_increment(&self) -> f64 {
        self.last_burnup_increment
    }

    /// Write this accumulator's burnup and fluence into a
    /// [`MaterialState`](crate::materials::MaterialState).
    ///
    /// Only those two fields are touched; temperature, porosity, swelling and
    /// the rest are left exactly as they were. This is the single point at which
    /// irradiation history enters the material correlations, which is the whole
    /// reason the unit conversion happens once, here, rather than at every use
    /// site as upstream does.
    ///
    /// ```
    /// use outram_park_fork_offbeat::burnup::{BurnupAccumulator, HeavyMetalBasis};
    /// use outram_park_fork_offbeat::materials::MaterialState;
    ///
    /// let mut acc = BurnupAccumulator::new(HeavyMetalBasis::uo2(0.95).unwrap());
    /// acc.advance(3.79e8, 1.0e18, 1000.0 * 86_400.0).unwrap();
    ///
    /// let mut state = MaterialState::fresh(900.0);
    /// acc.apply_to(&mut state);
    ///
    /// assert_eq!(state.temperature, 900.0);            // untouched
    /// assert!((state.burnup - 41.3).abs() < 0.1);      // MWd/kgHM
    /// assert!(state.fast_fluence > 8.0e25);            // n/m^2
    /// ```
    pub fn apply_to(&self, state: &mut MaterialState) {
        state.burnup = self.burnup;
        state.fast_fluence = self.fast_fluence;
    }

    /// Timestep \[s\] the burnup model would like to take next, so that the
    /// burnup increment stays under `max_increment`.
    ///
    /// # Methodology (port of `constantBurnup::nextDeltaT`)
    ///
    /// Upstream scales the current timestep by the ratio of the allowed to the
    /// observed burnup increment:
    ///
    /// `dt_next = dt_current · maxBuIncrease / (burnupIncrement + SMALL)`
    ///
    /// with `SMALL = 1e-15`. Upstream's `maxBuIncrease` is read from
    /// `controlDict` as `maxBurnupIncrease` in MWd/kg and immediately multiplied
    /// by 1000 to match its internal MWd/t field; here both arguments are in
    /// MWd/kgHM, so no factor of 1000 appears and none is needed.
    ///
    /// # Inputs
    ///
    /// - `current_dt` \[s\] — the timestep just taken, > 0.
    /// - `max_increment` \[MWd/kgHM\] — the largest burnup step wanted, > 0. A
    ///   typical value is 0.5–2 MWd/kgHM.
    ///
    /// # Returns
    ///
    /// The suggested next timestep \[s\]. It is *advice*, not a constraint: the
    /// caller is expected to take the minimum over this and every other model's
    /// suggestion (fission-gas release has its own — see
    /// [`crate::fgr::next_time_step_from_release_change`]). With a zero burnup
    /// increment the guard makes the answer enormous rather than infinite, which
    /// is upstream's behaviour and is why the caller must clamp it.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if either argument is not finite and
    /// strictly positive.
    pub fn next_time_step(&self, current_dt: f64, max_increment: f64) -> Result<f64> {
        if !current_dt.is_finite() || current_dt <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "current timestep",
                value: current_dt,
                unit: "s",
                reason: "must be finite and strictly positive",
            });
        }
        if !max_increment.is_finite() || max_increment <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "maximum burnup increase per timestep",
                value: max_increment,
                unit: "MWd/kgHM",
                reason: "must be finite and strictly positive",
            });
        }
        Ok(current_dt * max_increment / (self.last_burnup_increment + OPENFOAM_SMALL))
    }
}

// ---------------------------------------------------------------------------
// Fast flux (upstream offbeatLib/fastFlux)
// ---------------------------------------------------------------------------

/// How to interpolate a tabulated history between its time points.
///
/// Port of the two options upstream's `interpolateTableBase` exposes and that
/// `timeDependentAxialProfile` accepts under the `timeInterpolationMethod`
/// keyword (upstream default `linear`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeInterpolation {
    /// Piecewise linear between bracketing points. Upstream's default, and the
    /// right choice for a power or flux ramp.
    #[default]
    Linear,
    /// Piecewise constant: hold the value of the preceding time point until the
    /// next one. The right choice when the table represents discrete operating
    /// states rather than a continuous ramp.
    Step,
}

/// A tabulated rod-average fast-flux history `phi(t)`.
///
/// # What it represents
///
/// The fast-neutron flux (E > 1 MeV) averaged over the rod, as a function of
/// time — an irradiation history read from a reactor-physics calculation or from
/// an experiment's operating record. Port of the `timePoints` / `fastFlux`
/// tables in upstream's `timeDependentAxialProfile` fast-flux model.
///
/// # Units and ranges
///
/// - times \[s\], strictly increasing, at least one point.
/// - fluxes \[n/(m²·s)\], non-negative. **Note the unit differs from upstream**,
///   whose tables are in n/(cm²·s); multiply an upstream table by
///   [`FLUENCE_PER_CM2_TO_PER_M2`] before passing it here.
///
/// # Behaviour outside the table
///
/// Clamped: a query before the first time point returns the first value, after
/// the last returns the last. It never extrapolates, because extrapolating a
/// measured irradiation history is meaningless.
///
/// # What is deliberately missing
///
/// Upstream forms `fastFlux(t, z) = phi(t) · g(z, t)` where `g` is a normalised
/// axial shape supplied by the `axialProfile` class hierarchy, which needs a
/// mesh, a pin direction and the axial extent of the fuel. None of that is
/// ported here — this type gives `phi(t)` and the caller multiplies by their own
/// `g(z, t)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FastFluxHistory {
    times: Vec<f64>,
    fluxes: Vec<f64>,
    method: TimeInterpolation,
}

impl FastFluxHistory {
    /// Build a history from paired time \[s\] and flux \[n/(m²·s)\] tables.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if the two tables have different lengths or
    /// are empty, if any time or flux is not finite, if a flux is negative, or
    /// if the times are not strictly increasing.
    ///
    /// ```
    /// use outram_park_fork_offbeat::burnup::{FastFluxHistory, TimeInterpolation};
    ///
    /// // Upstream's documented example: ramp 0 -> 1e13 n/cm^2/s in 1 h, hold a
    /// // year, ramp down in 1 h — converted here to n/m^2/s.
    /// let h = FastFluxHistory::new(
    ///     vec![0.0, 3_600.0, 31_536_000.0, 31_539_600.0],
    ///     vec![0.0, 1.0e17, 1.0e17, 0.0],
    ///     TimeInterpolation::Linear,
    /// ).unwrap();
    ///
    /// assert!((h.flux_at(1_800.0) - 0.5e17).abs() < 1e10);
    /// ```
    pub fn new(times: Vec<f64>, fluxes: Vec<f64>, method: TimeInterpolation) -> Result<Self> {
        if times.is_empty() || times.len() != fluxes.len() {
            return Err(OffbeatError::Unphysical {
                quantity: "fast-flux history table",
                value: times.len() as f64,
                unit: "points",
                reason: "time and flux tables must be non-empty and of equal length",
            });
        }
        for (i, &t) in times.iter().enumerate() {
            if !t.is_finite() {
                return Err(OffbeatError::Unphysical {
                    quantity: "fast-flux history time point",
                    value: t,
                    unit: "s",
                    reason: "must be finite",
                });
            }
            if i > 0 && t <= times[i - 1] {
                return Err(OffbeatError::Unphysical {
                    quantity: "fast-flux history time point",
                    value: t,
                    unit: "s",
                    reason: "time points must be strictly increasing",
                });
            }
        }
        for &f in &fluxes {
            if !f.is_finite() || f < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity: "fast-flux history value",
                    value: f,
                    unit: "n/(m^2 s)",
                    reason: "must be finite and non-negative",
                });
            }
        }
        Ok(Self {
            times,
            fluxes,
            method,
        })
    }

    /// Rod-average fast flux \[n/(m²·s)\] at time `t` \[s\].
    ///
    /// Clamped outside the tabulated range (see the type docs). A non-finite `t`
    /// returns the first tabulated value, which is the same as clamping.
    #[must_use]
    pub fn flux_at(&self, t: f64) -> f64 {
        let n = self.times.len();
        if !t.is_finite() || t <= self.times[0] {
            return self.fluxes[0];
        }
        if t >= self.times[n - 1] {
            return self.fluxes[n - 1];
        }
        // Times are strictly increasing, so scanning for the bracketing pair is
        // correct; these histories are short (hundreds of points at most).
        let hi = self.times.iter().position(|&ti| ti > t).unwrap_or(n - 1);
        let lo = hi - 1;
        match self.method {
            TimeInterpolation::Step => self.fluxes[lo],
            TimeInterpolation::Linear => {
                let w = (t - self.times[lo]) / (self.times[hi] - self.times[lo]);
                self.fluxes[lo] + w * (self.fluxes[hi] - self.fluxes[lo])
            }
        }
    }

    /// The tabulated time points \[s\].
    #[must_use]
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// The tabulated flux values \[n/(m²·s)\].
    #[must_use]
    pub fn fluxes(&self) -> &[f64] {
        &self.fluxes
    }

    /// The interpolation method this history was built with.
    #[must_use]
    pub fn interpolation(&self) -> TimeInterpolation {
        self.method
    }
}

/// Which fast-flux model supplies `phi(t)` to the fluence accumulation.
///
/// # Why an enum and not a trait object
///
/// The set of fast-flux models is closed and known at compile time, so a `match`
/// here is exhaustive: adding a variant makes every dispatch site a compile
/// error rather than a runtime surprise. This is the workspace rule (root
/// `CLAUDE.md`, "No trait objects"), and it also means go-to-definition works on
/// each variant, which it does not on a `dyn` implementation.
///
/// # Variants map one-to-one onto upstream's runtime-selectable models
///
/// | Here | Upstream `fastFlux` typename | Upstream file |
/// |---|---|---|
/// | [`Self::Disabled`] | `none` | `fastFlux.C` |
/// | [`Self::Constant`] | `constant` | `constantFastFlux.C` |
/// | [`Self::Tabulated`] | `timeDependentAxialProfile` | `timeDependentAxialProfile.C` |
///
/// The axial-shape half of `timeDependentAxialProfile` is not ported; see
/// [`FastFluxHistory`].
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FastFluxModel {
    /// No fast flux is modelled; `phi(t) = 0` always, so fluence never grows.
    ///
    /// Upstream's `none` model does not create the `fastFlux`/`fastFluence`
    /// fields at all, so any correlation that needs them fails loudly. This port
    /// cannot do that — it returns zero — so **be aware that selecting this
    /// variant silently gives every fluence-dependent correlation an
    /// unirradiated input.** For a frozen non-zero fluence use [`Self::Constant`]
    /// with a zero flux together with [`BurnupAccumulator::restart`], which is
    /// the analogue of upstream's own advice to prefer `constant` over `none`.
    #[default]
    Disabled,

    /// A time-invariant flux \[n/(m²·s)\]; fluence grows linearly.
    ///
    /// Upstream's `constant` model: the flux field is read once from the start
    /// time directory (or defaults to zero) and never changes, but the fluence
    /// *is* still integrated every timestep.
    Constant(f64),

    /// A tabulated flux history `phi(t)`.
    Tabulated(FastFluxHistory),
}

impl FastFluxModel {
    /// Rod-average fast flux \[n/(m²·s)\] at time `t` \[s\].
    ///
    /// Multiply by your own normalised axial shape `g(z, t)` to get the local
    /// value, exactly as upstream forms `fastFlux(t, z) = phi(t) · g(z, t)`.
    #[must_use]
    pub fn flux_at(&self, t: f64) -> f64 {
        match self {
            Self::Disabled => 0.0,
            Self::Constant(phi) => *phi,
            Self::Tabulated(h) => h.flux_at(t),
        }
    }

    /// Upstream's runtime-selection typename for this model, for logging and
    /// error messages.
    #[must_use]
    pub fn upstream_name(&self) -> &'static str {
        match self {
            Self::Disabled => "none",
            Self::Constant(_) => "constant",
            Self::Tabulated(_) => "timeDependentAxialProfile",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative tolerance used where a value is exact in exact arithmetic but
    /// picks up a rounding or two in `f64`.
    const REL_TOL: f64 = 1e-12;

    fn rel_diff(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
    }

    /// **Code-equivalence verification against the upstream C++ expression.**
    ///
    /// Methodology: upstream `offbeatLib/burnup/burnupFromPower.C`
    /// (`burnupFromPower::correct()`) updates burnup as
    /// `Bu[i] = BuOld[i] + Q[i]*deltaT_days/rho[i]/1000`, with `Bu` in
    /// MWd/t(oxide), `Q` in W/m³, `rho` the *fuel* density in kg/m³ and
    /// `deltaT_days = deltaT_seconds/3600/24`. This test evaluates that
    /// expression literally with the same inputs given to
    /// [`BurnupAccumulator::advance_burnup`], and compares against
    /// [`BurnupAccumulator::burnup_mwd_per_tonne_oxide`].
    ///
    /// Inputs: Q = 3.79e8 W/m³; rho_fuel = 10960 x 0.95 = 10412 kg/m³;
    /// dt = 1000 d = 8.64e7 s; heavy-metal fraction 0.8815.
    /// Pass criterion: relative difference < 1e-12.
    ///
    /// Result (2026-07-29, this port at time of writing): the upstream
    /// expression gives 3.6400e4 MWd/t(oxide) and this port's
    /// `burnup_mwd_per_tonne_oxide()` agrees to a relative difference below
    /// 1e-15 — the two reduce to the same f64 operations up to associativity.
    /// Interpretation: moving the heavy-metal division from the use sites to the
    /// accumulator does not change the number upstream computes; it only changes
    /// where the division happens.
    ///
    /// This is **verification against the upstream implementation**, not
    /// validation against experiment.
    #[test]
    fn matches_upstream_burnup_from_power_expression() {
        let q = 3.79e8_f64; // W/m^3
        let density_fraction = 0.95_f64;
        let rho_fuel = UO2_THEORETICAL_DENSITY * density_fraction; // kg/m^3
        let dt = 1000.0 * SECONDS_PER_DAY; // s

        // Upstream, verbatim: Bu += Q*deltaT_days/rho/1000 -> MWd/t(oxide).
        let dt_days = dt / 3600.0 / 24.0;
        let upstream_mwd_per_tonne_oxide = q * dt_days / rho_fuel / 1000.0;

        let mut acc = BurnupAccumulator::new(HeavyMetalBasis::uo2(density_fraction).unwrap());
        acc.advance_burnup(q, dt).unwrap();

        assert!(
            rel_diff(
                acc.burnup_mwd_per_tonne_oxide(),
                upstream_mwd_per_tonne_oxide
            ) < REL_TOL,
            "port {} vs upstream {}",
            acc.burnup_mwd_per_tonne_oxide(),
            upstream_mwd_per_tonne_oxide
        );

        // ...and the heavy-metal-basis value is the oxide value / 0.8815.
        assert!(
            rel_diff(
                acc.burnup_mwd_per_tonne_hm(),
                upstream_mwd_per_tonne_oxide / UO2_HEAVY_METAL_MASS_FRACTION
            ) < REL_TOL
        );
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Burnup must accumulate linearly under constant power: N steps of `dt`
    /// must give the same answer as one step of `N·dt`, and exactly N times the
    /// burnup of a single step. This tests only that the integrator is the
    /// linear one it claims to be; it says nothing about whether the physics is
    /// right.
    ///
    /// Inputs: Q = 3.79e8 W/m³, dt = 1 d, N = 1000. Pass criterion: relative
    /// difference < 1e-12. Result (2026-07-29): both comparisons pass, largest
    /// observed relative difference below 1e-13.
    #[test]
    fn burnup_accumulates_linearly_under_constant_power() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let q = 3.79e8;
        let dt = SECONDS_PER_DAY;
        let n = 1000;

        let mut stepwise = BurnupAccumulator::new(basis);
        for _ in 0..n {
            stepwise.advance_burnup(q, dt).unwrap();
        }

        let mut one_shot = BurnupAccumulator::new(basis);
        one_shot.advance_burnup(q, dt * f64::from(n)).unwrap();

        assert!(
            rel_diff(
                stepwise.burnup_mwd_per_kg_hm(),
                one_shot.burnup_mwd_per_kg_hm()
            ) < REL_TOL
        );

        let mut single = BurnupAccumulator::new(basis);
        single.advance_burnup(q, dt).unwrap();
        assert!(
            rel_diff(
                stepwise.burnup_mwd_per_kg_hm(),
                single.burnup_mwd_per_kg_hm() * f64::from(n)
            ) < REL_TOL
        );

        // Elapsed time is bookkept correctly.
        assert!(rel_diff(stepwise.elapsed_time(), dt * f64::from(n)) < REL_TOL);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Every burnup accessor must be the same physical quantity in a different
    /// unit, so the conversions must round-trip. Methodology: accumulate an
    /// arbitrary burnup, then check MWd/tHM, MWd/t(oxide), J/kgHM and %FIMA
    /// against their definitions, and convert J/kgHM back to MWd/kgHM. Pass
    /// criterion: relative difference < 1e-12 for each. Result (2026-07-29): all
    /// pass, largest observed relative difference ~2e-16 (one f64 rounding).
    #[test]
    fn burnup_unit_conversions_round_trip() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let mut acc = BurnupAccumulator::new(basis);
        acc.advance_burnup(3.79e8, 1000.0 * SECONDS_PER_DAY)
            .unwrap();

        let bu = acc.burnup_mwd_per_kg_hm();

        assert!(rel_diff(acc.burnup_mwd_per_tonne_hm(), bu * 1000.0) < REL_TOL);
        assert!(
            rel_diff(
                acc.burnup_mwd_per_tonne_oxide(),
                bu * 1000.0 * UO2_HEAVY_METAL_MASS_FRACTION
            ) < REL_TOL
        );
        assert!(rel_diff(acc.burnup_joules_per_kg_hm(), bu * 8.64e10) < REL_TOL);
        assert!(rel_diff(acc.burnup_percent_fima(), bu / 9.3706) < REL_TOL);

        // And the reverse direction: J/kgHM back to MWd/kgHM.
        assert!(rel_diff(acc.burnup_joules_per_kg_hm() / 8.64e10, bu) < REL_TOL);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The MWd/kgHM burnup of a UO2 rod must exceed its MWd/kg(oxide) burnup by
    /// exactly `1/0.8815`, because the same energy is spread over less mass once
    /// the oxygen is excluded. This is the specific error the port is designed to
    /// prevent, so it earns an explicit assertion rather than only a comment.
    /// Pass criterion: relative difference < 1e-12; result (2026-07-29): passes
    /// at ~1e-16.
    #[test]
    fn heavy_metal_basis_is_not_the_oxide_basis() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let mut acc = BurnupAccumulator::new(basis);
        acc.advance_burnup(3.79e8, 500.0 * SECONDS_PER_DAY).unwrap();

        let ratio = acc.burnup_mwd_per_tonne_hm() / acc.burnup_mwd_per_tonne_oxide();
        assert!(rel_diff(ratio, 1.0 / UO2_HEAVY_METAL_MASS_FRACTION) < REL_TOL);
        assert!(ratio > 1.0, "heavy-metal burnup must be the larger number");
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Fluence integrates flux: constant flux over `dt` must give exactly
    /// `flux · dt`, the n/cm² accessor must be exactly 1e-4 of the n/m² one, and
    /// two half-steps must equal one whole step. Pass criterion: relative
    /// difference < 1e-12; result (2026-07-29): passes.
    #[test]
    fn fluence_integrates_flux_and_converts_units() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let flux = 1.0e18_f64; // n/m^2/s == 1e14 n/cm^2/s
        let dt = 1000.0 * SECONDS_PER_DAY;

        let mut acc = BurnupAccumulator::new(basis);
        acc.advance_fluence(flux, dt).unwrap();
        assert!(rel_diff(acc.fast_fluence_per_m2(), flux * dt) < REL_TOL);
        assert!(rel_diff(acc.fast_fluence_per_cm2(), flux * dt / 1.0e4) < REL_TOL);

        let mut halves = BurnupAccumulator::new(basis);
        halves.advance_fluence(flux, dt / 2.0).unwrap();
        halves.advance_fluence(flux, dt / 2.0).unwrap();
        assert!(rel_diff(halves.fast_fluence_per_m2(), acc.fast_fluence_per_m2()) < REL_TOL);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// `advance` must be exactly `advance_burnup` then `advance_fluence`, and
    /// must count elapsed time once, not twice.
    #[test]
    fn advance_is_the_two_partial_advances_and_counts_time_once() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let (q, flux, dt) = (3.79e8, 1.0e18, SECONDS_PER_DAY);

        let mut both = BurnupAccumulator::new(basis);
        both.advance(q, flux, dt).unwrap();

        let mut split = BurnupAccumulator::new(basis);
        split.advance_burnup(q, dt).unwrap();
        split.advance_fluence(flux, dt).unwrap();

        assert_eq!(both.burnup_mwd_per_kg_hm(), split.burnup_mwd_per_kg_hm());
        assert_eq!(both.fast_fluence_per_m2(), split.fast_fluence_per_m2());
        assert_eq!(both.elapsed_time(), dt);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// A restart must reproduce the state it was handed, and `apply_to` must
    /// write burnup and fluence into a `MaterialState` without disturbing any
    /// other field.
    #[test]
    fn restart_and_apply_to_material_state() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let acc = BurnupAccumulator::restart(basis, 42.0, 7.5e25).unwrap();
        assert_eq!(acc.burnup_mwd_per_kg_hm(), 42.0);
        assert_eq!(acc.fast_fluence_per_m2(), 7.5e25);

        let mut state = MaterialState::fresh(1200.0);
        state.porosity = 0.05;
        state.pu_fraction = 0.07;
        acc.apply_to(&mut state);

        assert_eq!(state.burnup, 42.0);
        assert_eq!(state.fast_fluence, 7.5e25);
        assert_eq!(state.temperature, 1200.0);
        assert_eq!(state.porosity, 0.05);
        assert_eq!(state.pu_fraction, 0.07);
    }

    /// **Code-equivalence verification against `constantBurnup::nextDeltaT`.**
    ///
    /// Methodology: upstream computes
    /// `nextDeltaT = deltaT · maxBuIncrease/(burnupIncrement + SMALL)` with
    /// `SMALL = 1e-15`. Inputs here: a single step of `dt = 86400 s` at
    /// Q = 3.79e8 W/m³ (increment ~0.0413 MWd/kgHM), then `max_increment` set to
    /// half and to twice that increment. Pass criterion: the returned timestep is
    /// `dt/2` and `2·dt` respectively, to a relative difference < 1e-9 (the
    /// `SMALL` guard perturbs the result at the ~1e-14 level).
    ///
    /// Result (2026-07-29): 43 200.0 s and 172 800.0 s against expected
    /// 43 200 s and 172 800 s, relative difference < 1e-13 in both cases.
    /// Interpretation: the controller halves the step when the requested burnup
    /// increment is half the observed one, as upstream does.
    #[test]
    fn next_time_step_scales_inversely_with_the_burnup_increment() {
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let dt = SECONDS_PER_DAY;
        let mut acc = BurnupAccumulator::new(basis);
        acc.advance_burnup(3.79e8, dt).unwrap();

        let observed = acc.last_burnup_increment();
        assert!(observed > 0.0);

        let next = acc.next_time_step(dt, observed / 2.0).unwrap();
        assert!(rel_diff(next, dt / 2.0) < 1e-9, "got {next}");

        let next2 = acc.next_time_step(dt, observed * 2.0).unwrap();
        assert!(rel_diff(next2, dt * 2.0) < 1e-9, "got {next2}");

        // A zero increment gives a huge but finite suggestion (upstream's SMALL
        // guard), which the caller is expected to clamp.
        let fresh = BurnupAccumulator::new(basis);
        let huge = fresh.next_time_step(dt, 1.0).unwrap();
        assert!(huge.is_finite() && huge > 1.0e15);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Every unphysical input must be rejected rather than silently producing a
    /// NaN or a negative burnup.
    #[test]
    fn unphysical_inputs_are_rejected() {
        assert!(HeavyMetalBasis::new(-1.0, 0.88).is_err());
        assert!(HeavyMetalBasis::new(10_000.0, 0.0).is_err());
        assert!(HeavyMetalBasis::new(10_000.0, 1.5).is_err());
        assert!(HeavyMetalBasis::new(f64::NAN, 0.88).is_err());
        assert!(HeavyMetalBasis::uo2(0.0).is_err());
        assert!(HeavyMetalBasis::uo2(1.5).is_err());

        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        assert!(BurnupAccumulator::restart(basis, -1.0, 0.0).is_err());
        assert!(BurnupAccumulator::restart(basis, 0.0, -1.0).is_err());

        let mut acc = BurnupAccumulator::new(basis);
        assert!(acc.advance_burnup(-1.0, 1.0).is_err());
        assert!(acc.advance_burnup(1.0, -1.0).is_err());
        assert!(acc.advance_fluence(-1.0, 1.0).is_err());
        assert!(acc.advance_fluence(1.0, f64::INFINITY).is_err());
        assert!(acc.next_time_step(0.0, 1.0).is_err());
        assert!(acc.next_time_step(1.0, 0.0).is_err());

        assert!(fission_rate_density(-1.0).is_err());
    }

    /// **Code-equivalence verification, with an order-of-magnitude sanity note.**
    ///
    /// Methodology: [`fission_rate_density`] must equal upstream's
    /// `Q/312.0e-13` (`fgrSCIANTIX.C:650`). Pass criterion: exact equality with
    /// that expression evaluated in f64.
    ///
    /// Result (2026-07-29): exact. Sanity note (not a validation): a
    /// pellet-average LWR power of 3.79e8 W/m³ gives 1.215e19 fissions/(m³·s) =
    /// 1.215e13 fissions/(cm³·s), the textbook order of magnitude for an LWR
    /// pellet.
    #[test]
    fn fission_rate_matches_the_upstream_conversion() {
        let q = 3.79e8;
        assert_eq!(fission_rate_density(q).unwrap(), q / 312.0e-13);
        assert_eq!(fission_rate_density(0.0).unwrap(), 0.0);

        let f = fission_rate_density(q).unwrap();
        assert!((1.0e19..1.3e19).contains(&f));
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The tabulated flux history must reproduce its own table at the tabulated
    /// points, interpolate linearly between them, clamp outside, and hold the
    /// left value under `Step`. Inputs are upstream's own documented example from
    /// `timeDependentAxialProfile.H` (ramp to 1e13 n/cm²/s in 1 h, hold one year,
    /// ramp down in 1 h), converted to n/m²/s.
    #[test]
    fn flux_history_interpolates_and_clamps() {
        let times = vec![0.0, 3_600.0, 31_536_000.0, 31_539_600.0];
        let fluxes = vec![0.0, 1.0e17, 1.0e17, 0.0];

        let lin =
            FastFluxHistory::new(times.clone(), fluxes.clone(), TimeInterpolation::Linear).unwrap();

        // Exact at the table points.
        for (t, f) in times.iter().zip(fluxes.iter()) {
            assert_eq!(lin.flux_at(*t), *f);
        }
        // Halfway up the first ramp.
        assert!(rel_diff(lin.flux_at(1_800.0), 0.5e17) < 1e-9);
        // Clamped outside, and NaN-safe.
        assert_eq!(lin.flux_at(-1.0e6), 0.0);
        assert_eq!(lin.flux_at(1.0e12), 0.0);
        assert_eq!(lin.flux_at(f64::NAN), 0.0);
        // Flat through the hold.
        assert!(rel_diff(lin.flux_at(1.0e7), 1.0e17) < REL_TOL);

        let step = FastFluxHistory::new(times, fluxes, TimeInterpolation::Step).unwrap();
        assert_eq!(step.flux_at(1_800.0), 0.0);
        assert_eq!(step.flux_at(3_600.1), 1.0e17);
        assert_eq!(step.interpolation(), TimeInterpolation::Step);
        assert_eq!(step.times().len(), 4);
        assert_eq!(step.fluxes().len(), 4);

        // Malformed tables are rejected.
        assert!(FastFluxHistory::new(vec![], vec![], TimeInterpolation::Linear).is_err());
        assert!(FastFluxHistory::new(vec![0.0, 1.0], vec![1.0], TimeInterpolation::Linear).is_err());
        assert!(
            FastFluxHistory::new(vec![1.0, 0.0], vec![1.0, 1.0], TimeInterpolation::Linear)
                .is_err()
        );
        assert!(
            FastFluxHistory::new(vec![0.0, 1.0], vec![1.0, -1.0], TimeInterpolation::Linear)
                .is_err()
        );
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The model enum must dispatch to the right flux, and driving a
    /// [`BurnupAccumulator`] with [`FastFluxModel::Constant`] over 365 daily
    /// steps must give the same fluence as the closed-form `phi · t`.
    #[test]
    fn fast_flux_model_dispatch_and_integration() {
        assert_eq!(FastFluxModel::Disabled.flux_at(1.0e6), 0.0);
        assert_eq!(FastFluxModel::Constant(2.5e17).flux_at(1.0e6), 2.5e17);
        assert_eq!(FastFluxModel::Disabled.upstream_name(), "none");
        assert_eq!(FastFluxModel::Constant(0.0).upstream_name(), "constant");
        assert_eq!(FastFluxModel::default(), FastFluxModel::Disabled);

        let model = FastFluxModel::Constant(1.0e18);
        let basis = HeavyMetalBasis::uo2(0.95).unwrap();
        let mut acc = BurnupAccumulator::new(basis);
        let dt = SECONDS_PER_DAY;
        let mut t = 0.0;
        for _ in 0..365 {
            acc.advance_fluence(model.flux_at(t), dt).unwrap();
            t += dt;
        }
        assert!(rel_diff(acc.fast_fluence_per_m2(), 1.0e18 * 365.0 * dt) < REL_TOL);

        let tab = FastFluxModel::Tabulated(
            FastFluxHistory::new(
                vec![0.0, 100.0],
                vec![0.0, 1.0e17],
                TimeInterpolation::Linear,
            )
            .unwrap(),
        );
        assert_eq!(tab.upstream_name(), "timeDependentAxialProfile");
        assert!(rel_diff(tab.flux_at(50.0), 0.5e17) < 1e-9);
    }
}
