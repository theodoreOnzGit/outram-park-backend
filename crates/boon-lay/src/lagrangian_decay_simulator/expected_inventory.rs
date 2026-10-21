//! **Expected inventory of a decay chain, by PETIR's CRAM48** — the
//! deterministic counterpart of the per-atom Monte Carlo (gh:#999).
//!
//! The Lagrangian simulators in this crate stay the simulators: they carry
//! positions and stochastic events, which a matrix cannot. But the *mean*
//! of their nuclide populations obeys the Bateman system `dN/dt = A N`, and
//! that mean has an exact solution, `N(t) = exp(A t) N(0)`. This module
//! builds `A` from the same [`DecayLibrary`] the Monte Carlo samples from and
//! solves it with [`petir::depletion::cram48`] (OpenMC's `CRAM48`, ported in
//! PETIR), so the Monte Carlo can be checked against its own expectation
//! (maintainer direction 2026-10-20: "make sure each crate tries to use
//! cram").
//!
//! # The matrix is the one the sampler actually samples
//!
//! - `A[j][j] = -ln 2 / t_half(j)`, and `A[i][j] = p(j -> i) * ln 2 / t_half(j)`
//!   (parent by column, daughter by row, per second).
//! - `p(j -> i)` is the probability that
//!   [`NuclideReactionAndDecayData::get_next_target_nuclide_with_float`]
//!   returns branch `k`, **not** the tabulated branching ratio: that routine
//!   walks the branches in order, subtracting each ratio from a uniform
//!   deviate, so branch `k` is chosen with probability
//!   `min(br_k, max(0, 1 - sum_{m<k} br_m))`. The two agree when the ratios
//!   sum to one; if they sum above one the later branches are truncated
//!   exactly as the sampler truncates them. (If they sum below one the
//!   sampler panics on the remainder, `todo!`, and the matrix loses those
//!   atoms from the network.)
//! - A branch with no target nuclide leaves the network: its probability
//!   appears on the diagonal, not in any row.
//! - A branch whose target is the parent itself adds back onto the
//!   diagonal: the atom "decays" into itself, as the sampler makes it do.
//!   The library writes spontaneous fission that way for Th-232
//!   (`<decay type="sf" target="Th232" branching_ratio="1.4e-11"/>`), so
//!   Th-232's effective loss rate is `lambda (1 - 1.4e-11)` and it feeds
//!   Ra-228 at `0.999999999986 lambda`; fission fragments are not tracked.
//! - Nuclides are collected breadth-first from the start nuclide, so a
//!   branching chain (Bi-212 -> Po-212 / Tl-208) is one matrix.
//!
//! The order is CRAM48, one substep: the most accurate of PETIR's two CRAMs
//! and OpenMC's default. No upstream code here has a CRAM to match.

use fission_yields_data::prelude::Nuclide;
use petir::depletion::{cram48, DepletionMatrix};
use petir::mathf::RealMath;
use uom::si::f64::Time;
use uom::si::ratio::ratio;
use uom::si::time::second;

use crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary;
use crate::nuclide_reaction_and_decay_data::HalfLifeAndDecayEnergyInfo;

/// Why an expected inventory could not be built or solved.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedInventoryError {
    /// A nuclide in the chain has no entry in the [`DecayLibrary`].
    NotInLibrary(Nuclide),
    /// The initial-inventory vector does not match the chain's length.
    LengthMismatch {
        /// [`ExpectedInventory::nuclides`]`.len()`.
        expected: usize,
        /// The length supplied.
        found: usize,
    },
    /// `t` was negative or not finite.
    BadTime,
    /// PETIR's CRAM reported an error (a singular pole solve).
    Cram(petir::PetirError),
}

/// A decay chain's Bateman matrix, built from a [`DecayLibrary`], solved by
/// CRAM48. See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedInventory {
    nuclides: Vec<Nuclide>,
    matrix: DepletionMatrix,
}

impl ExpectedInventory {
    /// Collects every nuclide reachable from `start` through the library's
    /// decay branches (breadth-first, `start` first) and builds the decay
    /// matrix the Monte Carlo sampler implies (module docs).
    ///
    /// # Errors
    ///
    /// [`ExpectedInventoryError::NotInLibrary`] if `start` or a daughter has
    /// no library entry.
    pub fn from_library(
        start: Nuclide,
        decay_library: &DecayLibrary,
    ) -> Result<Self, ExpectedInventoryError> {
        let mut nuclides = vec![start];
        let mut data = Vec::new();
        let mut k = 0;
        while k < nuclides.len() {
            let nuc = nuclides[k];
            let d = decay_library
                .try_match_nuclides_to_decay_data(nuc)
                .ok_or(ExpectedInventoryError::NotInLibrary(nuc))?;
            if d.is_unstable() {
                for branch in &d.decay_information {
                    if let Some(target) = branch.target {
                        if !nuclides.contains(&target) {
                            nuclides.push(target);
                        }
                    }
                }
            }
            data.push(d);
            k += 1;
        }

        let n = nuclides.len();
        let mut matrix = DepletionMatrix::zeros(n);
        for (j, d) in data.iter().enumerate() {
            let HalfLifeAndDecayEnergyInfo::Unstable(half_life, _) = d.half_life_information else {
                continue; // stable: column stays zero
            };
            let lambda = 2.0_f64.r_ln() / half_life.get::<second>();
            // `DepletionMatrix::set/add` only fail out of range; indices here
            // come from `nuclides` itself.
            let _ = matrix.set(j, j, -lambda);
            let mut remaining = 1.0_f64;
            for branch in &d.decay_information {
                let br = branch.branching_ratio.get::<ratio>();
                let p = br.min(remaining.max(0.0));
                remaining -= br;
                if let Some(target) = branch.target {
                    if let Some(i) = nuclides.iter().position(|&x| x == target) {
                        let _ = matrix.add(i, j, p * lambda);
                    }
                }
            }
        }
        Ok(Self { nuclides, matrix })
    }

    /// The tracked nuclides, in matrix order (`start` first).
    pub fn nuclides(&self) -> &[Nuclide] {
        &self.nuclides
    }

    /// The decay matrix, per second (parent by column, daughter by row).
    pub fn matrix(&self) -> &DepletionMatrix {
        &self.matrix
    }

    /// Index of `nuclide` in [`Self::nuclides`], if tracked.
    pub fn index_of(&self, nuclide: Nuclide) -> Option<usize> {
        self.nuclides.iter().position(|&x| x == nuclide)
    }

    /// The expected inventory `exp(A t) n0` by CRAM48. `n0` is in any amount
    /// unit (atoms, mol), one entry per [`Self::nuclides`].
    ///
    /// # Errors
    ///
    /// [`ExpectedInventoryError::LengthMismatch`],
    /// [`ExpectedInventoryError::BadTime`], or
    /// [`ExpectedInventoryError::Cram`].
    pub fn solve(&self, n0: &[f64], t: Time) -> Result<Vec<f64>, ExpectedInventoryError> {
        if n0.len() != self.nuclides.len() {
            return Err(ExpectedInventoryError::LengthMismatch {
                expected: self.nuclides.len(),
                found: n0.len(),
            });
        }
        let secs = t.get::<second>();
        if !secs.is_finite() || secs < 0.0 {
            return Err(ExpectedInventoryError::BadTime);
        }
        cram48(&self.matrix, n0, secs).map_err(ExpectedInventoryError::Cram)
    }

    /// [`Self::solve`] from one unit of the start nuclide.
    ///
    /// # Errors
    ///
    /// As [`Self::solve`].
    pub fn solve_from_start(&self, t: Time) -> Result<Vec<f64>, ExpectedInventoryError> {
        let mut n0 = vec![0.0; self.nuclides.len()];
        n0[0] = 1.0;
        self.solve(&n0, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lagrangian_decay_simulator::monte_carlo_single_radionuclide_decay_simulator::SingleNuclideSimulatorMC;
    use outram_mc_libs::rng::lcg::Lcg64 as Rand64;
    use petir::real::exp;
    use uom::si::time::{hour, year};

    fn lambda_of(lib: &DecayLibrary, nuc: Nuclide) -> f64 {
        match lib
            .try_match_nuclides_to_decay_data(nuc)
            .unwrap()
            .half_life_information
        {
            HalfLifeAndDecayEnergyInfo::Unstable(h, _) => 2.0_f64.r_ln() / h.get::<second>(),
            HalfLifeAndDecayEnergyInfo::Stable => 0.0,
        }
    }

    /// **Comparison (a), gh:#999: per-atom Monte Carlo mean vs the CRAM48
    /// expected inventory**, on the thorium-C end of the Th-232 chain.
    ///
    /// Methodology: `N = 20 000` atoms of Pb-212, each simulated by
    /// [`SingleNuclideSimulatorMC`] (the crate's per-atom decay simulator:
    /// it pre-samples a branch path and a lifetime per member) and advanced
    /// `t = 12 h` in one call; the nuclide each atom ends as is counted. The
    /// library's RNG is reseeded to a fixed `Lcg64` seed (`999`) first, so the
    /// run is deterministic (the decay simulator draws from `outram_mc_libs`'
    /// `Lcg64`, not `petir::rng`; gh:#995 moved only RustBCA's draws).
    /// Reference: [`ExpectedInventory::solve_from_start`] on the same
    /// library — Pb-212 -> Bi-212 -> {Po-212, Tl-208} -> Pb-208, ENDF/B-VIII.0
    /// half-lives and branchings, including Po-212's 0.3 us, which CRAM
    /// handles without a step limit.
    ///
    /// Bound, fixed before running: each atom ends in exactly one nuclide,
    /// independently, so the count of nuclide `i` is `Binomial(N, p_i)`. Pass
    /// if `|count_i / N - p_i| <= 4 sigma_i`, `sigma_i = sqrt(p_i (1 - p_i) / N)`,
    /// for every tracked nuclide with `p_i N >= 10` (below that the normal
    /// approximation is poor and the check is skipped for that nuclide and
    /// reported). With ~5 nuclides the family-wise false-alarm probability
    /// is <= 5 x 6.3e-5 = 3e-4 for a correct sampler.
    ///
    /// Measured 2026-10-20 (seed 999, 20 000 atoms):
    ///
    /// | nuclide | MC | CRAM48 | `|diff|` |
    /// |---|---|---|---|
    /// | Pb-212 | 0.46345 | 0.457607 | 1.66 sigma |
    /// | Bi-212 | 0.04875 | 0.0479227 | 0.55 sigma |
    /// | Tl-208 | 0.00075 (15 atoms) | 8.72574e-4 | 0.59 sigma |
    /// | Po-212 | 0 | 2.53e-12 | not gated (`p N < 10`) |
    /// | Pb-208 | 0.48705 | 0.493598 | 1.85 sigma |
    ///
    /// Worst 1.85 sigma, inside the 4-sigma bound. Pb-212 and Pb-208 deviate
    /// in opposite directions by the same count, as they must when they hold
    /// 95 % of the atoms between them.
    #[test]
    fn monte_carlo_mean_matches_cram48_expected_inventory() {
        let mut lib = DecayLibrary::new();
        lib.random_number_generator = Rand64::new(999);
        let start = Nuclide::Pb212;
        let inv = ExpectedInventory::from_library(start, &lib).unwrap();
        let t = Time::new::<hour>(12.0);
        let p = inv.solve_from_start(t).unwrap();

        let n_atoms = 20_000usize;
        let mut counts = vec![0usize; inv.nuclides().len()];
        for _ in 0..n_atoms {
            let mut sim = SingleNuclideSimulatorMC::new_decay_chain_simulation(start, &mut lib);
            let (end, _) = sim.advance_timestep(t);
            let i = inv
                .index_of(end)
                .expect("MC reached a nuclide the matrix lacks");
            counts[i] += 1;
        }

        let n = n_atoms as f64;
        let mut worst_sigmas = 0.0_f64;
        for (i, nuc) in inv.nuclides().iter().enumerate() {
            let phat = counts[i] as f64 / n;
            let sigma = (p[i] * (1.0 - p[i]) / n).sqrt();
            let z = if sigma > 0.0 {
                (phat - p[i]).abs() / sigma
            } else {
                0.0
            };
            println!(
                "{nuc:?}: MC {phat:.5} ({} atoms), CRAM48 {:.5e}, |diff| {:.2} sigma",
                counts[i], p[i], z
            );
            if p[i] * n >= 10.0 {
                worst_sigmas = worst_sigmas.max(z);
                assert!(
                    z <= 4.0,
                    "{nuc:?}: MC {phat} vs CRAM48 {} ({z:.2} sigma)",
                    p[i]
                );
            }
        }
        println!("worst |MC - CRAM48| = {worst_sigmas:.2} sigma over {n_atoms} atoms");
    }

    /// **Comparison (b), gh:#999: CRAM48 vs the exact Bateman answer** on the
    /// Th-232 chain built from the library.
    ///
    /// Methodology: the full Th-232 chain (Th-232 -> Ra-228 -> Ac-228 ->
    /// Th-228 -> Ra-224 -> ... -> Pb-208, branching at Bi-212) as one matrix,
    /// solved at `t = 1, 10, 100 a` from two starts:
    ///
    /// 1. one unit of **Th-232** (the stiffest use: half-lives 1.4e10 a down to
    ///    Po-212's 0.3 us), exact answer for Th-232, Ra-228, Ac-228, Th-228;
    /// 2. one unit of **Ra-228** (so the daughters carry resolvable shares,
    ///    Ac-228 ~1e-4), exact answer for Ra-228, Ac-228, Th-228, Ra-224.
    ///
    /// Each of those members is fed only from above, so it has the exact
    /// linear-chain Bateman solution on the library's own decay constants
    /// whatever happens downstream: `N_k(t) = N_0 prod_{m<k} lambda_m
    /// sum_i exp(-lambda_i t) / prod_{j!=i} (lambda_j - lambda_i)`. The
    /// constants in each head differ pairwise by factors >= 3, so the closed
    /// form is well conditioned. Also checked: total atoms conserved (every
    /// branch ends at stable Pb-208).
    ///
    /// Tolerances, fixed before running, **normwise** (absolute, on a unit
    /// inventory): `1e-13` per member and for the atom balance. CRAM48's
    /// rational error is below round-off; its round-off in 24 complex solves
    /// is `~1e-15` of the inventory, so 1e-13 leaves 100x margin. From
    /// Th-232 the daughters' shares are ~1e-10, below what a normwise gate
    /// resolves, so their **relative** errors are reported, not gated; start
    /// 2 is the one whose gate bites on the daughters (Ac-228's share ~1e-4
    /// makes 1e-13 absolute a 1e-9 relative check).
    ///
    /// Measured 2026-10-20: worst normwise **8.88e-16** (both starts, all
    /// times), atom balance <= 8.88e-16. From Ra-228 the daughters agree
    /// componentwise to <= 1.9e-15 relative. From Th-232 every daughter
    /// differs by exactly **1.40e-11 relative** at every time: that is not
    /// CRAM error but this test's reference, which uses Th-232's total
    /// `lambda` where the library feeds Ra-228 with the alpha branch,
    /// `0.999999999986` (the other 1.4e-11 is spontaneous fission written as
    /// Th-232 -> Th-232; see the module docs). The matrix follows the
    /// sampler; the hand-written reference ignores the branch. Normwise this
    /// is 6.5e-22, far inside the gate, so the gate (fixed before running)
    /// is unaffected.
    #[test]
    fn cram48_matches_bateman_on_the_th232_chain() {
        let lib = DecayLibrary::new();
        let inv = ExpectedInventory::from_library(Nuclide::Th232, &lib).unwrap();
        assert!(inv.index_of(Nuclide::Pb208).is_some());
        let heads = [
            [
                Nuclide::Th232,
                Nuclide::Ra228,
                Nuclide::Ac228,
                Nuclide::Th228,
            ],
            [
                Nuclide::Ra228,
                Nuclide::Ac228,
                Nuclide::Th228,
                Nuclide::Ra224,
            ],
        ];
        let mut worst_abs = 0.0_f64;
        for head in heads {
            let l: Vec<f64> = head.iter().map(|&nuc| lambda_of(&lib, nuc)).collect();
            let mut n0 = vec![0.0; inv.nuclides().len()];
            n0[inv.index_of(head[0]).unwrap()] = 1.0;
            for years in [1.0, 10.0, 100.0] {
                let t = Time::new::<year>(years);
                let ts = t.get::<second>();
                let c = inv.solve(&n0, t).unwrap();
                let bal = (c.iter().sum::<f64>() - 1.0).abs();
                for k in 0..head.len() {
                    let mut prod = 1.0;
                    for &lm in l.iter().take(k) {
                        prod *= lm;
                    }
                    let mut sum = 0.0;
                    for i in 0..=k {
                        let mut den = 1.0;
                        for j in 0..=k {
                            if j != i {
                                den *= l[j] - l[i];
                            }
                        }
                        sum += exp(-l[i] * ts) / den;
                    }
                    let exact = prod * sum;
                    let got = c[inv.index_of(head[k]).unwrap()];
                    let abs_err = (got - exact).abs();
                    worst_abs = worst_abs.max(abs_err);
                    println!(
                        "from {:?}, {years} a, {:?}: exact {exact:.6e}, CRAM48 abs {abs_err:.2e} rel {:.2e}",
                        head[0],
                        head[k],
                        abs_err / exact
                    );
                    assert!(
                        abs_err <= 1e-13,
                        "{:?} at {years} a: abs {abs_err}",
                        head[k]
                    );
                }
                println!("from {:?}, {years} a: atom balance {bal:.2e}", head[0]);
                assert!(bal <= 1e-13, "atom balance at {years} a: {bal}");
            }
        }
        println!("worst normwise CRAM48 vs Bateman: {worst_abs:.2e}");
    }

    /// The builder's edge cases: a stable start is a 1x1 zero matrix (the
    /// inventory does not move), and bad inputs are errors, not panics.
    #[test]
    fn stable_start_and_bad_inputs() {
        let lib = DecayLibrary::new();
        let inv = ExpectedInventory::from_library(Nuclide::Pb208, &lib).unwrap();
        assert_eq!(inv.nuclides(), &[Nuclide::Pb208]);
        assert_eq!(inv.matrix().order(), 1);
        assert_eq!(
            inv.solve_from_start(Time::new::<year>(1e9)).unwrap(),
            vec![1.0]
        );
        assert_eq!(
            inv.solve(&[1.0, 2.0], Time::new::<second>(1.0)),
            Err(ExpectedInventoryError::LengthMismatch {
                expected: 1,
                found: 2
            })
        );
        assert_eq!(
            inv.solve(&[1.0], Time::new::<second>(-1.0)),
            Err(ExpectedInventoryError::BadTime)
        );
    }
}
