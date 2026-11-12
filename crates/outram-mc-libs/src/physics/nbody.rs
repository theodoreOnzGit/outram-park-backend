// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Translation of `NBodyPhaseSpace` from OpenMC `src/secondary_nbody.cpp` and
// `include/openmc/secondary_nbody.h`.
//   Upstream:  OpenMC (https://github.com/openmc-dev/openmc), commit
//              8f79d9d02483c90e4134f03a9df4b75ce6b062bf (2026-11-10)
//   Routines:  NBodyPhaseSpace::sample_energy, NBodyPhaseSpace::sample
//   Copyright: 2011-2025 Massachusetts Institute of Technology, UChicago
//              Argonne LLC, and OpenMC contributors. MIT licence.

//! **N-body phase-space** emission (ENDF MF=6 LAW=6 / ACE LAW=66) sampled the
//! way OpenMC samples it (gh:#1444, #1386 prio-1 row 4).
//!
//! # Why this exists next to the tabulated route
//!
//! The ACE route in [`crate::material::nuclide`] turns LAW=66 into a tabulated
//! outgoing-energy table through `njoy_outram_park_fork::acer::ce_laws::
//! ace_phase_space_chi` (the table ACER itself would write). OpenMC never
//! tabulates it: `NBodyPhaseSpace` draws `E'/E'_max = x/(x+y)` directly from
//! Maxwellian and gamma-like variates (Monte Carlo sampler rules C45/C61 for 3,
//! 4 and 5 bodies). The two describe the same density but consume a different
//! number of random variates, so only this sampler can be bit-identical to
//! OpenMC for a fixed `xi` sequence. [`NBodyPhaseSpace`] is that sampler;
//! `tests/secondary_samplers_vs_openmc_code_to_code.rs` pins it against the
//! compiled upstream (bit identity of `E'`, `mu` and the end state). Transport
//! still reads the tabulated form (see the status note in the crate README).

use crate::mathf::RealMath;
use crate::rng::distributions::{maxwell, uniform};
use crate::rng::lcg::prn;
use std::f64::consts::PI;

/// An `n`-body phase-space emission law (`NBodyPhaseSpace`, OpenMC
/// `secondary_nbody.h`). Isotropic in the centre of mass by definition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NBodyPhaseSpace {
    /// Number of bodies in the final state (3, 4 or 5; upstream refuses more).
    pub n_bodies: i32,
    /// `Ap`, the total mass of the emitted particles in neutron masses (ACE `APSX`).
    pub mass_ratio: f64,
    /// Target atomic weight ratio `A`.
    pub awr: f64,
    /// Reaction Q-value \[eV\].
    pub q_value: f64,
}

impl NBodyPhaseSpace {
    /// `NBodyPhaseSpace::sample_energy`: the outgoing energy in the CM frame
    /// \[eV\] for incident energy `e_in` \[eV\].
    ///
    /// # Panics
    ///
    /// For `n_bodies` outside `3..=5`, as upstream's `fatal_error("N-body phase
    /// space with >5 bodies.")`.
    pub fn sample_energy(&self, e_in: f64, seed: &mut u64) -> f64 {
        // Determine E_max parameter
        let ap = self.mass_ratio;
        let e_max = (ap - 1.0) / ap * (self.awr / (self.awr + 1.0) * e_in + self.q_value);

        // x is essentially a Maxwellian distribution
        let x = maxwell(seed, 1.0);

        let y = match self.n_bodies {
            3 => maxwell(seed, 1.0),
            4 => {
                let r1 = prn(seed);
                let r2 = prn(seed);
                let r3 = prn(seed);
                -(r1 * r2 * r3).r_ln()
            }
            5 => {
                let r1 = prn(seed);
                let r2 = prn(seed);
                let r3 = prn(seed);
                let r4 = prn(seed);
                let r5 = prn(seed);
                let r6 = prn(seed);
                let c = (PI / 2.0 * r6).r_cos();
                // upstream: std::pow(std::cos(..), 2), which g++ folds to c * c
                -(r1 * r2 * r3 * r4).r_ln() - r5.r_ln() * (c * c)
            }
            n => panic!("N-body phase space with {n} bodies (upstream supports 3..=5)"),
        };

        // Now determine v and E_out
        let v = x / (x + y);
        e_max * v
    }

    /// `NBodyPhaseSpace::sample`: `(E_out, mu)` in the CM frame. The cosine is
    /// drawn first (uniform on `[-1, 1)`), then the energy, as upstream.
    pub fn sample(&self, e_in: f64, seed: &mut u64) -> (f64, f64) {
        let mu = uniform(seed, -1.0, 1.0);
        let e_out = self.sample_energy(e_in, seed);
        (e_out, mu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Physical bounds: `0 <= E' <= E'_max` and a cosine in `[-1, 1)`.
    #[test]
    fn energy_is_bounded_by_e_max() {
        for n in 3..=5 {
            let d = NBodyPhaseSpace { n_bodies: n, mass_ratio: 3.2, awr: 235.0, q_value: -6.5e6 };
            let e_in = 1.4e7;
            let e_max = (3.2 - 1.0) / 3.2 * (235.0 / 236.0 * e_in - 6.5e6);
            let mut s = 12345u64;
            for _ in 0..200 {
                let (e, mu) = d.sample(e_in, &mut s);
                assert!(e >= 0.0 && e <= e_max, "n={n}: {e} not in [0, {e_max}]");
                assert!((-1.0..1.0).contains(&mu));
            }
        }
    }

    /// Upstream refuses more than five bodies.
    #[test]
    #[should_panic(expected = "N-body phase space")]
    fn six_bodies_is_refused() {
        let d = NBodyPhaseSpace { n_bodies: 6, mass_ratio: 3.2, awr: 235.0, q_value: -6.5e6 };
        d.sample_energy(1.4e7, &mut 1u64);
    }
}
