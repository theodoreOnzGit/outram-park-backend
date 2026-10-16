//! The three ACE rungs' calculations (GitHub #863, #864, #865): build a
//! continuous-energy ACE table from an ENDF tape, build a thermal S(α,β) ACE
//! table from a thermal tape or a LEAPR deck, and compare two ACE files block
//! by block. Every input is a file the user supplies (maintainer,
//! 2026-10-14): the page ships no ENDF tape and no NJOY reference table for
//! these rungs.
//!
//! No physics lives here: each function calls `njoy-outram-park-fork`'s public
//! API, with the same deck the crate's own NJOY comparisons use, and returns
//! the file and a few lines saying what was done. The ACE generators call the
//! crate's high-level entry points (GitHub #871): `process::ace` for a
//! continuous-energy table and `process::thermal_ace` for a LEAPR deck.

use njoy_outram_park_fork::acer::compare::{compare, Verdict};
use njoy_outram_park_fork::acer::read::{self, AceClass, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::thermal::{ace_incident_grid, IncidentPendf, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, PurrSettings, ThermalSettings};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use uom::si::energy::electronvolt;
use uom::si::thermodynamic_temperature::kelvin;

/// RECONR's tolerance: NJOY's default, the one the word-for-word comparisons
/// use (the tsl-tape route; `process` uses the same default).
pub const RECONR_TOL: f64 = 1.0e-3;

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
    let mut settings = CrossSectionSettings::default();
    if !g.purr {
        settings = settings.without_purr();
    }
    if !g.heatr {
        settings = settings.without_heatr();
    }
    let xs = process::cross_sections(&tape, Temperature::new::<kelvin>(g.temp_k), &settings).map_err(|e| e.to_string())?;
    let ace = xs.to_ace().map_err(|e| format!("ACER: {e}"))?;
    let text = ace.to_type1_text().map_err(|e| e.to_string())?;
    let mut modules = vec!["RECONR (tolerance 0.001)".to_string()];
    if g.temp_k > 0.0 {
        modules.push(format!("BROADR ({} K)", g.temp_k));
    }
    if g.purr && g.temp_k > 0.0 {
        let p = PurrSettings::default();
        modules.push(format!("PURR ({} bands, {} ladders)", p.bands, p.ladders));
    }
    if g.heatr {
        modules.push("HEATR".into());
    }
    modules.push("ACER".into());
    let zaid = ace.zaid.trim().to_string();
    let lines = vec![
        format!("{zaid}: MAT {}, {} XSS words", xs.mat, ace.xss.len()),
        format!("deck: {}", modules.join(" -> ")),
        format!("{:.1} MB of Type-1 text", text.len() as f64 / 1e6),
    ];
    Ok(Made { name: format!("{zaid}.ace"), bytes: text.into_bytes(), lines })
}

/// What the thermal ACE generator runs.
#[derive(Clone, Debug, PartialEq)]
pub struct ThermalGen {
    /// The table's name in its ZAID, NJOY's ACER card 8 `tname` (6
    /// characters at most). Empty: taken from the file name.
    pub tname: String,
    /// Outgoing energies per incident energy (NJOY's `nbin`).
    pub n_outgoing: usize,
    /// Equiprobable cosines per outgoing energy.
    pub n_cosines: usize,
    /// Highest incident energy \[eV\] (THERMR's `emax`).
    pub emax_ev: f64,
    /// THERMR's card-4 tolerance, the one `calcem` and `coh` linearise to.
    /// 0.001 by default, as OpenMC's `make_ace_thermal` deck.
    pub tol: f64,
    /// THERMR's `natom`, principal scattering atoms per molecule.
    pub natom: f64,
    /// Take `natom` and `emax` from the tape's MF=7/MT=4 `B(6)` and `B(4)`
    /// instead, as OpenMC's `make_ace_thermal` does (`njoy.py:694-699`).
    pub from_tape: bool,
    /// ACER's `iwt`: 1 equiprobable bins (IFENG 0), 0 skewed `1 4 10 … 10 4 1`
    /// bins (IFENG 1), 2 the continuous form (IFENG 2, OpenMC's default).
    pub iwt: u8,
}

impl Default for ThermalGen {
    fn default() -> Self {
        let d = ThermalAceOptions::default();
        // 16 cosines: THERMR's `nbin` in NJOY's and OpenMC's thermal decks.
        Self { tname: String::new(), n_outgoing: d.n_outgoing, n_cosines: 16, emax_ev: d.emax_ev, tol: 0.001, natom: 1.0, from_tape: true, iwt: 1 }
    }
}

/// Whether `bytes` read as an ENDF tape; otherwise they are taken as a LEAPR
/// deck.
fn as_tape(bytes: &[u8]) -> Option<Tape> {
    Tape::read(std::io::Cursor::new(bytes)).ok().filter(|t| !t.materials().is_empty())
}

/// The table's name in its ZAID: `g.tname`, or the file's stem without the
/// ENDF/B "tsl-" prefix, letters and digits only, at most 6 (the ZAID field is
/// 10 characters with ".00t"), lower case. At least two characters: a
/// one-character name gives a ZAID like `x.00t`, whose `.` in column 2
/// OpenMC's reader takes for an ACE 2.0 header (`openmc/data/ace.py:350`),
/// and so does this crate's.
fn table_name(g: &ThermalGen, file_name: &str) -> String {
    let base = if g.tname.trim().is_empty() { file_name.split('.').next().unwrap_or("tsl") } else { g.tname.trim() };
    let base = base.strip_prefix("tsl-").or_else(|| base.strip_prefix("tsl_")).unwrap_or(base);
    let stem: String = base.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect();
    if stem.len() < 2 {
        "tsl".to_string()
    } else {
        stem.to_lowercase()
    }
}

/// Build a thermal S(α,β) ACE table, as Type-1 text, from a LEAPR deck or an
/// ENDF thermal scattering tape (`file_name` only names the result).
///
/// **A LEAPR deck** (the primary input, gh:#870) goes through
/// `process::thermal_ace` (gh:#871): LEAPR at the deck's first temperature,
/// RECONR and BROADR of the incident tape, THERMR, ACER. It needs the incident
/// nuclide's tape (`incident`).
///
/// **A thermal tape** (kept for comparison with NJOY runs on published tapes)
/// is built at the evaluation's base temperature on the incident energies
/// ACER keeps up to `g.emax_ev`. With the incident nuclide's ENDF tape, it is
/// run through RECONR (0.001) and BROADR to that temperature first, as NJOY's
/// deck does, and the table is NJOY2016's in every word (gh:#865, four cases
/// in `acer_thermal_word_parity.rs`). Without it, the inelastic cross section
/// is taken at each energy instead of interpolated on that grid, and the
/// header has no MAT and the thermal tape's AWR.
pub fn build_thermal_ace(bytes: &[u8], file_name: &str, g: ThermalGen, incident: Option<&[u8]>) -> Result<Made, String> {
    let mut lines = Vec::new();
    let tape = match as_tape(bytes) {
        Some(t) => {
            lines.push("input: an ENDF thermal scattering (MF=7) tape".to_string());
            t
        }
        None => {
            let text = std::str::from_utf8(bytes).map_err(|_| "neither an ENDF tape nor a LEAPR deck (not text)".to_string())?;
            return build_thermal_ace_from_deck(text, file_name, g, incident);
        }
    };
    let mat = *tape.materials().first().ok_or("the tape holds no material")?;
    let mf7 = parse_mf7(&tape, mat).map_err(|e| format!("MF=7: {e}"))?;
    let temp = mf7
        .incoherent_inelastic
        .as_ref()
        .ok_or("no incoherent inelastic (MT=4) S(α,β) on this tape")?
        .temperature_k;
    // ~~`EGRID[..egrid_count(emax)]`~~ CORRECTED 2026-10-14: that is calcem's
    // grid, one energy past emax; ACER drops it (gh:#865).
    let ii = mf7.incoherent_inelastic.as_ref().ok_or("no incoherent inelastic (MT=4) S(α,β) on this tape")?;
    let (natom, emax_ev) = match (g.from_tape, ii.b.get(5), ii.b.get(3)) {
        (true, Some(&n), Some(&e)) if n >= 1.0 && e > 0.0 => (n, e),
        _ => (g.natom, g.emax_ev),
    };
    let form = match g.iwt {
        0 => InelasticForm::Skewed,
        2 => InelasticForm::Continuous,
        _ => InelasticForm::Equiprobable,
    };
    let grid = ace_incident_grid(emax_ev);
    let name = table_name(&g, file_name);
    let opts = ThermalAceOptions { n_outgoing: g.n_outgoing, n_cosines: g.n_cosines, emax_ev, natom, form };
    let ace = match incident {
        Some(inc) => {
            let t = Tape::read(std::io::Cursor::new(inc)).map_err(|e| format!("incident tape: not a readable ENDF tape: {e}"))?;
            let imat = *t.materials().first().ok_or("the incident tape holds no material")?;
            let r0 = reconr(&t, &ReconrConfig { mat: imat, tolerance: RECONR_TOL, temperature: 0.0 }).map_err(|e| format!("RECONR: {e}"))?;
            let r = broaden_result(&r0, temp);
            let elastic = r.sections.iter().find(|s| s.mt.number() == 2).ok_or("the incident tape has no elastic (MT=2)")?.pairs.clone();
            lines.push(format!("incident: MAT {imat}, RECONR (0.001) -> BROADR ({temp} K), THERMR's MF=3 on its elastic grid"));
            let pendf = IncidentPendf { mat: imat, awr: r.material.awr, moderator_za: vec![r.material.za as i32], elastic };
            AceTable::thermal_from_mf7_on_pendf(&mf7, temp, &name, 0, &grid, opts, g.tol, &pendf)
        }
        None => {
            lines.push("no incident tape: the inelastic cross section is not on NJOY's grid, and the header has no MAT".into());
            AceTable::thermal_from_mf7_with_tolerance(&mf7, temp, &name, 0, &grid, opts, g.tol)
        }
    }
    .map_err(|e| format!("thermal ACER: {e}"))?;
    let raw = ace.to_raw(AceFileType::Type1Ascii).map_err(|e| e.to_string())?;
    let text = raw.to_type1_string();
    lines.push(format!("{}: MAT {mat}, {temp} K (the evaluation's base temperature), {} XSS words", raw.header.zaid.trim(), raw.xss.len()));
    let src = if g.from_tape { " (natom and emax from the tape's B(6), B(4))" } else { "" };
    lines.push(format!("{} incident energies up to {emax_ev} eV, {} outgoing energies x {} cosines, iwt {}, tol {}, natom {natom}{src}", grid.len(), g.n_outgoing, g.n_cosines, g.iwt, g.tol));
    lines.push(format!("{:.2} MB of Type-1 text", text.len() as f64 / 1e6));
    Ok(Made { name: format!("{}.ace", raw.header.zaid.trim()), bytes: text.into_bytes(), lines })
}

/// [`build_thermal_ace`] for a LEAPR deck: `process::thermal_ace` at the
/// deck's first temperature, with `g`'s settings (`natom` and `emax` from the
/// law's `B(6)` and `B(4)` when `g.from_tape`). LEAPR uses the deck's own
/// constants (those of its `EVAL` date, which reproduce the published tape).
fn build_thermal_ace_from_deck(text: &str, file_name: &str, g: ThermalGen, incident: Option<&[u8]>) -> Result<Made, String> {
    let deck = LeaprDeck::parse(text).map_err(|e| format!("neither an ENDF tape nor a LEAPR deck LEAPR can run: {e}"))?;
    let inc = incident.ok_or("a LEAPR deck needs the incident nuclide's ENDF tape (e.g. H-1 for H in H2O) in the incident slot")?;
    let inc = Tape::read(std::io::Cursor::new(inc)).map_err(|e| format!("incident tape: not a readable ENDF tape: {e}"))?;
    let temp = *deck.temperatures_k().first().ok_or("the LEAPR deck declares no temperature")?;
    let form = match g.iwt {
        0 => InelasticForm::Skewed,
        2 => InelasticForm::Continuous,
        _ => InelasticForm::Equiprobable,
    };
    let name = table_name(&g, file_name);
    let mut s = ThermalSettings::default()
        .with_thermr_tolerance(g.tol)
        .with_cosines(g.n_cosines)
        .with_outgoing_energies(g.n_outgoing)
        .with_form(form)
        .with_table_name(&name);
    if !g.from_tape {
        s = s.with_natom(g.natom).with_emax(NeutronEnergy::new::<electronvolt>(g.emax_ev));
    }
    let law = process::thermal_scattering(&inc, text, Temperature::new::<kelvin>(temp), &s).map_err(|e| format!("LEAPR/THERMR: {e}"))?;
    let ace = law.to_ace().map_err(|e| format!("thermal ACER: {e}"))?;
    let text = ace.to_type1_text().map_err(|e| e.to_string())?;
    let zaid = ace.zaid.trim().to_string();
    let t = &law.thermr;
    let src = if g.from_tape { " (natom and emax from the law's B(6), B(4))" } else { "" };
    let inc_mat = t.incident.as_ref().map_or(0, |i| i.mat);
    let lines = vec![
        format!("input: a LEAPR deck ({}), run by the port's LEAPR at {temp} K", law.title.trim()),
        format!("incident: MAT {inc_mat}, RECONR (0.001) -> BROADR ({temp} K), THERMR's MF=3 on its elastic grid"),
        format!("{zaid}: MAT {}, {temp} K, {} XSS words", law.mat, ace.xss.len()),
        format!("{} incident energies up to {} eV, {} outgoing energies x {} cosines, iwt {}, tol {}, natom {}{src}", ace_incident_grid(t.emax_ev).len(), t.emax_ev, g.n_outgoing, t.n_cosines, g.iwt, t.tol, t.natom),
        format!("{:.2} MB of Type-1 text", text.len() as f64 / 1e6),
    ];
    Ok(Made { name: format!("{zaid}.ace"), bytes: text.into_bytes(), lines })
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
    /// thermal tape, with and without the Al-27 incident tape, at the default
    /// deck's settings (tol 0.001, natom 1, emax 4 eV, 16 x 16), compared with the
    /// committed NJOY2016 table made by the deck in
    /// `reference-data/acer/thermal_decks/al27_20k_tol0p001.njoy`; and on a
    /// text that is neither a tape nor a LEAPR deck.
    /// **Result (2026-10-14):** with the incident tape, identical to NJOY2016's
    /// in every compared word; without it, ITIX and the material differ and
    /// every other block is the same; the junk input is refused with a
    /// message.
    #[test]
    fn the_thermal_generator_makes_njoys_table_and_refuses_junk() {
        let al = tape("tsl-013_Al_027-ENDF8.0.endf");
        let inc = tape("n-013_Al_027-ENDF8.0.endf");
        let njoy = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/acer/al27_20k_thermal_tol0p001_njoy2016.ace.gz")).expect("committed NJOY table");
        // The committed deck's settings: emax 4 eV, natom 1, iwt 1.
        let g = ThermalGen { tname: "al27".into(), from_tape: false, ..ThermalGen::default() };
        let m = build_thermal_ace(&al, "tsl-013_Al_027-ENDF8.0.endf", g.clone(), Some(&inc)).expect("build Al-27 thermal");
        assert!(m.name.ends_with("t.ace"), "{}", m.name);
        assert!(m.lines.iter().any(|l| l.contains("20 K")), "{:?}", m.lines);
        let same = compare_files(&m.bytes, "ours", &njoy, "njoy").unwrap();
        assert!(same.iter().any(|l| l.contains("identical in every compared word")), "{same:?}");
        let alone = build_thermal_ace(&al, "tsl-013_Al_027-ENDF8.0.endf", g, None).expect("build without incident");
        let ours_tape = build_thermal_ace(&al, "tsl-013_Al_027-ENDF8.0.endf", ThermalGen::default(), None).expect("emax from the tape");
        assert!(ours_tape.lines.iter().any(|l| l.contains("up to 2.277 eV") && l.contains("natom 1")), "B(4) = 2.277 eV on Al-27: {:?}", ours_tape.lines);
        let diff = compare_files(&alone.bytes, "ours", &njoy, "njoy").unwrap();
        assert!(diff.iter().any(|l| l.starts_with("ITIX") && l.contains("DIFFERENT")), "{diff:?}");
        assert!(diff.iter().any(|l| l.starts_with("ITXE") && l.contains("SAME")), "{diff:?}");
        let e = build_thermal_ace(b"hello, not data", "x.txt", ThermalGen::default(), None).err().expect("refused");
        assert!(e.contains("neither an ENDF tape nor a LEAPR deck"), "{e}");
    }

    /// A LEAPR deck is run and turned into a thermal table through
    /// `process::thermal_ace` (gh:#871): the committed ENDF/B-VIII.0 deck for
    /// H in H2O (the deck NJOY2016's own tape was made from) with H-1's tape,
    /// at the deck's first temperature (283.6 K). Without the incident tape it
    /// is refused with a message. **Result (2026-10-14):** a `t`-class table
    /// carrying H-1's MAT (125); the deck alone is refused.
    #[test]
    fn a_leapr_deck_goes_through_leapr_into_a_thermal_table() {
        let deck = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../njoy-outram-park-fork/src/leapr/decks/tsl-HinH2O.leapr")).expect("committed deck");
        let h1 = tape("n-001_H_001-ENDF8.0-Beta6.endf");
        let g = ThermalGen { emax_ev: 1.0, from_tape: false, ..ThermalGen::default() };
        let m = build_thermal_ace(&deck, "tsl-HinH2O.leapr", g.clone(), Some(&h1)).expect("deck to thermal ACE");
        assert!(m.lines[0].contains("LEAPR deck") && m.lines[0].contains("283.6 K"), "{:?}", m.lines);
        assert_eq!(m.name, "hinh2o.00t.ace", "named from the file, without its tsl- prefix");
        let line2 = std::str::from_utf8(&m.bytes).unwrap().lines().nth(1).unwrap().to_string();
        assert!(line2.ends_with("mat 125"), "the incident MAT: {line2}");
        let e = build_thermal_ace(&deck, "tsl-HinH2O.leapr", g, None).err().expect("refused");
        assert!(e.contains("incident"), "{e}");
    }

    /// Files the comparator cannot take are refused with their name.
    #[test]
    fn the_comparator_refuses_what_is_not_an_ace_file() {
        let e = compare_files(b"nonsense", "x.ace", b"nonsense", "y.ace").err().expect("refused");
        assert!(e.starts_with("x.ace"), "{e}");
    }
}
