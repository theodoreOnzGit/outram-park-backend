//! RECONR inputs that were read and then ignored (audit of 2026-10-06), now
//! honoured as NJOY2016's RECONR honours them. GitHub #662, #663, #664.
//!
//! # Methodology
//!
//! Each test changes one ENDF input on a real or a hand-built MF=2 and checks
//! that RECONR responds the way upstream `reconr.f90` does:
//!
//! - **#663, NIS and LRU=2.** A hand-built two-isotope MF=2 (ENDF-102 §2
//!   layout: an MLBW range with an `NRO = 1` `AP(E)` table and an `LRU = 2`
//!   Case A range in isotope 1, an SLBW range in isotope 2) must parse into
//!   three ranges with the values written into it. Before the fix the `AP(E)`
//!   `TAB1` header was taken as the SPI/AP record and the `LRU = 2` body was
//!   left unread, so everything after it was read from the wrong records.
//! - **#663, NRO on a resolved range.** Ar-37 (TENDL-2023, MAT 1828, MLBW,
//!   `NAPS = 1`) is given an `NRO = 1` table. With `AP(E)` equal to the
//!   range's constant `AP` the reconstruction must be **bit-identical** to
//!   the unmodified tape (`csmlbw` with `NAPS = 1` then uses the same radius
//!   everywhere, `reconr.f90:2906-2919`). With `AP(E)` rising linearly from
//!   0.30 to 0.40 (×10⁻¹² cm) the change in MT=2 at every shared energy must
//!   equal the change in the directly evaluated MLBW elastic, with `AP(E)`
//!   in the phase shift and at the energy and the constant `AP` at the
//!   resonances, as `csmlbw` and `rdf2bw` split them. An `NRO = 1` SLBW
//!   range must be refused (`rdf2bw`: "only works for mlbw").
//! - **#664, ABN.** Ar-37 with `ABN = 0.5` against `ABN = 1`: the
//!   difference in MT=102 at every shared energy must be half the directly
//!   evaluated resonance capture (`sig(j)=sig(j)+sigp(j)*abn`,
//!   `reconr.f90:2641-2645`).
//! - **#662, temperature.** `ReconrConfig::temperature > 0` is refused
//!   (`NotPorted`: the ψ/χ path is not ported), and a negative one is invalid.
//!
//! # Results (2026-11-10, ENDF data as committed)
//!
//! Predicted before running: identity table bit-identical; linear table
//! lowers MT=2 near 1e-5 eV (AP 0.30 < 0.3388, potential scattering ∝ AP²:
//! about −22 %) and raises it near 4 keV (0.40 > 0.3388: about +39 %);
//! ABN = 0.5 halves the resonance capture. Measured:
//!
//! | check | result |
//! |---|---|
//! | NIS = 2 hand MF=2 | 3 ranges, every value as written |
//! | NRO, `AP(E) = AP` | MT=1, 2, 102 bit-identical to the plain tape |
//! | NRO, `AP(E)` 0.30 → 0.40 | 1328 shared energies, worst 8.15e-7 against the direct `csmlbw` form; MT=2 −21.57 % below 1 eV, +39.27 % above 4 keV |
//! | ABN 0.5 vs 1 | 1337 shared energies, worst 4.38e-7 against 0.5 × resonance capture |
//! | T = 900 K, −1 K, NaN | refused (`NotPorted`, `InvalidInput`, `InvalidInput`) |
//!
//! The 7-significant-figure rounding of both reconstructions bounds the
//! residuals at about 1e-6. Nothing held in `reference-data/endf/` has
//! NIS > 1, ABN ≠ 1 or NRO ≠ 0 on a resolved range, so no recorded
//! reconstruction moves.

use njoy_outram_park_fork::{
    endf::tape::{Section, Tape},
    reconr::{
        mf2::{parse_resonance_info, ResonanceInfo},
        reconr,
        slbw::{eval_mlbw_lstate, eval_mlbw_lstate_radii},
        ReconrConfig, ReconrResult,
    },
    NjoyError,
};

const MAT: i32 = 1828; // Ar-37, TENDL-2023

fn ar37() -> Tape {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir().join("n-018_Ar_37-tendl2023.endf");
    Tape::read_file(&p).expect("Ar-37 fixture")
}

/// `tape` with its MF=2/MT=151 rows replaced by `f(rows)`.
fn with_mf2(tape: &Tape, f: impl FnOnce(&mut Vec<[f64; 6]>)) -> Tape {
    let mut secs: Vec<Section> = tape.sections().to_vec();
    let sec = secs
        .iter_mut()
        .find(|s| s.key.mat == MAT && s.key.mf == 2 && s.key.mt == 151)
        .expect("MF=2/MT=151");
    f(&mut sec.rows);
    Tape::from_sections(String::from("synthetic"), secs)
}

fn run(tape: &Tape) -> Result<ReconrResult, NjoyError> {
    reconr(tape, &ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 })
}

fn mf2(tape: &Tape) -> ResonanceInfo {
    parse_resonance_info(tape.section(MAT, 2, 151).unwrap()).unwrap()
}

fn section(r: &ReconrResult, mt: i32) -> Vec<(f64, f64)> {
    r.sections.iter().find(|s| s.mt.number() == mt).unwrap().pairs.clone()
}

/// `(E, a, b)` at every energy both tables carry inside `[lo, hi]`.
fn shared(a: &[(f64, f64)], b: &[(f64, f64)], lo: f64, hi: f64) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();
    let mut j = 0;
    for &(e, x) in a {
        while j < b.len() && b[j].0 < e {
            j += 1;
        }
        if j < b.len() && b[j].0 == e && e >= lo && e <= hi {
            out.push((e, x, b[j].1));
        }
    }
    out
}

/// Ar-37's resolved range: insert an `NRO = 1` `TAB1` of `AP(E)` given as
/// `(E, AP)` pairs, lin-lin, right after the range CONT (row 2).
fn add_nro(rows: &mut Vec<[f64; 6]>, pts: [(f64, f64); 2]) {
    assert_eq!(rows[2][2], 1.0, "row 2 is the LRU=1 range CONT");
    rows[2][4] = 1.0; // NRO
    let tab = [
        [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [pts[0].0, pts[0].1, pts[1].0, pts[1].1, 0.0, 0.0],
    ];
    for (k, r) in tab.iter().enumerate() {
        rows.insert(3 + k, *r);
    }
}

/// The resolved MLBW contribution `(elastic, capture)` of `info`'s first
/// range at `e`, summed over l and clamped as `sigma` clamps it. `ap_e` is
/// `None` for the constant-radius form, `Some(AP(E))` for the NRO form with
/// `NAPS = 1` (radius at the energy and phase shift AP(E), at the
/// resonances the constant AP).
fn mlbw_delta(info: &ResonanceInfo, e: f64, ap_e: Option<f64>) -> (f64, f64) {
    let r = &info.ranges[0];
    let (mut el, mut cap) = (0.0, 0.0);
    for ls in &r.l_states {
        let t: Vec<_> = ls.resonances.iter().map(|x| x.as_tuple()).collect();
        let s = match ap_e {
            None => eval_mlbw_lstate(e, &t, ls.l, r.spi, r.ap, ls.awri, r.ap),
            Some(a) => eval_mlbw_lstate_radii(e, &t, ls.l, r.spi, a, ls.awri, a, r.ap),
        };
        el += s.elastic;
        cap += s.capture;
    }
    (el.max(0.0), cap.max(0.0))
}

/// Ar-37's own range bounds and radius, as the checks below use them.
const EH: f64 = 4.268035e3;
const AP: f64 = 3.387790e-1;

#[test]
fn nis2_with_nro_and_an_lru2_body_parses_every_range() {
    // ENDF-102 MF=2/MT=151, NIS = 2. Isotope 1 (ABN 0.6): MLBW with an
    // NRO = 1 AP(E) table, then an LRU = 2 Case A range. Isotope 2 (ABN 0.4):
    // one SLBW resonance.
    let rows: Vec<[f64; 6]> = vec![
        [18037.0, 36.64921, 0.0, 0.0, 2.0, 0.0],
        [18037.0, 0.6, 0.0, 0.0, 2.0, 0.0],
        [1.0e-5, 1.0e3, 1.0, 2.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 0.0, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [1.0e-5, 0.5, 1.0e3, 1.0, 0.0, 0.0],
        [0.5, 0.7, 0.0, 0.0, 1.0, 0.0],
        [36.0, 0.0, 0.0, 0.0, 6.0, 1.0],
        [500.0, 1.0, 1.1, 1.0, 0.1, 0.0],
        [1.0e3, 1.0e5, 2.0, 1.0, 0.0, 0.0],
        [0.5, 0.65, 0.0, 0.0, 1.0, 0.0],
        [36.0, 0.0, 0.0, 0.0, 6.0, 1.0],
        [10.0, 1.0, 1.0, 0.01, 0.5, 0.0],
        [18037.0, 0.4, 0.0, 0.0, 1.0, 0.0],
        [1.0e-5, 1.0e3, 1.0, 1.0, 0.0, 0.0],
        [0.0, 0.6, 0.0, 0.0, 1.0, 0.0],
        [36.0, 0.0, 0.0, 0.0, 6.0, 1.0],
        [200.0, 0.5, 1.0, 0.9, 0.1, 0.0],
    ];
    let sec = Section { key: ar37().sections()[0].key, rows };
    let info = parse_resonance_info(&sec).expect("NIS = 2 MF=2 parses");
    assert_eq!(info.ranges.len(), 3, "two ranges in isotope 1, one in isotope 2");

    let r0 = &info.ranges[0];
    assert_eq!((r0.isotope, r0.abn, r0.nro), (0, 0.6, 1));
    assert_eq!((r0.spi, r0.ap), (0.5, 0.7), "SPI/AP from the CONT after the AP(E) table");
    let t = r0.ap_table.as_ref().expect("AP(E) table kept");
    assert_eq!(t.xy, vec![(1.0e-5, 0.5), (1.0e3, 1.0)]);
    assert_eq!(r0.l_states.len(), 1);
    assert_eq!(r0.l_states[0].resonances[0].er, 500.0);

    let r1 = &info.ranges[1];
    assert_eq!((r1.lru, r1.isotope, r1.spi, r1.ap), (2, 0, 0.5, 0.65), "LRU = 2 header");

    let r2 = &info.ranges[2];
    assert_eq!((r2.lru, r2.isotope, r2.abn, r2.ap), (1, 1, 0.4, 0.6), "isotope 2 after the LRU = 2 body");
    assert_eq!(r2.l_states[0].resonances[0].er, 200.0);

    // RECONR does not yet sum overlapping isotopes, so it refuses NIS > 1
    // rather than return a silently wrong answer.
    let tape = with_mf2(&ar37(), |rows| *rows = sec.rows.clone());
    match run(&tape) {
        Err(NjoyError::NotPorted(m)) => assert!(m.contains("NIS > 1"), "{m}"),
        other => panic!("NIS = 2 must be refused, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn nro_is_refused_off_mlbw() {
    let tape = with_mf2(&ar37(), |rows| {
        add_nro(rows, [(1.0e-5, AP), (EH, AP)]);
        rows[2][3] = 1.0; // LRF = 1, SLBW
    });
    let err = parse_resonance_info(tape.section(MAT, 2, 151).unwrap()).expect_err("SLBW + NRO refused");
    assert!(matches!(err, NjoyError::InvalidInput(ref m) if m.contains("only works")), "{err}");
}

#[test]
fn nro_with_a_constant_radius_reproduces_the_plain_range_bit_for_bit() {
    let plain = run(&ar37()).unwrap();
    let tape = with_mf2(&ar37(), |rows| add_nro(rows, [(1.0e-5, AP), (EH, AP)]));
    assert_eq!(mf2(&tape).ranges[0].nro, 1);
    let nro = run(&tape).unwrap();
    for mt in [1, 2, 102] {
        assert_eq!(section(&plain, mt), section(&nro, mt), "MT={mt}");
    }
}

#[test]
fn nro_radius_enters_the_mlbw_elastic_as_csmlbw_uses_it() {
    let (a0, a1) = (0.30, 0.40);
    let ap_at = |e: f64| a0 + (e - 1.0e-5) * (a1 - a0) / (EH - 1.0e-5);
    let base = ar37();
    let tape = with_mf2(&base, |rows| add_nro(rows, [(1.0e-5, a0), (EH, a1)]));
    let info = mf2(&base);
    let (plain, nro) = (run(&base).unwrap(), run(&tape).unwrap());

    let pts = shared(&section(&plain, 2), &section(&nro, 2), 1.0e-5, 0.999 * EH);
    assert!(pts.len() > 100, "{} shared energies", pts.len());
    let (mut worst, mut lo_shift, mut hi_shift) = (0.0f64, 0.0, 0.0);
    for &(e, s0, s1) in &pts {
        let expect = s0 - mlbw_delta(&info, e, None).0 + mlbw_delta(&info, e, Some(ap_at(e))).0;
        let rel = (s1 - expect).abs() / s1.abs().max(s0.abs());
        worst = worst.max(rel);
        if e < 1.0 {
            lo_shift = (s1 - s0) / s0;
        }
        if e > 4.0e3 {
            hi_shift = (s1 - s0) / s0;
        }
    }
    println!("MT=2 with AP(E) 0.30-0.40: {} energies, worst {worst:.2e}, shift {lo_shift:+.4} below 1 eV, {hi_shift:+.4} above 4 keV", pts.len());
    assert!(worst < 2.0e-6, "MT=2 vs csmlbw with AP(E): worst rel {worst:e}");
    assert!(lo_shift < -0.05 && hi_shift > 0.05, "the table moves the answer: {lo_shift}, {hi_shift}");
}

#[test]
fn abundance_weights_the_resolved_contribution() {
    let base = ar37();
    assert_eq!(base.section(MAT, 2, 151).unwrap().rows[1][1], 1.0, "Ar-37 ABN = 1");
    let tape = with_mf2(&base, |rows| rows[1][1] = 0.5);
    assert_eq!(mf2(&tape).ranges[0].abn, 0.5);
    let info = mf2(&base);
    let (full, half) = (run(&base).unwrap(), run(&tape).unwrap());

    let pts = shared(&section(&full, 102), &section(&half, 102), 1.0e-5, 0.999 * EH);
    assert!(pts.len() > 100, "{} shared energies", pts.len());
    let mut worst = 0.0f64;
    for &(e, s1, sh) in &pts {
        let d = mlbw_delta(&info, e, None).1;
        let err = ((s1 - sh) - 0.5 * d).abs() / s1.abs();
        worst = worst.max(err);
    }
    println!("MT=102 ABN 1 vs 0.5: {} energies, worst {worst:.2e}", pts.len());
    assert!(worst < 2.0e-6, "capture difference is not ABN x resonance capture: {worst:e}");
}

#[test]
fn reconstruction_temperature_is_refused_not_ignored() {
    let tape = ar37();
    let at = |t: f64| reconr(&tape, &ReconrConfig { mat: MAT, tolerance: 0.001, temperature: t });
    assert!(matches!(at(900.0), Err(NjoyError::NotPorted(_))), "T > 0");
    assert!(matches!(at(-1.0), Err(NjoyError::InvalidInput(_))), "T < 0");
    assert!(matches!(at(f64::NAN), Err(NjoyError::InvalidInput(_))), "T = NaN");
    assert_eq!(at(0.0).unwrap().temperature_k, 0.0, "0 K data are labelled 0 K");
}
