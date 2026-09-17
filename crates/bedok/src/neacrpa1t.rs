//! NEACRP case A1 — central control-assembly ejection at **hot zero power**.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `neacrpa1t.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # The transient
//!
//! NEACRP-L-335 (Revision 1), Figure 3.1. Same core as [`crate::neacrpa2`], but
//! at **hot zero power**: a 2775 W core (693.75 W in the modelled quarter) with
//! the coolant at 286 C. The central control assembly is ejected from **fully
//! inserted** to fully withdrawn in 0.1 s, and the transient is followed for
//! 5 s.
//!
//! # Why this is the harder of the two ejections
//!
//! At full power (case A2) the ejected rod is worth roughly a dollar and the
//! Doppler feedback of an already-hot core damps the excursion. At HZP the fuel
//! starts in equilibrium with the coolant — `fueltempavg` is the coolant
//! temperature, not 891 K — so there is **no stored Doppler margin**, and the
//! rod is being pulled from full insertion rather than half. The reference's
//! own note records the consequence:
//!
//! > the time grid uses 1 ms steps over the super-prompt-critical power spike
//! > (~0.1-0.5 s); the spike spans **several decades of power**.
//!
//! That is why the grid is 3.5x denser than A2's, and it
//! is the regime [`crate::thdiffusion_solvertimexyz`]'s `freqmode` note is
//! about: per-node exponential-transform frequencies are unstable in
//! super-prompt ejections, so [`crate::types::FreqMode::Global`] is the default.
//!
//! # The rod pattern is nearly all-in
//!
//! Figure 3.1: banks 1, 2, 3, 5, 6 and 7 **fully inserted** (0 steps), bank 4
//! fully withdrawn (228). Case A2's pattern is a partial insertion by
//! comparison. This is the configuration the reference describes as
//! "heavily-rodded", and the one
//! [`crate::criticalboron_xyz`]'s cold-start warnings were written against.
//!
//! # A second published number, and a second disagreement
//!
//! | | ppm |
//! |---|---|
//! | this code (frozen-T-H secant + coupled verification) | 551.31 |
//! | benchmark (PANTHER, NEA/NSC/DOC(93)25 Tab 3.1) | **567.7** |
//!
//! **-16.39 ppm, about -2.9%** — the same *direction* as case A2's -21.6 ppm,
//! and a similar relative size. Two independent cases disagreeing the same way
//! is more informative than either alone, and it is recorded here for that
//! reason.
//!
//! As with A2 the search that produced 551.31 is not reproducible from this
//! snapshot: the comment cites `test_critboron2.m`, which was not shipped.
//!
//! The reference also notes what happens if the boron is left at A2's value:
//!
//! > At e.g. 1000 ppm the core is ~4200 pcm subcritical and the ejected rod is
//! > no longer ~1$ (sub-prompt transient).
//!
//! So the boron here is not a tuning knob — get it wrong and the case stops
//! being the transient the benchmark specifies.

use crate::neacrpa2t::neacrpa2t;
use crate::sigmavalupd3d_handler::FeedbackTables;
use crate::types::{Geometry, Params, SigmaValues, Th};

/// The critical boron concentration this code computes for case A1, ppm.
///
/// From `neacrpa1t.m`: a frozen-T-H secant plus coupled verification giving
/// `k_eff = 0.999990`. Compare [`BENCHMARK_CRITICAL_BORON`].
pub const CRITICAL_BORON: f64 = 551.31;

/// The **published** critical boron concentration for case A1, ppm.
///
/// PANTHER, NEA/NSC/DOC(93)25 Table 3.1, as quoted by `neacrpa1t.m`'s comment.
/// **Quoted from that comment, not from a primary publication checked here.**
pub const BENCHMARK_CRITICAL_BORON: f64 = 567.7;

/// The HZP power ratio: 2775 W core, 693.75 W in the modelled quarter.
///
/// Applied to [`crate::neacrpa2`]'s 693.75 MW, so `1e-6` gives 693.75 W.
pub const HZP_POWER_RATIO: f64 = 1e-6;

/// The HZP fuel temperature, K — in equilibrium with the coolant.
pub const HZP_FUEL_TEMP: f64 = 559.15;

/// `[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa1t(params)`.
///
/// Builds [`crate::neacrpa2t`] — which shares the kinetics data, heat
/// capacities and ejection duration — and overrides the five things case A1
/// changes: the time grid, the power ratio, the boron, the initial fuel
/// temperature and the rod pattern.
///
/// The reference's own header says it is "based on `neacrpa2t.m`", and diffing
/// the two confirms those five are the only substantive differences.
///
/// # Returns
///
/// `(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
/// [`crate::neacrpa2::neacrpa2`].
#[allow(clippy::type_complexity)]
pub fn neacrpa1t(
    params: &Params,
) -> (Params, Geometry, Th, crate::matlab::Array3<usize>, SigmaValues, FeedbackTables) {
    let (mut params, mut geometry, mut th, whichsigma, sigmavalues, feedback) = neacrpa2t(params);

    // ----- hot zero power -----
    th.powratio = HZP_POWER_RATIO;
    params.fueltempavg = HZP_FUEL_TEMP;
    params.boron = CRITICAL_BORON;

    // ----- Figure 3.1: banks 1,2,3,5,6,7 in; bank 4 out -----
    geometry.crod = vec![0.0, 0.0, 0.0, 228.0, 0.0, 0.0, 0.0];

    // ----- a finer grid through the super-prompt spike -----
    // `[0:0.001:0.6, 0.6:0.005:1, 1:0.025:5, 5]` — 1 ms steps to 0.6 s.
    let mut tgrid: Vec<f64> = Vec::new();
    let push_range = |from: f64, step: f64, to: f64, out: &mut Vec<f64>| {
        let n = ((to - from) / step).round() as usize;
        for i in 0..=n {
            out.push(from + i as f64 * step);
        }
    };
    push_range(0.0, 0.001, 0.6, &mut tgrid);
    push_range(0.6, 0.005, 1.0, &mut tgrid);
    push_range(1.0, 0.025, 5.0, &mut tgrid);
    tgrid.push(5.0);
    params.tgrid = Some(tgrid);

    (params, geometry, th, whichsigma, sigmavalues, feedback)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The HZP configuration differs from A2 in exactly the documented ways.
    ///
    /// # Methodology
    ///
    /// `neacrpa1t.m` is `neacrpa2t.m` with five overrides. Everything else —
    /// the cross sections, the material map, the graded mesh, the kinetics
    /// data, the heat capacities and the ejection duration — must come through
    /// unchanged, and the five overrides must each take effect.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | | A2 | A1 |
    /// |---|---|---|
    /// | power ratio | 1 | 1e-6 |
    /// | initial fuel temperature | 891.19 K | 559.15 K |
    /// | boron | 1139.01 ppm | 551.31 ppm |
    /// | rod pattern | `[100,200,100,200,200,200,200]` | `[0,0,0,228,0,0,0]` |
    /// | time-grid points | 244 | **844** |
    ///
    /// Everything else identical.
    ///
    /// **Interpretation.** The HZP fuel temperature equals the coolant
    /// inlet to within 1 K, confirming there is no stored Doppler margin at
    /// `t = 0`. The grid is 3.5x denser than A2's, which is the reference
    /// buying resolution across a power spike its own comment says spans
    /// several decades — a transient at this grid will cost roughly that
    /// much more than A2's to march.
    #[test]
    fn the_hzp_configuration_differs_from_a2_in_exactly_five_ways() {
        let (p1, g1, th1, w1, sv1, _f1) = neacrpa1t(&Params::default());
        let (p2, g2, th2, w2, sv2, _f2) = crate::neacrpa2t::neacrpa2t(&Params::default());

        // The five overrides.
        eprintln!("power ratio : {} -> {}", th2.powratio, th1.powratio);
        eprintln!("fuel temp   : {} -> {} K", p2.fueltempavg, p1.fueltempavg);
        eprintln!("boron       : {} -> {} ppm", p2.boron, p1.boron);
        eprintln!("rod pattern : {:?} -> {:?}", g2.crod, g1.crod);
        eprintln!(
            "grid points : {} -> {}",
            p2.tgrid.as_ref().unwrap().len(),
            p1.tgrid.as_ref().unwrap().len()
        );
        assert_eq!(th1.powratio, HZP_POWER_RATIO);
        assert_eq!(p1.fueltempavg, HZP_FUEL_TEMP);
        assert_eq!(p1.boron, CRITICAL_BORON);
        assert_eq!(g1.crod, vec![0.0, 0.0, 0.0, 228.0, 0.0, 0.0, 0.0]);
        assert!(p1.tgrid.as_ref().unwrap().len() > p2.tgrid.as_ref().unwrap().len());

        // HZP: the fuel starts in equilibrium with the coolant, so there is no
        // stored Doppler margin. A2 starts at 891 K.
        assert!(
            p1.fueltempavg < p2.fueltempavg,
            "HZP fuel must start colder than the full-power case"
        );
        assert!(
            (p1.fueltempavg - th1.coolant.inlettemp).abs() < 1.0,
            "HZP fuel should sit at the coolant temperature"
        );

        // Everything else is untouched.
        assert_eq!(g1.lz, g2.lz, "the graded axial mesh");
        assert_eq!(p1.velocities, p2.velocities);
        assert_eq!(p1.beta_dnp, p2.beta_dnp);
        assert_eq!(p1.lambda_dnp, p2.lambda_dnp);
        assert_eq!(p1.ejectduration, p2.ejectduration);
        assert_eq!(g1.crodeject, g2.crodeject);
        assert_eq!(g1.crodejectto, g2.crodejectto);
        assert_eq!(g1.fuel.rhocp.len(), g2.fuel.rhocp.len());
        assert_eq!(th1.maxpow, th2.maxpow, "the rated power is the same core");
        for ix in 0..17 {
            for iy in 0..17 {
                for iz in 0..18 {
                    assert_eq!(w1.get(ix, iy, iz), w2.get(ix, iy, iz));
                }
            }
        }
        for m in 0..crate::neacrpa2::MATERIALS {
            for g in 0..2 {
                assert_eq!(sv1.tot.get(m, g), sv2.tot.get(m, g));
                assert_eq!(sv1.f.get(m, g), sv2.f.get(m, g));
            }
        }
    }

    /// The rod pattern is the near-all-in Figure 3.1 configuration.
    ///
    /// # Methodology
    ///
    /// Six of seven banks fully inserted at 0 steps, bank 4 fully withdrawn at
    /// 228. The ejected bank is bank 1, which must start **fully inserted** —
    /// that is what makes this a larger reactivity insertion than A2's, where
    /// bank 1 starts half out.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **6 of 7 banks fully inserted**, bank 4 out at 228 steps. Bank 1
    /// ejects 0 -> 228 steps in 0.1 s: **228 steps of travel**, against
    /// case A2's 128.
    ///
    /// **Interpretation.** A1 withdraws the same bank 1.8x further, from a
    /// core where six of seven banks are down. Combined with the absent
    /// Doppler margin that is why this is the super-prompt case and A2 is
    /// not.
    #[test]
    fn the_rod_pattern_is_the_figure_3_1_near_all_in_configuration() {
        let (params, geometry, ..) = neacrpa1t(&Params::default());

        let inserted = geometry.crod.iter().filter(|s| **s == 0.0).count();
        eprintln!("banks fully inserted: {inserted} of 7");
        eprintln!("bank 4 at {} steps (withdrawn)", geometry.crod[3]);
        assert_eq!(inserted, 6);
        assert_eq!(geometry.crod[3], 228.0);

        // Bank 1 is ejected, and it starts fully in.
        let bank = geometry.crodeject.expect("A1 ejects a bank");
        assert_eq!(bank, 1);
        let from = geometry.crod[bank - 1];
        assert_eq!(from, 0.0, "the ejected bank must start fully inserted");
        let travel = geometry.crodejectto - from;
        eprintln!(
            "bank 1 ejects {from} -> {} steps in {} s ({travel} steps of travel)",
            geometry.crodejectto,
            params.ejectduration.unwrap()
        );

        // A2 pulls the same bank only 128 steps.
        let (_p2, g2, ..) = crate::neacrpa2t::neacrpa2t(&Params::default());
        let a2_travel = g2.crodejectto - g2.crod[0];
        eprintln!("case A2 travel for comparison: {a2_travel} steps");
        assert!(
            travel > a2_travel,
            "A1 should insert more reactivity than A2"
        );
    }

    /// Both published critical-boron comparisons disagree the same way.
    ///
    /// # Methodology
    ///
    /// The snapshot quotes a benchmark critical boron for **both** PWR cases.
    /// Taken together they say more than either does alone: a consistent sign
    /// and relative size across two independent configurations points at
    /// something systematic, where a one-off would not.
    ///
    /// This compares only the reference's own numbers against the published
    /// ones — it runs nothing. See
    /// [`crate::criticalboron_xyz`] for what **this port** computes, which is a
    /// separate and larger disagreement (X1).
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | case | code | benchmark | difference |
    /// |---|---|---|---|
    /// | A1 (HZP) | 551.31 | 567.7 | **-16.39 ppm (-2.89%)** |
    /// | A2 (full power) | 1139.01 | 1160.6 | **-21.59 ppm (-1.86%)** |
    ///
    /// **Interpretation.** The reference undershoots the published critical
    /// boron in **both** cases, by 1.9% and 2.9%, across configurations that
    /// differ in power, rod pattern and fuel temperature. A consistent sign
    /// across two such different states suggests a systematic bias in the
    /// MATLAB relative to PANTHER rather than a per-case coincidence —
    /// plausibly the coarse two-group nodal treatment, or defect G1's graded
    /// axial mesh, which both cases share.
    ///
    /// **This is a property of the reference, not of this port**, and it is
    /// a separate question from X1 (where *this port* disagrees with *the
    /// reference*). Neither number here was produced by running anything.
    #[test]
    fn both_pwr_cases_undershoot_the_published_critical_boron() {
        let cases = [
            ("A1 (HZP)", CRITICAL_BORON, BENCHMARK_CRITICAL_BORON),
            (
                "A2 (full power)",
                crate::neacrpa2t::CRITICAL_BORON,
                crate::neacrpa2t::BENCHMARK_CRITICAL_BORON,
            ),
        ];
        for (name, code, bench) in cases {
            let d = code - bench;
            eprintln!("{name}: {code} vs {bench} ppm = {d:+.2} ppm ({:+.2}%)", d / bench * 100.0);
            assert!(
                d < 0.0,
                "{name}: the reference should sit below the benchmark"
            );
        }
        // Same direction, comparable relative size.
        let r1 = (CRITICAL_BORON - BENCHMARK_CRITICAL_BORON) / BENCHMARK_CRITICAL_BORON;
        let r2 = (crate::neacrpa2t::CRITICAL_BORON - crate::neacrpa2t::BENCHMARK_CRITICAL_BORON)
            / crate::neacrpa2t::BENCHMARK_CRITICAL_BORON;
        eprintln!("relative: A1 {:+.3}%, A2 {:+.3}%", r1 * 100.0, r2 * 100.0);
        assert!(
            (r1 - r2).abs() < 0.02,
            "the two relative offsets should be within 2 percentage points"
        );
    }
}
