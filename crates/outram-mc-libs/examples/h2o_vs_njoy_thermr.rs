//! Compare this crate's **H in H₂O** S(α,β) cross section against **NJOY2016's
//! own THERMR output** for the same tape and temperature.
//!
//! # Why this exists
//!
//! Graphite's thermal law was checked against THERMR to ±0.05 %
//! (`graphite_vs_njoy_thermr.rs`). **`c_H_in_H2O` never was**, and it turns out
//! to sit under both of the benchmarks this crate disagrees with, while the one
//! thermal benchmark it reproduces (HEU-SOL-THERM-009) is *homogeneous* — where
//! the spatial thermal flux distribution the water law controls does not matter.
//!
//! The reason for looking here is a measurement, not a hunch. ICSBEP
//! LEU-COMP-THERM-008 ships several **independently critical** cases that share
//! one lattice and differ in how the poison is supplied. Running two of them:
//!
//! | case | soluble B-10 | poison rods | `Δk` |
//! |---|---|---|---|
//! | 1 | 1511 ppm | none | **+2950 ± 61 pcm** |
//! | 8 | 794 ppm | 144 pyrex | **+1713 ± 60 pcm** |
//!
//! Same lattice, same pitch, same fuel — so resonance escape `p` is the same in
//! both, and an error in U-238 resonance absorption would give the **same** `Δk`.
//! It does not. Expressed as the soluble boron's share of total absorption
//! (27.2 % and 15.7 %), both cases imply the same deficit: **the soluble boron
//! absorbs ~10.9 % less than it should**, in a code whose B-10 cross section is
//! 3845.9 b at 0.0253 eV against the 3835 b standard and exactly 1/v.
//!
//! B-10 is right, so what is wrong is the **thermal flux in the water it is
//! dissolved in** — and the water's thermal flux is set by this law.
//!
//! # Generating the oracle
//!
//! `tape20` is the H-1 evaluation, `tape30` the `tsl-HinH2O` tape. The THERMR
//! card is `matde matdp nbin ntemp iinc icoh iform natom mtref iprint`; `natom`
//! is **2** for H in H₂O (two principal scatterers per molecule) against 1 for
//! carbon in graphite, and `icoh = 0` because water has no coherent elastic
//! channel. Taken from NJOY2016's own test 68, which processes this very tape.
//!
//! ```text
//! cd /home/user/h2ooracle
//! cp .../n-001_H_001-ENDF8.0-Beta6.endf tape20
//! cp .../tsl-HinH2O.endf tape30
//! njoy <<'EOF'
//! reconr
//!  20 21/
//!  'pendf for h1'/
//!  125 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  125 1/
//!  0.001/
//!  293.6/
//!  0/
//! thermr
//!  30 22 23/
//!  1 125 20 1 2 0 0 2 222 1/
//!  293.6/
//!  0.001 10.0/
//! stop
//! EOF
//! ```
//!
//! `tape23` then carries MT=222 (incoherent inelastic — the whole of water's
//! thermal scattering).
//!
//! ```text
//! H2O_THERMR=/home/user/h2ooracle/tape23 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example h2o_vs_njoy_thermr
//! ```

fn main() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::groupr::panel::PointwiseXs;
    use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::thermal::ThermalScattering;

    const MATDP: i32 = 125; // H-1, the PENDF material THERMR wrote onto
    const TEMP: f64 = 293.6;

    let tsl_for_gate = reference_endf("tsl-HinH2O.endf").expect("H(H2O) tsl tape");
    let law_for_gate = ThermalScattering::from_endf_file(
        tsl_for_gate.to_str().expect("path"),
        1,
        TEMP,
        "c_H_in_H2O",
    )
    .expect("thermal law");
    golden_gate(&law_for_gate);

    let Ok(thermr_path) = std::env::var("H2O_THERMR") else {
        eprintln!(
            "\nThe live-tape comparison needs H2O_THERMR pointing at an NJOY THERMR tape\n\
             (deck in the module docs). The golden gate above already ran, so this program\n\
             is still a V&V case without it — it just cannot re-measure the oracle."
        );
        return;
    };
    eprintln!("reading NJOY THERMR tape {thermr_path} ...");
    let njoy_tape =
        Tape::read_file(std::path::Path::new(&thermr_path)).expect("THERMR tape parses");

    let tsl = reference_endf("tsl-HinH2O.endf").expect("H(H2O) tsl tape");
    eprintln!("building our S(alpha,beta) @ {TEMP} K ...");
    let ours = ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 1, TEMP, "c_H_in_H2O")
        .expect("thermal law");
    eprintln!(
        "  resolved temperature: {} K\n",
        ours.selected_temperature_k()
    );

    for (mt, label) in [(222i32, "incoherent inelastic (all of water's scattering)")] {
        let njoy = match read_pendf_cross_section(&njoy_tape, MATDP, mt) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("== MT={mt} ({label}): absent from the THERMR tape ({e:?})");
                continue;
            }
        };
        let PointwiseXs::LinLin(pairs) = &njoy.xs else {
            eprintln!("== MT={mt} ({label}): not tabulated, skipping");
            continue;
        };
        println!("\n== MT={mt}  {label}  (NJOY grid: {} points)", pairs.len());
        println!(
            "{:>11}  {:>14}  {:>14}  {:>14}  {:>9}",
            "E [eV]", "NJOY [b]", "ours inel [b]", "ours total [b]", "rel diff"
        );
        let mut worst = (0.0_f64, 0.0_f64);
        for &e in &[
            1.0e-3, 5.0e-3, 0.01, 0.0253, 0.05, 0.1, 0.2, 0.4, 0.625, 1.0, 2.0, 4.0, 8.0,
        ] {
            let n = interp_linlin(pairs, e);
            let o = ours.inelastic_xs(e);
            let t = ours.total_xs(e);
            let rel = if n.abs() > 1e-30 { (o - n) / n } else { 0.0 };
            if rel.abs() > worst.0.abs() {
                worst = (rel, e);
            }
            println!(
                "{e:>11.4e}  {n:>14.6e}  {o:>14.6e}  {t:>14.6e}  {:>+8.2}%",
                100.0 * rel
            );
        }
        println!("   worst: {:+.2}% at {:.4e} eV", 100.0 * worst.0, worst.1);
    }
}

use outram_mc_libs::vv::njoy_golden::interp_linlin;

/// V&V gate against the committed NJOY2016 oracle — runs with no tape on disk.
///
/// # Methodology
///
/// `ThermalScattering::total_xs` against THERMR MT=222 at eleven energies from
/// 1 meV to 2 eV, on `tsl-HinH2O` (MAT 1, with H-1 MAT 125) at 293.6 K — a
/// *tabulated* temperature on that tape. The oracle values, their provenance
/// and the NJOY release that produced them are in
/// [`outram_mc_libs::vv::njoy_golden`]; the deck is in this file's module docs.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// This crate sits a consistent **+0.65 % to +1.47 %** above NJOY across the
/// whole range — worst +1.47 % at 2.0 eV. The gate is **2 %**, which *permits*
/// the observed excess: this is a **characterisation** gate for the open defect
/// GitHub #188, pinning the current state so a further regression fails while
/// the fix is pending. When #188 lands, tighten it to graphite's ~0.5 %.
///
/// Three things are asserted, not one:
///
/// 1. **magnitude** — every point inside 2 %;
/// 2. **sign** — the deviation is a consistent *excess*. "Within 2 %" would
///    pass if the error flipped sign, and a flip would mean a different bug;
/// 3. **range** — this crate's law ends between 2 and 4 eV where NJOY's runs to
///    10 eV. That handover is invisible to any comparison that stops at 2 eV,
///    and it is where the thermal/epithermal seam sits.
///
/// # Interpretation
///
/// The cross section is the *area* of the scattering law. It is off by ~+1 %.
/// The *kernel* — the outgoing-energy distribution transport actually samples —
/// is off by up to −5.5 % in the opposite direction
/// (`examples/h2o_kernel_vs_njoy_thermr.rs`). A magnitude oracle alone would
/// have cleared this law, which is exactly what happened: `tests/thermal_h2o_sab.rs`
/// checks area, detailed balance, the free-atom limit and the effective
/// temperature, and a too-narrow kernel passes all four.
fn golden_gate(law: &outram_mc_libs::material::thermal::ThermalScattering) {
    use outram_mc_libs::vv::assert_table_relative;
    use outram_mc_libs::vv::njoy_golden::{H2O_LAW_UPPER_BOUND_EV, H2O_XS, H2O_XS_TOL};

    println!("=== V&V gate: H-in-H2O S(alpha,beta) cross section vs committed NJOY2016 oracle ===");

    let rows: Vec<(f64, f64, f64)> = H2O_XS
        .iter()
        .map(|&(e, njoy)| (e, law.total_xs(e), njoy))
        .collect();
    let worst = assert_table_relative(
        "H(H2O) MT=222 incoherent inelastic vs NJOY THERMR",
        &rows,
        H2O_XS_TOL,
        1.0e-6,
    );

    assert!(
        worst.rel > 0.0,
        "the H(H2O) cross-section error has changed SIGN (now {:+.2} % at {:.4e} eV). \
         The recorded defect is a consistent EXCESS of ~+1 % (GitHub #188); a deficit \
         is a different bug and this gate's premise no longer holds.",
        worst.rel * 100.0,
        worst.at,
    );
    println!(
        "  [PASS] the deviation is a consistent excess ({:+.2} % worst), as recorded",
        worst.rel * 100.0
    );

    let (lo, hi) = H2O_LAW_UPPER_BOUND_EV;
    assert!(
        law.total_xs(lo) > 0.0,
        "the H(H2O) law no longer covers {lo} eV; it did on 2026-09-11 (21.257 b)"
    );
    assert_eq!(
        law.total_xs(hi),
        0.0,
        "the H(H2O) law now extends past {hi} eV. That may be an improvement — NJOY's \
         own run reaches 10 eV — but the handover to free gas has moved, and the \
         transport's thermal/epithermal seam moved with it."
    );
    println!("  [PASS] the law still ends between {lo} and {hi} eV (NJOY's runs to 10 eV)");
}
