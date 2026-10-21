// SPDX-License-Identifier: GPL-3.0

//! **One nuclide, ACE route against ENDF route, quantity by quantity** — the
//! localiser for a k_eff gap between outram-mc's two data routes.
//!
//! Built for the five-route ICSBEP study
//! (`verification_and_validation/icsbep/five_route_keff_2026_09_29.md`). There,
//! outram-mc on NJOY2016 ACE sat about +1850 pcm above OpenMC on the identical
//! tables for Godiva, and outram-mc read directly from ENDF did not. This prints
//! what transport actually consumes, on both routes, at fast energies:
//!
//! - the cross sections (total, elastic, fission, absorption, inelastic, (n,2n));
//! - `nu_bar(E)`;
//! - the mean of 200 000 sampled fission-neutron energies;
//! - the CM mean scattering cosine, elastic and per discrete inelastic level,
//!   both as the nuclide reports it and as its sampler actually draws it.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example ace_vs_endf_nuclide_probe -- <table.ace> <NAME> <endf tape file>
//! ```

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
const PROBE_EV: [f64; 8] = [2.53e-2, 1.0e3, 1.0e5, 5.0e5, 1.0e6, 2.0e6, 5.0e6, 1.4e7];

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [ace_path, name, tape] = &a[..] else {
        panic!("usage: <table.ace> <NAME> <endf tape file>")
    };
    let ace = Nuclide::from_ace_file(ace_path, name).expect("from_ace_file");
    let p = reference_endf(tape).expect("tape");
    let endf = Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, Default::default())
        .expect("from_endf_file");
    println!("{name}: ACE ({ace_path}) vs ENDF ({tape}), T = {TEMP_K} K");
    println!(
        "{:>9} | {:>22} | {:>22} | {:>22} | {:>22} | {:>22} | {:>22}",
        "E [eV]", "total  ACE / ENDF", "fission", "absorption", "inelastic", "n2n", "nu_bar"
    );
    for e in PROBE_EV {
        let x = ace.xs_at_energy(e, TEMP_K);
        let y = endf.xs_at_energy(e, TEMP_K);
        let c = |p: f64, q: f64| format!("{p:>10.4e}/{q:>10.4e}");
        println!(
            "{e:>9.2e} | {} | {} | {} | {} | {} | {:>10.5}/{:>10.5}",
            c(x.total, y.total),
            c(x.fission, y.fission),
            c(x.absorption, y.absorption),
            c(x.inelastic, y.inelastic),
            c(x.n2n, y.n2n),
            ace.nu_bar(e),
            endf.nu_bar(e)
        );
    }
    println!("\nmean sampled fission-neutron energy [MeV], 200 000 draws each:");
    for e in [2.53e-2, 1.0e6, 2.0e6, 5.0e6] {
        let mean = |n: &Nuclide| {
            let mut seed = 12_345_u64;
            (0..200_000)
                .map(|_| n.sample_fission_energy(e, &mut seed))
                .sum::<f64>()
                / 200_000.0
                / 1e6
        };
        println!(
            "  E_in {e:>9.2e} eV:  ACE {:.4}   ENDF {:.4}",
            mean(&ace),
            mean(&endf)
        );
    }

    println!("\nCM mean cosine: reported (elastic_mubar_cm / inelastic_mubar_cm) and SAMPLED (100 000 draws):");
    let sampled = |n: &Nuclide, mt: Option<i32>, e: f64| {
        let mut seed = 777_u64;
        let mut s = 0.0;
        for _ in 0..100_000 {
            let mu = match mt {
                None => n.sample_elastic_mu_cm(e, &mut seed),
                Some(m) => n.sample_inelastic_mu_cm(m, e, &mut seed),
            };
            s += mu.unwrap_or(f64::NAN);
        }
        s / 100_000.0
    };
    for e in [1.0e5, 5.0e5, 1.0e6, 2.0e6, 5.0e6] {
        println!(
            "  elastic  E {e:>8.1e}: reported ACE {:+.4} ENDF {:+.4} | sampled ACE {:+.4} ENDF {:+.4}",
            ace.elastic_mubar_cm(e),
            endf.elastic_mubar_cm(e),
            sampled(&ace, None, e),
            sampled(&endf, None, e)
        );
    }
    for mt in [51, 52, 53, 60, 70] {
        for e in [1.0e6, 2.0e6, 5.0e6] {
            if endf.inelastic_channel_xs(mt, e).unwrap_or(0.0) <= 0.0 {
                continue;
            }
            println!(
                "  MT={mt}   E {e:>8.1e}: reported ACE {:+.4} ENDF {:+.4} | sampled ACE {:+.4} ENDF {:+.4}",
                ace.inelastic_mubar_cm(mt, e),
                endf.inelastic_mubar_cm(mt, e),
                sampled(&ace, Some(mt), e),
                sampled(&endf, Some(mt), e)
            );
        }
    }
    println!("\ninelastic levels (mt, Q or threshold, anisotropic?) -- ACE / ENDF:");
    println!(
        "  ACE : {:?}",
        &ace.inelastic_levels_table()[..ace.inelastic_levels_table().len().min(8)]
    );
    println!(
        "  ENDF: {:?}",
        &endf.inelastic_levels_table()[..endf.inelastic_levels_table().len().min(8)]
    );
}
