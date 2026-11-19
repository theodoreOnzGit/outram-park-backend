//! **gh:#690 measurement: how often does the CM two-body cap bind on MT=91?**
//!
//! # Methodology
//!
//! `continuum_evaluated_impl` clamps the sampled CM outgoing energy to
//! `cap = E (A/(A+1))^2 + Q A/(A+1)`; OpenMC applies no clamp
//! (`inelastic_scatter`, `src/physics.cpp:1160-1175`). For U-238 MT=91 from
//! `reference-data/endf` (ENDF/B-VIII.0, 293.6 K), at five incident energies, draw
//! 200 000 CM energies through the same sampler the transport uses
//! (`sample_continuum_branch`) and count draws above the cap computed with
//! `Q = 0`. Because `Q_I < 0` for a continuum channel, the true cap is LOWER than
//! the `Q = 0` cap, so the counts below are a FLOOR on the number of clamped
//! draws, not the number. `Q_I` is not public on `Nuclide`, so the exact cap is
//! not computed here.
//!
//! # Results (2026-11-19, release, 200 000 draws per energy, seed 12345)
//!
//! | `E_in` [MeV] | `P(E'_cm > cap[Q=0])` | `P(E'_cm > 0.99 cap[Q=0])` |
//! |---|---|---|
//! | 1, 2, 3, 5, 10 | 0 of 200 000 each | 0 of 200 000 each |
//!
//! **Reading:** against the `Q = 0` cap nothing is clamped, and nothing even
//! reaches 99 % of it. That is a floor, not the answer: the true cap uses
//! `Q_I < 0` and is lower, so it can still bind; this measurement cannot say
//! how often, and `Q_I` is not public on `Nuclide`. **Not measured:** the exact
//! cap, O-16 / Al-27 MT=16, and a histogram of the top bin against OpenMC. The
//! clamp was therefore NOT removed.
//!
//! The test asserts only that the sampler ran, never a threshold (the clamp is
//! not being judged, it is being measured). ~110 s, so it is `#[ignore]`d.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{sample_continuum_branch, ContinuumAngularMode};
use outram_mc_libs::rng::lcg::prn;

#[test]
#[ignore = "measurement, ~110 s (gh:#690); run with --ignored --nocapture"]
fn how_often_the_q0_two_body_cap_is_exceeded_on_u238_mt91() {
    let Some(path) = reference_file_or_skip("endf", "n-092_U_238.endf", "U-238 (gh:#690 cap measurement)") else {
        return;
    };
    let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(293.6);
    let nuc = Nuclide::from_endf_file(&path, "U238", t, 1.0e-3).expect("evaluation");
    let law = nuc.continuum_law(91).expect("MT=91 law");
    assert!(law.cm_frame);
    let awr = nuc.awr;
    let ap1 = awr + 1.0;
    let mut seed = 12345u64;
    let _ = prn(&mut seed);
    for e in [1.0e6, 2.0e6, 3.0e6, 5.0e6, 1.0e7] {
        let n = 200_000usize;
        let cap0 = e * (awr / ap1).powi(2);
        let mut above = 0usize;
        let mut top = 0usize;
        for _ in 0..n {
            let (s, _mu) = sample_continuum_branch(&law.branches[0], e, ContinuumAngularMode::Evaluated, &mut seed);
            if s > cap0 {
                above += 1;
            }
            if s > 0.99 * cap0 {
                top += 1;
            }
        }
        println!(
            "U-238 MT=91 E_in = {:>5.1} MeV: P(E'_cm > cap[Q=0]) = {:.3e} ({above}/{n}), P(E'_cm > 0.99 cap) = {:.3e}",
            e / 1e6,
            above as f64 / n as f64,
            top as f64 / n as f64
        );
    }
}
