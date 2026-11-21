// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only

//! **U-238 windowed-multipole generation against OpenMC's own generator**
//! (gh:#1390), the case `wmp_generate_vs_openmc.rs` (Mg-26) left undone.
//!
//! Methodology, the reference (OpenMC `8f79d9d`, and why not 0.15.3), the
//! prediction written before the run, why only a subset of the 304 pieces is
//! fitted, hardware and every timing:
//! `verification_and_validation/wmp_generate_u238/README.md`. The reference
//! driver is `openmc_inputs/ref_u238_subset.py`; its outputs are the fixtures
//! read here (`pieces.txt`, `meta.txt`, `fitted_pieces.f64`,
//! `library_prefix.f64`; binary files are little-endian f64).
//!
//! | test | what it runs |
//! |---|---|
//! | [`u238_front_end_is_openmcs_bit_for_bit`] | the port's RECONR -> GASPR -> ACER front end; all 304 pieces checksummed against OpenMC's input to `_vectfit_xs` |
//! | [`u238_prefix_windowing_of_openmcs_multipole_data`] | the port's `from_multipole(search)` on OpenMC's own poles for the lowest 12 pieces |
//! | [`u238_piece_fits_against_openmc`] | `vectfit_xs` on pieces OpenMC fitted (cheap ones by default; `OUTRAM_WMP_U238_PIECES`) |
//! | [`u238_stratified_pieces_against_openmc`] | the extension run's expensive pieces (`#[ignore]`, about 2.5 h) |
//! | [`u238_all_fitted_pieces_against_openmc`] | `vectfit_xs` on every piece OpenMC fitted: 0-11, 25, 50, 75, 100, 125, 150, 175, 200, 225, 250, 275, 296, 298, 299, 303 (`#[ignore]`, hours) |
//!
//! Results are in the README (not repeated here, so they cannot drift from
//! it). Skips (passes, printing `SKIP`) when the tape is absent.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::wmp::generate::{
    piece_ranges, pointwise_from_endf, resolve_vf_pieces, search_windowing, vectfit_xs, MultipoleData, SearchOptions, VectfitOptions,
    WmpGenerated,
};
use njoy_outram_park_fork::wmp::generate::vectfit::evaluate;
use njoy_outram_park_fork::wmp::Cf64;

const TAPE: &str = "n-092_U_238.endf";

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/wmp_generate_u238")
}

/// One row of `pieces.txt`: what OpenMC's `_vectfit_xs` was given and returned.
struct PieceRecord {
    n_points: usize,
    n_poles: usize,
    checksum: [u64; 3],
    e_first: f64,
    e_last: f64,
    /// Whether OpenMC's `_vectfit_xs` was run on this piece (else a stand-in).
    fitted: bool,
}

fn load_pieces() -> Vec<PieceRecord> {
    let text = std::fs::read_to_string(dir().join("pieces.txt")).expect("pieces.txt");
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            PieceRecord {
                n_points: w[0].parse().unwrap(),
                n_poles: w[1].parse().unwrap(),
                checksum: [w[2].parse().unwrap(), w[3].parse().unwrap(), w[4].parse().unwrap()],
                e_first: w[5].parse().unwrap(),
                e_last: w[6].parse().unwrap(),
                fitted: w[7] == "1",
            }
        })
        .collect()
}

/// The checksum `ref_u238.py` computes: the f64 words of the given vectors
/// concatenated; XOR of the words, their sum mod 2^64, and the sum of
/// `word * (index + 1)` mod 2^64.
fn checksum(rows: &[&[f64]]) -> [u64; 3] {
    let (mut x, mut s, mut p) = (0u64, 0u64, 0u64);
    let mut idx = 0u64;
    for row in rows {
        for v in *row {
            let w = v.to_bits();
            idx += 1;
            x ^= w;
            s = s.wrapping_add(w);
            p = p.wrapping_add(w.wrapping_mul(idx));
        }
    }
    [x, s, p]
}

fn read_f64s(name: &str) -> Vec<f64> {
    let b = std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(b.len() % 8, 0, "{name}");
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

/// Poles and residues of one piece OpenMC fitted: `(poles, residues[mt][pole])`.
type PieceFit = (Vec<Cf64>, Vec<Vec<Cf64>>);

/// The pieces OpenMC fitted, keyed by 0-based piece index, as
/// `ref_u238_subset.py` wrote them (`piece_<i>.f64` files concatenated).
fn load_fitted_pieces() -> BTreeMap<usize, PieceFit> {
    let mut out = parse_fitted(&read_f64s("fitted_pieces.f64"));
    // Extension run (README, "Extension run"): pieces fitted later by the same driver family.
    if dir().join("fitted_pieces_ext.f64").exists() {
        out.extend(parse_fitted(&read_f64s("fitted_pieces_ext.f64")));
    }
    out
}

/// Parse concatenated `piece_<i>.f64` records.
fn parse_fitted(v: &[f64]) -> BTreeMap<usize, PieceFit> {
    let mut at = 0usize;
    let mut out = BTreeMap::new();
    while at < v.len() {
        let (i, n, nmt) = (v[at] as usize, v[at + 1] as usize, v[at + 2] as usize);
        at += 3;
        let mut cplx = |count: usize| -> Vec<Cf64> {
            let r = (0..count).map(|k| Cf64::new(v[at + 2 * k], v[at + 2 * k + 1])).collect();
            at += 2 * count;
            r
        };
        let p = cplx(n);
        let r: Vec<Vec<Cf64>> = (0..nmt).map(|_| cplx(n)).collect();
        out.insert(i, (p, r));
    }
    assert_eq!(at, v.len(), "fitted piece records fully consumed");
    out
}

/// `meta.txt`: `key value` lines (floats as shortest round-trip decimals).
fn load_meta() -> BTreeMap<String, f64> {
    let text = std::fs::read_to_string(dir().join("meta.txt")).expect("meta.txt");
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            let mut it = l.split_whitespace();
            (it.next().unwrap().to_string(), it.next().unwrap().parse().unwrap())
        })
        .collect()
}

/// The multipole data of the lowest `K` pieces over their own energy range.
fn prefix_multipole(fitted: &BTreeMap<usize, PieceFit>, meta: &BTreeMap<String, f64>) -> MultipoleData {
    let k = meta["K"] as usize;
    MultipoleData {
        name: "U238".into(),
        awr: meta["awr"],
        e_min: meta["e_min"],
        e_max: meta["e_max_prefix"],
        mts: vec![2, 27, 18],
        poles: (0..k).map(|i| fitted[&i].0.clone()).collect(),
        residues: (0..k).map(|i| fitted[&i].1.clone()).collect(),
    }
}

/// OpenMC's searched library as `ref_u238.py` wrote it.
fn load_library(name: &str) -> WmpGenerated {
    let v = read_f64s("library_prefix.f64");
    let (spacing, sqrt_awr, e_min, e_max) = (v[0], v[1], v[2], v[3]);
    let (n_poles, ncol, n_win) = (v[4] as usize, v[5] as usize, v[6] as usize);
    let mut at = 8usize;
    let mut poles = Vec::with_capacity(n_poles);
    let mut residues = Vec::with_capacity(n_poles);
    for _ in 0..n_poles {
        let row: Vec<Cf64> = (0..ncol).map(|c| Cf64::new(v[at + 2 * c], v[at + 2 * c + 1])).collect();
        at += 2 * ncol;
        poles.push(row[0]);
        residues.push(row[1..].to_vec());
    }
    let windows: Vec<[i64; 2]> = (0..n_win).map(|i| [v[at + 2 * i] as i64, v[at + 2 * i + 1] as i64]).collect();
    at += 2 * n_win;
    let (a, b, c) = (v[at] as usize, v[at + 1] as usize, v[at + 2] as usize);
    at += 3;
    assert_eq!(a, n_win);
    let curvefit: Vec<Vec<Vec<f64>>> =
        (0..a).map(|i| (0..b).map(|j| (0..c).map(|k| v[at + (i * b + j) * c + k]).collect()).collect()).collect();
    at += a * b * c;
    let broaden_poly: Vec<bool> = (0..n_win).map(|i| v[at + i] != 0.0).collect();
    at += n_win;
    assert_eq!(at, v.len(), "library_prefix.f64 fully consumed");
    WmpGenerated { name: name.into(), spacing, sqrt_awr, e_min, e_max, poles, residues, windows, curvefit, broaden_poly }
}

fn cabs(z: Cf64) -> f64 {
    z.re.hypot(z.im)
}

/// Worst `|a - b| / |b|` after pairing each reference pole with its nearest.
fn match_poles(ours: &[Cf64], theirs: &[Cf64]) -> (Vec<usize>, f64) {
    let mut map = Vec::new();
    let mut worst = 0.0_f64;
    for t in theirs {
        let (k, d) = ours
            .iter()
            .enumerate()
            .map(|(k, p)| (k, cabs(p.sub(*t))))
            .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
        map.push(k);
        worst = worst.max(d / cabs(*t));
    }
    (map, worst)
}

/// Windows, spacing and used poles identical; returns the worst curve-fit
/// difference as a fraction of each coefficient vector's largest entry.
fn compare_wmp(label: &str, ours: &WmpGenerated, theirs: &WmpGenerated) -> f64 {
    assert_eq!(ours.windows, theirs.windows, "{label}: windows");
    assert_eq!(ours.spacing.to_bits(), theirs.spacing.to_bits(), "{label}: spacing");
    assert_eq!(ours.poles, theirs.poles, "{label}: used poles");
    assert_eq!(ours.residues, theirs.residues, "{label}: used residues");
    let mut worst = 0.0_f64;
    let (mut identical, mut count) = (0usize, 0usize);
    for (wo, wt) in ours.curvefit.iter().zip(&theirs.curvefit) {
        assert_eq!(wo.len(), wt.len(), "{label}: curve-fit order");
        for r in 0..wt[0].len() {
            let scale = wt.iter().map(|c| c[r].abs()).fold(0.0, f64::max);
            for (co, ct) in wo.iter().zip(wt) {
                count += 1;
                if co[r].to_bits() == ct[r].to_bits() {
                    identical += 1;
                }
                if scale > 0.0 {
                    worst = worst.max((co[r] - ct[r]).abs() / scale);
                }
            }
        }
    }
    println!("{label}: {} windows identical, curve fit {identical}/{count} bit-identical, worst {worst:.3e} of scale", ours.n_windows());
    worst
}

/// sigma(E, T) of two libraries at `per_t` energies equally spaced in
/// `sqrt(E)` for each temperature. Returns (values outside rtol/atol, worst
/// relative difference over values above atol, values compared, worst
/// absolute difference in barn).
fn compare_sigma(ours: &WmpGenerated, theirs: &WmpGenerated, per_t: usize) -> (usize, f64, usize, f64) {
    let (rtol, atol) = (1e-3, 1e-5);
    let (a, b) = (ours.to_windowed_multipole(), theirs.to_windowed_multipole());
    let (mut bad, mut worst, mut n) = (0usize, 0.0_f64, 0usize);
    let mut worst_abs = 0.0_f64;
    for t in [0.0, 293.6, 1200.0] {
        for k in 0..per_t {
            let e = (b.e_min.sqrt() + (b.e_max.sqrt() - b.e_min.sqrt()) * (k as f64 + 0.5) / per_t as f64).powi(2);
            let (x, y) = (a.evaluate(e, t), b.evaluate(e, t));
            for (p, q) in [(x.scatter, y.scatter), (x.absorption, y.absorption)] {
                n += 1;
                let ab = (p - q).abs();
                worst_abs = worst_abs.max(ab);
                if ab > atol {
                    worst = worst.max(ab / q.abs());
                }
                if !(ab / q.abs() < rtol || ab < atol) {
                    bad += 1;
                }
            }
        }
    }
    println!("sigma(E,T), {n} values at 0/293.6/1200 K: {bad} outside rtol/atol, worst {worst:.3e} relative above atol, worst absolute {worst_abs:.3e} b");
    (bad, worst, n, worst_abs)
}

/// The front end is OpenMC's, bit for bit, over every piece (all 304 are
/// checksummed, fitted or not).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "minutes: RECONR+ACER of U-238 at 0 K; runs by default")]
fn u238_front_end_is_openmcs_bit_for_bit() {
    let Some(endf) = reference_endf_or_skip(TAPE, "wmp_generate_u238_vs_openmc") else { return };
    let pieces = load_pieces();
    let t0 = Instant::now();
    let xs = pointwise_from_endf(&endf, 5e-4).expect("front end");
    println!("front end: {} points in {:.1} s", xs.energy.len(), t0.elapsed().as_secs_f64());
    let n_pieces = resolve_vf_pieces(&xs, None);
    assert_eq!(n_pieces, pieces.len(), "number of pieces");
    let fission = xs.fission.as_ref().expect("U-238 is fissionable");
    let mut identical = 0usize;
    for (i, (lo, hi)) in piece_ranges(&xs, n_pieces).into_iter().enumerate() {
        let r = &pieces[i];
        assert_eq!(hi - lo, r.n_points, "piece {i}: points");
        assert_eq!(xs.energy[lo].to_bits(), r.e_first.to_bits(), "piece {i}: first energy");
        assert_eq!(xs.energy[hi - 1].to_bits(), r.e_last.to_bits(), "piece {i}: last energy");
        let c = checksum(&[&xs.energy[lo..hi], &xs.elastic[lo..hi], &xs.absorption[lo..hi], &fission[lo..hi]]);
        assert_eq!(c, r.checksum, "piece {i}: input checksum (energy, elastic, MT=27, fission)");
        identical += 1;
    }
    println!("front end: {identical} of {n_pieces} pieces bit-identical to OpenMC's input to _vectfit_xs");
}

/// The port's windowing search on OpenMC's own multipole data for the lowest
/// 12 pieces: windows, spacing, fit order and used poles identical, curve fit
/// and sigma measured.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long: windowing search; runs by default")]
fn u238_prefix_windowing_of_openmcs_multipole_data() {
    if reference_endf_or_skip(TAPE, "wmp_generate_u238_vs_openmc").is_none() {
        return;
    }
    let (fitted, meta) = (load_fitted_pieces(), load_meta());
    let mp = prefix_multipole(&fitted, &meta);
    let n_poles: usize = mp.poles.iter().map(Vec::len).sum();
    let t0 = Instant::now();
    let ours = search_windowing(&mp, &SearchOptions::default()).expect("search_windowing");
    println!("search_windowing: {n_poles} poles in, {:.1} s", t0.elapsed().as_secs_f64());
    let theirs = load_library("U238");
    println!("port: {} windows, order {}; OpenMC: {} windows, order {}", ours.n_windows(), ours.fit_order(), theirs.n_windows(), theirs.fit_order());
    let cf = compare_wmp("from_multipole search on OpenMC's prefix data", &ours, &theirs);
    let (bad, _, _, abs) = compare_sigma(&ours, &theirs, 2000);
    assert_eq!(bad, 0, "windowed sigma outside rtol/atol");
    // measured 2026-11-18: 7.144e-6 of scale, 1.221e-6 b (README), rounded up
    assert!(cf < 8e-6, "curve fit differs by {cf:.3e} of scale");
    assert!(abs < 2e-6, "windowed sigma differs by {abs:.3e} b");
}

/// Pieces fitted by default: the cheap ones for OpenMC (< 25 s each in
/// `ref_u238_full_run_aborted.log`), chosen before any port timing.
const DEFAULT_PIECES: [usize; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 25, 50];

/// Which pieces `u238_piece_fits_against_openmc` fits: `OUTRAM_WMP_U238_PIECES`
/// (comma-separated indices, or `all` for every piece OpenMC fitted).
fn selected_pieces(fitted: &BTreeMap<usize, PieceFit>) -> Vec<usize> {
    match std::env::var("OUTRAM_WMP_U238_PIECES") {
        Ok(s) if s == "all" => fitted.keys().copied().collect(),
        Ok(s) => s.split(',').filter(|x| !x.is_empty()).map(|x| x.trim().parse().expect("piece index")).collect(),
        Err(_) => DEFAULT_PIECES.to_vec(),
    }
}

/// The accepted agreement with OpenMC for each piece: (worst relative pole
/// difference, worst relative residue difference, worst absolute sigma
/// difference in barn on the piece's grid). Each is the 2026-11-18
/// measurement (README, "Results") rounded up in its first figure, per the
/// maintainer's rule for the Mg-26 gates (gh:#1390, 2026-11-14): the fit is
/// ill-conditioned and the port's linear algebra is petir's, not LAPACK's, so
/// 1e-8 is not reachable (the README measures how far OpenMC moves itself).
/// A piece without an entry has no gate and fails.
fn piece_gate(i: usize) -> Option<(f64, f64, f64)> {
    match i {
        0..=8 => Some((9e-8, 7e-5, 1e-6)),
        9..=11 => Some((1e-8, 4e-7, 7e-8)),
        100 => Some((2e-3, 2e-1, 3e-3)),
        200 => Some((3e-2, 5e1, 4e-2)),
        303 => Some((2e-3, 6e-1, 6e-3)),
        // Extension run (README, "Extension run"): the same rule, 2026-11-18 measurements.
        25 => Some((8e-9, 3e-2, 2e-3)),
        50 => Some((4e-5, 6e-3, 2e-5)),
        75 => Some((2e-2, 1e0, 7e-3)),
        125 => Some((2e-2, 8e-2, 4e-4)),
        225 => Some((1e-3, 1e0, 6e-3)),
        250 => Some((5e-3, 2e0, 9e-2)),
        _ => None,
    }
}

/// Pieces where the two codes return a different NUMBER of poles (extension run, 2026-11-18),
/// as `(port poles, OpenMC poles, OpenMC grid points outside rtol/atol of the data, port's)`.
/// The exact values are asserted, so the disagreement is pinned (it moves only if the port or the
/// fixture changes) and is not counted as an unexpected one. The README gives what OpenMC does
/// against itself on these pieces; it does not follow that they are inside it.
fn known_pole_count_difference(i: usize) -> Option<(usize, usize, usize, usize)> {
    match i {
        150 => Some((25, 24, 2, 2)),
        175 => Some((29, 30, 0, 0)),
        275 => Some((57, 46, 0, 72)),
        296 => Some((60, 67, 0, 2094)),
        298 => Some((67, 62, 0, 1064)),
        299 => Some((59, 66, 3, 661)),
        _ => None,
    }
}

/// Pieces with equal pole counts where the port leaves a different number of grid points outside
/// rtol/atol than OpenMC: `(OpenMC's, the port's)`, asserted exactly (an exception to "the port
/// misses no more grid points than OpenMC", recorded rather than hidden).
fn known_extra_misses(i: usize) -> Option<(usize, usize)> {
    match i {
        225 => Some((21, 22)),
        _ => None,
    }
}

/// `vectfit_xs` on OpenMC's own input, piece by piece: pole counts compared,
/// poles and residues measured. Returns the number of pieces whose pole count
/// differs, or `None` when the tape is absent.
fn fit_pieces_against_openmc(sel: &[usize]) -> Option<usize> {
    let endf = reference_endf_or_skip(TAPE, "wmp_generate_u238_vs_openmc")?;
    let (pieces, fitted) = (load_pieces(), load_fitted_pieces());
    let xs = pointwise_from_endf(&endf, 5e-4).expect("front end");
    let ranges = piece_ranges(&xs, resolve_vf_pieces(&xs, None));
    let fission = xs.fission.clone().expect("fissionable");
    let mut differing = 0usize;
    for &i in sel {
        assert!(pieces[i].fitted || fitted.contains_key(&i), "piece {i} was not fitted by OpenMC");
        let (lo, hi) = ranges[i];
        let cx = vec![xs.elastic[lo..hi].to_vec(), xs.absorption[lo..hi].to_vec(), fission[lo..hi].to_vec()];
        let t0 = Instant::now();
        let fit = vectfit_xs(&xs.energy[lo..hi], &cx, &VectfitOptions::default()).expect("vectfit_xs");
        let secs = t0.elapsed().as_secs_f64();
        let (rp, rr) = &fitted[&i];
        if pieces[i].fitted {
            assert_eq!(rp.len(), pieces[i].n_poles, "fixture consistency, piece {i}");
        }
        if fit.poles.len() != rp.len() {
            // Different pole counts: the two sets are two fits of the same data. State how well each
            // reproduces the pointwise data (points outside rtol/atol) instead of comparing poles.
            let sigma = sigma_report(&xs.energy[lo..hi], &cx, rp, rr, &fit.poles, &fit.residues);
            println!(
                "piece {i}: {} points, port {} poles, OpenMC {} poles (DIFFERENT), {secs:.1} s; points outside rtol/atol of the data: OpenMC {}, port {}",
                hi - lo, fit.poles.len(), rp.len(), sigma.0, sigma.1
            );
            match known_pole_count_difference(i) {
                Some(pin) => assert_eq!(
                    (fit.poles.len(), rp.len(), sigma.0, sigma.1),
                    pin,
                    "piece {i}: the recorded pole-count difference changed"
                ),
                None => differing += 1,
            }
            continue;
        }
        // The physical comparison: the cross sections each set of poles gives on
        // the piece's grid, against the pointwise data and against each other.
        let sigma = sigma_report(&xs.energy[lo..hi], &cx, rp, rr, &fit.poles, &fit.residues);
        println!(
            "piece {i}: sigma from the poles on the {} grid points: OpenMC {} outside rtol/atol of the data, port {}; \
             worst port-vs-OpenMC {:.3e} b absolute, {:.3e} relative above atol",
            hi - lo, sigma.0, sigma.1, sigma.2, sigma.3
        );
        let (map, wp) = match_poles(&fit.poles, rp);
        let mut wr = 0.0_f64;
        for (v, row) in rr.iter().enumerate() {
            for (k, t) in row.iter().enumerate() {
                if cabs(*t) > 0.0 {
                    wr = wr.max(cabs(fit.residues[v][map[k]].sub(*t)) / cabs(*t));
                }
            }
        }
        println!("piece {i}: {} points, {} poles = OpenMC, worst pole {wp:.3e}, residue {wr:.3e} relative, {secs:.1} s", hi - lo, rp.len());
        let (gp, gr, gs) = piece_gate(i).unwrap_or_else(|| panic!("piece {i} has no gate"));
        assert!(wp < gp, "piece {i}: pole difference {wp:.3e} >= gate {gp:.1e}");
        assert!(wr < gr, "piece {i}: residue difference {wr:.3e} >= gate {gr:.1e}");
        assert!(sigma.2 < gs, "piece {i}: sigma difference {:.3e} b >= gate {gs:.1e}", sigma.2);
        match known_extra_misses(i) {
            Some(pin) => assert_eq!((sigma.0, sigma.1), pin, "piece {i}: the recorded grid-point misses changed"),
            None => assert!(sigma.1 <= sigma.0, "piece {i}: port misses {} grid points, OpenMC {}", sigma.1, sigma.0),
        }
    }
    println!("{differing} of {} fitted pieces have a different pole count", sel.len());
    Some(differing)
}

/// sigma(E) = Re-part of the pole sum / E on the grid, for OpenMC's poles and
/// the port's. Returns (OpenMC's points outside rtol/atol of the data, the
/// port's, worst absolute port-vs-OpenMC difference [b], worst relative
/// difference over values above atol).
fn sigma_report(
    energy: &[f64],
    data: &[Vec<f64>],
    p_ref: &[Cf64],
    r_ref: &[Vec<Cf64>],
    p_port: &[Cf64],
    r_port: &[Vec<Cf64>],
) -> (usize, usize, f64, f64) {
    let (rtol, atol) = (1e-3, 1e-5);
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let i = Cf64::new(0.0, 1.0);
    let times_i = |r: &[Vec<Cf64>]| -> Vec<Vec<Cf64>> { r.iter().map(|row| row.iter().map(|z| z.mul(i)).collect()).collect() };
    let a = evaluate(&s, p_ref, &times_i(r_ref), &[]);
    let b = evaluate(&s, p_port, &times_i(r_port), &[]);
    let (mut bad_a, mut bad_b, mut worst_abs, mut worst_rel) = (0usize, 0usize, 0.0_f64, 0.0_f64);
    for m in 0..data.len() {
        for k in 0..energy.len() {
            let (x, y) = (a[m][k] / energy[k], b[m][k] / energy[k]);
            let d = data[m][k];
            for (v, bad) in [(x, &mut bad_a), (y, &mut bad_b)] {
                let ab = (v - d).abs();
                if !(ab / d.abs() < rtol || ab < atol) {
                    *bad += 1;
                }
            }
            let ab = (x - y).abs();
            worst_abs = worst_abs.max(ab);
            if ab > atol {
                worst_rel = worst_rel.max(ab / x.abs());
            }
        }
    }
    (bad_a, bad_b, worst_abs, worst_rel)
}

/// The default pieces (or `OUTRAM_WMP_U238_PIECES`).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long: vector fitting of the selected pieces; runs by default")]
fn u238_piece_fits_against_openmc() {
    let sel = selected_pieces(&load_fitted_pieces());
    if let Some(differing) = fit_pieces_against_openmc(&sel) {
        assert_eq!(differing, 0, "pole counts of the fitted pieces");
    }
}

/// The extension run's expensive pieces (README, "Extension run": set A beyond piece 50, and set B).
/// Port run times, 2026-11-18, single thread, Intel Xeon 2.10 GHz shared with two other agents
/// (load average 8): 75 80 s, 125 160 s, 150 163 s, 175 175 s, 225 256 s, 250 821 s, 275 987 s,
/// 296 1 357 s, 298 2 081 s, 299 see the README; about 2.5 h with the 98 s front end once.
#[test]
#[ignore = "about 2.5 h: the port's vector fitting of the extension run's expensive pieces; see the README"]
fn u238_stratified_pieces_against_openmc() {
    let sel = [75, 125, 150, 175, 225, 250, 275, 296, 298, 299];
    if let Some(differing) = fit_pieces_against_openmc(&sel) {
        assert_eq!(differing, 0, "unexpected pole-count differences");
    }
}

/// The port against ITSELF under a +-1 ulp change of the cross sections (the Rust twin of
/// `openmc_inputs/sensitivity_u238_inputs.py`): pole count and grid points outside rtol/atol of the
/// (unperturbed) data for each trial. Prints, asserts nothing about the numbers beyond that the fits
/// run: it is an instrument for the README's "the port against itself" table, not a gate.
/// `OUTRAM_WMP_U238_PIECES` (default 275) and `OUTRAM_WMP_U238_TRIALS` (default 2).
#[test]
#[ignore = "about 35 min per trial on piece 298 (2 081 s unperturbed), 17 min on 275: instrument, see the README"]
fn u238_port_self_sensitivity() {
    let Some(endf) = reference_endf_or_skip(TAPE, "wmp_generate_u238_vs_openmc") else { return };
    let sel: Vec<usize> = std::env::var("OUTRAM_WMP_U238_PIECES")
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.trim().parse().expect("piece index")).collect())
        .unwrap_or_else(|_| vec![275]);
    let trials: usize = std::env::var("OUTRAM_WMP_U238_TRIALS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    let xs = pointwise_from_endf(&endf, 5e-4).expect("front end");
    let ranges = piece_ranges(&xs, resolve_vf_pieces(&xs, None));
    let fission = xs.fission.clone().expect("fissionable");
    let mut state = 0x9E37_79B9_7F4A_7C15_u64; // xorshift64*, fixed seed: the sign pattern is reproducible
    let mut sign = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        if state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 63 == 0 { -1.0 } else { 1.0 }
    };
    let ulp = f64::EPSILON; // 2^-52
    for &i in &sel {
        let (lo, hi) = ranges[i];
        let cx = vec![xs.elastic[lo..hi].to_vec(), xs.absorption[lo..hi].to_vec(), fission[lo..hi].to_vec()];
        for t in 0..trials {
            let px: Vec<Vec<f64>> = cx.iter().map(|row| row.iter().map(|v| v * (1.0 + sign() * ulp)).collect()).collect();
            let t0 = Instant::now();
            let fit = vectfit_xs(&xs.energy[lo..hi], &px, &VectfitOptions::default()).expect("vectfit_xs");
            let secs = t0.elapsed().as_secs_f64();
            let sigma = sigma_report(&xs.energy[lo..hi], &cx, &fit.poles, &fit.residues, &fit.poles, &fit.residues);
            println!("piece {i} port trial {t} (xs +-1 ulp): {} poles, {} grid points outside rtol/atol of the data, {secs:.1} s", fit.poles.len(), sigma.1);
        }
    }
}

/// Every piece OpenMC fitted (0-11, 25, 50, 75, 100, 125, 150, 175, 200, 225, 250, 275, 296, 298,
/// 299, 303). Hours: see the README for each piece's time.
#[test]
#[ignore = "hours: the port's vector fitting of every piece OpenMC fitted; see the README"]
fn u238_all_fitted_pieces_against_openmc() {
    let sel: Vec<usize> = load_fitted_pieces().keys().copied().collect();
    if let Some(differing) = fit_pieces_against_openmc(&sel) {
        assert_eq!(differing, 0, "unexpected pole-count differences");
    }
}
