//! `bwr_mark1` against the digitised BSAF Unit 1 figures (NEA No. 7525,
//! Figs 2.1, 2.2, 2.5; `crates/kovan-literature/derived/nea7525-bsaf-unit1-figures/`).
//!
//! **Methodology** (instruments fixed in `bwr_mark1::compare` before any
//! digitised curve was seen, 2026-10-29):
//! - **Measured series** (Fig 2.1a RPV pressure, Fig 2.1b drywell
//!   pressure): the model, sampled every 60 s to 26 h with every mechanism
//!   on, is interpolated at each measured time inside the run; reported
//!   are the count, the mean and the largest absolute difference, MPa. The
//!   drywell file's four points marked `upper-suspect` are excluded, as its
//!   README says.
//! - **Code families** (Figs 2.1a, 2.1b, 2.2, 2.5a Xe, 2.5b I; nine codes
//!   each): the fraction of the times 0, 0.25, ... 25 h at which the model
//!   lies within the codes' envelope (at least two codes covering the time).
//!   These are code predictions, not measurements: a differential test,
//!   not a validation.
//!
//! **Results** (2026-10-29, the model at `eca21cc`, the data at `8067264`):
//! in the test's doc comment, with the interpretation; pinned.
//!
//! Acknowledgement: OECD Nuclear Energy Agency, *Benchmark Study of the
//! Accident at the Fukushima Daiichi Nuclear Power Plant*, NEA No. 7525,
//! © OECD 2021. Facts and numbers only.

use sembawang::bwr_mark1::compare::{against_measured, histories, inside_envelope, Histories};
use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::Mechanisms;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/derived/nea7525-bsaf-unit1-figures")
}

/// `time_h,value[,basis]` rows, skipping the header and any row whose
/// basis contains `suspect`.
fn read_series(file: &str) -> Vec<(f64, f64)> {
    let text = std::fs::read_to_string(data_dir().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    text.lines()
        .skip(1)
        .filter(|l| !l.contains("suspect"))
        .map(|l| {
            let mut c = l.split(',');
            (c.next().unwrap().parse().unwrap(), c.next().unwrap().parse().unwrap())
        })
        .collect()
}

const CODES: [&str; 9] = ["ciemat", "iae", "ibrae", "irsn", "jaea", "kaeri", "nra", "snl", "vtt"];

fn family(fig: &str) -> Vec<Vec<(f64, f64)>> {
    CODES.iter().map(|c| read_series(&format!("fig{fig}-{c}.csv"))).collect()
}

fn model() -> Histories {
    histories(Mechanisms::default(), Params::default(), 60.0, 26.0 * 3600.0)
}

/// The model's histories against the digitised figures.
///
/// **Results, 2026-10-29** (the first run; pinned below):
///
/// | Check | Result |
/// |---|---|
/// | Measured RPV pressure (2 points) | mean 3.634, max 6.712 MPa |
/// | Measured drywell pressure (33 points) | mean 0.338, max 0.509 MPa |
/// | Inside the 9-code envelope: RPV / drywell / H2 | 3.3 % / 17.6 % / 9.2 % of times |
/// | Inside the 9-code envelope: Xe / I release | 54.2 % / 93.8 % of times |
///
/// **Interpretation.**
/// - **The RPV was measured at 7.00 MPa at 5.34 h; the model is at 0.29
///   MPa.** Its main steam line has already ruptured by creep (4.78 h),
///   so the measurement contradicts the rupture's timing. The rupture
///   rests on \[A\] inputs (line size and steel, the dome internals) and
///   on ~~an upper-bound gas temperature~~ the gas temperature after the
///   dome internals (CORRECTED 2026-10-29: the internals heat structure
///   was in place by then; the "upper bound" was the line's gas before it). For context only, not adopted:
///   with `without_msl_creep` the model is at 4.25 MPa at 5.34 h (mean
///   1.63 MPa over the two points), still low, because the \[A\] hot
///   leak depressurises it. Both leak paths need plant sources (F13).
/// - **The drywell is 0.24-0.51 MPa low over 10-23 h** (0.34 against
///   0.85 MPa at 11.7 h; 0.53 against 0.77 MPa at 22.9 h), with or without
///   the steam-line rupture. ~~Candidates, to test, not to tune: the
///   drywell heat sink is three \[A\] placeholders (4000 m^2, 20 W/(m^2
///   K), 5e8 J/K); no MCCI gas; the pool over-condenses once near
///   saturation.~~ **Tested the same day** (diagnostic runs, nothing
///   adopted): the drywell rides the wetwell (0.342 against 0.330 MPa at
///   11.7 h; the downcomers pass anything above the submergence head),
///   and the pool is at 344 K, about 100 K subcooled, so every kilogram of
///   steam condenses and the containment pressure is the non-condensable
///   gas alone. Hence: the **heat sink** cannot carry it (a tenth of its
///   area raises the drywell 0.02-0.03 MPa); **pool over-condensation**
///   cannot act before the pool nears saturation (about 20 h); what is
///   missing is **about 6e5 mol of non-condensable gas** (0.4 MPa over
///   about 6000 m^3 of containment gas near 500 K: about 1.2 t of H2 or
///   16 t of CO). BSAF's text says the codes held the 15-24 h plateau by
///   balancing **MCCI gas** against a drywell-head flange leak (NEA No.
///   7525, p.19), and the model has no MCCI (F8, blocked on Unit 1's
///   concrete).
/// - **H2** is inside the codes' spread only 9 % of the time: it is
///   lower than all nine codes from about 5 h on (215 kg at 25 h against
///   398-1315 kg).
/// - **Release from fuel** sits inside the I envelope 94 % of the time
///   and the Xe envelope 54 %.
///
/// **Moved 2026-10-29, by the internals-heat-loss regime flip** (the run
/// now in the early regime): RPV 3.634/6.712 -> 3.591/6.675 MPa; drywell
/// 0.338/0.509 -> 0.289/0.458 MPa; envelopes RPV 3.3 % (same), drywell
/// 17.6 -> 20.9 %, H2 9.2 % (same), Xe 54.2 -> 43.8 %, I 93.8 -> 95.8 %.
///
/// **Moved 2026-10-29 by decay heat following the fission products**
/// (`fp_decay_follows`): RPV 3.591/6.675 -> 3.579/6.664 MPa; drywell
/// 0.289/0.458 -> 0.274/0.448 MPa (closer); envelopes RPV 3.3 -> 4.4 %,
/// drywell 20.9 -> 22.0 %, H2 9.2 % (same), Xe 43.8 -> 41.7 %, I 95.8 %
/// (same).
///
/// **Moved 2026-10-29 by tellurium transport** (`te_transport`): RPV
/// 3.579/6.664 -> 3.578/6.662 MPa; drywell 0.274/0.448 -> 0.274/0.446
/// MPa; envelopes RPV 4.4 -> 3.3 %, H2 9.2 -> 10.3 %, the rest the same.
///
/// **Moved 2026-10-29 by debris oxidation** (`debris_oxidation`): RPV
/// 3.578/6.662 -> 3.564/6.648 MPa; drywell 0.274/0.446 -> 0.260/0.433 MPa
/// (closer); envelopes drywell 22.0 -> 23.1 %, Xe 41.7 -> 58.3 %, the rest
/// the same.
///
/// **Moved 2026-10-29 by candling, by much** (`candling`): drywell
/// 0.260/0.433 -> **0.189/0.368 MPa**; RPV 3.564/6.648 -> 3.505/6.596
/// MPa; envelopes RPV 3.3 -> 6.6 %, drywell 23.1 -> 26.4 %, **H2 10.3 ->
/// 98.9 %**, Xe 58.3 -> 89.6 %, I 95.8 -> 91.7 %.
///
/// **Moved 2026-10-29 by MELCOR's falling-debris quench**
/// (`debris_quench`): drywell 0.189/0.368 -> **0.158/0.332 MPa**; RPV
/// 3.505/6.596 -> 3.468/6.558 MPa; envelopes **RPV 6.6 -> 35.2 %**,
/// drywell 26.4 -> 48.4 %, H2 98.9 -> 18.4 % (H2 at 25 h is now just
/// below its band), Xe 89.6 -> 83.3 %, I 91.7 % (same).
///
/// **Moved 2026-10-29 by the lower head under water** (`wet_head_contact`,
/// `penetration_coverage` and `failed_vessel_drain`, measured together):
/// drywell 0.158/0.332 -> **0.263/0.391 MPa (worse)**; RPV 3.468/6.558 ->
/// 3.533/6.626 MPa; envelopes RPV 35.2 -> 4.4 %, drywell 48.4 -> 26.4 %,
/// H2 18.4 -> 9.2 % (253 kg at 25 h, further below its band), Xe 83.3 %
/// (same), I 91.7 -> 87.5 %. Kept: the head conducting from the debris it
/// touches under water is MELCOR's model, and the fit is not a reason to
/// leave it out. The head now fails earlier (4.56 h) with less H2 made,
/// and leans on the missing core-plate holdup (L4).
///
/// **Moved 2026-10-29 by the wetwell atmosphere** (`ww_atmosphere`):
/// drywell 0.263/0.391 -> **0.275/0.398 MPa (worse)**; RPV 3.533/6.626 ->
/// 3.535/6.624 MPa; envelopes drywell 26.4 -> 23.1 %, the rest the same.
/// The wetwell gas, no longer saturated at the pool's temperature, holds
/// the wetwell and so the drywell lower before the vent (22 kPa at 22 h).
///
/// **Moved 2026-10-29 by the drywell structures' MELCOR surface**
/// (`dw_wall_hs`): drywell 0.275/0.398 -> 0.278/0.400 MPa; RPV 3.535/6.624
/// -> 3.540/6.632 MPa; envelopes the same.
///
/// **Moved 2026-10-29 by the torus shell** (`ww_shell`; the vent 0.13 h
/// later): drywell 0.278/0.400 -> 0.282/0.404 MPa; RPV 3.540/6.632 ->
/// 3.547/6.642 MPa; drywell envelope 23.1 -> 22.0 %.
///
/// **Moved 2026-10-29 by the drywell's outflows taking their enthalpy**
/// (`outflow_enthalpy`): drywell 0.282/0.404 -> 0.279/0.403 MPa; RPV
/// 3.547/6.642 -> 3.548/6.645 MPa; drywell envelope 22.0 -> 23.1 %.
///
/// **Moved 2026-10-29 by the drywell's condensate draining to its floor**
/// (`condensate_to_floor`; the debris boils it again): drywell
/// 0.279/0.403 -> **0.263/0.390 MPa** (closer); RPV 3.548/6.645 ->
/// 3.541/6.646 MPa; envelopes the same.
///
/// **Moved 2026-10-29 by the RPV wall** (`rpv_wall`; the head fails 0.07 h
/// later, 25 kg less H2): drywell 0.263/0.390 -> 0.274/0.400 MPa; RPV
/// 3.541/6.646 -> 3.549/6.651 MPa; envelopes RPV 4.4 -> 3.3 %, drywell
/// 23.1 -> 25.3 %, the rest the same.
///
/// **Moved 2026-10-29 by the internals' and the core top's radiation**
/// (`internals_radiation`, `core_top_radiation`): drywell 0.274/0.400 ->
/// 0.276/0.402 MPa; RPV 3.549/6.651 -> 3.550/6.652 MPa; envelopes the same.
///
/// **Moved 2026-10-29 by core-concrete interaction** (`mcci`, a reduced
/// MELCOR CAV; 41 t of basaltic concrete ablated by 25 h, 271 kg of H2 and
/// 395 kg of CO): drywell **0.276/0.402 -> 0.153/0.375 MPa** (much
/// closer); RPV 3.550/6.652 -> 3.526/6.631 MPa; envelopes RPV 3.3 -> 8.8
/// %, drywell 25.3 -> 33.0 %, the rest the same.
///
/// **Moved 2026-10-29 by the drywell structures' outer heat path**
/// (`dw_wall_outer`): drywell 0.153/0.375 -> 0.198/0.386 MPa (further:
/// the concrete takes heat the structures kept); RPV 3.526/6.631 ->
/// 3.537/6.642 MPa; envelopes RPV 8.8 -> 5.5 %, drywell 33.0 -> 26.4 %.
/// Kept: the structures had no outer path at all, which drove the
/// unbounded long-run heating recorded in the Cs-137 test.
///
/// **Moved 2026-10-29 by the core plate's holdup** (`core_plate`; the head
/// fails at 5.00 h, H2 184 kg): drywell 0.198/0.386 -> 0.236/0.419 MPa;
/// RPV 3.537/6.642 -> 3.569/6.672 MPa; envelopes drywell 26.4 -> 24.2 %,
/// H2 9.2 -> 8.0 %, I 87.5 -> 89.6 %, the rest the same.
///
/// The measured series are the validation; the envelopes are a
/// differential test against other codes, none of which is the truth.
#[test]
fn the_model_against_the_digitised_bsaf_figures() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let rpv = against_measured(&m.t_h, &m.p_rpv_mpa, &read_series("fig2.1a-measurement.csv"));
    let dw = against_measured(&m.t_h, &m.p_dw_mpa, &read_series("fig2.1b-measurement.csv"));
    let env = |v: &[f64], fig: &str| inside_envelope(&m.t_h, v, &family(fig), &grid);
    let (e_rpv, e_dw, e_h2, e_xe, e_i) =
        (env(&m.p_rpv_mpa, "2.1a"), env(&m.p_dw_mpa, "2.1b"), env(&m.h2_kg, "2.2"), env(&m.fp_released, "2.5a"), env(&m.fp_released, "2.5b"));
    println!("measured RPV pressure: {rpv:?}");
    println!("measured drywell pressure: {dw:?}");
    println!("inside the code envelope (fraction, times counted): RPV {e_rpv:?}, drywell {e_dw:?}, H2 {e_h2:?}, Xe {e_xe:?}, I {e_i:?}");
    // Pinned to the first run: a change to the model moves these, and the
    // move is then recorded here with its old and new value.
    assert_eq!((rpv.n, dw.n), (2, 33));
    // With the bottom of the core's radiation (2026-10-29): RPV mean
    // 3.5690 -> 3.5577, max 6.6724 -> 6.6805 MPa; drywell mean 0.2361 ->
    // 0.2640, max 0.4187 -> 0.3860 MPa; inside the envelopes RPV 0.0549,
    // drywell 0.2418 -> 0.2747, H2 0.0805 -> 0.0920, Xe 0.8333, I 0.8958 ->
    // 0.9167. With a core plate per ring (2026-10-29): RPV mean 3.5577 ->
    // 3.5090, max 6.6805 -> 6.6246 MPa; drywell mean 0.2640 -> 0.1476, max
    // 0.3860 -> 0.3468 MPa; envelopes RPV 0.0549 -> 0.2308, drywell 0.2747
    // -> 0.4945, H2 0.0920, Xe 0.8333 -> 0.7708, I 0.9167 -> 0.8542. With
    // the pool in two layers: RPV 3.5090/6.6246 -> 3.4653/6.6211 MPa;
    // drywell 0.1476/0.3468 -> 0.1186/0.2657 MPa; envelopes RPV 0.2308 ->
    // 0.5055, drywell 0.4945 -> 0.5714, the rest the same. With the
    // plate's debris oxidising: RPV 3.4548/6.6142, drywell 0.1152/0.2515
    // MPa; envelopes RPV 0.5165, drywell 0.5934, Xe 0.7917. With the
    // plenum's oxidation steam consumed: RPV 3.4597/6.6170, drywell
    // 0.1168/0.2585 MPa; envelopes RPV 0.5055, drywell 0.5714, Xe 0.8125.
    // With the control blades: RPV 3.4523/6.5929, drywell 0.1048/0.2644
    // MPa; envelopes RPV 0.4945, drywell 0.5824, **H2 0.0920 -> 0.9885**,
    // Xe 0.8333. With the blades' heat of fusion: RPV 3.4393/6.5634,
    // drywell 0.1086/0.2678 MPa; envelopes RPV 0.4835, I 0.8750. With the
    // cavity debris's phases: RPV 3.4328/6.5634, **drywell 0.0780**/0.2636
    // MPa; envelopes RPV 0.3736. With the vent and flange choked: drywell
    // mean **0.0666** MPa; envelopes RPV 0.6264, drywell 0.7143. With the
    // blades' diffusion limit: RPV 3.4340/6.5645, drywell 0.0671/0.2650
    // MPa; envelopes RPV 0.6154, I 0.8542. With the two-sided equalising:
    // RPV 3.4540/6.6080, drywell 0.0670/0.2625 MPa; envelopes RPV 0.5714,
    // drywell 0.6593, Xe 0.8125. With the blades' B4C: RPV 3.4542/6.5997,
    // drywell 0.0706/0.2709 MPa; envelopes RPV 0.5165, drywell 0.7033.
    // With uo2 carried by candling zr: RPV 3.4388/6.5826, drywell 0.0670/0.2573
    // MPa; envelopes RPV 0.6154, drywell 0.6703.
    assert!((rpv.mean_abs - 3.4388).abs() < 1e-3 && (rpv.max_abs - 6.5826).abs() < 1e-3, "{rpv:?}");
    assert!((dw.mean_abs - 0.0670).abs() < 1e-3 && (dw.max_abs - 0.2573).abs() < 1e-3, "{dw:?}");
    let near = |(f, _): (f64, usize), x: f64| (f - x).abs() < 1e-3;
    assert!(near(e_rpv, 0.6154) && near(e_dw, 0.6703) && near(e_h2, 0.9885) && near(e_xe, 0.8125) && near(e_i, 0.8542));
}



/// **Beyond the model's window: 0-500 h against Fig 2.3a's measured
/// drywell pressure** (the same instrument; diagnostic, pinned so F12 has a
/// baseline). Results, 2026-10-29: 94 measured points, mean 0.653 MPa, max
/// ~~0.818~~ ~~0.915~~ 0.835 MPa (since the failed vessel's gas leaves to the
/// drywell, the same day: mean 0.6535 -> 0.6539; since the internals heat
/// loss: mean 0.6539 -> 0.6616, max 0.915 -> 0.835). ~~The model's drywell,
/// vented on the recorded order near 24 h, falls to 0.16 MPa at 30 h, then
/// with no long-term leak path rises to a 0.97 MPa plateau held from about
/// 100 h to 500 h~~ **CORRECTED 2026-10-29:** the rise to 0.97 MPa was not
/// a missing leak path. It was the vent defect below (gh:#1053): with no
/// non-condensable left, the vent carried no steam, so the pool heated to
/// 466 K under a "vented" wetwell. With the mixture vent, the drywell
/// holds 0.113-0.119 MPa from 30 h to 500 h, the vent open throughout:
/// mean **0.6616 -> 0.1670 MPa**, max **0.835 -> 0.626**. The model is now
/// low where it was high: the plant re-pressurised to about 0.6 MPa from
/// 45 to 56 h (0.5955 MPa at 54.0 h; 93 of the 94 points are after 30 h).
/// The plant fell to 0.15 MPa by about 257 h.
/// Every BSAF code followed the measurement by imposing a variable-area
/// containment leak from 50 h (NEA No. 7525, p.19); that leak, and the
/// injection that bypasses the core, are F12, and imposing an area to
/// follow the curve would be fitting it. The model's vent never closes;
/// the plant's line-up after 24 h is not modelled.
///
/// **A defect underneath, found 2026-10-29 (gh:#1047 work):** the wetwell's
/// pressure went NaN at 73.48 h. The vent had taken all of its
/// non-condensable gas, and the vent's share `n / n_g` was then 0/0.
/// Guarded since, with no change to these numbers: by then the
/// pool, at 466 K, held the wetwell at its steam pressure (about 1.4 MPa),
/// level with the drywell, so no downcomer would have flowed either way. A
/// pool that hot under a vented wetwell is itself a gap: with no
/// non-condensable left, the vent carries no steam, because its steam is
/// tied to the gas it carries (gh:#1053, with F12). **Fixed the same day**
/// by the mixture vent (`Mechanisms::mixture_vent`; above).
#[test]
fn the_model_beyond_its_window_against_fig_2_3a() {
    let m = histories(Mechanisms::default(), Params::default(), 600.0, 500.0 * 3600.0);
    let a = against_measured(&m.t_h, &m.p_dw_mpa, &read_series("fig2.3a-measurement.csv"));
    println!("0-500 h drywell against Fig 2.3a: {a:?}");
    assert_eq!(a.n, 94);
    // Max 0.6265 -> 0.6212 with tellurium transport, and mean 0.1670 ->
    // 0.1660, max 0.6219 with candling; mean 0.1672, max 0.6224 with the
    // lower head under water; mean 0.1677, max 0.6245 with the wetwell
    // atmosphere; mean 0.1659, max 0.6239 with MCCI; mean 0.1664, max
    // 0.6251 with the drywell's outer heat path; mean 0.1677, max 0.6255
    // with the bottom of the core's radiation; mean 0.1714, max 0.6265 with
    // the pool in two layers (2026-10-29); mean 0.1665, max 0.6249 with the
    // control blades; mean 0.1690, max 0.6259 with the cavity debris's
    // phases (2026-10-30).
    assert!((a.mean_abs - 0.1690).abs() < 1e-3 && (a.max_abs - 0.6259).abs() < 1e-3, "{a:?}");
}


/// **Cesium against Fig 2.6** (gh:#1047; the envelope instrument fixed
/// before any curve was seen). Methodology: cesium released by CORSOR-M
/// (154.0 kg inventory, the benchmark specification's), carried as vapour
/// with the UO2 seed aerosol out of the vessel (no in-vessel deposition,
/// stated), through the containment by the sectional aerosol and
/// TRAP-MELT2 condensation (`rn_volume`); no pool scrubbing (SPARC-90 not
/// built; MELCOR's default with pool interaction off), so the S/C water's
/// cesium is what settles onto the pool surface. ~~no pool scrubbing~~
/// Since 2026-10-29 the gas entering the pool (relief quenchers,
/// downcomers) is scrubbed by its steam's condensation, SPARC-90's DF_EC
/// (a lower bound on SPARC's scrubbing); the S/C water's cesium is the
/// scrubbed plus the settled. Fractions of the
/// inventory against the nine codes' envelope, 0-25 h in 0.25 h steps.
///
/// Results, 2026-10-29:
/// - **Cs in the S/C water (Fig 2.6c): inside the envelope 85.4 % of 89
///   times.** 0.064 at 10 h, 0.706 at 20 h, 0.807 at 26 h, just above the
///   highest code at the end (JAEA 0.787; IBRAE 0.000, SNL 0.33). Predicted
///   before the run: at or above the top late on, so mostly outside.
///   **Missed**: inside most of the time, crossing the top only late.
/// - **Cs on drywell structures (Fig 2.6a): inside 46.7 % of 75 times.**
///   0.081 at 10 h, then 0.000 by 20 h: the deposit re-evaporates as the
///   drywell structures heat (the ex-vessel debris's heat goes into them),
///   which five of BSAF's nine codes also show as remobilisation.
///
/// **Moved 2026-10-29 by the pool scrubbing (DF_EC):** Cs in the S/C water
/// 0.807 -> 0.830 at 26 h (predicted about unchanged, +-0.05: held), but
/// inside the envelope 85.4 % -> 60.7 % of times, because the scrubbed
/// cesium reaches the pool earlier and runs above the codes sooner. Drywell
/// structures unchanged. `without_pool_scrubbing` reproduces the earlier
/// numbers exactly (pinned below).
///
/// **Moved again 2026-10-29 by in-vessel deposition** (cesium vapour onto
/// the dome internals): Cs in the S/C water inside 60.7 % -> 64.0 %.
/// Predicted before the run: the S/C water's cesium falling to 0.4-0.7
/// and spending more time inside. **Missed on the value**: the internals
/// hold up to 19.4 kg (5.45 h), then reach 1005 K, and with no heat-loss
/// path (a stated artefact) the deposit re-evaporates and reaches the
/// containment after all. Iodine does not condense on them.
///
/// **Moved again 2026-10-29 by the internals heat loss** (and its regime
/// flip): the internals now keep 11.3 kg of cesium at 26 h; Cs in the S/C
/// water inside 64.0 % -> 97.8 %, on drywell structures 46.7 % -> 36.0 %.
///
/// **Unmoved 2026-10-29 by `X_o` and the swarm rise** (to the 1e-3 of
/// these fractions), though both move the amounts: at 26 h, cesium in the
/// pool 106.22 kg (neither) -> 103.96 (swarm only) and 105.98 (`X_o`
/// only) -> 103.52 (both); vented at 23.7 h 3.67 -> 5.93, 3.91 -> 6.37.
/// **The swarm's sign on the vented cesium is a miss** (predicted: less
/// aerosol into the wetwell, so less vented). It removes more at the
/// downcomers (pool 53.85 against 50.01 kg at 12 h), but by 23 h more is
/// airborne (6.95 against 4.50 kg). The leading hypothesis~~, untested~~:
/// the cesium on aerosol is not tracked by section but lumped, so it leaves
/// the wetwell gas at the aerosol's *mass-weighted* settling rate. Without
/// the swarm, the large particles kept that rate high; once they are
/// scrubbed at the vent, the rate falls and the lumped cesium stays
/// airborne. Tracking each class by section, as MELCOR does, is the test.
///
/// **Tested the same day, partly confirmed** (`sectional_classes`: each
/// class condensed onto each section by its mass transfer, carried through
/// agglomeration and deposited with its section). Vented cesium at 26 h:
///
/// | | without swarm | with swarm | swarm's effect |
/// |---|---|---|---|
/// | lumped | 3.906 kg | 6.369 kg | +2.463 kg |
/// | sectional | 6.994 kg | 7.930 kg | +0.936 kg |
///
/// Lumping carried 62 % of the swarm's effect, but the sign survives.
/// Predicted "at most 0": **missed**. Remaining candidate~~, untested~~:
/// with the large particles scrubbed at the vent, less coagulation
/// scavenges the small cesium-bearing ones in the wetwell. **Confirmed the
/// same day** by a scratch run with the wetwell's agglomeration switched
/// off (not committed): vented cesium 10.634 kg without the swarm, 9.122
/// with it. The swarm then *lowers* the vent's cesium by 1.51 kg. So the
/// +0.94 kg is the model's physics, not an artefact: the drywell's large
/// particles, which the swarm removes, would have swept up the small ones
/// in the wetwell. Predicted for sectional
/// tracking itself: more cesium airborne, since condensation by area puts
/// it on the slow-settling small particles. **Held**: vented cesium 6.37
/// -> 7.93 kg; Cs in the pool at 26 h 103.52 -> 101.96 kg; Cs on drywell
/// structures inside the envelope **36.0 % -> 33.3 %**, S/C water
/// unchanged at 97.8 %. `without_sectional_classes` reproduces the lumped
/// numbers to ten digits.
///
/// **Moved 2026-10-29 by phoresis** (thermophoresis from the drywell
/// walls' heat flux, diffusiophoresis from the steam condensing on them;
/// MELCOR's defaults): Cs on drywell structures inside **33.3 % -> 38.7
/// %**. Predicted: more on the structures. Held while there is a deposit
/// (4 h 0.67 against 0.10 kg, 8 h 5.49 against 3.99, 12 h 6.76 against
/// 6.39); both runs re-evaporate it by 16 h as the drywell heats. S/C
/// water unchanged.
///
/// **Moved 2026-10-29 by aerosol dynamics in the vessel**
/// (`vessel_aerosol`): Cs on drywell structures inside **38.7 % ->
/// 41.3 %**, S/C water unchanged. Predicted: some of the release held in
/// the vessel. **Missed, in the other direction**: the in-vessel UO2
/// aerosol (about 1.6e5 m^2 of surface against the internals' 2500 m^2)
/// takes the CsOH vapour, and leaves with the gas before it can deposit.
/// The internals hold 5.5 kg at 5 h (was 21.0) and 0.05 kg at 26 h (was
/// 11.3). Cs in the pool at 26 h is 101.95 -> 114.23 kg, vented 7.95 ->
/// 8.76 kg. A stated limit bears on it: the condensed cesium does not
/// grow the particles, which in MELCOR would cut their area and speed
/// their settling. ~~(that limit)~~ **Revisited the same day:** MELCOR
/// does not grow particles across sections by vapour condensation; the
/// condensed mass becomes its section's aerosol mass, so area, number and
/// agglomeration follow the total (RN §2.5.2), which is what was missing.
///
/// **Moved 2026-10-29 by the condensate's mass** (`condensate_mass`):
/// Cs on drywell structures inside **41.3 % -> 22.7 %**, S/C water
/// unchanged. With about 100 kg of CsOH on 2.6 kg of UO2, the aerosol
/// agglomerates and settles far faster. Predicted: more kept in the vessel
/// (held, internals 0.05 -> 5.78 kg at 26 h), more on the drywell
/// structures (held, 8.6 -> 60.2 kg at 12 h, which overshoots the codes
/// and lowers the score; the deposit then re-evaporates by 16 h as the
/// drywell heats), less vented (held, 8.76 -> 0.37 kg), less in the pool
/// (**missed**: 114.2 -> 116.9 kg).
///
/// **Moved 2026-10-29 by decay heat following the fission products**:
/// Cs on drywell structures inside **22.7 % -> 37.3 %**, S/C water 97.8
/// -> 96.6 %. Each location now takes its share of the cesium class's
/// decay heat, and the thermal history moved with it.
///
/// **Moved 2026-10-29 by tellurium transport** (its heat now leaves the
/// core too; the drywell's history moves): Cs on drywell structures
/// inside **37.3 % -> 50.7 %**, S/C water unchanged. Then by debris
/// oxidation: 50.7 % -> 49.3 %. Then by candling: **49.3 % -> 80.0 %**,
/// S/C water 96.6 -> 94.4 %. Then by MELCOR's quench: S/C water 94.4 ->
/// 95.5 %, structures unchanged. Then by the lower head under water: S/C
/// water 95.5 -> 96.6 %, structures unchanged. Then by the drywell
/// structures' MELCOR surface (condensation on them carries Cs there by
/// diffusiophoresis): structures 80.0 -> 82.7 %. Then by the RPV wall:
/// structures 82.7 -> 81.3 %. Then by MCCI: structures 81.3 -> 93.3 %; by
/// the drywell's outer heat path, 93.3 -> 80.0 %; by the core plate,
/// 80.0 -> 84.0 %.
///
/// These are a differential test against codes, not a validation. The
/// result rests on \[A\] areas (drywell floor, pool surface) and on the
/// missing in-vessel deposition and pool scrubbing.
#[test]
fn cesium_against_fig_2_6() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let a = inside_envelope(&m.t_h, &m.cs_dw_structures, &family("2.6a"), &grid);
    let c = inside_envelope(&m.t_h, &m.cs_sc_water, &family("2.6c"), &grid);
    println!("Cs on drywell structures {a:?}; Cs in S/C water {c:?}");
    assert_eq!((a.1, c.1), (75, 89));
    // Cs on the drywell's structures 0.8400 -> 0.9200 with the bottom of
    // the core's radiation, -> 0.8000 with a plate per ring (2026-10-29),
    // -> 0.9067 with the control blades, -> 0.9333 with their heat of
    // fusion, -> 0.9467 with the cavity debris's phases (2026-10-30), ->
    // 0.9733 with the blades' B4C, -> 0.8667 with UO2 carried by candling Zr.
    assert!((a.0 - 0.8667).abs() < 1e-3 && (c.0 - 0.9663).abs() < 1e-3, "{a:?} {c:?}");
    // The earlier model: also lumped (sectional classes alone move this
    // ablation's 0.8539 to 0.9213).
    let off = histories(
        Mechanisms::default().without_pool_scrubbing().without_in_vessel_deposition().without_internals_heat_loss().without_sectional_classes().without_phoresis().without_vessel_aerosol().without_condensate_mass().without_fp_decay_follows().without_te_transport().without_debris_oxidation().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising().without_b4c_oxidation().without_burn_dynamics().without_fdi_lpme().without_uo2_candling().without_molten_pool(),
        Params::default(),
        60.0,
        26.0 * 3600.0,
    );
    let c_off = inside_envelope(&off.t_h, &off.cs_sc_water, &family("2.6c"), &grid);
    assert!((c_off.0 - 0.8539).abs() < 1e-3, "{c_off:?}");
}


/// **Iodine against Fig 2.6b and 2.6d** (gh:#1047; same method as cesium,
/// 11.98 kg inventory, the benchmark specification's; released by the same
/// CORSOR-M row, as I2 vapour).
///
/// Results, 2026-10-29: **inside the codes' envelope 0 % of the time** for
/// both iodine on drywell structures (76 times counted) and in the S/C water
/// (89): the model keeps all 10.19 kg of released iodine airborne. At the
/// 344 K wetwell I2's saturation concentration is about 0.19 kg/m^3 against
/// a loading of about 0.004, so it never condenses. Predicted before the
/// run: near zero, inside only where the lowest codes are near zero;
/// consistent (every code keeps some iodine at every counted time). This is
/// what MELCOR's defaults give without SPARC (released iodine is I2; CsI
/// forms only when requested): iodine's retention in the codes comes from
/// pool scrubbing (SPARC-90 scrubs I2 vapour) and chemistry, neither built.
///
/// **Moved 2026-10-29 by the pool scrubbing (DF_EC):** iodine in the S/C
/// water is inside the envelope **53.9 %** of the times (was 0 %); 7.98 of
/// the 10.19 kg released is in the pool at 26 h (0.666 of inventory, just
/// above the highest code, CIEMAT's about 0.62, at the end). Predicted
/// before the run: rising to 0.3-0.6, inside much of the time: held,
/// roughly. Drywell structures still 0 %. `without_pool_scrubbing`
/// reproduces 0 % (pinned).
///
/// **Moved 2026-10-29 by MELCOR's `X_o`** (the condensation scrubbing stops
/// where the pool is near saturation, Eq D-6; `scrubbing_saturation`):
/// inside **53.9 % -> 55.1 %**; iodine in the pool at 26 h 7.937 -> 7.785
/// kg, vented 1.634 -> 1.786 kg. Predicted: less scrubbing, so less in the
/// pool and more vented: held. The swarm rise (`swarm_scrubbing`) moves no
/// iodine: it is all vapour, and the vapour's swarm scrubbing (MELCOR
/// Appendix F) ~~is not built~~ is built since (below).
///
/// **Moved 2026-10-29 by the swarm's scrubbing of I2 vapour** (Appendix
/// F, `iodine_swarm`): inside **55.1 % -> 52.8 %**; iodine in the pool at
/// 26 h 7.785 -> 9.410 kg (0.786 of the inventory), vented 1.787 -> 0.161
/// kg. Predicted: more in the pool, less vented, with a downcomer factor
/// of order tens (the unit test gives 11.6): held. The pool's fraction
/// moves further above the highest code's (about 0.62); the fit worsens
/// and is recorded, not tuned. The relief valves' quenchers get no swarm
/// scrubbing, their geometry not being known.
///
/// **Moved 2026-10-29 by decay heat following the fission products**:
/// inside 52.8 % -> 51.7 %.
///
/// **Moved 2026-10-29 by candling**: inside **51.7 % -> 4.5 %**. The
/// release is now 0.99, and the pool holds 11.45 kg of iodine at 26 h
/// (0.96 of the inventory), far above every code (at most about 0.62).
/// The fit worsens and is recorded, not tuned: the scrubbing is MELCOR's,
/// and the gap is iodine's chemistry (MELCOR releases it as I2 by
/// default; CsI and pool re-evolution are not modelled).
///
/// **Moved 2026-10-29 by the lower head under water**: inside 4.5 % ->
/// 6.7 %; by the RPV wall, 6.7 -> 7.9 %; by MCCI, 7.9 -> 6.7 %; by the
/// core plate, 6.7 -> 7.9 %.
#[test]
fn iodine_against_fig_2_6() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let b = inside_envelope(&m.t_h, &m.i_dw_structures, &family("2.6b"), &grid);
    let d = inside_envelope(&m.t_h, &m.i_sc_water, &family("2.6d"), &grid);
    println!("I on drywell structures {b:?}; I in S/C water {d:?}");
    assert_eq!(b, (0.0, 76));
    // 0.0787 -> 0.0674 with the bottom of the core's radiation (2026-10-29);
    // -> 0.0562 with UO2 carried by candling Zr (2026-10-30).
    assert!((d.0 - 0.0562).abs() < 1e-3 && d.1 == 89, "{d:?}");
    let off = histories(
        Mechanisms::default().without_pool_scrubbing().without_in_vessel_deposition().without_internals_heat_loss().without_fp_decay_follows().without_debris_oxidation().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising().without_b4c_oxidation().without_burn_dynamics().without_fdi_lpme().without_uo2_candling().without_molten_pool(),
        Params::default(),
        60.0,
        26.0 * 3600.0,
    );
    assert_eq!(inside_envelope(&off.t_h, &off.i_sc_water, &family("2.6d"), &grid), (0.0, 89));
}

/// One BSAF Fig 5.1 series (`time_day,release_rate_bq_per_hr`) as `(t_h,
/// Bq/hr)`.
fn read_release(code: &str) -> Vec<(f64, f64)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/derived/nea7525-bsaf-cs137-release-rates");
    let text = std::fs::read_to_string(dir.join(format!("fig5.1-{code}.csv"))).unwrap_or_else(|e| panic!("{code}: {e}"));
    text.lines()
        .skip(1)
        .map(|l| {
            let mut c = l.split(',');
            (c.next().unwrap().parse::<f64>().unwrap() * 24.0, c.next().unwrap().parse().unwrap())
        })
        .collect()
}

/// **The Unit 1 Cs-137 release against BSAF Fig 5.1** (gh:#1040, the F9
/// "done when"): five codes' release rates to the atmosphere, Bq/hr over
/// 20 days (CIEMAT, IAE, IRSN, KAERI, SNL), and Table 5.1's integrals
/// (codes 0.1-18.9 PBq; GRS's backward reconstruction 0.79 PBq, the
/// closest thing to an observation, though BSAF grades per-unit splits
/// weakly). Differential against codes, with one reconstruction.
///
/// **Methodology.** The model's cesium released to the environment (vented
/// to the stack plus leaked to the building, whose own retention is not
/// modelled) as a fraction of its 154 kg inventory, times the Cs-137 share
/// of the cesium mass at the scram from the model's ENDF/B-VIII summation
/// (`fp_decay`: 0.3167), times Cs-137's specific activity, 3.212e15 Bq/kg
/// (`ln 2 / 30.08 y` times `N_A / 0.13691 kg`); the isotopes are taken to
/// move together. **The share is biased low**: the summation has no
/// neutron capture during irradiation, so Xe-135 is not burned and Cs-135
/// (33 kg) outweighs Cs-133 (32 kg); stated, not corrected. The rate is the
/// central difference of the cumulative release on the run's 600 s
/// samples; the envelope test is in `log10` of Bq/hr over 24-480 h hourly
/// (a zero rate counts as `1 Bq/hr`), each code interpolated where it has
/// points. The run is the 0-500 h one of Fig 2.3a.
///
/// **Results (2026-10-29):** 0.0795 PBq of Cs-137 reaches the environment
/// by 26 h, 0.0797 by 100 h and by 500 h (0.0783 and 0.0785 since
/// `see_through`, the same day)
///
/// **Moved 2026-10-29 by MCCI, by a factor of 200, for a reason that is
/// not MCCI's: 16.97 PBq by 500 h**, 0.064 at 26 h, 0.089 at 100 h;
/// inside the envelope 28.9 % (was 7.3 %). The drywell, with the dry
/// debris radiating into structures that are one lumped, outwardly
/// adiabatic heat sink (`dw_wall_c`, \[A\]), heats without bound in the
/// long run (its gas 558 K at 30 h, 1008 K at 480 h; 1099 K at 480 h
/// without MCCI too), so the 29 kg of cesium on its structures
/// revaporises (CsOH holds about 3 kg airborne at 850 K). MCCI's gas
/// opens the flange leak at about 23 h, and the leak, once open, never
/// closes (\[A\]), so that cesium reaches the building: 16.6 kg between
/// 120 and 260 h. Without MCCI the same revaporised cesium returns to the
/// pool. The defect is the structures' missing outer heat path, the next
/// step; this number is recorded as measured, and is not a result.
///
/// **Moved 2026-10-29 by that outer heat path** (`dw_wall_outer`): **0.0590
/// PBq** by 26 h and by 500 h; inside the envelope 0.4 %. Predicted below
/// 1 PBq and the drywell gas below 600 K at 480 h: **held** (420 K, a
/// plateau from about 60 h). The flange leak no longer opens, and nothing
/// leaves after the vent's first hours. (0.0649 PBq since the core plate,
/// the same day.) (5.1e-4 of the 1.57e17 Bq
/// inventory this share implies): below every code (IRSN 0.1, SNL 2.8,
/// IAE 4.4, CIEMAT 5.6, KAERI 18.9 PBq) and a tenth of GRS's 0.79 PBq. The
/// rate is inside the codes' envelope at 7.3 % of 454 counted hours.
/// Predicted 0.1-1.3 PBq and 20-60 %: **both missed**, low. Nearly all
/// of it leaves in the vent's first hours; after that the wetwell pool
/// holds the cesium (97 % of it is there at 26 h) and nothing more
/// leaves, where the codes go on releasing for days: the reactor
/// building's leaks, the late-phase leakage F12 has not built, and
/// revaporisation carried out of the vessel by flows through it (here
/// revaporisation is modelled, `vapour::condense`, but the 19.4 kg on the
/// 812 K internals leaves only as fast as gas leaves the failed vessel:
/// CsOH at about 17 Pa, about 0.1 kg in its 292 m^3). A missed match
/// recorded, not tuned.
#[test]
fn cesium_137_release_against_fig_5_1() {
    const SHARE: f64 = 0.316_659;
    const BQ_PER_KG: f64 = 3.212e15;
    let prm = Params::default();
    let m = histories(Mechanisms::default(), prm, 600.0, 500.0 * 3600.0);
    let bq: Vec<f64> = m.cs_released.iter().map(|f| f * prm.cs_inventory_kg * SHARE * BQ_PER_KG).collect();
    let n = bq.len();
    let rate: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
            ((bq[b] - bq[a]) / (m.t_h[b] - m.t_h[a])).max(1.0).log10()
        })
        .collect();
    let codes: Vec<Vec<(f64, f64)>> = ["ciemat-melcor", "iae-sampson", "irsn-astec", "kaeri-melcor", "snl-melcor"]
        .iter()
        .map(|c| read_release(c).into_iter().map(|(t, r)| (t, r.max(1.0).log10())).collect())
        .collect();
    let grid: Vec<f64> = (24..=480).map(|h| h as f64).collect();
    let env = inside_envelope(&m.t_h, &rate, &codes, &grid);
    let total_pbq = bq[n - 1] / 1e15;
    let at = |h: f64| bq[m.t_h.partition_point(|&t| t < h).min(n - 1)] / 1e15;
    println!("Cs-137 to the environment: {:.4} PBq at 26 h, {:.4} PBq at 100 h, {total_pbq:.4} PBq at 500 h; rate inside the Fig 5.1 envelope {env:?}", at(26.0), at(100.0));
    // Pinned to the first run (2026-10-29); a model change moves these and
    // records the move here.
    // 0.0649 -> 0.0778 PBq with the bottom of the core's radiation, ->
    // 0.0599 with a plate per ring, the rate inside the envelope 0.0044 ->
    // 0.0000; -> 0.1543 PBq (0.1542 at 26 h) with the pool in two layers,
    // the flange open from about 17.5 h: inside the codes' 0.1-18.9 PBq
    // for the first time, the rate inside the envelope 0.0110
    // (2026-10-29).
    // -> 0.1894 PBq (0.1893 at 26 h) with the plate's debris oxidising;
    // -> 0.1848 (0.1847) with the plenum's oxidation steam consumed; ->
    // 0.0740, the rate inside the envelope 0.0022, with the control blades;
    // -> 0.0801, 0.0044, with their heat of fusion; -> 0.1071, 0.0088, with
    // the cavity debris's phases (the flange open from 16.7 h); -> 0.0977,
    // 0.0066, with the vent and flange choked; -> 0.0755 with the two-sided
    // equalising; -> 0.0956 with the blades' B4C; -> 0.0748, the rate
    // inside the envelope 0.0044, with UO2 carried by candling Zr; 0.0748,
    // the rate inside 0.0044 -> 0.0066, with the fresh water from 15:00 h
    // (F13, the vented flow differs by a few grid hours).
    assert!((total_pbq - 0.0748).abs() < 1e-3 && (at(26.0) - 0.0748).abs() < 1e-3, "{total_pbq}");
    assert!((env.0 - 0.0066).abs() < 1e-3 && env.1 == 454, "{env:?}");
}
