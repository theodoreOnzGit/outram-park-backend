//! Decay modes and neutron-induced reaction channels, with daughter lookup.
//!
//! Each variant carries the `(dZ, dA, dm)` operation ONIX applies to a parent
//! nuclide's packed id to obtain the daughter. Enum dispatch is used throughout
//! (workspace rule: no trait objects) — the set of channels is closed and known
//! at compile time, so adding a variant forces every `match` to handle it.
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! Ported from the ONIX depletion code (open-source, MIT licensed):
//!   * upstream project: ONIX — <https://github.com/jlanversin/ONIX>
//!   * upstream commit:  `7328dc6`
//!   * source file/lines: `onix/data/list_and_dict.py:244` (`xs_prod_fromS_toS`
//!     — the `(n,gamma)/(n,2n)/(n,3n)/(n,p)/(n,a)/(n,t)` delta triples) and
//!     `onix/data/list_and_dict.py:250` (`decay_prod_fromS_toS` — the
//!     `betaneg/betapos/alpha/neutron/proton` delta triples).
//!
//! Independent Rust re-implementation; the OUTRAM PARK fork relicenses under
//! **GPL-3.0-only** (MIT is GPL-3.0-compatible; upstream MIT notice preserved).

use crate::nuclide::Nuclide;

/// A radioactive decay mode.
///
/// Each mode maps a parent nuclide to a daughter via a fixed `(dZ, dA, dm)`
/// transformation. The associated *rate* of a mode is the partial decay
/// constant (units `1/s`, see [`crate::chain::DecayData`]); this enum only
/// encodes the identity transformation, not the rate.
///
/// Delta triples reproduce ONIX `decay_prod_fromS_toS`
/// (`onix/data/list_and_dict.py:250`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecayMode {
    /// β⁻ decay: a neutron → proton, so `Z+1`, `A` unchanged. `[+1, 0, 0]`.
    BetaMinus,
    /// β⁺ decay / electron capture: `Z-1`, `A` unchanged. `[-1, 0, 0]`.
    BetaPlus,
    /// α decay: emit a ⁴He nucleus, so `Z-2`, `A-4`. `[-2, -4, 0]`.
    Alpha,
    /// Neutron emission: `A-1`, `Z` unchanged. `[0, -1, 0]`.
    NeutronEmission,
    /// Proton emission: `Z-1`, `A-1`. `[-1, -1, 0]`.
    ProtonEmission,
    /// Isomeric transition: same `Z`, same `A`, de-excite to ground (`m -> 0`).
    ///
    /// ONIX handles metastable de-excitation through its state bookkeeping; here
    /// we model the common ground-state landing (`dm` set so the daughter is
    /// `m = 0`) via [`DecayMode::daughter`]'s special case.
    IsomericTransition,
}

impl DecayMode {
    /// The `(dZ, dA, dm)` delta this mode applies to a parent.
    ///
    /// Dimensionless integer deltas. For [`DecayMode::IsomericTransition`] the
    /// `dm` is not fixed (it depends on the parent's `m`); see
    /// [`DecayMode::daughter`], which returns the ground-state isomer directly.
    pub const fn delta(&self) -> (i32, i32, i32) {
        match self {
            DecayMode::BetaMinus => (1, 0, 0),
            DecayMode::BetaPlus => (-1, 0, 0),
            DecayMode::Alpha => (-2, -4, 0),
            DecayMode::NeutronEmission => (0, -1, 0),
            DecayMode::ProtonEmission => (-1, -1, 0),
            DecayMode::IsomericTransition => (0, 0, 0),
        }
    }

    /// The daughter nuclide produced by this decay mode, or `None` if the
    /// transformation would be unphysical.
    ///
    /// Isomeric transition lands the parent in its own ground state (`m = 0`).
    /// All other modes apply the fixed delta from [`DecayMode::delta`].
    pub fn daughter(&self, parent: Nuclide) -> Option<Nuclide> {
        match self {
            DecayMode::IsomericTransition => {
                let d = Nuclide::new(parent.z, parent.a, 0);
                if d == parent {
                    None // a ground-state nuclide has no IT daughter
                } else {
                    Some(d)
                }
            }
            _ => {
                let (dz, da, dm) = self.delta();
                parent.apply_delta(dz, da, dm)
            }
        }
    }
}

/// A neutron-induced reaction channel.
///
/// The transmutation channels each map parent → daughter through a fixed
/// `(dZ, dA, dm)` delta (ONIX `xs_prod_fromS_toS`,
/// `onix/data/list_and_dict.py:244`). [`ReactionChannel::Fission`] is special:
/// it has no single daughter — its products come from a fission-yield table
/// (see [`crate::chain::FissionYields`]).
///
/// The associated *rate* of a channel is the one-group (or few-group collapsed)
/// reaction rate in `1/s` (i.e. microscopic cross section in barns × `1e-24`
/// cm²/barn × scalar flux in n·cm⁻²·s⁻¹). This enum encodes only identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactionChannel {
    /// Radiative capture `(n,γ)`: `A+1`, `Z` unchanged. `[0, +1, 0]`.
    NGamma,
    /// `(n,2n)`: net loss of one nucleon, `A-1`. `[0, -1, 0]`.
    N2n,
    /// `(n,3n)`: net loss of two nucleons, `A-2`. `[0, -2, 0]`.
    N3n,
    /// `(n,p)`: `Z-1`, `A` unchanged (absorb n, emit p). `[-1, 0, 0]`.
    Np,
    /// `(n,α)`: `Z-2`, `A-3` (absorb n, emit ⁴He). `[-2, -3, 0]`.
    NAlpha,
    /// `(n,t)`: `Z-1`, `A-2` (absorb n, emit triton). `[-1, -2, 0]`.
    NT,
    /// Neutron-induced fission. No single daughter — products are drawn from a
    /// fission-yield table. [`ReactionChannel::daughter`] returns `None`.
    Fission,
}

impl ReactionChannel {
    /// The `(dZ, dA, dm)` delta for the transmutation channels.
    ///
    /// Returns `None` for [`ReactionChannel::Fission`], which has no fixed
    /// daughter. Deltas reproduce ONIX `xs_prod_fromS_toS`
    /// (`onix/data/list_and_dict.py:244`).
    pub const fn delta(&self) -> Option<(i32, i32, i32)> {
        match self {
            ReactionChannel::NGamma => Some((0, 1, 0)),
            ReactionChannel::N2n => Some((0, -1, 0)),
            ReactionChannel::N3n => Some((0, -2, 0)),
            ReactionChannel::Np => Some((-1, 0, 0)),
            ReactionChannel::NAlpha => Some((-2, -3, 0)),
            ReactionChannel::NT => Some((-1, -2, 0)),
            ReactionChannel::Fission => None,
        }
    }

    /// Whether this channel is fission (its products come from a yield table).
    pub const fn is_fission(&self) -> bool {
        matches!(self, ReactionChannel::Fission)
    }

    /// The transmutation daughter, or `None` for fission / unphysical results.
    pub fn daughter(&self, parent: Nuclide) -> Option<Nuclide> {
        let (dz, da, dm) = self.delta()?;
        parent.apply_delta(dz, da, dm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ngamma_captures_a_neutron() {
        // U-238 (n,gamma) -> U-239.
        let u238 = Nuclide::new(92, 238, 0);
        assert_eq!(
            ReactionChannel::NGamma.daughter(u238),
            Some(Nuclide::new(92, 239, 0))
        );
    }

    #[test]
    fn beta_minus_raises_z() {
        // U-239 beta- -> Np-239.
        let u239 = Nuclide::new(92, 239, 0);
        assert_eq!(
            DecayMode::BetaMinus.daughter(u239),
            Some(Nuclide::new(93, 239, 0))
        );
    }

    #[test]
    fn alpha_drops_z2_a4() {
        // Pu-239 alpha -> U-235.
        let pu239 = Nuclide::new(94, 239, 0);
        assert_eq!(
            DecayMode::Alpha.daughter(pu239),
            Some(Nuclide::new(92, 235, 0))
        );
    }

    #[test]
    fn fission_has_no_single_daughter() {
        assert!(ReactionChannel::Fission.delta().is_none());
        assert!(ReactionChannel::Fission
            .daughter(Nuclide::new(92, 235, 0))
            .is_none());
    }
}
