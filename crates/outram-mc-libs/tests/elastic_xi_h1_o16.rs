//! **Measured mean lethargy gain per elastic collision, `⟨ln(E/E′)⟩`, for H-1
//! and O-16, against the analytic `ξ` from each tape's own AWR** (GitHub #532).
//!
//! # Why this exists
//!
//! Rung 4 of the Monte Carlo tutorial (`docs/tutorial/src/lct008.md`, step 1;
//! `verification_and_validation/tutorial_rung4/lesson_draft.md`) computes `α`
//! and `ξ` for H-1 and O-16 from the tapes' AWR and says the code has *no
//! function that computes ξ*: `ξ` emerges from repeating one elastic collision.
//! Until this test nothing measured that collision for hydrogen or oxygen.
//! `examples/graphite_energy_decrement.rs` measures `ξ` from the graphite
//! S(α,β) law (thermal range only), and `examples/epithermal_slowing_down.rs`
//! measures `ξ/ξ₀` above the cutoff through the target-at-rest form
//! (`two_body_scatter_with_mu`) and the older `free_gas_elastic_scatter`
//! wrapper, printing a table without a σ-based gate and without H-1. Neither
//! generalises to this check without changing what it gates, so this is a
//! sibling with the same pattern (sample, average `ln(E/E′)`, compare with the
//! two-body oracle), written as a test so it runs with the suite.
//!
//! # Methodology
//!
//! **Data.** ENDF/B-VIII.0 from `reference-data/endf/`: `n-001_H_001-ENDF8.0-Beta6.endf`
//! and `n-008_O_016-ENDF8.0.endf` (the tapes the LCT-008 lattice runs),
//! through `Nuclide::from_endf_file` at 293.6 K and RECONR tolerance 0.001 —
//! the ordinary constructor, so DBRC is attached as in transport. H-1 also
//! carries `tsl-HinH2O.endf` (H in H₂O, as in the lattice's water), so the
//! branch order is the transport loop's (`transport_csg.rs`, the scatter
//! branch): `sample_thermal` first, which must decline at every probe energy,
//! then the elastic kernel.
//!
//! **Sampler.** `free_gas_elastic_scatter_dbrc(E, u, AWR, kT, μ_cm, seed,
//! nuc.dbrc_table())` with `kT = nuc.free_gas_kt(293.6)`, exactly the call
//! transport makes. The CM cosine is drawn **isotropic** (`μ_cm = 2ζ − 1`),
//! because the analytic `ξ` is defined for isotropic CM scattering; the
//! evaluation's MF=4 law is reported beside it (not gated) to show what
//! transport adds.
//!
//! **Energies.** 1 keV, 10 keV, 100 keV and 1 MeV. The lowest is 100 times
//! H(H₂O)'s S(α,β) cutoff, which is the tape's own `E_max` = 10.0 eV (asserted
//! below from the table, to be at least ten times below every probe), and
//! 99 times `400 kT` = 10.1 eV.
//!
//! *A correction from the first run (2026-10-05):* this paragraph first said
//! the cutoff was ~5 eV and asserted the lowest probe at ≥ 100× it; the table
//! reports 10.000000000000998 eV, so 1 keV sat 1e-12 below that bound. The
//! assumption was wrong, not the sampler, and the precondition is now the
//! order of magnitude it was meant to express. Nothing in the pass criterion
//! below was changed. At 1 keV the DBRC window (`E ≤ 1 keV`) still applies, so
//! that point exercises the moving-target sampler with the 0 K rejection; above
//! it the target is at rest, as in OpenMC's `sample_target_velocity`. A third
//! arm, **H-1 with DBRC removed** (`without_dbrc`), exercises the plain
//! free-gas target sampling at every energy: H-1's AWR is below 1, so OpenMC's
//! `400 kT` at-rest gate (`awr > 1.0`) never applies to it.
//!
//! **Oracle.** `α = ((A−1)/(A+1))²`, `ξ = 1 + α ln α / (1 − α)`, with `A` the
//! tape's AWR. H-1: AWR 0.9991673, `ξ = 0.9999973`. O-16: AWR 15.85751,
//! `ξ = 0.1210` (the lesson's table).
//!
//! **Pass criterion (written 2026-10-05, before the first run).** `N = 4 000 000`
//! collisions per point, from one LCG stream per arm. For each arm and energy,
//! `|⟨ln(E/E′)⟩ − ξ| ≤ 4 sem`, where `sem = sd/√N` is the sample standard
//! error of the mean. Twelve points; a correct sampler fails one by chance
//! with probability ~8e-4 (and the seeds are fixed, so the result is
//! reproducible).
//!
//! **Predicted systematic, so the criterion is not consumed by it.** Target
//! motion shifts `⟨E′⟩` by about `3A·kT/(A+1)²`: for H-1 ~0.75 kT, i.e.
//! `2e-5 E` at 1 keV, and through the `ln` near `E′ → 0` perhaps ten times that,
//! ≤ 3e-4 absolute (0.6 sem at `N = 4e6`, where H-1's `sd ≈ 1`). For O-16 it is
//! ~5e-6 relative (≪ its sem of ~4e-5). At 10 keV and above every arm is below
//! 0.1 sem. A failure therefore means a defect, not thermal motion.
//!
//! # Results
//!
//! **Measured 2026-10-05** at `6faff1ed8` (library unchanged; this file new),
//! ENDF/B-VIII.0, 293.6 K, `N = 4 000 000` per point. **All twelve points pass;
//! the largest departure is 1.66 sem**, and `χ² = 8.5` over 12 points
//! (expected 12 ± 4.9), so the scatter is what the sems say it should be.
//!
//! | arm | E | `⟨ln(E/E′)⟩` | − ξ | in sem |
//! |---|---|---|---|---|
//! | H-1, DBRC, H₂O S(α,β) | 1 keV | 1.0004456 | +4.5e-4 | +0.90 |
//! | | 10 keV | 1.0002837 | +2.9e-4 | +0.57 |
//! | | 100 keV | 0.9999401 | −5.7e-5 | −0.11 |
//! | | 1 MeV | 1.0002897 | +2.9e-4 | +0.58 |
//! | H-1, DBRC removed (free gas at every E) | 1 keV | 0.9998472 | −1.5e-4 | −0.30 |
//! | | 10 keV | 1.0003982 | +4.0e-4 | +0.80 |
//! | | 100 keV | 1.0008274 | +8.3e-4 | +1.66 |
//! | | 1 MeV | 0.9994186 | −5.8e-4 | −1.16 |
//! | O-16, DBRC | 1 keV | 0.1209869 | +6.9e-6 | +0.19 |
//! | | 10 keV | 0.1209317 | −4.8e-5 | −1.33 |
//! | | 100 keV | 0.1209601 | −2.0e-5 | −0.55 |
//! | | 1 MeV | 0.1209708 | −9.2e-6 | −0.25 |
//!
//! sem is 5.0e-4 for H-1 (`sd = 1.00`, as for an exponential: `E′/E` is
//! uniform on `[α, 1]` with `α ≈ 0`) and 3.6e-5 for O-16. Analytic `ξ`:
//! H-1 0.9999973, O-16 0.1209800.
//!
//! **Interpretation.** The elastic sampler transport calls reproduces the
//! two-body `ξ` for both nuclides to 5e-4 (H-1, 0.05 %) and 4e-5 (O-16,
//! 0.04 %), with DBRC's moving-target branch (1 keV), the target-at-rest branch
//! (≥ 10 keV) and, for H-1, the plain free-gas branch at all energies. The
//! predicted thermal bias at 1 keV for H-1 (≤ 3e-4) is not resolved: the
//! measured +4.5e-4 ± 5.0e-4 is consistent with it and with zero. So the
//! lesson's "18.2 collisions from 2 MeV to 0.025 eV" rests on a kernel that
//! has now been measured to give `ξ_H = 1.0000 ± 0.0005` per collision, where
//! the formula holds (above the S(α,β) range; below it the bound-atom law
//! takes over and `ξ` is not the free-atom number).
//!
//! **Reported, not gated: the evaluation's MF=4 law**, as transport samples
//! it. H-1's `⟨μ_cm⟩` is within 6e-4 of zero at every probe, and its `⟨ln⟩`
//! within 2.1 sem of the isotropic `ξ`. O-16 is not isotropic in the CM above
//! ~10 keV (`⟨μ_cm⟩` = −0.031 at 100 keV, +0.010 at 1 MeV), and its `⟨ln⟩`
//! moves with the law's shape: 0.1249 at 100 keV (+3.2 %, a backward-leaning
//! law loses more energy) and 0.1217 at 1 MeV (+0.6 % despite a slightly
//! forward mean; `ln(E/E′)` is convex in `μ`, so a law weighted toward both
//! ends raises the mean — a reading, not measured here). That is the
//! evaluation's physics, not a defect, and it is why the gate uses isotropic
//! cosines.
//!
//! **Not covered.** Below the S(α,β) cutoff (H in H₂O, where `ξ` has no
//! free-atom value) and the MF=4-weighted oracle. Timing: the test ran in 47 s
//! (data load ~30 s) on an Intel Xeon @ 2.10 GHz, single thread pinned to 2 of
//! 4 shared logical cores (`taskset -c 2,3`), 15 GB RAM, Linux 6.18, CPU only;
//! the same two cores were also running a Godiva ensemble, so the time is an
//! upper bound.
//!
//! ```text
//! cargo test --release -p outram-mc-libs --test elastic_xi_h1_o16 -- --nocapture
//! ```

use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::scatter::free_gas_elastic_scatter_dbrc;
use outram_mc_libs::prelude::prn;

const TEMP_K: f64 = 293.6;
const N: usize = 4_000_000;
const PROBE_EV: [f64; 4] = [1.0e3, 1.0e4, 1.0e5, 1.0e6];
/// The criterion fixed before the first run: `|mean − ξ| ≤ K_SEM · sem`.
const K_SEM: f64 = 4.0;

/// `ξ` for isotropic CM elastic scattering off a target at rest.
fn xi_analytic(awr: f64) -> f64 {
    let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
    1.0 + alpha * alpha.ln() / (1.0 - alpha)
}

/// Mean, sample sd and sem of `ln(E/E′)` over `N` collisions at `e`, plus
/// `⟨μ_cm⟩` of the cosines used. `mf4` selects the evaluation's CM law instead
/// of isotropic.
fn measure(nuc: &Nuclide, e: f64, mf4: bool, seed: &mut u64) -> (f64, f64, f64, f64) {
    let kt = nuc.free_gas_kt(TEMP_K);
    let u = Direction::new(0.0, 0.0, 1.0);
    let (mut mean, mut m2, mut mu_sum) = (0.0_f64, 0.0_f64, 0.0_f64);
    for i in 0..N {
        assert!(
            nuc.sample_thermal(e, seed).is_none(),
            "{}: the S(a,b) law sampled at {e} eV, which must be above its cutoff",
            nuc.name
        );
        let mu_cm = if mf4 {
            nuc.sample_elastic_mu_cm(e, seed)
                .unwrap_or_else(|| 2.0 * prn(seed) - 1.0)
        } else {
            2.0 * prn(seed) - 1.0
        };
        mu_sum += mu_cm;
        let (ep, _) =
            free_gas_elastic_scatter_dbrc(e, u, nuc.awr, kt, mu_cm, seed, nuc.dbrc_table());
        assert!(ep > 0.0, "non-positive outgoing energy {ep} at {e} eV");
        // Welford: one pass, no catastrophic cancellation at N = 4e6.
        let x = (e / ep).ln();
        let d = x - mean;
        mean += d / (i + 1) as f64;
        m2 += d * (x - mean);
    }
    let sd = (m2 / (N - 1) as f64).sqrt();
    (mean, sd, sd / (N as f64).sqrt(), mu_sum / N as f64)
}

#[test]
fn measured_xi_matches_analytic_for_h1_and_o16() {
    let Some(h_path) = reference_endf_or_skip("n-001_H_001-ENDF8.0-Beta6.endf", "H-1 (#532)")
    else {
        return;
    };
    let Some(o_path) = reference_endf_or_skip("n-008_O_016-ENDF8.0.endf", "O-16 (#532)") else {
        return;
    };
    let Some(sab_path) = reference_endf_or_skip("tsl-HinH2O.endf", "H in H2O (#532)") else {
        return;
    };

    let sab = ThermalScattering::from_endf_file(
        sab_path.to_str().expect("path"),
        1,
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("H in H2O S(a,b)");
    let cutoff = sab.cutoff_ev();
    assert!(
        PROBE_EV[0] >= 10.0 * cutoff,
        "lowest probe {} eV is not well above the S(a,b) cutoff {cutoff} eV",
        PROBE_EV[0]
    );
    let h1 = Nuclide::from_endf_file(&h_path, "H1", TEMP_K, 1.0e-3)
        .expect("H-1 reconstructs")
        .with_thermal_scattering(sab);
    let o16 = Nuclide::from_endf_file(&o_path, "O16", TEMP_K, 1.0e-3).expect("O-16 reconstructs");
    assert!(h1.has_dbrc() && o16.has_dbrc(), "the ordinary constructor attaches DBRC");
    let h1_no_dbrc = h1.clone().without_dbrc();
    let kt = h1.free_gas_kt(TEMP_K);
    println!(
        "S(a,b) cutoff {cutoff:.4} eV; kT {kt:.5e} eV; 400 kT = {:.2} eV; DBRC e_max {} eV",
        400.0 * kt,
        h1.dbrc_table().map_or(f64::NAN, |t| t.e_max_ev())
    );

    let arms: [(&str, &Nuclide, u64); 3] = [
        ("H-1 (DBRC, H2O S(a,b))", &h1, 20_261_005),
        ("H-1 (DBRC removed)", &h1_no_dbrc, 20_261_006),
        ("O-16 (DBRC)", &o16, 20_261_007),
    ];
    let mut failures = Vec::new();
    for (label, nuc, seed0) in arms {
        let xi = xi_analytic(nuc.awr);
        println!("\n{label}: AWR {:.7}, analytic xi {xi:.7}, N = {N} per energy", nuc.awr);
        println!(
            "{:>9}  {:>11}  {:>9}  {:>10}  {:>7} | {:>11}  {:>8}",
            "E [eV]", "<ln E/E'>", "sem", "diff", "diff/sem", "MF=4 <ln>", "<mu_cm>"
        );
        let mut seed = seed0;
        for &e in &PROBE_EV {
            let (m, _sd, sem, _) = measure(nuc, e, false, &mut seed);
            let (m4, _, _, mu4) = measure(nuc, e, true, &mut seed);
            let z = (m - xi) / sem;
            println!(
                "{e:>9.0e}  {m:>11.7}  {sem:>9.2e}  {:>+10.2e}  {z:>+7.2} | {m4:>11.7}  {mu4:>+8.5}",
                m - xi
            );
            if z.abs() > K_SEM {
                failures.push(format!("{label} at {e} eV: {m:.7} vs xi {xi:.7} ({z:+.2} sem)"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "measured <ln(E/E')> departs from the analytic xi by more than {K_SEM} sem:\n{}",
        failures.join("\n")
    );
}
