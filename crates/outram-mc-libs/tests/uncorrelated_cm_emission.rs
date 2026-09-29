// SPDX-License-Identifier: GPL-3.0

//! **A centre-of-mass uncorrelated law (ACE `TY < 0`) is transformed to the
//! laboratory, not dropped** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! `Nuclide::from_ace` placed an analytic-law (7/9/11) or MF=5-style chain
//! only when `TY >= 0`, with no else branch: a CM law was silently dropped and
//! the reaction fell back to the Weisskopf stand-in. It is now kept with
//! `lct = 2`, and `sample_inelastic_emission` applies OpenMC's `scatter_in_cm`
//! transform (`src/physics.cpp`, `inelastic_scatter`) via `cm_to_lab`.
//!
//! # Methodology
//!
//! No held table has such a reaction (all are `TY > 0`), so the check is on a
//! nuclide whose MT=91 law is replaced by a synthetic CM law with a closed-form
//! answer: a single-table spectrum uniform on `[E0, E0 + w]` (mean
//! `E0 + w/2`) and an isotropic CM cosine. For velocity addition with
//! `<mu_cm> = 0`, the laboratory mean energy is exactly
//! `<E_lab> = <E_cm> + E_in / (A+1)^2`; the laboratory mean cosine is checked
//! against a 800 x 800 midpoint quadrature of the same velocity addition over
//! `(E_cm, mu_cm)`, computed in the test without the sampler. `N = 400 000` draws at
//! `E_in = 2 MeV` on U-238 (`A = 236.0058`); pass within 5 sigma of the sample
//! mean. The same law with `lct = 1` must return the CM mean unchanged, so the
//! test sees the transform itself: the `E_in/(A+1)^2 = 35.6 eV` shift is
//! about 30 sample-mean sigma at this N, so applying the transform to a lab
//! law, or skipping it for a CM one, fails.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test; recorded in the #365 thread.

use njoy_outram_park_fork::acer::angular::ElasticAngular;
use njoy_outram_park_fork::nuclear_data::secondary::{
    ChiEout, ChiTabular, FissionSpectrum, UncorrelatedEmission,
};
use outram_mc_libs::geometry::position::Direction;

fn law(lct: i32, e0: f64, w: f64) -> UncorrelatedEmission {
    let t = ChiEout {
        e_out: vec![e0, e0 + w],
        pdf: vec![1.0 / w, 1.0 / w],
        cdf: vec![0.0, 1.0],
        linlin: false,
        n_discrete: 0,
    };
    UncorrelatedEmission {
        energy: FissionSpectrum::ContinuousTabular(ChiTabular {
            incident: vec![1.0e5, 2.0e7],
            tables: vec![t.clone(), t],
            incident_interp: Vec::new(),
        }),
        angular: ElasticAngular {
            energies: Vec::new(),
            lct,
        },
        lct,
        yield_n: 1,
    }
}

#[test]
fn cm_uncorrelated_law_is_transformed_to_the_lab() {
    let Some(p) = njoy_outram_park_fork::reference_data::ace_reference_file_or_skip(
        "reference-njoy/endf-b-viii.0/293.6K/U238.ace.gz",
        "uncorrelated CM emission",
    ) else {
        return;
    };
    let raw = njoy_outram_park_fork::acer::read::read(&p).expect("read");
    let base = outram_mc_libs::material::nuclide::Nuclide::from_ace(&raw, "U238").expect("U238");
    let a = base.awr;
    let (e_in, e0, w) = (2.0e6, 1.0e3, 2.0e3);
    const N: usize = 400_000;
    for lct in [2, 1] {
        let nuc = base.clone().with_uncorrelated_emission(91, law(lct, e0, w));
        let mut seed = 0xC0FFEE_u64 + lct as u64;
        let u = Direction::new(0.0, 0.0, 1.0);
        let draws: Vec<(f64, f64)> = (0..N)
            .map(|_| {
                let (e, d) = nuc.sample_inelastic_emission(91, e_in, u, -1.0e5, &mut seed);
                (e, d.w)
            })
            .collect();
        let mean = draws.iter().map(|d| d.0).sum::<f64>() / N as f64;
        let var = draws.iter().map(|d| (d.0 - mean).powi(2)).sum::<f64>() / (N - 1) as f64;
        let sem = (var / N as f64).sqrt();
        let mu = draws.iter().map(|d| d.1).sum::<f64>() / N as f64;
        let want = if lct == 2 {
            e0 + w / 2.0 + e_in / (a + 1.0).powi(2)
        } else {
            e0 + w / 2.0
        };
        let z = (mean - want) / sem;
        println!(
            "lct = {lct}: <E_lab> = {mean:.3} eV, expected {want:.3} (sem {sem:.3}, z = {z:+.2}); <mu_lab> = {mu:+.4}"
        );
        assert!(z.abs() < 5.0, "lct = {lct}: mean {mean} vs {want}, z = {z}");
        if lct == 2 {
            // Independent of the sampler: 2-D midpoint quadrature of velocity
            // addition over E_cm ~ U[e0, e0 + w] and mu_cm ~ U[-1, 1].
            let m = 800;
            let mut mu_want = 0.0;
            for i in 0..m {
                let ecm = e0 + w * (i as f64 + 0.5) / m as f64;
                for j in 0..m {
                    let mc = -1.0 + 2.0 * (j as f64 + 0.5) / m as f64;
                    let et = e_in / (a + 1.0).powi(2);
                    let el = ecm + et + 2.0 * mc * (ecm * et).sqrt();
                    mu_want += mc * (ecm / el).sqrt() + (et / el).sqrt();
                }
            }
            mu_want /= (m * m) as f64;
            let mu_sem = (1.0 / 3.0 / N as f64).sqrt();
            println!(
                "  <mu_lab> expected {mu_want:+.4} (sem {mu_sem:.1e}), z = {:+.2}",
                (mu - mu_want) / mu_sem
            );
            assert!(
                ((mu - mu_want) / mu_sem).abs() < 5.0,
                "mu_lab {mu} vs {mu_want}"
            );
        } else {
            assert!(
                (mu / (1.0 / 3.0 / N as f64).sqrt()).abs() < 5.0,
                "lab law must be isotropic, <mu> = {mu}"
            );
        }
    }
}
