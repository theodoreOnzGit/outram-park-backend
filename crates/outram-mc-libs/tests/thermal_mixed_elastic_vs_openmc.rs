// SPDX-License-Identifier: GPL-3.0

//! **Mixed coherent + incoherent thermal elastic (ACE IDPNC = 5) is read and
//! sampled as OpenMC does** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! An IDPNC = 5 table was decoded as if it were incoherent: its coherent
//! Bragg data (cumulative `S*E`) was read as an incoherent cross section in
//! barns, and the incoherent ITCEI/ITCXI/ITCAI blocks were never read. The
//! first audit chunk made it a named refusal. It is now read in full
//! (`AceThermalElastic::Mixed`) and sampled as OpenMC's `MixedElasticAE`:
//! coherent with probability `sigma_coh/(sigma_coh + sigma_inc)`, else
//! incoherent.
//!
//! # Methodology
//!
//! No ENDF/B-VIII.0 thermal evaluation is LTHR = 3. All 34 TSL tapes in
//! `~/Documents/research/endf_8/ENDF-B-VIII.0_thermal_scatt` were scanned on
//! 2026-09-29, so NJOY cannot build one from held data.
//! `verification_and_validation/ace_route_physics/openmc_inputs/thermal_mixed_elastic_reference.py`
//! therefore **constructs** a format-exact one. It takes NJOY2016 C in
//! crystalline graphite (coherent, deck `target/ace_extra/Cgraph/input`) and
//! appends H in ZrH's incoherent-elastic block (deck
//! `target/ace_extra/HZrH/input`) as ITCEI/ITCXI/ITCAI, writing
//! `target/ace_extra/mixed_elastic.ace`. The physics of that combination is
//! artificial; the layout is ACE's.
//!
//! OpenMC's reader of that file gives the elastic cross section and the exact
//! `<mu>`, `<mu^2>` of its sampling at six energies
//! (`data/thermal_mixed_elastic_openmc.csv`). Pass criteria:
//! - `ThermalScattering::elastic_xs` equals OpenMC to 1e-9 relative;
//! - with `N = 1e6` draws of the elastic channel, both moments lie within 5
//!   sample-sem of the reference.
//!
//! The test skips when the constructed file is absent.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

const N: usize = 1_000_000;

#[test]
fn mixed_elastic_matches_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ace = root.join("../../target/ace_extra/mixed_elastic.ace");
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&ace).expect("read");
    let th = ThermalScattering::from_ace(&raw, "mixed").expect("IDPNC = 5 must load");
    assert_eq!(
        th.elastic().kind_name(),
        "mixed coherent + incoherent elastic"
    );
    let csv = root.join(
        "verification_and_validation/ace_route_physics/data/thermal_mixed_elastic_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, sig, m1w, m2w) = (v[0], v[1], v[2], v[3]);
        let xs = th.elastic_xs(e);
        let rel = ((xs - sig) / sig).abs();
        let mut seed = 0x31CE_0000 + rows as u64;
        let mus: Vec<f64> = (0..N)
            .map(|_| th.elastic().sample(e, &mut seed).expect("elastic").1)
            .collect();
        let stat = |g: &dyn Fn(f64) -> f64| {
            let m = mus.iter().map(|&x| g(x)).sum::<f64>() / N as f64;
            let var = mus.iter().map(|&x| (g(x) - m).powi(2)).sum::<f64>() / (N - 1) as f64;
            (m, (var / N as f64).sqrt())
        };
        let (m1, s1) = stat(&|x| x);
        let (m2, s2) = stat(&|x| x * x);
        let (z1, z2) = ((m1 - m1w) / s1, (m2 - m2w) / s2);
        println!(
            "E = {e} eV: sigma {xs:.6} (OpenMC {sig:.6}, rel {rel:.1e}); <mu> {m1:.5} (OpenMC {m1w:.5}, z {z1:+.2}); <mu^2> z {z2:+.2}"
        );
        assert!(rel < 1.0e-9, "E = {e}: sigma {xs} vs {sig}");
        assert!(z1.abs() < 5.0 && z2.abs() < 5.0, "E = {e}: z = {z1}, {z2}");
        rows += 1;
    }
    assert_eq!(rows, 6);
}
