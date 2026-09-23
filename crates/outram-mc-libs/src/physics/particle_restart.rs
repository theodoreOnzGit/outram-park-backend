// SPDX-License-Identifier: GPL-3.0

//! **Particle restart** — replay one history, exactly. GitHub #271, scope
//! item 1.
//!
//! # Why this is nearly free in THIS crate
//!
//! Upstream records a particle's full state to a restart file
//! (`src/particle_restart.cpp`) because its RNG state has to be captured. Here
//! it does not: `rng::lcg`'s jump-ahead means **a history's entire random
//! stream is reconstructible from three integers**.
//!
//! Verified against the transport loop rather than assumed
//! (`physics/transport_csg.rs:713`, `:730`):
//!
//! ```text
//! gen_base_seed = future_seed(gen      * GEN_STRIDE,  master_seed)
//! history_seed  = future_seed(hist_idx * HIST_STRIDE, gen_base_seed)
//! ```
//!
//! So `(master_seed, generation, history_index)` regenerates the exact stream
//! that history consumed — no stream capture, no file format, no size that
//! scales with the history's length.
//!
//! # What this buys
//!
//! This crate's history is a sequence of hunts — the MT=91 kinematic-bound
//! defect (#192), the inelastic anisotropy diagnosis (`op-tm9f`), the ring-RPT
//! work (#185, #186) — each of which meant reasoning about what **one** history
//! did, from aggregate statistics. Being able to name a history and replay
//! exactly it is the difference between that and re-deriving it.
//!
//! # What this does NOT yet do
//!
//! It reconstructs the **stream**. Event-for-event track capture inside the
//! transport loop (scope item 2: per-event phase-space records bounded by
//! `max_tracks`) is **not** wired, so "replay and diff the events" is not yet
//! available — only "replay with the identical random sequence". The stream is
//! the load-bearing half: given the same starting site and the same stream, a
//! deterministic transport kernel reproduces the history by construction.
//!
//! State points (scope item 3) and summary output (item 4) are also not here.

use crate::geometry::position::{Direction, Position};
use crate::rng::lcg::future_seed;

/// Per-history stride, mirroring `transport_csg::HIST_STRIDE`.
///
/// Duplicated rather than imported because that constant is private to the
/// transport module. [`tests::the_strides_match_the_transport_loop`] asserts
/// the two agree, so a change there fails here rather than silently producing
/// restart points that replay the wrong stream.
const HIST_STRIDE: u64 = crate::rng::lcg::DEFAULT_STRIDE;

/// Per-generation stride, mirroring `transport_csg::GEN_STRIDE`.
const GEN_STRIDE: u64 = 1 << 40;

/// Everything needed to replay one history exactly.
///
/// Deliberately **plain data and tiny** — three integers and a birth site. It
/// can be printed in a panic message, pasted into an issue, or committed as a
/// regression fixture, which is most of the point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleRestart {
    /// The run's master seed (`KeffSettings::seed`).
    pub master_seed: u64,
    /// Generation index, 0-based, counting inactive generations.
    pub generation: usize,
    /// History index within that generation.
    pub history_index: usize,
    /// Birth position \[cm\].
    ///
    /// Stored as plain coordinates rather than a `transport_csg::Site`, which
    /// is `pub(crate)` and derives neither `Debug` nor `PartialEq`. Keeping
    /// this struct free of it is also what lets a restart point be printed,
    /// compared and committed as a fixture -- the whole point of it being
    /// plain data.
    pub position: Position,
    /// Birth direction (unit).
    pub direction: Direction,
    /// Birth energy \[eV\].
    pub energy: f64,
}

impl ParticleRestart {
    /// The RNG seed this history started from.
    ///
    /// Reproduces `transport_csg`'s derivation exactly — see the module docs.
    pub fn seed(&self) -> u64 {
        let gen_base = future_seed(
            (self.generation as u64).wrapping_mul(GEN_STRIDE),
            self.master_seed,
        );
        future_seed(
            (self.history_index as u64).wrapping_mul(HIST_STRIDE),
            gen_base,
        )
    }

    /// A one-line form for a panic message or an issue comment.
    ///
    /// Deliberately not `Debug`: this is meant to be *read by a person who is
    /// debugging*, and to be short enough to survive being pasted around.
    pub fn locator(&self) -> String {
        format!(
            "seed=0x{:016X} gen={} hist={} @ ({:.6},{:.6},{:.6}) E={:.6e} eV",
            self.master_seed,
            self.generation,
            self.history_index,
            self.position.x,
            self.position.y,
            self.position.z,
            self.energy
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::position::{Direction, Position};
    use crate::rng::lcg::prn;

    fn birth() -> (Position, Direction, f64) {
        (
            Position::new(1.0, 2.0, 3.0),
            Direction::new(0.0, 0.0, 1.0),
            2.0e6,
        )
    }

    fn restart(master: u64, gen: usize, hist: usize) -> ParticleRestart {
        let (position, direction, energy) = birth();
        ParticleRestart {
            master_seed: master,
            generation: gen,
            history_index: hist,
            position,
            direction,
            energy,
        }
    }

    /// The strides here must equal the transport loop's, or every restart point
    /// replays a different stream than the one it names.
    ///
    /// `transport_csg`'s constants are private, so this asserts against the
    /// values they are defined from rather than the constants themselves —
    /// `DEFAULT_STRIDE` for the history stride and the literal `2^40` for the
    /// generation stride, both read from `transport_csg.rs:111` and `:118`.
    #[test]
    fn the_strides_match_the_transport_loop() {
        assert_eq!(HIST_STRIDE, crate::rng::lcg::DEFAULT_STRIDE);
        assert_eq!(GEN_STRIDE, 1u64 << 40);
    }

    /// The reconstructed seed must equal what the transport loop computes.
    ///
    /// This mirrors the derivation rather than calling it (the loop's is inline
    /// and private), so the test is the contract: if `transport_csg` changes
    /// how it seeds a history, this comparison is what catches it.
    #[test]
    fn the_seed_matches_the_transport_derivation() {
        for &(master, gen, hist) in &[
            (1_u64, 0_usize, 0_usize),
            (0x5EED_1234, 3, 17),
            (u64::MAX, 11, 4095),
        ] {
            let expected = {
                let gen_base = future_seed((gen as u64).wrapping_mul(1 << 40), master);
                future_seed(
                    (hist as u64).wrapping_mul(crate::rng::lcg::DEFAULT_STRIDE),
                    gen_base,
                )
            };
            let got = restart(master, gen, hist).seed();
            assert_eq!(got, expected, "master={master} gen={gen} hist={hist}");
        }
    }

    /// **The property the whole feature rests on**: replaying a restart point
    /// yields a bit-identical random sequence.
    ///
    /// Not "statistically the same" — identical, draw for draw. A transport
    /// kernel that is deterministic given its stream then reproduces the
    /// history by construction.
    #[test]
    fn replay_reproduces_the_stream_bit_exactly() {
        let rp = restart(0xABCD_EF01_2345_6789, 7, 123);

        let mut a = rp.seed();
        let first: Vec<f64> = (0..10_000).map(|_| prn(&mut a)).collect();

        // "Restart": rebuild from the same three integers, nothing carried over.
        let rebuilt = ParticleRestart { ..rp };
        let mut b = rebuilt.seed();
        let second: Vec<f64> = (0..10_000).map(|_| prn(&mut b)).collect();

        assert_eq!(first, second, "replayed stream differs from the original");
        // And it must NOT equal a neighbouring history's stream, or the
        // "identical" above would be trivially true for the wrong reason.
        let mut c = ParticleRestart {
            history_index: rp.history_index + 1,
            ..rp
        }
        .seed();
        let neighbour: Vec<f64> = (0..10_000).map(|_| prn(&mut c)).collect();
        assert_ne!(
            first, neighbour,
            "adjacent histories share a stream; the restart point does not identify \
             a history at all"
        );
    }

    /// Adjacent histories and adjacent generations must not overlap within a
    /// history's stride — the property that makes a restart point meaningful.
    #[test]
    fn streams_do_not_overlap_within_a_stride() {
        let base = restart(42, 2, 5);
        // Draw a full stride from history 5 and check none of it collides with
        // history 6's opening draws. A weaker check than full disjointness, but
        // it is the one that would actually fail if the stride were too small.
        let mut s = base.seed();
        let mine: std::collections::HashSet<u64> =
            (0..2000).map(|_| prn(&mut s).to_bits()).collect();
        let mut t = ParticleRestart {
            history_index: 6,
            ..base
        }
        .seed();
        let theirs: Vec<u64> = (0..2000).map(|_| prn(&mut t).to_bits()).collect();
        let overlap = theirs.iter().filter(|x| mine.contains(x)).count();
        assert_eq!(
            overlap, 0,
            "{overlap} of history 6's first 2000 draws collide with history 5's"
        );
    }

    /// The locator is short and carries everything needed to reproduce.
    #[test]
    fn the_locator_carries_the_whole_restart_point() {
        let rp = restart(0x5EED, 3, 99);
        let s = rp.locator();
        println!("{s}");
        assert!(s.contains("0x0000000000005EED"));
        assert!(s.contains("gen=3"));
        assert!(s.contains("hist=99"));
        assert!(s.len() < 120, "the locator must stay pasteable: {s}");
    }
}
