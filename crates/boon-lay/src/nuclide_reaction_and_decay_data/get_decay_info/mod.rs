use crate::prelude::{DecayType, NuclideReactionAndDecayData};
use super::DecayData;
use fission_yields_data::prelude::Nuclide;
use outram_mc_libs::rng::lcg::Lcg64 as Rand64;
use uom::si::{f64::*, ratio::ratio};

impl NuclideReactionAndDecayData {
    /// this obtains half life of the nuclide
    ///
    /// if stable, this returns none
    pub fn try_get_half_life(&self) -> Option<Time> {
        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => None,
            super::HalfLifeAndDecayEnergyInfo::Unstable(half_life, _decay_energy) => {
                Some(half_life)
            }
        }
    }
    /// checks whether the nuclide is stable
    #[inline]
    pub fn is_stable(&self) -> bool {
        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => true,
            super::HalfLifeAndDecayEnergyInfo::Unstable(_half_life, _decay_energy) => false,
        }
    }

    /// checks whether nuclide is unstable (just for readability sake)
    #[inline]
    pub fn is_unstable(&self) -> bool {
        // use the previous code lol, opposite of is_stable()
        return !self.is_stable();
    }

    /// this obtains decay energy of the nuclide
    ///
    /// if stable, this returns none
    pub fn get_decay_energy(&self) -> Option<Energy> {
        match self.half_life_information {
            super::HalfLifeAndDecayEnergyInfo::Stable => None,
            super::HalfLifeAndDecayEnergyInfo::Unstable(_half_life, decay_energy) => {
                Some(decay_energy)
            }
        }
    }

    /// Every decay branch as `(branching ratio, target, decay type)`, in the
    /// evaluation's order.
    ///
    /// The target is `None` where the chain file gives none (He-5's alpha
    /// branch). It is kept as written for spontaneous fission, where the
    /// OpenMC chain names the parent itself; [`DecayType::has_no_tracked_daughter`]
    /// says how to read it. ~~The body was `todo!()`~~ (implemented
    /// 2026-11-10, gh:#538; the target became an `Option` because He-5's is
    /// absent).
    pub fn get_decay_branch_info(&self) -> Vec<(Ratio, Option<Nuclide>, DecayType)> {
        self.decay_information
            .iter()
            .map(|d| (d.branching_ratio, d.target, d.decay_type))
            .collect()
    }

    /// Sample one decay of this nuclide from a uniform deviate `u` in `[0, 1)`.
    ///
    /// Follows how OpenMC's depletion chain reads the same data (upstream
    /// `openmc/deplete/chain.py`, `Chain.from_endf` and `Chain.form_matrix`,
    /// read 2026-11-10 at openmc-dev/openmc `develop`):
    ///
    /// - **Spontaneous fission** (`sf`, `ec/beta+,sf`): `form_matrix` adds no
    ///   daughter (`if target is not None and 'sf' not in decay_type`), so the
    ///   atom leaves the chain. The chain file's `target` (the parent itself)
    ///   is ignored. Returns [`DecaySample::NoTrackedDaughter`].
    /// - **No `target`** (He-5's alpha branch): the same, no heavy daughter.
    ///   (OpenMC also credits He-4 for each `alpha`; light products are not
    ///   tracked here.)
    /// - **Branching ratios not summing to exactly 1**: `from_endf` gives the
    ///   residual `1 - sum` to the largest branch. So a `u` past the running
    ///   sum selects the largest branch rather than falling through. With the
    ///   bundled libraries this is floating-point rounding only (no nuclide
    ///   has a sum below 1; pinned by `branching_ratios_sum_to_one_within_1e_9`).
    ///
    /// Every other draw is bit-identical to the original
    /// [`Self::get_next_target_nuclide_with_float`]: the same sequential
    /// subtraction, in the same order.
    ///
    /// # Panics
    /// If `u` is outside `[0, 1)`, a caller contract violation (unchanged).
    pub fn sample_decay_with_float(&self, mut u: f64) -> DecaySample {
        if u >= 1.0 || u < 0.0 {
            panic!("random number is not between 0 and 1");
        }
        let branches = &self.decay_information;
        if branches.is_empty() {
            return DecaySample::Stable;
        }
        if branches.len() == 1 {
            return DecaySample::from_branch(&branches[0]);
        }
        for branch in branches.iter() {
            let br = branch.branching_ratio.get::<ratio>();
            if u > br {
                u -= br;
            } else {
                return DecaySample::from_branch(branch);
            }
        }
        // `u` lies in the rounding gap past the running sum: OpenMC assigns
        // that residual to the largest branch (first one on a tie).
        let mut largest = &branches[0];
        for branch in branches.iter() {
            if branch.branching_ratio > largest.branching_ratio {
                largest = branch;
            }
        }
        DecaySample::from_branch(largest)
    }

    // get next target using oorandom pseudorandom number generator
    // this is done along with decay type
    // from oorandom
    ///
    /// `None` for a stable nuclide and, since 2026-11-10 (gh:#538), also for
    /// a decay with no tracked daughter (spontaneous fission, He-5's alpha),
    /// which ends a stochastic chain. Before, spontaneous fission returned
    /// the parent as its own daughter and He-5 panicked. Use
    /// [`Self::sample_decay_with_float`] to tell the cases apart.
    pub fn get_next_target_nuclide_with_rng(
        &self,
        rng: &mut Rand64,
    ) -> Option<(Nuclide, DecayType)> {
        // first let's use the rng to get a number between 0 and 1
        let random_num_between_0_and_1 = rng.rand_float();
        self.sample_decay_with_float(random_num_between_0_and_1)
            .daughter()
    }

    // get next target using oorandom pseudorandom number generator
    // this is done along with decay type
    // from oorandom
    ///
    /// `None` for a stable nuclide and, since 2026-11-10 (gh:#538), also for
    /// a decay with no tracked daughter; see
    /// [`Self::get_next_target_nuclide_with_rng`].
    pub fn get_next_target_nuclide_with_float(
        &self,
        random_num_between_0_and_1: f64,
    ) -> Option<(Nuclide, DecayType)> {
        self.sample_decay_with_float(random_num_between_0_and_1)
            .daughter()
    }
}

/// One sampled decay, from [`NuclideReactionAndDecayData::sample_decay_with_float`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecaySample {
    /// The nuclide has no decay branches.
    Stable,
    /// The atom becomes `nuclide`.
    Daughter {
        /// The daughter.
        nuclide: Nuclide,
        /// The branch taken.
        decay_type: DecayType,
    },
    /// The atom decays with no tracked heavy daughter (spontaneous fission,
    /// or a branch without a `target`), and leaves the chain (gh:#538).
    NoTrackedDaughter {
        /// The branch taken.
        decay_type: DecayType,
    },
}

impl DecaySample {
    fn from_branch(branch: &DecayData) -> Self {
        if branch.decay_type.has_no_tracked_daughter() {
            return DecaySample::NoTrackedDaughter {
                decay_type: branch.decay_type,
            };
        }
        match branch.target {
            Some(nuclide) => DecaySample::Daughter {
                nuclide,
                decay_type: branch.decay_type,
            },
            None => DecaySample::NoTrackedDaughter {
                decay_type: branch.decay_type,
            },
        }
    }

    /// The daughter and branch, or `None` if there is no tracked daughter.
    #[inline]
    pub fn daughter(self) -> Option<(Nuclide, DecayType)> {
        match self {
            DecaySample::Daughter {
                nuclide,
                decay_type,
            } => Some((nuclide, decay_type)),
            DecaySample::Stable | DecaySample::NoTrackedDaughter { .. } => None,
        }
    }
}

impl DecayType {
    /// Whether OpenMC's depletion chain gives this branch no heavy daughter:
    /// any mode containing `sf` (upstream `form_matrix`:
    /// `'sf' not in decay_type`), whatever `target` the chain file names.
    #[inline]
    pub fn has_no_tracked_daughter(self) -> bool {
        matches!(
            self,
            DecayType::SpontaneousFission | DecayType::ElectronCaptureBetaPlusAndSpontaneousFission
        )
    }
}

#[cfg(test)]
pub mod tests;
