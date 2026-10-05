// SPDX-License-Identifier: GPL-3.0

//! **Total photon energy production, ENDF MT=442, against NJOY2016's HEATR**
//! (GitHub #535, phase H6a).
//!
//! MT=442 is what HEATR's energy-balance method subtracts from the kinematic
//! KERMA: the energy the evaluation's photons carry away. In this crate it is
//! [`PhotonProduction::eval`], and [`Kerma::with_energy_balance`] subtracts it.
//!
//! # Methodology
//!
//! - **Oracle:** NJOY2016 (`ac5adf5f33`) HEATR run with `local = 0` and
//!   `442` in `npk`, on ENDF/B-VIII.0 Fe-58 (MAT 2637) and Si-28 (MAT 1425),
//!   RECONR `err = 0.001`, 0 K. Tapes and decks:
//!   `reference-data/heatr/{fe58,si28}-ENDF8.0-0K-local0.*`.
//!   `local = 0` is required: HEATR calls `gheat`, the MF=12/MF=13 pass, only
//!   then (`heatr.f90:378`), so a `local = 1` MT=442 holds the MF=6 photons
//!   alone.
//! - **Ours:** `PhotonProduction::from_endf` on the same evaluation, RECONR
//!   at the same tolerance.
//! - **Which points, and how they are judged.** Every NJOY point with a
//!   non-zero value counts; none is excluded. HEATR writes MT=442 to 7
//!   significant figures, energies included, but inside narrow resonances its
//!   grid is spaced finer than that: Fe-58's PENDF holds 204189.888 and
//!   204189.925 eV, and MT=442 prints both as `2.041899+5`. A printed energy
//!   `E` therefore stands for anything in `E·(1 ± 5e-7)`, and on a steep
//!   resonance flank the value changes by percent across that interval. So
//!   each NJOY value is compared with the **envelope** of ours over the
//!   interval: ours at both ends, at `E`, and at every node of our own RECONR
//!   grid inside it (the cross sections are linear between nodes, so an
//!   interior resonance peak is a node). The measure is how far outside that
//!   envelope NJOY's value lies, relative to it. The plain `ours(E)/njoy − 1`
//!   is reported too, as its median, in three bands: below 1 eV, 1 eV to the
//!   first inelastic threshold, and above it.
//! - **Pass criterion:** every point within **1e-6** of the envelope: the
//!   7-figure print precision of NJOY's values (±5e-7) with the same again
//!   for ours. Fixed from the file format before the H6a numbers were judged
//!   against it.
//!
//! **The instrument changed twice while this was written, and why matters.**
//! A first version excluded points within 0.1 % of a reaction threshold (a
//! guess). A second excluded points whose printed energy equals a
//! neighbour's (1610 such energies on Fe-58). Both left resonance-flank
//! points at up to 9.6 %; tracing them to NJOY's full-precision PENDF grid
//! showed the abscissa's rounding as the cause, and the envelope measure
//! above is the one the format calls for. It excludes nothing.
//!
//! # How NJOY builds MT=442 (read from `heatr.f90`, 2026-10-05)
//!
//! - **MF=6 photons** (`ZAP = 0`): `nheat` adds `ebar·yld·σ`, the subsection's
//!   mean photon energy times its yield times the reaction cross section
//!   (`:1447-1454`). MF=12/13 data for an MT that also has MF=6 photons is
//!   skipped (`gheat`, `:5147-5161`).
//! - **MF=12 LO=2** cascades are first converted to constant LO=1 yields
//!   (`hconvr`, `:4553-5044`), level energies from the MF=12 `ES` (and MF=3
//!   `−QI` for a level without MF=12).
//! - **MF=12 / MF=13, not capture:** `gheat` adds `y·σ·Ē` (`:5319-5331`).
//! - **Capture with MF=12:** by energy balance, not from the photon lines:
//!   `σ·(E + Q − E/(A+1)) − Σ_k y_k·σ·E_R,k`, where `E_R = E_γ²/(2·m_n c²·(A+1))`
//!   is the recoil from emitting photon `k` (`:5276-5300`, `disgam` `:5671`).
//!
//! # Results (2026-10-05, this crate after H6a, NJOY2016 `ac5adf5f33`)
//!
//! **MT=442, the gate.** Worst distance outside the envelope, and median
//! plain `|ours/njoy − 1|`:
//!
//! | nuclide | points | E < 1 eV | 1 eV – first inelastic | above |
//! |---|---|---|---|---|
//! | Fe-58 | 34 277 | 2.2e-7 (median 7.5e-8) | 4.6e-7 (7.8e-8) | 3.2e-7 (5.5e-8) |
//! | Si-28 | 9 210 | 1.7e-7 (5.7e-8) | 3.4e-7 (6.7e-8) | 4.1e-7 (7.5e-8) |
//!
//! Every point of both materials is inside NJOY's print precision. Before
//! H6a the same comparison read: Fe-58 **0** everywhere (all its photons are
//! MF=6 or LO=2); Si-28 +6.8e-4 below 1 eV (capture from the photon lines
//! rather than the energy balance), 8 % median to the first inelastic
//! threshold, and 0.04 % to 1.5 % of NJOY's value above it.
//!
//! **MT=301, recorded and not asserted** (median `|ours/njoy − 1|` above the
//! first inelastic threshold; below it, unchanged by either constructor):
//!
//! | comparison | `QI` only (`from_reconr`) | `nheat` Q (`from_endf`), H6a | `from_endf`, H6b part 1 |
//! |---|---|---|---|
//! | Fe-58, `local = 1` | 0.76 (ours 0.23× NJOY at 2 MeV) | 0.11 (1.10× at 2 and 5 MeV) | **3.3e-4** |
//! | Si-28, `local = 1` | 0.38 | 0.31 | **2.3e-3** |
//! | Fe-58, `local = 0` | 1.00 (clamped to 0) | 1.35 (1.9× at 2 MeV, 2.9× at 5 MeV) | **3.9e-3** |
//! | Si-28, `local = 0` | 1.00 (clamped to 0) | 0.99 (1.9× at 2 MeV) | **7.4e-3** |
//!
//! The H6b column is `nheat`'s neutron side for the two-body channels
//! (2026-10-05): `disbar`'s anisotropic mean outgoing energy for elastic and
//! the discrete levels without MF=6, `σ·(E + q0)` for MT=600-849, and
//! `nheat`'s skip list (MT=4 beside its levels, the 103-107 sums beside
//! their partials, 16 beside 875-890, 18 beside 19). The worst points in
//! every column are at 14-150 MeV, where the continuum and MF=6 neutron means
//! (`conbar`, `sixbar`, H6b part 2) are still the kinematic estimate.
//!
//! How to read it:
//!
//! - **`QI` left the level energy out.** H3 with `QI` deposits only the
//!   recoil of a discrete level; NJOY's `q0 = 0` deposits recoil plus
//!   excitation. That was most of the `−75 %` the 2026-09-17 record could not
//!   separate, and with H6a's photons subtracted it drove the energy balance
//!   negative (clamped to 0). [`Kerma::from_endf`] applies `nheat`'s rule.
//! - **The `local = 0` miss is the `local = 1` miss, amplified.** NJOY's own
//!   values obey `local0 = local1 − MT442` exactly (Fe-58 at 2 MeV:
//!   1.40741e6 − 1.24625e6 = 1.6116e5), and so do ours, with MT=442 exact. At
//!   2 MeV ours was 10 % high at `local = 1` after H6a, which is 90 % of what
//!   is left after the photons are removed. ~~That residual is the neutron
//!   side (H6b, not ported).~~ **CORRECTED 2026-10-05 (H6b part 1):** it was
//!   the neutron side, and two defects in it: MT=4 heated beside its own
//!   levels (RECONR rebuilds MT=4, `nheat` skips it), and the levels'
//!   isotropic mean energy where NJOY uses MF=4's. Both fixed; see the H6b
//!   column.
//! - **Below 1 eV:** Si-28 agrees to 6e-8 at `local = 0` (capture with MF=12:
//!   the energy balance and the recoil). Fe-58 is 755× NJOY at `local = 0`
//!   and 3.3 % at `local = 1`: its capture photons are MF=6, for which NJOY
//!   deposits only the photon recoil (`tabsq6`), and its photon lines fall
//!   208 keV short of `Q`, which ours deposits. That is H6c's (capture
//!   recoil, `kchk`).
//! - **1 eV to the first inelastic threshold:** ~~4–17 % medians, unchanged by
//!   this work; elastic heating here is isotropic in the CM, NJOY uses MF=4's
//!   mean cosine (H6b/H7).~~ **CORRECTED 2026-10-05 (H6b part 1):** elastic
//!   now uses MF=4's mean cosine through `disbar`'s node chain. Si-28, whose
//!   capture photons are MF=12, matches NJOY at print precision here at both
//!   `local` settings (worst 4.0e-7 and 4.7e-7, gated by
//!   [`heating_matches_njoy_where_elastic_and_mf12_capture_are_the_only_channels`]).
//!   Fe-58's medians are 1.7 % (`local = 1`) and 6.3 % (`local = 0`), from
//!   its MF=6 capture (H6c); `from_reconr` stays at 4–17 %.
//!
//! Set `MT442_DEBUG=1` to print the eight worst points of each comparison.

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{build_emission_spectra, Kerma};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::photon::PhotonProduction;
use njoy_outram_park_fork::reconr::{eval_lin_lin, reconr, ReconrConfig, ReconrResult};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

struct Case {
    label: &'static str,
    endf: &'static str,
    mat: i32,
}

const CASES: &[Case] = &[
    Case { label: "fe58", endf: "n-026_Fe_058-ENDF8.0.endf", mat: 2637 },
    Case { label: "si28", endf: "n-014_Si_028-ENDF8.0.endf", mat: 1425 },
];

/// Half-width of a 7-significant-figure abscissa, relative.
const ABSCISSA: f64 = 5.0e-7;

/// Criterion for MT=442: within 1e-6 of the envelope everywhere.
const MT442_TOL: f64 = 1.0e-6;

fn setup(case: &Case, tag: &str) -> Option<(Tape, ReconrResult, Tape)> {
    let endf_path = reference_endf_or_skip(case.endf, tag)?;
    let pendf_path = reference_file_or_skip("heatr", &format!("{}-ENDF8.0-0K-local0.heatr.pendf", case.label), tag)?;
    let tape = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&pendf_path).expect("NJOY HEATR PENDF parses");
    let recon = reconr(&tape, &ReconrConfig { mat: case.mat, tolerance: 0.001, temperature: 0.0 }).expect("RECONR");
    Some((tape, recon, njoy))
}

/// One MF=3 section of NJOY's PENDF as `(E, value)` pairs.
fn njoy_mt(njoy: &Tape, mat: i32, mt: i32) -> Vec<(f64, f64)> {
    let sec = njoy.section(mat, 3, mt).unwrap_or_else(|| panic!("NJOY MT={mt}"));
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().expect("HEAD");
    cur.read_tab1().expect("TAB1").pairs
}

/// Lowest threshold among the discrete inelastic levels (MT=51..90).
fn first_inelastic(recon: &ReconrResult) -> f64 {
    recon
        .sections
        .iter()
        .filter(|s| (51..=90).contains(&i32::from(s.mt)))
        .filter_map(|s| s.pairs.iter().find(|p| p.1 > 0.0).map(|p| p.0))
        .fold(f64::INFINITY, f64::min)
}

/// Every node of every RECONR section strictly inside `(a, z)`.
fn nodes_in(recon: &ReconrResult, a: f64, z: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for s in &recon.sections {
        let i = s.pairs.partition_point(|p| p.0 <= a);
        out.extend(s.pairs[i..].iter().take_while(|p| p.0 < z).map(|p| p.0));
    }
    out
}

#[derive(Default)]
struct Band {
    n: usize,
    worst: f64,
    worst_e: f64,
    rel: Vec<f64>,
}

impl Band {
    fn add(&mut self, e: f64, plain: f64, outside: f64) {
        self.n += 1;
        self.rel.push(plain.abs());
        if outside > self.worst {
            self.worst = outside;
            self.worst_e = e;
        }
    }
    fn median(&mut self) -> f64 {
        if self.rel.is_empty() {
            return f64::NAN;
        }
        self.rel.sort_by(|a, b| a.partial_cmp(b).unwrap());
        self.rel[self.rel.len() / 2]
    }
}

/// Compare `ours` with NJOY's `(E, value)` pairs band by band, by the
/// envelope measure in the module docs; print and return `(band, worst
/// envelope distance, median plain |rel|, n)`.
fn envelope_compare(
    label: &str,
    njoy: &[(f64, f64)],
    recon: &ReconrResult,
    ours: impl Fn(f64) -> f64,
) -> Vec<(&'static str, f64, f64, usize)> {
    let e_inel = first_inelastic(recon);
    let mut bands = [Band::default(), Band::default(), Band::default()];
    let mut worst_pts: Vec<(f64, f64, f64, f64)> = Vec::new();
    for &(e, n) in njoy {
        if n == 0.0 {
            continue;
        }
        let o = ours(e);
        let (a, z) = (e * (1.0 - ABSCISSA), e * (1.0 + ABSCISSA));
        let (mut emin, mut emax) = (o, o);
        for x in [a, z].into_iter().chain(nodes_in(recon, a, z)) {
            let v = ours(x);
            emin = emin.min(v);
            emax = emax.max(v);
        }
        let outside = ((emin - n).max(n - emax)).max(0.0) / n.abs();
        let b = if e < 1.0 {
            0
        } else if e < e_inel {
            1
        } else {
            2
        };
        bands[b].add(e, o / n - 1.0, outside);
        worst_pts.push((outside, e, n, o));
    }
    if std::env::var("MT442_DEBUG").is_ok() {
        worst_pts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for w in worst_pts.iter().take(8) {
            println!("    DEBUG outside {:.3e} at E={:.9e} njoy {:.7e} ours {:.7e}", w.0, w.1, w.2, w.3);
        }
    }
    let names = ["E < 1 eV", "1 eV .. first inelastic", "above first inelastic"];
    println!("{label}: first inelastic threshold {e_inel:.6e} eV, {} NJOY points", njoy.len());
    let mut out = Vec::new();
    for (i, b) in bands.iter_mut().enumerate() {
        let med = b.median();
        println!(
            "  {:<24} n={:>6}  median |ours/njoy-1| {:.3e}  worst outside envelope {:.3e} at {:.6e} eV",
            names[i], b.n, med, b.worst, b.worst_e
        );
        out.push((names[i], b.worst, med, b.n));
    }
    for &e in &[1.0e-5, 1.0, 1.0e5, 2.0e6, 5.0e6, 1.4e7, 1.95e7] {
        let n = eval_lin_lin(njoy, e);
        let o = ours(e);
        println!("    E={e:>10.3e}  njoy {n:>12.5e}  ours {o:>12.5e}  ratio {:.6}", if n != 0.0 { o / n } else { f64::NAN });
    }
    out
}

/// **The H6a gate:** MT=442 matches NJOY2016's HEATR on Fe-58 and Si-28 at
/// the oracle's print precision, at every point.
#[test]
fn mt442_matches_njoy_at_print_precision() {
    for case in CASES {
        let tag = format!("mt442 {}", case.label);
        let Some((tape, recon, njoy)) = setup(case, &tag) else { return };
        let ours = PhotonProduction::from_endf(&tape, case.mat, &recon);
        assert!(ours.unhandled().is_empty(), "[{tag}] unvalued MF=6 photons {:?}", ours.unhandled());
        let bands = envelope_compare(&tag, &njoy_mt(&njoy, case.mat, 442), &recon, |e| ours.eval(e, &recon));
        for (band, worst, _, n) in bands {
            assert!(n > 0, "[{tag}] no NJOY points in band {band}");
            assert!(worst <= MT442_TOL, "[{tag}] {band}: {worst:.3e} outside the envelope (gate {MT442_TOL:.0e})");
        }
    }
}

/// Build `(QI-only kinematic KERMA, nheat-Q kinematic KERMA)` for a case.
fn kermas(tape: &Tape, recon: &ReconrResult, mat: i32) -> (Kerma, Kerma) {
    let nu = NuBar::from_endf(tape, mat).expect("nubar").unwrap_or_default();
    let chi = FissionSpectrum::from_endf_mf5(tape, mat).expect("chi").unwrap_or_default();
    let emission = build_emission_spectra(tape, mat);
    (
        Kerma::from_reconr(recon, &nu, &chi, &emission),
        Kerma::from_endf(tape, mat, recon, &nu, &chi, &emission),
    )
}

/// **Downstream, recorded and not asserted:** the kinematic-limit MT=301
/// against NJOY's `local = 1` MT=301 (photons deposited locally), for the
/// `QI`-only constructor and for [`Kerma::from_endf`]'s `nheat` Q rule.
///
/// ~~The neutron side is the kinematic estimate either way (isotropic two-body,
/// H5 spectra); H6b ports `nheat`'s own mean outgoing energies.~~
/// **CORRECTED 2026-10-05 (H6b part 1):** `from_endf` now uses `disbar`'s
/// mean outgoing energy for elastic and the discrete levels; the continuum
/// and MF=6 channels are still the kinematic estimate (H6b part 2).
#[test]
fn mt301_local1_kinematic_against_njoy_is_recorded() {
    for case in CASES {
        let tag = format!("mt301 local=1 {}", case.label);
        let Some((tape, recon, _)) = setup(case, &tag) else { return };
        let Some(path) = reference_file_or_skip("heatr", &format!("{}-ENDF8.0-0K-local1.heatr.pendf", case.label), &tag) else {
            return;
        };
        let njoy1 = Tape::read_file(&path).expect("NJOY local=1 PENDF");
        let (qi_only, nheat_q) = kermas(&tape, &recon, case.mat);
        let n301 = njoy_mt(&njoy1, case.mat, 301);
        envelope_compare(&format!("{tag} [QI only, from_reconr]"), &n301, &recon, |e| qi_only.eval(e));
        envelope_compare(&format!("{tag} [nheat q0, from_endf]"), &n301, &recon, |e| nheat_q.eval(e));
    }
}

/// **Downstream, recorded and not asserted:** this crate's energy-balance
/// MT=301 (`Kerma::from_endf(..).with_energy_balance(..)`, what the ACE
/// heating column carries when the deck runs HEATR) against NJOY's
/// `local = 0` MT=301, by the same measure; the `QI`-only kinematic arm is
/// shown beside it.
///
/// The photon side is H6a's and matches NJOY. ~~The neutron side is still the
/// kinematic estimate, because H6b (`nheat`'s per-reaction mean outgoing
/// neutron energy) is not ported, so agreement is expected only where
/// capture and elastic are the only open channels.~~ **CORRECTED 2026-10-05
/// (H6b part 1):** the two-body channels use `disbar`; only the continuum
/// and MF=6 neutron means (`conbar`, `sixbar`) remain kinematic. Results in
/// the module docs.
#[test]
fn mt301_energy_balance_against_njoy_is_recorded() {
    for case in CASES {
        let tag = format!("mt301 local=0 {}", case.label);
        let Some((tape, recon, njoy)) = setup(case, &tag) else { return };
        let photons = PhotonProduction::from_endf(&tape, case.mat, &recon);
        let (qi_only, nheat_q) = kermas(&tape, &recon, case.mat);
        let (qi_only, nheat_q) = (qi_only.with_energy_balance(&photons, &recon), nheat_q.with_energy_balance(&photons, &recon));
        let n301 = njoy_mt(&njoy, case.mat, 301);
        envelope_compare(&format!("{tag} [QI only, from_reconr]"), &n301, &recon, |e| qi_only.eval(e));
        envelope_compare(&format!("{tag} [nheat q0, from_endf]"), &n301, &recon, |e| nheat_q.eval(e));
    }
}

/// NJOY's `local = 1` HEATR tape for a case (the 2026-09-17 oracle).
fn njoy_local1(case: &Case, tag: &str) -> Option<Tape> {
    let path = reference_file_or_skip("heatr", &format!("{}-ENDF8.0-0K-local1.heatr.pendf", case.label), tag)?;
    Some(Tape::read_file(&path).expect("NJOY local=1 PENDF"))
}

/// **The H6b-part-1 gate:** where elastic scattering and MF=12 capture are
/// the only open channels (Si-28 below its first inelastic threshold),
/// `Kerma::from_endf` matches NJOY's MT=301 at print precision, with photons
/// deposited (`local = 1`) and with the energy balance (`local = 0`).
///
/// That tests three ports at once: `disbar`'s anisotropic elastic mean
/// energy with its 10 % node chain, capture's energy balance with the photon
/// recoil (H6a), and the MT=442 subtraction. Criterion: 1e-6 of the envelope
/// (module docs), fixed before the run.
///
/// Result (2026-10-05): `local = 1` worst 4.0e-7, `local = 0` worst 4.7e-7.
#[test]
fn heating_matches_njoy_where_elastic_and_mf12_capture_are_the_only_channels() {
    let case = &CASES[1];
    let tag = "heating-elastic-capture si28";
    let Some((tape, recon, njoy0)) = setup(case, tag) else { return };
    let Some(njoy1) = njoy_local1(case, tag) else { return };
    let (_, kin) = kermas(&tape, &recon, case.mat);
    let photons = PhotonProduction::from_endf(&tape, case.mat, &recon);
    let bal = kin.clone().with_energy_balance(&photons, &recon);
    for (label, njoy, k) in [("local=1", &njoy1, &kin), ("local=0", &njoy0, &bal)] {
        let bands = envelope_compare(&format!("{tag} {label}"), &njoy_mt(njoy, case.mat, 301), &recon, |e| k.eval(e));
        for (band, worst, _, n) in bands.into_iter().take(2) {
            assert!(n > 0, "[{label}] no points in {band}");
            assert!(worst <= MT442_TOL, "[{label}] {band}: {worst:.3e} outside the envelope");
        }
    }
}

/// **MT=4 is heated once.** RECONR rebuilds MT=4 as the sum of the levels,
/// and `nheat` never heats it (`heatr.f90:1074`). Heating it beside the
/// levels would add at least its recoil term `σ_4·E·2A/(A+1)²` (Q = 0 on
/// the rebuilt sum). Criterion, derived from that and not from the result:
/// at 2 MeV on Fe-58 ours is within **half** that amount of NJOY's
/// `local = 1` MT=301.
///
/// Result (2026-10-05): ours − NJOY = 4.75e2 eV·b; a double count would add
/// 8.00e4, so the bound is 4.0e4.
#[test]
fn mt4_is_not_heated_beside_the_levels() {
    let case = &CASES[0];
    let tag = "mt4-once fe58";
    let Some((tape, recon, _)) = setup(case, tag) else { return };
    let Some(njoy1) = njoy_local1(case, tag) else { return };
    let (_, kin) = kermas(&tape, &recon, case.mat);
    let e = 2.0e6;
    let a = recon.material.awr;
    let sigma4 = recon
        .sections
        .iter()
        .find(|s| i32::from(s.mt) == 4)
        .map(|s| eval_lin_lin(&s.pairs, e))
        .expect("RECONR carries the redundant MT=4");
    let double_count = sigma4 * e * 2.0 * a / ((a + 1.0) * (a + 1.0));
    let diff = kin.eval(e) - eval_lin_lin(&njoy_mt(&njoy1, case.mat, 301), e);
    println!("[{tag}] ours - NJOY at 2 MeV: {diff:.3e} eV·b; a double count would add {double_count:.3e}");
    assert!(diff.abs() < 0.5 * double_count, "MT=4 looks heated beside its levels: {diff:.3e} vs {double_count:.3e}");
}
