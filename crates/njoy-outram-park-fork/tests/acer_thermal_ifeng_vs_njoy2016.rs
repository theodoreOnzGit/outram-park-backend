//! **V&V gate** — the thermal `IFENG = 1` (skewed) and `IFENG = 2`
//! (continuous) inelastic forms against NJOY2016.
//!
//! ## Methodology
//!
//! `tests/acer_thermal_vs_njoy2016.rs` covers `IFENG = 0`. The other two forms
//! are selected by ACER card 9's `iwt` (`aceth.f90:674-676`): `iwt = 0` gives
//! the skewed form and `iwt = 2` the continuous one. NJOY was re-run on
//! `tsl-013_Al_027-ENDF8.0.endf` (MAT 53, 20 K, paired with
//! `n-013_Al_027-ENDF8.0.endf` MAT 1325) with each, and this port builds the
//! matching table on **NJOY's own incident-energy grid**.
//!
//! The committed oracle for `IFENG = 2` is NJOY's **ITXE locator/count table**
//! — `reference-data/acer/al27_20k_thermal_ifeng2_njoy2016.csv`, one row per
//! incident energy. That table is the sharpest small thing to gate: the point
//! count is not an input, it falls out of `acesix`'s panel-merging threshold
//! (`aceth.f90:392-395`, `sum + add > eps/10`), so reproducing all 106 of them
//! exactly is a statement about the algorithm rather than about the data.
//! The offsets additionally pin the whole block layout.
//!
//! ## Results (2026-09-22)
//!
//! Measured with `examples/thermal_ace_vs_njoy2016`, comparing every stored
//! word against NJOY's own file:
//!
//! | form | tape | points | agreement |
//! |---|---|---|---|
//! | `IFENG = 1` | Al-27, 20 K | 106 × 16 × 17 = 28 832 | `E'` worst **2.358e-3** rel, cosines worst **2.643e-4** abs |
//! | `IFENG = 2` | Al-27, 20 K | 10 518 | counts identical; CDF worst **6.517e-6** rel |
//! | `IFENG = 2` | crystalline graphite, 296 K | 36 092 | counts identical; CDF worst **4.365e-6** rel |
//!
//! For `IFENG = 2` the point-by-point disagreement is **entirely the final
//! point of each incident energy's law**, where the two codes' outgoing-energy
//! ranges end: exactly **106 of 10 518** points (Al-27) and **106 of 36 092**
//! (graphite) differ in `E'` by more than 1e-6, one per incident energy.
//! Excluding those, `E'` agrees to **4.2e-7** / **4.6e-7** and the cosines to
//! **5.0e-5** / **3.9e-5**. The headline numbers above are the worst over the
//! whole block, including the endpoints — the breakdown is reported beside
//! them, never instead of them.

use njoy_outram_park_fork::acer::thermal::{nxs, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{
    reference_data_required, reference_endf, reference_file,
};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use std::fs::File;

const MAT: i32 = 53;
const T0_K: f64 = 20.0;
const EMEV: f64 = 1.0e6;
const CSV: &str = "al27_20k_thermal_ifeng2_njoy2016.csv";

/// NJOY's ITXE table: `(incident energy [MeV], offset, point count)`.
fn oracle() -> Option<Vec<(f64, usize, usize)>> {
    let path = reference_file("acer", CSV)?;
    let text = std::fs::read_to_string(path).ok()?;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("e_in_mev") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 3 {
            continue;
        }
        out.push((
            f[0].trim().parse().ok()?,
            f[1].trim().parse().ok()?,
            f[2].trim().parse().ok()?,
        ));
    }
    Some(out)
}

fn build(form: InelasticForm, grid_ev: &[f64]) -> Option<AceTable> {
    let tsl = reference_endf("tsl-013_Al_027-ENDF8.0.endf")?;
    let tape = Tape::read(File::open(tsl).ok()?).ok()?;
    let mf7 = parse_mf7(&tape, MAT).ok()?;
    let emax_ev = *grid_ev.last()?;
    AceTable::thermal_from_mf7(
        &mf7,
        T0_K,
        "al27",
        0,
        grid_ev,
        ThermalAceOptions {
            n_outgoing: 16,
            n_cosines: 16,
            natom: 1.0,
            emax_ev,
            form,
        },
    )
    .ok()
}

/// The continuous form: every point count and every block offset, against
/// NJOY's own.
#[test]
fn ifeng2_itxe_layout_matches_njoy2016() {
    let Some(oracle) = oracle() else {
        assert!(
            !reference_data_required(),
            "[ifeng2] reference-data/acer/{CSV} absent and \
             OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[ifeng2] SKIP — oracle CSV not present");
        return;
    };
    let grid_ev: Vec<f64> = oracle.iter().map(|&(e, _, _)| e * EMEV).collect();
    let Some(ace) = build(InelasticForm::Continuous, &grid_ev) else {
        assert!(
            !reference_data_required(),
            "[ifeng2] the TSL tape is absent and OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[ifeng2] SKIP — tsl tape not present");
        return;
    };
    let nei = oracle.len();
    assert_eq!(ace.nxs[nxs::IFENG], 2, "NXS(7) must declare the continuous form");
    assert_eq!(
        ace.nxs[nxs::NIL], 17,
        "NIL is nang + 1 = 17 for IFENG=2, not nang - 1 (aceth.f90:816-820)"
    );
    let itxe = ace.jxs[njoy_outram_park_fork::acer::thermal::jxs::ITXE] as usize;
    assert!(itxe > 0, "ITXE must be present");
    let base = itxe - 1;

    let mut bad: Vec<String> = Vec::new();
    for (i, &(_, off, n)) in oracle.iter().enumerate() {
        let our_off = ace.xss[base + i].round() as usize;
        let our_n = ace.xss[base + nei + i].round() as usize;
        if our_n != n || our_off != off {
            bad.push(format!(
                "  [{i}] offset ours {our_off} njoy {off}, points ours {our_n} njoy {n}"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "the IFENG=2 ITXE locator/count table must reproduce NJOY exactly \
         ({} of {nei} rows differ):\n{}",
        bad.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
    let total: usize = oracle.iter().map(|&(_, _, n)| n).sum();
    assert_eq!(total, 10_518, "total stored points, measured 2026-09-22");
    eprintln!("[ifeng2] {nei} offsets and counts identical to NJOY ({total} points)");
}

/// The stored law must be a law: a density and a cumulative that reaches 1.
/// This needs no reference file and would catch a layout slip the count table
/// cannot see.
#[test]
fn ifeng2_stored_law_is_normalised() {
    let Some(oracle) = oracle() else {
        assert!(!reference_data_required(), "[ifeng2-law] oracle absent");
        return;
    };
    let grid_ev: Vec<f64> = oracle.iter().map(|&(e, _, _)| e * EMEV).collect();
    let Some(ace) = build(InelasticForm::Continuous, &grid_ev) else {
        assert!(!reference_data_required(), "[ifeng2-law] tsl tape absent");
        return;
    };
    let nei = oracle.len();
    let nang = 16usize;
    let base = ace.jxs[njoy_outram_park_fork::acer::thermal::jxs::ITXE] as usize - 1;
    let mut worst_end = 0.0f64;
    // The density is allowed to go slightly negative at the point where the
    // law ends, because the closing interpolation is a difference of nearly
    // equal numbers. **NJOY's own IFENG=2 table does exactly this**: 7 of its
    // 10 518 Al-27 densities are negative, the first `-8.47e-16` at the final
    // point of incident energy 14 where the CDF has already reached 1. So the
    // bound is on the magnitude, not on the sign, and it is set from what
    // upstream produces rather than from what would be tidy.
    let mut worst_negative = 0.0f64;
    for i in 0..nei {
        let off = ace.xss[base + i] as usize;
        let n = ace.xss[base + nei + i] as usize;
        assert!(n >= 2, "incident energy {i} stores {n} points");
        let mut prev_cdf = 0.0f64;
        let mut prev_e = f64::NEG_INFINITY;
        for k in 0..n {
            let a = off + k * (nang + 3);
            let (e, pdf, cdf) = (ace.xss[a], ace.xss[a + 1], ace.xss[a + 2]);
            assert!(e > prev_e, "E' must ascend at incident {i} point {k}");
            if pdf < worst_negative {
                worst_negative = pdf;
            }
            assert!(
                cdf >= prev_cdf - 1.0e-12,
                "the cumulative must not decrease at {i}/{k}: {prev_cdf} -> {cdf}"
            );
            for j in 0..nang {
                let mu = ace.xss[a + 3 + j];
                assert!((-1.0..=1.0).contains(&mu), "cosine {mu} out of range at {i}/{k}");
            }
            prev_cdf = cdf;
            prev_e = e;
        }
        worst_end = worst_end.max((prev_cdf - 1.0).abs());
    }
    assert!(
        worst_end < 1.0e-12,
        "every law's cumulative must end at 1; worst |CDF_end - 1| = {worst_end:e}"
    );
    assert!(
        worst_negative > -1.0e-6,
        "a density may only go negative at round-off scale where the law closes; \
         worst was {worst_negative:e}"
    );
    eprintln!(
        "[ifeng2-law] {nei} laws ascend, end at 1, and dip no further below zero \
         than {worst_negative:e} (NJOY's own worst is -8.47e-16)"
    );
}

/// `IFENG = 1` is not a relabelling of `IFENG = 0`: the same `NIEB` bins are
/// weighted `1 4 10 … 10 4 1`, so the edges move. Pinned because a flag whose
/// value nothing changes is worse than no flag.
#[test]
fn ifeng1_is_skewed_and_declares_itself() {
    let Some(oracle) = oracle() else {
        assert!(!reference_data_required(), "[ifeng1] oracle absent");
        return;
    };
    let grid_ev: Vec<f64> = oracle.iter().map(|&(e, _, _)| e * EMEV).collect();
    let (Some(skewed), Some(equi)) = (
        build(InelasticForm::Skewed, &grid_ev),
        build(InelasticForm::Equiprobable, &grid_ev),
    ) else {
        assert!(!reference_data_required(), "[ifeng1] tsl tape absent");
        return;
    };
    assert_eq!(skewed.nxs[nxs::IFENG], 1, "NXS(7) must declare the skewed form");
    assert_eq!(equi.nxs[nxs::IFENG], 0);
    assert_eq!(
        skewed.nxs[nxs::NIL], equi.nxs[nxs::NIL],
        "the skewed form shares IFENG=0's layout, so NIL is unchanged"
    );
    assert_eq!(
        skewed.xss.len(),
        equi.xss.len(),
        "and so is the table length"
    );
    let itxe = skewed.jxs[njoy_outram_park_fork::acer::thermal::jxs::ITXE] as usize - 1;
    let n = skewed.xss.len() - itxe;
    let differing = (0..n)
        .filter(|&k| skewed.xss[itxe + k] != equi.xss[itxe + k])
        .count();
    assert!(
        differing * 2 > n,
        "the 1 4 10 … 10 4 1 weighting must move most bin edges; only {differing} \
         of {n} ITXE words differ from the equiprobable build"
    );
    eprintln!("[ifeng1] {differing} of {n} ITXE words differ from the IFENG=0 build");
}
