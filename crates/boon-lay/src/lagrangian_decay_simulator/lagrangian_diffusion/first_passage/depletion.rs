//! Decay and transmutation coupled to the Walk-on-Spheres diffusion.
//!
//! A diffusing atom does not keep its identity forever: it decays along its
//! chain, and under a neutron field it can capture, (n,2n), or fission. This
//! module runs those as **competing exponential clocks alongside each
//! Walk-on-Spheres hop**. For the current nuclide the total event rate is
//!
//! ```text
//! lambda_total = lambda_decay + lambda_transmute,
//! lambda_decay = ln2 / t_half,
//! lambda_transmute = phi * sigma   (an external neutron field),
//! ```
//!
//! and the time to the next event is `Exp(lambda_total)`. Each hop takes a
//! first-passage time `tau`; if an event time falls inside `tau`, the atom
//! changes nuclide (its diffusion coefficient updates on the spot) instead of
//! completing that hop.
//!
//! The ensemble of walker identities over time **is** the depleted inventory —
//! no Bateman matrix, no CRAM, no stiffness handling (the design the crate's
//! `CLAUDE.md` calls for).
//!
//! ## Scope and the one approximation
//!
//! - **Decay** is fully wired to the crate's `DecayLibrary` (real half-lives and
//!   branching ratios); the daughter is drawn from the walker's own RNG stream.
//! - **Transmutation** is a framework that takes the neutron field as an
//!   **external** input ([`Transmutation`]). A single `(n,gamma)` channel is
//!   provided as the minimal, explicit stand-in; wiring per-nuclide cross
//!   sections and fission yields to `njoy-outram-park-fork` /
//!   `outram-mc-libs` flux maps is the deferred follow-up.
//! - **Placement approximation.** When an event preempts a hop, the atom is left
//!   at the hop's *start* position (its identity changes there) while its clock
//!   advances by the exact event time. This is unbiased in *timing* but
//!   conservative in *position*; it is accurate whenever the hop time is short
//!   compared with the event time (the usual case, since a mobile — high `D` —
//!   nuclide has short hops, and a slow — low `D` — nuclide barely moves in the
//!   event time either way). The exact Brownian-bridge placement is a possible
//!   refinement.

use fission_yields_data::prelude::Nuclide;
use outram_mc_libs::rng::lcg::prn;
use uom::si::f64::{Frequency, Time};
use uom::si::ratio::ratio;
use uom::ConstZero;

use crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary;
use crate::nuclide_reaction_and_decay_data::HalfLifeAndDecayEnergyInfo;

use super::walk_on_spheres::{HopOutcome, WalkParams, WoSWalker};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;

/// The external neutron field driving transmutation.
///
/// This is where the neutron flux enters the Lagrangian model. It is kept
/// deliberately explicit and small; a per-nuclide cross-section library and
/// fission-yield sampling (from `njoy-outram-park-fork`) plug in here later.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transmutation {
    /// No neutron field — radioactive decay only.
    None,
    /// A single `(n,gamma)` capture channel: while the walker is `target`, it
    /// captures at rate `capture_rate = phi * sigma_capture` and becomes
    /// `product`. A minimal explicit stand-in for the deferred per-nuclide
    /// cross-section coupling.
    SingleCapture {
        /// The nuclide the channel acts on.
        target: Nuclide,
        /// Capture rate `phi * sigma_capture` folded into one frequency.
        capture_rate: Frequency,
        /// The nuclide produced by capture.
        product: Nuclide,
    },
}

impl Transmutation {
    /// Transmutation rate acting on `nuclide` under this field.
    #[inline]
    pub fn rate_for(&self, nuclide: Nuclide) -> Frequency {
        match *self {
            Transmutation::None => Frequency::ZERO,
            Transmutation::SingleCapture {
                target,
                capture_rate,
                ..
            } => {
                if nuclide == target {
                    capture_rate
                } else {
                    Frequency::ZERO
                }
            }
        }
    }

    /// The product `nuclide` transmutes into under this field, if any channel
    /// applies.
    #[inline]
    pub fn product_for(&self, nuclide: Nuclide) -> Option<Nuclide> {
        match *self {
            Transmutation::None => None,
            Transmutation::SingleCapture {
                target, product, ..
            } => {
                if nuclide == target {
                    Some(product)
                } else {
                    None
                }
            }
        }
    }
}

/// The fate of a walker after [`WoSWalker::advance_until`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepletionOutcome {
    /// The atom was released from the OPyC surface at `time` as `nuclide`.
    Released {
        /// Simulated release time.
        time: Time,
        /// Nuclide identity at release.
        nuclide: Nuclide,
    },
    /// The atom was still inside the particle when the requested time was
    /// reached, as `nuclide`.
    Surviving {
        /// Nuclide identity at the requested time.
        nuclide: Nuclide,
    },
    /// The step cap was hit before either of the above (a runaway guard).
    StepLimit {
        /// Nuclide identity when the cap was hit.
        nuclide: Nuclide,
    },
}

/// Sample a waiting time `~ Exp(rate)` from a uniform deviate `u` in `[0, 1)`.
///
/// Returns `-ln(1 - u) / rate`, whose mean is `1/rate`. An event is deemed to
/// occur within an interval `tau` iff the returned time is `< tau` (which holds
/// with probability `1 - exp(-rate*tau)`), so this one draw serves as both the
/// occurrence test and the event time.
#[inline]
pub fn sample_event_time(rate: Frequency, u: f64) -> Time {
    -((1.0 - u).ln()) / rate
}

impl WoSWalker {
    /// Radioactive decay rate `ln2 / t_half` of the walker's current nuclide,
    /// or zero if it is stable or absent from the library.
    #[inline]
    pub fn decay_rate(&self, decay_library: &DecayLibrary) -> Frequency {
        match decay_library.try_match_nuclides_to_decay_data(self.nuclide) {
            Some(data) => match data.half_life_information {
                HalfLifeAndDecayEnergyInfo::Unstable(half_life, _energy) => {
                    2.0_f64.ln() / half_life
                }
                HalfLifeAndDecayEnergyInfo::Stable => Frequency::ZERO,
            },
            None => Frequency::ZERO,
        }
    }

    /// Replace the current nuclide with a stochastically chosen decay daughter,
    /// drawn from the branching ratios using the walker's own RNG. Returns
    /// `true` if the nuclide changed (it was unstable), `false` otherwise.
    #[inline]
    pub fn decay_once(&mut self, decay_library: &DecayLibrary) -> bool {
        let Some(data) = decay_library.try_match_nuclides_to_decay_data(self.nuclide) else {
            return false;
        };
        let u = prn(&mut self.rng.0);
        match data.get_next_target_nuclide_with_float(u) {
            Some((daughter, _decay_type)) => {
                self.nuclide = daughter;
                true
            }
            None => false,
        }
    }

    /// Advance the walker — diffusing while decaying and transmuting — until it
    /// is released from the OPyC surface or its simulated time reaches `until`.
    ///
    /// At each step the total event rate `lambda_decay + lambda_transmute` sets a
    /// competing clock. If an event precedes the hop's first-passage time (and
    /// precedes `until`), the walker changes nuclide at the hop's start position
    /// with its clock advanced by the exact event time; otherwise the hop
    /// commits. See the module docs for the placement approximation.
    pub fn advance_until(
        &mut self,
        triso_cell: &TrisoCell,
        params: &WalkParams,
        decay_library: &DecayLibrary,
        transmutation: Transmutation,
        until: Time,
    ) -> DepletionOutcome {
        for _ in 0..params.max_steps {
            if self.time >= until {
                return DepletionOutcome::Surviving {
                    nuclide: self.nuclide,
                };
            }
            let remaining = until - self.time;

            let decay_rate = self.decay_rate(decay_library);
            let transmute_rate = transmutation.rate_for(self.nuclide);
            let total_rate = decay_rate + transmute_rate;

            let event_time = if total_rate > Frequency::ZERO {
                Some(sample_event_time(total_rate, prn(&mut self.rng.0)))
            } else {
                None
            };

            let pre_pos = self.position;
            let pre_time = self.time;
            let outcome = self.step_multilayer(triso_cell, params);
            let tau = self.time - pre_time;

            if outcome == HopOutcome::Released {
                return DepletionOutcome::Released {
                    time: self.time,
                    nuclide: self.nuclide,
                };
            }

            if let Some(t_e) = event_time {
                if t_e < tau && t_e < remaining {
                    // An event preempts this hop, before `until`.
                    self.position = pre_pos;
                    self.time = pre_time + t_e;

                    // Choose the channel in proportion to its rate.
                    let pick = prn(&mut self.rng.0) * total_rate;
                    if pick < decay_rate {
                        self.decay_once(decay_library);
                    } else if let Some(product) = transmutation.product_for(self.nuclide) {
                        self.nuclide = product;
                    }
                    continue;
                }
            }

            // No event before `until`. If the hop crossed `until`, the identity
            // there is unchanged.
            if self.time >= until {
                return DepletionOutcome::Surviving {
                    nuclide: self.nuclide,
                };
            }
        }

        DepletionOutcome::StepLimit {
            nuclide: self.nuclide,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
    use approx::assert_relative_eq;
    use uom::si::frequency::hertz;
    use uom::si::time::second;

    #[test]
    fn sample_event_time_has_mean_inverse_rate() {
        let rate = Frequency::new::<hertz>(2.5);
        let mut seed = 0xfeed_face_dead_beef_u64;
        let n = 200_000;
        let mean = (0..n)
            .map(|_| sample_event_time(rate, prn(&mut seed)).get::<second>())
            .sum::<f64>()
            / n as f64;
        assert_relative_eq!(mean, 1.0 / 2.5, max_relative = 0.02);
    }

    #[test]
    fn decay_changes_nuclide_to_higher_z_daughter() {
        // Cs-137 (Z=55) beta-decays to a barium isotope (Z=56).
        let library = DecayLibrary::new();
        let mut walker = WoSWalker::new_at_center(Nuclide::Cs137, OoRng64::from_u64(1));
        let changed = walker.decay_once(&library);
        assert!(changed, "Cs-137 is unstable and should decay");
        let (z, _a) = walker.nuclide.get_z_a();
        assert_eq!(z, 56, "beta-minus decay of Cs (Z=55) should give Ba (Z=56)");
    }

    #[test]
    fn irradiation_transmutes_a_stable_target() {
        // A stable target (Cs-133) under a strong (n,gamma) field becomes Cs-134
        // well before it can diffuse out through the SiC barrier.
        let library = DecayLibrary::new();
        let cell = TrisoCell::new_crp6_geometry();
        let params = WalkParams::crp6_default();
        let transmutation = Transmutation::SingleCapture {
            target: Nuclide::Cs133,
            capture_rate: Frequency::new::<hertz>(10.0), // mean ~0.1 s
            product: Nuclide::Cs134,
        };
        let mut walker = WoSWalker::new_at_center(Nuclide::Cs133, OoRng64::from_u64(0x9999));
        let outcome = walker.advance_until(
            &cell,
            &params,
            &library,
            transmutation,
            Time::new::<second>(10.0),
        );
        match outcome {
            DepletionOutcome::Surviving { nuclide } => {
                assert_eq!(nuclide, Nuclide::Cs134, "target should have captured to Cs-134");
            }
            other => panic!("expected Surviving as Cs-134, got {other:?}"),
        }
    }

    #[test]
    fn no_field_leaves_a_stable_nuclide_unchanged() {
        let library = DecayLibrary::new();
        let cell = TrisoCell::new_crp6_geometry();
        let params = WalkParams::crp6_default();
        let mut walker = WoSWalker::new_at_center(Nuclide::Cs133, OoRng64::from_u64(3));
        let outcome = walker.advance_until(
            &cell,
            &params,
            &library,
            Transmutation::None,
            Time::new::<second>(1.0),
        );
        match outcome {
            DepletionOutcome::Surviving { nuclide } => assert_eq!(nuclide, Nuclide::Cs133),
            other => panic!("stable Cs-133 with no field should survive unchanged, got {other:?}"),
        }
    }
}
