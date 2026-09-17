//! Compare this crate's graphite S(alpha,beta) cross sections against
//! **NJOY2016's own THERMR output** for the same tape and temperature.
//!
//! # Why
//!
//! With the U-238 reconstruction exonerated against NJOY to +-0.04%
//! (`examples/u238_vs_njoy_pendf.rs`) and the pebble geometry excluded, the
//! remaining ~+1.7% k offset vs OpenMC points at the moderator. The six factors
//! say our spectrum is *softer* than the reference's — in the reference's own
//! two-group convention, `p` (thermal share of absorption) is +6.4% and
//! `epsilon` (non-thermal share of production) is -4.0%, both meaning reactions
//! are more thermally concentrated here. Over-moderation, not under.
//!
//! Graphite sets the slowing-down on this system, so it is the suspect. This
//! asks the same question that cleared U-238: does our thermal law match the
//! code we are a port of?
//!
//! 600 K is a **tabulated** temperature on this tape, so no temperature
//! interpolation is involved and any difference is in the law itself.
//!
//! # Generating the oracle
//!
//! ```text
//! cd /home/user/graphiteoracle
//! cp .../n-006_C_012-ENDF8.0.endf tape20
//! cp .../tsl-crystalline-graphite.endf tape30
//! njoy <<'EOF'
//! reconr
//!  20 21/
//!  'pendf for c12'/
//!  625 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  625 1/
//!  0.001/
//!  600./
//!  0/
//! thermr
//!  30 22 23/
//!  30 625 16 1 2 1 0 1 229 1/
//!  600./
//!  0.001 4.0/
//! stop
//! EOF
//! ```
//!
//! The card is `matde matdp nbin ntemp iinc icoh iform natom mtref iprint`;
//! `iinc = 2` reads S(alpha,beta) from the tsl tape rather than using free gas,
//! and `icoh = 1` turns on coherent elastic. `tape23` then carries MT=229
//! (incoherent inelastic) and MT=230 (coherent elastic).
//!
//! ```text
//! GRAPHITE_THERMR=/home/user/graphiteoracle/tape23 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example graphite_vs_njoy_thermr
//! ```

fn main() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::groupr::panel::PointwiseXs;
    use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::thermal::ThermalScattering;

    const MATDP: i32 = 625; // C-12, the PENDF material THERMR wrote onto
    const TEMP: f64 = 600.0;

    let tsl_for_gate = reference_endf("tsl-crystalline-graphite.endf").expect("graphite tsl tape");
    let law_for_gate = ThermalScattering::from_endf_file(
        tsl_for_gate.to_str().expect("path"),
        30,
        TEMP,
        "c_Graphite",
    )
    .expect("thermal law");
    golden_gate(&law_for_gate);

    let Ok(thermr_path) = std::env::var("GRAPHITE_THERMR") else {
        eprintln!(
            "\nThe live-tape comparison needs GRAPHITE_THERMR pointing at an NJOY THERMR \n\
             tape (deck in the module docs). The golden gate above already ran, so this \n\
             program is still a V&V case without it — it just cannot re-measure the oracle."
        );
        return;
    };

    eprintln!("reading NJOY THERMR tape {thermr_path} ...");
    let njoy_tape =
        Tape::read_file(std::path::Path::new(&thermr_path)).expect("THERMR tape parses");

    let tsl = reference_endf("tsl-crystalline-graphite.endf").expect("graphite tsl tape");
    eprintln!("building our S(alpha,beta) @ {TEMP} K (tabulated, no interpolation) ...");
    let ours = ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 30, TEMP, "C in graphite")
        .expect("thermal law");
    eprintln!(
        "  resolved temperature: {} K\n",
        ours.selected_temperature_k()
    );

    // MT=229 incoherent inelastic, MT=230 coherent elastic (mtref, mtref+1).
    for (mt, label, ours_fn) in [
        (229i32, "incoherent inelastic", 0u8),
        (230i32, "coherent elastic", 1u8),
    ] {
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
            "{:>11}  {:>14}  {:>14}  {:>9}",
            "E [eV]", "NJOY [b]", "ours [b]", "rel diff"
        );
        let mut worst = (0.0_f64, 0.0_f64);
        for &e in &[
            1.0e-4, 1.0e-3, 5.0e-3, 0.0253, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0, 3.9,
        ] {
            let n = interp_linlin(pairs, e);
            let o = if ours_fn == 0 {
                ours.inelastic_xs(e)
            } else {
                ours.elastic_xs(e)
            };
            let rel = if n.abs() > 1e-30 { (o - n) / n } else { 0.0 };
            if rel.abs() > worst.0.abs() {
                worst = (rel, e);
            }
            println!("{e:>11.4e}  {n:>14.6e}  {o:>14.6e}  {:>+8.2}%", 100.0 * rel);
        }
        println!("   worst: {:+.2}% at {:.4e} eV", 100.0 * worst.0, worst.1);
    }
}

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

/// V&V gate against the committed NJOY2016 oracle — runs with no tape on disk.
///
/// # Methodology
///
/// `ThermalScattering::inelastic_xs` against THERMR MT=229 and `elastic_xs`
/// against MT=230, at twelve energies from 0.1 meV to 3.9 eV, on
/// `tsl-crystalline-graphite` (MAT 30) at 600 K — a *tabulated* temperature on
/// that tape, so no interpolation is involved and any difference is in the law.
/// The oracle values, their provenance and the NJOY release that produced them
/// are in [`outram_mc_libs::vv::njoy_golden`]; the deck is in this file's module
/// docs.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// Worst deviation **−0.14 %** on the incoherent-inelastic channel (at 3.9 eV)
/// and **−0.09 %** on the coherent-elastic one (at 3.0 eV). Both channels are
/// gated inside **0.5 %**.
///
/// Separately, the coherent-elastic cross section is asserted to be **exactly
/// zero** below the first Bragg edge. That is structural: a law with the cutoff
/// in the wrong place, or one that smears it, cannot produce an exact zero, and
/// no percentage envelope would notice.
///
/// # Interpretation
///
/// Graphite is the **control** for the H-in-H₂O comparison (GitHub #188). Same
/// code path, same THERMR oracle, a different law: water's cross section is
/// +1.47 % and its kernel −5.5 %, graphite's are −0.14 % and −1.53 %. That
/// contrast is what makes the water defect attributable to water rather than to
/// this crate's thermal machinery in general — so if graphite drifts, the
/// argument in #188 drifts with it, which is why this gate exists.
fn golden_gate(law: &outram_mc_libs::material::thermal::ThermalScattering) {
    use outram_mc_libs::vv::assert_table_relative;
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_FIRST_BRAGG_EDGE_ABOVE_EV, GRAPHITE_XS_COHERENT, GRAPHITE_XS_INELASTIC,
        GRAPHITE_XS_TOL,
    };

    println!(
        "=== V&V gate: graphite S(alpha,beta) cross sections vs committed NJOY2016 oracle ==="
    );

    let inel: Vec<(f64, f64, f64)> = GRAPHITE_XS_INELASTIC
        .iter()
        .map(|&(e, njoy)| (e, law.inelastic_xs(e), njoy))
        .collect();
    assert_table_relative(
        "graphite MT=229 incoherent inelastic vs NJOY THERMR",
        &inel,
        GRAPHITE_XS_TOL,
        1.0e-6,
    );

    // The Bragg cutoff is asserted exactly, before the percentage comparison,
    // because a percentage envelope is blind to it.
    for &(e, njoy) in GRAPHITE_XS_COHERENT {
        if njoy != 0.0 {
            continue;
        }
        let ours = law.elastic_xs(e);
        assert_eq!(
            ours, 0.0,
            "graphite's coherent-elastic cross section is {ours} b at {e} eV, below the \
             first Bragg edge (NJOY gives exactly zero up to \
             {GRAPHITE_FIRST_BRAGG_EDGE_ABOVE_EV} eV). The cutoff has moved or is being \
             smeared — a structural defect no percentage envelope would catch."
        );
    }
    println!("  [PASS] coherent elastic is exactly zero below the first Bragg edge");

    let coh: Vec<(f64, f64, f64)> = GRAPHITE_XS_COHERENT
        .iter()
        .map(|&(e, njoy)| (e, law.elastic_xs(e), njoy))
        .collect();
    assert_table_relative(
        "graphite MT=230 coherent elastic vs NJOY THERMR",
        &coh,
        GRAPHITE_XS_TOL,
        1.0e-6,
    );
}
