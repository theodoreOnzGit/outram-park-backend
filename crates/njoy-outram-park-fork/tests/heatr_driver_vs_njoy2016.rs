// SPDX-License-Identifier: GPL-3.0

//! **HEATR as a whole against NJOY2016's HEATR, byte for byte** (GitHub
//! #535).
//!
//! `heatr::heatr` is the translation of all of `heatr.f90` (audit:
//! `verification_and_validation/heatr_upstream_audit.md`). This test runs it
//! on NJOY2016's own PENDF and compares the **entire** output tape, every
//! byte of every line, with NJOY2016's, and the `viewr` plot file where the
//! deck asks for one.
//!
//! # Methodology
//!
//! - **Oracle:** NJOY2016 `ac5adf5f33`, built from source with gfortran, run
//!   on the committed decks in `reference-data/heatr/driver/` (regenerate
//!   with `regenerate.sh` there). Each deck runs RECONR (`err = 0.001`, 0 K),
//!   writes the PENDF in ASCII (`moder -22 32`), runs **HEATR reading that
//!   ASCII PENDF** (`nin = 32`), and passes HEATR's tape through MODER
//!   (`moder 33 34`).
//! - **Ours:** `heatr(endf, pendf, input)` on the same ENDF tape and NJOY's
//!   ASCII PENDF, with the cards read from the deck by
//!   `HeatrInput::from_cards`; the tape is written with `Tape::write`.
//! - **Why the ASCII PENDF on both sides.** Fed RECONR's binary tape, NJOY's
//!   HEATR sees energies the 7-figure text cannot carry, and the two then
//!   differ at reaction thresholds where a grid energy and a threshold agree
//!   to the last bit in one and not the other (measured: Si-28 MT=304, one
//!   point at 1.843139 MeV). Identical input is the only fair test of HEATR.
//! - **Why MODER.** HEATR writes its tape's sequence numbers continuously
//!   across sections; MODER restarts them per section, which is what
//!   `Tape::write` reproduces (GitHub #553). Columns 1-75 are what HEATR
//!   computes; the MODER pass makes the comparison cover columns 76-80 too.
//! - **Criterion:** every line identical. There is no tolerance. HEATR's
//!   listing (NJOY's `nsyso`, from the `heatr...` banner to the next module)
//!   is compared too, line for line, with only the CPU-time fields replaced.
//!
//! The cases were chosen to cover the code paths:
//!
//! | case | what it exercises |
//! |---|---|
//! | `si28-local0` | `local = 0`, 10 partial MTs: MF=12 capture energy balance with recoil, MF=6 levels with generated recoil, damage 444-447 |
//! | `si28-kchk-plot` | `iprint = 2`: the kinematic limits, MT=443, the photon-production check, the `viewr` plot file |
//! | `fe58-userq-ed` | user Q values (`nqa`), an energy-dependent Q (`qbar`) for capture in `nheat` and `gheat`, a user displacement energy (`ed`), MF=6 capture photons |
//! | `si28-two-temperatures` | a BROADR tape with two temperatures (`ntemp = 0`): `hinit`/`nheat`/`gheat`/`hout` per temperature |
//! | `h2-local0` | MF=6 LAW=6 (n-body phase space) through `h6cm`/`h6psp` |
//! | `li6-local1` | `local = 1`, charged-particle levels MT=600-849 with the MT=103-107 skip, partials 401/403/405 |
//! | `be9-local0` | MF=6 LAW=7 (laboratory angle-energy) |
//! | `o16-local0` | MF=6 Kalbach-Mann (`bacha`) |
//!
//! # Results (2026-10-05)
//!
//! All eight cases are **identical in every byte**: the output tapes (Be-9
//! 2 875 lines, H-2 2 949, Li-6 4 984, O-16 21 947, Si-28 57 856 and 50 946,
//! Si-28 at two temperatures 73 230, Fe-58 83 066) and the 13 850-line plot
//! file. The whole test runs in 3.7 s.
//!
//! The same instrument (`examples/heatr_driver_vs_njoy.rs`) was run over
//! **every one of the 62 neutron evaluations in `reference-data/endf/`**
//! with 11 partial MTs at `local = 0`, including U-234, U-235 (4 312 867
//! lines for ENDF/B-VIII.0), three U-238 evaluations and Pu-239: all 62
//! output tapes are byte-identical. Those tapes are too large to commit;
//! the record is `verification_and_validation/heatr_vs_njoy2016.md` §6.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{heatr, HeatrInput};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

/// The HEATR cards of a committed deck (the lines after `heatr`, up to the
/// next module card).
fn heatr_cards(deck: &str) -> String {
    let lines: Vec<&str> = deck.lines().filter(|l| !l.starts_with('#')).collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == "heatr")
        .expect("deck has a heatr card")
        + 1;
    lines[start..]
        .iter()
        .take_while(|l| l.trim() != "moder" && l.trim() != "stop")
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn check(case: &str) {
    let Some(deck_path) =
        reference_file_or_skip("heatr/driver", &format!("{case}.njoy-input"), case)
    else {
        return;
    };
    let deck = std::fs::read_to_string(&deck_path).expect("deck");
    let endf_name = deck
        .lines()
        .find_map(|l| l.strip_prefix("# endf: "))
        .expect("deck names its ENDF tape")
        .trim()
        .to_string();
    let Some(endf_path) = reference_endf_or_skip(&endf_name, case) else {
        return;
    };
    let Some(pendf_path) =
        reference_file_or_skip("heatr/driver", &format!("{case}.pendf.gz"), case)
    else {
        return;
    };
    let Some(theirs_path) =
        reference_file_or_skip("heatr/driver", &format!("{case}.heatr.gz"), case)
    else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("endf");
    let pendf = Tape::read_file(&pendf_path).expect("pendf");
    let (_, input) = HeatrInput::from_cards(&heatr_cards(&deck)).expect("cards");
    let out = heatr(&endf, &pendf, &input).unwrap_or_else(|e| panic!("[{case}] heatr failed: {e}"));
    let mut buf = Vec::new();
    out.tape.write(&mut buf).expect("write");
    let ours = String::from_utf8(buf).expect("utf8");

    // NJOY's tape, read as bytes (inflated, not re-written).
    let gz = std::fs::read(&theirs_path).expect("oracle");
    let theirs = inflate(&gz, &theirs_path);
    let ol: Vec<&str> = ours.lines().collect();
    let tl: Vec<&str> = theirs.lines().collect();
    let first = ol.iter().zip(tl.iter()).position(|(a, b)| a != b);
    println!(
        "[{case}] ours {} lines, NJOY {} lines, first difference {first:?}",
        ol.len(),
        tl.len()
    );
    if let Some(i) = first {
        panic!(
            "[{case}] line {}:\n  ours |{}|\n  njoy |{}|",
            i + 1,
            ol[i],
            tl[i]
        );
    }
    assert_eq!(ol.len(), tl.len(), "[{case}] line counts differ");

    if let Some(list_path) = njoy_outram_park_fork::reference_data::reference_file(
        "heatr/driver",
        &format!("{case}.listing.gz"),
    ) {
        let theirs_list = inflate(&std::fs::read(&list_path).expect("listing"), &list_path);
        // NJOY's extract ends with the blank line that opens the next
        // module's banner (`(/' moder...')`); trailing blanks are not HEATR's.
        let trimmed = |mut v: Vec<String>| {
            while v.last().is_some_and(|l| l.is_empty()) {
                v.pop();
            }
            v
        };
        let t: Vec<String> = trimmed(theirs_list.lines().map(untimed).collect());
        let o: Vec<String> = trimmed(
            out.listing
                .lines()
                .skip_while(|l| l.trim().is_empty())
                .map(untimed)
                .collect(),
        );
        let first = o.iter().zip(t.iter()).position(|(a, b)| a != b);
        println!(
            "[{case}] listing: ours {} lines, NJOY {} lines, first difference {first:?}",
            o.len(),
            t.len()
        );
        if let Some(i) = first {
            panic!(
                "[{case}] listing line {}:\n  ours |{}|\n  njoy |{}|",
                i + 1,
                o[i],
                t[i]
            );
        }
        assert_eq!(o.len(), t.len(), "[{case}] listing line counts differ");
    }

    if let Some(plot_path) = njoy_outram_park_fork::reference_data::reference_file(
        "heatr/driver",
        &format!("{case}.plot.gz"),
    ) {
        let theirs_plot = inflate(&std::fs::read(&plot_path).expect("plot"), &plot_path);
        let ours_plot = out.plot.clone().expect("deck asked for a plot file");
        let first = ours_plot
            .lines()
            .zip(theirs_plot.lines())
            .position(|(a, b)| a != b);
        println!(
            "[{case}] plot: ours {} lines, NJOY {} lines, first difference {first:?}",
            ours_plot.lines().count(),
            theirs_plot.lines().count()
        );
        assert!(
            first.is_none(),
            "[{case}] plot files differ at line {:?}",
            first.map(|i| i + 1)
        );
        assert_eq!(
            ours_plot.lines().count(),
            theirs_plot.lines().count(),
            "[{case}] plot line counts differ"
        );
    }
}

/// A listing line with its timing field (`f8.1,'s'` at the end of the
/// banner and `temp` lines) replaced, and trailing blanks dropped.
fn untimed(line: &str) -> String {
    let t = line.trim_end();
    if let Some(stripped) = t.strip_suffix('s') {
        let head = stripped.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        if head.len() < stripped.len() && stripped[head.len()..].contains('.') {
            return format!("{}T", head.trim_end());
        }
    }
    t.to_string()
}

/// Inflate a gzip file to text with the crate's own decoder (through a
/// one-section tape would re-format it; the oracle must stay NJOY's bytes).
fn inflate(gz: &[u8], path: &std::path::Path) -> String {
    let bytes = njoy_outram_park_fork::acer::read::inflate_gzip(gz, path).expect("gunzip");
    String::from_utf8(bytes).expect("utf8")
}

#[test]
fn si28_local0() {
    check("si28-local0");
}

#[test]
fn si28_kchk_plot() {
    check("si28-kchk-plot");
}

#[test]
fn fe58_userq_ed() {
    check("fe58-userq-ed");
}

#[test]
fn si28_two_temperatures() {
    check("si28-two-temperatures");
}

#[test]
fn h2_local0() {
    check("h2-local0");
}

#[test]
fn li6_local1() {
    check("li6-local1");
}

#[test]
fn be9_local0() {
    check("be9-local0");
}

#[test]
fn o16_local0() {
    check("o16-local0");
}
