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

/// The metastable-state change ONIX applies for a parent in state `parent_m`,
/// given the change `dm_ground` the same channel applies to a ground-state
/// parent.
///
/// ONIX keeps four tables per family (`xs_prod_fromS_toS`, `_fromS_toX`,
/// `_fromX_toS`, `_fromX_toX`, and the `decay_prod_*` equivalents,
/// `onix/data/list_and_dict.py:244-259`). Every `fromX` entry is the matching
/// `fromS` entry with `dm` lowered by one (`X(n,gamma)` is `[0,1,-1]` against
/// `(n,gamma)` `[0,1,0]`; `X(n,2n)X` is `[0,-1,0]` against `(n,2n)X`
/// `[0,-1,1]`), so the daughter's state depends only on the channel, not on the
/// parent's. ONIX supports only states 0 and 1 (`Passport._set_state` reads the
/// last digit, but the table lookups in `passport.py:355-410` branch on
/// `state == 0` / `state == 1` only), so a state above 1 has no daughter here.
const fn state_adjusted_dm(parent_m: u8, dm_ground: i32) -> Option<i32> {
    match parent_m {
        0 => Some(dm_ground),
        1 => Some(dm_ground - 1),
        _ => None,
    }
}

/// A radioactive decay mode.
///
/// Each mode maps a parent nuclide to a daughter via a fixed `(dZ, dA, dm)`
/// transformation. The associated *rate* of a mode is the partial decay
/// constant (units `1/s`, see [`crate::chain::DecayData`]); this enum only
/// encodes the identity transformation, not the rate.
///
/// Delta triples reproduce ONIX `decay_prod_fromS_toS` and `decay_prod_fromS_toX`
/// (`onix/data/list_and_dict.py:250-251`), the table entries for a **ground-state**
/// parent; [`DecayMode::daughter`] derives the `fromX_*` entries (`Xbetaneg`,
/// `XbetanegX`, ...) from the parent's state, as ONIX does through its four tables.
/// The `...ToIsomer` variants are ONIX's `betanegX`, `betaposX`, `alphaX`,
/// `neutronX`, `protonX` (the daughter is born in the metastable state).
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
    /// β⁻ decay into the metastable state of the daughter (ONIX `betanegX`,
    /// `[+1, 0, +1]`).
    BetaMinusToIsomer,
    /// β⁺ / EC into the metastable state of the daughter (ONIX `betaposX`,
    /// `[-1, 0, +1]`).
    BetaPlusToIsomer,
    /// α decay into the metastable state of the daughter (ONIX `alphaX`,
    /// `[-2, -4, +1]`).
    AlphaToIsomer,
    /// Neutron emission into the metastable state of the daughter (ONIX
    /// `neutronX`, `[0, -1, +1]`).
    NeutronEmissionToIsomer,
    /// Proton emission into the metastable state of the daughter (ONIX
    /// `protonX`, `[-1, -1, +1]`).
    ProtonEmissionToIsomer,
    /// Isomeric transition: same `Z`, same `A`, de-excite to ground (`m -> 0`).
    ///
    /// ONIX handles metastable de-excitation through its state bookkeeping; here
    /// we model the common ground-state landing (`dm` set so the daughter is
    /// `m = 0`) via [`DecayMode::daughter`]'s special case.
    IsomericTransition,
}

impl DecayMode {
    /// The `(dZ, dA, dm)` delta this mode applies to a **ground-state** parent.
    ///
    /// Dimensionless integer deltas. For [`DecayMode::IsomericTransition`] the
    /// `dm` is not fixed (it depends on the parent's `m`); see
    /// [`DecayMode::daughter`], which returns the ground-state isomer directly,
    /// and which also lowers `dm` by one for an excited parent (ONIX's `fromX`
    /// tables).
    pub const fn delta(&self) -> (i32, i32, i32) {
        match self {
            DecayMode::BetaMinus => (1, 0, 0),
            DecayMode::BetaPlus => (-1, 0, 0),
            DecayMode::Alpha => (-2, -4, 0),
            DecayMode::NeutronEmission => (0, -1, 0),
            DecayMode::ProtonEmission => (-1, -1, 0),
            DecayMode::BetaMinusToIsomer => (1, 0, 1),
            DecayMode::BetaPlusToIsomer => (-1, 0, 1),
            DecayMode::AlphaToIsomer => (-2, -4, 1),
            DecayMode::NeutronEmissionToIsomer => (0, -1, 1),
            DecayMode::ProtonEmissionToIsomer => (-1, -1, 1),
            DecayMode::IsomericTransition => (0, 0, 0),
        }
    }

    /// The daughter nuclide produced by this decay mode, or `None` if the
    /// transformation would be unphysical.
    ///
    /// Isomeric transition lands the parent in its own ground state (`m = 0`).
    /// All other modes apply the delta from [`DecayMode::delta`] with `dm`
    /// adjusted for the parent's state: a decay from an excited parent returns
    /// to the ground state unless the mode is a `...ToIsomer` one (ONIX
    /// `Xbetaneg` `[1,0,-1]`, `XbetanegX` `[1,0,0]`, ...). A parent with `m > 1`
    /// has no daughter (ONIX models states 0 and 1 only).
    ///
    /// Before 2026-11-13 (gh:#1404) the delta was applied as-is, so the daughter of
    /// an excited parent kept `m = 1`, where ONIX lands in the ground state.
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
                let dm = state_adjusted_dm(parent.m, dm)?;
                parent.apply_delta(dz, da, dm)
            }
        }
    }
}

/// A neutron-induced reaction channel.
///
/// The transmutation channels each map parent → daughter through a fixed
/// `(dZ, dA, dm)` delta (ONIX `xs_prod_fromS_toS` and `xs_prod_fromS_toX`,
/// `onix/data/list_and_dict.py:244-245`, the ground-state-parent entries;
/// [`ReactionChannel::daughter`] derives the `fromX_*` entries from the
/// parent's state). [`ReactionChannel::Fission`] is special:
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
    /// `(n,γ)` into the metastable state (ONIX `(n,gamma)X`): `A+1`, `m+1`.
    /// `[0, +1, +1]`.
    NGammaToIsomer,
    /// `(n,2n)` into the metastable state (ONIX `(n,2n)X`): `A-1`, `m+1`.
    /// `[0, -1, +1]`.
    N2nToIsomer,
    /// Neutron-induced fission. No single daughter — products are drawn from a
    /// fission-yield table. [`ReactionChannel::daughter`] returns `None`.
    Fission,
}

impl ReactionChannel {
    /// The `(dZ, dA, dm)` delta for the transmutation channels.
    ///
    /// Returns `None` for [`ReactionChannel::Fission`], which has no fixed
    /// daughter. Deltas reproduce ONIX `xs_prod_fromS_toS` and `xs_prod_fromS_toX`
    /// (`onix/data/list_and_dict.py:244-245`), i.e. the entries for a
    /// ground-state parent.
    pub const fn delta(&self) -> Option<(i32, i32, i32)> {
        match self {
            ReactionChannel::NGamma => Some((0, 1, 0)),
            ReactionChannel::N2n => Some((0, -1, 0)),
            ReactionChannel::N3n => Some((0, -2, 0)),
            ReactionChannel::Np => Some((-1, 0, 0)),
            ReactionChannel::NAlpha => Some((-2, -3, 0)),
            ReactionChannel::NT => Some((-1, -2, 0)),
            ReactionChannel::NGammaToIsomer => Some((0, 1, 1)),
            ReactionChannel::N2nToIsomer => Some((0, -1, 1)),
            ReactionChannel::Fission => None,
        }
    }

    /// Whether this channel is fission (its products come from a yield table).
    pub const fn is_fission(&self) -> bool {
        matches!(self, ReactionChannel::Fission)
    }

    /// The transmutation daughter, or `None` for fission / unphysical results.
    ///
    /// `dm` is adjusted for the parent's state as ONIX's `xs_prod_fromX_*` tables
    /// do: a reaction on an excited parent lands in the ground state unless the
    /// channel is a `...ToIsomer` one (`X(n,gamma)` `[0,1,-1]`, `X(n,gamma)X`
    /// `[0,1,0]`). A parent with `m > 1` has no daughter. Before 2026-11-13
    /// (gh:#1404) the delta was applied as-is, so an excited parent's daughter
    /// kept `m = 1`.
    pub fn daughter(&self, parent: Nuclide) -> Option<Nuclide> {
        let (dz, da, dm) = self.delta()?;
        let dm = state_adjusted_dm(parent.m, dm)?;
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

    /// ONIX's four decay tables and four reaction tables
    /// (`onix/data/list_and_dict.py:244-259`), transcribed as `(dZ, dA, state of
    /// the daughter)` for a parent at Z=60, A=150: the daughter's state depends
    /// only on the channel (`fromS_toS`/`fromX_toS` give 0, `fromS_toX`/
    /// `fromX_toX` give 1), whatever the parent's state.
    #[test]
    fn daughters_equal_the_four_onix_tables() {
        let ground = Nuclide::new(60, 150, 0);
        let excited = Nuclide::new(60, 150, 1);
        let at = |m: u8, dz: i32, da: i32| Nuclide::new((60 + dz) as u32, (150 + da) as u32, m);
        let decays: [(DecayMode, i32, i32, u8); 10] = [
            (DecayMode::BetaMinus, 1, 0, 0),
            (DecayMode::BetaPlus, -1, 0, 0),
            (DecayMode::Alpha, -2, -4, 0),
            (DecayMode::NeutronEmission, 0, -1, 0),
            (DecayMode::ProtonEmission, -1, -1, 0),
            (DecayMode::BetaMinusToIsomer, 1, 0, 1),
            (DecayMode::BetaPlusToIsomer, -1, 0, 1),
            (DecayMode::AlphaToIsomer, -2, -4, 1),
            (DecayMode::NeutronEmissionToIsomer, 0, -1, 1),
            (DecayMode::ProtonEmissionToIsomer, -1, -1, 1),
        ];
        for (mode, dz, da, to) in decays {
            assert_eq!(mode.daughter(ground), Some(at(to, dz, da)), "{mode:?} from S");
            assert_eq!(mode.daughter(excited), Some(at(to, dz, da)), "{mode:?} from X");
        }
        // Xgamma [0,0,-1]; a ground state has no isomeric transition
        assert_eq!(DecayMode::IsomericTransition.daughter(excited), Some(ground));
        assert_eq!(DecayMode::IsomericTransition.daughter(ground), None);
        let reacs: [(ReactionChannel, i32, i32, u8); 8] = [
            (ReactionChannel::NGamma, 0, 1, 0),
            (ReactionChannel::N2n, 0, -1, 0),
            (ReactionChannel::N3n, 0, -2, 0),
            (ReactionChannel::Np, -1, 0, 0),
            (ReactionChannel::NAlpha, -2, -3, 0),
            (ReactionChannel::NT, -1, -2, 0),
            (ReactionChannel::NGammaToIsomer, 0, 1, 1),
            (ReactionChannel::N2nToIsomer, 0, -1, 1),
        ];
        for (ch, dz, da, to) in reacs {
            assert_eq!(ch.daughter(ground), Some(at(to, dz, da)), "{ch:?} from S");
            assert_eq!(ch.daughter(excited), Some(at(to, dz, da)), "{ch:?} from X");
        }
    }

    #[test]
    fn states_above_one_have_no_daughter() {
        let m2 = Nuclide::new(60, 150, 2);
        assert_eq!(DecayMode::BetaMinus.daughter(m2), None);
        assert_eq!(ReactionChannel::NGamma.daughter(m2), None);
    }
}
