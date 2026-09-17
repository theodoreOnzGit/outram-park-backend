//! **Band-averaged cross sections against OpenMC's, in the bands `op-os8x`
//! lives in — the quantity that actually sets the flux there.**
//!
//! # Why the cross sections are worth re-examining after being "excluded"
//!
//! The cross-code study excluded them on two numbers: the **flux-weighted**
//! difference over the whole range (≤0.06 %) and the worst **pointwise**
//! difference. Neither can exclude a band-local difference:
//!
//! - A flux-weighted average over the whole spectrum is a **mean**, and a mean
//!   cannot see a redistribution. That is the same blind spot that hid χ's
//!   shape behind χ's mean until it was measured band by band, and it is the
//!   third time in this study a mean has hidden something.
//! - The worst pointwise difference is dominated by grid alignment inside
//!   resonances — the study says so itself and deliberately does not chase it —
//!   so it cannot exclude anything either.
//!
//! What sets the flux in a band is the **band-averaged** cross section. A 1 %
//! difference in mean `σ_t` across 67–174 keV produces roughly a 1 % flux
//! difference there, which is the size of the residual (1.2–1.9 %).
//!
//! # Methodology
//!
//! Lethargy-averaged microscopic cross sections,
//!
//! ```text
//! <sigma>_band = int sigma(E) dE/E  /  int dE/E
//! ```
//!
//! per band, for U-235 and U-238, on elastic, fission, capture and inelastic
//! (the channels the transport kernel partitions on) plus their sum. The
//! `dE/E` weight is deliberate: it needs **no flux**, so the comparison cannot
//! be contaminated by the very flux difference it is meant to explain.
//!
//! Oracle: OpenMC 0.15.3's own tabulated cross sections from `work/U*.h5`,
//! integrated on the same log grid
//! (`verification_and_validation/openmc_godiva_cross_code/band_xs_oracle.py`,
//! regenerable).
//!
//! # Results
//!
//! Printed by the test.
//!
//! # Reading the outcome
//!
//! - **A band-local difference of the right sign and size** — ours higher in
//!   67–174 keV, or lower in 1.9–3.0 MeV — would make the cross sections the
//!   explanation after all, and the "excluded" verdict an artefact of averaging.
//! - **Agreement** excludes them properly this time, band by band rather than
//!   on a mean.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
const NSUB: usize = 4000;

/// The bands, matching `band_xs_oracle.py`'s `BANDS` exactly.
const BANDS: &[(f64, f64)] = &[
    (2.0e4, 6.7e4),
    (6.7e4, 1.74e5),
    (1.74e5, 6.0e5),
    (6.0e5, 1.9e6),
    (1.9e6, 3.0e6),
    (3.0e6, 6.0e6),
];

/// OpenMC band averages `[elastic, fission, capture, inelastic, sum]` per band,
/// from `band_xs_oracle.py`.
const OPENMC_U235: &[[f64; 5]] = &[
    [11.017651, 1.987839, 0.647687, 0.029321, 13.682498],
    [9.622001, 1.543524, 0.431340, 0.376125, 11.972990],
    [6.888111, 1.237761, 0.218033, 1.190966, 9.534871],
    [3.986001, 1.187215, 0.103090, 1.793700, 7.070005],
    [4.072335, 1.263977, 0.045174, 2.191125, 7.572610],
    [4.405270, 1.123938, 0.011639, 2.237270, 7.778117],
];
const OPENMC_U238: &[[f64; 5]] = &[
    [13.053376, 0.000078, 0.390016, 0.040916, 13.484387],
    [11.019992, 0.000079, 0.174228, 0.578911, 11.773210],
    [8.018997, 0.000238, 0.114975, 1.390207, 9.524417],
    [4.311844, 0.129384, 0.100363, 2.684440, 7.226032],
    [3.842433, 0.542298, 0.033442, 3.138706, 7.556880],
    [4.341587, 0.552582, 0.006346, 2.884852, 7.785367],
];

const DEFICIT_BAND: usize = 1; // 67-174 keV
const EXCESS_BAND: usize = 4; // 1.9-3.0 MeV

/// Lethargy average of `f` over `[lo, hi]` — trapezoid in `ln E`, which is
/// exactly the `dE/E` weight.
fn band_average(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let (l0, l1) = (lo.ln(), hi.ln());
    let mut acc = 0.0;
    let mut prev = f(lo);
    for i in 1..NSUB {
        let ln = l0 + (l1 - l0) * i as f64 / (NSUB - 1) as f64;
        let v = f(ln.exp());
        acc += 0.5 * (v + prev) * (l1 - l0) / (NSUB - 1) as f64;
        prev = v;
    }
    acc / (l1 - l0)
}

fn check(file: &str, name: &str, oracle: &[[f64; 5]]) -> Option<(f64, f64, f64)> {
    let tape = reference_file_or_skip("endf", file, "band-averaged xs")?;
    let nuc = Nuclide::from_endf_file(&tape, name, TEMP_K, 1.0e-3).ok()?;

    println!("\n{name}: band-averaged sigma (barn), ours vs OpenMC");
    println!(
        "{:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>9}",
        "band lo", "band hi", "channel", "ours", "OpenMC", "diff %", "note"
    );
    let labels = ["elastic", "fission", "capture", "inelastic", "sum"];
    let mut worst = 0.0f64;
    let (mut deficit_sum, mut excess_sum) = (0.0f64, 0.0f64);

    for (b, &(lo, hi)) in BANDS.iter().enumerate() {
        let el = band_average(lo, hi, |e| nuc.xs_at_energy(e, TEMP_K).elastic);
        let fi = band_average(lo, hi, |e| nuc.xs_at_energy(e, TEMP_K).fission);
        let ca = band_average(lo, hi, |e| {
            let x = nuc.xs_at_energy(e, TEMP_K);
            (x.absorption - x.fission).max(0.0)
        });
        let inl = band_average(lo, hi, |e| nuc.xs_at_energy(e, TEMP_K).inelastic);
        let ours = [el, fi, ca, inl, el + fi + ca + inl];
        for (k, label) in labels.iter().enumerate() {
            let theirs = oracle[b][k];
            if theirs < 1.0e-6 {
                continue; // a channel that is closed here says nothing
            }
            let rel = (ours[k] - theirs) / theirs * 100.0;
            if *label == "sum" {
                worst = worst.max(rel.abs());
                if b == DEFICIT_BAND {
                    deficit_sum = rel;
                }
                if b == EXCESS_BAND {
                    excess_sum = rel;
                }
            }
            let note = if b == DEFICIT_BAND {
                "DEFICIT"
            } else if b == EXCESS_BAND {
                "EXCESS"
            } else {
                ""
            };
            println!(
                "{lo:10.3e} {hi:10.3e} {label:>10} {:10.6} {theirs:10.6} {rel:+10.4} {note:>9}",
                ours[k]
            );
        }
    }
    Some((worst, deficit_sum, excess_sum))
}

#[test]
fn band_averaged_cross_sections_match_openmc() {
    let mut ran = false;
    let mut worst_overall = 0.0f64;
    let mut notes = Vec::new();

    for (file, name, oracle) in [
        ("n-092_U_235-ENDF8.0.endf", "U235", OPENMC_U235),
        ("n-092_U_238.endf", "U238", OPENMC_U238),
    ] {
        let Some((worst, deficit, excess)) = check(file, name, oracle) else {
            continue;
        };
        ran = true;
        worst_overall = worst_overall.max(worst);
        notes.push(format!(
            "{name}: sum in 67-174 keV {deficit:+.4} %, in 1.9-3.0 MeV {excess:+.4} %"
        ));
    }
    // A SKIP MUST NOT READ AS A PASS. The first run of this test named U-238's
    // tape `n-092_U_238-ENDF8.0.endf`, which does not exist -- the file is
    // `n-092_U_238.endf` -- so half the comparison silently vanished and the
    // test still passed. U-238 is the half that matters here: it carries the
    // resonances in the deficit band.
    if notes.len() < 2 {
        assert!(
            !ran,
            "only {} of 2 nuclides were compared ({notes:?}). A missing tape must not read as \
             agreement -- that is exactly how the first run of this test passed while skipping \
             U-238 entirely.",
            notes.len()
        );
        eprintln!("SKIP: no evaluations available");
        return;
    }

    println!("\n--- the two bands op-os8x lives in ---");
    for n in &notes {
        println!("   {n}");
    }
    println!("   worst band-summed difference = {worst_overall:.4} %");
    println!(
        "   for reference, the flux residual is -1.2..-1.9 % at 67-174 keV and +0.42 % at \
         1.9-3.0 MeV"
    );

    // 0.5 % on a band-summed cross section. The flux residual is 1.2-1.9 %, so a
    // cross-section difference large enough to cause it would be well above
    // this; a difference below it cannot be the explanation.
    assert!(
        worst_overall < 0.5,
        "band-averaged cross sections differ from OpenMC's by {worst_overall:.4} %. If the sign \
         lines up with op-os8x -- ours higher in 67-174 keV or lower in 1.9-3.0 MeV -- then the \
         cross sections ARE the residual, and the earlier 'excluded' verdict was an artefact of \
         flux-weighting over the whole range, which is a mean and cannot see a band-local \
         difference."
    );
}
