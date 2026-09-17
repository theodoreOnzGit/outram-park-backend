//! NEACRP case A2 — the central control-assembly ejection transient.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `neacrpa2t.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # The transient
//!
//! NEACRP-L-335 (Revision 1), Figure 3.2 / section 3.2. The **central control
//! assembly (bank 1)** is withdrawn from 100 steps to 228 (fully out) in
//! **0.1 s** at full power, and the transient is followed for 5 s.
//!
//! This is the case [`crate::thdiffusion_solvertimexyz`] was written for, and
//! the first in this crate to exercise its rod-ejection path at all —
//! [`crate::neacrpd1t`] has no rod motion. A super-prompt ejection is also the
//! regime the driver's `freqmode` note warns about: per-node frequencies are
//! unstable there, which is why `Global` is the default.
//!
//! # This case duplicates the steady case rather than calling it
//!
//! `neacrpa2t.m` is a **verbatim copy** of `neacrpa2.m` with the transient data
//! appended, where `neacrpd1t.m` calls `neacrpd1.m` directly. Diffing the two
//! shows the steady halves are byte-identical apart from **one line** — the
//! boron concentration.
//!
//! This translation therefore calls [`crate::neacrpa2::neacrpa2`] and overrides
//! that one value, rather than duplicating ~450 numbers a second time. The
//! equivalence is **tested**, not assumed: if a future snapshot lets the two
//! copies drift, that test fails and this module must be split out again. Same
//! reasoning, and the same safeguard, as the shared enthalpy inversion between
//! the two flow solvers.
//!
//! # The boron concentration is this code's own critical value
//!
//! The steady case runs at 1000 ppm; this one raises it to
//! [`CRITICAL_BORON`] = 1139.01 ppm, which the reference's comment identifies
//! as **the critical boron concentration calculated for this code** — a
//! warm-started coupled search giving `k_eff = 1.000005`.
//!
//! It also records the **official benchmark value**, and the two do not agree:
//!
//! | | ppm |
//! |---|---|
//! | this code (coupled search) | 1139.01 |
//! | benchmark reference (PANTHER, NEA/NSC/DOC(93)25 Tab 3.1) | **1160.6** |
//!
//! a difference of **-21.6 ppm, about -1.9%**. The reference's own note adds
//! that "the solver's `1/keff` scaling absorbs any small residual" — meaning the
//! transient starts exactly critical either way, so the discrepancy does not
//! propagate into the transient as a reactivity step. It is nonetheless a real
//! disagreement with the published benchmark, at the steady state, and it is
//! recorded here because it is the **only published NEACRP number anywhere in
//! the snapshot**.
//!
//! **The search that produced 1139.01 cannot be reproduced from this snapshot:**
//! the comment cites `test_critboron3.m`, which was not shipped. See
//! `docs/bedok-reference-defects.md`, "Missing files".

use crate::neacrpa2::neacrpa2;
use crate::sigmavalupd3d_handler::FeedbackTables;
use crate::types::{Geometry, Params, SigmaValues, Th, VolumetricHeatCapacity};

/// The critical boron concentration this code computes for case A2, ppm.
///
/// From `neacrpa2t.m`: a warm-started coupled search giving `k_eff = 1.000005`.
/// Compare [`BENCHMARK_CRITICAL_BORON`].
pub const CRITICAL_BORON: f64 = 1139.01;

/// The **published** critical boron concentration for case A2, ppm.
///
/// PANTHER, NEA/NSC/DOC(93)25 Table 3.1, as quoted by `neacrpa2t.m`'s comment.
/// **Quoted from that comment, not from a primary publication checked here** —
/// the specification is not in `crates/kovan-literature`. See
/// `src/data/PROVENANCE.md` before citing it.
pub const BENCHMARK_CRITICAL_BORON: f64 = 1160.6;

/// The ejected bank's final position, in steps (228 = fully withdrawn).
pub const EJECT_TO: f64 = 228.0;

/// The ejection time, seconds — independent of insertion depth.
pub const EJECT_DURATION: f64 = 0.1;

/// `[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa2t(params)`.
///
/// Builds [`crate::neacrpa2`], raises the boron to its critical value, and adds
/// the transient data: the time window and grid, two-group prompt velocities,
/// six-group delayed-neutron constants, fuel and cladding volumetric heat
/// capacities, and the bank-1 ejection scenario.
///
/// # Returns
///
/// `(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
/// [`crate::neacrpa2::neacrpa2`].
#[allow(clippy::type_complexity)]
pub fn neacrpa2t(
    params: &Params,
) -> (Params, Geometry, Th, crate::matlab::Array3<usize>, SigmaValues, FeedbackTables) {
    let (mut params, mut geometry, th, whichsigma, sigmavalues, feedback) = neacrpa2(params);

    // ----- the one steady-state value that differs; see the module docs -----
    params.boron = CRITICAL_BORON;

    // ----- transient window -----
    // `[0:0.0025:0.2, 0.2:0.01:1, 1:0.05:5, 5]` — refined through the 0.1 s
    // ejection. The driver rounds to 1 us and deduplicates the joins.
    params.tend = Some(5.0);
    let mut tgrid: Vec<f64> = Vec::new();
    let push_range = |from: f64, step: f64, to: f64, out: &mut Vec<f64>| {
        let n = ((to - from) / step).round() as usize;
        for i in 0..=n {
            out.push(from + i as f64 * step);
        }
    };
    push_range(0.0, 0.0025, 0.2, &mut tgrid);
    push_range(0.2, 0.01, 1.0, &mut tgrid);
    push_range(1.0, 0.05, 5.0, &mut tgrid);
    tgrid.push(5.0);
    params.tgrid = Some(tgrid);

    // ----- Table 2.1: prompt neutron velocities, cm/s -----
    params.velocities = vec![0.28E8, 0.44E6];

    // ----- Table 2.2: delayed neutron data, six groups, total fraction 0.76% -----
    // The reference writes the total and the per-group *shares* separately.
    const BETA_TOTAL: f64 = 0.0076;
    params.beta_dnp = [0.034, 0.200, 0.183, 0.404, 0.145, 0.034]
        .iter()
        .map(|share| BETA_TOTAL * share)
        .collect();
    params.lambda_dnp = vec![0.0128, 0.0318, 0.1190, 0.3181, 1.4027, 3.9286];

    // ----- section 2.7 volumetric heat capacities -----
    // Indexed like `tcon`: fuel then cladding. UO2 at 10.412 g/cm3 reduced by
    // 1.248% pellet dishing; Zircaloy-4 at 6.6 g/cm3.
    geometry.fuel.rhocp = vec![
        VolumetricHeatCapacity::Uo2Fuel,
        VolumetricHeatCapacity::ZircaloyClad,
    ];

    // ----- Figure 3.2: eject the central CA, bank 1 -----
    geometry.crodeject = Some(1);
    geometry.crodejectto = EJECT_TO;
    params.ejectduration = Some(EJECT_DURATION);

    (params, geometry, th, whichsigma, sigmavalues, feedback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neacrpa2::MATERIALS;

    /// **The steady half is identical to [`crate::neacrpa2`] apart from boron.**
    ///
    /// # Methodology
    ///
    /// This is the test that justifies calling `neacrpa2` instead of
    /// duplicating it, and the guard against a future snapshot letting the two
    /// copies drift. Every cross-section table, the material map, the mesh, the
    /// rod banks and the thermal-hydraulic state must come through unchanged;
    /// **only** `params.boron` may differ, and it must differ by exactly the
    /// documented amount.
    ///
    /// If this ever fails, the right response is to stop calling `neacrpa2` and
    /// transcribe `neacrpa2t.m`'s own copy — not to relax the assertion.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Boron 1000 -> 1139.01 ppm, and **everything else identical**: 22
    /// material-group entries across the base set and all five feedback
    /// channels, the full 17x17x18 material map, the graded axial mesh, the
    /// rod banks and the thermal-hydraulic state.
    ///
    /// **Interpretation.** The reference's duplication is exact, so calling
    /// [`crate::neacrpa2`] reproduces `neacrpa2t.m` faithfully while
    /// transcribing ~450 numbers once instead of twice. This test is the
    /// standing guard on that decision.
    #[test]
    fn the_steady_half_matches_the_static_case_apart_from_boron() {
        let (pt, gt, tht, wt, svt, fbt) = neacrpa2t(&Params::default());
        let (ps, gs, ths, ws, svs, fbs) = crate::neacrpa2::neacrpa2(&Params::default());

        // The one intended difference.
        eprintln!("boron: static {} ppm -> transient {} ppm", ps.boron, pt.boron);
        assert_eq!(ps.boron, 1000.0);
        assert_eq!(pt.boron, CRITICAL_BORON);

        // Everything else in the steady state.
        assert_eq!(pt.fueltempavg, ps.fueltempavg);
        assert_eq!(pt.cooltempavg, ps.cooltempavg);
        assert_eq!(pt.cooldenavg, ps.cooldenavg);
        assert_eq!(pt.g, ps.g);
        assert_eq!(pt.fuel.maxir, ps.fuel.maxir);

        assert_eq!(gt.lz, gs.lz, "the graded axial mesh must be unchanged");
        assert_eq!(gt.vi, gs.vi);
        assert_eq!(gt.crod, gs.crod, "steady rod positions");
        assert_eq!(gt.crodbtm, gs.crodbtm);
        assert_eq!(gt.fuel.rtot, gs.fuel.rtot);
        assert_eq!(gt.fuel.gap_conductance, gs.fuel.gap_conductance);
        assert_eq!(tht.maxpow, ths.maxpow);
        assert_eq!(tht.coolant.inlettemp, ths.coolant.inlettemp);
        assert_eq!(tht.coolant.inletpress, ths.coolant.inletpress);

        for ix in 0..17 {
            for iy in 0..17 {
                for iz in 0..18 {
                    assert_eq!(wt.get(ix, iy, iz), ws.get(ix, iy, iz), "map ({ix},{iy},{iz})");
                }
            }
        }
        let mut checked = 0;
        for m in 0..MATERIALS {
            for g in 0..2 {
                assert_eq!(svt.tot.get(m, g), svs.tot.get(m, g));
                assert_eq!(svt.f.get(m, g), svs.f.get(m, g));
                assert_eq!(
                    svt.fp.as_ref().unwrap().get(m, g),
                    svs.fp.as_ref().unwrap().get(m, g)
                );
                for gt2 in 0..2 {
                    assert_eq!(svt.s.get(m, gt2, g), svs.s.get(m, gt2, g));
                }
                checked += 1;
            }
        }
        // All five feedback channels, slope by slope.
        let chans: [(&str, &Option<crate::sigmavalupd3d::DeltaSigmaValues>, &Option<crate::sigmavalupd3d::DeltaSigmaValues>); 5] = [
            ("boron", &fbt.boron, &fbs.boron),
            ("fueltemp", &fbt.fueltemp, &fbs.fueltemp),
            ("cooltemp", &fbt.cooltemp, &fbs.cooltemp),
            ("coolden", &fbt.coolden, &fbs.coolden),
            ("crod", &fbt.crod, &fbs.crod),
        ];
        for (name, a, b) in chans {
            let (a, b) = (a.as_ref().unwrap(), b.as_ref().unwrap());
            assert_eq!(a.reference, b.reference, "{name} reference");
            for m in 0..MATERIALS {
                for g in 0..2 {
                    assert_eq!(a.tot.get(m, g), b.tot.get(m, g), "{name} tot {m}/{g}");
                    assert_eq!(a.f.get(m, g), b.f.get(m, g), "{name} f {m}/{g}");
                }
            }
        }
        eprintln!("cross-section entries compared: {checked} material-groups across 5 channels");
    }

    /// The transient data matches the specification tables and Figure 3.2.
    ///
    /// # Methodology
    ///
    /// Table 2.2 gives the delayed fractions as *shares* of a 0.76% total, so
    /// the shares must sum to 1 and the reconstructed total must be 0.0076
    /// exactly. Table 2.1 gives the two prompt velocities. Figure 3.2 defines
    /// the ejection: bank 1, to 228 steps, in 0.1 s.
    ///
    /// The ejection is also checked to be a genuine *withdrawal* — the final
    /// position must be above the steady one, or the "ejection" would be an
    /// insertion.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Total beta **0.007600 (0.760%)**, matching Table 2.2's quoted total
    /// exactly from the six shares. Bank 1 ejects **100 -> 228 steps in
    /// 0.1 s**, its tip travelling **197.12 -> 401.18 cm**.
    ///
    /// **Interpretation.** The tip starts near the core mid-plane and ends
    /// at 401.18 cm, below the 427.3 cm core top — so "fully withdrawn" at
    /// 228 steps still leaves the tip inside the modelled height, which is
    /// what the benchmark's stroke gives. A 128-step withdrawal of a
    /// mid-plane bank in 0.1 s is a super-prompt ejection, the regime the
    /// driver's `freqmode` note says needs the global frequency mode.
    #[test]
    fn the_transient_data_matches_the_specification() {
        let (params, geometry, ..) = neacrpa2t(&Params::default());

        let betatot: f64 = params.beta_dnp.iter().sum();
        eprintln!("total beta = {betatot:.6} ({:.3}%)", betatot * 100.0);
        assert!((betatot - 0.0076).abs() < 1e-15, "total beta is {betatot}");
        assert_eq!(params.beta_dnp.len(), 6);
        assert!(
            params.lambda_dnp.windows(2).all(|w| w[0] < w[1]),
            "decay constants should be ordered"
        );
        assert_eq!(params.velocities, vec![0.28E8, 0.44E6]);

        // Figure 3.2: bank 1, 100 -> 228 steps in 0.1 s.
        assert_eq!(geometry.crodeject, Some(1));
        assert_eq!(geometry.crodejectto, EJECT_TO);
        assert_eq!(params.ejectduration, Some(EJECT_DURATION));
        let from = geometry.crod[0];
        eprintln!("bank 1 ejects {from} -> {EJECT_TO} steps in {EJECT_DURATION} s");
        assert!(
            EJECT_TO > from,
            "an ejection must withdraw the bank, not insert it"
        );
        let tip_from = geometry.crodbtm + from * geometry.crodstep;
        let tip_to = geometry.crodbtm + EJECT_TO * geometry.crodstep;
        eprintln!("  tip travels {tip_from:.2} -> {tip_to:.2} cm");

        // Heat capacities are added for the transient rod solver.
        assert_eq!(geometry.fuel.rhocp.len(), 2);
        assert_eq!(params.tend, Some(5.0));
    }

    /// This code's critical boron disagrees with the published benchmark.
    ///
    /// # Methodology
    ///
    /// `neacrpa2t.m` records both its own coupled-search result and the
    /// official value, so the comparison is available without running anything.
    /// This test states the discrepancy as a number so it cannot be lost, and
    /// asserts the direction and rough size rather than an exact match — there
    /// is nothing here to tune.
    ///
    /// **This is a comparison of two quoted numbers, not a validation.** The
    /// 1139.01 ppm figure was produced by `test_critboron3.m`, which is **not
    /// in the snapshot**, so it cannot be reproduced or checked here; and
    /// 1160.6 ppm is quoted from a comment, not from a publication in the
    /// archive.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | | ppm |
    /// |---|---|
    /// | this code (coupled search) | 1139.01 |
    /// | benchmark (PANTHER) | 1160.6 |
    /// | difference | **-21.59 ppm (-1.86%)** |
    ///
    /// **Interpretation.** The reference code needs *less* boron than the
    /// benchmark to reach criticality, so at the same boron it computes a
    /// slightly **more** reactive core. At the usual PWR sensitivity of very
    /// roughly 10 pcm/ppm, 21.6 ppm is on the order of 200 pcm — a plausible
    /// size for a coarse-mesh two-group nodal method with five feedback
    /// channels, and not obviously a defect.
    ///
    /// **This is not a validation of anything in this crate.** Both numbers
    /// are quoted from a MATLAB comment. The 1139.01 figure came from
    /// `test_critboron3.m`, which is not in the snapshot, so it cannot be
    /// reproduced here; and 1160.6 has not been checked against the primary
    /// publication. What the test does is keep the discrepancy visible, so
    /// that when `criticalboron_xyz.m` is translated there is a number
    /// waiting for it.
    #[test]
    fn the_critical_boron_differs_from_the_published_benchmark() {
        let diff = CRITICAL_BORON - BENCHMARK_CRITICAL_BORON;
        let rel = diff / BENCHMARK_CRITICAL_BORON * 100.0;
        eprintln!("critical boron, case A2:");
        eprintln!("  this code           = {CRITICAL_BORON} ppm (coupled k_eff = 1.000005)");
        eprintln!("  benchmark (PANTHER) = {BENCHMARK_CRITICAL_BORON} ppm");
        eprintln!("  difference          = {diff:+.2} ppm ({rel:+.2}%)");

        assert!(diff < 0.0, "this code sits below the benchmark value");
        assert!(
            diff.abs() > 20.0 && diff.abs() < 30.0,
            "the recorded gap is about 21.6 ppm; got {diff}"
        );
        // The transient starts critical regardless, because the driver divides
        // the fission operator by the converged k_eff.
        let (params, ..) = neacrpa2t(&Params::default());
        assert_eq!(params.boron, CRITICAL_BORON);
    }
}
