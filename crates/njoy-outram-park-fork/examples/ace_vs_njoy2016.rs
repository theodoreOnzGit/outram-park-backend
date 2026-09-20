//! **Does this port's ACER reproduce NJOY2016's own ACE file?**
//!
//! Builds a continuous-energy ACE table through this crate's RECONR + ACER path
//! and compares it, block by block, against a Type-1 ASCII ACE file that
//! **NJOY2016 itself produced from the same ENDF tape**. NJOY is the
//! specification; anywhere the two differ, this port is wrong until shown
//! otherwise.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example ace_vs_njoy2016 -- <njoy_ace_file>
//! ```
//!
//! # The comparison is matched on purpose, and that took a deliberate choice
//!
//! This crate's [`examples/write_ace.rs`] runs **RECONR only, at 0 K**. The
//! production deck used to feed OpenMC
//! (`outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/make_ace.sh`)
//! runs **RECONR → BROADR(293.6 K) → PURR → ACER**.
//!
//! Comparing this port's 0 K RECONR-only table against that 293.6 K
//! RECONR+BROADR+PURR table would confound **three** differences at once —
//! Doppler broadening, unresolved probability tables, and any genuine ACER
//! discrepancy — and a disagreement could not be attributed to any of them.
//! So the oracle for this example is a **matched 0 K, RECONR→ACER-only** NJOY
//! run (`make_ace_0k.sh`). That isolates table construction, which is the one
//! thing this example claims to test.
//!
//! Broadening and probability tables are then separate comparisons, against
//! their own NJOY stages — `tests/broadr_light_nuclide_pendf_golden.rs` and
//! `tests/purr_u238_ptables_vs_njoy.rs` already do exactly that.
//!
//! # What is compared, and what is knowingly absent
//!
//! Header scalars (ZA, AWR, kT), the whole **NXS** and **JXS** arrays, and the
//! **ESZ** block (union energy grid, total, absorption, elastic, heating)
//! element by element.
//!
//! # Results — 2026-09-20, U-235 (MAT 9228), ENDF/B-VIII.0, 0 K
//!
//! Oracle: NJOY2016 built from `github.com/njoy/NJOY2016`, matched 0 K
//! RECONR→ACER deck (`make_ace_0k.sh`). Header agrees exactly — ZAID
//! `92235.00c`, AWR `233.024800` (rel `0.000e0`), `kT = 0`.
//!
//! | comparison | total | absorption | elastic |
//! |---|---|---|---|
//! | ours interpolated onto NJOY's grid | 3.355e-3 | 9.839e-3 | 6.297e-3 |
//! | **at the 2463 shared grid points** | **4.697e-7** | **1.527e-6** | **8.058e-7** |
//!
//! **Quote the second row.** The first is dominated by this comparison's own
//! lin-lin error across resonance peaks — all three of its worst cases land at
//! 1.4–2.2 keV, in the resolved resonance region, which is where interpolating
//! onto a point our adaptive grid did not choose under-shoots a peak. Removing
//! the interpolation improves the agreement by three orders of magnitude, so
//! **the port reproduces NJOY2016 to ~1e-6 relative, 6–7 significant figures**.
//!
//! Both grids span exactly `[1e-11, 30]` MeV; ours has 237 049 points against
//! NJOY's 233 515 (+1.51 %), ordinary adaptive-subdivision difference at the
//! same 0.001 tolerance. Only **1.1 %** of NJOY's energies are also in ours —
//! a small intersection, stated rather than glossed.
//!
//! **An earlier draft of this example compared `xss[i]` to `xss[i]` across the
//! two different unions** and reported absorption differing by a factor of
//! 1753. Index `i` is a different energy in each file; that number was entirely
//! an artefact of the misalignment. Both rows are kept above because the gap
//! between them is what justifies the method.
//!
//! ## What this port does not yet produce
//!
//! | gap | evidence | consequence |
//! |---|---|---|
//! | ~~fission ν̄ (NU)~~ **CLOSED 2026-09-20** | ours **bit-identical to NJOY, 347/347 values** | the table carries a fission source |
//! | ~~37 reactions~~ **CLOSED 2026-09-20** | ours **84**, NJOY 84, sets identical | MT=649 and MT=800–835 now stored; excluded from the ESZ sums under the lumped MT=103/107, so the ESZ numbers above are unchanged |
//! | photon production | `NXS(6)` ours `0`, NJOY `583` | no photon transport |
//!
//! No MT is in ours and absent from NJOY — a strict subset, not a divergence.
//!
//! Full record: `outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/ace_pipeline.md`.
//!
//! ~~`JXS(2)` — the fission ν̄ (NU) block — is **deferred and written as 0** by
//! this port.~~ **CLOSED 2026-09-20** — it is written by `acer::nu` and is
//! bit-identical to NJOY2016's, 347 of 347 values.
//!
//! The reporting convention that found it stays: a zero locator is reported as
//! an explicit ABSENT row rather than allowed to read as agreement, because a
//! missing block and a matching block are indistinguishable in a naive diff.
//! That convention is why this gap was visible at all, and it still guards
//! `NXS(6)` (photon production), which really is absent.

fn main() {
    desktop::run();
}

mod desktop {
    use njoy_outram_park_fork::{
        acer::{angular::parse_elastic_angular, energy::build_emissions, jxs, nxs, AceTable},
        endf::tape::Tape,
        heatr::{build_emission_spectra, Kerma},
        nuclear_data::secondary::{FissionSpectrum, NuBar},
        photon::PhotonProduction,
        reconr::{reconr, ReconrConfig},
    };
    use std::fs::File;

    /// A Type-1 ASCII ACE table, parsed into the same three pieces
    /// [`AceTable`] holds so the two can be diffed directly.
    struct NjoyAce {
        zaid: String,
        awr: f64,
        kt_mev: f64,
        nxs: [i32; 16],
        jxs: [i32; 32],
        xss: Vec<f64>,
    }

    /// Parse NJOY2016's Type-1 ASCII ACE layout.
    ///
    /// Line 1 `zaid awr tz date`; line 2 free comment + material id; lines 3-6
    /// the 16 IZ/AW pairs; then NXS as 2 lines of 8 integers, JXS as 4 lines of
    /// 8, and the XSS block 4 values to a line. Whitespace-splitting is enough
    /// for all of it — NJOY writes the fixed-width fields space-separated.
    fn parse_njoy_ace(path: &str) -> NjoyAce {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines.len() > 12,
            "{path} has {} lines; a Type-1 ACE header alone is 12",
            lines.len()
        );

        let h: Vec<&str> = lines[0].split_whitespace().collect();
        let zaid = h[0].to_string();
        let awr: f64 = h[1].parse().expect("AWR");
        let kt_mev: f64 = h[2].parse().expect("kT [MeV]");

        // Lines 3-6 are the IZ/AW pairs; NXS starts at line 7 (index 6).
        let ints: Vec<i32> = lines[6..12]
            .iter()
            .flat_map(|l| l.split_whitespace())
            .map(|t| t.parse::<i32>().unwrap_or_else(|e| panic!("int {t:?}: {e}")))
            .collect();
        assert_eq!(ints.len(), 48, "NXS(16) + JXS(32) should be 48 integers");
        let mut nxs = [0i32; 16];
        let mut jxs = [0i32; 32];
        nxs.copy_from_slice(&ints[..16]);
        jxs.copy_from_slice(&ints[16..]);

        let xss: Vec<f64> = lines[12..]
            .iter()
            .flat_map(|l| l.split_whitespace())
            .map(|t| t.parse::<f64>().unwrap_or_else(|e| panic!("xss {t:?}: {e}")))
            .collect();

        NjoyAce { zaid, awr, kt_mev, nxs, jxs, xss }
    }

    /// Worst relative difference between two slices, and where it occurred.
    /// Falls back to absolute difference where the reference is exactly zero,
    /// so a spurious `inf` cannot hide a real disagreement.
    fn worst_rel(ours: &[f64], theirs: &[f64]) -> (f64, usize) {
        let mut worst = 0.0f64;
        let mut at = 0usize;
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            let d = if *b == 0.0 { (a - b).abs() } else { ((a - b) / b).abs() };
            if d > worst {
                worst = d;
                at = i;
            }
        }
        (worst, at)
    }

    /// Boltzmann constant \[eV/K\] — kT\[MeV\] = k_B·T/1e6, the ACE header's
    /// temperature field.
    const BOLTZMANN_EV_PER_K: f64 = 8.617_333_262e-5;

    /// The evaluations this comparator knows how to build, keyed by MAT.
    ///
    /// `(mat, tape file)`. Extend here rather than in the caller, so a new
    /// nuclide is one line and the rest of the pipeline is shared.
    const KNOWN: &[(i32, &str)] = &[
        (9225, "n-092_U_234-ENDF8.0.endf"),
        (9228, "n-092_U_235-ENDF8.0.endf"),
        (9237, "n-092_U_238.endf"),
    ];


    pub fn run() {
        let njoy_path = std::env::args().nth(1).unwrap_or_else(|| {
            eprintln!(
                "usage: ace_vs_njoy2016 <njoy_type1_ace_file> [--mat N] [--temp-k K] \
                 [--dump-oracle PATH]\n\
                 \n\
                 --mat defaults to 9228 (U-235); --temp-k defaults to 0 (no BROADR)."
            );
            std::process::exit(2);
        });
        let argv: Vec<String> = std::env::args().collect();
        let flag = |name: &str| -> Option<String> {
            argv.iter().position(|a| a == name).and_then(|i| argv.get(i + 1).cloned())
        };
        let mat: i32 = flag("--mat").and_then(|v| v.parse().ok()).unwrap_or(9228);
        let temp_k: f64 = flag("--temp-k").and_then(|v| v.parse().ok()).unwrap_or(0.0);

        let theirs = parse_njoy_ace(&njoy_path);

        // ── Ours: the identical pipeline examples/write_ace.rs uses ──────────
        let tape_file = KNOWN
            .iter()
            .find(|&&(m, _)| m == mat)
            .map(|&(_, f)| f)
            .unwrap_or_else(|| {
                eprintln!("unknown MAT {mat}; known: {:?}", KNOWN.iter().map(|&(m, _)| m).collect::<Vec<_>>());
                std::process::exit(2);
            });
        println!("comparing MAT {mat} ({tape_file}) at {temp_k} K against {njoy_path}\n");
        let path = njoy_outram_park_fork::reference_data::reference_endf_dir().join(tape_file);
        let tape = Tape::read(File::open(&path).expect("open ENDF")).expect("parse ENDF");
        let cfg = ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 };
        let result = reconr(&tape, &cfg).expect("RECONR");
        // BROADR at the requested temperature. At 0 K this is skipped entirely
        // so the 0 K path stays byte-for-byte what it was.
        let result = if temp_k > 0.0 {
            njoy_outram_park_fork::broadr::broaden_result(&result, temp_k)
        } else {
            result
        };
        let angular = tape
            .section(mat, 4, 2)
            .map(|s| parse_elastic_angular(s).expect("parse MF=4"));
        let partials: Vec<(i32, f64)> = result
            .sections
            .iter()
            .map(|s| (i32::from(s.mt), s.qi))
            .collect();
        let emissions = build_emissions(&tape, mat, result.material.awr, &partials);
        let nu = NuBar::from_endf(&tape, mat).expect("MF=1").unwrap_or_default();
        let chi = FissionSpectrum::from_endf_mf5(&tape, mat).expect("MF=5").unwrap_or_default();
        let emission = build_emission_spectra(&tape, mat);
        let photons = PhotonProduction::from_endf(&tape, mat, &result);
        let kerma = Kerma::from_reconr(&result, &nu, &chi, &emission)
            .with_energy_balance(&photons, &result);
        // The ACE NU block (fission nu-bar); None for a non-fissile nuclide.
        let nu_block = njoy_outram_park_fork::acer::nu::build(&tape, mat).expect("NU block");
        match njoy_outram_park_fork::acer::photon_blocks::build(&tape, mat) {
            None => println!("photon blocks: REFUSED (a form this port does not write)"),
            Some(v) if v.is_empty() => println!("photon blocks: none found on the tape"),
            Some(v) => println!("photon blocks: {} entries built", v.len()),
        }

        let ours = AceTable::from_reconr_full(
            &result,
            temp_k * BOLTZMANN_EV_PER_K / 1.0e6,
            0,
            angular.as_ref(),
            &emissions,
            Some(&kerma),
            nu_block.as_deref(),
            njoy_outram_park_fork::acer::has_mt19_distributions(&tape, mat),
        njoy_outram_park_fork::acer::photon_blocks::build(&tape, mat).as_deref(),
        );

        // ── Header ───────────────────────────────────────────────────────────
        println!("=== header ===");
        println!("  {:<10} ours {:>14}   njoy {:>14}", "ZAID", ours.zaid.trim(), theirs.zaid);
        println!(
            "  {:<10} ours {:>14.6} njoy {:>14.6}   rel {:.3e}",
            "AWR",
            ours.awr,
            theirs.awr,
            ((ours.awr - theirs.awr) / theirs.awr).abs()
        );
        println!("  {:<10} ours {:>14.6e} njoy {:>14.6e}", "kT [MeV]", ours.kt_mev, theirs.kt_mev);
        if (ours.kt_mev - theirs.kt_mev).abs() > 1e-12 {
            println!(
                "  !! TEMPERATURE MISMATCH -- ours and the reference were not built at the\n     \
                 same temperature, so every block difference below is confounded with Doppler\n     \
                 broadening and cannot be attributed to ACER. Pass --temp-k to match the\n     \
                 reference deck (0 for a RECONR-only table, 293.6 for the broadened set)."
            );
        }

        // ── NXS / JXS ────────────────────────────────────────────────────────
        println!("\n=== NXS (table dimensions) ===");
        let nxs_names = [
            (nxs::LEN_XSS, "LEN_XSS"), (nxs::ZA, "ZA"), (nxs::NES, "NES"),
            (nxs::NTR, "NTR"), (nxs::NR, "NR"), (nxs::NTRP, "NTRP"),
        ];
        for (i, name) in nxs_names {
            let (a, b) = (ours.nxs[i], theirs.nxs[i]);
            println!("  NXS({:<2}) {:<8} ours {:>10}  njoy {:>10}  {}",
                i + 1, name, a, b, if a == b { "ok" } else { "DIFFER" });
        }

        println!("\n=== JXS (block locators) ===");
        let jxs_names = [
            (jxs::ESZ, "ESZ"), (jxs::NU, "NU"), (jxs::MTR, "MTR"), (jxs::LQR, "LQR"),
            (jxs::TYR, "TYR"), (jxs::LSIG, "LSIG"), (jxs::SIG, "SIG"),
            (jxs::LAND, "LAND"), (jxs::AND, "AND"), (jxs::LDLW, "LDLW"), (jxs::DLW, "DLW"),
        ];
        for (i, name) in jxs_names {
            let (a, b) = (ours.jxs[i], theirs.jxs[i]);
            let verdict = match (a, b) {
                (0, 0) => "both absent",
                (0, _) => "ABSENT IN OURS",
                (_, 0) => "absent in njoy",
                _ if a == b => "ok",
                _ => "DIFFER",
            };
            println!("  JXS({:<2}) {:<5} ours {:>10}  njoy {:>10}  {}", i + 1, name, a, b, verdict);
        }

        // ── ESZ, compared on a COMMON energy grid ───────────────────────────
        //
        // The two unions are NOT the same length, and they are not meant to be:
        // RECONR chooses grid points adaptively, so a 1 % difference in point
        // count is ordinary. Comparing xss[i] against xss[i] across two
        // different unions compares DIFFERENT ENERGIES and is meaningless -- an
        // earlier draft of this example did exactly that and reported a "1753x
        // disagreement" in absorption that was entirely an artefact of the
        // misalignment. The fix is to evaluate both on one grid.
        //
        // NJOY's grid is the reference, so ours is interpolated onto it.
        // ACE ESZ cross sections are lin-lin in energy between grid points,
        // which is the interpolation used here.
        println!("\n=== ESZ block, ours interpolated onto NJOY's grid ===");
        let nes_o = ours.nxs[nxs::NES] as usize;
        let nes_t = theirs.nxs[nxs::NES] as usize;
        let eo = (ours.jxs[jxs::ESZ] - 1) as usize;
        let et = (theirs.jxs[jxs::ESZ] - 1) as usize;
        let e_ours = &ours.xss[eo..eo + nes_o];
        let e_theirs = &theirs.xss[et..et + nes_t];

        println!(
            "  grid: ours {} pts [{:.6e}, {:.6e}] MeV",
            nes_o, e_ours[0], e_ours[nes_o - 1]
        );
        println!(
            "  grid: njoy {} pts [{:.6e}, {:.6e}] MeV   ({:+.2} % more points in ours)",
            nes_t,
            e_theirs[0],
            e_theirs[nes_t - 1],
            100.0 * (nes_o as f64 / nes_t as f64 - 1.0)
        );

        /// Lin-lin interpolation of `(xs)` on grid `(e)` at `at`.
        fn interp(e: &[f64], xs: &[f64], at: f64) -> f64 {
            match e.binary_search_by(|p| p.partial_cmp(&at).unwrap()) {
                Ok(i) => xs[i],
                Err(0) => xs[0],
                Err(i) if i >= e.len() => xs[e.len() - 1],
                Err(i) => {
                    let (e0, e1) = (e[i - 1], e[i]);
                    let f = if e1 > e0 { (at - e0) / (e1 - e0) } else { 0.0 };
                    xs[i - 1] + f * (xs[i] - xs[i - 1])
                }
            }
        }

        // Compare only where both grids have support, so an endpoint
        // extrapolation cannot masquerade as a disagreement.
        let lo = e_ours[0].max(e_theirs[0]);
        let hi = e_ours[nes_o - 1].min(e_theirs[nes_t - 1]);
        println!("  compared over the shared span [{lo:.6e}, {hi:.6e}] MeV");

        // NOTE: the 0 K oracle deck is RECONR -> ACER with no HEATR stage, so
        // NJOY's heating column is identically zero there while this port
        // always computes a KERMA. Those zeros are skipped below, which is why
        // `heating` reports n=0 -- that is the DECK differing, not the port.
        for (k, label) in [(1, "total"), (2, "absorption"), (3, "elastic"), (4, "heating")] {
            let xs_o = &ours.xss[eo + k * nes_o..eo + (k + 1) * nes_o];
            let xs_t = &theirs.xss[et + k * nes_t..et + (k + 1) * nes_t];
            let (mut worst, mut at_e, mut a_w, mut b_w) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut n_cmp = 0usize;
            for (j, &e) in e_theirs.iter().enumerate() {
                if e < lo || e > hi {
                    continue;
                }
                let b = xs_t[j];
                if b == 0.0 {
                    continue;
                }
                let a = interp(e_ours, xs_o, e);
                let d = ((a - b) / b).abs();
                if d > worst {
                    worst = d;
                    at_e = e;
                    a_w = a;
                    b_w = b;
                }
                n_cmp += 1;
            }
            println!(
                "  {label:<11} n={n_cmp:<7} worst rel {worst:.3e} at {at_e:.6e} MeV  \
                 (ours {a_w:.6e} njoy {b_w:.6e})"
            );
        }

        // ── The interpolation-free comparison ────────────────────────────────
        //
        // The loop above interpolates OURS onto NJOY's energies, so wherever
        // NJOY has a point that our adaptive grid did not choose, a lin-lin
        // segment across a resonance peak under-shoots it. That error is the
        // COMPARISON's, not the port's, and it lands hardest exactly where the
        // worst differences above were found (1-2 keV, resolved resonances).
        //
        // At energies BOTH grids contain, no interpolation happens at all, so
        // this is the figure that actually measures the port.
        // WHERE OUR GRID COMES FROM vs where NJOY's does. `acelod` takes the
        // ACE energy grid straight off MF=3 MT=1 of the PENDF
        // (`acefc.f90:5343`, `call findf(matd,3,1,nin)`); this port instead
        // builds the UNION of elastic and every stored partial. At 0 K those
        // are nearly the same set, because RECONR already wrote every MT on a
        // common union grid. After BROADR they are not: BROADR thins each MT
        // separately to `errthn`, so the union over ~85 sections is far denser
        // than MT=1's own thinned grid. Printed so the size gap is attributable
        // rather than mysterious.
        if let Some(mt1) = result.sections.iter().find(|s| i32::from(s.mt) == 1) {
            println!("\n=== grid provenance ===");
            println!(
                "  broadened MF=3 MT=1 has {} points — this is the grid `acelod` would use",
                mt1.pairs.len()
            );
            println!(
                "  our ACE grid has {} points — the union over elastic + stored partials",
                nes_o
            );
            println!("  NJOY's ACE grid has {nes_t} points");
        }

        // FISSION CONSISTENCY. MT=18 is defined as the sum of the partial
        // chances, so if both are on the tape they must agree. They do NOT
        // always: RECONR adds the resonance contribution to MT=18, while the
        // MF=3 partials may carry only the smooth background. A table whose
        // total is rebuilt from the partials then LOSES resonance fission —
        // silently, because every individual section is still self-consistent.
        {
            let get = |mt: i32| result.sections.iter().find(|s| i32::from(s.mt) == mt);
            if let Some(m18) = get(18) {
                let parts: Vec<_> = [19, 20, 21, 38].iter().filter_map(|&m| get(m)).collect();
                if !parts.is_empty() {
                    for p in &parts {
                        println!(
                            "  MT={:<4} spans [{:.4e}, {:.4e}] eV, {} points",
                            i32::from(p.mt),
                            p.pairs.first().map(|&(e, _)| e).unwrap_or(0.0),
                            p.pairs.last().map(|&(e, _)| e).unwrap_or(0.0),
                            p.pairs.len()
                        );
                    }
                    // Compare ONLY where every partial is defined. Below the
                    // threshold of a higher-chance channel the partials are
                    // legitimately zero, and counting that as disagreement is
                    // the same below-range artefact this file already warns
                    // about for the interpolated ESZ row.
                    let lo = parts
                        .iter()
                        .filter_map(|p| p.pairs.first().map(|&(e, _)| e))
                        .fold(0.0f64, f64::max);
                    println!("  comparing only at E >= {lo:.4e} eV (all partials defined)");
                    let mut worst = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                    for &(e, x18) in &m18.pairs {
                        if x18 <= 0.0 || e < lo {
                            continue;
                        }
                        let sum: f64 = parts
                            .iter()
                            .map(|p| njoy_outram_park_fork::reconr::eval_lin_lin(&p.pairs, e))
                            .sum();
                        let d = ((sum - x18) / x18).abs();
                        if d > worst.0 {
                            worst = (d, e, sum, x18);
                        }
                    }
                    println!("\n=== fission consistency: MT=18 vs sum(19,20,21,38) ===");
                    println!(
                        "  worst rel {:.3e} at {:.6e} eV  (sum {:.6e}, MT=18 {:.6e})",
                        worst.0, worst.1, worst.2, worst.3
                    );
                    if worst.0 > 1.0e-3 {
                        println!(
                            "  !! The partials do NOT sum to MT=18. Rebuilding the ESZ total from\n  \
                             the partials would lose {:.1} % of fission at that energy.",
                            100.0 * (worst.3 - worst.2) / worst.3
                        );
                    }
                }
            }
        }

        println!("\n=== ESZ at shared grid points (no interpolation) ===");
        // SPLIT AT THE BROADENING LIMIT. Above `thnmax` neither code runs
        // SIGMA1 (`broadr.f90:441`, ported as `broadr::broadening_limit`), so
        // both tables there are the *same* unbroadened RECONR output and the
        // comparison says nothing about Doppler broadening — it re-measures the
        // 0 K agreement. Only points BELOW the limit test the broadening.
        //
        // Reporting one pooled number across both regions would let the large,
        // easy, unbroadened population hide a disagreement in the small, hard,
        // broadened one. That is the same failure as the interpolated row
        // above, one level subtler.
        let thnmax_ev = njoy_outram_park_fork::broadr::broadening_limit(&result);
        let thnmax_mev = thnmax_ev / 1.0e6;
        println!(
            "  broadening limit (thnmax) = {:.6e} MeV — below it both codes broaden, \
             above it neither does",
            thnmax_mev
        );
        let mut shared = 0usize;
        let mut worst = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); 3];
        let mut shared_lo = 0usize;
        let mut worst_lo = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); 3];
        let labels = ["total", "absorption", "elastic"];
        let mut i = 0usize;
        let mut j = 0usize;
        while i < nes_o && j < nes_t {
            let (a, b) = (e_ours[i], e_theirs[j]);
            if a == b {
                for (k, w) in worst.iter_mut().enumerate() {
                    let col = k + 1;
                    let xo = ours.xss[eo + col * nes_o + i];
                    let xt = theirs.xss[et + col * nes_t + j];
                    if xt != 0.0 {
                        let d = ((xo - xt) / xt).abs();
                        if d > w.0 {
                            *w = (d, a, xo, xt);
                        }
                    }
                }
                if a < thnmax_mev {
                    for (k, w) in worst_lo.iter_mut().enumerate() {
                        let col = k + 1;
                        let xo = ours.xss[eo + col * nes_o + i];
                        let xt = theirs.xss[et + col * nes_t + j];
                        if xt != 0.0 {
                            let d = ((xo - xt) / xt).abs();
                            if d > w.0 {
                                *w = (d, a, xo, xt);
                            }
                        }
                    }
                    shared_lo += 1;
                }
                shared += 1;
                i += 1;
                j += 1;
            } else if a < b {
                i += 1;
            } else {
                j += 1;
            }
        }
        println!(
            "  {shared} of NJOY's {nes_t} grid energies are also in ours ({:.1} %)",
            100.0 * shared as f64 / nes_t as f64
        );
        for (k, w) in worst.iter().enumerate() {
            println!(
                "  {:<11} worst rel {:.3e} at {:.6e} MeV  (ours {:.6e} njoy {:.6e})",
                labels[k], w.0, w.1, w.2, w.3
            );
        }
        println!(
            "\n  -- of those, {shared_lo} are BELOW thnmax (the ones that actually test \
             Doppler broadening) --"
        );
        if shared_lo == 0 {
            println!(
                "  NONE. Every shared point is above the broadening limit, so this run \
                 measures\n  the unbroadened table only and must NOT be reported as \
                 agreement on broadening."
            );
        } else {
            for (k, w) in worst_lo.iter().enumerate() {
                println!(
                    "  {:<11} worst rel {:.3e} at {:.6e} MeV  (ours {:.6e} njoy {:.6e})",
                    labels[k], w.0, w.1, w.2, w.3
                );
            }
        }

        // ── Photon production blocks ─────────────────────────────────────
        println!("\n=== photon production (MTRP / SIGP / DLWP) ===");
        {
            let ntrp_o = ours.nxs[nxs::NTRP] as usize;
            let ntrp_t = theirs.nxs[nxs::NTRP] as usize;
            println!("  NTRP: ours {ntrp_o}, njoy {ntrp_t}");
            let mtrp = |t: &NjoyAce, n: usize| -> Vec<i32> {
                let b = t.jxs[jxs::MTRP];
                if b == 0 || n == 0 {
                    return Vec::new();
                }
                (0..n).map(|i| t.xss[(b - 1) as usize + i].round() as i32).collect()
            };
            let mo: Vec<i32> = if ours.jxs[jxs::MTRP] == 0 {
                Vec::new()
            } else {
                (0..ntrp_o)
                    .map(|i| ours.xss[(ours.jxs[jxs::MTRP] - 1) as usize + i].round() as i32)
                    .collect()
            };
            let mt = mtrp(&theirs, ntrp_t);
            let miss: Vec<i32> = mt.iter().copied().filter(|m| !mo.contains(m)).collect();
            let extra: Vec<i32> = mo.iter().copied().filter(|m| !mt.contains(m)).collect();
            if miss.is_empty() && extra.is_empty() && !mt.is_empty() {
                println!("  MTRP sets IDENTICAL ({} entries)", mt.len());
            } else {
                if !miss.is_empty() {
                    println!("  in NJOY, not ours ({}): {:?}", miss.len(), &miss[..miss.len().min(12)]);
                }
                if !extra.is_empty() {
                    println!("  in ours, not NJOY ({}): {:?}", extra.len(), &extra[..extra.len().min(12)]);
                }
            }
            // MFTYPE and LAW per entry, in NJOY's order, for the entries both have.
            let describe = |t_jxs: &[i32; 32], xss: &[f64], n: usize, i: usize| -> (i32, i32) {
                let lsigp = t_jxs[jxs::LSIGP];
                let sigp = t_jxs[jxs::SIGP];
                let ldlwp = t_jxs[jxs::LDLWP];
                let dlwp = t_jxs[jxs::DLWP];
                if lsigp == 0 || i >= n {
                    return (0, 0);
                }
                let so = xss[(lsigp - 1) as usize + i] as i32;
                let mft = xss[(sigp - 1 + so - 1) as usize].round() as i32;
                let lo = xss[(ldlwp - 1) as usize + i] as i32;
                let law = xss[(dlwp - 1 + lo - 1) as usize + 1].round() as i32;
                (mft, law)
            };
            // Compare BY MT, not by position: a pure ordering difference would
            // otherwise read as every entry being wrong, which is exactly how
            // the first run of this comparison misreported itself.
            let nshow = ntrp_t.min(ntrp_o).min(12);
            let mut mismatched = 0usize;
            for i in 0..ntrp_t.min(ntrp_o) {
                let Some(j) = mo.iter().position(|m| Some(m) == mt.get(i)) else {
                    mismatched += 1;
                    continue;
                };
                let a = describe(&ours.jxs, &ours.xss, ntrp_o, j);
                let b2 = describe(&theirs.jxs, &theirs.xss, ntrp_t, i);
                if a != b2 {
                    mismatched += 1;
                }
                if i < nshow {
                    let flag = if a == b2 { "ok" } else { "DIFFER" };
                    println!(
                        "    {:<8} MFTYPE ours {:>3} njoy {:>3} | LAW ours {:>2} njoy {:>2}  {}",
                        mt.get(i).copied().unwrap_or(0),
                        a.0, b2.0, a.1, b2.1, flag
                    );
                }
            }
            if ntrp_t.min(ntrp_o) > nshow {
                println!("    … {} more", ntrp_t.min(ntrp_o) - nshow);
            }
            println!(
                "  MFTYPE/LAW agree on {} of {} compared entries",
                ntrp_t.min(ntrp_o) - mismatched,
                ntrp_t.min(ntrp_o)
            );
        }

        // ── Band integrals: the ONLY instrument here that reaches the
        //    broadened region ───────────────────────────────────────────────
        //
        // Both tables claim to represent the same sigma(E) to their own 0.001
        // thinning tolerance. Integrating each over a FIXED energy band, on its
        // own grid, compares the functions rather than the sampling — so unlike
        // the shared-point comparison it works where the grids disagree, which
        // below thnmax is everywhere.
        //
        // Equal-lethargy bands, so resonance structure is not averaged away by
        // a handful of wide bins at low energy.
        println!("\n=== band integrals (grid-independent) ===");
        const BANDS_PER_DECADE: usize = 20;
        let e_min = e_ours[0].max(e_theirs[0]).max(1.0e-11);
        let e_max = e_ours[nes_o - 1].min(e_theirs[nes_t - 1]);
        let decades = (e_max / e_min).log10();
        let nbands = ((decades * BANDS_PER_DECADE as f64).ceil() as usize).max(1);
        let edge = |k: usize| e_min * 10f64.powf(decades * k as f64 / nbands as f64);

        let mut band_rows: Vec<(f64, f64, f64, f64, f64)> = Vec::new(); // e_lo, e_hi, njoy tot/abs/el
        for (col, label) in [(1usize, "total"), (2, "absorption"), (3, "elastic")] {
            let xo = &ours.xss[eo + col * nes_o..eo + (col + 1) * nes_o];
            let xt = &theirs.xss[et + col * nes_t..et + (col + 1) * nes_t];
            let mut worst_lo = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut worst_hi = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut n_lo = 0usize;
            for k in 0..nbands {
                let (a, b) = (edge(k), edge(k + 1));
                let io = njoy_outram_park_fork::acer::integrate_linlin(e_ours, xo, a, b);
                let it = njoy_outram_park_fork::acer::integrate_linlin(e_theirs, xt, a, b);
                if it == 0.0 {
                    continue;
                }
                let d = ((io - it) / it).abs();
                // A band is "broadened" only if it lies wholly below thnmax;
                // a band straddling the limit mixes the two regimes and is
                // attributed to neither.
                if b <= thnmax_mev {
                    n_lo += 1;
                    if col == 1 {
                        band_rows.push((a, b, it, 0.0, 0.0));
                    } else if let Some(r) = band_rows.iter_mut().find(|r| r.0 == a) {
                        if col == 2 {
                            r.3 = it;
                        } else {
                            r.4 = it;
                        }
                    }
                    if d > worst_lo.0 {
                        worst_lo = (d, a, io, it);
                    }
                } else if a >= thnmax_mev && d > worst_hi.0 {
                    worst_hi = (d, a, io, it);
                }
            }
            println!(
                "  {:<11} BROADENED  ({n_lo:>3} bands < thnmax)  worst rel {:.3e} at band from {:.4e} MeV",
                label, worst_lo.0, worst_lo.1
            );
            println!(
                "  {:<11} unbroadened                      worst rel {:.3e} at band from {:.4e} MeV",
                "", worst_hi.0, worst_hi.1
            );
        }
        println!(
            "  ({BANDS_PER_DECADE} bands/decade over [{:.3e}, {:.3e}] MeV; thnmax {:.3e} MeV)",
            e_min, e_max, thnmax_mev
        );

        // ── Optional: dump NJOY's BROADENED band integrals as a test oracle ──
        //
        // Committed so the broadening gate does not need the 135 MB reference
        // table at test time: 167 rows against a file that big is the same
        // trade the ESZ oracle already makes.
        if let Some(out) = flag("--dump-band-oracle") {
            use std::io::Write;
            let mut f = std::io::BufWriter::new(
                std::fs::File::create(&out).unwrap_or_else(|e| panic!("create {out}: {e}")),
            );
            writeln!(
                f,
                "# NJOY2016 2016.79 ac5adf5 -- band integrals of the ESZ columns over"
            )
            .unwrap();
            writeln!(
                f,
                "# equal-lethargy bands ({BANDS_PER_DECADE}/decade) lying wholly BELOW thnmax"
            )
            .unwrap();
            writeln!(
                f,
                "# = {thnmax_mev:.6e} MeV, i.e. the bands that actually test Doppler broadening."
            )
            .unwrap();
            writeln!(f, "# MAT {mat}, {temp_k} K. Integrals in barn*MeV; energies in MeV.").unwrap();
            writeln!(f, "e_lo_mev,e_hi_mev,total,absorption,elastic").unwrap();
            for r in &band_rows {
                writeln!(
                    f,
                    "{:.17e},{:.17e},{:.17e},{:.17e},{:.17e}",
                    r.0, r.1, r.2, r.3, r.4
                )
                .unwrap();
            }
            println!("\nwrote {} band rows to {out}", band_rows.len());
        }

        // ── Optional: dump the shared-grid rows as a committed test oracle ──
        //
        // The full NJOY table is 316 MB and cannot live in the repository. The
        // rows compared above can: they are the interpolation-free subset, so a
        // test built on them measures exactly what this example measures, with
        // no 316 MB dependency and no decompression. Same shape as
        // `tests/acer_acesix_vs_njoy2016_ace.rs`, which keeps a slice of its
        // own comparison rather than the whole ACE file.
        if let Some(out) = std::env::args().skip_while(|a| a != "--dump-oracle").nth(1) {
            use std::io::Write;
            let mut f = std::io::BufWriter::new(
                std::fs::File::create(&out).unwrap_or_else(|e| panic!("create {out}: {e}")),
            );
            writeln!(f, "# NJOY2016 0 K U-235 ESZ at grid energies this port also chose.")
                .unwrap();
            writeln!(f, "# Oracle for tests/acer_ce_esz_vs_njoy2016.rs. Columns are MeV and barns.\n# 17 significant digits: f64 needs that to round-trip exactly, and the test\n# matches grid energies by EXACT equality.")
                .unwrap();
            writeln!(f, "energy_mev,total_b,absorption_b,elastic_b").unwrap();
            let (mut i, mut j, mut n) = (0usize, 0usize, 0usize);
            while i < nes_o && j < nes_t {
                let (a, b) = (e_ours[i], e_theirs[j]);
                if a == b {
                    writeln!(
                        f,
                        "{:.17e},{:.17e},{:.17e},{:.17e}",
                        b,
                        theirs.xss[et + nes_t + j],
                        theirs.xss[et + 2 * nes_t + j],
                        theirs.xss[et + 3 * nes_t + j]
                    )
                    .unwrap();
                    n += 1;
                    i += 1;
                    j += 1;
                } else if a < b {
                    i += 1;
                } else {
                    j += 1;
                }
            }
            println!("\n  wrote {n} oracle rows to {out}");
        }

        // ── Reaction inventory ───────────────────────────────────────────────
        //
        // A cross section can agree everywhere it exists and still leave the
        // table incomplete, so the MT list is compared as a SET rather than
        // only through NTR.
        println!("\n=== reaction inventory (MTR block) ===");
        let mts = |t_nxs: &[i32; 16], t_jxs: &[i32; 32], xss: &[f64]| -> Vec<i32> {
            let ntr = t_nxs[nxs::NTR] as usize;
            let m = t_jxs[jxs::MTR];
            if m == 0 || ntr == 0 {
                return Vec::new();
            }
            let base = (m - 1) as usize;
            xss[base..base + ntr].iter().map(|v| *v as i32).collect()
        };
        let mo = mts(&ours.nxs, &ours.jxs, &ours.xss);
        let mt = mts(&theirs.nxs, &theirs.jxs, &theirs.xss);
        let missing: Vec<i32> = mt.iter().copied().filter(|m| !mo.contains(m)).collect();
        let extra: Vec<i32> = mo.iter().copied().filter(|m| !mt.contains(m)).collect();
        println!("  ours {} reactions, njoy {}", mo.len(), mt.len());
        if missing.is_empty() {
            println!("  every NJOY MT is present in ours");
        } else {
            println!("  MTs in NJOY and NOT in ours ({}): {:?}", missing.len(), missing);
        }
        if !extra.is_empty() {
            println!("  MTs in ours and NOT in NJOY ({}): {:?}", extra.len(), extra);
        }

        println!(
            "\nNJOY2016 is the specification. A difference here is this port's defect until\n\
             a reason is demonstrated -- do not widen a tolerance to absorb one."
        );
    }
}
