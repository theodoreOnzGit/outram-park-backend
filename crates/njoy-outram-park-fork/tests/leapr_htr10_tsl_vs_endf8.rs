//! V&V (GitHub #1140, epic #1137): the LEAPR port's regeneration of the four
//! HTR-10 scattering laws — reactor graphite 10P (MAT 31) and 30P (MAT 32),
//! C in SiC (MAT 44) and Si in SiC (MAT 43) — against their published
//! ENDF/B-VIII.0 tapes, at **every temperature each tape tabulates**.
//!
//! # Why
//!
//! The HTR-10 per-zone-temperature model generates these laws by LEAPR at
//! arbitrary zone temperatures (20–1800 °C). That is only defensible if LEAPR
//! reproduces the published tapes at the tapes' own temperatures. Before this
//! file only crystalline graphite (MAT 30) had been compared with its tape at
//! the stored-value level (Si in SiC's MT=4 had a 1 % gate,
//! `tests/leapr_temperature_block_selection.rs`).
//!
//! # Methodology
//!
//! - **Generator under test:** the path `outram-mc-libs`'
//!   `ThermalScattering::from_embedded_leapr_deck` (the wasm-safe path the
//!   demos use) runs — the deck embedded in this crate
//!   (`leapr::decks::embedded_deck_text`), parsed by `LeaprDeck::parse`, cut to
//!   one temperature and run through LEAPR's whole driver by
//!   `leapr::run::run_deck_at_temperature`, written as tape text by
//!   `LeaprRun::write_text` (`endout` applies NJOY's `sigfig` rounding), and
//!   read back with `Tape::read` + `parse_mf7_at_temperature`, as a consumer
//!   reads it. One generation per tabulated temperature.
//! - **Reference:** `reference-data/endf/tsl-reactor-graphite-10P.endf`,
//!   `tsl-reactor-graphite-30P.endf`, `tsl-CinSiC.endf`, `tsl-SiinSiC.endf`
//!   (ENDF/B-VIII.0, NCSU LEIP; provenance in `reference-data/endf/README.md`),
//!   read through the same parser at the same tabulated temperature.
//! - **MF=7/MT=4:** the `beta` and `alpha` grids (exactly), every stored
//!   `S(alpha, beta)` (count bit-identical after both are parsed from text,
//!   worst relative difference over reference values above 1e-30), `B(1..6)`,
//!   and `T_eff(T)`. **Gate, fixed before measuring: 1e-9 relative** — the gate
//!   `tests/leapr_run_at_temperature.rs` uses for the same driver against
//!   NJOY2016's own tapes, two orders below one unit in a 7-figure field.
//! - **MF=7/MT=2, graphite** (`iel = 1`, LEAPR's built-in graphite lattice),
//!   three ways:
//!   1. *Single-temperature run vs the tape's row* (the HTR-10 path). Every
//!      Bragg edge energy and `S(E_i, T)` is compared position by position up
//!      to the shorter list's last slot; past it the two step functions `S(E)`
//!      are compared at every edge energy of either list. **Gate: one unit in
//!      the 7th significant figure** ("print round-off", the bound #1140 set
//!      before measuring), expressed as `|a - b| / 10^(floor(log10|b|) - 6)`.
//!   2. *Whole deck through `run_deck`* (every temperature in one run, as the
//!      evaluator ran it) vs the whole tape. **Gate: bit-identical.**
//!   3. *Cause check:* the 296 K single-temperature run's output spliced with
//!      the T run's into a two-temperature `endout` call, vs the tape's row at
//!      T. **Gate: bit-identical.** This isolates the two upstream rules that
//!      make (1) differ from the tape (see Results).
//! - **SiC MT=2 is reported, not gated, here.** The decks have `iel = 0`; the
//!   published MT=2 came from an in-house routine, not LEAPR
//!   (`tsl-SiinSiC.readme`: "the provided leapr input may reproduce MT = 4
//!   only"). Its Bragg pattern is root-caused in
//!   `docs/leapr-sic-coherent-elastic-vv.md` and gated at 5 % by
//!   `tests/leapr_sic_coherent_elastic_oracle.rs`. What `run_deck_at_temperature`
//!   emits for it is pinned below.
//! - **The `SabRequest` path too** (`generate::thermal_scattering_tape`,
//!   which `generate_tape` serves and `SabRequest::validation` reports on): MT=4
//!   (and graphite MT=2) at each material's base temperature, same gates.
//!
//! # Results (2026-11-03, release build, glibc Linux x86_64)
//!
//! Full per-temperature table: `verification_and_validation/leapr_htr10_tsl_vs_endf8.md`.
//!
//! - **MT=4: bit-identical at all 36 tabulated temperatures** (10 + 10 + 8 + 8):
//!   every one of the 60 000 stored `S` per temperature is identical after
//!   parsing (Si in SiC at 1000 K: 59 999 of 60 000, the one exception below
//!   1e-30), worst relative difference 0 over 43 976–56 599 values above
//!   1e-30 per temperature. `B(1..6)` and `T_eff` identical at every one.
//!   The `SabRequest` path gives the same at each base temperature. (The one
//!   sub-floor exception: Si in SiC, 1000 K, last beta, alpha 16.42225 —
//!   1.82254e-75 here, 0 on the tape; not root-caused, see the record.)
//! - **Graphite MT=2, whole deck: bit-identical** at all 10 temperatures,
//!   169 edges (10P) and 160 (30P).
//! - **Graphite MT=2, single-temperature run: bit-identical at 296 K; at the
//!   other 9 temperatures within one unit in the 7th figure** (worst 9.4e-7
//!   relative, 10P 500 K), with **fewer retained edges** (e.g. 146 vs 169 at
//!   400 K, 70 vs 169 at 2000 K) and the same `S(E -> inf)` to within one
//!   unit. Both differences are NJOY2016's `endout`, not a port defect:
//!   `endout` (a) thins the 1/E tail of the Bragg list with the **first**
//!   temperature's `W'` (`leapr.f90` ~3204–3216, ported in
//!   `endout::build_coherent_elastic`), so a run whose first temperature is T
//!   keeps the edges significant at T; and (b) evaluates the first
//!   temperature's `S` with the unrounded edge energy but every later
//!   temperature's with `sigfig(E, 7)`, which moves the 7th figure. The cause
//!   check (3) reproduces the tape's row bit for bit at all 9.
//! - **SiC MT=2:** `run_deck_at_temperature` writes **incoherent** elastic
//!   (LTHR = 2, `iel = 0` with `twt = 0` becomes `iel = -1`,
//!   `leapr.f90:3043`) where the published tape is **coherent** (LTHR = 1).
//!   `leapr::generate::generate_tape` (the `SabRequest` path) instead builds
//!   coherent elastic from the 3C-SiC catalogue. See the V&V record.
//!
//! Gated behind `long-tests` on the **data** criterion (four 7–9 MB reference
//! tapes); the whole file takes ~2–3 min on 4 cores.

use std::time::Instant;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
use njoy_outram_park_fork::leapr::endout::{endout, LeaprOutput};
use njoy_outram_park_fork::leapr::generate::{thermal_scattering_tape, SabRequest};
use njoy_outram_park_fork::leapr::run::{run_deck, run_deck_at_temperature, LeaprRun};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7_at_temperature, CoherentElastic, Mf7};
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

/// The MT=4 / `T_eff` / bit-identity gate, fixed before measuring.
const TOL: f64 = 1.0e-9;
/// Values below this are not compared relatively (NJOY's own `1e-30` floor).
const FLOOR: f64 = 1.0e-30;

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / b.abs()
    }
}

/// `|a - b|` in units of the 7th significant figure of `b` (an ENDF 7-figure
/// field's last printed digit). 0 when equal.
fn units7(a: f64, b: f64) -> f64 {
    if a == b {
        return 0.0;
    }
    let scale = b.abs().max(a.abs());
    if scale <= FLOOR {
        return 0.0;
    }
    (a - b).abs() / 10f64.powi(scale.log10().floor() as i32 - 6)
}

fn read_back(tape_text: String, mat: i32, t: Option<f64>) -> Mf7 {
    let tape = Tape::read(std::io::Cursor::new(tape_text)).unwrap();
    parse_mf7_at_temperature(&tape, mat, t).unwrap()
}

fn deck(material: SabMaterial) -> LeaprDeck {
    LeaprDeck::parse(embedded_deck_text(material).expect("embedded deck")).unwrap()
}

/// One temperature, MT=4.
#[allow(dead_code)] // every field is read through `Debug` in the printed table
#[derive(Debug, Clone)]
struct Mt4 {
    n: usize,
    n_ident: usize,
    worst: f64,
    worst_at: (f64, f64, f64, f64),
    teff: (f64, f64),
    b_ident: bool,
    /// The first stored pair that differs with the reference at or below
    /// 1e-30 (beta, alpha, ours, theirs), if any.
    subfloor_diff: Option<(f64, f64, f64, f64)>,
}

fn teff_at(mf7: &Mf7, t: f64) -> f64 {
    mf7.incoherent_inelastic
        .as_ref()
        .unwrap()
        .teff_table
        .iter()
        .find(|p| (p.0 - t).abs() < 1e-6)
        .map(|p| p.1)
        .expect("T_eff at the tabulated temperature")
}

fn compare_mt4(material: SabMaterial, ours: &Mf7, theirs: &Mf7, t: f64) -> Mt4 {
    let o = ours.incoherent_inelastic.as_ref().expect("our MT=4");
    let r = theirs.incoherent_inelastic.as_ref().expect("their MT=4");
    assert_eq!((o.lat, o.lasym, o.lln), (r.lat, r.lasym, r.lln), "{material:?} {t} K: LAT/LASYM/LLN");
    assert_eq!(o.beta, r.beta, "{material:?} {t} K: beta grid");
    let (mut n, mut n_ident, mut worst, mut worst_at) = (0, 0, 0.0f64, (0.0, 0.0, 0.0, 0.0));
    let mut subfloor_diff = None;
    for (ot, rt) in o.s_tables.iter().zip(&r.s_tables) {
        assert_eq!(ot.alpha, rt.alpha, "{material:?} {t} K: alpha grid at beta {}", rt.beta);
        for ((&a, &b), &alpha) in ot.s.iter().zip(&rt.s).zip(&rt.alpha) {
            if a == b {
                n_ident += 1;
            }
            if b.abs() <= FLOOR {
                if a != b && subfloor_diff.is_none() {
                    subfloor_diff = Some((rt.beta, alpha, a, b));
                }
                continue;
            }
            n += 1;
            if rel(a, b) > worst {
                worst = rel(a, b);
                worst_at = (rt.beta, alpha, a, b);
            }
        }
    }
    Mt4 { n, n_ident, worst, worst_at, teff: (teff_at(ours, t), teff_at(theirs, t)), b_ident: o.b == r.b, subfloor_diff }
}

/// One MF=7/MT=2 coherent-elastic comparison, our row `jo` against their row `jr`.
#[allow(dead_code)] // every field is read through `Debug` in the printed table
#[derive(Debug, Clone)]
struct Mt2 {
    /// Retained Bragg edges, ours and theirs.
    n_edges: (usize, usize),
    /// Edges compared position by position: all of them when the lists are the
    /// same length, otherwise every edge before the shorter list's last slot
    /// (the slot NJOY's thinning collapses the tail onto).
    n_prefix: usize,
    n_prefix_ident: usize,
    worst_prefix_rel: f64,
    worst_prefix_units7: f64,
    /// Worst difference of the step functions `S(E)`, in 7th-figure units,
    /// at every edge energy of either list at or above the shorter list's last
    /// slot (`0` when the lists have the same length).
    worst_tail_units7: f64,
    /// `S(E -> infinity)`: the last stored value of each.
    s_total: (f64, f64),
}

/// `S(E)` of a histogram (LTHR=1) table: the value at the last edge `<= e`.
fn s_at(edges: &[f64], s: &[f64], e: f64) -> f64 {
    edges.iter().zip(s).filter(|(x, _)| **x <= e).last().map(|(_, v)| *v).unwrap_or(0.0)
}

fn compare_mt2(oc: &CoherentElastic, jo: usize, rc: &CoherentElastic, jr: usize) -> Mt2 {
    let (eo, so) = (&oc.bragg_energies_ev, &oc.s_tables[jo]);
    let (er, sr) = (&rc.bragg_energies_ev, &rc.s_tables[jr]);
    let n = eo.len().min(er.len());
    let n_prefix = if eo.len() == er.len() { n } else { n - 1 };
    let (mut ident, mut wrel, mut wu) = (0usize, 0.0f64, 0.0f64);
    for i in 0..n_prefix {
        if eo[i] == er[i] && so[i] == sr[i] {
            ident += 1;
        }
        wrel = wrel.max(rel(eo[i], er[i]));
        wu = wu.max(units7(eo[i], er[i]));
        if sr[i].abs() > FLOOR {
            wrel = wrel.max(rel(so[i], sr[i]));
        }
        wu = wu.max(units7(so[i], sr[i]));
    }
    let mut wt = 0.0f64;
    if n_prefix < n {
        let e_tail = eo[n - 1].min(er[n - 1]);
        for &e in eo.iter().chain(er.iter()).filter(|&&e| e >= e_tail) {
            wt = wt.max(units7(s_at(eo, so, e), s_at(er, sr, e)));
        }
    }
    Mt2 {
        n_edges: (eo.len(), er.len()),
        n_prefix,
        n_prefix_ident: ident,
        worst_prefix_rel: wrel,
        worst_prefix_units7: wu,
        worst_tail_units7: wt,
        s_total: (*so.last().unwrap(), *sr.last().unwrap()),
    }
}

/// The base run's output with a second temperature's law appended — what
/// `endout` sees when a deck lists `[base, t]`.
fn spliced(base: &LeaprOutput, at_t: &LeaprOutput) -> LeaprOutput {
    let mut out = base.clone();
    out.temperatures_k.extend_from_slice(&at_t.temperatures_k);
    out.dwpix.extend_from_slice(&at_t.dwpix);
    out.tempf.extend_from_slice(&at_t.tempf);
    out.ssm.extend(at_t.ssm.iter().cloned());
    out
}

fn tape_text(tape: &Tape) -> String {
    let mut buf = Vec::new();
    tape.write(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Everything measured at one tabulated temperature.
#[allow(dead_code)] // every field is read through `Debug` in the printed table
#[derive(Debug, Clone)]
struct Row {
    t: f64,
    mt4: Mt4,
    /// Graphite only: single-temperature run vs the tape's row.
    mt2: Option<Mt2>,
    /// Graphite only: the 296 K run spliced with this one, vs the tape's row.
    mt2_spliced: Option<Mt2>,
    /// Base temperature only: the `SabRequest` path (`generate::generate_tape`)
    /// against the tape — MT=4, and MT=2 when both are coherent.
    sab_request: Option<(Mt4, Option<Mt2>)>,
    /// The elastic law kinds (ours, theirs) as `LTHR`.
    lthr: (u8, u8),
    gen_s: f64,
}

fn lthr(m: &Mf7) -> u8 {
    u8::from(m.coherent_elastic.is_some()) + 2 * u8::from(m.incoherent_elastic.is_some())
}

/// Measure one material at every tabulated temperature through the
/// single-temperature path, print each row, and return them.
fn measure(material: SabMaterial, file: &str) -> Option<Vec<Row>> {
    let path = reference_endf_or_skip(file, "leapr-htr10-tsl-vs-endf8")?;
    let tape = Tape::read_file(&path).unwrap();
    let mat = material.mat();
    let temps = parse_mf7_at_temperature(&tape, mat, None)
        .unwrap()
        .incoherent_inelastic
        .unwrap()
        .tabulated_temperatures_k;
    let d = deck(material);
    let mut base: Option<LeaprRun> = None;
    let mut rows = Vec::new();
    for &t in &temps {
        let theirs = parse_mf7_at_temperature(&tape, mat, Some(t)).unwrap();
        let t0 = Instant::now();
        let run = run_deck_at_temperature(&d, t).expect("LEAPR runs at a tabulated temperature");
        let text = run.write_text().expect("tape text");
        let gen_s = t0.elapsed().as_secs_f64();
        let ours = read_back(text, mat, Some(t));
        let mt4 = compare_mt4(material, &ours, &theirs, t);
        let row_of = |rc: &CoherentElastic| rc.temperatures_k.iter().position(|&x| (x - t).abs() < 1e-6);
        let mt2 = match (&ours.coherent_elastic, &theirs.coherent_elastic) {
            (Some(oc), Some(rc)) => Some(compare_mt2(oc, 0, rc, row_of(rc).expect("MT=2 row"))),
            _ => None,
        };
        let mt2_spliced = match (&base, &theirs.coherent_elastic) {
            (Some(b), Some(rc)) if mt2.is_some() => {
                let two = endout(&spliced(&b.output, &run.output));
                let sp = read_back(tape_text(&two), mat, None);
                Some(compare_mt2(sp.coherent_elastic.as_ref().unwrap(), 1, rc, row_of(rc).unwrap()))
            }
            _ => None,
        };
        let sab_request = if base.is_none() {
            let req = SabRequest::new(material, Temperature::new::<kelvin>(t));
            let via = read_back(tape_text(&thermal_scattering_tape(&req).unwrap()), mat, Some(t));
            let m2 = match (&via.coherent_elastic, &theirs.coherent_elastic, &mt2) {
                (Some(vc), Some(rc), Some(_)) => Some(compare_mt2(vc, 0, rc, row_of(rc).unwrap())),
                _ => None,
            };
            Some((compare_mt4(material, &via, &theirs, t), m2))
        } else {
            None
        };
        let row = Row { t, mt4, mt2, mt2_spliced, sab_request, lthr: (lthr(&ours), lthr(&theirs)), gen_s };
        println!("{material:?} {t:7.1} K: {row:?}");
        if base.is_none() {
            base = Some(run);
        }
        rows.push(row);
    }
    Some(rows)
}

fn gate_mt4(material: SabMaterial, rows: &[Row], expect_temps: usize) {
    assert_eq!(rows.len(), expect_temps, "{material:?}: tabulated temperatures");
    let (via, via_mt2) = rows[0].sab_request.as_ref().expect("SabRequest comparison at the base temperature");
    assert!(via.n > 40_000 && via.worst < TOL && via.b_ident, "{material:?}: SabRequest path MT=4 {via:?}");
    if let Some(m) = via_mt2 {
        assert_eq!((m.n_edges.0, m.n_prefix_ident), (m.n_edges.1, m.n_edges.1), "{material:?}: SabRequest MT=2 {m:?}");
    }
    for r in rows {
        let m = &r.mt4;
        assert!(m.n > 40_000, "{material:?} {} K: only {} values compared", r.t, m.n);
        assert!(m.worst < TOL, "{material:?} {} K: MT=4 {m:?}", r.t);
        assert!(m.b_ident, "{material:?} {} K: B(1..6) differ", r.t);
        assert!(rel(m.teff.0, m.teff.1) < TOL, "{material:?} {} K: T_eff {:?}", r.t, m.teff);
    }
}

fn gate_graphite(material: SabMaterial, rows: &[Row]) {
    gate_mt4(material, rows, 10);
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.lthr, (1, 1), "{material:?} {} K: coherent elastic on both", r.t);
        let m = r.mt2.as_ref().unwrap();
        // Print round-off: within one unit in the 7th significant figure.
        let one_unit = 1.0 + 1e-6; // the unit itself is computed in floating point
        assert!(m.n_prefix > 60, "{material:?} {} K: {m:?}", r.t);
        assert!(m.worst_prefix_units7 <= one_unit && m.worst_tail_units7 <= one_unit, "{material:?} {} K: {m:?}", r.t);
        assert!(units7(m.s_total.0, m.s_total.1) <= one_unit, "{material:?} {} K: {m:?}", r.t);
        if i == 0 {
            // The base temperature: the single-temperature run IS the tape's row.
            assert_eq!((m.n_edges.0, m.n_prefix_ident), (m.n_edges.1, m.n_edges.1), "{material:?} {} K: {m:?}", r.t);
        } else {
            let s = r.mt2_spliced.as_ref().expect("spliced comparison");
            assert_eq!(s.n_edges.0, s.n_edges.1, "{material:?} {} K spliced: {s:?}", r.t);
            assert_eq!(s.n_prefix_ident, s.n_prefix, "{material:?} {} K spliced: {s:?}", r.t);
        }
    }
}

fn gate_sic(material: SabMaterial, rows: &[Row]) {
    gate_mt4(material, rows, 8);
    for r in rows {
        // Pins a RECORDED DIFFERENCE, not agreement: the deck path writes
        // incoherent elastic where the published evaluation is coherent (see
        // the module docs). If this starts failing, the elastic channel of the
        // demo path changed: re-measure and update the V&V record.
        assert_eq!(r.lthr, (2, 1), "{material:?} {} K: elastic LTHR (ours, tape)", r.t);
    }
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (8.7 MB ENDF/B-VIII.0 tape); runs by default")]
fn reactor_graphite_10p_reproduces_its_endf8_tape_at_every_temperature() {
    if let Some(rows) = measure(SabMaterial::ReactorGraphite10P, "tsl-reactor-graphite-10P.endf") {
        gate_graphite(SabMaterial::ReactorGraphite10P, &rows);
    }
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (8.7 MB ENDF/B-VIII.0 tape); runs by default")]
fn reactor_graphite_30p_reproduces_its_endf8_tape_at_every_temperature() {
    if let Some(rows) = measure(SabMaterial::ReactorGraphite30P, "tsl-reactor-graphite-30P.endf") {
        gate_graphite(SabMaterial::ReactorGraphite30P, &rows);
    }
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (7.2 MB ENDF/B-VIII.0 tape); runs by default")]
fn c_in_sic_mt4_reproduces_its_endf8_tape_at_every_temperature() {
    if let Some(rows) = measure(SabMaterial::CInSiC, "tsl-CinSiC.endf") {
        gate_sic(SabMaterial::CInSiC, &rows);
    }
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (7.2 MB ENDF/B-VIII.0 tape); runs by default")]
fn si_in_sic_mt4_reproduces_its_endf8_tape_at_every_temperature() {
    if let Some(rows) = measure(SabMaterial::SiInSiC, "tsl-SiinSiC.endf") {
        gate_sic(SabMaterial::SiInSiC, &rows);
    }
}

/// The whole deck through `run_deck` — every temperature in one LEAPR run, as
/// the evaluator ran it — against the whole published tape: MT=2 at every
/// temperature must be bit-identical, edges included.
fn whole_deck(material: SabMaterial, file: &str) {
    let Some(path) = reference_endf_or_skip(file, "leapr-htr10-tsl-vs-endf8") else {
        return;
    };
    let theirs = parse_mf7_at_temperature(&Tape::read_file(&path).unwrap(), material.mat(), None).unwrap();
    let run = run_deck(&deck(material)).expect("whole deck");
    let ours = read_back(run.write_text().unwrap(), material.mat(), None);
    let (oc, rc) = (ours.coherent_elastic.unwrap(), theirs.coherent_elastic.unwrap());
    assert_eq!(oc.temperatures_k, rc.temperatures_k);
    for j in 0..rc.temperatures_k.len() {
        let m = compare_mt2(&oc, j, &rc, j);
        println!("{material:?} whole deck, MT=2 at {} K: {m:?}", rc.temperatures_k[j]);
        assert_eq!(m.n_edges.0, m.n_edges.1, "{material:?}: retained Bragg edges");
        assert_eq!(m.n_prefix_ident, m.n_prefix, "{material:?} {} K: {m:?}", rc.temperatures_k[j]);
    }
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (8.7 MB ENDF/B-VIII.0 tape); runs by default")]
fn reactor_graphite_10p_whole_deck_mt2_is_the_published_tapes() {
    whole_deck(SabMaterial::ReactorGraphite10P, "tsl-reactor-graphite-10P.endf");
}

#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "reference-data test (8.7 MB ENDF/B-VIII.0 tape); runs by default")]
fn reactor_graphite_30p_whole_deck_mt2_is_the_published_tapes() {
    whole_deck(SabMaterial::ReactorGraphite30P, "tsl-reactor-graphite-30P.endf");
}

/// HTR-10's slider range is 20–1800 °C. Every one of the four laws must
/// generate at both ends (and at 300.15 K), through the demo path, with a
/// sane result: a finite, non-negative `S` everywhere, `T_eff >= T`, and the
/// tape tabulated at the request. Needs no reference data. Prints the
/// generation time (deck parse + LEAPR + tape text), which the V&V record
/// quotes from an isolated run of this test alone.
#[test]
fn every_htr10_law_generates_across_the_slider_range() {
    for material in [
        SabMaterial::ReactorGraphite10P,
        SabMaterial::ReactorGraphite30P,
        SabMaterial::CInSiC,
        SabMaterial::SiInSiC,
    ] {
        for t in [293.15, 300.15, 2073.15] {
            let t0 = Instant::now();
            let run = run_deck_at_temperature(&deck(material), t)
                .unwrap_or_else(|e| panic!("{material:?} at {t} K: {e}"));
            let text = run.write_text().unwrap();
            let dt = t0.elapsed().as_secs_f64();
            let law = read_back(text, material.mat(), Some(t));
            let ii = law.incoherent_inelastic.as_ref().expect("MT=4");
            assert_eq!(ii.tabulated_temperatures_k.len(), 1);
            assert!((ii.temperature_k - t).abs() < 1e-3 * t, "{material:?}: tabulated at {}", ii.temperature_k);
            let mut n_pos = 0usize;
            for row in &ii.s_tables {
                for &s in &row.s {
                    assert!(s.is_finite() && s >= 0.0, "{material:?} at {t} K: S = {s}");
                    n_pos += usize::from(s > 0.0);
                }
            }
            let teff = teff_at(&law, ii.temperature_k);
            assert!(teff >= t, "{material:?} at {t} K: T_eff {teff}");
            println!("{material:?} at {t} K: {dt:.3} s, T_eff {teff:.2} K, {n_pos} positive S, LTHR {}", lthr(&law));
            assert!(n_pos > 10_000, "{material:?} at {t} K: {n_pos} positive S");
        }
    }
}
