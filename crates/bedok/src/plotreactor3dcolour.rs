//! The scaled power map behind the 3-D power-density plot.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `plotreactor3dcolour.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What is and is not translated
//!
//! The reference does two things: it **computes a scaled power map**, then it
//! **renders that map as a MATLAB figure** — building `fill3` patch vertices for
//! each node, mirroring them into four quadrants, attaching a colour bar and
//! writing `pwrdens3d.jpg`.
//!
//! Only the first half is translated. The rendering half emits a MATLAB figure
//! and has no library equivalent; reproducing it would mean choosing a plotting
//! stack and writing files as a side effect, which this crate deliberately does
//! not do (see the flux solvers' `Diagnostics`, and the CSV policy in the crate
//! README). [`scaled_power`] returns the same quantity the figure colours by, so
//! a caller can render it however they like.
//!
//! **What that omits, concretely:** the quadrant mirroring, the patch geometry,
//! and the `PWRlin` 256-step colour scale. None of it changes a number; all of
//! it is presentation.
//!
//! # Two defects in the half that IS translated
//!
//! Both are pinned by tests and neither is repaired.
//!
//! **P1 — a one-group case gets an all-zero map.** The group collapse sits
//! entirely inside `if params.G > 1`, and `pwrdensG` is preallocated to zeros.
//! At `G == 1` nothing ever writes it, so every node plots as zero power. The
//! reference's only call site passes `G = 2`, so it has never been seen.
//!
//! **P2 — for more than two groups the collapse overwrites instead of
//! accumulating.** The loop body is
//!
//! ```text
//! pwrdensG(1:es) = pwrdens(1:es) + pwrdens((g-1)*es+1 : g*es)
//! ```
//!
//! which **assigns** rather than adding into `pwrdensG`. After the loop only
//! group 1 plus the *last* group survive; groups 2 to `G-1` are silently
//! dropped. Correct at `G == 2` — the value every case in the snapshot uses —
//! and wrong for anything larger.
//!
//! # The normalisation divides by the ungrouped total
//!
//! `scaledpwr = pwrdensG / sum(pwrdens) ./ Vi`. Note the denominator is the sum
//! over the **whole** `pwrdens` vector, all groups, while the numerator is the
//! collapsed map. That is not a defect — it makes the map a fraction of total
//! core power per unit volume — but it does mean the map does not sum to 1.

use crate::types::{Geometry, Params};

/// The scaled power map, and what the collapse did to get there.
#[derive(Clone, Debug)]
pub struct ScaledPower {
    /// `scaledpwr` — power per unit volume as a fraction of the core total,
    /// one entry per node.
    pub scaled: Vec<f64>,
    /// `pwrdensG` — the group-collapsed node power, before dividing by volume.
    pub collapsed: Vec<f64>,
    /// `PWRHIGH` — the map's maximum, which sets the colour scale.
    pub peak: f64,
    /// Whether defect P1 fired: a one-group case, so `collapsed` is all zeros.
    pub all_zero_from_single_group: bool,
    /// How many groups defect P2 silently dropped.
    ///
    /// `0` for `G <= 2`; `G - 2` for anything larger, because only group 1 and
    /// the last group survive the overwriting loop.
    pub groups_dropped: usize,
}

/// The scaled power map `plotreactor3dcolour.m` colours its figure by.
///
/// # Arguments
///
/// - `pwrdens` — `results.pwrdens`, length `G * es`.
/// - `geometry` — needs `Vi`.
///
/// # Panics
///
/// If `pwrdens` is shorter than `G * es`.
pub fn scaled_power(params: &Params, geometry: &Geometry, pwrdens: &[f64]) -> ScaledPower {
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let g_count = params.g;
    assert!(
        pwrdens.len() >= g_count * es,
        "pwrdens is {} long, expected at least {}",
        pwrdens.len(),
        g_count * es
    );

    // The group collapse, reproduced with both defects intact.
    let mut collapsed = vec![0.0; es];
    if g_count > 1 {
        for g in 1..g_count {
            for (i, c) in collapsed.iter_mut().enumerate() {
                // **Assignment, not accumulation** — defect P2.
                *c = pwrdens[i] + pwrdens[g * es + i];
            }
        }
    }

    // The denominator is the sum over every group, not over `collapsed`.
    let total: f64 = pwrdens.iter().sum();
    let scaled: Vec<f64> = collapsed
        .iter()
        .zip(&geometry.vi)
        .map(|(p, v)| p / total / v)
        .collect();

    let peak = scaled.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    ScaledPower {
        all_zero_from_single_group: g_count == 1,
        groups_dropped: g_count.saturating_sub(2),
        collapsed,
        scaled,
        peak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(g_count: usize) -> (Params, Geometry, Vec<f64>) {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(1),
            maxiz: Some(1),
            g: g_count,
            ..Default::default()
        };
        let es = 2;
        let geometry = Geometry {
            vi: vec![2.0; es],
            ..Default::default()
        };
        // Group g contributes 10^g per node, so a dropped group is obvious.
        let pwrdens: Vec<f64> = (0..g_count)
            .flat_map(|g| (0..es).map(move |_| 10f64.powi(g as i32 + 1)))
            .collect();
        (params, geometry, pwrdens)
    }

    /// Two groups — the case every benchmark uses — collapse correctly.
    ///
    /// # Methodology
    ///
    /// At `G = 2` the loop runs once and its assignment is equivalent to the
    /// accumulation that was presumably meant, so the result is right. This
    /// establishes the baseline the two defects depart from.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `collapsed = [110, 110]` from groups of 10 and 100; scaled to
    /// `[0.25, 0.25]` against a total of 220 and a cell volume of 2.
    ///
    /// **Interpretation.** At `G = 2` the assignment and the intended
    /// accumulation coincide, which is why both defects below have gone
    /// unnoticed: every case in the snapshot is two-group.
    #[test]
    fn two_groups_collapse_correctly() {
        let (params, geometry, pwrdens) = setup(2);
        let out = scaled_power(&params, &geometry, &pwrdens);

        eprintln!("pwrdens  = {pwrdens:?}");
        eprintln!("collapsed = {:?} (expect 10 + 100)", out.collapsed);
        eprintln!("scaled    = {:?}", out.scaled);
        eprintln!("peak      = {}", out.peak);

        assert_eq!(out.collapsed, vec![110.0, 110.0]);
        assert_eq!(out.groups_dropped, 0);
        assert!(!out.all_zero_from_single_group);
        // total = 2*10 + 2*100 = 220; scaled = 110/220/2 = 0.25
        assert_eq!(out.scaled, vec![0.25, 0.25]);
        assert_eq!(out.peak, 0.25);
    }

    /// **Defect P1: a one-group case produces an all-zero map.**
    ///
    /// # Methodology
    ///
    /// `pwrdensG` is preallocated to zeros and only ever written inside
    /// `if params.G > 1`. At `G = 1` that branch never runs, so the map stays
    /// zero however much power the case is producing. Pinned rather than
    /// repaired.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Input power `[10, 10]`; output map **`[0, 0]`**, peak 0.
    ///
    /// **Interpretation.** Defect P1 confirmed. A one-group case would
    /// render an entirely blank figure while the core is producing
    /// power — a silent failure, since a blank plot looks like a
    /// plotting problem rather than a dropped collapse.
    #[test]
    fn one_group_yields_an_all_zero_map() {
        let (params, geometry, pwrdens) = setup(1);
        let out = scaled_power(&params, &geometry, &pwrdens);

        eprintln!("pwrdens  = {pwrdens:?} (real power!)");
        eprintln!("collapsed = {:?} (defect P1)", out.collapsed);
        eprintln!("peak      = {}", out.peak);

        assert!(pwrdens.iter().all(|p| *p > 0.0), "the input has real power");
        assert_eq!(out.collapsed, vec![0.0, 0.0], "but the map is empty");
        assert_eq!(out.scaled, vec![0.0, 0.0]);
        assert_eq!(out.peak, 0.0);
        assert!(out.all_zero_from_single_group, "the flag must surface it");
    }

    /// **Defect P2: more than two groups silently drops the middle ones.**
    ///
    /// # Methodology
    ///
    /// With four groups contributing 10, 100, 1000 and 10000 per node, a
    /// correct collapse would give 11110. The reference's assignment leaves
    /// only group 1 plus the **last** group — 10010 — dropping groups 2 and 3.
    /// The `groups_dropped` field reports how many went missing.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Groups of 10, 100, 1000 and 10000 collapse to **10010**, not the
    /// correct **11110**. Groups 2 and 3 vanish.
    ///
    /// **Interpretation.** Defect P2 confirmed, and it is not a small
    /// error: 90% of the power is missing here. Group 1 plus the *last*
    /// group survive because each pass overwrites the previous one.
    #[test]
    fn more_than_two_groups_drops_the_middle_ones() {
        let (params, geometry, pwrdens) = setup(4);
        let out = scaled_power(&params, &geometry, &pwrdens);

        let correct: f64 = 10.0 + 100.0 + 1000.0 + 10000.0;
        eprintln!("a correct collapse would give {correct}");
        eprintln!("the reference gives          {:?}", out.collapsed);
        eprintln!("groups dropped               {}", out.groups_dropped);

        // Group 1 + group 4 only.
        assert_eq!(out.collapsed, vec![10010.0, 10010.0]);
        assert_ne!(out.collapsed[0], correct);
        assert_eq!(out.groups_dropped, 2, "groups 2 and 3 vanished");
    }
}
