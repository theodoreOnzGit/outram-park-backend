//! Compare this crate's reconstructed U-238 cross sections against **NJOY2016's
//! own PENDF** for the same tape, temperature and tolerance.
//!
//! # Why
//!
//! The FHR ring-RPT pebble sits ~+1700 pcm above the OpenMC reference, and the
//! offset is shared by both the explicit-TRISO and ring-RPT pebbles (they differ
//! by 194 pcm, inside noise), so it is a property of the cross sections rather
//! than of the pebble model (`op-mzvp.2.12`).
//!
//! The obvious oracle — the NNDC HDF5 library the OpenMC deck used — is
//! unreachable from this environment (403). NJOY2016 is not: it is built
//! in-session, and this crate is a port of it, so "does our reconstruction match
//! NJOY's on the same input?" is both answerable and exactly the right question.
//!
//! # Generating the oracle
//!
//! ```text
//! cd /home/user/u238oracle && cp .../n-092_U_238.endf tape20
//! njoy <<'EOF'
//! reconr
//!  20 21/
//!  'pendf for u238, err 0.001'/
//!  9237 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  9237 1/
//!  0.001/
//!  600./
//!  0/
//! stop
//! EOF
//! ```
//!
//! `tape22` is then the 600 K PENDF. Point `U238_PENDF` at it:
//!
//! ```text
//! U238_PENDF=/home/user/u238oracle/tape22 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example u238_vs_njoy_pendf
//! ```
//!
//! A disagreement here is a port defect. Agreement means our data matches the
//! code we are a port of, and the OpenMC offset lives somewhere else — either in
//! transport, or in the difference between RECONR-on-device and the NNDC
//! processing chain that produced OpenMC's library.
//!
//! # Results, 2026-09-11 — every nuclide in the FHR pebble
//!
//! NJOY2016 was rebuilt in-session (`ac5adf5`, cmake + gfortran, ~5 min) and the
//! same RECONR + BROADR deck run for each nuclide at 600 K, tolerance 1e-3.
//! Worst relative difference over the 19 probe energies:
//!
//! | nuclide | MAT | total | elastic |
//! |---|---|---|---|
//! | U-238 | 9237 | ±0.04 % | ±0.04 % |
//! | U-235 | 9228 | ±0.06 % | ±0.06 % |
//! | C-12 | 625 | +0.00 % | +0.01 % |
//! | C-13 | 628 | +0.00 % | +0.01 % |
//! | O-16 | 825 | +0.00 % | +0.01 % |
//! | F-19 | 925 | −0.02 % | +0.02 % |
//! | Be-9 | 425 | +0.02 % | +0.01 % |
//! | Li-7 | 328 | +0.02 % | −0.01 % |
//! | Li-6 | 325 | +0.17 % | −0.01 % |
//! | Si-28 | 1425 | −0.05 % | −0.05 % |
//!
//! Thermal capture at 0.0253 eV likewise agrees to ≤0.03 % for C-12, O-16, F-19,
//! Be-9 and Li-7.
//!
//! **Read the `MT=102` column with care: it compares two different quantities
//! for a light nuclide, and the mismatch is not a defect.** This crate's
//! `absorption` is the OpenMC MT=27 quantity — fission plus *all* of MT=101, the
//! disappearance reactions — so `absorption − fission` includes `(n,α)`, `(n,p)`,
//! `(n,t)` and the rest, while NJOY's MT=102 is radiative capture alone. For
//! **Li-6** that is the whole story: `(n,t)α` (MT=105) is 938 b at 0.0253 eV
//! against MT=102's 0.0385 b, a factor of 24 000. Counting only MT=102 there is
//! exactly the defect GH #169 fixed (Li-6 absorption 0.04 → 938 b). The same
//! applies above threshold for C-12, O-16, Be-9 and Si-28, where `(n,α)`/`(n,p)`
//! open and MT=102 collapses.

fn main() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::groupr::panel::PointwiseXs;
    use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::nuclide::Nuclide;

    // Defaults are U-238; override to check another nuclide against its own
    // NJOY PENDF (U-235 in particular -- it is the fissile driver, so an error
    // there moves k directly, and checking only U-238 would have missed it).
    let mat: i32 = std::env::var("NJOY_MAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9237);
    let tape_name = std::env::var("NJOY_TAPE").unwrap_or_else(|_| "n-092_U_238.endf".into());
    let nuc_name = std::env::var("NJOY_NUCLIDE").unwrap_or_else(|_| "U238".into());
    const TEMP: f64 = 600.0;

    let tape_for_gate = reference_endf(&tape_name).expect("ENDF tape");
    if nuc_name == "U238" {
        let ours_for_gate = Nuclide::from_endf_file(&tape_for_gate, &nuc_name, TEMP, 1.0e-3)
            .expect("reconstruction");
        golden_gate(&ours_for_gate, TEMP);
    } else {
        println!(
            "(No golden gate for {nuc_name}: the committed oracle tables are U-238's. \
             Set U238_PENDF to compare this nuclide against its own PENDF.)"
        );
    }

    let pendf_path = match std::env::var("U238_PENDF") {
        Ok(p) => p,
        Err(_) => {
            eprintln!(
                "\nThe live-tape comparison needs U238_PENDF pointing at an NJOY 600 K \n\
                 PENDF (deck in the module docs). The golden gate above already ran, so \n\
                 this program is still a V&V case without it — it just cannot re-measure \n\
                 the oracle."
            );
            return;
        }
    };

    eprintln!("reading NJOY PENDF {pendf_path} ...");
    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("PENDF parses");

    let tape = reference_endf(&tape_name).expect("ENDF tape");
    eprintln!("reconstructing {nuc_name} (MAT {mat}) with this crate @ {TEMP} K, tol 1e-3 ...");
    let ours = Nuclide::from_endf_file(&tape, &nuc_name, TEMP, 1.0e-3).expect("reconstruction");
    eprintln!("done.\n");

    // (MT, label, how to pull it out of our MicroXS)
    let reactions: [(i32, &str); 4] = [
        (1, "total"),
        (2, "elastic"),
        (18, "fission"),
        (102, "capture"),
    ];

    // Energies spanning thermal, the big low-lying resonances, the
    // resolved/unresolved seam, the URR band and fast.
    const PROBE_EV: &[f64] = &[
        0.0253, 1.0, 6.674, 20.87, 36.68, 66.03, 102.6, 1.0e3, 1.0e4, 1.9e4, 2.0e4, 3.0e4, 5.0e4,
        1.0e5, 1.5e5, 5.0e5, 1.0e6, 2.0e6, 1.4e7,
    ];

    for (mt, label) in reactions {
        let njoy = match read_pendf_cross_section(&pendf, mat, mt) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("== MT={mt} ({label}): NJOY PENDF has no such section ({e:?})");
                continue;
            }
        };
        let PointwiseXs::LinLin(pairs) = &njoy.xs else {
            eprintln!("== MT={mt} ({label}): not a tabulated section, skipping");
            continue;
        };

        println!("\n== MT={mt}  {label}  (NJOY grid: {} points)", pairs.len());
        println!(
            "{:>11}  {:>14}  {:>14}  {:>9}",
            "E [eV]", "NJOY [b]", "ours [b]", "rel diff"
        );
        let mut worst = (0.0_f64, 0.0_f64);
        for &e in PROBE_EV {
            let n = interp_linlin(pairs, e);
            let x = ours.xs_at_energy(e, TEMP);
            let o = match mt {
                1 => x.total,
                2 => x.elastic,
                18 => x.fission,
                102 => x.absorption - x.fission,
                _ => unreachable!(),
            };
            let rel = if n.abs() > 1e-30 { (o - n) / n } else { 0.0 };
            if rel.abs() > worst.0.abs() {
                worst = (rel, e);
            }
            println!("{e:>11.4e}  {n:>14.6e}  {o:>14.6e}  {:>+8.2}%", 100.0 * rel);
        }
        println!("   worst: {:+.2}% at {:.4e} eV", 100.0 * worst.0, worst.1);
    }

    // nu-bar sanity check. This CANNOT be validated against the PENDF: NJOY
    // copies MF=1/452 through unchanged and this crate reads the same section,
    // so the two agree by construction and the comparison would be circular.
    // Check it against published physics instead -- nu-bar multiplies k
    // directly, so an error here would be invisible in every sigma comparison
    // above and still move the eigenvalue.
    println!("\n== nu-bar (from MF=1/452; vs published values, NOT vs the PENDF)");
    println!("{:>11}  {:>12}", "E [eV]", "nu-bar");
    for &e in &[0.0253, 1.0e3, 1.0e6, 2.0e6, 1.4e7] {
        let x = ours.xs_at_energy(e, TEMP);
        let nu = if x.fission > 0.0 {
            x.nu_fission / x.fission
        } else {
            0.0
        };
        println!("{e:>11.4e}  {nu:>12.5}");
    }
    println!(
        "   reference points: U-235 thermal nu-bar is 2.43-2.44, rising to ~2.6\n\
         at 1 MeV and ~3.1 at 5 MeV; U-238 is ~2.49 at its fission threshold,\n\
         rising to ~3.0 at 6 MeV. A value outside those bands is a real defect."
    );
}

/// Linear interpolation on an ascending `(E, sigma)` table; zero outside it,
/// matching `gety1`.
fn interp_linlin(pairs: &[(f64, f64)], e: f64) -> f64 {
    if pairs.is_empty() || e < pairs[0].0 || e > pairs[pairs.len() - 1].0 {
        return 0.0;
    }
    let i = match pairs.binary_search_by(|p| p.0.partial_cmp(&e).unwrap()) {
        Ok(i) => return pairs[i].1,
        Err(i) => i,
    };
    let (e0, s0) = pairs[i - 1];
    let (e1, s1) = pairs[i];
    if e1 == e0 {
        return s1;
    }
    s0 + (s1 - s0) * (e - e0) / (e1 - e0)
}

/// V&V gate against the committed NJOY2016 oracle — runs with no PENDF on disk.
///
/// # Methodology
///
/// `Nuclide::xs_at_energy` against NJOY2016 2016.79's own PENDF for U-238
/// (MAT 9237) at 600 K, at nineteen probe energies spanning thermal, the four
/// big low-lying capture resonances, the resolved tail, the resolved/unresolved
/// seam, the unresolved band, and fast. The oracle values and their provenance
/// are in [`outram_mc_libs::vv::njoy_golden::u238_pendf`]; the NJOY deck that
/// produces the tape is in this file's module docs.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0 @ 600 K)
///
/// | MT | channel | worst | where |
/// |---|---|---|---|
/// | 1 | total | +0.04 % | 20.87 eV |
/// | 2 | elastic | −0.03 % | 6.674 eV |
/// | 102 | capture | −0.17 % | 19 keV |
/// | 18 | fission | see below | |
///
/// # MT=18 needs a significance floor, and the floor needs its own assertion
///
/// U-238 fission has a ~1 MeV threshold, so most of the probe list is
/// sub-threshold and the tabulated values there are the evaluation's tiny tail.
/// At 1 keV NJOY gives 1.354e-7 b and this crate 1.453e-7 b — **+7.31 %**, which
/// in absolute terms is 1e-8 b and measures reconstruction round-off, not
/// physics.
///
/// So the fission comparison carries a **1e-6 b significance floor**. That alone
/// would be a hole: a data change that silently zeroed the whole column would
/// drop every point below the floor and report "everything passed". The gate
/// therefore also asserts **how many** points the floor excluded — exactly one —
/// so the exclusion cannot quietly grow.
///
/// # What this pins, and what it cannot
///
/// Point values pin *peak heights*. They cannot see the **area** under a
/// resonance, which is what drives resonance escape — a grid too coarse between
/// the nodes loses area without moving any node value. That gap is closed by
/// `examples/u238_resonance_integral.rs` (+0.001 % against NJOY over 0.5 eV to
/// 100 keV, on this crate's own grid).
fn golden_gate(ours: &outram_mc_libs::material::nuclide::Nuclide, temp: f64) {
    use outram_mc_libs::vv::assert_table_relative;
    use outram_mc_libs::vv::njoy_golden::u238_pendf as njoy;

    println!("=== V&V gate: U-238 point cross sections vs committed NJOY2016 PENDF ===");

    let rows = |table: &[(f64, f64)],
                pick: fn(&outram_mc_libs::material::nuclide::MicroXS) -> f64|
     -> Vec<(f64, f64, f64)> {
        table
            .iter()
            .map(|&(e, n)| (e, pick(&ours.xs_at_energy(e, temp)), n))
            .collect()
    };

    assert_table_relative(
        "U-238 MT=1 total vs NJOY PENDF",
        &rows(njoy::TOTAL, |x| x.total),
        0.002,
        1.0e-6,
    );
    assert_table_relative(
        "U-238 MT=2 elastic vs NJOY PENDF",
        &rows(njoy::ELASTIC, |x| x.elastic),
        0.002,
        1.0e-6,
    );
    assert_table_relative(
        "U-238 MT=102 capture vs NJOY PENDF",
        &rows(njoy::CAPTURE, |x| x.absorption - x.fission),
        0.003,
        1.0e-6,
    );

    // MT=18: significance floor, plus an assertion on the floor itself.
    const FISSION_SIGNIFICANCE_B: f64 = 1.0e-6;
    let fission: Vec<(f64, f64, f64)> = rows(njoy::FISSION, |x| x.fission);
    let n_excluded = fission
        .iter()
        .filter(|&&(_, _, n)| n.abs() < FISSION_SIGNIFICANCE_B)
        .count();
    assert_eq!(
        n_excluded,
        1,
        "the {FISSION_SIGNIFICANCE_B:.0e} b significance floor excluded {n_excluded} \
         of {} fission points; exactly one (1 keV, 1.354e-7 b) was excluded on \
         2026-09-11. If this grew, U-238's fission channel has collapsed toward \
         zero somewhere it should not have, and the floor would hide it by \
         reporting a clean pass on the remaining points.",
        fission.len(),
    );
    assert_table_relative(
        "U-238 MT=18 fission vs NJOY PENDF (above the significance floor)",
        &fission,
        0.03,
        FISSION_SIGNIFICANCE_B,
    );
}
