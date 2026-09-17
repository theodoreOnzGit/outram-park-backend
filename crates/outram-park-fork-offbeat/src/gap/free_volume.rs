// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/gapGasModel/gapFRAPCON.C`
//     (`correct()`'s pressure update, `calcInitialMass()`, `correctDish()`,
//      `correctCrack()`),
//   `offbeatLib/gapGasModel/gapGasTimeTabulated.{H,C}`
//     (the time-tabulated pressure model).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Rod free volume and the internal gas pressure.
//!
//! # What this computes
//!
//! The pressure of the gas inside a fuel rod. That pressure matters twice over:
//! it loads the cladding from the inside (and if it exceeds the coolant pressure,
//! the cladding creeps *outwards*, reopening the gap), and it divides the
//! temperature-jump distance in the gap conductance
//! ([`super::conductance::temperature_jump_distance`]), so a depressurised rod
//! has a worse gap than its geometry suggests.
//!
//! # The model
//!
//! Upstream (`gapFRAPCON`) makes three assumptions, stated in its own class
//! documentation:
//!
//! 1. the gas is ideal;
//! 2. each part of the free volume has its **own** volume and temperature;
//! 3. the pressure equalises **instantaneously** everywhere in the free volume.
//!
//! Assumption 3 is what makes this a single scalar rather than a field. Applying
//! `pV = nRT` to each region at a common pressure and summing the amounts gives
//!
//! ```text
//! p = n R / Σᵢ (Vᵢ / Tᵢ)
//! ```
//!
//! That `Σ V/T` — not `V_total/T_mean` — is the whole content of the model. The
//! two differ whenever the regions are at different temperatures, and in a rod
//! they always are: the gap runs hot, the plenum runs near coolant temperature.
//!
//! # The regions
//!
//! Upstream tracks, and this module represents: the fuel/cladding **gap**, a
//! user-supplied gap volume **offset**, the fuel central **hole**, the pellet
//! **dishes**, the **top** and **bottom plena**, an external gas **reserve**, and
//! the **cracks** in the relocated fuel.
//!
//! # Deferred — this module does not compute volumes
//!
//! Every one of those volumes is computed upstream by walking the mesh:
//!
//! - gap, hole and plenum volumes come from the Gauss–Green surface integral
//!   `V = ⅓ ∮_S (r_s · n) dS` over the **deformed** bounding patches, with
//!   per-face scaling factors built from cutting-plane/edge intersections to
//!   separate the gap from the plena on a non-conformal cylindrical interface;
//! - the region temperatures are face- or cell-area/volume-weighted averages of
//!   the temperature field;
//! - the dish and crack volumes are cell-volume sums over the fuel material.
//!
//! All of that needs mesh topology and the multi-region coupling, and is
//! **deferred**. [`RodFreeVolume`] takes the resulting `(V, T)` or `(V, Σ V/T)`
//! pairs as inputs and does the thermodynamics. The two cell-level *summands*
//! that are pure arithmetic — [`crack_volume_contribution`] and
//! [`dish_volume_contribution`] — are ported, so a caller who has cell volumes
//! can build those two sums itself.
//!
//! # Units
//!
//! Strict SI raw `f64`: m³, kelvin, pascal, mole, kg, m³/K for a `V/T`.

use crate::error::{OffbeatError, Result};
use crate::gap::gas::{GapGasMixture, GAS_CONSTANT};

/// One region of the rod free volume: how big it is and how hot.
///
/// Upstream carries each region as a pair of scalars — a volume and either a
/// temperature or a pre-accumulated `Σ V/T`. Which of the two depends on the
/// region: the plena and the gas reserve are treated as **isothermal**
/// (`V/T` computed from a single mean temperature), while the gap, hole, dish
/// and cracks accumulate `Σ Vᵢ/Tᵢ` face-by-face or cell-by-cell over a
/// non-uniform temperature. Both forms are representable here, and the
/// distinction is preserved rather than flattened, because flattening it would
/// change the pressure.
///
/// # Units
///
/// [`volume`](Self::volume) in m³, [`v_over_t`](Self::v_over_t) in m³/K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreeVolumeRegion {
    /// What this region is, for diagnostics — e.g. `"gap"`, `"top plenum"`.
    name: &'static str,
    /// Region volume \[m³\], `>= 0`.
    volume: f64,
    /// Region `Σ V/T` \[m³/K\], `>= 0`.
    v_over_t: f64,
}

impl FreeVolumeRegion {
    /// An **isothermal** region: volume `volume` \[m³\] all at temperature
    /// `temperature` \[K\].
    ///
    /// `V/T` is computed as `volume / temperature`. Use this for the plena, the
    /// gas reserve and the gap-volume offset — the regions upstream treats with
    /// a single mean temperature.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite volume, or a
    /// non-positive temperature.
    pub fn uniform(name: &'static str, volume: f64, temperature: f64) -> Result<Self> {
        if !(volume >= 0.0) || !volume.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "free-volume region volume",
                value: volume,
                unit: "m^3",
                reason: "must be finite and non-negative",
            });
        }
        if !(temperature > 0.0) || !temperature.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "free-volume region temperature",
                value: temperature,
                unit: "K",
                reason: "must be finite and strictly positive",
            });
        }
        Ok(Self {
            name,
            volume,
            v_over_t: volume / temperature,
        })
    }

    /// A region with a **non-uniform** temperature, given its volume \[m³\] and
    /// its already-accumulated `Σ Vᵢ/Tᵢ` \[m³/K\].
    ///
    /// Use this for the gap, the central hole, the dishes and the cracks — the
    /// regions upstream accumulates face-by-face or cell-by-cell. Supplying the
    /// sum rather than a mean temperature is not a nicety: for a region spanning
    /// 600–1200 K the two differ by several percent in the pressure, and the
    /// error is systematic.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite volume or sum,
    /// or for a non-zero volume with a zero sum (which would imply an infinite
    /// temperature).
    pub fn distributed(name: &'static str, volume: f64, v_over_t: f64) -> Result<Self> {
        if !(volume >= 0.0) || !volume.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "free-volume region volume",
                value: volume,
                unit: "m^3",
                reason: "must be finite and non-negative",
            });
        }
        if !(v_over_t >= 0.0) || !v_over_t.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "free-volume region sum(V/T)",
                value: v_over_t,
                unit: "m^3/K",
                reason: "must be finite and non-negative",
            });
        }
        if volume > 0.0 && v_over_t == 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "free-volume region sum(V/T)",
                value: v_over_t,
                unit: "m^3/K",
                reason: "a region with non-zero volume must have a non-zero sum(V/T); \
                         zero would imply an infinite temperature",
            });
        }
        Ok(Self {
            name,
            volume,
            v_over_t,
        })
    }

    /// An empty region — zero volume, zero `V/T`. Contributes nothing.
    #[must_use]
    pub fn empty(name: &'static str) -> Self {
        Self {
            name,
            volume: 0.0,
            v_over_t: 0.0,
        }
    }

    /// The region's name, for diagnostics.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Region volume \[m³\].
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.volume
    }

    /// Region `Σ V/T` \[m³/K\] — the quantity that actually enters the pressure.
    #[must_use]
    pub fn v_over_t(&self) -> f64 {
        self.v_over_t
    }

    /// Effective (harmonic-mean) temperature \[K\] of the region, `V / (V/T)`.
    ///
    /// For an isothermal region this returns the temperature it was built with,
    /// exactly. For a distributed region it is the temperature a *single*
    /// isothermal region of the same volume would need to hold the same amount
    /// of gas at the same pressure — which is **not** the volume-weighted mean
    /// temperature, and is lower than it whenever the region is non-isothermal.
    ///
    /// Returns `0.0` for an empty region.
    #[must_use]
    pub fn effective_temperature(&self) -> f64 {
        if self.v_over_t > 0.0 {
            self.volume / self.v_over_t
        } else {
            0.0
        }
    }
}

/// The complete free volume of one rod.
///
/// A list of [`FreeVolumeRegion`]s plus the thermodynamics that turns them into
/// a pressure. Mirrors the eight scalars-plus-eight-temperatures upstream
/// carries on `gapFRAPCON`, but named and extensible rather than hard-coded, so
/// a caller modelling a rod without a central hole simply omits that region
/// instead of setting a magic zero.
///
/// # Units
///
/// m³ and m³/K, as the regions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RodFreeVolume {
    regions: Vec<FreeVolumeRegion>,
}

impl RodFreeVolume {
    /// An empty free volume — no regions. Its pressure is undefined until at
    /// least one region with non-zero `V/T` is added.
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Add a region, consuming and returning `self` for chained construction.
    #[must_use]
    pub fn with_region(mut self, region: FreeVolumeRegion) -> Self {
        self.regions.push(region);
        self
    }

    /// Add a region in place.
    pub fn push(&mut self, region: FreeVolumeRegion) {
        self.regions.push(region);
    }

    /// The regions, in insertion order.
    #[must_use]
    pub fn regions(&self) -> &[FreeVolumeRegion] {
        &self.regions
    }

    /// Total free volume \[m³\] — the plain sum of the region volumes.
    ///
    /// This is *not* the quantity that sets the pressure; see
    /// [`Self::total_v_over_t`].
    #[must_use]
    pub fn total_volume(&self) -> f64 {
        self.regions.iter().map(FreeVolumeRegion::volume).sum()
    }

    /// Total `Σ (Vᵢ / Tᵢ)` \[m³/K\] over all regions — the denominator of the
    /// pressure.
    #[must_use]
    pub fn total_v_over_t(&self) -> f64 {
        self.regions.iter().map(FreeVolumeRegion::v_over_t).sum()
    }

    /// Effective (harmonic-mean) gas temperature \[K\] of the whole free volume,
    /// `V_total / Σ(V/T)`.
    ///
    /// The temperature a single isothermal volume of the same total size would
    /// need to give the same pressure for the same amount of gas. Returns `0.0`
    /// for an empty rod.
    #[must_use]
    pub fn effective_temperature(&self) -> f64 {
        let s = self.total_v_over_t();
        if s > 0.0 {
            self.total_volume() / s
        } else {
            0.0
        }
    }

    /// Volume-weighted mean gas temperature \[K\], `Σ VᵢTᵢ / Σ Vᵢ` — upstream's
    /// `gasT` inside `calcInitialMass()`.
    ///
    /// **This is a different average from [`Self::effective_temperature`], and
    /// upstream uses each in a different place**: the volume-weighted mean only
    /// for the one-off initial-mass calculation, and the harmonic mean (through
    /// `Σ V/T`) for every subsequent pressure update. The two are equal only for
    /// an isothermal rod; for a rod with a 900 K gap and a 600 K plenum the
    /// volume-weighted mean is the higher of the two. Reproduced as upstream has
    /// it, and named distinctly so the two cannot be confused.
    ///
    /// Returns `0.0` for an empty rod. Regions built with
    /// [`FreeVolumeRegion::distributed`] contribute their
    /// [`effective_temperature`](FreeVolumeRegion::effective_temperature) here,
    /// which is the closest this port can get to upstream's per-region mean
    /// without the mesh.
    #[must_use]
    pub fn volume_weighted_temperature(&self) -> f64 {
        let v_total = self.total_volume();
        if !(v_total > 0.0) {
            return 0.0;
        }
        let weighted: f64 = self
            .regions
            .iter()
            .map(|r| r.volume() * r.effective_temperature())
            .sum();
        weighted / v_total
    }

    /// Internal gas pressure \[Pa\] for `moles` \[mol\] of gas —
    /// upstream's `gasP_` update in `gapFRAPCON::correct()`.
    ///
    /// ```text
    /// p = n R / Σᵢ (Vᵢ / Tᵢ)
    /// ```
    ///
    /// # Assumptions
    ///
    /// Ideal gas; instantaneous pressure equalisation across the whole free
    /// volume (so this is one scalar for the rod, not a field); each region
    /// isothermal at its own temperature or already summed as `Σ V/T`.
    ///
    /// Returns `0.0` for a rod with no free volume, rather than an infinity.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{FreeVolumeRegion, RodFreeVolume};
    ///
    /// // A single isothermal region must reduce to pV = nRT exactly.
    /// let rod = RodFreeVolume::new()
    ///     .with_region(FreeVolumeRegion::uniform("plenum", 1.0e-5, 600.0).unwrap());
    /// let n = 1.0e-3;
    /// let p = rod.pressure(n);
    /// assert!((p * 1.0e-5 - n * 8.314_462_618_153_24 * 600.0).abs() < 1e-9);
    /// ```
    #[must_use]
    pub fn pressure(&self, moles: f64) -> f64 {
        let s = self.total_v_over_t();
        if !(s > 0.0) || !moles.is_finite() || moles < 0.0 {
            return 0.0;
        }
        moles * GAS_CONSTANT / s
    }

    /// [`Self::pressure`], reporting the degenerate cases instead of returning
    /// zero.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite gas amount, or
    /// for a rod whose total `Σ V/T` is zero (no free volume at all — a rod with
    /// nowhere to put its gas has no defined pressure).
    pub fn pressure_checked(&self, moles: f64) -> Result<f64> {
        if !(moles >= 0.0) || !moles.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas amount",
                value: moles,
                unit: "mol",
                reason: "must be finite and non-negative",
            });
        }
        let s = self.total_v_over_t();
        if !(s > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "rod free volume sum(V/T)",
                value: s,
                unit: "m^3/K",
                reason: "must be strictly positive; a rod with no free volume has no \
                         defined gas pressure",
            });
        }
        Ok(moles * GAS_CONSTANT / s)
    }

    /// Initial gas mass \[kg\] that fills this rod at pressure `p` \[Pa\] —
    /// upstream's `gapFRAPCON::calcInitialMass()`.
    ///
    /// `m = ρ(p, T_vw) · V_total`, where `T_vw` is the **volume-weighted** mean
    /// temperature ([`Self::volume_weighted_temperature`]) and `ρ` the
    /// ideal-gas mixture density ([`GapGasMixture::density`]).
    ///
    /// # Upstream inconsistency, reproduced deliberately
    ///
    /// This uses the volume-weighted mean temperature, whereas the pressure
    /// update uses the harmonic mean through `Σ V/T`. The two disagree for a
    /// non-isothermal rod, so **feeding the resulting mass straight back through
    /// [`Self::pressure`] does not return the pressure you started from** —
    /// there is a step at initialisation. That is upstream's behaviour; it is
    /// reproduced, quantified in this module's tests, and flagged here rather
    /// than silently corrected, because correcting it would shift the whole
    /// beginning-of-life pressure history relative to an OFFBEAT run.
    ///
    /// Returns `0.0` for an empty rod.
    #[must_use]
    pub fn initial_mass(&self, gas: &GapGasMixture, p: f64) -> f64 {
        let v_total = self.total_volume();
        if !(v_total > 0.0) {
            return 0.0;
        }
        gas.density(p, self.volume_weighted_temperature()) * v_total
    }
}

/// Crack free volume \[m³\] contributed by one fuel cell — upstream's
/// `correctCrack()`.
///
/// ```text
/// V_crack = 2 · ε_relocation · V_cell
/// ```
///
/// # Where the factor of two comes from
///
/// Upstream's comment says only *"the following result comes from supposing
/// relocation as a 2D phenomenon"*. The reading that makes the algebra work: the
/// relocation strain `ε` is a **radial** strain (see
/// [`crate::materials::behavioral::relocation`]), and a 2D radial expansion of a
/// disc by `ε` increases its area by `(1+ε)² − 1 ≈ 2ε` to first order. The
/// crack volume opened up is that areal increase times the cell height, i.e.
/// `2 ε V_cell`. The factor is *not* the 3 of a volumetric strain, and it is not
/// arbitrary.
///
/// # Arguments
///
/// - `relocation_strain` — the **radial** relocation strain `ε` \[-\], positive
///   outward, from
///   [`RelocationModel::value`](crate::materials::behavioral::relocation::RelocationModel::value).
/// - `cell_volume` — the fuel cell's volume \[m³\].
///
/// Negative or non-finite inputs contribute zero.
#[must_use]
pub fn crack_volume_contribution(relocation_strain: f64, cell_volume: f64) -> f64 {
    if !relocation_strain.is_finite() || !cell_volume.is_finite() {
        return 0.0;
    }
    (2.0 * relocation_strain * cell_volume).max(0.0)
}

/// Dish free volume \[m³\] contributed by one fuel cell — upstream's
/// `correctDish()`.
///
/// `V_dish = f_dish · V_cell`, where `f_dish` \[-\] is the fuel material's dish
/// fraction: the fraction of a pellet's nominal volume removed by the dishes and
/// chamfers machined into its end faces. Typical LWR values are a few percent.
///
/// Negative or non-finite inputs contribute zero; the fraction is not clamped
/// above, so a caller supplying a fraction above 1 gets an unphysical answer
/// rather than a silent clamp — upstream does not clamp either.
#[must_use]
pub fn dish_volume_contribution(dish_fraction: f64, cell_volume: f64) -> f64 {
    if !dish_fraction.is_finite() || !cell_volume.is_finite() {
        return 0.0;
    }
    (dish_fraction * cell_volume).max(0.0)
}

/// How the rod internal pressure is obtained — upstream's `gasPressureType`
/// entry (`fromModel` / `fixed` / `fromList`).
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
#[derive(Debug, Clone, PartialEq)]
pub enum GasPressureModel {
    /// Compute it from the free volume and the gas inventory — upstream's
    /// `fromModel`, the physically meaningful choice.
    FromFreeVolume,

    /// Hold it at a constant \[Pa\] — upstream's `fixed`.
    ///
    /// Useful to isolate the thermal problem from the pressure feedback in a
    /// verification case; not a model of a real rod, whose pressure rises by a
    /// factor of several through life.
    Fixed(f64),

    /// Read it from a time table — upstream's `fromList` and the
    /// `gapGasTimeTabulated` model.
    ///
    /// Pairs of `(time \[s\], pressure \[Pa\])`, which **must be sorted by
    /// increasing time**. Interpolation is linear between points and **clamped**
    /// outside the table (upstream's `outOfBounds clamp` default), i.e. the
    /// first and last values are held rather than extrapolated. This is how a
    /// pressure history measured or computed by another fuel-performance code is
    /// imposed on an OFFBEAT run.
    Tabulated(Vec<(f64, f64)>),
}

impl GasPressureModel {
    /// The gas pressure \[Pa\] at time `time` \[s\].
    ///
    /// `free_volume` and `moles` are used only by
    /// [`FromFreeVolume`](Self::FromFreeVolume) and ignored by the other two;
    /// they are still required so the call site reads the same whichever model
    /// is selected.
    ///
    /// Returns `0.0` for an empty table.
    #[must_use]
    pub fn pressure(&self, time: f64, free_volume: &RodFreeVolume, moles: f64) -> f64 {
        match self {
            Self::FromFreeVolume => free_volume.pressure(moles),
            Self::Fixed(p) => *p,
            Self::Tabulated(table) => interpolate_clamped(table, time),
        }
    }

    /// Reject an unusable configuration.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for a negative or non-finite fixed
    ///   pressure.
    /// - [`OffbeatError::Mesh`] for an empty or non-monotonic table, or one
    ///   containing a non-finite entry. (`Mesh` is the crate's "a precondition on
    ///   supplied data was violated" variant; a malformed table is exactly
    ///   that.)
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::FromFreeVolume => Ok(()),
            Self::Fixed(p) => {
                if !(*p >= 0.0) || !p.is_finite() {
                    Err(OffbeatError::Unphysical {
                        quantity: "fixed rod gas pressure",
                        value: *p,
                        unit: "Pa",
                        reason: "must be finite and non-negative",
                    })
                } else {
                    Ok(())
                }
            }
            Self::Tabulated(table) => {
                if table.is_empty() {
                    return Err(OffbeatError::Mesh(
                        "tabulated gas-pressure history is empty".to_string(),
                    ));
                }
                for (i, (t, p)) in table.iter().enumerate() {
                    if !t.is_finite() || !p.is_finite() || *p < 0.0 {
                        return Err(OffbeatError::Mesh(format!(
                            "tabulated gas-pressure entry {i} is ({t}, {p}); times must be \
                             finite and pressures finite and non-negative"
                        )));
                    }
                    if i > 0 && *t < table[i - 1].0 {
                        return Err(OffbeatError::Mesh(format!(
                            "tabulated gas-pressure history is not sorted by increasing \
                             time at entry {i}: {t} < {}",
                            table[i - 1].0
                        )));
                    }
                }
                Ok(())
            }
        }
    }
}

/// Linear interpolation in a `(x, y)` table, **clamped** outside its range —
/// upstream's `Function1s::Table` with `outOfBounds clamp`.
///
/// The table must be sorted by increasing `x`; [`GasPressureModel::validate`]
/// checks that. Below the first point the first `y` is returned and above the
/// last point the last `y`, so a history never extrapolates into a nonsense
/// pressure. Returns `0.0` for an empty table.
///
/// Exposed because upstream applies the same rule to its
/// `gasReserveTemperatureList` and to the tabulated mass fractions of
/// `gapGasTimeTabulated`, not only to the pressure.
#[must_use]
pub fn interpolate_clamped(table: &[(f64, f64)], x: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if x <= table[0].0 {
        return table[0].1;
    }
    let last = table.len() - 1;
    if x >= table[last].0 {
        return table[last].1;
    }
    for i in 1..table.len() {
        let (x1, y1) = table[i];
        if x <= x1 {
            let (x0, y0) = table[i - 1];
            let span = x1 - x0;
            if span <= 0.0 {
                return y1;
            }
            return y0 + (y1 - y0) * (x - x0) / span;
        }
    }
    table[last].1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gap::gas::GapGasSpecies;

    /// Self-consistency check — a single isothermal region reduces exactly to
    /// `pV = nRT`.
    ///
    /// **Methodology.** With one region, `Σ V/T = V/T`, so
    /// `p = nR/(V/T) = nRT/V`. Compared against `nRT/V` computed directly for
    /// three (V, T) pairs; tolerance 1e-14 relative. This is an algebraic
    /// identity — the whole `Σ V/T` machinery must not disturb the textbook
    /// single-volume answer.
    ///
    /// **Result** (2026-07-29): all three pairs agreed to within 1e-16 relative.
    #[test]
    fn single_region_reduces_to_the_ideal_gas_law() {
        for (v, t) in [(1.0e-5, 600.0), (2.5e-6, 900.0), (1.0, 300.0)] {
            let rod = RodFreeVolume::new()
                .with_region(FreeVolumeRegion::uniform("plenum", v, t).unwrap());
            let n = 1.0e-3;
            let p = rod.pressure(n);
            let direct = n * GAS_CONSTANT * t / v;
            assert!(
                (p - direct).abs() < 1e-14 * direct,
                "V={v}, T={t}: {p} vs {direct}"
            );
        }
    }

    /// Self-consistency check — `Σ V/T` is not `V_total / T_mean`, and the
    /// difference is recorded.
    ///
    /// **Methodology.** Build a two-region rod: a 1.0e-6 m³ gap at 900 K and a
    /// 1.0e-5 m³ plenum at 600 K, holding 1 mmol of gas. Compute the correct
    /// pressure from `Σ V/T` and compare it against the (wrong) answer from the
    /// volume-weighted mean temperature. Pass criterion: they differ by more
    /// than 1%, demonstrating that the distinction is not cosmetic.
    ///
    /// **Result** (2026-07-29, measured): `Σ V/T = 1.77778e-8 m³/K`, giving
    /// `p = 4.67689e5 Pa`. The volume-weighted mean temperature is 627.273 K,
    /// giving 4.74131e5 Pa — 1.377% higher. The effective (harmonic-mean)
    /// temperature is 618.750 K.
    #[test]
    fn sum_of_v_over_t_differs_from_the_volume_weighted_mean() {
        let rod = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("gap", 1.0e-6, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("top plenum", 1.0e-5, 600.0).unwrap());

        let s = rod.total_v_over_t();
        assert!((s - 1.7778e-8).abs() < 1e-11, "sum(V/T) = {s}");

        let n = 1.0e-3;
        let p = rod.pressure(n);
        assert!((p - 4.6769e5).abs() < 1.0e2, "p = {p}");

        let t_vw = rod.volume_weighted_temperature();
        assert!((t_vw - 627.27).abs() < 0.1, "T_vw = {t_vw}");
        let t_eff = rod.effective_temperature();
        assert!((t_eff - 618.75).abs() < 0.1, "T_eff = {t_eff}");
        assert!(t_vw > t_eff);

        let naive = n * GAS_CONSTANT * t_vw / rod.total_volume();
        let relative = (naive - p).abs() / p;
        assert!(relative > 0.01, "relative difference only {relative}");
        assert!((relative - 0.013_774).abs() < 1e-5, "relative = {relative}");
    }

    /// Self-consistency check — pressure rises with released gas and falls with
    /// free volume.
    ///
    /// **Methodology.** Fix the free volume and double the gas amount: pressure
    /// must double exactly. Fix the gas amount and double every region's volume:
    /// pressure must halve exactly. Both are `p ∝ n/V` at fixed temperature.
    /// Tolerance 1e-14 relative.
    ///
    /// **Result** (2026-07-29): both exact to within 1e-16 relative.
    #[test]
    fn pressure_scales_with_moles_and_inversely_with_volume() {
        let rod = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("gap", 1.0e-6, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("plenum", 1.0e-5, 600.0).unwrap());

        let p1 = rod.pressure(1.0e-3);
        let p2 = rod.pressure(2.0e-3);
        assert!((p2 / p1 - 2.0).abs() < 1e-14);

        let doubled = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("gap", 2.0e-6, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("plenum", 2.0e-5, 600.0).unwrap());
        assert!((doubled.pressure(1.0e-3) / p1 - 0.5).abs() < 1e-14);
    }

    /// Self-consistency check — a distributed region is not the same as an
    /// isothermal one at its volume-weighted temperature.
    ///
    /// **Methodology.** A gap spanning 600 K and 1200 K in two equal halves has
    /// `Σ V/T = V/2/600 + V/2/1200`, whose effective temperature is the harmonic
    /// mean 800 K, not the arithmetic mean 900 K. Assert
    /// [`FreeVolumeRegion::effective_temperature`] returns 800 K.
    ///
    /// **Result** (2026-07-29, measured): 800.0 K exactly, against an arithmetic
    /// mean of 900 K — a 12.5% difference in the temperature and therefore in
    /// the pressure this region supports.
    #[test]
    fn distributed_region_effective_temperature_is_the_harmonic_mean() {
        let v = 1.0e-6;
        let sum = 0.5 * v / 600.0 + 0.5 * v / 1200.0;
        let region = FreeVolumeRegion::distributed("gap", v, sum).unwrap();
        assert!(
            (region.effective_temperature() - 800.0).abs() < 1e-9,
            "T_eff = {}",
            region.effective_temperature()
        );
        // An isothermal region round-trips exactly.
        let iso = FreeVolumeRegion::uniform("plenum", v, 600.0).unwrap();
        assert!((iso.effective_temperature() - 600.0).abs() < 1e-12);
    }

    /// Reproduced upstream inconsistency — the initial mass and the pressure
    /// update use different temperature averages.
    ///
    /// **Methodology.** Fill a two-region rod (1.0e-6 m³ gap at 900 K,
    /// 1.0e-5 m³ plenum at 600 K) with pure helium to a nominal 2.25 MPa using
    /// [`RodFreeVolume::initial_mass`], then feed that mass straight back
    /// through [`RodFreeVolume::pressure`]. If the two used the same temperature
    /// average, the round trip would return 2.25 MPa. It does not, because the
    /// mass calculation uses the volume-weighted mean and the pressure the
    /// harmonic mean. This test **pins upstream's step**, so a future "fix"
    /// cannot land unnoticed.
    ///
    /// **Result** (2026-07-29, measured): initial mass 1.89945e-5 kg; the
    /// round-tripped pressure is 2.21943e6 Pa against the 2.25e6 Pa requested —
    /// a −1.359% step at initialisation, exactly the ratio of the two
    /// temperature averages (618.750 / 627.273 − 1 = −1.359%).
    #[test]
    fn initial_mass_round_trip_shows_the_upstream_temperature_inconsistency() {
        let rod = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("gap", 1.0e-6, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("plenum", 1.0e-5, 600.0).unwrap());
        let helium = GapGasMixture::pure(GapGasSpecies::Helium, 1.0).unwrap();

        let requested = 2.25e6;
        let mass = rod.initial_mass(&helium, requested);
        assert!((mass - 1.899_445e-5).abs() < 1e-10, "m = {mass}");

        let filled = GapGasMixture::pure(GapGasSpecies::Helium, mass).unwrap();
        let round_trip = rod.pressure(filled.moles());
        let step = (round_trip - requested) / requested;
        assert!((round_trip - 2.219_429e6).abs() < 1.0e2, "p = {round_trip}");
        assert!((step + 0.013_587).abs() < 1e-5, "step = {step}");
    }

    /// Self-consistency check — the crack volume follows the documented `2εV`
    /// form and is non-negative.
    ///
    /// **Methodology.** `V_crack = 2 ε V_cell`. Checked at `ε = 0.005` and
    /// `V_cell = 1.0e-9 m³`, and that a negative strain (physically a gap that
    /// has not relocated) contributes zero rather than a negative volume.
    ///
    /// **Result** (2026-07-29): 1.0e-11 m³, and zero for negative strain.
    #[test]
    fn crack_volume_is_twice_the_relocation_strain() {
        assert!((crack_volume_contribution(0.005, 1.0e-9) - 1.0e-11).abs() < 1e-24);
        assert_eq!(crack_volume_contribution(-0.005, 1.0e-9), 0.0);
        assert_eq!(crack_volume_contribution(f64::NAN, 1.0e-9), 0.0);
    }

    /// Self-consistency check — the dish volume is the dish fraction of the cell
    /// volume.
    #[test]
    fn dish_volume_is_the_dish_fraction_of_the_cell() {
        assert!((dish_volume_contribution(0.03, 1.0e-9) - 3.0e-11).abs() < 1e-24);
        assert_eq!(dish_volume_contribution(-0.03, 1.0e-9), 0.0);
    }

    /// Self-consistency check — table interpolation is linear inside and clamped
    /// outside.
    ///
    /// **Methodology.** A three-point pressure history from upstream's own
    /// `gapGasTimeTabulated` documentation example
    /// (`(0, 4.37114e6) (100, 5.68517e6) (1000, 9.65693e6)`). Assert the exact
    /// node values, the midpoint of the first interval, and clamping below 0 s
    /// and above 1000 s.
    ///
    /// **Result** (2026-07-29, measured): node values exact; the midpoint at
    /// 50 s gives 5.028155e6 Pa (the arithmetic mean of the two bracketing
    /// values, as linear interpolation requires); clamped to 4.37114e6 Pa at
    /// −100 s and 9.65693e6 Pa at 10000 s.
    #[test]
    fn tabulated_history_interpolates_linearly_and_clamps() {
        let table = vec![(0.0, 4.37114e6), (100.0, 5.68517e6), (1000.0, 9.65693e6)];
        let model = GasPressureModel::Tabulated(table.clone());
        model.validate().unwrap();

        let rod = RodFreeVolume::new();
        for (t, p) in &table {
            assert!((model.pressure(*t, &rod, 0.0) - p).abs() < 1.0);
        }
        let mid = model.pressure(50.0, &rod, 0.0);
        assert!((mid - 5.028_155e6).abs() < 1.0, "mid = {mid}");
        assert!((model.pressure(-100.0, &rod, 0.0) - 4.37114e6).abs() < 1.0);
        assert!((model.pressure(10_000.0, &rod, 0.0) - 9.65693e6).abs() < 1.0);
    }

    /// Self-consistency check — the fixed and free-volume models do what they
    /// say.
    #[test]
    fn fixed_and_free_volume_models_dispatch_correctly() {
        let rod = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("plenum", 1.0e-5, 600.0).unwrap());

        let fixed = GasPressureModel::Fixed(2.25e6);
        assert_eq!(fixed.pressure(0.0, &rod, 1.0), 2.25e6);
        assert_eq!(fixed.pressure(1.0e6, &rod, 1.0e9), 2.25e6);

        let from_model = GasPressureModel::FromFreeVolume;
        assert!((from_model.pressure(0.0, &rod, 1.0e-3) - rod.pressure(1.0e-3)).abs() < 1e-9);
    }

    /// Self-consistency check — malformed configurations are rejected.
    #[test]
    fn validation_rejects_malformed_input() {
        assert!(FreeVolumeRegion::uniform("gap", -1.0, 600.0).is_err());
        assert!(FreeVolumeRegion::uniform("gap", 1.0e-6, 0.0).is_err());
        assert!(FreeVolumeRegion::distributed("gap", 1.0e-6, 0.0).is_err());
        assert!(FreeVolumeRegion::distributed("gap", 0.0, 0.0).is_ok());

        assert!(GasPressureModel::Fixed(-1.0).validate().is_err());
        assert!(GasPressureModel::Tabulated(vec![]).validate().is_err());
        assert!(
            GasPressureModel::Tabulated(vec![(1.0, 1.0e5), (0.0, 2.0e5)])
                .validate()
                .is_err()
        );
        assert!(GasPressureModel::Tabulated(vec![(0.0, -1.0)])
            .validate()
            .is_err());

        let empty = RodFreeVolume::new();
        assert_eq!(empty.pressure(1.0e-3), 0.0);
        assert!(empty.pressure_checked(1.0e-3).is_err());
        assert!(RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("plenum", 1.0e-5, 600.0).unwrap())
            .pressure_checked(-1.0)
            .is_err());
    }

    /// Self-consistency check — an eight-region rod sums as the sum of its parts.
    ///
    /// **Methodology.** Build all eight regions upstream tracks, and assert the
    /// total volume and total `Σ V/T` equal the plain sums, and that the
    /// pressure equals `nR` over that sum. Empty regions must contribute
    /// nothing.
    ///
    /// **Result** (2026-07-29, measured): total volume 1.50500e-5 m³,
    /// `Σ V/T = 2.25606e-8 m³/K`, giving 3.68539e5 Pa for 1 mmol — an
    /// order-of-magnitude-plausible cold beginning-of-life rod pressure.
    #[test]
    fn eight_region_rod_sums_correctly() {
        let rod = RodFreeVolume::new()
            .with_region(FreeVolumeRegion::uniform("gap", 1.0e-6, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("gap offset", 5.0e-8, 900.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("central hole", 2.0e-6, 1400.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("dishes", 1.0e-6, 1100.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("top plenum", 8.0e-6, 580.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("bottom plenum", 3.0e-6, 570.0).unwrap())
            .with_region(FreeVolumeRegion::uniform("gas reserve", 0.0, 290.0).unwrap())
            .with_region(FreeVolumeRegion::empty("cracks"));

        assert_eq!(rod.regions().len(), 8);
        let v: f64 = rod.regions().iter().map(FreeVolumeRegion::volume).sum();
        let s: f64 = rod.regions().iter().map(FreeVolumeRegion::v_over_t).sum();
        assert!((rod.total_volume() - v).abs() < 1e-20);
        assert!((rod.total_v_over_t() - s).abs() < 1e-20);
        assert!((rod.total_volume() - 1.505_00e-5).abs() < 1e-11, "V = {v}");
        assert!(
            (rod.total_v_over_t() - 2.256_059e-8).abs() < 1e-13,
            "sum = {s}"
        );

        let p = rod.pressure(1.0e-3);
        assert!((p - 3.685_392e5).abs() < 1.0, "p = {p}");
        assert!((p - 1.0e-3 * GAS_CONSTANT / s).abs() < 1e-9 * p);
    }
}
