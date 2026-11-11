// SPDX-License-Identifier: GPL-3.0

//! **The displacement-energy override of `make_ace` against NJOY2016**
//! (OpenMC #4022, `openmc/data/njoy.py` at `8f79d9d0`; GitHub #1456 part 1).
//!
//! # What was ported
//!
//! OpenMC's `make_ace(displacement_energy=ed)` writes `{mat} 4 0 0 0 0 {ed}/`
//! as HEATR's card 2 (`ed` the seventh field, `0` when the argument is `None`),
//! so HEATR's damage-energy production (MT=444..447, partial kerma 444) uses
//! the caller's threshold displacement energy instead of NJOY's per-element
//! table. HEATR itself already honoured `ed` (`HeatrInput::ed`, verified byte
//! for byte by `heatr_driver_vs_njoy2016.rs`, case `fe58-userq-ed`); what the
//! pipeline lacked was the *route* from the process settings to that card. Now
//! `CrossSectionSettings::with_displacement_energy(ed)` ->
//! `AceDeck::with_displacement_energy` -> `heatr_kerma_with_partials_ed`, and
//! the result is `NuclearCrossSections::damage_energy` (MT=444, eV b).
//!
//! # Methodology
//!
//! - **Oracle:** NJOY2016 `ac5adf5f33` run on the committed decks in
//!   `reference-data/heatr/pipeline_ed/` (regenerate with `regenerate.sh`):
//!   RECONR (`err = 0.001`), BROADR at 293.6 K, HEATR reading the ASCII PENDF
//!   with `ed` on card 2, asking for partial 444. The MF=3/MT=444 section of
//!   its tape is the reference text (`<case>.mt444.txt`).
//! - **Ours:** `process::cross_sections(tape, 293.6 K, default.without_purr()
//!   .with_displacement_energy(ed))`, `damage_energy`.
//! - **Criterion:** every energy and every `h` value bit-identical to the
//!   oracle's, parsed from its 11-column text. No tolerance.
//! - **Controls:** the override must *change* the result (`ed = 33` against the
//!   built-in table for Al-27, whose default is 27 eV), the default settings must
//!   carry no `damage_energy`, `ed = 0` is NJOY's table, and MT=301 must be
//!   identical with and without the override.
//!
//! # Results (2026-11-11, rustc 1.99.1)
//!
//! See the assertions' output with `--nocapture`; recorded in
//! `verification_and_validation/` of this crate (`heatr_vs_njoy2016.md`, the
//! #4022 section).

use njoy_outram_park_fork::endf::parse::parse_endf_float;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

/// The `(E, h)` pairs of an ENDF MF=3 section's text: three header lines (HEAD,
/// TAB1 control, one interpolation line) then six 11-column fields per line.
fn section_pairs(text: &str) -> Vec<(f64, f64)> {
    let mut v = Vec::new();
    for line in text.lines().skip(3) {
        let line = &line[..66.min(line.len())];
        let mut f = Vec::new();
        for k in 0..6 {
            let s = line.get(11 * k..11 * (k + 1)).unwrap_or("");
            if s.trim().is_empty() {
                break;
            }
            f.push(parse_endf_float(s).expect("endf float"));
        }
        for p in f.chunks(2) {
            if p.len() == 2 {
                v.push((p[0], p[1]));
            }
        }
    }
    v
}

fn run(endf: &Tape, ed: Option<f64>) -> process::NuclearCrossSections {
    let mut s = CrossSectionSettings::default().without_purr();
    if let Some(ed) = ed {
        s = s.with_displacement_energy(ed);
    }
    process::cross_sections(endf, Temperature::new::<kelvin>(293.6), &s).expect("cross_sections")
}

fn check(case: &str, endf_name: &str, ed: f64) -> Option<Vec<(f64, f64)>> {
    let endf_path = reference_endf_or_skip(endf_name, case)?;
    let ref_path = reference_file_or_skip("heatr/pipeline_ed", &format!("{case}.mt444.txt"), case)?;
    let endf = Tape::read_file(&endf_path).expect("endf");
    let theirs = section_pairs(&std::fs::read_to_string(ref_path).expect("oracle"));
    let ours_xs = run(&endf, Some(ed));
    let ours = ours_xs.damage_energy.as_ref().expect("damage energy requested");
    assert_eq!(ours.energy.len(), theirs.len(), "[{case}] point counts differ");
    let mut identical = 0usize;
    for (i, ((e, h), (te, th))) in ours.energy.iter().zip(&ours.h).zip(&theirs).enumerate() {
        assert_eq!(e.to_bits(), te.to_bits(), "[{case}] energy {i}: ours {e:e}, NJOY {te:e}");
        assert_eq!(h.to_bits(), th.to_bits(), "[{case}] h {i} at {e:e} eV: ours {h:e}, NJOY {th:e}");
        identical += 1;
    }
    println!("[{case}] ed = {ed} eV: {identical}/{} (E, MT=444) pairs bit-identical to NJOY2016", theirs.len());
    Some(theirs)
}

/// **Function-by-function:** NJOY's built-in table (`ed = 0`) and an override
/// reach HEATR's card 2 through the process settings, Al-27 and Si-28.
#[test]
fn displacement_energy_reaches_heatr_like_openmc_make_ace() {
    let a0 = check("al27-ed0", "n-013_Al_027-ENDF8.0.endf", 0.0);
    let a33 = check("al27-ed33", "n-013_Al_027-ENDF8.0.endf", 33.0);
    let _ = check("si28-ed25", "n-014_Si_028-ENDF8.0.endf", 25.0);
    if let (Some(a0), Some(a33)) = (a0, a33) {
        // The override must be visible: 33 eV against the 27 eV of the table.
        assert_ne!(a0, a33, "ed = 33 changed nothing: the override is not reaching HEATR");
        let (s0, s33): (f64, f64) = (a0.iter().map(|p| p.1).sum(), a33.iter().map(|p| p.1).sum());
        println!("Al-27 sum of MT=444: ed 0 (table, 27 eV) {s0:.6e}, ed 33 {s33:.6e}");
        assert!(s33 < s0, "a higher threshold displaces less: {s33:e} vs {s0:e}");
    }
}

/// **Integration, defaults and controls:** the default settings request no
/// damage energy, the override changes nothing but `damage_energy` (MT=301
/// identical), and the ACE table is the same bytes.
#[test]
fn override_leaves_everything_else_alone() {
    let Some(p) = reference_endf_or_skip("n-013_Al_027-ENDF8.0.endf", "override_leaves_everything_else_alone")
    else {
        return;
    };
    let endf = Tape::read_file(&p).expect("endf");
    let plain = run(&endf, None);
    let with = run(&endf, Some(33.0));
    assert!(plain.damage_energy.is_none(), "default settings must not ask for damage energy");
    let (a, b) = (plain.heating.as_ref().unwrap(), with.heating.as_ref().unwrap());
    assert_eq!(a.energy.len(), b.energy.len());
    assert!(a.h.iter().zip(&b.h).all(|(x, y)| x.to_bits() == y.to_bits()), "MT=301 must not depend on ed");
    let (ta, tb) = (plain.to_ace().expect("ace"), with.to_ace().expect("ace"));
    assert_eq!(ta.to_type2_bytes(), tb.to_type2_bytes(), "the ACE table must not depend on ed");
}
