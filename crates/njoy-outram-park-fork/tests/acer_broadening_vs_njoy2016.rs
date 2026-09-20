//! Gate: this port's **Doppler broadening** reproduces NJOY2016's.
//!
//! # Why this exists as a separate gate
//!
//! `acer_ce_esz_vs_njoy2016.rs` compares ESZ cross sections at energies both
//! tables happen to share. That gate cannot see broadening **at all**: above
//! `thnmax` neither code runs SIGMA1, so both tables there are the same
//! unbroadened RECONR output, and below `thnmax` the two adaptive grids
//! essentially never coincide. Measured 2026-09-20: of the shared grid points,
//! **zero** lie below `thnmax` on U-234, U-235 or U-238.
//!
//! So a 293.6 K run of that gate reports ~1e-6 agreement while saying nothing
//! whatever about broadening. This file is the instrument that does.
//!
//! # Methodology
//!
//! Both tables are lin-lin by construction, so the integral of each over a
//! fixed energy band is **exact** and depends only on the function the table
//! defines — not on where either code placed its grid points. Equal-lethargy
//! bands, 20 per decade, each integrated on its own grid, restricted to bands
//! lying wholly below `thnmax`.
//!
//! The reference is `reference-data/acer/u235_293k_bands_njoy2016.csv`,
//! dumped once from NJOY2016's own 293.6 K table (2016.79 `ac5adf5`) so this
//! gate does not need the 135 MB reference at test time.
//!
//! # Results (2026-09-20, U-235 ENDF/B-VIII.0 at 293.6 K, 167 bands)
//!
//! | column | worst relative difference |
//! |---|---|
//! | total | 4.393e-4 |
//! | absorption | 4.088e-4 |
//! | elastic | 3.174e-4 |
//!
//! All inside `errthn = 1e-3`, the thinning tolerance each table is written
//! to — so the two codes' broadened cross sections agree to better than either
//! table individually promises.
//!
//! **Runtime: 69 s** (RECONR + BROADR on the 36 MB U-235 tape), measured
//! 2026-09-20 on 4 cores. Under the workspace's 5-minute threshold, so this is
//! ~~deliberately NOT behind `long-tests`~~ **not gated on RUNTIME**.
//!
//! **CHANGED 2026-09-20 (maintainer direction): it IS behind `long-tests` now,
//! on a DATA criterion.** `long-tests` is default-on, so it still runs in the
//! ordinary suite — which
//! is the point: it is the only evidence this port's broadening reproduces
//! NJOY's, and a V&V claim protected by nothing rots. Reading the small
//! committed oracle instead of the 135 MB reference table is what keeps it
//! cheap.
//!
//! **Interpretation.** This is verification against NJOY2016, not validation:
//! it says this port reproduces NJOY's SIGMA1, not that either describes a
//! reactor.

#![cfg(not(target_os = "android"))]

use njoy_outram_park_fork::acer::{integrate_linlin, jxs, nxs, AceTable};
use njoy_outram_park_fork::acer::{angular::parse_elastic_angular, energy::build_emissions};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{build_emission_spectra, Kerma};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::photon::PhotonProduction;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const MAT: i32 = 9228;
const TEMP_K: f64 = 293.6;

/// Gate. Worst measured 2026-09-20 was 4.393e-4; `errthn` is 1e-3.
///
/// DO NOT widen this to absorb a failure. It sits between the measured value
/// and the tolerance the tables are written to, so a breach means the
/// broadening changed, not that the bar is too tight.
const TOL: f64 = 1.0e-3;

struct Band {
    e_lo: f64,
    e_hi: f64,
    njoy: [f64; 3],
}

fn oracle() -> Vec<Band> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reference-data/acer/u235_293k_bands_njoy2016.csv"
    );
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read band oracle {path}: {e}"));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("e_lo_mev") && !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l
                .split(',')
                .map(|t| t.trim().parse().expect("band oracle number"))
                .collect();
            assert_eq!(v.len(), 5, "band oracle row {l:?}");
            Band {
                e_lo: v[0],
                e_hi: v[1],
                njoy: [v[2], v[3], v[4]],
            }
        })
        .collect()
}

fn build_ours() -> Option<AceTable> {
    let path = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (broadening vs NJOY2016)",
    )?;
    let tape = Tape::read(std::fs::File::open(&path).expect("open")).expect("parse ENDF");
    let cfg = ReconrConfig {
        mat: MAT,
        tolerance: 0.001,
        temperature: 0.0,
    };
    let result = reconr(&tape, &cfg).expect("RECONR");
    let result = njoy_outram_park_fork::broadr::broaden_result(&result, TEMP_K);

    let angular = tape
        .section(MAT, 4, 2)
        .map(|s| parse_elastic_angular(s).expect("parse MF=4"));
    let partials: Vec<(i32, f64)> = result
        .sections
        .iter()
        .map(|s| (i32::from(s.mt), s.qi))
        .collect();
    let emissions = build_emissions(&tape, MAT, result.material.awr, &partials);
    let nu = NuBar::from_endf(&tape, MAT)
        .expect("MF=1")
        .unwrap_or_default();
    let chi = FissionSpectrum::from_endf_mf5(&tape, MAT)
        .expect("MF=5")
        .unwrap_or_default();
    let emission = build_emission_spectra(&tape, MAT);
    let photons = PhotonProduction::from_endf(&tape, MAT, &result);
    let kerma =
        Kerma::from_reconr(&result, &nu, &chi, &emission).with_energy_balance(&photons, &result);
    let nu_block = njoy_outram_park_fork::acer::nu::build(&tape, MAT).expect("NU block");
    Some(AceTable::from_reconr_full(
        &result,
        TEMP_K * 8.617_333_262e-5 / 1.0e6,
        0,
        angular.as_ref(),
        &emissions,
        Some(&kerma),
        nu_block.as_deref(),
        njoy_outram_park_fork::acer::has_mt19_distributions(&tape, MAT),
        njoy_outram_park_fork::acer::photon_blocks::build(&tape, MAT).as_deref(),
    ))
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reference-data tier (NJOY2016 oracles); runs by default, skipped under --no-default-features"
)]
fn broadened_band_integrals_match_njoy2016() {
    let bands = oracle();
    assert!(
        bands.len() > 100,
        "band oracle looks truncated: {} rows",
        bands.len()
    );
    let Some(ours) = build_ours() else {
        eprintln!("reference ENDF tape absent — skipping");
        return;
    };

    let nes = ours.nxs[nxs::NES] as usize;
    let eo = (ours.jxs[jxs::ESZ] - 1) as usize;
    let e = &ours.xss[eo..eo + nes];

    let labels = ["total", "absorption", "elastic"];
    let mut worst = [(0.0f64, 0.0f64); 3];
    for b in &bands {
        for col in 0..3 {
            let x = &ours.xss[eo + (col + 1) * nes..eo + (col + 2) * nes];
            let it = b.njoy[col];
            if it == 0.0 {
                continue;
            }
            let io = integrate_linlin(e, x, b.e_lo, b.e_hi);
            let d = ((io - it) / it).abs();
            if d > worst[col].0 {
                worst[col] = (d, b.e_lo);
            }
        }
    }

    for (col, w) in worst.iter().enumerate() {
        println!(
            "broadening vs NJOY2016: {:<11} worst rel {:.3e} at band from {:.4e} MeV over {} bands",
            labels[col],
            w.0,
            w.1,
            bands.len()
        );
    }
    for (col, w) in worst.iter().enumerate() {
        assert!(
            w.0 < TOL,
            "{} band integrals disagree with NJOY2016 by {:.3e} at the band from {:.4e} MeV, \
             over {} bands below thnmax. Worst measured when this was written was 4.393e-4, \
             against an errthn of 1e-3. DO NOT widen this gate: it sits between the measured \
             value and the tolerance the tables are written to, so a breach means the \
             broadening changed.",
            labels[col],
            w.0,
            w.1,
            bands.len()
        );
    }
}
