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
//! | fission ν̄ (NU) | `JXS(2)` ours `0`, NJOY `1167576` | **no fission source** — such an ACE cannot drive an eigenvalue |
//! | ~~37 reactions~~ **CLOSED 2026-09-20** | ours **84**, NJOY 84, sets identical | MT=649 and MT=800–835 now stored; excluded from the ESZ sums under the lumped MT=103/107, so the ESZ numbers above are unchanged |
//! | photon production | `NXS(6)` ours `0`, NJOY `583` | no photon transport |
//!
//! No MT is in ours and absent from NJOY — a strict subset, not a divergence.
//!
//! Full record: `outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/ace_pipeline.md`.
//!
//! `JXS(2)` — the fission ν̄ (NU) block — is **deferred and written as 0** by
//! this port (`acer/mod.rs`). The example reports that as an explicit ABSENT
//! row rather than letting a zero locator read as agreement, because a missing
//! block and a matching block are indistinguishable in a naive diff, and an ACE
//! file with no NU block cannot drive a fission eigenvalue at all.

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

    pub fn run() {
        let njoy_path = std::env::args().nth(1).unwrap_or_else(|| {
            eprintln!("usage: ace_vs_njoy2016 <njoy_type1_ace_file>");
            std::process::exit(2);
        });
        let theirs = parse_njoy_ace(&njoy_path);

        // ── Ours: the identical pipeline examples/write_ace.rs uses ──────────
        const MAT: i32 = 9228;
        let path = njoy_outram_park_fork::reference_data::reference_endf_dir()
            .join("n-092_U_235-ENDF8.0.endf");
        let tape = Tape::read(File::open(&path).expect("open ENDF")).expect("parse ENDF");
        let cfg = ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 };
        let result = reconr(&tape, &cfg).expect("RECONR");
        let angular = tape
            .section(MAT, 4, 2)
            .map(|s| parse_elastic_angular(s).expect("parse MF=4"));
        let partials: Vec<(i32, f64)> = result
            .sections
            .iter()
            .map(|s| (i32::from(s.mt), s.qi))
            .collect();
        let emissions = build_emissions(&tape, MAT, result.material.awr, &partials);
        let nu = NuBar::from_endf(&tape, MAT).expect("MF=1").unwrap_or_default();
        let chi = FissionSpectrum::from_endf_mf5(&tape, MAT).expect("MF=5").unwrap_or_default();
        let emission = build_emission_spectra(&tape, MAT);
        let photons = PhotonProduction::from_endf(&tape, MAT, &result);
        let kerma = Kerma::from_reconr(&result, &nu, &chi, &emission)
            .with_energy_balance(&photons, &result);
        let ours =
            AceTable::from_reconr_full(&result, 0.0, 0, angular.as_ref(), &emissions, Some(&kerma));

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
                "  !! TEMPERATURE MISMATCH -- the oracle is not a matched 0 K RECONR-only run.\n     \
                 Every block difference below is then confounded with Doppler broadening and\n     \
                 cannot be attributed to ACER. Regenerate with make_ace_0k.sh."
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
        println!("\n=== ESZ at shared grid points (no interpolation) ===");
        let mut shared = 0usize;
        let mut worst = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); 3];
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
