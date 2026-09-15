//! # U-238 capture Doppler-broadening — code-to-code verification
//!
//! Reference document: this directory's `README.md`.
//!
//! **What this verifies.** The Rust NJOY port reconstructs the U-238 radiative
//! capture cross section (MT=102) *directly from the ENDF/B-VIII.0 tape*
//! (`reference-data/endf/n-092_U_238.endf`) with RECONR (Reich-Moore, LRF=3), then
//! Doppler-broadens it to 900 K and 1200 K with BROADR (SIGMA1 free-gas kernel).
//! The result is compared **point-for-point** against OpenMC's own ENDF/B-VIII.0
//! pointwise capture cross section, pre-extracted into the committed reference
//! CSVs `reference/openmc_capture_{900,1200}K.csv`. It is therefore a
//! full-pipeline (RECONR + BROADR) code-to-code check of the Rust port against
//! canonical NJOY, read through OpenMC's data.
//!
//! **Why CSVs, not the .h5.** OpenMC's `U238.h5` is ≈115 MB — too large for
//! GitHub. The one-time `examples/extract_u238_doppler_ref.rs` samples the h5's
//! pointwise capture onto a **100,000-point log-spaced grid** (10⁻⁵ eV →
//! 20 MeV) and writes the ~23 KB reference CSVs. Those are committed; the h5 can
//! then be deleted and this test still runs.
//!
//! **Energy regions.** The Rust RECONR reconstructs only the resolved resonance
//! region (RRR); U-238's RRR ends at **20 keV**, which holds every strong,
//! Doppler-sensitive capture resonance (6.67, 20.9, 36.7, 66, 81, 90, 103,
//! 117 eV, …). Above 20 keV the port returns the MF=3 infinite-dilution average
//! (the unresolved UNRESR/PURR modules are the next to port —
//! `docs/porting-plan.md` §8).
//!
//! **Method.** For each reference energy `E`, the OpenMC value (from the CSV) is
//! compared to the Rust port's broadened capture at the same `E`
//! (`NuclearDataLibrary::capture_xs`, lin-lin interpolation of the port's
//! broadened grid). Plottable output: `results/compare_capture_{900,1200}K.csv`
//! (`energy_eV, sigma_openmc_b, sigma_rust_b, rel_diff, region`).
//!
//! **What the test gates.** All capture values finite and non-negative; the
//! above-RRR (MF=3) band matches OpenMC to < 0.1 % L1; the RRR
//! magnitude-weighted L1 `Σ|σ_rust − σ_omc| / Σ|σ_omc|` < 0.1 %; the worst
//! single RRR point < 3 %; and the 6.67 eV / 20.87 eV capture peaks within
//! 0.5 % — at both 900 K and 1200 K.
//!
//! ## Results (re-baselined 2026-09-10 after `WAVE_K`; RECONR tol 0.1%, SIGMA1)
//!
//! - Fast/above-RRR (MF=3 infinite dilution): **L1 = 0.0000** (pass-through).
//! - RRR: **L1 = 0.0002** at 900 K and 1200 K; worst point 0.74 % (80.06 eV,
//!   900 K) / 0.72 % (516.4 eV, 1200 K); 6.67 eV peak 4532.5 vs 4530.8 b
//!   (900 K), 3997.1 vs 3995.9 b (1200 K); 20.87 eV peak 4218.6 vs 4214.6 b
//!   (900 K), 3705.5 vs 3702.9 b (1200 K).
//! - History: on 2026-07-06 the RRR L1 was 0.30/0.32 — a SIGMA1 wing pedestal
//!   (~200 b several eV past each resonance where OpenMC decays to ~1 b),
//!   reported but not gated. That bug was fixed in the BROADR pass and the
//!   2026-09-10 `WAVE_K` correction (`cwaven` had been rounded up in the 4th
//!   figure; every resonance σ moved +0.08 %) took the residual from ~0.0007 to
//!   0.0002, at which point the L1 became a hard gate (`op-cjw.10`).
//!
//! See `README.md` for the full table and interpretation; numbers are printed by
//! `capture_doppler_matches_openmc` (`-- --nocapture`).
//!
//! Run under the memory cap: `scripts/test.sh capture -- --nocapture`.

use njoy_outram_park_fork::interface::NuclearDataLibrary;
use std::io::Write;
use std::path::PathBuf;
use uom::si::area::barn;
use uom::si::energy::electronvolt;
use uom::si::f64::{Energy, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

/// ENDF material number for U-238 (ENDF/B-VIII.0).
const U238_MAT: i32 = 9237;
/// RECONR reconstruction tolerance (NJOY default 0.1%).
const RECONR_TOL: f64 = 0.001;
/// Upper energy [eV] of U-238's resolved resonance region (LRF=3). The hard
/// agreement gate applies at and below this; above it is the unresolved region
/// the port does not yet reconstruct.
const RRR_MAX_EV: f64 = 2.0e4;
/// Below this cross section [b] a point sits in an inter-resonance valley where
/// relative error is dominated by division by a near-zero σ; excluded from the
/// per-point max/mean-relative statistics (still in the CSV and the L1).
const SIGNIFICANT_B: f64 = 1.0;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn endf_path() -> PathBuf {
    njoy_outram_park_fork::reference_data::reference_endf_dir().join("n-092_U_238.endf")
}
fn reference_csv(temp_k: u32) -> PathBuf {
    manifest().join(format!(
        "tests/u238_doppler_verification/reference/openmc_capture_{temp_k}K.csv"
    ))
}
fn results_dir() -> PathBuf {
    manifest().join("tests/u238_doppler_verification/results")
}

/// Read a committed OpenMC reference CSV: `(energy_eV, sigma_b)` pairs.
fn read_reference(temp_k: u32) -> Vec<(f64, f64)> {
    let path = reference_csv(temp_k);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "read reference {} : {e}\n(run `cargo run --release --example extract_u238_doppler_ref` while U238.h5 is present)",
            path.display()
        )
    });
    text.lines()
        .skip(1) // header
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split(',');
            let e: f64 = it.next().unwrap().trim().parse().expect("energy parse");
            let s: f64 = it.next().unwrap().trim().parse().expect("sigma parse");
            (e, s)
        })
        .collect()
}

/// Reconstruct U-238 from ENDF and Doppler-broaden to `temp_k` [K].
fn rust_broadened(temp_k: f64) -> NuclearDataLibrary {
    NuclearDataLibrary::from_file(endf_path(), U238_MAT)
        .expect("load U-238 ENDF")
        .reconstruct(RECONR_TOL)
        .expect("RECONR U-238")
        .broaden(ThermodynamicTemperature::new::<kelvin>(temp_k))
        .expect("BROADR U-238")
}

// ── Test 1 — pipeline sanity (0 K peaks; Doppler lowers peak, raises wing) ────

/// Sanity check: reconstructed 0 K capture has thousands-of-barns resonance
/// peaks and the correct thermal value; Doppler broadening at 900 K lowers the
/// 6.67 eV peak while raising the wing (area broadly conserved).
#[test]
fn capture_pipeline_is_physical() {
    let ev = |e: f64| Energy::new::<electronvolt>(e);
    let lib0 = NuclearDataLibrary::from_file(endf_path(), U238_MAT)
        .expect("load")
        .reconstruct(RECONR_TOL)
        .expect("RECONR");

    let thermal = lib0.capture_xs(ev(0.0253)).get::<barn>();
    let peak0 = lib0.capture_xs(ev(6.673)).get::<barn>();
    let wing0 = lib0.capture_xs(ev(6.5)).get::<barn>();
    assert!(
        (2.0..3.5).contains(&thermal),
        "thermal capture {thermal} b (expect ~2.7)"
    );
    assert!(
        peak0 > 5000.0,
        "6.67 eV 0 K peak {peak0} b (expect thousands)"
    );

    let lib900 = rust_broadened(900.0);
    let peak900 = lib900.capture_xs(ev(6.673)).get::<barn>();
    let wing900 = lib900.capture_xs(ev(6.5)).get::<barn>();
    assert!(
        peak900 < peak0,
        "Doppler must lower the peak: {peak900} !< {peak0}"
    );
    assert!(
        wing900 > wing0,
        "Doppler must raise the wing: {wing900} !> {wing0}"
    );
    eprintln!("0 K   : peak {peak0:.0} b, wing {wing0:.1} b, thermal {thermal:.3} b");
    eprintln!("900 K : peak {peak900:.0} b, wing {wing900:.1} b");
}

// ── Test 2 — point-for-point vs OpenMC, write plottable CSVs ──────────────────

struct Metrics {
    n_rrr: usize,
    l1_rrr: f64, // Σ|Δ|/Σ|omc| over RRR (E ≤ 20 keV)
    mean_rel_rrr: f64,
    max_rel_rrr: f64,
    max_rel_e: f64,
    l1_above: f64, // same, above the RRR
    n_above: usize,
    nonfinite: usize, // Rust points that were NaN/inf or negative
}

/// Compare the Rust port's broadened capture against the OpenMC reference at
/// `temp_k`, write the plottable CSV, and return the agreement metrics.
fn compare_temperature(temp_k: u32) -> Metrics {
    let lib = rust_broadened(temp_k as f64);
    let omc = read_reference(temp_k);

    std::fs::create_dir_all(results_dir()).expect("create results dir");
    let csv = results_dir().join(format!("compare_capture_{temp_k}K.csv"));
    let mut w = std::io::BufWriter::new(std::fs::File::create(&csv).expect("create csv"));
    writeln!(w, "energy_eV,sigma_openmc_b,sigma_rust_b,rel_diff,region").unwrap();

    let (mut abs_rrr, mut omc_rrr, mut abs_abv, mut omc_abv) = (0.0, 0.0, 0.0, 0.0);
    let (mut sum_rel, mut max_rel, mut max_rel_e) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut n_rrr, mut n_abv, mut n_sig, mut nonfinite) = (0usize, 0usize, 0usize, 0usize);

    for (e, s_omc) in omc {
        let s_rust = lib.capture_xs(Energy::new::<electronvolt>(e)).get::<barn>();
        if !s_rust.is_finite() || s_rust < 0.0 {
            nonfinite += 1;
        }
        let d = (s_rust - s_omc).abs();
        let rel = d / s_omc.max(1e-30);
        let region = if e <= RRR_MAX_EV { "RRR" } else { "above_RRR" };
        writeln!(w, "{e:.6e},{s_omc:.6e},{s_rust:.6e},{rel:.6e},{region}").unwrap();

        if e <= RRR_MAX_EV {
            abs_rrr += d;
            omc_rrr += s_omc.abs();
            n_rrr += 1;
            if s_omc >= SIGNIFICANT_B {
                sum_rel += rel;
                if rel > max_rel {
                    max_rel = rel;
                    max_rel_e = e;
                }
                n_sig += 1;
            }
        } else {
            abs_abv += d;
            omc_abv += s_omc.abs();
            n_abv += 1;
        }
    }
    w.flush().unwrap();

    Metrics {
        n_rrr,
        l1_rrr: abs_rrr / omc_rrr.max(1e-30),
        mean_rel_rrr: if n_sig > 0 {
            sum_rel / n_sig as f64
        } else {
            0.0
        },
        max_rel_rrr: max_rel,
        max_rel_e,
        l1_above: abs_abv / omc_abv.max(1e-30),
        n_above: n_abv,
        nonfinite,
    }
}

/// Point-for-point verification of the Rust RECONR+BROADR capture cross section
/// against OpenMC's ENDF/B-VIII.0 pointwise reference at 900 K and 1200 K.
#[test]
fn capture_doppler_matches_openmc() {
    let ev = |e: f64| Energy::new::<electronvolt>(e);
    println!("── U-238 (n,γ) Doppler code-to-code: Rust RECONR+BROADR vs OpenMC ENDF/B-8.0 ──");
    println!("RRR gate: E ≤ {RRR_MAX_EV:.0} eV (resolved region). 100k-pt log grid.");
    println!("temp_K,n_RRR,L1_RRR,mean_rel_RRR,max_rel_RRR,max_rel_at_eV,n_above,L1_above");

    for temp_k in [900u32, 1200u32] {
        let m = compare_temperature(temp_k);
        println!(
            "{temp_k},{},{:.4},{:.4},{:.4},{:.3},{},{:.4}",
            m.n_rrr, m.l1_rrr, m.mean_rel_rrr, m.max_rel_rrr, m.max_rel_e, m.n_above, m.l1_above
        );

        // Peak spot-checks at the two strongest capture resonances.
        let lib = rust_broadened(temp_k as f64);
        let omc = read_reference(temp_k);
        for e0 in [6.673_f64, 20.87] {
            let s_rust = lib.capture_xs(ev(e0)).get::<barn>();
            let (e_omc, s_omc) = omc
                .iter()
                .copied()
                .min_by(|a, b| (a.0 - e0).abs().partial_cmp(&(b.0 - e0).abs()).unwrap())
                .unwrap();
            println!(
                "  ~{e0} eV @ {temp_k} K: rust {s_rust:.1} b, openmc {s_omc:.1} b (nearest grid {e_omc:.4} eV)"
            );
        }

        // ── Gates (what the port must satisfy) ──────────────────────────────
        assert!(
            m.n_rrr > 100,
            "too few RRR points ({}) — reference grid wrong?",
            m.n_rrr
        );
        assert_eq!(
            m.nonfinite, 0,
            "{temp_k} K: {} non-finite/negative capture points",
            m.nonfinite
        );
        // Above the resolved region the port returns the MF=3 infinite-dilution
        // average — the same numbers OpenMC's tape carries, so this is a
        // pass-through check. Measured 0.0000 (2026-09-10); gate 0.1 %.
        assert!(
            m.l1_above < 1e-3,
            "{temp_k} K: fast-region L1 {:.4} vs OpenMC exceeds 0.1%",
            m.l1_above
        );

        // ── RRR gates (op-cjw.10, re-baselined 2026-09-10 after WAVE_K) ─────
        // The wing pedestal (RRR L1 ≈ 0.30 in 2026-07) is gone: RRR L1 is
        // 0.0002 at both temperatures, the worst single point 0.74 % (900 K,
        // 80.06 eV) / 0.72 % (1200 K, 516.4 eV), and the 6.67 / 20.87 eV
        // peaks agree to 0.04–0.1 %. Gates sit 3–5× above those measurements:
        // a regression of the wing bug (or of WAVE_K, which moves every
        // resonance σ by 0.08 %) trips the L1 gate long before the old 5 %
        // fast-region gate would have noticed anything.
        assert!(
            m.l1_rrr < 1e-3,
            "{temp_k} K: RRR L1 {:.5} vs OpenMC exceeds 0.1% (measured 0.0002 on 2026-09-10)",
            m.l1_rrr
        );
        assert!(
            m.max_rel_rrr < 0.03,
            "{temp_k} K: worst RRR point {:.4} at {:.3} eV exceeds 3% (measured 0.74%)",
            m.max_rel_rrr,
            m.max_rel_e
        );
        for e0 in [6.673_f64, 20.87] {
            let s_rust = lib.capture_xs(ev(e0)).get::<barn>();
            let (_, s_omc) = omc
                .iter()
                .copied()
                .min_by(|a, b| (a.0 - e0).abs().partial_cmp(&(b.0 - e0).abs()).unwrap())
                .unwrap();
            assert!(
                ((s_rust - s_omc) / s_omc).abs() < 5e-3,
                "{temp_k} K: {e0} eV peak {s_rust:.1} b vs OpenMC {s_omc:.1} b differs by more than 0.5%"
            );
        }
    }

    println!("plottable CSVs -> {}", results_dir().display());
}
