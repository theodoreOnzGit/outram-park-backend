// SPDX-License-Identifier: GPL-3.0

//! **Iterated fission probability (IFP)** — adjoint-weighted `β_eff` and `Λ`.
//! GitHub #262 scope item 4.
//!
//! Ported from `src/ifp.cpp` and `include/openmc/ifp.h`, with the three
//! `SCORE_IFP_*` accumulations from `src/tallies/tally_scoring.cpp:939-1000`,
//! at OpenMC `afa7a14`.
//!
//! # Why this exists when the k-ratio route already gives a number
//!
//! `physics::kinetics` gives `β_eff ≈ 1 − k_p/k` from two eigenvalue solves.
//! That is the **prompt-`k`** definition and it is biased: it weights every
//! fission neutron equally, when what β_eff means is the delayed fraction
//! weighted by each neutron's **importance** — its probability of causing a
//! fission chain that survives.
//!
//! IFP measures that importance directly and without an adjoint solve. The
//! idea: follow a fission neutron's descendants for `N` generations. The
//! weight of fission produced in generation `N` **is** the neutron's
//! importance, because that is what importance means. So carry each fission
//! site's lineage — was its `N`-generations-ago ancestor delayed, and how long
//! did that ancestor live — and score against it.
//!
//! # The three scores, and what they divide into
//!
//! ```text
//! beta_eff = ifp-beta-numerator / ifp-denominator
//! Lambda   = ifp-time-numerator / ifp-denominator
//! ```
//!
//! The denominator is the total weight of fissions whose lineage is `N`
//! generations deep. The numerators are the same weight, restricted to
//! delayed ancestors (β) or multiplied by the ancestor's lifetime (Λ).
//!
//! # The cost, and the bias that replaces the one it removes
//!
//! Every fission site carries an `N`-entry lineage, so the fission bank grows
//! by `N` numbers per site. More importantly, **no fission scores anything
//! until generation `N`**: the first `N` active generations produce a
//! denominator of exactly zero. A run with fewer than `N` active generations
//! returns `0/0`, and [`IfpTallies::beta_eff`] refuses rather than returning a
//! NaN that propagates.
//!
//! `N` is a convergence parameter, not a free choice. Too small and the
//! importance is not converged — the answer is somewhere between the
//! bare delayed fraction (`N = 0`) and the true adjoint-weighted one. Upstream
//! defaults to 10. [`IfpSettings::n_generation`] carries it on the result so a
//! β_eff cannot be quoted without it.

/// Which lineage data to carry — `settings::ifp_delayed_group_on` and
/// `ifp_lifetime_on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfpSettings {
    /// Generations of lineage to carry. Upstream's default is 10.
    pub n_generation: usize,
    /// Carry the ancestor's delayed group, for `β_eff`.
    pub delayed_group: bool,
    /// Carry the ancestor's lifetime, for `Λ`.
    pub lifetime: bool,
}

impl Default for IfpSettings {
    /// Upstream's defaults, with both quantities on: carrying one and not the
    /// other saves little and gives you half a kinetics answer.
    fn default() -> Self {
        Self {
            n_generation: 10,
            delayed_group: true,
            lifetime: true,
        }
    }
}

impl IfpSettings {
    /// Whether IFP is doing anything at all.
    pub fn is_on(&self) -> bool {
        self.n_generation > 0 && (self.delayed_group || self.lifetime)
    }
}

/// One fission site's ancestry — the IFP banks, per site.
///
/// `delayed_groups[0]` and `lifetimes[0]` are the **oldest** entry, i.e. the
/// ancestor `n_generation` generations back once the list is full. That
/// ordering is what makes the score a single index rather than a search, and
/// it is why [`Self::push`] shifts left rather than appending.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IfpLineage {
    /// Delayed group of each ancestor, 0 for prompt. Oldest first.
    pub delayed_groups: Vec<usize>,
    /// Lifetime of each ancestor \[s\], as fixed-point microseconds so the
    /// type can stay `Eq`; see [`Self::lifetimes_seconds`].
    lifetimes_ns: Vec<u64>,
}

impl IfpLineage {
    /// Extend the lineage with one generation — `_ifp`
    /// (`include/openmc/ifp.h:38`).
    ///
    /// Below `n_generation` entries the value is appended; **at**
    /// `n_generation` the list shifts left and the new value goes last, so the
    /// window always holds the most recent `n_generation` ancestors with the
    /// oldest at index 0.
    pub fn push(
        &self,
        settings: &IfpSettings,
        delayed_group: usize,
        lifetime_s: f64,
    ) -> Self {
        let shift = |v: &Vec<u64>, new: u64| -> Vec<u64> {
            let mut out = v.clone();
            if out.len() < settings.n_generation {
                out.push(new);
            } else {
                out.remove(0);
                out.push(new);
            }
            out
        };
        let shift_us = |v: &Vec<usize>, new: usize| -> Vec<usize> {
            let mut out = v.clone();
            if out.len() < settings.n_generation {
                out.push(new);
            } else {
                out.remove(0);
                out.push(new);
            }
            out
        };
        Self {
            delayed_groups: if settings.delayed_group {
                shift_us(&self.delayed_groups, delayed_group)
            } else {
                Vec::new()
            },
            lifetimes_ns: if settings.lifetime {
                shift(&self.lifetimes_ns, (lifetime_s * 1.0e9).round() as u64)
            } else {
                Vec::new()
            },
        }
    }

    /// How many generations of ancestry this site carries.
    pub fn depth(&self) -> usize {
        self.delayed_groups.len().max(self.lifetimes_ns.len())
    }

    /// Whether the lineage is deep enough to score — upstream's
    /// `size() == settings::ifp_n_generation` guard.
    pub fn is_converged(&self, settings: &IfpSettings) -> bool {
        self.depth() == settings.n_generation
    }

    /// Ancestor lifetimes in seconds, oldest first.
    pub fn lifetimes_seconds(&self) -> Vec<f64> {
        self.lifetimes_ns.iter().map(|&n| n as f64 * 1.0e-9).collect()
    }

    /// The `n_generation`-back ancestor's delayed group, or `None` until the
    /// lineage is deep enough.
    pub fn oldest_delayed_group(&self, settings: &IfpSettings) -> Option<usize> {
        self.is_converged(settings)
            .then(|| self.delayed_groups.first().copied())
            .flatten()
    }

    /// The `n_generation`-back ancestor's lifetime \[s\], or `None` until the
    /// lineage is deep enough.
    pub fn oldest_lifetime(&self, settings: &IfpSettings) -> Option<f64> {
        self.is_converged(settings)
            .then(|| self.lifetimes_ns.first().map(|&n| n as f64 * 1.0e-9))
            .flatten()
    }
}

/// The three IFP scores, accumulated over a run —
/// `SCORE_IFP_BETA_NUM`, `SCORE_IFP_TIME_NUM`, `SCORE_IFP_DENOM`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IfpTallies {
    /// Weight of fissions whose `n_generation`-back ancestor was **delayed**.
    pub beta_numerator: f64,
    /// Weight of fissions times their ancestor's lifetime \[s\].
    pub time_numerator: f64,
    /// Weight of all fissions with a converged lineage.
    pub denominator: f64,
    /// Fissions seen whose lineage was **not** deep enough to score. Carried
    /// because a large count against a small denominator is the signature of
    /// a run too short for its `n_generation`.
    pub skipped_shallow: usize,
}

impl IfpTallies {
    /// Score one fission — the three `SCORE_IFP_*` branches in one place,
    /// because they share the same guard and scoring them separately is how
    /// they drift apart.
    pub fn score_fission(
        &mut self,
        settings: &IfpSettings,
        lineage: &IfpLineage,
        weight: f64,
    ) {
        if !settings.is_on() {
            return;
        }
        if !lineage.is_converged(settings) {
            self.skipped_shallow += 1;
            return;
        }
        self.denominator += weight;
        if let Some(g) = lineage.oldest_delayed_group(settings) {
            if g > 0 {
                self.beta_numerator += weight;
            }
        }
        if let Some(t) = lineage.oldest_lifetime(settings) {
            self.time_numerator += t * weight;
        }
    }

    /// Adjoint-weighted `β_eff`.
    ///
    /// # Errors
    ///
    /// A zero denominator. That is not "β_eff is zero" — it means **no
    /// fission ever had a lineage `n_generation` deep**, i.e. the run had
    /// fewer than `n_generation` active generations, or IFP was off. Returning
    /// `0/0` would give a NaN that propagates into a kinetics model.
    pub fn beta_eff(&self, settings: &IfpSettings) -> Result<f64, String> {
        if !settings.delayed_group {
            return Err(
                "this run carried no ancestor delayed groups \
                 (`IfpSettings::delayed_group` is false), so the beta numerator is \
                 identically zero. That is a configuration, not a beta_eff of zero."
                    .into(),
            );
        }
        if self.denominator <= 0.0 {
            return Err(format!(
                "the IFP denominator is {}, with {} fissions skipped for a shallow \
                 lineage. No fission reached a lineage {} generations deep, so the run \
                 was shorter than n_generation (or IFP was off). This is NOT a beta_eff \
                 of zero.",
                self.denominator, self.skipped_shallow, settings.n_generation
            ));
        }
        Ok(self.beta_numerator / self.denominator)
    }

    /// Adjoint-weighted generation time `Λ` \[s\].
    ///
    /// # Errors
    ///
    /// As [`Self::beta_eff`].
    pub fn generation_time(&self, settings: &IfpSettings) -> Result<f64, String> {
        if !settings.lifetime {
            return Err(
                "this run carried no ancestor lifetimes (`IfpSettings::lifetime` is \
                 false), so the time numerator is identically zero. That is a \
                 configuration, not a generation time of zero."
                    .into(),
            );
        }
        if self.denominator <= 0.0 {
            return Err(format!(
                "the IFP denominator is {}, with {} fissions skipped for a shallow \
                 lineage; no fission reached {} generations deep. See `beta_eff`.",
                self.denominator, self.skipped_shallow, settings.n_generation
            ));
        }
        Ok(self.time_numerator / self.denominator)
    }

    /// Fraction of fissions that actually scored.
    ///
    /// Below ~0.5 the run is spending most of its histories filling lineages
    /// rather than measuring, which is the signal to raise the generation
    /// count rather than the history count.
    pub fn scoring_fraction(&self) -> f64 {
        let scored = if self.denominator > 0.0 { 1.0 } else { 0.0 };
        let total = self.skipped_shallow as f64 + scored;
        if total > 0.0 {
            scored / total
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(n: usize) -> IfpSettings {
        IfpSettings {
            n_generation: n,
            ..IfpSettings::default()
        }
    }

    /// **The lineage window slides, oldest first.** Index 0 must always be the
    /// `n_generation`-back ancestor, because that is what the score reads.
    #[test]
    fn the_lineage_window_slides_and_keeps_the_oldest_first() {
        let s = settings(3);
        let mut l = IfpLineage::default();
        assert!(!l.is_converged(&s));

        // Fill: groups 1, 2, 3.
        for g in 1..=3 {
            l = l.push(&s, g, g as f64 * 1.0e-6);
        }
        assert_eq!(l.delayed_groups, vec![1, 2, 3]);
        assert!(l.is_converged(&s));
        assert_eq!(l.oldest_delayed_group(&s), Some(1));
        assert!((l.oldest_lifetime(&s).unwrap() - 1.0e-6).abs() < 1e-15);

        // One more: 1 falls off the front.
        l = l.push(&s, 4, 4.0e-6);
        assert_eq!(l.delayed_groups, vec![2, 3, 4]);
        assert_eq!(l.oldest_delayed_group(&s), Some(2));
        assert!((l.oldest_lifetime(&s).unwrap() - 2.0e-6).abs() < 1e-15);
        assert_eq!(l.depth(), 3, "the window never grows past n_generation");
    }

    /// Nothing scores until the lineage is deep enough, and the shallow ones
    /// are counted rather than silently dropped.
    #[test]
    fn nothing_scores_until_the_lineage_is_deep_enough() {
        let s = settings(4);
        let mut t = IfpTallies::default();
        let mut l = IfpLineage::default();
        for g in 1..=3 {
            l = l.push(&s, g, 1.0e-6);
            t.score_fission(&s, &l, 1.0);
        }
        assert_eq!(t.denominator, 0.0);
        assert_eq!(t.skipped_shallow, 3);

        l = l.push(&s, 0, 1.0e-6);
        t.score_fission(&s, &l, 1.0);
        assert_eq!(t.denominator, 1.0);
    }

    /// **A zero denominator is refused, not reported as beta_eff = 0.**
    ///
    /// It means no fission reached the required lineage depth — the run was
    /// shorter than `n_generation`. Returning `0/0` gives a NaN that
    /// propagates into a kinetics model.
    #[test]
    fn a_zero_denominator_is_refused_rather_than_returning_nan() {
        let s = settings(10);
        let t = IfpTallies {
            skipped_shallow: 5000,
            ..IfpTallies::default()
        };
        let err = t.beta_eff(&s).unwrap_err();
        assert!(err.contains("NOT a beta_eff of zero"), "{err}");
        assert!(err.contains("5000"), "the skipped count must be reported: {err}");
        assert!(t.generation_time(&s).is_err());
    }

    /// **beta_eff is the delayed share of the CONVERGED lineages.**
    ///
    /// Three fissions score, one of them with a delayed ancestor, so
    /// `beta = 1/3` exactly — and the prompt ones must not contribute to the
    /// numerator however much weight they carry.
    #[test]
    fn beta_eff_is_the_delayed_share_of_the_converged_lineages() {
        let s = settings(2);
        let mut t = IfpTallies::default();
        let base = IfpLineage::default().push(&s, 0, 1.0e-7);

        // Ancestor prompt (group 0).
        let prompt = base.push(&s, 0, 2.0e-7);
        t.score_fission(&s, &prompt, 1.0);
        t.score_fission(&s, &prompt, 1.0);
        // Ancestor delayed (group 3): its OLDEST entry must be the delayed one.
        let delayed = IfpLineage::default().push(&s, 3, 5.0e-7).push(&s, 0, 1.0e-7);
        t.score_fission(&s, &delayed, 1.0);

        assert_eq!(t.denominator, 3.0);
        assert_eq!(t.beta_numerator, 1.0);
        let beta = t.beta_eff(&s).unwrap();
        assert!((beta - 1.0 / 3.0).abs() < 1e-15, "beta = {beta}");
    }

    /// Λ is the weight-averaged ancestor lifetime.
    #[test]
    fn lambda_is_the_weighted_mean_ancestor_lifetime() {
        let s = settings(1);
        let mut t = IfpTallies::default();
        // Lineage depth 1, so the oldest entry is the only entry.
        let a = IfpLineage::default().push(&s, 0, 4.0e-6);
        let b = IfpLineage::default().push(&s, 0, 8.0e-6);
        t.score_fission(&s, &a, 3.0);
        t.score_fission(&s, &b, 1.0);
        // (3*4 + 1*8) / 4 = 5 us.
        let l = t.generation_time(&s).unwrap();
        assert!((l - 5.0e-6).abs() < 1e-12, "Lambda = {l}");
    }

    /// Turning IFP off makes every score a no-op, including the skip counter —
    /// so an "off" run cannot be mistaken for one that skipped everything.
    #[test]
    fn ifp_off_scores_nothing_and_counts_nothing() {
        let s = IfpSettings {
            n_generation: 0,
            ..IfpSettings::default()
        };
        assert!(!s.is_on());
        let mut t = IfpTallies::default();
        t.score_fission(&s, &IfpLineage::default(), 1.0);
        assert_eq!(t.denominator, 0.0);
        assert_eq!(
            t.skipped_shallow, 0,
            "an IFP-off run must not look like one that skipped every fission"
        );
    }

    /// Carrying only one of the two quantities works, and the other is empty.
    #[test]
    fn either_quantity_can_be_carried_alone() {
        let s = IfpSettings {
            n_generation: 2,
            delayed_group: true,
            lifetime: false,
        };
        let l = IfpLineage::default().push(&s, 1, 1.0e-6).push(&s, 2, 2.0e-6);
        assert_eq!(l.delayed_groups, vec![1, 2]);
        assert!(l.lifetimes_seconds().is_empty());
        assert!(l.is_converged(&s));
        assert_eq!(l.oldest_delayed_group(&s), Some(1));
        assert_eq!(l.oldest_lifetime(&s), None);

        // And asking for the quantity that was NOT carried is an error rather
        // than a zero: a Lambda of zero from a run that never recorded a
        // lifetime is a configuration, not a measurement.
        let mut t = IfpTallies::default();
        t.score_fission(&s, &l, 1.0);
        assert!(t.beta_eff(&s).is_ok());
        let err = t.generation_time(&s).unwrap_err();
        assert!(err.contains("configuration, not a generation time"), "{err}");
    }
}
