// SPDX-License-Identifier: GPL-3.0

//! Dry deposition of airborne activity onto the ground.
//!
//! The model is the standard one-line dry-deposition relation: the activity
//! deposited per unit area is the deposition velocity times the
//! time-integrated air concentration at ground level,
//!
//! ```text
//! D [Bq/m^2] = v_d [m/s] * TIC(z = 0) [Bq.s/m^3]
//! ```
//!
//! # Four limits, none of them buried
//!
//! - **Dry only.** Wet scavenging is not ported, so the answer is **not an
//!   upper bound** — rain would raise it. That is the opposite of the usual
//!   conservative framing and is the one most likely to be misread.
//! - **Diagnostic, not depleting.** Deposited activity is *not* removed from
//!   the plume, so the airborne concentration downwind of a deposition
//!   receptor is unchanged by it. At long range this over-predicts both the
//!   airborne concentration and the cumulative deposition.
//! - **Evaluated at `z = 0`, not breathing height.** `TIC` falls off with
//!   height, so feeding a 1.5 m receptor's `TIC` into this relation
//!   under-predicts deposition. Carry two receptor sets and pair them by
//!   index — the day-3 example does exactly that.
//! - **One velocity per group, no speciation.** A single elemental-iodine
//!   velocity over-predicts iodine deposition and under-predicts airborne
//!   iodine downwind, because organic iodides deposit far more slowly.
//!
//! # Deposition velocities are NOT supplied as cited data
//!
//! [`DryDepositionVelocity`] has **no default and no cited table**, and that is
//! deliberate. The values in general circulation for these three groups are
//! order-of-magnitude figures that vary with surface roughness, wind speed,
//! stability and particle size by more than an order of magnitude each; quoting
//! one from memory and attaching a reference to it would be exactly the
//! "tuned parameter wearing a citation" the workspace rules forbid.
//!
//! So: **the caller supplies the velocity.** [`DryDepositionVelocity::new`]
//! takes a `Velocity` and says nothing about where it came from;
//! [`DryDepositionVelocity::order_of_magnitude_placeholder`] exists so the
//! end-to-end example can run, and is named so that no reader can mistake its
//! output for a sourced number.
//!
//! Candidate sources to read before this module reports anything trustworthy —
//! **none of them has been consulted, none is in this workspace's literature
//! archive, and the placeholder values below were not taken from any of them**:
//! NUREG/CR-4691 (the MACCS2 model description), IAEA Safety Reports Series
//! No. 19, and Sehmel's 1980 deposition review. Tracked as a GitHub issue.

use uom::si::f64::Velocity;
use uom::si::velocity::meter_per_second;

use super::units::{GroundDeposition, TimeIntegratedAirConcentration};

/// How a nuclide behaves when it meets the ground.
///
/// Three groups, because dry deposition velocity spans roughly three orders of
/// magnitude across them and finer resolution is not supported by anything this
/// module knows.
///
/// **This is a DEPOSITION grouping and deliberately differs from `boon-lay`'s
/// transport `ElementGroup`.** Selenium (Z 34) and tellurium (Z 52) travel
/// through TRISO layers like halogens and are grouped with them for *release*;
/// once airborne they behave as condensed aerosols and are grouped here with
/// aerosols instead. Mapping one grouping onto the other would be wrong in one
/// of the two places, so they are kept separate and the divergence is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepositionGroup {
    /// Chemically inert; does not deposit. Deposition velocity is exactly zero,
    /// not merely small — a noble gas passes over the ground and leaves nothing.
    NobleGas,
    /// Reactive halogens — F, Cl, Br, I, At. The fastest-depositing group, and
    /// the one where the lack of speciation hurts most: this treats all of it as
    /// the elemental form, which deposits far faster than organic iodides.
    Halogen,
    /// Everything else, treated as a condensed particulate: caesium, strontium,
    /// barium, silver, the lanthanides, and — see the note on this enum —
    /// selenium and tellurium.
    Aerosol,
}

impl DepositionGroup {
    /// Classify by atomic number.
    ///
    /// Noble gases are Z in {2, 10, 18, 36, 54, 86}; halogens are Z in
    /// {9, 17, 35, 53, 85}; everything else is [`Self::Aerosol`], which is what
    /// puts Se (34) and Te (52) in the aerosol group — see the enum's note.
    #[must_use]
    pub const fn from_atomic_number(z: u32) -> Self {
        match z {
            2 | 10 | 18 | 36 | 54 | 86 => Self::NobleGas,
            9 | 17 | 35 | 53 | 85 => Self::Halogen,
            _ => Self::Aerosol,
        }
    }

    /// A short label for printing.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NobleGas => "noble gas",
            Self::Halogen => "halogen",
            Self::Aerosol => "aerosol",
        }
    }
}

/// A dry deposition velocity, in m/s.
///
/// Constructed from a value the **caller** justifies. See the module docs for
/// why this module ships no cited table.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DryDepositionVelocity(Velocity);

impl DryDepositionVelocity {
    /// From a velocity the caller has a source for.
    ///
    /// # Panics
    /// Panics if negative — a negative deposition velocity would resuspend
    /// activity from the ground, which this model does not represent.
    #[must_use]
    pub fn new(velocity: Velocity) -> Self {
        let v = velocity.get::<meter_per_second>();
        assert!(
            v >= 0.0,
            "deposition velocity must be non-negative; got {v} m/s"
        );
        Self(velocity)
    }

    /// **NOT A CITED VALUE.** An order-of-magnitude placeholder so an
    /// end-to-end run is possible before a source has been read.
    ///
    /// | group | value | status |
    /// |---|---|---|
    /// | [`DepositionGroup::NobleGas`] | `0` m/s | **physics, not a placeholder** — inert gases do not dry-deposit, and zero is exact |
    /// | [`DepositionGroup::Halogen`] | `1e-2` m/s | placeholder |
    /// | [`DepositionGroup::Aerosol`] | `1e-3` m/s | placeholder |
    ///
    /// The two non-zero rows are round numbers chosen to be the right order of
    /// magnitude and nothing more. **They were not read from any source**, they
    /// carry no uncertainty, and the real values vary by more than an order of
    /// magnitude with surface roughness, wind speed, stability and particle
    /// size. Any deposition figure computed from them inherits all of that and
    /// must be reported as illustrative.
    ///
    /// The method is named the way it is so that a reader meeting it at a call
    /// site cannot mistake it for a reference table. Replace it with
    /// [`Self::new`] and a sourced value before quoting a number.
    #[must_use]
    pub fn order_of_magnitude_placeholder(group: DepositionGroup) -> Self {
        let v = match group {
            DepositionGroup::NobleGas => 0.0,
            DepositionGroup::Halogen => 1.0e-2,
            DepositionGroup::Aerosol => 1.0e-3,
        };
        Self(Velocity::new::<meter_per_second>(v))
    }

    /// The value in metres per second.
    #[must_use]
    pub fn meters_per_second(self) -> f64 {
        self.0.get::<meter_per_second>()
    }

    /// The underlying dimensioned velocity.
    #[must_use]
    pub const fn velocity(self) -> Velocity {
        self.0
    }
}

/// Dry deposition from a ground-level time-integrated air concentration.
///
/// `D = v_d * TIC`, giving Bq/m^2 from (m/s) times (Bq.s/m^3).
///
/// `air` must be the `TIC` evaluated at **`z = 0`**, not at breathing height —
/// see the module docs. Nothing here can check that, because a
/// [`TimeIntegratedAirConcentration`] does not carry the height it was
/// evaluated at.
#[must_use]
pub fn dry_deposition(
    air: TimeIntegratedAirConcentration,
    velocity: DryDepositionVelocity,
) -> GroundDeposition {
    GroundDeposition::new(air.becquerel_seconds_per_cubic_meter() * velocity.meters_per_second())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noble_gases_deposit_exactly_nothing() {
        let v = DryDepositionVelocity::order_of_magnitude_placeholder(DepositionGroup::NobleGas);
        assert_eq!(v.meters_per_second(), 0.0);
        let d = dry_deposition(TimeIntegratedAirConcentration::new(1.0e12), v);
        // Exactly zero, not merely small.
        assert_eq!(d.becquerel_per_square_meter(), 0.0);
    }

    #[test]
    fn deposition_is_velocity_times_air_concentration() {
        let v = DryDepositionVelocity::new(Velocity::new::<meter_per_second>(1.0e-2));
        let d = dry_deposition(TimeIntegratedAirConcentration::new(2.0e8), v);
        assert!((d.becquerel_per_square_meter() - 2.0e6).abs() < 1e-6);
    }

    /// Trap 7: the deposition grouping is NOT `boon-lay`'s transport grouping.
    #[test]
    fn selenium_and_tellurium_deposit_as_aerosols_not_halogens() {
        assert_eq!(
            DepositionGroup::from_atomic_number(34),
            DepositionGroup::Aerosol,
            "Se travels like a halogen but deposits like an aerosol"
        );
        assert_eq!(
            DepositionGroup::from_atomic_number(52),
            DepositionGroup::Aerosol,
            "Te travels like a halogen but deposits like an aerosol"
        );
        assert_eq!(
            DepositionGroup::from_atomic_number(53),
            DepositionGroup::Halogen
        );
        assert_eq!(
            DepositionGroup::from_atomic_number(35),
            DepositionGroup::Halogen
        );
    }

    #[test]
    fn the_noble_gases_are_all_classified_as_such() {
        for z in [2, 10, 18, 36, 54, 86] {
            assert_eq!(
                DepositionGroup::from_atomic_number(z),
                DepositionGroup::NobleGas,
                "Z = {z}"
            );
        }
        // Kr and Xe are the two that matter for a reactor source term.
        assert_eq!(
            DepositionGroup::from_atomic_number(36),
            DepositionGroup::NobleGas
        );
        assert_eq!(
            DepositionGroup::from_atomic_number(54),
            DepositionGroup::NobleGas
        );
    }

    #[test]
    fn the_consequence_dominant_aerosols_are_classified_as_aerosols() {
        // Cs 55, Sr 38, Ba 56, Ag 47.
        for z in [55, 38, 56, 47] {
            assert_eq!(
                DepositionGroup::from_atomic_number(z),
                DepositionGroup::Aerosol,
                "Z = {z}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "non-negative")]
    fn a_negative_deposition_velocity_is_rejected() {
        let _ = DryDepositionVelocity::new(Velocity::new::<meter_per_second>(-1.0e-3));
    }
}
