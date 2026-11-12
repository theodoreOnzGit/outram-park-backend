//! **The OpenMC-faithful collision kernels are the transport DEFAULT**
//! (maintainer decision 2026-11-11, gh:#1386; pattern of
//! `correct_physics_is_default.rs`).
//!
//! Methodology: build O-16 (ENDF/B-VIII.0, 293.6 K) through the ordinary path and
//! drive the nuclide-level collision samplers transport calls
//! (`Nuclide::sample_elastic_collision`, `sample_level_inelastic_scatter`,
//! `physics::scatter::scatter_ladder_xi`) on one seed. The default must equal the
//! OpenMC-faithful translations (`elastic_scatter_openmc`,
//! `level_inelastic_scatter_openmc`) bit for bit, and `with_legacy_scatter_kernels`
//! must give the earlier kernels (`free_gas_elastic_scatter_dbrc`,
//! `two_body_scatter_with_mu`) bit for bit: the named alternative exists and is
//! the opt-in, not the default. Pass: bitwise equality over 200 draws per energy.
//! The two kernel sets must differ (else the test would not tell them apart).
//!
//! Result (2026-11-11): see the assertions; skipped without the O-16 tape.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};
use outram_mc_libs::physics::scatter::{
    elastic_scatter_openmc, free_gas_elastic_scatter_dbrc, level_inelastic_scatter_openmc,
    scatter_kernels_from_env, scatter_ladder_xi, two_body_scatter, two_body_scatter_with_mu,
    uniform_pm1, ScatterKernels,
};
use outram_mc_libs::rng::lcg::prn;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

fn o16() -> Option<Nuclide> {
    let p = reference_endf("n-008_O_016-ENDF8.0.endf")?;
    Nuclide::from_endf_file(&p, "O16", ThermodynamicTemperature::new::<kelvin>(293.6), 1.0e-3).ok()
}

fn same(a: (f64, Direction), b: (f64, Direction)) -> bool {
    a.0.to_bits() == b.0.to_bits()
        && a.1.u.to_bits() == b.1.u.to_bits()
        && a.1.v.to_bits() == b.1.v.to_bits()
        && a.1.w.to_bits() == b.1.w.to_bits()
}

#[test]
fn openmc_kernels_are_the_default_and_legacy_is_named() {
    if scatter_kernels_from_env() == ScatterKernels::Legacy {
        println!("SKIP: OUTRAM_MC_LEGACY_SCATTER_KERNELS is set");
        return;
    }
    let Some(nuc) = o16() else {
        println!("SKIP: O-16 tape absent");
        return;
    };
    assert_eq!(nuc.scatter_kernels(), ScatterKernels::OpenMc, "the default must be OpenMC's kernels");
    let legacy = nuc.clone().with_legacy_scatter_kernels();
    assert_eq!(legacy.scatter_kernels(), ScatterKernels::Legacy);

    let u = Direction::new(0.36, 0.48, 0.8);
    let mut elastic_differs = 0usize;
    let mut level_differs = 0usize;
    let (mut n_elastic, mut n_level) = (0usize, 0usize);
    for &e in &[1.0e3_f64, 2.5e5, 3.0e6, 7.0e6] {
        for k in 0..200u64 {
            let s0 = 0x1234_5678_9abc_def1_u64.wrapping_add(k * 7919);
            // elastic, default vs the OpenMC translation
            let mut a = s0;
            let got = nuc.sample_elastic_collision(e, u, 293.6, &mut a);
            let mut b = s0;
            let kt = nuc.free_gas_kt(293.6);
            let use_ptable = nuc.needs_urr_draw(e);
            let (eo, uo, _) = elastic_scatter_openmc(
                e,
                u,
                nuc.awr,
                kt,
                nuc.dbrc_table(),
                use_ptable,
                |s| nuc.sample_elastic_mu_cm(e, s).unwrap_or_else(|| uniform_pm1(s)),
                &mut b,
            );
            assert!(same(got, (eo, uo)), "default elastic differs from elastic_scatter_openmc at {e} eV");
            assert_eq!(a, b, "same number of draws");
            // elastic, legacy vs the earlier kernel
            let mut c = s0;
            let leg = legacy.sample_elastic_collision(e, u, 293.6, &mut c);
            let mut d = s0;
            let mu_cm = nuc.sample_elastic_mu_cm(e, &mut d).unwrap_or_else(|| 2.0 * prn(&mut d) - 1.0);
            let old = free_gas_elastic_scatter_dbrc(e, u, nuc.awr, kt, mu_cm, &mut d, nuc.dbrc_table());
            assert!(same(leg, old), "legacy elastic differs from free_gas_elastic_scatter_dbrc");
            n_elastic += 1;
            if !same(got, leg) {
                elastic_differs += 1;
            }
            // level inelastic
            let mut f = s0;
            if let Inelastic::Level { q, mt } = nuc.sample_inelastic(e, &mut f) {
                let s1 = f;
                let got = nuc.sample_level_inelastic_scatter(mt, q, e, u, &mut f);
                let mut g = s1;
                let mu = nuc.sample_inelastic_mu_cm(mt, e, &mut g).unwrap_or_else(|| uniform_pm1(&mut g));
                let want = level_inelastic_scatter_openmc(e, u, nuc.awr, q, mu, &mut g);
                assert!(same(got, want), "default level inelastic differs from level_inelastic_scatter_openmc");
                let mut h = s1;
                let leg = legacy.sample_level_inelastic_scatter(mt, q, e, u, &mut h);
                let mut i = s1;
                let old = match nuc.sample_inelastic_mu_cm(mt, e, &mut i) {
                    Some(m) => two_body_scatter_with_mu(e, u, nuc.awr, q, m, &mut i),
                    None => two_body_scatter(e, u, nuc.awr, q, &mut i),
                };
                assert!(same(leg, old), "legacy level inelastic differs from two_body_scatter_with_mu");
                n_level += 1;
                if !same(got, leg) {
                    level_differs += 1;
                }
            }
        }
    }
    println!("elastic: {n_elastic} draws, default != legacy in {elastic_differs}; level inelastic: {n_level}, differ in {level_differs}");
    assert!(elastic_differs > n_elastic / 2, "the two elastic kernels must be distinguishable");
    assert!(n_level == 0 || level_differs > 0, "the two level kernels must be distinguishable");
}

/// The reaction ladder: OpenMC's second draw in `scatter()` is taken by default
/// for a surviving collision and skipped for an absorbed one and for legacy.
#[test]
fn the_two_draw_reaction_ladder() {
    let (abs, tot) = (0.3_f64, 1.0_f64);
    let mut s = 99_u64;
    // absorbed: no further draw
    let before = s;
    assert_eq!(scatter_ladder_xi(ScatterKernels::OpenMc, 0.1, abs, tot, &mut s), 0.1);
    assert_eq!(s, before);
    // survivor: second draw in [abs, tot)
    let xi = scatter_ladder_xi(ScatterKernels::OpenMc, 0.6, abs, tot, &mut s);
    assert!(xi >= abs && xi < tot && s != before);
    let mut t = 99_u64;
    assert_eq!(xi.to_bits(), (abs + prn(&mut t) * (tot - abs)).to_bits());
    // legacy keeps the single variate
    let mut z = 5_u64;
    assert_eq!(scatter_ladder_xi(ScatterKernels::Legacy, 0.6, abs, tot, &mut z), 0.6);
    assert_eq!(z, 5);
}
