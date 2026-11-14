// SPDX-License-Identifier: GPL-3.0

//! **A nuclide read from an OpenMC-format `.h5` is the nuclide the ACE route
//! builds** — GitHub #303, acceptance row 1.
//!
//! # Methodology
//!
//! No OpenMC install and no NJOY library are needed; every file here is one
//! this workspace's own writer produces (`njoy_outram_park_fork::hdf5::
//! nuclide_write`).
//!
//! 1. Take an ENDF tape from `reference-data/endf` (the first light nuclide of `CANDIDATES` whose neutron emission is elastic plus levels, ENDF/B-VIII.0),
//!    RECONR + BROADR to 293.6 K, assemble the ACE table with `acer::build_full`
//!    and build route A, `Nuclide::from_ace` (the oracle).
//! 2. Re-express the **same decoded ACE arrays** as an OpenMC nuclide file: the
//!    energy grid, MT=1 (ESZ's total), MT=2, every reaction from its threshold,
//!    the elastic and level angle tables, the level laws. Write it, read it back
//!    with `read_nuclide`, and build route B, `Nuclide::from_hdf5`.
//! 3. Require the two nuclides to agree **bit for bit** on `xs_at_energy` (every
//!    field, via `Debug`, which prints the shortest string that round-trips an
//!    `f64`, so equal text is equal bits) at every grid point and every
//!    midpoint, and on `reaction_xs` for every MT up to 900.
//!
//! The pass criterion is exact equality: the file stores `f64`s verbatim and
//! `from_hdf5` builds its sections by a copy of `from_ace`'s own code, so any
//! difference is a plumbing defect and not rounding. Prediction, written before
//! the run: **zero differing words**.
//!
//! # What this does not show
//!
//! It shows the conversion; it does not show that OpenMC's *own* library
//! reproduces the ENDF-reconstructed cross sections within reconstruction
//! tolerance (that needs `OUTRAM_OPENMC_XS_DIR`, a ~74 MB generated library, and
//! is `band_xs_from_hdf5_reader.rs`'s job), and U-235 itself is refused by
//! `from_hdf5` because its fission and continuum laws are not decoded.

use njoy_outram_park_fork::acer::ce_decode::{decode_ce, CeNeutronAce};
use njoy_outram_park_fork::acer::ce_laws::decode_angular;
use njoy_outram_park_fork::acer::read::AceFileType;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::hdf5::nuclide_laws::{
    AngleDistribution, AngleEnergy, EnergyDist, Product,
};
use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide;
use njoy_outram_park_fork::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use petir::mathf::RealMath;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

fn angle_of(
    raw: &njoy_outram_park_fork::acer::read::RawAceTable,
    index: usize,
    lct: i32,
    lo: f64,
    hi: f64,
) -> AngleDistribution {
    match decode_angular(raw, index, lct).expect("decode angular") {
        None => AngleDistribution::isotropic(vec![lo, hi]),
        Some(a) => {
            let mut d = AngleDistribution {
                energy: vec![],
                mu: vec![],
                p: vec![],
                interpolation: vec![],
            };
            for e in &a.energies {
                d.energy.push(e.e_mev * 1.0e6);
                if e.cosines.is_empty() {
                    d.mu.push(vec![-1.0, 1.0]);
                    d.p.push(vec![0.5, 0.5]);
                    d.interpolation.push(2);
                } else {
                    d.mu.push(e.cosines.clone());
                    d.p.push(e.pdf.clone());
                    d.interpolation.push(if e.histogram { 1 } else { 2 });
                }
            }
            d
        }
    }
}

/// The ACE arrays as the writer's `NuclideData`.
fn nuclide_data_from_ace(
    raw: &njoy_outram_park_fork::acer::read::RawAceTable,
    ace: &CeNeutronAce,
    with_total: bool,
    name: &str,
) -> Result<NuclideData, String> {
    let (lo, hi) = (ace.energy[0], *ace.energy.last().unwrap());
    let a = ace.awr;
    let mut reactions = Vec::new();
    if with_total {
        reactions.push(ReactionData {
            mt: 1,
            q_value: 0.0,
            center_of_mass: false,
            xs: ace.total.clone(),
            threshold_idx: 0,
            redundant: false,
            products: vec![],
        });
    }
    reactions.push(
        ReactionData::elastic(ace.elastic.clone(), angle_of(raw, 0, 2, lo, hi), lo, hi).unwrap(),
    );
    let sampled = ace.channel_mts();
    for (i, rx) in ace.reactions.iter().enumerate() {
        let mut products = vec![];
        if (51..=90).contains(&rx.mt) {
            let lct = if rx.ty < 0 { 2 } else { 1 };
            let angle = angle_of(raw, i + 1, lct, lo, hi);
            products.push(
                Product::prompt_neutron(
                    AngleEnergy::Uncorrelated {
                        angle: Some(angle),
                        energy: Some(EnergyDist::Level {
                            threshold: (a + 1.0) / a * (-rx.q_value),
                            mass_ratio: (a / (a + 1.0)).r_powi(2),
                        }),
                    },
                    lo,
                    hi,
                )
                .unwrap(),
            );
        } else {
            // A neutron-emitting channel whose law the writer cannot express
            // ((n,2n), MT=5, MT=91 ...) is written WITHOUT its product. That is
            // legitimate for what this test compares, the cross sections, and
            // the converter's refusal of such a channel when the product is
            // present is tested in `nuclide_from_hdf5_fissile.rs`.
            let _ = rx.ty;
        }
        reactions.push(ReactionData {
            mt: rx.mt,
            q_value: rx.q_value,
            center_of_mass: rx.ty < 0,
            xs: rx.xs.clone(),
            threshold_idx: rx.threshold_index as i64,
            // Redundant = not a member of the non-double-counting set that reconstructs the total (`channel_mts`).
            redundant: !sampled.contains(&rx.mt),
            products,
        });
    }
    Ok(NuclideData {
        name: name.into(),
        z: ace.za / 1000,
        a: ace.za % 1000,
        metastable: 0,
        atomic_weight_ratio: ace.awr,
        temperature: "294K".into(),
        kt_ev: ace.kt_ev,
        energy: ace.energy.clone(),
        reactions,
        total_nu: None,
        urr: vec![],
    })
}

/// Light nuclides; their emission laws beyond elastic and the discrete levels
/// are not expressible by the file builder above and are left out of the file. Every one that
/// qualifies is built; the one with the most reactions is compared.
const CANDIDATES: [(&str, &str); 6] = [
    ("n-002_He_004-ENDF8.0.endf", "He4"),
    ("n-014_Si_028-ENDF8.0.endf", "Si28"),
    ("n-012_Mg_024-ENDF8.0.endf", "Mg24"),
    ("n-013_Al_027-ENDF8.0.endf", "Al27"),
    ("n-020_Ca_040-ENDF8.0.endf", "Ca40"),
    ("n-016_S_032-ENDF8.0.endf", "S32"),
];

type Built = (Nuclide, Nuclide, CeNeutronAce, &'static str);

fn build_both(with_total: bool) -> Option<Built> {
    let mut best: Option<Built> = None;
    for (file, name) in CANDIDATES {
        match build_one(file, name, with_total) {
            Ok(Some(r)) => {
                println!("{name}: expressible, {} reactions", r.2.reactions.len());
                if best.as_ref().is_none_or(|b| r.2.reactions.len() > b.2.reactions.len()) {
                    best = Some(r);
                }
            }
            Ok(None) => println!("{file}: tape absent"),
            Err(why) => println!("{name}: not expressible ({why})"),
        }
    }
    best
}

fn build_one(file: &str, name: &'static str, with_total: bool) -> Result<Option<Built>, String> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        return Ok(None);
    };
    let tape = Tape::read_file(&path).expect("parse ENDF");
    let mat = tape.materials()[0];
    let recon0 = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let recon = broaden_result(&recon0, TEMP_K);
    let kt_mev = 8.617_333_262e-5 * TEMP_K * 1.0e-6;
    let table = njoy_outram_park_fork::acer::build_full(&tape, mat, &recon, kt_mev, 0)
        .expect("assemble ACE");
    let raw = table.to_raw(AceFileType::Type1Ascii).expect("raw table");
    let oracle = Nuclide::from_ace(&raw, name).expect("from_ace");

    let ace = decode_ce(&raw).expect("decode_ce");
    let data = nuclide_data_from_ace(&raw, &ace, with_total, name)?;
    let p = std::env::temp_dir().join(format!("{name}_from_hdf5_{with_total}.h5"));
    write_nuclide(&p, &data).expect("write h5");
    let read = read_nuclide(&p).expect("read h5");
    let _ = std::fs::remove_file(&p);
    let converted = Nuclide::from_hdf5(&read, name).expect("from_hdf5");
    Ok(Some((oracle, converted, ace, name)))
}

/// Every `MicroXS` field a cross-section comparison means, as text that is
/// equal exactly when the bits are. **`other` is left out, on purpose:** it is
/// the sum over the "other neutron-emitting reactions" (MT=22, 28, ...), which
/// exist as channels only when their emission law is present, and the file
/// builder above cannot write those laws (so the channels are absent from route
/// B by construction, and a real file carrying them is refused by `from_hdf5`).
fn xs_key(n: &Nuclide, e: f64) -> String {
    let x = n.xs_at_energy(e, TEMP_K);
    format!(
        "{:?}",
        (x.total, x.elastic, x.fission, x.absorption, x.inelastic, x.n2n, x.n3n, x.mt5, x.nu_fission)
    )
}

/// **Bit-identical cross sections** with MT=1 in the file.
///
/// # Results (2026-11-14)
///
/// Printed by the test: number of energies compared and differing words.
#[test]
fn the_h5_route_reproduces_the_ace_route_bit_for_bit() {
    let (a, b, ace, name) = build_both(true).expect("no candidate nuclide qualified");
    let mut energies: Vec<f64> = ace.energy.clone();
    for w in ace.energy.windows(2) {
        energies.push(0.5 * (w[0] + w[1]));
    }
    let mut bad = 0usize;
    for &e in &energies {
        if xs_key(&a, e) != xs_key(&b, e) {
            if bad == 0 {
                println!(
                    "first difference at {e}:\n  ace {:?}\n  h5  {:?}",
                    a.xs_at_energy(e, TEMP_K),
                    b.xs_at_energy(e, TEMP_K)
                );
            }
            bad += 1;
        }
        if format!("{:?}", a.total_at_energy(e, TEMP_K))
            != format!("{:?}", b.total_at_energy(e, TEMP_K))
        {
            bad += 1;
        }
    }
    let mut mts = 0usize;
    // OpenMC's exporter does not write a redundant reaction unless its MT is in
    // `keep_mts` (`neutron.py:423-428`), so such a reaction is absent from the
    // file by design and from route B with it (MT=650 here: a level of the
    // (n,d) family whose lump MT=104 is present). Compared: what the file holds.
    let kept = ace.channel_mts();
    const KEEP_MTS: [i32; 15] = [4, 16, 103, 104, 105, 106, 107, 203, 204, 205, 206, 207, 301, 444, 901];
    for mt in 1..=900 {
        if mt > 2 && !kept.contains(&mt) && !KEEP_MTS.contains(&mt) {
            continue;
        }
        for &e in energies.iter().step_by(37) {
            let (x, y) = (a.reaction_xs(mt, e), b.reaction_xs(mt, e));
            if format!("{x:?}") != format!("{y:?}") {
                if bad < 5 {
                    println!("MT={mt} at {e}: ace {x:?}, h5 {y:?}");
                }
                bad += 1;
            }
        }
    }
    mts += ace.reactions.len() + 2;
    println!(
        "{name}: {} energies, {} MTs present, {} differing words (prediction: 0)",
        energies.len(),
        mts,
        bad
    );
    assert_eq!(bad, 0);
    assert!(mts > 3, "the comparison must cover several reactions");
}

/// **Without MT=1 in the file** (as OpenMC writes it), the total is OpenMC's
/// own sum of the non-redundant reactions and agrees with ESZ's to ACER's
/// rounding of the partials.
///
/// The bound is a derived one: ACE stores 7-8 significant figures, so a sum of
/// up to a few dozen partials differs from the stored total by at most a few
/// units in the last stored figure. `1e-6` relative at every energy where the
/// total is not tiny is the stated criterion, fixed before the run.
#[test]
fn without_mt1_the_total_is_openmcs_sum_of_partials() {
    let (a, b, ace, name) = build_both(false).expect("no candidate nuclide qualified");
    let mut worst = 0.0f64;
    for &e in ace.energy.iter().step_by(11) {
        let (ta, tb) = (a.total_at_energy(e, TEMP_K), b.total_at_energy(e, TEMP_K));
        if ta > 1.0e-3 {
            worst = worst.max(((ta - tb) / ta).abs());
        }
    }
    println!("{name} total, sum-of-partials vs ESZ: worst relative difference {worst:.2e}");
    assert!(worst < 1.0e-6, "worst {worst:.2e}");
}
