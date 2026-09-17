// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
//
// Ported from, and cross-checked against, these upstream files:
//   offbeatLib/fissionGasRelease/fissionGasRelease.H / .C  (the `none` model,
//                                                           release-fraction
//                                                           bookkeeping and
//                                                           `nextDeltaT`)
//   offbeatLib/fissionGasRelease/fgrSCIANTIXRIA.H / .C     (the transient
//                                                           threshold-venting
//                                                           model, which
//                                                           OFFBEAT implements
//                                                           itself and does NOT
//                                                           delegate to
//                                                           SCIANTIX)
//   offbeatLib/fissionGasRelease/fgrSCIANTIX.H / .C        (Xe/Kr yields, the
//                                                           mole and released-
//                                                           volume accounting,
//                                                           and the coupling
//                                                           surface that is
//                                                           deliberately NOT
//                                                           ported)
//
// No SCIANTIX source is copied, vendored or translated here. SCIANTIX is a
// separate MIT-licensed 0-D grain-scale code by Politecnico di Milano and is not
// part of this port; see `FissionGasReleaseModel::Sciantix`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Fission-gas release (FGR) — how xenon and krypton get out of the fuel, and
//! what that does to the rod.
//!
//! # The physics, for a reader with no fuel-performance background
//!
//! About 30 fission events in 100 produce an atom of xenon or krypton. These are
//! noble gases: they are chemically inert, essentially insoluble in the UO2
//! lattice, and they do not go away. Their fate through life is a three-stage
//! journey:
//!
//! 1. **Born in the grain.** A fission fragment stops within a few micrometres
//!    of where it was created, so the gas atom starts inside a UO2 grain
//!    (typical grain radius 5–10 µm).
//! 2. **Diffuses to the grain boundary.** Thermally activated diffusion carries
//!    the atom to the grain face, where it joins a lenticular bubble. This is
//!    strongly temperature-dependent — below roughly 1000–1200 K almost nothing
//!    moves, above ~1700 K it is fast. Some of the gas is knocked back into the
//!    lattice by passing fission fragments ("irradiation re-solution"), which is
//!    why release is a competition rather than a one-way trip.
//! 3. **Vents to the rod free volume.** Once the grain-boundary bubbles
//!    interlink, or once the fuel cracks, the gas escapes into the gap and
//!    plenum.
//!
//! Two consequences make FGR one of the most important couplings in a
//! fuel-performance code, and both are *bad*:
//!
//! - **Rod pressure rises.** Released gas adds moles to a nearly fixed volume.
//!   At high burnup this can lift rod internal pressure above coolant pressure
//!   and re-open the pellet-cladding gap ("lift-off").
//! - **Gap conductance collapses.** The as-filled helium in the gap is an
//!   excellent conductor for a gas; xenon and krypton are roughly an order of
//!   magnitude worse. Diluting the helium raises the fuel temperature, which
//!   raises the diffusion rate, which releases more gas — a genuine positive
//!   feedback that the coupled solve has to resolve.
//!
//! Gas that does *not* escape is not harmless either: it collects in
//! intragranular and intergranular bubbles and swells the fuel, closing the gap
//! from the other side.
//!
//! # What upstream OFFBEAT actually provides — and what this port contains
//!
//! This matters, because it is easy to assume a fuel-performance code ships a
//! menu of simple empirical FGR correlations. **At upstream commit
//! `80e8445`, OFFBEAT does not.** The whole of
//! `offbeatLib/fissionGasRelease/` is three classes:
//!
//! | Upstream typename | File | What it is |
//! |---|---|---|
//! | `none` | `fissionGasRelease.C` | Release switched off; the gas fields exist but never evolve. |
//! | `SCIANTIX` | `fgrSCIANTIX.C` | A coupling shim that calls **SCIANTIX**, a separate MIT-licensed 0-D grain-scale code (Politecnico di Milano), once per fuel cell per outer iteration. |
//! | `SCIANTIXRIA` | `fgrSCIANTIXRIA.C` | A restart model for reactivity-initiated accidents that **does not call SCIANTIX at all** — it reads the gas inventories left by a base-irradiation `SCIANTIX` run and vents them on temperature/burnup/damage thresholds. |
//!
//! There is no Vitanza-threshold, ANS-5.4, Forsberg-Massih or Booth-diffusion
//! model in the tree to port. Rather than invent one and present it as a port,
//! [`FissionGasReleaseModel`] mirrors exactly what is there:
//!
//! - [`FissionGasReleaseModel::Disabled`] — a faithful port of `none`.
//! - [`FissionGasReleaseModel::TransientVenting`] — a faithful port of the
//!   `SCIANTIXRIA` threshold logic, which is genuinely OFFBEAT's own code.
//! - [`FissionGasReleaseModel::Sciantix`] — **declared but not implemented**. It
//!   returns [`OffbeatError::NotImplemented`]. It never returns zero release,
//!   because a silent zero here would look like "this fuel released no gas",
//!   which is a physically meaningful and dangerously wrong statement.
//!
//! The gas *bookkeeping* that OFFBEAT does around whichever model is selected —
//! Xe/Kr yields, atoms to moles, released volume at reference conditions,
//! release fraction, and the FGR-driven timestep control — **is** ported, as
//! free functions and small value types, because it is model-independent and
//! reusable.
//!
//! # Units
//!
//! Raw `f64` in strict SI, with two conventions worth stating up front:
//!
//! - Gas inventories are **atoms per cubic metre of fuel** \[at/m³\], which is
//!   upstream's convention for the `Gas_grain`, `Gas_boundary` and `Gas_released`
//!   fields.
//! - Release *fractions* here are dimensionless in `[0, 1]`. **Upstream carries
//!   them as percentages**, so `fgr_` in the C++ is 100x these values; see
//!   [`release_fraction`].
//! - Swelling strains are **volumetric** \[-\], not linear. Divide by three for
//!   the linear equivalent, as [`crate::materials::MaterialState`] documents.
//!
//! # Status
//!
//! Scaffold. No human verification or validation. Every test below is a
//! self-consistency or code-equivalence check against the upstream C++
//! expressions; none is a validation against experiment or against a
//! fission-gas-release benchmark.

use crate::error::{OffbeatError, Result};

// ---------------------------------------------------------------------------
// Yields, constants, and gas bookkeeping
// ---------------------------------------------------------------------------

/// Xenon atoms produced per fission \[at/fission\].
///
/// Value 0.268, from upstream `fgrSCIANTIX.C::gasMols()`, which splits the total
/// released fission gas as `Xe = (0.268/0.301)·molFGR`. It is a cumulative
/// fission yield for the stable and long-lived xenon isotopes; it varies by a
/// few percent with the fissioning nuclide (²³⁵U vs ²³⁹Pu) and upstream uses one
/// number for all of them.
pub const XENON_ATOMS_PER_FISSION: f64 = 0.268;

/// Krypton atoms produced per fission \[at/fission\].
///
/// Value 0.033, from upstream `fgrSCIANTIX.C::gasMols()`
/// (`Kr = (0.033/0.301)·molFGR`). Same caveats as
/// [`XENON_ATOMS_PER_FISSION`].
pub const KRYPTON_ATOMS_PER_FISSION: f64 = 0.033;

/// Total stable fission-gas (Xe + Kr) atoms produced per fission
/// \[at/fission\].
///
/// Value 0.301, written as the literal that upstream uses as its denominator in
/// `fgrSCIANTIX.C::gasMols()` (`0.268/0.301`, `0.033/0.301`) rather than as the
/// sum [`XENON_ATOMS_PER_FISSION`] + [`KRYPTON_ATOMS_PER_FISSION`]. The two
/// agree to one unit in the last place — the `f64` sum is
/// `0.30100000000000005` — and using the literal keeps the Xe and Kr fractions
/// exactly upstream's ratios.
///
/// The familiar rule of thumb "about 30 % of fissions make a noble-gas atom" is
/// this number.
pub const FISSION_GAS_ATOMS_PER_FISSION: f64 = 0.301;

/// Avogadro constant \[1/mol\], the 2019 SI defined value 6.02214076e23.
///
/// Upstream uses the rounded `6.02e23` in the mole accumulation
/// (`fgrSCIANTIX.C:673, 680`) and `6.022e23` in the post-processing
/// (`fgrSCIANTIX.C:811, 822`). Those differ from the exact value by 0.036 % and
/// 0.0024 % respectively. This port uses the exact value everywhere, so mole
/// counts here are ~0.04 % below upstream's — far inside any FGR model's
/// uncertainty, but stated so the difference is not mistaken for a bug.
pub const AVOGADRO: f64 = 6.022_140_76e23;

/// Molar gas constant \[J/(mol·K)\], the 2019 SI defined value 8.314462618.
///
/// Upstream uses `8.314` (`fgrSCIANTIX.C:812`), which is the same to 6
/// significant figures.
pub const MOLAR_GAS_CONSTANT: f64 = 8.314_462_618;

/// Reference temperature \[K\] for quoting a released-gas *volume*.
///
/// Value 293 K, upstream's choice in `fgrSCIANTIX.C:812`
/// (`fgrM3_ = nMoles*8.314*293/101325`). Released FGR is conventionally reported
/// as a volume at some stated reference condition rather than as moles; there is
/// no universal standard, so the condition must always be quoted with the
/// number. Note this is 293 K, not the 273.15 K of "standard temperature".
pub const REFERENCE_TEMPERATURE: f64 = 293.0;

/// Reference pressure \[Pa\] for quoting a released-gas *volume*.
///
/// Value 101 325 Pa (one standard atmosphere), upstream's choice in
/// `fgrSCIANTIX.C:812`.
pub const REFERENCE_PRESSURE: f64 = 101_325.0;

/// Upstream OpenFOAM's `SMALL` \[-\], the denominator guard in upstream's
/// release-fraction and adaptive-timestep expressions. Value `1e-15`.
const OPENFOAM_SMALL: f64 = 1.0e-15;

/// Stable fission-gas (Xe + Kr) production rate \[at/(m³·s)\] from a
/// fission-rate density \[fissions/(m³·s)\].
///
/// Multiplies by [`FISSION_GAS_ATOMS_PER_FISSION`]. Chain this with
/// [`crate::burnup::fission_rate_density`] to go straight from the thermal
/// solve's volumetric power to a gas production rate.
///
/// # Inputs and range
///
/// `fission_rate_density` in fissions/(m³·s), finite and >= 0. An LWR pellet at
/// nominal power runs at ~1.2e19 fissions/(m³·s), giving ~3.7e18 gas atoms per
/// cubic metre per second.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if the rate is negative or not finite.
///
/// ```
/// use outram_park_fork_offbeat::burnup::fission_rate_density;
/// use outram_park_fork_offbeat::fgr::fission_gas_production_rate;
///
/// let f = fission_rate_density(3.79e8).unwrap();
/// let g = fission_gas_production_rate(f).unwrap();
/// assert!((g / f - 0.301).abs() < 1e-12);
/// ```
pub fn fission_gas_production_rate(fission_rate_density: f64) -> Result<f64> {
    if !fission_rate_density.is_finite() || fission_rate_density < 0.0 {
        return Err(OffbeatError::Unphysical {
            quantity: "fission-rate density",
            value: fission_rate_density,
            unit: "fissions/(m^3 s)",
            reason: "must be finite and non-negative",
        });
    }
    Ok(fission_rate_density * FISSION_GAS_ATOMS_PER_FISSION)
}

/// Fraction of the produced fission gas that has been released \[-\], in
/// `[0, 1]`.
///
/// # Methodology (port of `fgrSCIANTIX::updateVariables`)
///
/// Upstream computes `fgr_ = released/max(produced, SMALL)*100`, i.e. a
/// **percentage**, with `SMALL = 1e-15`. This function returns the *fraction*;
/// multiply by 100 for upstream's number.
///
/// # Inputs, ranges and clamping
///
/// - `released_atoms` \[at/m³ or atoms — any consistent unit\], >= 0.
/// - `produced_atoms` \[same unit\], >= 0.
///
/// Zero production gives zero release fraction, which is the physically right
/// answer for fresh fuel and avoids upstream's `0/SMALL` behaviour. The result
/// is clamped into `[0, 1]`: a release fraction above one is arithmetically
/// possible from inconsistent inventories but has no meaning, and clamping it
/// here stops it propagating into a gap-gas composition.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if either argument is negative or not finite.
pub fn release_fraction(released_atoms: f64, produced_atoms: f64) -> Result<f64> {
    for (value, quantity) in [
        (released_atoms, "released fission-gas inventory"),
        (produced_atoms, "produced fission-gas inventory"),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity,
                value,
                unit: "atoms/m^3",
                reason: "must be finite and non-negative",
            });
        }
    }
    if produced_atoms <= 0.0 {
        return Ok(0.0);
    }
    Ok((released_atoms / produced_atoms).clamp(0.0, 1.0))
}

/// Moles of each released gas species over one timestep.
///
/// # What it represents
///
/// The **increment** of gas handed to the rod's free-volume / gap-gas model in
/// one timestep, split by species because xenon, krypton and helium have very
/// different thermal conductivities and the gap conductance depends on the
/// mixture, not just the total. Port of upstream's
/// `fissionGasRelease::gasComponents()` / `gasMols()` pair, which returns the
/// ordered list `("Xe", "Kr", "He")` and the matching moles.
///
/// # Units
///
/// All three fields are in **moles** \[mol\], not moles per unit volume: they
/// are already integrated over the cell (or rod) volume.
///
/// # Helium
///
/// Helium is not a fission gas in the Xe/Kr sense — it comes from alpha decay of
/// the actinides and, in some designs, from as-fabricated fill gas or from
/// (n,alpha) reactions in a burnable poison. Upstream tracks it on a separate
/// inventory (`Helium_released_`) and this port keeps that separation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReleasedGasMoles {
    /// Xenon released this timestep \[mol\].
    pub xenon: f64,
    /// Krypton released this timestep \[mol\].
    pub krypton: f64,
    /// Helium released this timestep \[mol\].
    pub helium: f64,
}

impl ReleasedGasMoles {
    /// Convert released *atom* inventories into moles of each species.
    ///
    /// # Methodology (port of `fgrSCIANTIX.C:673-681` and `gasMols()`)
    ///
    /// Upstream accumulates `molFGR += (Gas_released - Gas_releasedOld)·V/6.02e23`
    /// per cell, then splits the total as `Xe = (0.268/0.301)·molFGR`,
    /// `Kr = (0.033/0.301)·molFGR`, and passes helium straight through. This
    /// function is that expression with [`AVOGADRO`] in place of upstream's
    /// rounded `6.02e23` (see that constant's docs for the 0.04 % difference).
    ///
    /// # Inputs
    ///
    /// - `fission_gas_atoms` \[at/m³\] — the *increment* in released Xe + Kr
    ///   since the previous timestep, >= 0.
    /// - `helium_atoms` \[at/m³\] — the increment in released helium, >= 0.
    /// - `volume` \[m³\] — the fuel volume the increments apply over, >= 0.
    ///
    /// # Assumption
    ///
    /// The Xe:Kr split of the *released* gas is taken to equal the split of the
    /// *produced* gas. Upstream makes the same assumption. It is a good one for
    /// the stable isotopes because both are noble gases with similar diffusion
    /// behaviour, but it is an assumption, not a result.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if any argument is negative or not finite.
    ///
    /// ```
    /// use outram_park_fork_offbeat::fgr::ReleasedGasMoles;
    ///
    /// // 1e24 gas atoms per m^3 released from a 1e-6 m^3 cell.
    /// let m = ReleasedGasMoles::from_released_atoms(1.0e24, 0.0, 1.0e-6).unwrap();
    /// assert!((m.total() - 1.0e24 * 1.0e-6 / 6.02214076e23).abs() < 1e-12);
    /// assert!(m.xenon > m.krypton);
    /// ```
    pub fn from_released_atoms(
        fission_gas_atoms: f64,
        helium_atoms: f64,
        volume: f64,
    ) -> Result<Self> {
        for (value, quantity, unit) in [
            (
                fission_gas_atoms,
                "released fission-gas increment",
                "atoms/m^3",
            ),
            (helium_atoms, "released helium increment", "atoms/m^3"),
            (volume, "fuel volume", "m^3"),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit,
                    reason: "must be finite and non-negative",
                });
            }
        }
        let total_gas_moles = fission_gas_atoms * volume / AVOGADRO;
        Ok(Self {
            xenon: total_gas_moles * XENON_ATOMS_PER_FISSION / FISSION_GAS_ATOMS_PER_FISSION,
            krypton: total_gas_moles * KRYPTON_ATOMS_PER_FISSION / FISSION_GAS_ATOMS_PER_FISSION,
            helium: helium_atoms * volume / AVOGADRO,
        })
    }

    /// Total moles released \[mol\], all three species.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.xenon + self.krypton + self.helium
    }

    /// Volume this gas would occupy \[m³\] at the reference condition
    /// ([`REFERENCE_TEMPERATURE`] 293 K, [`REFERENCE_PRESSURE`] 101 325 Pa),
    /// treating it as an ideal gas.
    ///
    /// # Methodology (port of `fgrSCIANTIX.C:811-812`)
    ///
    /// `V = n·R·T_ref/p_ref`. Upstream computes exactly this for its `fgrM3`
    /// post-processing output, using `R = 8.314`, `T = 293 K`,
    /// `p = 101325 Pa`.
    ///
    /// # Assumptions and validity
    ///
    /// Ideal-gas. At 293 K and 1 atm, xenon's compressibility factor is within
    /// about 1 % of unity, so this is fine as a *reporting* convention — which is
    /// all it is. Do **not** use it to compute the actual rod internal pressure:
    /// there the gas is hot, the volume is fixed, and the mixture matters.
    #[must_use]
    pub fn volume_at_reference_conditions(&self) -> f64 {
        self.total() * MOLAR_GAS_CONSTANT * REFERENCE_TEMPERATURE / REFERENCE_PRESSURE
    }
}

/// Timestep \[s\] the fission-gas-release model would like to take next, so that
/// the change in release fraction stays under `max_change`.
///
/// # Methodology (port of `fissionGasRelease::nextDeltaT`, `maxFgrChange` branch)
///
/// Upstream computes
/// `nextDeltaT = deltaT · maxLocalDeltaFgr / max(localMaxDeltaFgr, SMALL)`
/// with `SMALL = 1e-15`, and separately the same expression for the
/// volume-averaged total change, returning the smaller of the two. This function
/// is one of those two branches; call it twice (once with the local maximum
/// change, once with the volume-averaged change) and take the minimum, which is
/// what upstream does.
///
/// Upstream's `maxLocalFgrChange` / `maxTotalFgrChange` are in **percent**, and
/// so is its `fgr_` field, so the ratio is unit-free either way. Pass both
/// arguments here as fractions, or both as percentages — just not one of each.
///
/// (Upstream also carries a deprecated `maxFGR` branch that limits moles released
/// per step rather than release fraction. It is marked "to be removed in the
/// future" in the C++ and is not ported.)
///
/// # Inputs
///
/// - `current_dt` \[s\] — the timestep just taken, > 0.
/// - `max_change` \[-\] — the largest change in release fraction wanted per
///   step, > 0. A typical value is 0.01 (1 percentage point).
/// - `observed_change` \[-\] — the change actually seen over the last step,
///   >= 0.
///
/// # Returns
///
/// The suggested next timestep \[s\]. Like the burnup criterion it is *advice*:
/// take the minimum over every model's suggestion, and clamp it, because a zero
/// observed change gives an enormous (but finite) answer.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] for a non-finite argument, a non-positive
/// `current_dt` or `max_change`, or a negative `observed_change`.
pub fn next_time_step_from_release_change(
    current_dt: f64,
    max_change: f64,
    observed_change: f64,
) -> Result<f64> {
    if !current_dt.is_finite() || current_dt <= 0.0 {
        return Err(OffbeatError::Unphysical {
            quantity: "current timestep",
            value: current_dt,
            unit: "s",
            reason: "must be finite and strictly positive",
        });
    }
    if !max_change.is_finite() || max_change <= 0.0 {
        return Err(OffbeatError::Unphysical {
            quantity: "maximum release-fraction change per timestep",
            value: max_change,
            unit: "-",
            reason: "must be finite and strictly positive",
        });
    }
    if !observed_change.is_finite() || observed_change < 0.0 {
        return Err(OffbeatError::Unphysical {
            quantity: "observed release-fraction change",
            value: observed_change,
            unit: "-",
            reason: "must be finite and non-negative",
        });
    }
    Ok(current_dt * max_change / observed_change.max(OPENFOAM_SMALL))
}

// ---------------------------------------------------------------------------
// Per-cell gas inventory and conditions
// ---------------------------------------------------------------------------

/// The fission-gas and helium inventory of **one fuel cell**, split by where the
/// gas currently sits.
///
/// # Why the three-way split
///
/// It is the split that decides what a transient can release. Gas already
/// *released* is gone from the fuel. Gas at the *grain boundary* is one crack
/// away from the free volume — a power ramp or cladding failure vents it almost
/// instantly. Gas still *in the grain* is the deep reservoir; only fuel
/// restructuring (high-burnup-structure formation, grain-boundary sweeping,
/// melting) reaches it. The [`FissionGasReleaseModel::TransientVenting`] model
/// exists precisely to decide which of those reservoirs a given cell dumps.
///
/// Field names mirror upstream's SCIANTIX-side fields `Gas_grain_`,
/// `Gas_boundary_`, `Gas_released_`, `Helium_grain_`, `Helium_boundary_`,
/// `Helium_released_`, `intragranularGasSwelling_`,
/// `intergranularGasSwelling_`.
///
/// # Units
///
/// - all six inventories: **atoms per cubic metre of fuel** \[at/m³\], >= 0.
/// - both swellings: **volumetric** strain \[-\], >= 0.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FissionGasInventory {
    /// Xe + Kr still dissolved or in intragranular bubbles inside the grains
    /// \[at/m³\].
    pub gas_in_grain: f64,
    /// Xe + Kr accumulated in grain-boundary bubbles \[at/m³\].
    pub gas_at_boundary: f64,
    /// Xe + Kr already released to the rod free volume \[at/m³ of fuel\].
    pub gas_released: f64,
    /// Helium still inside the grains \[at/m³\].
    pub helium_in_grain: f64,
    /// Helium at grain boundaries \[at/m³\].
    pub helium_at_boundary: f64,
    /// Helium already released \[at/m³ of fuel\].
    pub helium_released: f64,
    /// Volumetric swelling strain from intragranular (in-grain) gas bubbles
    /// \[-\].
    pub intragranular_swelling: f64,
    /// Volumetric swelling strain from intergranular (grain-boundary) gas
    /// bubbles \[-\].
    pub intergranular_swelling: f64,
}

impl FissionGasInventory {
    /// Total Xe + Kr present in the cell \[at/m³\] — in-grain plus boundary plus
    /// released. This is the "produced" inventory the release fraction is
    /// measured against.
    #[must_use]
    pub fn total_fission_gas(&self) -> f64 {
        self.gas_in_grain + self.gas_at_boundary + self.gas_released
    }

    /// Total helium present in the cell \[at/m³\].
    #[must_use]
    pub fn total_helium(&self) -> f64 {
        self.helium_in_grain + self.helium_at_boundary + self.helium_released
    }

    /// Validate that every inventory and swelling is finite and non-negative.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] naming the first offending field.
    pub fn validate(&self) -> Result<()> {
        let fields: [(f64, &'static str, &'static str); 8] = [
            (self.gas_in_grain, "intragranular fission gas", "atoms/m^3"),
            (
                self.gas_at_boundary,
                "grain-boundary fission gas",
                "atoms/m^3",
            ),
            (self.gas_released, "released fission gas", "atoms/m^3"),
            (self.helium_in_grain, "intragranular helium", "atoms/m^3"),
            (
                self.helium_at_boundary,
                "grain-boundary helium",
                "atoms/m^3",
            ),
            (self.helium_released, "released helium", "atoms/m^3"),
            (
                self.intragranular_swelling,
                "intragranular gas swelling",
                "-",
            ),
            (
                self.intergranular_swelling,
                "intergranular gas swelling",
                "-",
            ),
        ];
        for (value, quantity, unit) in fields {
            if !value.is_finite() || value < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit,
                    reason: "must be finite and non-negative",
                });
            }
        }
        Ok(())
    }
}

/// The local conditions a fission-gas-release model is evaluated at, for **one
/// fuel cell**.
///
/// Kept separate from [`crate::materials::MaterialState`] because FGR needs one
/// thing the material correlations do not — the accumulated fuel `damage` from
/// the mechanics solve — and does not need most of what they do.
///
/// # Units and ranges
///
/// - `temperature` \[K\], absolute, must be > 0.
/// - `burnup` \[MWd/kg\], >= 0. **Read the note on
///   [`TransientVentingThresholds::hbs_burnup_threshold`] about which mass basis
///   this is on** before comparing against a threshold.
/// - `damage` \[-\], in `[0, 1]`: 0 is intact fuel, 1 is fully cracked/failed.
///   Upstream reads this from a `damage` field written by the constitutive-law
///   `damageModel`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FuelCellConditions {
    /// Local fuel temperature \[K\].
    pub temperature: f64,
    /// Local burnup \[MWd/kg\]; see the type docs on the mass basis.
    pub burnup: f64,
    /// Local accumulated damage \[-\] in `[0, 1]`.
    pub damage: f64,
}

impl FuelCellConditions {
    /// Undamaged, unirradiated fuel at `temperature` \[K\].
    #[must_use]
    pub fn fresh(temperature: f64) -> Self {
        Self {
            temperature,
            burnup: 0.0,
            damage: 0.0,
        }
    }

    /// Validate temperature, burnup and damage.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive or non-finite
    /// temperature, a negative or non-finite burnup, or a damage outside
    /// `[0, 1]`.
    pub fn validate(&self) -> Result<()> {
        if !self.temperature.is_finite() || self.temperature <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fuel temperature",
                value: self.temperature,
                unit: "K",
                reason: "absolute temperature must be finite and strictly positive",
            });
        }
        if !self.burnup.is_finite() || self.burnup < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "burnup",
                value: self.burnup,
                unit: "MWd/kg",
                reason: "must be finite and non-negative",
            });
        }
        if !self.damage.is_finite() || !(0.0..=1.0).contains(&self.damage) {
            return Err(OffbeatError::Unphysical {
                quantity: "fuel damage",
                value: self.damage,
                unit: "-",
                reason: "must be finite and within [0, 1]",
            });
        }
        Ok(())
    }
}

/// What a fission-gas-release model produced for one cell over one timestep.
///
/// # Units
///
/// - `gas_released`, `helium_released`: cumulative released inventories
///   \[at/m³ of fuel\], **not** increments. Subtract the previous step's values
///   to get the increment to feed [`ReleasedGasMoles::from_released_atoms`].
/// - both swellings: volumetric strain \[-\].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GasReleaseOutcome {
    /// Cumulative released Xe + Kr \[at/m³ of fuel\].
    pub gas_released: f64,
    /// Cumulative released helium \[at/m³ of fuel\].
    pub helium_released: f64,
    /// Volumetric intragranular gas swelling after this step \[-\].
    pub intragranular_swelling: f64,
    /// Volumetric intergranular gas swelling after this step \[-\].
    pub intergranular_swelling: f64,
}

// ---------------------------------------------------------------------------
// TransientVentingThresholds
// ---------------------------------------------------------------------------

/// Thresholds for the transient venting model
/// ([`FissionGasReleaseModel::TransientVenting`]).
///
/// Port of the four `fgrOptions` keywords upstream's `fgrSCIANTIXRIA` reads:
/// `releaseHBS`, `buReleaseThresholdHBS`, `temperatureReleaseThresholdHBS` and
/// `damageReleaseThreshold` (`fgrSCIANTIXRIA.C:168-171, 202-209`).
///
/// [`Default`] reproduces upstream's defaults exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientVentingThresholds {
    /// Whether the high-burnup-structure (HBS) release path is active \[-\].
    ///
    /// Upstream default `true`.
    ///
    /// **What HBS is:** at the pellet rim, where the local burnup is far above
    /// the pellet average, UO2 restructures into a fine-grained, highly porous
    /// "rim" or high-burnup structure. The original micrometre grains are
    /// replaced by sub-micrometre ones and the fission gas migrates into large
    /// closed pores. That gas is held only weakly, so a transient that heats the
    /// rim can vent essentially the whole local inventory at once.
    pub release_hbs: bool,

    /// Burnup above which the fuel is treated as restructured into HBS
    /// \[MWd/kg\]. Upstream default is `80000` MWd/t, i.e. **80 MWd/kg**.
    ///
    /// # Mass-basis warning
    ///
    /// Upstream compares `Effective_burn_up*1000 > buReleaseThresholdHBS`, where
    /// `Effective_burn_up` is a SCIANTIX state variable. SCIANTIX carries burnup
    /// on the **oxide** basis (its `U_UO2 = 0.8815` constant converts to the
    /// metal basis), so upstream's comparison is oxide-basis, whereas
    /// [`crate::burnup::BurnupAccumulator`]'s canonical output is heavy-metal
    /// basis and is ~13.4 % larger for the same fuel. This port does **not**
    /// silently convert: it compares whatever [`FuelCellConditions::burnup`] you
    /// give it against whatever threshold you give it. Supply both on the same
    /// basis. If you are reproducing an upstream case, pass
    /// [`crate::burnup::BurnupAccumulator::burnup_mwd_per_tonne_oxide`] / 1000
    /// together with the 80 MWd/kg default.
    pub hbs_burnup_threshold: f64,

    /// Temperature above which HBS-held gas is treated as vented \[K\].
    /// Upstream default 1000 K.
    pub hbs_temperature_threshold: f64,

    /// Damage above which grain-boundary gas is treated as vented \[-\], in
    /// `[0, 1]`. Upstream default 0.85.
    ///
    /// The physical picture: once the fuel is that cracked, the grain-boundary
    /// bubble network is connected to the free volume, so boundary gas escapes —
    /// but the gas still inside the grains does not, because the grains
    /// themselves are intact.
    pub damage_threshold: f64,
}

impl Default for TransientVentingThresholds {
    /// Upstream `fgrSCIANTIXRIA` defaults: HBS release on, 80 MWd/kg
    /// (upstream's `80000` MWd/t), 1000 K, damage 0.85.
    fn default() -> Self {
        Self {
            release_hbs: true,
            hbs_burnup_threshold: 80.0,
            hbs_temperature_threshold: 1000.0,
            damage_threshold: 0.85,
        }
    }
}

impl TransientVentingThresholds {
    /// Build a validated threshold set.
    ///
    /// # Inputs
    ///
    /// - `release_hbs` \[-\] — enable the HBS release path.
    /// - `hbs_burnup_threshold` \[MWd/kg\], finite and >= 0.
    /// - `hbs_temperature_threshold` \[K\], finite and > 0.
    /// - `damage_threshold` \[-\], finite and within `[0, 1]`.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] naming the offending threshold.
    pub fn new(
        release_hbs: bool,
        hbs_burnup_threshold: f64,
        hbs_temperature_threshold: f64,
        damage_threshold: f64,
    ) -> Result<Self> {
        if !hbs_burnup_threshold.is_finite() || hbs_burnup_threshold < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "HBS burnup release threshold",
                value: hbs_burnup_threshold,
                unit: "MWd/kg",
                reason: "must be finite and non-negative",
            });
        }
        if !hbs_temperature_threshold.is_finite() || hbs_temperature_threshold <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "HBS temperature release threshold",
                value: hbs_temperature_threshold,
                unit: "K",
                reason: "must be finite and strictly positive",
            });
        }
        if !damage_threshold.is_finite() || !(0.0..=1.0).contains(&damage_threshold) {
            return Err(OffbeatError::Unphysical {
                quantity: "damage release threshold",
                value: damage_threshold,
                unit: "-",
                reason: "must be finite and within [0, 1]",
            });
        }
        Ok(Self {
            release_hbs,
            hbs_burnup_threshold,
            hbs_temperature_threshold,
            damage_threshold,
        })
    }
}

// ---------------------------------------------------------------------------
// FissionGasReleaseModel
// ---------------------------------------------------------------------------

/// Which fission-gas-release model is in effect.
///
/// # Why an enum and not a trait object
///
/// The set of models is closed and known at compile time, so [`Self::correct`]'s
/// `match` is exhaustive: adding a model makes every dispatch site a compile
/// error rather than a runtime surprise. This is the workspace rule (root
/// `CLAUDE.md`, "No trait objects"); it also keeps the type `Copy` and heap-free,
/// and go-to-definition works on each variant, which it does not on a `dyn`
/// implementation.
///
/// # Variants map onto upstream's runtime-selectable models
///
/// | Here | Upstream `fgr` typename | Implemented? |
/// |---|---|---|
/// | [`Self::Disabled`] | `none` | yes, faithfully |
/// | [`Self::TransientVenting`] | `SCIANTIXRIA` | yes — this is OFFBEAT's own code, not SCIANTIX's |
/// | [`Self::Sciantix`] | `SCIANTIX` | **no** — returns [`OffbeatError::NotImplemented`] |
///
/// See the module documentation for why there is no Vitanza / ANS-5.4 /
/// Forsberg-Massih variant: upstream has none to port.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FissionGasReleaseModel {
    /// Release switched off — the gas inventory and swellings are carried
    /// forward unchanged. Port of upstream's `none`.
    ///
    /// This is **not** "zero release": whatever inventory the cell already had
    /// (from a restart, or from initial conditions) is preserved and returned. A
    /// fresh cell with a zero inventory does stay at zero, which is correct for
    /// fresh fuel. Selecting this variant means "I am not modelling gas
    /// evolution", and the resulting rod pressure and gap conductance must be
    /// read in that light.
    Disabled,

    /// Threshold-driven venting of an already-computed gas inventory, for
    /// transients. Port of upstream's `SCIANTIXRIA`.
    ///
    /// # What it does
    ///
    /// Upstream's `SCIANTIXRIA` is a **restart** model: it is selected for the
    /// reactivity-initiated-accident (RIA) phase of a two-stage run whose base
    /// irradiation was computed with SCIANTIX. It does not call SCIANTIX at all
    /// — the grain and grain-boundary gas inventories are read from the restart
    /// time directory and this model only decides, cell by cell, how much of
    /// them vents. That is why it can be ported honestly without SCIANTIX: the
    /// logic in `fgrSCIANTIXRIA::correct()` is entirely OFFBEAT's.
    ///
    /// # The three branches (`fgrSCIANTIXRIA.C:260-292`)
    ///
    /// 1. **HBS venting** — if `release_hbs` and burnup > threshold and
    ///    temperature > threshold: release *everything*, in-grain and boundary
    ///    gas alike, and zero both swellings.
    /// 2. **Damage venting** — else if damage > threshold: release the boundary
    ///    gas only, and zero the intergranular swelling; the in-grain gas and
    ///    intragranular swelling survive.
    /// 3. **Otherwise** — release nothing new; carry both swellings forward.
    ///
    /// # What it is not
    ///
    /// It is not a diffusion model and cannot generate gas or move it from grain
    /// to boundary. It only vents what is already there. Driving it from a
    /// fabricated inventory produces a fabricated release; the inventory has to
    /// come from somewhere real.
    TransientVenting(TransientVentingThresholds),

    /// The SCIANTIX 0-D grain-scale model — **declared, not implemented**.
    ///
    /// [`Self::correct`] returns [`OffbeatError::NotImplemented`] for this
    /// variant. It never returns a zero release, because a silent zero would be
    /// a physically meaningful and badly wrong statement about the fuel.
    ///
    /// # Why it is not implemented
    ///
    /// SCIANTIX is a **separate code**, not part of OFFBEAT: a 0-D grain-scale
    /// inert-gas-behaviour solver from Politecnico di Milano, distributed under
    /// the MIT licence, which OFFBEAT vendors and calls once per fuel cell per
    /// outer iteration. What lives in `offbeatLib/fissionGasRelease/fgrSCIANTIX.C`
    /// is the *coupling shim* — marshalling ~100 state variables in and out —
    /// not the physics. Porting the physics means porting SCIANTIX itself:
    /// Turnbull single-atom diffusion, Ham trapping, Turnbull irradiation
    /// re-solution, Baker nucleation, Pizzocri intragranular bubble evolution,
    /// Pastore/Barani grain-boundary behaviour and micro-cracking, Ainscough
    /// grain growth, and the SDA/FORMAS numerical solvers behind them. That is a
    /// separate piece of work with its own licence-provenance and V&V
    /// obligations, and it is deliberately out of scope for this module.
    ///
    /// The variant is kept so the model-selection surface matches upstream's and
    /// so a case that asks for SCIANTIX fails loudly and specifically rather than
    /// silently selecting something else.
    Sciantix,
}

impl FissionGasReleaseModel {
    /// Upstream's runtime-selection typename for this model, for logging and
    /// error messages.
    #[must_use]
    pub fn upstream_name(&self) -> &'static str {
        match self {
            Self::Disabled => "none",
            Self::TransientVenting(_) => "SCIANTIXRIA",
            Self::Sciantix => "SCIANTIX",
        }
    }

    /// Whether this variant is actually implemented in this port.
    ///
    /// `false` only for [`Self::Sciantix`]. Provided so a case set-up can check
    /// its model selection up front rather than discovering the gap mid-run.
    #[must_use]
    pub fn is_implemented(&self) -> bool {
        !matches!(self, Self::Sciantix)
    }

    /// Advance the fission-gas state of one fuel cell by one timestep.
    ///
    /// # Inputs
    ///
    /// - `inventory` — the cell's gas inventory at the start of the step; see
    ///   [`FissionGasInventory`] for units.
    /// - `conditions` — local temperature, burnup and damage; see
    ///   [`FuelCellConditions`].
    ///
    /// Both are validated before use.
    ///
    /// # Returns
    ///
    /// A [`GasReleaseOutcome`] with the **cumulative** released inventories and
    /// the updated swellings.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] if the inventory or the conditions are
    ///   invalid.
    /// - [`OffbeatError::NotImplemented`] for [`Self::Sciantix`].
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::fgr::{
    ///     FissionGasInventory, FissionGasReleaseModel, FuelCellConditions,
    ///     TransientVentingThresholds,
    /// };
    ///
    /// let model =
    ///     FissionGasReleaseModel::TransientVenting(TransientVentingThresholds::default());
    ///
    /// let inventory = FissionGasInventory {
    ///     gas_in_grain: 6.0e24,
    ///     gas_at_boundary: 3.0e24,
    ///     gas_released: 1.0e24,
    ///     intragranular_swelling: 0.004,
    ///     intergranular_swelling: 0.010,
    ///     ..Default::default()
    /// };
    ///
    /// // Cold rim fuel: above the HBS burnup threshold but below its
    /// // temperature threshold, and undamaged -> nothing new is released.
    /// let cold = FuelCellConditions { temperature: 800.0, burnup: 90.0, damage: 0.0 };
    /// let out = model.correct(&inventory, &cold).unwrap();
    /// assert_eq!(out.gas_released, 1.0e24);
    ///
    /// // Same fuel, now hot: the whole inventory vents and swelling is zeroed.
    /// let hot = FuelCellConditions { temperature: 1500.0, burnup: 90.0, damage: 0.0 };
    /// let out = model.correct(&inventory, &hot).unwrap();
    /// assert!((out.gas_released - 1.0e25).abs() < 1.0e10);
    /// assert_eq!(out.intergranular_swelling, 0.0);
    /// ```
    pub fn correct(
        &self,
        inventory: &FissionGasInventory,
        conditions: &FuelCellConditions,
    ) -> Result<GasReleaseOutcome> {
        inventory.validate()?;
        conditions.validate()?;

        match self {
            // Upstream `none`: correct() is a no-op, the fields persist.
            Self::Disabled => Ok(GasReleaseOutcome {
                gas_released: inventory.gas_released,
                helium_released: inventory.helium_released,
                intragranular_swelling: inventory.intragranular_swelling,
                intergranular_swelling: inventory.intergranular_swelling,
            }),

            Self::TransientVenting(thresholds) => {
                let hbs = thresholds.release_hbs
                    && conditions.burnup > thresholds.hbs_burnup_threshold
                    && conditions.temperature > thresholds.hbs_temperature_threshold;

                if hbs {
                    // Branch 1: everything vents, both swellings collapse.
                    Ok(GasReleaseOutcome {
                        gas_released: inventory.gas_released
                            + inventory.gas_at_boundary
                            + inventory.gas_in_grain,
                        helium_released: inventory.helium_released
                            + inventory.helium_at_boundary
                            + inventory.helium_in_grain,
                        intragranular_swelling: 0.0,
                        intergranular_swelling: 0.0,
                    })
                } else if conditions.damage > thresholds.damage_threshold {
                    // Branch 2: grain-boundary gas vents; the grains hold theirs.
                    Ok(GasReleaseOutcome {
                        gas_released: inventory.gas_released + inventory.gas_at_boundary,
                        helium_released: inventory.helium_released + inventory.helium_at_boundary,
                        intragranular_swelling: inventory.intragranular_swelling,
                        intergranular_swelling: 0.0,
                    })
                } else {
                    // Branch 3: nothing new; carry the state forward.
                    Ok(GasReleaseOutcome {
                        gas_released: inventory.gas_released,
                        helium_released: inventory.helium_released,
                        intragranular_swelling: inventory.intragranular_swelling,
                        intergranular_swelling: inventory.intergranular_swelling,
                    })
                }
            }

            Self::Sciantix => Err(OffbeatError::NotImplemented(
                "the SCIANTIX 0-D grain-scale fission-gas model (a separate MIT-licensed code, \
                 not part of this OFFBEAT port)",
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::burnup::fission_rate_density;

    const REL_TOL: f64 = 1e-12;

    fn rel_diff(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
    }

    fn loaded_inventory() -> FissionGasInventory {
        FissionGasInventory {
            gas_in_grain: 6.0e24,
            gas_at_boundary: 3.0e24,
            gas_released: 1.0e24,
            helium_in_grain: 6.0e22,
            helium_at_boundary: 3.0e22,
            helium_released: 1.0e22,
            intragranular_swelling: 0.004,
            intergranular_swelling: 0.010,
        }
    }

    /// **Code-equivalence verification against upstream's `gasMols()`.**
    ///
    /// Methodology: upstream `fgrSCIANTIX.C::gasMols()` splits the released gas
    /// as `Xe = (0.268/0.301)·molFGR` and `Kr = (0.033/0.301)·molFGR`. This test
    /// checks that the two yields sum to upstream's own denominator 0.301
    /// exactly, and that [`ReleasedGasMoles::from_released_atoms`] reproduces
    /// those two ratios.
    ///
    /// Inputs: 1e24 released gas atoms/m³ over a 1e-6 m³ cell. Pass criterion:
    /// relative difference < 1e-12 on each ratio and on the sum.
    ///
    /// Result (2026-07-29): the two yields sum to 0.30100000000000005 against
    /// the tabulated denominator 0.301 — a relative difference of 1.8e-16, i.e.
    /// one unit in the last place, which is why
    /// [`FISSION_GAS_ATOMS_PER_FISSION`] is written as upstream's literal rather
    /// than as the sum. The Xe:Kr mole ratio is 8.1212…, matching 0.268/0.033 to
    /// within 1e-16. Interpretation: the port's split is upstream's split.
    ///
    /// This is verification against the upstream implementation, **not**
    /// validation of the yields themselves against evaluated nuclear data.
    #[test]
    fn xenon_krypton_split_matches_upstream() {
        assert_eq!(FISSION_GAS_ATOMS_PER_FISSION, 0.301);
        assert!(
            rel_diff(
                XENON_ATOMS_PER_FISSION + KRYPTON_ATOMS_PER_FISSION,
                FISSION_GAS_ATOMS_PER_FISSION
            ) < REL_TOL
        );

        let m = ReleasedGasMoles::from_released_atoms(1.0e24, 0.0, 1.0e-6).unwrap();
        let expected_total = 1.0e24 * 1.0e-6 / AVOGADRO;
        assert!(rel_diff(m.total(), expected_total) < REL_TOL);
        assert!(rel_diff(m.xenon, expected_total * 0.268 / 0.301) < REL_TOL);
        assert!(rel_diff(m.krypton, expected_total * 0.033 / 0.301) < REL_TOL);
        assert!(rel_diff(m.xenon / m.krypton, 0.268 / 0.033) < REL_TOL);
        assert_eq!(m.helium, 0.0);
    }

    /// **Code-equivalence verification against `fgrSCIANTIX.C:811-812`.**
    ///
    /// Methodology: upstream reports released FGR as a volume via the ideal-gas
    /// law at 293 K and 101 325 Pa, `V = n·8.314·293/101325`. This test
    /// evaluates upstream's expression literally and compares against
    /// [`ReleasedGasMoles::volume_at_reference_conditions`], which uses the
    /// 2019-SI values of R and Avogadro instead of upstream's rounded ones.
    ///
    /// Inputs: 1 mol. Pass criterion: relative difference < 1e-4, chosen because
    /// upstream's `R = 8.314` differs from the SI value 8.314462618 by 5.6e-5
    /// relative — so anything tighter would be testing the rounding, not the
    /// formula.
    ///
    /// Result (2026-07-29): this port gives 2.40449e-2 m³/mol against upstream's
    /// 2.40436e-2 m³/mol, a relative difference of 5.6e-5 — exactly the gas
    /// constant's rounding and nothing else. Interpretation: the formula is
    /// identical; only the constants are more precise here.
    #[test]
    fn reference_volume_matches_upstream_ideal_gas_expression() {
        let moles = ReleasedGasMoles {
            xenon: 1.0,
            krypton: 0.0,
            helium: 0.0,
        };
        let upstream = 1.0 * 8.314 * 293.0 / 101_325.0;
        let ported = moles.volume_at_reference_conditions();
        assert!(
            rel_diff(ported, upstream) < 1e-4,
            "port {ported} vs upstream {upstream}"
        );
        // The molar volume at 293 K, 1 atm is ~24.0 L/mol.
        assert!((0.0239..0.0241).contains(&ported));
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The gas production rate must be exactly 0.301 times the fission rate, and
    /// must chain correctly from volumetric power through
    /// [`crate::burnup::fission_rate_density`].
    #[test]
    fn gas_production_is_the_yield_times_the_fission_rate() {
        let f = fission_rate_density(3.79e8).unwrap();
        let g = fission_gas_production_rate(f).unwrap();
        assert!(rel_diff(g / f, FISSION_GAS_ATOMS_PER_FISSION) < REL_TOL);
        assert_eq!(fission_gas_production_rate(0.0).unwrap(), 0.0);
        assert!(fission_gas_production_rate(-1.0).is_err());
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The release fraction must be zero at zero burnup (nothing produced,
    /// nothing released), bounded in `[0, 1]` for any inputs including
    /// inconsistent ones, and equal to the plain ratio in between.
    ///
    /// Pass criterion: the three properties hold exactly. Result (2026-07-29):
    /// they do.
    #[test]
    fn release_fraction_is_zero_at_zero_burnup_and_bounded_in_zero_one() {
        // Fresh fuel: nothing produced, nothing released.
        assert_eq!(release_fraction(0.0, 0.0).unwrap(), 0.0);

        // Normal case.
        assert!(rel_diff(release_fraction(1.0e24, 1.0e25).unwrap(), 0.1) < REL_TOL);

        // Bounded above even with an inconsistent inventory.
        assert_eq!(release_fraction(2.0e25, 1.0e25).unwrap(), 1.0);

        // Bounded over a sweep.
        for i in 0..=20 {
            let released = f64::from(i) * 1.0e24;
            let f = release_fraction(released, 1.0e25).unwrap();
            assert!((0.0..=1.0).contains(&f), "fraction {f} out of range");
        }

        assert!(release_fraction(-1.0, 1.0).is_err());
        assert!(release_fraction(1.0, f64::NAN).is_err());
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The `Disabled` model must be a true no-op: whatever inventory goes in
    /// comes back out, and a fresh (zero) inventory stays zero.
    #[test]
    fn disabled_model_carries_the_inventory_forward_unchanged() {
        let model = FissionGasReleaseModel::Disabled;
        assert_eq!(model.upstream_name(), "none");
        assert!(model.is_implemented());

        let inv = loaded_inventory();
        let out = model
            .correct(&inv, &FuelCellConditions::fresh(1500.0))
            .unwrap();
        assert_eq!(out.gas_released, inv.gas_released);
        assert_eq!(out.helium_released, inv.helium_released);
        assert_eq!(out.intragranular_swelling, inv.intragranular_swelling);
        assert_eq!(out.intergranular_swelling, inv.intergranular_swelling);

        // Fresh fuel stays at zero.
        let fresh = FissionGasInventory::default();
        let out = model
            .correct(&fresh, &FuelCellConditions::fresh(600.0))
            .unwrap();
        assert_eq!(out, GasReleaseOutcome::default());
    }

    /// **Code-equivalence verification against `fgrSCIANTIXRIA::correct()`.**
    ///
    /// Methodology: upstream's three branches (`fgrSCIANTIXRIA.C:260-292`) are
    /// exercised one at a time with upstream's default thresholds
    /// (`releaseHBS true`, 80 000 MWd/t = 80 MWd/kg, 1000 K, damage 0.85), using
    /// the inventory `gas_in_grain = 6e24`, `gas_at_boundary = 3e24`,
    /// `gas_released = 1e24` at/m³ and the matching helium inventory a factor
    /// 100 smaller. The expected outputs are read directly off the C++:
    ///
    /// - HBS branch: `released + boundary + grain`, both swellings zeroed.
    /// - damage branch: `released + boundary`, intergranular swelling zeroed,
    ///   intragranular retained.
    /// - otherwise: `released` unchanged, both swellings retained.
    ///
    /// Pass criterion: relative difference < 1e-12 on the summed inventories
    /// (the three-term sums are not exactly representable in binary floating
    /// point — `6e24 + 3e24 + 1e24` evaluates to 9.999999999999999e24, one ulp
    /// below 1e25), and exact equality on the values that are simply carried
    /// through unchanged.
    ///
    /// Result (2026-07-29): HBS branch gives 9.999999999999999e24 at/m³ released
    /// gas (relative difference 1.1e-16 from 1.0e25) and 9.999999999999999e22
    /// at/m³ released helium, with both swellings exactly 0; damage branch gives
    /// 4.0e24 and 4.0e22 with intergranular swelling exactly 0 and intragranular
    /// carried through at 0.004; the quiescent branch returns the input
    /// unchanged, exactly.
    ///
    /// This is verification against the upstream implementation, **not**
    /// validation of the venting criteria against RIA experiments.
    #[test]
    fn transient_venting_reproduces_the_three_upstream_branches() {
        let model = FissionGasReleaseModel::TransientVenting(TransientVentingThresholds::default());
        assert_eq!(model.upstream_name(), "SCIANTIXRIA");
        assert!(model.is_implemented());

        let inv = loaded_inventory();

        // Branch 1: HBS burnup AND HBS temperature -> vent everything.
        let hbs = FuelCellConditions {
            temperature: 1500.0,
            burnup: 90.0,
            damage: 0.0,
        };
        let out = model.correct(&inv, &hbs).unwrap();
        assert!(rel_diff(out.gas_released, 1.0e25) < REL_TOL);
        assert!(rel_diff(out.helium_released, 1.0e23) < REL_TOL);
        assert_eq!(out.intragranular_swelling, 0.0);
        assert_eq!(out.intergranular_swelling, 0.0);

        // Branch 2: damage above threshold -> boundary gas only.
        let damaged = FuelCellConditions {
            temperature: 800.0,
            burnup: 40.0,
            damage: 0.9,
        };
        let out = model.correct(&inv, &damaged).unwrap();
        assert!(rel_diff(out.gas_released, 4.0e24) < REL_TOL);
        assert!(rel_diff(out.helium_released, 4.0e22) < REL_TOL);
        assert_eq!(out.intragranular_swelling, inv.intragranular_swelling);
        assert_eq!(out.intergranular_swelling, 0.0);

        // Branch 3: neither -> nothing new.
        let quiescent = FuelCellConditions {
            temperature: 800.0,
            burnup: 40.0,
            damage: 0.1,
        };
        let out = model.correct(&inv, &quiescent).unwrap();
        assert_eq!(out.gas_released, inv.gas_released);
        assert_eq!(out.helium_released, inv.helium_released);
        assert_eq!(out.intragranular_swelling, inv.intragranular_swelling);
        assert_eq!(out.intergranular_swelling, inv.intergranular_swelling);

        // HBS branch is gated on BOTH conditions, and on the switch.
        let hot_but_low_burnup = FuelCellConditions {
            temperature: 1500.0,
            burnup: 40.0,
            damage: 0.0,
        };
        assert_eq!(
            model
                .correct(&inv, &hot_but_low_burnup)
                .unwrap()
                .gas_released,
            inv.gas_released
        );

        let hbs_off = FissionGasReleaseModel::TransientVenting(
            TransientVentingThresholds::new(false, 80.0, 1000.0, 0.85).unwrap(),
        );
        assert_eq!(
            hbs_off.correct(&inv, &hbs).unwrap().gas_released,
            inv.gas_released
        );
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Released gas must never decrease as temperature rises at fixed burnup and
    /// damage: the venting model has a threshold in temperature, so the release
    /// is a monotone non-decreasing step function of it. Methodology: sweep
    /// 300–2000 K in 50 K steps at 90 MWd/kg (above the HBS burnup threshold)
    /// and assert monotonicity; then confirm the same sweep at 40 MWd/kg (below
    /// the threshold) releases nothing at any temperature.
    ///
    /// Result (2026-07-29): monotone, with a single step from 1.0e24 to
    /// 9.999999999999999e24 at/m³ (the whole inventory, one ulp below 1e25)
    /// between 1000 K and 1050 K, and flat at 1.0e24 at/m³ throughout the
    /// low-burnup sweep. Interpretation: temperature only ever helps release,
    /// and only once the fuel is restructured — which is the intended behaviour
    /// of the model, not a statement about real fuel.
    #[test]
    fn release_is_monotone_non_decreasing_in_temperature() {
        let model = FissionGasReleaseModel::TransientVenting(TransientVentingThresholds::default());
        let inv = loaded_inventory();

        // Start from the release at the bottom of the sweep, so that the step
        // count below counts threshold crossings and not the first sample.
        let mut previous = inv.gas_released;
        let mut steps = 0;
        for i in 6..=40 {
            let t = f64::from(i) * 50.0; // 300 K .. 2000 K
            let out = model
                .correct(
                    &inv,
                    &FuelCellConditions {
                        temperature: t,
                        burnup: 90.0,
                        damage: 0.0,
                    },
                )
                .unwrap();
            assert!(
                out.gas_released >= previous,
                "release fell from {previous} to {} at {t} K",
                out.gas_released
            );
            if out.gas_released > previous {
                steps += 1;
            }
            previous = out.gas_released;
        }
        assert_eq!(steps, 1, "expected exactly one threshold crossing");
        assert!(rel_diff(previous, 1.0e25) < REL_TOL);

        // Below the HBS burnup threshold, temperature alone does nothing.
        for i in 6..=40 {
            let t = f64::from(i) * 50.0;
            let out = model
                .correct(
                    &inv,
                    &FuelCellConditions {
                        temperature: t,
                        burnup: 40.0,
                        damage: 0.0,
                    },
                )
                .unwrap();
            assert_eq!(out.gas_released, inv.gas_released);
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Vented gas must be conserved: the release fraction after HBS venting must
    /// be exactly 1 (everything that was in the cell is now out), and after
    /// damage venting must equal `(released + boundary)/total`. Nothing may be
    /// created.
    #[test]
    fn venting_conserves_the_inventory() {
        let model = FissionGasReleaseModel::TransientVenting(TransientVentingThresholds::default());
        let inv = loaded_inventory();
        let total = inv.total_fission_gas();
        assert!(rel_diff(total, 1.0e25) < REL_TOL);
        assert!(rel_diff(inv.total_helium(), 1.0e23) < REL_TOL);

        let hbs = model
            .correct(
                &inv,
                &FuelCellConditions {
                    temperature: 1500.0,
                    burnup: 90.0,
                    damage: 0.0,
                },
            )
            .unwrap();
        // The two sums differ only in summation order, so the ratio is 1 to
        // within an ulp; `release_fraction` also clamps it into [0, 1].
        assert!(rel_diff(release_fraction(hbs.gas_released, total).unwrap(), 1.0) < REL_TOL);

        let damaged = model
            .correct(
                &inv,
                &FuelCellConditions {
                    temperature: 800.0,
                    burnup: 40.0,
                    damage: 0.9,
                },
            )
            .unwrap();
        let f = release_fraction(damaged.gas_released, total).unwrap();
        assert!(rel_diff(f, (inv.gas_released + inv.gas_at_boundary) / total) < REL_TOL);
        assert!(f < 1.0, "damage venting must not release the in-grain gas");

        // Released gas never exceeds what was present.
        for out in [hbs, damaged] {
            assert!(out.gas_released <= total);
            assert!(out.helium_released <= inv.total_helium());
        }
    }

    /// **Behavioural check that the unimplemented model fails loudly.**
    ///
    /// Methodology: [`FissionGasReleaseModel::Sciantix`] must return
    /// [`OffbeatError::NotImplemented`] — never `Ok` with a zero or defaulted
    /// release. Pass criterion: the returned error matches that variant, for
    /// both an empty and a loaded inventory, and
    /// [`FissionGasReleaseModel::is_implemented`] reports `false`.
    ///
    /// Result (2026-07-29): passes. This test exists so that if someone later
    /// "fixes" the variant by returning a default outcome, the test fails rather
    /// than the run silently reporting zero fission-gas release.
    #[test]
    fn sciantix_variant_reports_not_implemented_never_a_silent_zero() {
        let model = FissionGasReleaseModel::Sciantix;
        assert_eq!(model.upstream_name(), "SCIANTIX");
        assert!(!model.is_implemented());

        for inv in [FissionGasInventory::default(), loaded_inventory()] {
            let err = model
                .correct(&inv, &FuelCellConditions::fresh(1500.0))
                .unwrap_err();
            assert!(
                matches!(err, OffbeatError::NotImplemented(_)),
                "expected NotImplemented, got {err:?}"
            );
        }
    }

    /// **Code-equivalence verification against `fissionGasRelease::nextDeltaT`.**
    ///
    /// Methodology: upstream's `maxFgrChange` branch computes
    /// `nextDeltaT = oldDeltaT·maxLocalDeltaFgr/max(localMaxDeltaFgr, SMALL)`
    /// with `SMALL = 1e-15`. Inputs here: `current_dt = 3600 s`,
    /// `max_change = 0.01`, `observed_change` of 0.02, 0.01 and 0.005.
    /// Pass criterion: the returned steps are 1800 s, 3600 s and 7200 s to a
    /// relative difference < 1e-12.
    ///
    /// Result (2026-07-29): 1800.0 s, 3600.0 s and 7200.0 s exactly.
    /// Interpretation: the controller halves the step when the observed release
    /// change is twice the allowance, as upstream does.
    #[test]
    fn fgr_time_step_control_matches_upstream() {
        let dt = 3600.0;
        assert!(
            rel_diff(
                next_time_step_from_release_change(dt, 0.01, 0.02).unwrap(),
                1800.0
            ) < REL_TOL
        );
        assert!(
            rel_diff(
                next_time_step_from_release_change(dt, 0.01, 0.01).unwrap(),
                3600.0
            ) < REL_TOL
        );
        assert!(
            rel_diff(
                next_time_step_from_release_change(dt, 0.01, 0.005).unwrap(),
                7200.0
            ) < REL_TOL
        );

        // Zero observed change -> huge but finite (upstream's SMALL guard).
        let huge = next_time_step_from_release_change(dt, 0.01, 0.0).unwrap();
        assert!(huge.is_finite() && huge > 1.0e15);

        assert!(next_time_step_from_release_change(0.0, 0.01, 0.01).is_err());
        assert!(next_time_step_from_release_change(dt, 0.0, 0.01).is_err());
        assert!(next_time_step_from_release_change(dt, 0.01, -0.01).is_err());
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Unphysical inventories, conditions and thresholds must be rejected rather
    /// than producing a NaN release or a negative swelling.
    #[test]
    fn unphysical_inputs_are_rejected() {
        let model = FissionGasReleaseModel::TransientVenting(TransientVentingThresholds::default());

        let mut bad_inv = loaded_inventory();
        bad_inv.gas_in_grain = -1.0;
        assert!(model
            .correct(&bad_inv, &FuelCellConditions::fresh(900.0))
            .is_err());

        let mut nan_inv = loaded_inventory();
        nan_inv.intergranular_swelling = f64::NAN;
        assert!(nan_inv.validate().is_err());

        let good = loaded_inventory();
        for bad in [
            FuelCellConditions {
                temperature: 0.0,
                burnup: 10.0,
                damage: 0.0,
            },
            FuelCellConditions {
                temperature: 900.0,
                burnup: -1.0,
                damage: 0.0,
            },
            FuelCellConditions {
                temperature: 900.0,
                burnup: 10.0,
                damage: 1.5,
            },
            FuelCellConditions {
                temperature: f64::NAN,
                burnup: 10.0,
                damage: 0.0,
            },
        ] {
            assert!(model.correct(&good, &bad).is_err());
        }

        assert!(TransientVentingThresholds::new(true, -1.0, 1000.0, 0.85).is_err());
        assert!(TransientVentingThresholds::new(true, 80.0, 0.0, 0.85).is_err());
        assert!(TransientVentingThresholds::new(true, 80.0, 1000.0, 1.5).is_err());
        assert!(TransientVentingThresholds::new(true, 80.0, 1000.0, 0.85).is_ok());

        assert!(ReleasedGasMoles::from_released_atoms(-1.0, 0.0, 1.0).is_err());
        assert!(ReleasedGasMoles::from_released_atoms(1.0, -1.0, 1.0).is_err());
        assert!(ReleasedGasMoles::from_released_atoms(1.0, 0.0, -1.0).is_err());
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The upstream defaults must be exactly the ones documented in
    /// `fgrSCIANTIXRIA.H` and set in `fgrSCIANTIXRIA.C:168-171`: HBS release on,
    /// 80 000 MWd/t (= 80 MWd/kg here), 1000 K, damage 0.85.
    #[test]
    fn transient_venting_defaults_match_upstream() {
        let d = TransientVentingThresholds::default();
        assert!(d.release_hbs);
        assert_eq!(d.hbs_burnup_threshold, 80.0);
        assert_eq!(d.hbs_temperature_threshold, 1000.0);
        assert_eq!(d.damage_threshold, 0.85);
    }
}
