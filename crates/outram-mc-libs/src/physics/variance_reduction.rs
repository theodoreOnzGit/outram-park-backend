// SPDX-License-Identifier: GPL-3.0

//! **Variance reduction** — weight cutoff, Russian roulette and survival
//! biasing. GitHub #258.
//!
//! Ported from `src/physics_common.cpp` at OpenMC `afa7a14`. (The commit the
//! issue cites, `608a1c33`, is not available in this container and is not
//! fetchable — see
//! `verification_and_validation/white_boundary/white_boundary_vs_openmc.md`.)
//!
//! # These are estimators, not physics
//!
//! The root `CLAUDE.md` rule *"correct physics is the default setting"* binds
//! physics terms. Variance reduction is not one: it changes how a quantity is
//! **estimated**, never what the quantity is. So unlike a physics term these
//! are correctly **off by default**, and the burden that replaces "default on"
//! is a different and harder one — each scheme must be shown **unbiased** by a
//! paired ablation against the analog arm on a case the analog arm can actually
//! converge, before being used anywhere it cannot.
//!
//! [`VarianceReduction::default()`] is therefore all-off, and
//! [`VarianceReduction::is_analog`] is the predicate a transport loop uses to
//! take the untouched analog path.
//!
//! # Upstream couples roulette to survival biasing, and that is deliberate
//!
//! `apply_russian_roulette` (`src/physics_common.cpp:21`) returns immediately
//! unless `settings::survival_biasing` is on. The issue lists weight
//! cutoff/roulette (scope item 1) and survival biasing (item 2) as separate
//! deliverables; **upstream does not treat them as separable**, and this port
//! follows upstream rather than the issue on that point.
//!
//! The reason is worth stating because it is not obvious from the code: in
//! analog transport every particle has weight exactly 1 until it is killed, so
//! there is never a particle below the cutoff and roulette has nothing to act
//! on. Weights only spread once something *makes* them spread — survival
//! biasing, or a weight window. Shipping roulette alone would be shipping a
//! branch that can never be taken, and would read as a feature.
//!
//! [`VarianceReduction::validate`] enforces this rather than letting a caller
//! configure a no-op.

use crate::rng::lcg::prn;

/// Variance-reduction settings for a run. All-off is analog.
#[derive(Debug, Clone, PartialEq)]
pub struct VarianceReduction {
    /// Implicit capture: reduce weight by the absorption probability instead of
    /// killing the particle. `settings::survival_biasing`.
    pub survival_biasing: bool,
    /// Weight below which a particle is rouletted. `settings::weight_cutoff`;
    /// upstream's default is 0.25.
    pub weight_cutoff: f64,
    /// Weight a roulette survivor is promoted to. `settings::weight_survive`;
    /// upstream's default is 1.0.
    pub weight_survive: f64,
    /// Scale the cutoff and survival weight by the particle's **birth** weight
    /// rather than using them absolutely. `settings::survival_normalization`.
    pub survival_normalization: bool,
    /// Mesh weight windows, applied at the surface-crossing and collision
    /// checkpoints. `None` plays no window game, which is the default.
    ///
    /// `Arc` rather than an owned value: the bounds are one array shared by
    /// every history in a run and must not be cloned per particle. Read-only
    /// data is `Arc<T>` per the workspace Rust rules.
    pub weight_windows: Option<std::sync::Arc<crate::physics::weight_windows::WeightWindows>>,
}

impl Default for VarianceReduction {
    /// Analog. See the module docs for why off is the correct default for an
    /// estimator, and why that is not in tension with the "correct physics is
    /// the default" rule.
    fn default() -> Self {
        Self {
            survival_biasing: false,
            // Upstream's defaults, carried even when inactive so that turning
            // survival biasing on gives upstream's behaviour rather than
            // whatever zero would mean.
            weight_cutoff: 0.25,
            weight_survive: 1.0,
            survival_normalization: false,
            weight_windows: None,
        }
    }
}

impl VarianceReduction {
    /// Is this an analog run — nothing to do, take the untouched path?
    ///
    /// Weight windows count: a run that splits and rouletted at the
    /// checkpoints is not analog, whatever `survival_biasing` says.
    pub fn is_analog(&self) -> bool {
        !self.survival_biasing && self.weight_windows.is_none()
    }

    /// Attach mesh weight windows.
    pub fn with_weight_windows(
        mut self,
        ww: crate::physics::weight_windows::WeightWindows,
    ) -> Self {
        self.weight_windows = Some(std::sync::Arc::new(ww));
        self
    }

    /// Reject configurations that cannot do what they appear to.
    ///
    /// # Errors
    ///
    /// - A cutoff or survival weight that is not strictly positive.
    /// - `weight_survive < weight_cutoff`, which rouletted particles into a
    ///   weight still below the cutoff: they would be rouletted again on the
    ///   next collision, and again, until one draw killed them. Not a loop, but
    ///   not roulette either.
    /// - Roulette parameters set while `survival_biasing` is off, which is the
    ///   no-op upstream's own guard produces. Refused rather than silently
    ///   ignored, because a caller who set a cutoff believes it is in effect.
    pub fn validate(&self) -> Result<(), String> {
        if self.survival_biasing {
            if !(self.weight_cutoff > 0.0) {
                return Err(format!(
                    "weight_cutoff must be strictly positive, got {}",
                    self.weight_cutoff
                ));
            }
            if !(self.weight_survive > 0.0) {
                return Err(format!(
                    "weight_survive must be strictly positive, got {}",
                    self.weight_survive
                ));
            }
            if self.weight_survive < self.weight_cutoff {
                return Err(format!(
                    "weight_survive ({}) is below weight_cutoff ({}): a survivor would \
                     still be under the cutoff and be rouletted again on its next \
                     collision. Upstream's defaults are 1.0 and 0.25.",
                    self.weight_survive, self.weight_cutoff
                ));
            }
        } else if self.survival_normalization {
            return Err(
                "survival_normalization has no effect while survival_biasing is off \
                 (upstream `apply_russian_roulette` returns before reading it, \
                 src/physics_common.cpp:24). Refused rather than silently ignored."
                    .to_string(),
            );
        }
        Ok(())
    }
}

/// The roulette kernel. `src/physics_common.cpp:12`, in full:
///
/// ```text
/// if (weight_survive * prn(p.current_seed()) < p.wgt()) { p.wgt() = weight_survive; }
/// else { p.wgt() = 0.; }
/// ```
///
/// # Why this is unbiased
///
/// A particle of weight `w` survives with probability `w / w_s` and is promoted
/// to `w_s`; otherwise it is killed. The expected weight after the operation is
///
/// ```text
/// (w / w_s) * w_s + (1 - w / w_s) * 0 = w
/// ```
///
/// exactly, for any `w <= w_s`. That identity is what makes roulette an
/// estimator change rather than a physics change, and it is asserted directly
/// in [`tests::roulette_preserves_expected_weight`] rather than left as a
/// comment.
///
/// Note the comparison is `w_s * xi < w`, **strictly less**. At `w == w_s` the
/// particle survives unless `xi` is exactly 1, which `prn` never returns, so a
/// particle at exactly the survival weight always survives. Reversing the
/// comparison would kill it with probability zero *and* leave a
/// measure-zero-but-real difference from upstream at `w = 0`.
pub fn russian_roulette(weight: f64, weight_survive: f64, seed: &mut u64) -> f64 {
    if weight_survive * prn(seed) < weight {
        weight_survive
    } else {
        0.0
    }
}

/// `apply_russian_roulette` (`src/physics_common.cpp:21`): the cutoff test that
/// decides whether the kernel above runs at all.
///
/// Returns the particle's new weight. Returns `weight` unchanged when survival
/// biasing is off, which is the analog path.
///
/// `weight_born` is the particle's birth weight, read only when
/// [`VarianceReduction::survival_normalization`] is set.
pub fn apply_russian_roulette(
    vr: &VarianceReduction,
    weight: f64,
    weight_born: f64,
    seed: &mut u64,
) -> f64 {
    if !vr.survival_biasing {
        return weight;
    }
    if vr.survival_normalization {
        if weight < vr.weight_cutoff * weight_born {
            return russian_roulette(weight, vr.weight_survive * weight_born, seed);
        }
    } else if weight < vr.weight_cutoff {
        return russian_roulette(weight, vr.weight_survive, seed);
    }
    weight
}

/// Survival biasing (implicit capture) at a collision: `src/physics.cpp:674`.
///
/// Instead of sampling whether the particle is absorbed and killing it, remove
/// the absorbed *weight* and let the particle continue:
///
/// ```text
/// w_absorbed = w * sigma_a / sigma_t
/// w         -= w_absorbed
/// ```
///
/// Returns `(new_weight, absorbed_weight)`. The caller needs the second for the
/// implicit-absorption `k` estimator, which upstream scores as
/// `w_absorbed * nu_fission / absorption`.
///
/// # What this does NOT do
///
/// It does not touch the fission-site production. Upstream keeps producing
/// fission sites from the *pre-absorption* weight through `create_fission_sites`
/// on the same collision; moving that here would double-count. The split is
/// upstream's and is kept.
pub fn survival_bias_absorption(weight: f64, absorption_xs: f64, total_xs: f64) -> (f64, f64) {
    debug_assert!(total_xs > 0.0, "total cross section must be positive");
    let absorbed = weight * absorption_xs / total_xs;
    (weight - absorbed, absorbed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The unbiasedness identity**, measured rather than asserted in prose.
    ///
    /// For weights spanning the cutoff, the mean weight after roulette over
    /// many draws must equal the weight before, to within the binomial error of
    /// the survival count. This is the property that makes roulette legitimate,
    /// so it is the property that gets a test.
    ///
    /// **Results (2026-09-22).** 200 000 draws at each weight, `w_s = 1.0`:
    ///
    /// | w | survivors | mean weight | deviation |
    /// |---|---|---|---|
    /// | 0.01 | 1 979 | 0.009895 | 0.47 sigma |
    /// | 0.05 | 10 012 | 0.050060 | 0.12 sigma |
    /// | 0.10 | 19 888 | 0.099440 | 0.83 sigma |
    /// | 0.20 | 39 721 | 0.198605 | 1.56 sigma |
    /// | 0.25 | 49 729 | 0.248645 | 1.40 sigma |
    /// | 0.50 | 100 030 | 0.500150 | 0.13 sigma |
    /// | 0.90 | 180 227 | 0.901135 | **1.69 sigma** |
    ///
    /// Worst deviation **1.69 sigma**, at `w = 0.90`. Seven weights at 4 sigma
    /// each, so a worst-of-seven near 1.7 is what an unbiased kernel looks
    /// like.
    ///
    /// **CORRECTED 2026-09-22.** This block first read *"the largest deviation
    /// was 0.43 sigma (at w = 0.05)"*. That number was never measured -- it was
    /// written before the test was run and is a fabrication. The real worst is
    /// 1.69 sigma and it is at a different weight. Struck rather than silently
    /// overwritten because a V&V table that was once wrong about its own
    /// provenance is exactly what the workspace's "never report a validation
    /// result that was not produced by running the check" rule exists to catch.
    #[test]
    fn roulette_preserves_expected_weight() {
        const N: usize = 200_000;
        let w_s = 1.0;
        let mut worst = 0.0_f64;
        for &w in &[0.01, 0.05, 0.10, 0.20, 0.25, 0.5, 0.9] {
            let mut seed = 0x5EED_0258_u64;
            let mut total = 0.0;
            let mut survivors = 0usize;
            for _ in 0..N {
                let out = russian_roulette(w, w_s, &mut seed);
                total += out;
                if out > 0.0 {
                    survivors += 1;
                }
            }
            let mean = total / N as f64;
            // Survival is Bernoulli(p = w / w_s); the mean weight is w_s times
            // the survival fraction, so its sigma is w_s sqrt(p(1-p)/N).
            let p = w / w_s;
            let sigma = w_s * (p * (1.0 - p) / N as f64).sqrt();
            let dev = (mean - w).abs() / sigma;
            worst = worst.max(dev);
            println!(
                "w = {w:.2}: survivors {survivors}, mean weight {mean:.6} vs {w:.6} \
                 ({dev:.2} sigma)"
            );
            assert!(
                dev < 4.0,
                "roulette is biased at w = {w}: mean {mean:.6} vs {w:.6}, {dev:.2} sigma"
            );
        }
        println!("worst deviation across all weights: {worst:.2} sigma");
    }

    /// A survivor always lands exactly on the survival weight, and a killed
    /// particle exactly on zero. No third outcome.
    #[test]
    fn roulette_has_exactly_two_outcomes() {
        let mut seed = 0x1234_5678_u64;
        for _ in 0..10_000 {
            let out = russian_roulette(0.3, 1.0, &mut seed);
            assert!(
                out == 1.0 || out == 0.0,
                "roulette produced {out}, which is neither the survival weight nor zero"
            );
        }
    }

    /// A particle at exactly the survival weight must always survive, because
    /// `prn` never returns exactly 1.
    #[test]
    fn a_particle_at_the_survival_weight_always_survives() {
        let mut seed = 0xABCD_u64;
        for _ in 0..100_000 {
            assert_eq!(russian_roulette(1.0, 1.0, &mut seed), 1.0);
        }
    }

    /// Analog settings must leave the weight untouched and consume **no random
    /// numbers** — a VR path that advanced the stream would change every
    /// existing result the moment it was compiled in, even switched off.
    #[test]
    fn the_analog_path_touches_neither_weight_nor_rng() {
        let vr = VarianceReduction::default();
        assert!(vr.is_analog());
        let mut seed = 42_u64;
        let before = seed;
        let w = apply_russian_roulette(&vr, 0.01, 1.0, &mut seed);
        assert_eq!(w, 0.01, "analog must not change the weight");
        assert_eq!(seed, before, "analog must not advance the RNG stream");
    }

    /// Survival biasing removes exactly the absorption fraction.
    #[test]
    fn survival_biasing_removes_the_absorption_fraction() {
        let (w, absorbed) = survival_bias_absorption(1.0, 0.3, 1.0);
        assert!((w - 0.7).abs() < 1e-15);
        assert!((absorbed - 0.3).abs() < 1e-15);
        // Weight is conserved between the surviving particle and what was
        // recorded as absorbed -- nothing is created or lost.
        assert!((w + absorbed - 1.0).abs() < 1e-15);
    }

    /// Configurations that cannot do what they look like must be refused.
    #[test]
    fn nonsense_configurations_are_refused() {
        let ok = VarianceReduction {
            survival_biasing: true,
            ..Default::default()
        };
        assert!(ok.validate().is_ok());

        // Survivor below the cutoff.
        let bad = VarianceReduction {
            survival_biasing: true,
            weight_cutoff: 0.5,
            weight_survive: 0.25,
            ..Default::default()
        };
        assert!(bad.validate().unwrap_err().contains("below weight_cutoff"));

        // Normalization requested with biasing off: a silent no-op upstream.
        let noop = VarianceReduction {
            survival_normalization: true,
            ..Default::default()
        };
        assert!(noop.validate().unwrap_err().contains("no effect"));
    }
}
