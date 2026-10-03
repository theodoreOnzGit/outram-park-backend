// SPDX-License-Identifier: GPL-3.0

//! **The correlated angle laws (ACE law 61, law 44) and the AND histogram forms
//! are sampled as OpenMC samples them.** This is part of the GitHub #365 audit.
//!
//! # What changed
//!
//! | piece | before | OpenMC (and now) |
//! |---|---|---|
//! | law-61 cosine row | the lower edge `k` of the `E'` bin | `k` or `k+1`, whichever cdf edge is nearer the draw (lin-lin tables); `k` for histogram tables (`CorrelatedAngleEnergy::sample_dist`) |
//! | cosine inverse in a row | linear in the cdf | `Tabular::sample`: quadratic for lin-lin, linear for histogram |
//! | law-44 `r`, `a` | row `k`'s | interpolated to the raw `E'`, lin-lin tables (`KalbachMann::sample_params`) |
//! | AND / law-61 `intt = 1` | refused | histogram |
//! | AND 32 equiprobable bins | read lin-lin, the end densities borrowed | histogram (`angle_distribution.py`) |
//!
//! # Methodology
//!
//! `verification_and_validation/ace_route_physics/openmc_inputs/correlated_angle_reference.py`
//! reads each table with OpenMC's own ACE reader. It computes the **exact**
//! `<mu>` and `<mu E'>` of OpenMC's scheme, and the same moments under the old
//! scheme, and writes them to `data/correlated_angle_openmc.csv`. The exact
//! values come from a split at the cdf midpoint and 48-point Gauss-Legendre in
//! the energy variate.
//!
//! The tables:
//! - **NJOY2016 U-235** (293.6 K, ENDF/B-VIII.0, `reference-data/ace`): MT=91
//!   and MT=16 are law 61, lin-lin.
//! - **NJOY2016 O-16** (five-route build): MT=91 is law 44, histogram in `E'`,
//!   so nothing should move. It is the control.
//! - **Constructed** from those two, because no table in reach has the forms.
//!   Every held AND and law-61 cosine row is `intt = 2`, and every law-44
//!   table is a histogram.
//!   - `U235_hist.ace`: MT=91 cosine rows as histograms, MT=2 rows as 32
//!     equiprobable bins, MT=51 rows as histograms.
//!   - `O16_kmlin.ace`: MT=91 law 44 lin-lin in `E'`.
//!
//! Pass: over `N = 4e6` draws per row from the transport path
//! (`physics::scatter::sample_continuum_branch`,
//! `Nuclide::sample_elastic_mu_cm`, `sample_inelastic_mu_cm`), every mean
//! lies within 5 sample-sem of OpenMC's. **Power:** at least one row per
//! correlated case must sit more than 5 sem from the OLD scheme's value;
//! O-16 histogram is exempt, since old = new there.
//!
//! `N` was `1e6` in the first run. Every row agreed with OpenMC (worst |z|
//! 1.74), but the power gate failed: the constructed cases sat only 5.0 and
//! 4.8 sem from the old scheme. The gate was kept at 5 and `N` was raised
//! instead, which doubles the separation.
//!
//! # Results (2026-09-29)
//!
//! Every row passes. The worst |z| against OpenMC is recorded in
//! `verification_and_validation/ace_route_physics/correlated_angle_2026-09-29.md`,
//! with the separation from the old scheme. U-235 MT=91 at 5 MeV:
//! OpenMC `<mu_cm>` 0.02017, old scheme 0.01561.

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{sample_continuum_branch, ContinuumAngularMode};
use std::path::PathBuf;

const N: usize = 4_000_000;

fn ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

fn load(case: &str) -> Option<Nuclide> {
    let root = ws();
    let (path, name) = match case {
        "U235" => (
            root.join("reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz"),
            "U235",
        ),
        "U235_hist" => (root.join("target/ace_extra/U235_hist.ace"), "U235"),
        "O16" => (
            root.join("target/five_route_keff/njoy/293.6K/O16.ace"),
            "O16",
        ),
        "O16_kmlin" => (root.join("target/ace_extra/O16_kmlin.ace"), "O16"),
        _ => unreachable!(),
    };
    if !path.is_file() {
        println!("{} absent: skipping {case}", path.display());
        return None;
    }
    Some(Nuclide::from_ace_file(&path, name).expect("table loads"))
}

#[test]
fn correlated_angles_and_histograms_sample_as_openmc() {
    let csv = ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/correlated_angle_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut cache: Vec<(String, Option<Nuclide>)> = Vec::new();
    let mut worst: f64 = 0.0;
    let mut power: Vec<(String, f64)> = Vec::new();
    let mut checked = 0;
    for (row, line) in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .enumerate()
    {
        let f: Vec<&str> = line.split(',').collect();
        let (case, mt, e) = (
            f[0],
            f[1].parse::<i32>().unwrap(),
            f[2].parse::<f64>().unwrap(),
        );
        let v: Vec<f64> = f[3..].iter().map(|x| x.parse().unwrap()).collect();
        if !cache.iter().any(|(c, _)| c == case) {
            cache.push((case.to_string(), load(case)));
        }
        let Some(nuc) = cache.iter().find(|(c, _)| c == case).unwrap().1.as_ref() else {
            continue;
        };
        let mut seed = 0xC0A1_0000 + row as u64;
        if v[1].is_nan() {
            // AND: a cosine only.
            let mus: Vec<f64> = (0..N)
                .map(|_| {
                    if mt == 2 {
                        nuc.sample_elastic_mu_cm(e, &mut seed)
                    } else {
                        nuc.sample_inelastic_mu_cm(mt, e, &mut seed)
                    }
                    .expect("an AND law")
                })
                .collect();
            let (m, s) = mean_sem(&mus);
            let z = (m - v[0]) / s;
            println!(
                "{case} MT={mt} E={:.2} MeV: <mu> {m:.6} (OpenMC {:.6}) z {z:+.2}",
                e / 1e6,
                v[0]
            );
            assert!(z.abs() < 5.0, "{case} MT={mt} E={e}: z {z}");
            worst = worst.max(z.abs());
            checked += 1;
            continue;
        }
        let law = nuc.continuum_law(mt).expect("a correlated law");
        assert_eq!(law.branches.len(), 1);
        let br = &law.branches[0];
        let draws: Vec<(f64, f64)> = (0..N)
            .map(|_| sample_continuum_branch(br, e, ContinuumAngularMode::Evaluated, &mut seed))
            .collect();
        let mus: Vec<f64> = draws.iter().map(|d| d.1).collect();
        let mues: Vec<f64> = draws.iter().map(|d| d.0 * d.1).collect();
        let (m1, s1) = mean_sem(&mus);
        let (m2, s2) = mean_sem(&mues);
        let (z1, z2) = ((m1 - v[0]) / s1, (m2 - v[1]) / s2);
        let (o1, o2) = ((m1 - v[2]) / s1, (m2 - v[3]) / s2);
        println!(
            "{case} MT={mt} E={:.2} MeV: <mu> {m1:.6} (OpenMC {:.6}, old {:.6}) z {z1:+.2} [old {o1:+.1}]; \
             <mu E'> {m2:.1} (OpenMC {:.1}) z {z2:+.2} [old {o2:+.1}]",
            e / 1e6,
            v[0],
            v[2],
            v[1]
        );
        assert!(
            z1.abs() < 5.0 && z2.abs() < 5.0,
            "{case} MT={mt} E={e}: z {z1} {z2}"
        );
        worst = worst.max(z1.abs()).max(z2.abs());
        let sep = o1.abs().max(o2.abs());
        match power.iter_mut().find(|(c, _)| c == case) {
            Some(p) => p.1 = p.1.max(sep),
            None => power.push((case.to_string(), sep)),
        }
        checked += 1;
    }
    println!(
        "rows checked {checked}, worst |z| {worst:.2}, separation from the old scheme {power:?}"
    );
    for (case, sep) in &power {
        if case != "O16" {
            assert!(
                *sep > 5.0,
                "{case}: the test cannot tell the old scheme apart ({sep:.1} sem)"
            );
        }
    }
    assert!(checked > 0);
}

/// **The whole cosine distribution, not only its moments.** Below ~3 MeV the
/// U-235 and U-238 continuum is nearly isotropic in mean, so a moment test
/// cannot see a shape error where a fast system does most of its continuum
/// scattering. `openmc_inputs/correlated_mu_cdf_reference.py` writes the exact
/// CDF of OpenMC's law-61 cosine (nearer-row rule, each row's Tabular CDF) on
/// 101 points of mu. It covers NJOY2016 U-235 and U-238 MT=91 at 0.6-14 MeV and
/// MT=16 at 8 and 14 MeV. Pass: the KS distance of `N = 1e6` transport-path
/// draws, evaluated on those points, is below `1.95 / sqrt(N)` (alpha = 1e-3)
/// at every energy.
///
/// Results (2026-09-29): 20 energies, worst `D / crit = 0.74`. The **old**
/// scheme (lower row, linear-cdf inverse), put back in place, gives
/// `D / crit` up to 2.20. It fails only at 3 MeV and above (U-235 MT=16 at
/// 14 MeV, `D = 4.29e-3`). At 0.6-2 MeV it is indistinguishable from the new
/// one (`D <= 1.42e-3`), so the change does not reach the energies where most
/// of a fast system's continuum scattering happens.
#[test]
fn correlated_cosine_distribution_matches_openmc() {
    let csv = ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/correlated_mu_cdf_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let root = ws();
    let mut cases: Vec<(String, i32, f64, Vec<(f64, f64)>)> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split(',').collect();
        let (nuc, mt, e) = (f[0].to_string(), f[1].parse::<i32>().unwrap(), f[2].parse::<f64>().unwrap());
        let pt = (f[3].parse::<f64>().unwrap(), f[4].parse::<f64>().unwrap());
        match cases.last_mut() {
            Some(c) if c.0 == nuc && c.1 == mt && c.2 == e => c.3.push(pt),
            _ => cases.push((nuc, mt, e, vec![pt])),
        }
    }
    const M: usize = 1_000_000;
    let crit = 1.95 / (M as f64).sqrt();
    let mut loaded: Vec<(String, Option<Nuclide>)> = Vec::new();
    let mut worst: f64 = 0.0;
    let mut n = 0;
    for (i, (name, mt, e, pts)) in cases.iter().enumerate() {
        if !loaded.iter().any(|(c, _)| c == name) {
            let p = root.join(format!(
                "reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz"
            ));
            let nuc = p.is_file().then(|| Nuclide::from_ace_file(&p, name).expect("loads"));
            loaded.push((name.clone(), nuc));
        }
        let Some(nuc) = loaded.iter().find(|(c, _)| c == name).unwrap().1.as_ref() else {
            continue;
        };
        let br = &nuc.continuum_law(*mt).expect("law").branches[0];
        let mut seed = 0xCDF0_0000 + i as u64;
        let mut mus: Vec<f64> = (0..M)
            .map(|_| sample_continuum_branch(br, *e, ContinuumAngularMode::Evaluated, &mut seed).1)
            .collect();
        mus.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut d: f64 = 0.0;
        for &(mu, f) in pts {
            let emp = mus.partition_point(|&x| x <= mu) as f64 / M as f64;
            d = d.max((emp - f).abs());
        }
        println!("{name} MT={mt} E={:.2} MeV: KS D = {d:.2e} (crit {crit:.2e})", e / 1e6);
        assert!(d < crit, "{name} MT={mt} E={e}: D = {d}");
        worst = worst.max(d / crit);
        n += 1;
    }
    println!("{n} energies, worst D/crit {worst:.2}");
    assert!(n > 0);
}
