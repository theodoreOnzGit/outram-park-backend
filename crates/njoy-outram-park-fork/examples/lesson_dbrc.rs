//! **The numbers of the neutronics lesson pages on DBRC and on how the two ACE
//! files are put together** (gh:#906): written to
//! `docs/site/science/neutronics/dbrc-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_dbrc [out.js]
//! ```
//!
//! Everything comes straight out of this crate's own processing path
//! ([`njoy_outram_park_fork::process`]), with no comparison against NJOY2016
//! files (maintainer, 2026-10-18: "just use our port straight"):
//!
//! - **U-238, ENDF/B-VIII.0 (`n-092_U_238.endf`)**: the 0 K elastic
//!   cross section DBRC reads (`NuclearCrossSections::elastic_0k`, i.e. RECONR
//!   at 0 K through the PENDF text rounding) and the same reaction broadened to
//!   293.6 K by BROADR (`NuclearCrossSections::pendf`), 5.8-7.6 eV. PURR and
//!   HEATR are switched off (they cannot change MT=2 in this range).
//! - **H-1 and C-12, 293.6 K**: the continuous-energy ACE tables
//!   (`process::ace`, default settings) decoded with `decode_ce`: NES, NTR,
//!   whether an angular-distribution block is present, and the elastic column
//!   on 0.001-20 eV.
//! - **H in H2O and crystalline graphite, 293.6 K**: the thermal ACE tables
//!   (`process::thermal_ace` on the embedded LEAPR decks, the incident
//!   tapes H-1 and C-12) decoded with `decode_thermal`: the NXS values, the
//!   inelastic cross section (ITIE/ITIX) and, for graphite, the Bragg edges
//!   (ITCE/ITCX); the table's last inelastic energy is the cutoff the
//!   transport code uses (`outram-mc-libs` `ThermalScattering::from_ace`).
//!
//! Driver glue (`main`): every function it calls is tested in the crate.

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::acer::read::parse_type1;
use njoy_outram_park_fork::acer::thermal_read::{decode_thermal, AceThermalElastic};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
use njoy_outram_park_fork::process::{self, CrossSectionSettings, ThermalSettings};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::MtReaction;
use std::fmt::Write as _;
use uom::si::thermodynamic_temperature::kelvin;

fn nums(v: &[f64]) -> String {
    v.iter().map(|x| format!("{x:.9e}")).collect::<Vec<_>>().join(",")
}

fn tape(name: &str) -> Result<Tape, String> {
    let p = reference_endf(name).ok_or(format!("{name} missing"))?;
    Tape::read_file(&p).map_err(|e| e.to_string())
}

/// A pair list restricted to `[lo, hi]` as two JSON arrays.
fn window(p: &[(f64, f64)], lo: f64, hi: f64) -> String {
    let w: Vec<&(f64, f64)> = p.iter().filter(|q| q.0 >= lo && q.0 <= hi).collect();
    let e: Vec<f64> = w.iter().map(|q| q.0).collect();
    let s: Vec<f64> = w.iter().map(|q| q.1).collect();
    format!("{{\"e\":[{}],\"s\":[{}]}}", nums(&e), nums(&s))
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/dbrc-data.js").to_string()
    });
    let t = Temperature::new::<kelvin>(293.6);

    // ── U-238: the 0 K elastic DBRC reads, and the broadened one ────────────
    let u238 = tape("n-092_U_238.endf")?;
    let mut s = CrossSectionSettings::default().without_heatr();
    s.purr = None;
    let xs = process::cross_sections(&u238, t, &s).map_err(|e| e.to_string())?;
    let el0 = xs.elastic_0k.as_ref().ok_or("no 0 K elastic")?;
    let el_t = xs.pendf.sections.iter().find(|q| q.mt == MtReaction::Mt2Elastic).ok_or("no MT=2")?;
    let awr = xs.pendf.material.awr;
    println!("U-238: AWR {awr}, 0 K elastic {} points in total, {} broadened", el0.len(), el_t.pairs.len());
    let (w_lo, w_hi) = (5.8, 7.6);
    let u = format!(
        "{{\"mat\":{},\"awr\":{awr},\"n0k\":{},\"nT\":{},\"el0k\":{},\"elT\":{}}}",
        xs.mat,
        el0.len(),
        el_t.pairs.len(),
        window(el0, w_lo, w_hi),
        window(&el_t.pairs, w_lo, w_hi)
    );

    // ── H-1 and C-12: the continuous-energy tables ─────────────────────────
    let mut ce = String::new();
    for (name, file) in [("H-1", "n-001_H_001-ENDF8.0-Beta6.endf"), ("C-12", "n-006_C_012-ENDF8.0.endf")] {
        let tp = tape(file)?;
        let a = process::ace(&tp, t, &CrossSectionSettings::default()).map_err(|e| e.to_string())?;
        let raw = parse_type1(&a.to_type1_text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let d = decode_ce(&raw).map_err(|e| e.to_string())?;
        use njoy_outram_park_fork::acer::{jxs, nxs};
        let pairs: Vec<(f64, f64)> = d.energy.iter().copied().zip(d.elastic.iter().copied()).collect();
        println!(
            "{name} CE ACE {}: NES {}, NTR {}, kT {:.6e} eV, LAND {}, AND {}",
            d.zaid, raw.nxs[nxs::NES], raw.nxs[nxs::NTR], d.kt_ev, raw.jxs[jxs::LAND], raw.jxs[jxs::AND]
        );
        let _ = write!(
            ce,
            "\"{name}\":{{\"zaid\":\"{}\",\"nes\":{},\"ntr\":{},\"kt\":{},\"awr\":{},\"land\":{},\"and\":{},\"xss\":{},\"elastic\":{}}},",
            d.zaid,
            raw.nxs[nxs::NES],
            raw.nxs[nxs::NTR],
            d.kt_ev,
            d.awr,
            raw.jxs[jxs::LAND],
            raw.jxs[jxs::AND],
            raw.xss.len(),
            window(&pairs, 1.0e-3, 20.0)
        );
    }
    ce.pop();

    // ── H in H2O and graphite: the thermal tables ──────────────────────────
    let mut th = String::new();
    for (name, mat, file, tname) in [
        ("H in H2O", SabMaterial::HInH2O, "n-001_H_001-ENDF8.0-Beta6.endf", "hh2o"),
        ("graphite", SabMaterial::CrystallineGraphite, "n-006_C_012-ENDF8.0.endf", "gr"),
    ] {
        let deck = embedded_deck_text(mat).ok_or("no embedded deck")?;
        let tp = tape(file)?;
        let settings = ThermalSettings::default().with_table_name(tname);
        let a = process::thermal_ace(&tp, deck, t, &settings).map_err(|e| e.to_string())?;
        let raw = parse_type1(&a.to_type1_text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let d = decode_thermal(&raw).map_err(|e| e.to_string())?;
        use njoy_outram_park_fork::acer::thermal::{jxs, nxs};
        let _ = jxs::ITIE;
        let cutoff = *d.inel_energy.last().ok_or("no inelastic grid")?;
        let (kind, ee, cc) = match &d.elastic {
            AceThermalElastic::None => ("none", vec![], vec![]),
            AceThermalElastic::Coherent { energy, cumulative } => ("coherent", energy.clone(), cumulative.clone()),
            _ => ("other", vec![], vec![]),
        };
        println!(
            "{name} thermal ACE {}: IDPNI {}, NIL {}, NIEB {}, IDPNC {}, NCL {}, IFENG {}; {} inelastic energies to {cutoff:.6} eV; elastic {kind} with {} edges; XSS {} words",
            a.zaid, raw.nxs[nxs::IDPNI], raw.nxs[nxs::NIL], raw.nxs[nxs::NIEB], raw.nxs[nxs::IDPNC], raw.nxs[nxs::NCL],
            raw.nxs[nxs::IFENG], d.inel_energy.len(), ee.len(), raw.xss.len()
        );
        let _ = write!(
            th,
            "\"{name}\":{{\"zaid\":\"{}\",\"idpni\":{},\"nil\":{},\"nieb\":{},\"idpnc\":{},\"ncl\":{},\"ifeng\":{},\"kt\":{},\"cutoff\":{cutoff},\"xss\":{},\"einel\":[{}],\"sinel\":[{}],\"elastic\":\"{kind}\",\"edges\":[{}],\"cum\":[{}]}},",
            a.zaid,
            raw.nxs[nxs::IDPNI],
            raw.nxs[nxs::NIL],
            raw.nxs[nxs::NIEB],
            raw.nxs[nxs::IDPNC],
            raw.nxs[nxs::NCL],
            raw.nxs[nxs::IFENG],
            d.kt_ev,
            raw.xss.len(),
            nums(&d.inel_energy),
            nums(&d.inel_xs),
            nums(&ee),
            nums(&cc)
        );
    }
    th.pop();

    let file = format!(
        "// Numbers for the neutronics lessons on DBRC and the two ACE files (gh:#906),\n\
         // written by crates/njoy-outram-park-fork/examples/lesson_dbrc.rs (regenerate with it;\n\
         // do not edit). Generated 2026-10-18 by OUTRAM PARK's own processing path, 293.6 K,\n\
         // ENDF/B-VIII.0 tapes n-092_U_238.endf, n-001_H_001-ENDF8.0-Beta6.endf,\n\
         // n-006_C_012-ENDF8.0.endf and the embedded LEAPR decks tsl-HinH2O and\n\
         // tsl-crystalline-graphite (ENDF/B-VIII.0 thermal sublibrary). Energies in eV, cross\n\
         // sections in barn. u238: el0k = process::cross_sections().elastic_0k (RECONR at 0 K,\n\
         // PENDF text rounding, what DBRC reads), elT = the BROADR-broadened MT=2, both on\n\
         // 5.8-7.6 eV; n0k/nT count the points of the whole tables. ce: process::ace decoded\n\
         // with decode_ce (nes, ntr, land/and = JXS locators, xss = table length; elastic =\n\
         // the ESZ elastic column on 1e-3..20 eV). th: process::thermal_ace decoded with\n\
         // decode_thermal (nxs values; cutoff = last ITIE energy; einel/sinel = ITIE/ITIX per\n\
         // principal atom; edges/cum = ITCE/ITCX Bragg edges and cumulative S*E [eV b]).\n\
         window.DBRC_DATA = {{\"u238\":{u},\"ce\":{{{ce}}},\"th\":{{{th}}}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
