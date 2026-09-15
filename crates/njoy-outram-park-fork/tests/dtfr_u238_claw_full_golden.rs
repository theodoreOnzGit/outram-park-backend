//! DTFR full-channel CLAW tables vs NJOY2016 — `nu*sigma_f`, `chi` (prompt
//! matrix + constant spectrum + delayed), `chid`, `nud`, the P1 (`l=1`)
//! neutron table, the photon-production (`n-p`) table, the edit columns,
//! and the self-shielding factors `ffis`/`fcap` at a finite dilution.
//!
//! # Oracle
//! `reference-data/dtfr/u238-29g-claw-full.njoy-input` (NJOY2016 `ac5adf5`,
//! 2026-09-10): GROUPR on NJOY's own 293.6 K U-238 PENDF — 29 groups,
//! `iwt = 3`, six sigma-zero values, **`lord = 1`**, LANL 12-group photon
//! structure (`igg = 3`), reactions `3/1 2 4 16 18 102 452 455 456`,
//! `6/2`, `6/18` (the fission matrix with its `ig = 0` constant-spectrum
//! record and `ig2lo = 0` rows below `econst`), `5/455` (six delayed
//! spectra), `16/51`, `16/18` — MODER to binary — DTFR in CLAW mode
//! (`0 0 1 / 2 29 / 1 12 /`) for two material cards: `'u238' 9237 1 293.6`
//! (sigma-zero index 1, infinite dilution) and `'u238s' 9237 6 293.6`
//! (index 6 = 1 b, so `ffis`/`fcap` scale the fission and photon channels).
//! The ASCII GENDF (`…-lord1-full.gendf`) and the CLAW file
//! (`u238-29g-claw-full.dtf`, 914 lines) are committed.
//!
//! # What upstream does, read before measuring
//! - The `ig = 0` spectrum record of MF=6/MT=18 carries `nl*nz*ng2 = 174`
//!   words but `dtfr` takes `spect(jg)` from its first 29 raw words
//!   (`:451-456`); the constant-spectrum rows' production word is
//!   `a(lz+il+nl*(jz-1)+jz)` (`:471`). Both are replicated literally.
//! - The `nusf` edit column is position `iptotl-1` (the matrix-accumulated
//!   `nu*sigma_f`), not the MT=452 edit; `totl` is `iptotl` (`:826-827`).
//! - `chi` is normalised by `cnorm` only in the `l=0` pass (`:559-565`).
//!
//! **Prediction stated before running:** every printed number within 2e-5
//! relative (the `1PE12.5` six-figure print) for both material cards.
//!
//! **Result:** see the printed summary / assertions.

use njoy_outram_park_fork::dtfr::assemble::assemble_tables;
use njoy_outram_park_fork::dtfr::input::{DtfrInput, CLAW_HMTID};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const MAT: i32 = 9237;
const NG: usize = 29;
const NGP: usize = 12;
const TOL: f64 = 2e-5;
const ABS_FLOOR: f64 = 1e-12;

/// Parse a block of `6E12.5` lines (`per_line` values each) into `n` values.
fn parse_block(lines: &[&str], n: usize) -> Vec<f64> {
    let mut v = Vec::with_capacity(n + 6);
    for line in lines {
        if v.len() >= n {
            break;
        }
        for k in 0..6 {
            v.push(line[12 * k..12 * (k + 1)].trim().parse::<f64>().unwrap());
        }
    }
    v.truncate(n);
    v
}

struct Worst {
    rel: f64,
    what: String,
    n: usize,
    nonzero: usize,
}

impl Worst {
    fn new() -> Self {
        Worst {
            rel: 0.0,
            what: String::new(),
            n: 0,
            nonzero: 0,
        }
    }
    fn update(&mut self, got: f64, want: f64, what: impl Fn() -> String) {
        self.n += 1;
        if want != 0.0 {
            self.nonzero += 1;
        }
        let scale = got.abs().max(want.abs());
        if scale <= ABS_FLOOR {
            return;
        }
        let rel = (got - want).abs() / scale;
        if rel > self.rel {
            self.rel = rel;
            self.what = format!("{} (crate {got:.6e}, njoy {want:.6e})", what());
        }
    }
}

#[test]
fn dtfr_u238_claw_full_tables_match_njoy() {
    let tag = "dtfr-u238-claw-full";
    let Some(gendf) = reference_file_or_skip(
        "dtfr",
        "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord1-full.gendf",
        tag,
    ) else {
        return;
    };
    let Some(dtf) = reference_file_or_skip("dtfr", "u238-29g-claw-full.dtf", tag) else {
        return;
    };
    let text = std::fs::read_to_string(dtf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let tape = Tape::read_file(&gendf).expect("GENDF parses");
    let mut deck = DtfrInput::claw_defaults(2, NG as i32);
    deck.nptabl = 1;
    deck.ngp = NGP as i32;
    let iptotl = deck.neutron.iptotl;
    let itabl = deck.neutron.itabl;
    let nj = CLAW_HMTID.iter().position(|n| n.trim() == "totl").unwrap() as i32 + 1;
    let per_col = NG.div_ceil(6);

    // Locate the two material blocks by their `edit xsec` headers.
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("edit xsec"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(starts.len(), 2, "two material cards");
    let mut worst_overall = 0.0f64;
    for (m, (&start, jz)) in starts.iter().zip([1i32, 6]).enumerate() {
        let end = if m + 1 < starts.len() {
            starts[m + 1]
        } else {
            lines.len()
        };
        let block = &lines[start..end];
        let tables = assemble_tables(&tape, MAT, &deck, jz).expect("assemble_tables");
        assert_eq!(tables.neutron.len(), 2);
        assert_eq!(tables.photon.len(), 1);
        assert!(tables.has_fission);
        let p0 = &tables.neutron[0];

        // Edit block: labelled columns until the first n-n table header.
        let nn0 = block
            .iter()
            .position(|l| l.contains("l=0 n-n table"))
            .unwrap();
        let mut w_edit = Worst::new();
        let mut cols = 0;
        let mut k = 1;
        while k + per_col <= nn0 {
            let label = block[k][74..79].to_string();
            let j = CLAW_HMTID
                .iter()
                .position(|n| *n == label)
                .unwrap_or_else(|| panic!("unknown edit label {label:?}"))
                as i32
                + 1;
            let jpos = if j == nj {
                iptotl
            } else if j == nj - 1 {
                iptotl - 1
            } else {
                j
            };
            let vals = parse_block(&block[k..k + per_col], NG);
            for (g, &want) in vals.iter().enumerate() {
                let got = p0.get(jpos, g as i32 + 1);
                w_edit.update(got, want, || {
                    format!("edit {} group {}", label.trim(), g + 1)
                });
            }
            cols += 1;
            k += per_col;
        }
        println!(
            "[{tag}] jz={jz}: edit block {cols} columns, {} values ({} non-zero), worst {:.3e} at {}",
            w_edit.n, w_edit.nonzero, w_edit.rel, w_edit.what
        );
        assert!(cols >= 17, "edit columns {cols}");
        for (ltab, table) in tables.neutron.iter().enumerate() {
            let hdr = block
                .iter()
                .position(|l| l.contains(&format!("l={ltab} n-n table")))
                .unwrap();
            let width = (itabl - (iptotl - 2) + 1) as usize;
            let vals = parse_block(&block[hdr + 1..], width * NG);
            let mut w = Worst::new();
            for g in 0..NG {
                for p in 0..width {
                    let got = table.get(iptotl - 2 + p as i32, g as i32 + 1);
                    w.update(got, vals[g * width + p], || {
                        format!("l={ltab} group {} position {}", g + 1, p + 1)
                    });
                }
            }
            println!(
                "[{tag}] jz={jz}: l={ltab} n-n table {width}x{NG}, {} non-zero, worst {:.3e} at {}",
                w.nonzero, w.rel, w.what
            );
            assert!(w.nonzero > 50);
            worst_overall = worst_overall.max(w.rel);
        }
        let hdr = block.iter().position(|l| l.contains("n-p table")).unwrap();
        let vals = parse_block(&block[hdr + 1..], NGP * NG);
        let mut w = Worst::new();
        for g in 0..NG {
            for igp in 0..NGP {
                let got = tables.photon[0].get(igp as i32 + 1, g as i32 + 1);
                w.update(got, vals[g * NGP + igp], || {
                    format!("n-p group {} photon group {}", g + 1, igp + 1)
                });
            }
        }
        println!(
            "[{tag}] jz={jz}: n-p table {NGP}x{NG}, {} non-zero, worst {:.3e} at {}",
            w.nonzero, w.rel, w.what
        );
        assert!(w.nonzero > 50);
        worst_overall = worst_overall.max(w.rel).max(w_edit.rel);
    }
    assert!(
        worst_overall < TOL,
        "DTFR full tables deviate from NJOY: {worst_overall:e}"
    );
}
