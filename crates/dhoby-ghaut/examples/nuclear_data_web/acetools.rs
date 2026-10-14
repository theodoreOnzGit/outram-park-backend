//! The three ACE rungs' calculations (GitHub #863, #864, #865): build a
//! continuous-energy ACE table from an ENDF tape, build a thermal S(α,β) ACE
//! table from a thermal tape or a LEAPR deck, and compare two ACE files block
//! by block. Every input is a file the user supplies (maintainer,
//! 2026-10-14): the page ships no ENDF tape and no NJOY reference table for
//! these rungs.
//!
//! No physics lives here: each function calls `njoy-outram-park-fork`'s public
//! API, with the same deck the crate's own NJOY comparisons use, and returns
//! the file and a few lines saying what was done.

use njoy_outram_park_fork::acer::compare::{compare, Verdict};
use njoy_outram_park_fork::acer::read::{self, AceClass, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::thermal::ThermalAceOptions;
use njoy_outram_park_fork::acer::{build_deck, AceDeck, AceTable};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::thermr::calcem::egrid::{egrid_count, EGRID};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

/// Boltzmann's constant \[MeV/K\], NJOY2016's current value.
const K_B_MEV: f64 = 8.617_333_262e-11;

/// RECONR's tolerance: NJOY's default, the one the word-for-word comparisons
/// use.
pub const RECONR_TOL: f64 = 1.0e-3;

/// The PURR settings of NJOY2016's own reference decks (20 bands, 64 ladders)
/// and of `ace_blocks_vs_reference.rs` (10 000 samples).
pub const PURR_BINS: usize = 20;
pub const PURR_LADDERS: usize = 64;
pub const PURR_SAMPLES: usize = 10_000;

/// What the fast ACE generator runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AceGen {
    /// Deck temperature \[K\]. 0 runs RECONR → ACER; above 0, BROADR first.
    pub temp_k: f64,
    /// PURR probability tables in the unresolved range.
    pub purr: bool,
    /// HEATR heating numbers (on by default; NJOY's reference decks in this
    /// repository ran without it, so turn it off to compare against them).
    pub heatr: bool,
}

impl Default for AceGen {
    fn default() -> Self {
        Self { temp_k: 293.6, purr: true, heatr: true }
    }
}

/// A file made by a generator, and what was done to make it.
pub struct Made {
    pub name: String,
    pub bytes: Vec<u8>,
    pub lines: Vec<String>,
}

/// Build a continuous-energy ACE table, as Type-1 text, from the first
/// material on `endf`: RECONR at [`RECONR_TOL`], BROADR to `g.temp_k` when it
/// is above 0, then ACER with PURR and HEATR as chosen. The same calls as
/// `ace_blocks_vs_reference.rs`, which reproduces NJOY2016's tables word for
/// word on U-234, U-235 and U-238.
pub fn build_ace(endf: &[u8], g: AceGen) -> Result<Made, String> {
    let tape = Tape::read(std::io::Cursor::new(endf)).map_err(|e| format!("not a readable ENDF tape: {e}"))?;
    let mat = *tape.materials().first().ok_or("the tape holds no material")?;
    if g.temp_k < 0.0 {
        return Err("the temperature must be 0 K or above".into());
    }
    let recon0 = reconr(&tape, &ReconrConfig { mat, tolerance: RECONR_TOL, temperature: 0.0 }).map_err(|e| format!("RECONR: {e}"))?;
    let recon = if g.temp_k > 0.0 { broaden_result(&recon0, g.temp_k) } else { recon0 };
    let mut deck = AceDeck::default().with_temperature(g.temp_k);
    if !g.heatr {
        deck = deck.without_heatr();
    }
    if g.purr && g.temp_k > 0.0 {
        deck = deck.with_purr(PURR_BINS, PURR_LADDERS, PURR_SAMPLES);
    }
    let ace = build_deck(&tape, mat, &recon, K_B_MEV * g.temp_k, 0, &deck).map_err(|e| format!("ACER: {e}"))?;
    let raw = ace.to_raw(AceFileType::Type1Ascii).map_err(|e| e.to_string())?;
    let text = raw.to_type1_string();
    let mut modules = vec!["RECONR (tolerance 0.001)".to_string()];
    if g.temp_k > 0.0 {
        modules.push(format!("BROADR ({} K)", g.temp_k));
    }
    if g.purr && g.temp_k > 0.0 {
        modules.push(format!("PURR ({PURR_BINS} bands, {PURR_LADDERS} ladders)"));
    }
    if g.heatr {
        modules.push("HEATR".into());
    }
    modules.push("ACER".into());
    let lines = vec![
        format!("{}: MAT {mat}, {} XSS words", raw.header.zaid.trim(), raw.xss.len()),
        format!("deck: {}", modules.join(" -> ")),
        format!("{:.1} MB of Type-1 text", text.len() as f64 / 1e6),
    ];
    Ok(Made { name: format!("{}.ace", raw.header.zaid.trim()), bytes: text.into_bytes(), lines })
}

/// What the thermal ACE generator runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalGen {
    /// Outgoing energies per incident energy (NJOY's `nbin`).
    pub n_outgoing: usize,
    /// Equiprobable cosines per outgoing energy.
    pub n_cosines: usize,
    /// Highest incident energy \[eV\] (THERMR's `emax`).
    pub emax_ev: f64,
}

impl Default for ThermalGen {
    fn default() -> Self {
        let d = ThermalAceOptions::default();
        Self { n_outgoing: d.n_outgoing, n_cosines: d.n_cosines, emax_ev: d.emax_ev }
    }
}

/// Whether `bytes` read as an ENDF tape; otherwise they are taken as a LEAPR
/// deck.
fn as_tape(bytes: &[u8]) -> Option<Tape> {
    Tape::read(std::io::Cursor::new(bytes)).ok().filter(|t| !t.materials().is_empty())
}

/// Build a thermal S(α,β) ACE table, as Type-1 text, from an ENDF thermal
/// scattering tape or a LEAPR deck (`file_name` only names the result). A deck
/// is run through the port's LEAPR first. The table is built at the
/// evaluation's base temperature on THERMR's own incident-energy grid up to
/// `g.emax_ev`.
pub fn build_thermal_ace(bytes: &[u8], file_name: &str, g: ThermalGen) -> Result<Made, String> {
    let mut lines = Vec::new();
    let tape = match as_tape(bytes) {
        Some(t) => {
            lines.push("input: an ENDF thermal scattering (MF=7) tape".to_string());
            t
        }
        None => {
            let text = std::str::from_utf8(bytes).map_err(|_| "neither an ENDF tape nor a LEAPR deck (not text)".to_string())?;
            let run = njoy_outram_park_fork::leapr::run::run_deck_text(text).map_err(|e| format!("neither an ENDF tape nor a LEAPR deck LEAPR can run: {e}"))?;
            lines.push(format!("input: a LEAPR deck ({}), run by the port's LEAPR", run.title.trim()));
            run.tape
        }
    };
    let mat = *tape.materials().first().ok_or("the tape holds no material")?;
    let mf7 = parse_mf7(&tape, mat).map_err(|e| format!("MF=7: {e}"))?;
    let temp = mf7
        .incoherent_inelastic
        .as_ref()
        .ok_or("no incoherent inelastic (MT=4) S(α,β) on this tape")?
        .temperature_k;
    let grid: Vec<f64> = EGRID[..egrid_count(g.emax_ev)].to_vec();
    // The name in the ZAID: the file's stem without the ENDF/B "tsl-" prefix,
    // letters and digits only, at most 6 (the ZAID field is 10 characters
    // with ".00t").
    let base = file_name.split('.').next().unwrap_or("tsl");
    let base = base.strip_prefix("tsl-").or_else(|| base.strip_prefix("tsl_")).unwrap_or(base);
    let stem: String = base.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect();
    let name = if stem.is_empty() { "tsl".to_string() } else { stem.to_lowercase() };
    let opts = ThermalAceOptions { n_outgoing: g.n_outgoing, n_cosines: g.n_cosines, emax_ev: g.emax_ev, ..Default::default() };
    let ace = AceTable::thermal_from_mf7(&mf7, temp, &name, 0, &grid, opts).map_err(|e| format!("thermal ACER: {e}"))?;
    let raw = ace.to_raw(AceFileType::Type1Ascii).map_err(|e| e.to_string())?;
    let text = raw.to_type1_string();
    lines.push(format!("{}: MAT {mat}, {temp} K (the evaluation's base temperature), {} XSS words", raw.header.zaid.trim(), raw.xss.len()));
    lines.push(format!("{} incident energies up to {} eV, {} outgoing energies x {} cosines", grid.len(), g.emax_ev, g.n_outgoing, g.n_cosines));
    lines.push(format!("{:.2} MB of Type-1 text", text.len() as f64 / 1e6));
    Ok(Made { name: format!("{}.ace", raw.header.zaid.trim()), bytes: text.into_bytes(), lines })
}

/// The one table in an ACE file, read from its bytes (Type 1 or 2, gzipped or
/// not).
fn one_table(bytes: &[u8], name: &str) -> Result<RawAceTable, String> {
    let mut t = read::read_library_from_bytes(bytes.to_vec(), name).map_err(|e| format!("{name}: {e}"))?;
    match t.len() {
        1 => Ok(t.pop().expect("one table")),
        0 => Err(format!("{name}: no ACE table in the file")),
        n => Err(format!("{name}: {n} tables in the file; this comparator takes files of one table")),
    }
}

/// Compare two ACE files: the report's lines, then, for each block that
/// differs, the first few differing words.
pub fn compare_files(first: &[u8], first_name: &str, second: &[u8], second_name: &str) -> Result<Vec<String>, String> {
    let a = one_table(first, first_name)?;
    let b = one_table(second, second_name)?;
    let c = compare(&a, &b);
    let mut v = vec![format!("first: {first_name} ({})", a.header.zaid.trim()), format!("second: {second_name} ({})", b.header.zaid.trim())];
    if a.header.class != b.header.class {
        v.push("the two tables are of different classes, so only the whole XSS is compared".into());
    } else if !matches!(a.header.class, AceClass::ContinuousNeutron | AceClass::Thermal) {
        v.push("blocks are named for continuous-energy (c) and thermal (t) tables; this class is compared as a whole".into());
    }
    v.extend(c.report_lines());
    for s in c.blocks.iter().chain(std::iter::once(&c.whole_xss)) {
        if s.verdict != Verdict::Different {
            continue;
        }
        if let Some(d) = s.first_difference {
            let start = d.xss_index + 1 - d.word;
            let shown: Vec<String> = (0..s.words.0.max(s.words.1))
                .map(|i| (i, a.xss.get(start - 1 + i), b.xss.get(start - 1 + i)))
                .filter(|(_, x, y)| x != y)
                .take(5)
                .map(|(i, x, y)| {
                    let f = |z: Option<&f64>| z.map_or("(none)".to_string(), |z| format!("{z:e}"));
                    format!("    {} word {} (XSS {}): {} vs {}", s.name, i + 1, start + i, f(x), f(y))
                })
                .collect();
            v.extend(shown);
        }
    }
    Ok(v)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use njoy_outram_park_fork::reference_data::reference_endf;

    fn tape(name: &str) -> Vec<u8> {
        std::fs::read(reference_endf(name).expect("reference tape")).expect("read tape")
    }

    /// **Methodology.** The fast generator on ENDF/B-VIII.0 H-2 at 293.6 K
    /// with the default deck, its output read back by the crate's reader and
    /// compared with itself, then with the same deck at 0 K. **Result
    /// (2026-10-14):** the table reads back as `1002.00c`, compares identical
    /// with itself, and differs from the 0 K table in its header kT and ESZ.
    #[test]
    fn the_ace_generator_makes_a_readable_table_the_comparator_judges() {
        let h2 = tape("n-001_H_002-ENDF8.0.endf");
        let warm = build_ace(&h2, AceGen::default()).expect("build H-2 at 293.6 K");
        assert_eq!(warm.name, "1002.00c.ace");
        let line2 = std::str::from_utf8(&warm.bytes).unwrap().lines().nth(1).unwrap().to_string();
        assert!(line2.ends_with("mat 128"), "NJOY's material tag, the ENDF MAT: {line2}");
        assert!(warm.lines.iter().any(|l| l.contains("BROADR (293.6 K)") && l.contains("HEATR")), "{:?}", warm.lines);
        let same = compare_files(&warm.bytes, "a", &warm.bytes, "b").unwrap();
        assert!(same.iter().any(|l| l.contains("identical in every compared word")), "{same:?}");
        let cold = build_ace(&h2, AceGen { temp_k: 0.0, ..AceGen::default() }).expect("build H-2 at 0 K");
        let diff = compare_files(&warm.bytes, "warm", &cold.bytes, "cold").unwrap();
        assert!(diff.iter().any(|l| l.starts_with("kT (MeV)") && l.contains("DIFFERENT")), "{diff:?}");
        assert!(diff.iter().any(|l| l.starts_with("VERDICT") && l.contains("differ")), "{diff:?}");
        assert!(diff.iter().any(|l| l.trim_start().starts_with("ESZ word")), "the first differing ESZ words are listed: {diff:?}");
    }

    /// **Methodology.** The thermal generator on ENDF/B-VIII.0 Al-27's
    /// thermal tape and on a text that is neither a tape nor a LEAPR deck.
    /// **Result (2026-10-14):** a `t`-class table at 20 K (the evaluation's
    /// base temperature) that compares identical with itself; the junk input
    /// is refused with a message.
    #[test]
    fn the_thermal_generator_makes_a_thermal_table_and_refuses_junk() {
        let al = tape("tsl-013_Al_027-ENDF8.0.endf");
        let m = build_thermal_ace(&al, "tsl-013_Al_027-ENDF8.0.endf", ThermalGen::default()).expect("build Al-27 thermal");
        assert!(m.name.ends_with("t.ace"), "{}", m.name);
        assert!(m.lines.iter().any(|l| l.contains("20 K")), "{:?}", m.lines);
        let same = compare_files(&m.bytes, "a", &m.bytes, "b").unwrap();
        assert!(same.iter().any(|l| l.starts_with("ITXE") && l.contains("SAME")), "{same:?}");
        let e = build_thermal_ace(b"hello, not data", "x.txt", ThermalGen::default()).err().expect("refused");
        assert!(e.contains("neither an ENDF tape nor a LEAPR deck"), "{e}");
    }

    /// A LEAPR deck is run and turned into a thermal table: the committed
    /// ENDF/B-VIII.0 deck for H in H2O (the deck NJOY2016's own tape was made
    /// from). **Result (2026-10-14):** a `t`-class table, from the LEAPR run.
    #[test]
    fn a_leapr_deck_goes_through_leapr_into_a_thermal_table() {
        let deck = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../njoy-outram-park-fork/src/leapr/decks/tsl-HinH2O.leapr")).expect("committed deck");
        let m = build_thermal_ace(&deck, "tsl-HinH2O.leapr", ThermalGen { emax_ev: 1.0, ..ThermalGen::default() }).expect("deck to thermal ACE");
        assert!(m.lines[0].contains("LEAPR deck"), "{:?}", m.lines);
        assert_eq!(m.name, "hinh2o.00t.ace", "named from the file, without its tsl- prefix");
    }

    /// Files the comparator cannot take are refused with their name.
    #[test]
    fn the_comparator_refuses_what_is_not_an_ace_file() {
        let e = compare_files(b"nonsense", "x.ace", b"nonsense", "y.ace").err().expect("refused");
        assert!(e.starts_with("x.ace"), "{e}");
    }
}
