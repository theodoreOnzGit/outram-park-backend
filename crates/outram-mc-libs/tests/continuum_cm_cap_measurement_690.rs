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
//! the channel's own `Q_I`, found through `Nuclide::sample_inelastic_at`.
//!
//! # Results (2026-11-20, release, 200 000 draws per energy, seed 12345, exact cap)
//!
//! `Q_I = -433.76 keV` (read from the continuum level, as the transport does).
//!
//! | `E_in` [MeV] | `P(E'_cm > cap)` | `P(E'_cm > 0.99 cap)` |
//! |---|---|---|
//! | 1 | **1.78 %** (3563 / 200 000) | 1.91 % |
//! | 2 | 5e-6 (1 / 200 000) | 5e-6 |
//! | 3, 5, 10 | 0 / 200 000 | 0 / 200 000 |
//!
//! **Reading:** the clamp binds only just above the MT=91 threshold (1 MeV is
//! 0.56 MeV above it), where 1.8 % of the draws exceed the two-body bound and
//! are piled at it; from 2 MeV up it is inactive. OpenMC applies no clamp, so
//! this is a deliberate deviation that is **small in extent** (near-threshold,
//! U-238 MT=91 only), **not removed**: the clamp enforces energy balance, and
//! removing it needs the ICSBEP re-measurement the issue asks for (integration).
//! Not measured: O-16 / Al-27 MT=16 and a top-bin histogram against OpenMC.
//!
//! The test asserts only that the sampler ran, never a threshold (the clamp is
//! not being judged, it is being measured). ~110 s, so it is `#[ignore]`d.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};
use outram_mc_libs::physics::scatter::{sample_continuum_branch, ContinuumAngularMode};
use outram_mc_libs::rng::lcg::prn;

#[test]
#[ignore = "measurement, ~110 s (gh:#690); run with --ignored --nocapture"]
fn how_often_the_two_body_cap_is_exceeded_on_u238_mt91() {
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
        // The channel's own Q_I, read the way the transport reads it: the
        // continuum level returned by `sample_inelastic_at`.
        let q = (0..10_000)
            .find_map(|_| match nuc.sample_inelastic_at(e, None, &mut seed) {
                Inelastic::Continuum { q } if q != 0.0 => Some(q),
                _ => None,
            })
            .unwrap_or(0.0);
        // The EXACT cap `continuum_evaluated_impl` applies.
        let cap0 = (e * (awr / ap1).powi(2) + q * awr / ap1).max(0.0);
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
            "U-238 MT=91 E_in = {:>5.1} MeV, Q_I = {q:.4e} eV: P(E'_cm > cap) = {:.3e} ({above}/{n}), P(E'_cm > 0.99 cap) = {:.3e}",
            e / 1e6,
            above as f64 / n as f64,
            top as f64 / n as f64
        );
    }
}
