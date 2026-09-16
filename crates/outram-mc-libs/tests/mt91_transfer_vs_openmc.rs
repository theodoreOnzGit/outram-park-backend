//! **The MT=91 continuum transfer law `f₀(E→E')`, compared against OpenMC's own,
//! in the energy band where `op-os8x` lives.**
//!
//! # The question this answers
//!
//! `op-os8x` is a spectral residual against OpenMC that survives every angular
//! fix: our Godiva flux is harder than OpenMC's in mean `E` while `k` agrees to
//! `−32 ± 34 pcm`. It was localised per-bin on 2026-09-16 to **+0.88 % excess
//! flux at 1.9–3.0 MeV** (4.7σ, 13.8 % of the flux) against **1.4–1.9 %
//! deficits at 67–174 keV** — a deficit of down-scatter *out* of the MeV window.
//!
//! Only inelastic scattering moves a 2 MeV neutron to ~100 keV in one collision;
//! elastic off U-238 loses at most 1.7 % per collision. The angular laws are
//! excluded (both `op-tm9f` and `op-og56` are in, and the continuum one's
//! ablation does not move the spectrum), and so are the cross sections
//! (≤0.06 % flux-weighted). That left **the MT=91 continuum `f₀(E→E')` shape**
//! as the leading suspect, recorded as such in
//! `verification_and_validation/openmc_godiva_cross_code/README.md`.
//!
//! # Why this comparison, and not the per-MT collision tally first proposed
//!
//! The record named "a per-MT collision tally in the 1.9–3.0 MeV band on both
//! sides" as the discriminating measurement. On working it through, **that
//! measurement cannot discriminate**: a collision rate is flux × σ, the cross
//! sections already agree to ≤0.06 % flux-weighted, so a rate comparison would
//! largely restate the flux difference it is supposed to explain.
//!
//! What discriminates is the **transfer itself** — *where* an MT=91 collision at
//! 2–3 MeV puts the neutron. If our law sends neutrons to a higher outgoing
//! energy than OpenMC's, that is the missing down-scatter, and it is the cause.
//! If the two laws agree, the `f₀` shape is **excluded** and the suspect list
//! must change. Either way it is a data-level comparison needing no transport,
//! no seeds and no statistics.
//!
//! # Provenance — both sides trace to one evaluation
//!
//! - **Ours:** `reference-data/endf/n-092_U_238.endf`, MF=6/MT=91, read by
//!   `njoy_outram_park_fork::nuclear_data::secondary::ContinuumEmission`.
//! - **OpenMC's:** the same tape → NJOY2016 (`ac5adf5f`) → ACE → HDF5 → OpenMC
//!   0.15.3 (`27e38e89`), built and run in-session. Extracted by
//!   `verification_and_validation/openmc_godiva_cross_code/mt91_transfer_oracle.py`
//!   into the committed `mt91_transfer_oracle.csv`.
//!
//! Because both derive from the same evaluation, **any difference is a port
//! defect, not a data difference.** That is what makes this sharp.
//!
//! Both sides store this law in the **centre-of-mass frame** (ENDF `LCT=2`;
//! OpenMC reports `reaction.center_of_mass == True`), so the comparison is
//! frame-consistent with no transform on either side.
//!
//! # Methodology
//!
//! For each incident-energy row of OpenMC's law in `[1.5, 3.5]` MeV, find our
//! law's row at the same incident energy and compare three functionals of the
//! outgoing pdf, all by trapezoid on each side's own grid:
//!
//! - **mean `⟨E'⟩`** — the first moment, which is what sets how far a collision
//!   moves a neutron;
//! - **median** — shape, insensitive to the tail;
//! - **`P(E' < 300 keV)`** — chosen because 300 keV is the boundary of the flux
//!   deficit `op-os8x` localises, so it is the functional most directly tied to
//!   the residual.
//!
//! # Results — see the test's own output for the measured numbers
//!
//! Recorded in this file's assertions and printed on every run.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

/// OpenMC's law, extracted by `mt91_transfer_oracle.py`. Columns:
/// `e_in_ev, mean_eout_ev, median_eout_ev, frac_below_300kev`.
const OPENMC_MT91: &[(f64, f64, f64, f64)] = &[
    (1.500000e6, 1.333520e5, 1.355220e5, 0.988766),
    (1.540000e6, 1.579531e5, 1.622010e5, 0.990431),
    (1.600000e6, 1.941962e5, 2.003856e5, 0.952021),
    (1.640000e6, 2.174933e5, 2.259463e5, 0.794565),
    (1.700000e6, 2.468090e5, 2.560282e5, 0.647259),
    (1.800000e6, 3.031514e5, 3.124222e5, 0.469568),
    (1.900000e6, 3.514001e5, 3.601625e5, 0.381126),
    (1.945000e6, 3.732413e5, 3.805995e5, 0.352115),
    (2.100000e6, 4.399006e5, 4.428208e5, 0.285614),
    (2.170000e6, 4.701146e5, 4.693651e5, 0.264602),
    (2.240000e6, 4.980344e5, 4.936324e5, 0.247352),
    (2.400000e6, 5.551641e5, 5.401300e5, 0.219791),
    (2.500000e6, 5.848456e5, 5.642812e5, 0.207762),
    (2.575000e6, 6.078981e5, 5.816885e5, 0.200266),
    (2.760000e6, 6.505933e5, 6.168931e5, 0.185640),
    (3.000000e6, 7.095720e5, 6.581210e5, 0.169033),
    (3.100000e6, 7.288561e5, 6.792109e5, 0.165530),
    (3.400000e6, 7.880623e5, 7.119718e5, 0.155181),
];

/// Trapezoidal mean, median and `P(E' < 300 keV)` of one tabulated outgoing-energy
/// row — the same three functionals, computed the same way, as the Python oracle.
fn moments(e_out: &[f64], pdf: &[f64]) -> Option<(f64, f64, f64)> {
    if e_out.len() < 2 {
        return None;
    }
    let mut norm = 0.0;
    let mut first = 0.0;
    let mut cum = vec![0.0f64; e_out.len()];
    for w in 0..e_out.len() - 1 {
        let dx = e_out[w + 1] - e_out[w];
        let seg = 0.5 * dx * (pdf[w] + pdf[w + 1]);
        norm += seg;
        first += 0.5 * dx * (e_out[w] * pdf[w] + e_out[w + 1] * pdf[w + 1]);
        cum[w + 1] = norm;
    }
    if !(norm > 0.0) {
        return None;
    }
    for c in cum.iter_mut() {
        *c /= norm;
    }
    let interp = |target: f64, xs: &[f64], ys: &[f64]| -> f64 {
        // ys ascending; return the x where ys crosses `target`.
        match ys.iter().position(|&y| y >= target) {
            None => xs[xs.len() - 1],
            Some(0) => xs[0],
            Some(i) => {
                let (y0, y1) = (ys[i - 1], ys[i]);
                let (x0, x1) = (xs[i - 1], xs[i]);
                if (y1 - y0).abs() > 0.0 {
                    x0 + (x1 - x0) * (target - y0) / (y1 - y0)
                } else {
                    x0
                }
            }
        }
    };
    let median = interp(0.5, e_out, &cum);
    // P(E' < 300 keV): interpolate the cdf AT 300 keV.
    let below = {
        let t = 3.0e5;
        match e_out.iter().position(|&x| x >= t) {
            None => 1.0,
            Some(0) => 0.0,
            Some(i) => {
                let (x0, x1) = (e_out[i - 1], e_out[i]);
                let (c0, c1) = (cum[i - 1], cum[i]);
                c0 + (c1 - c0) * (t - x0) / (x1 - x0)
            }
        }
    };
    Some((first / norm, median, below))
}

/// Our MT=91 `f₀(E→E')` against OpenMC's, on the same evaluation, in the band
/// where `op-os8x`'s excess flux sits.
///
/// **This test answers a question rather than guarding a tolerance**, so its
/// gates are sized to the agreement actually observed and are stated in the
/// failure messages. A regression that moves either side will trip them.
#[test]
fn mt91_transfer_law_agrees_with_openmc_in_the_op_os8x_band() {
    let Some(tape_path) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (MT=91 transfer vs OpenMC)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape_path, "U238", TEMP_K, 1.0e-3).expect("U-238");
    let law = nuc
        .continuum_law(91)
        .expect("U-238 carries an MF=6 LAW=1 MT=91 emission law");
    assert!(
        law.cm_frame,
        "our MT=91 law reports LAB frame; OpenMC reports centre-of-mass for this reaction, so \
         the comparison below would be between different frames."
    );
    assert_eq!(
        law.branches.len(),
        1,
        "U-238 MT=91 has {} neutron subsections; this comparison assumes the single-branch \
         layout OpenMC collapses to.",
        law.branches.len()
    );
    let chi = &law.branches[0].spectrum;

    println!(
        "U-238 MT=91 f0(E->E'), ours vs OpenMC on the same ENDF/B-VIII.0 tape (CM frame, {} of \
         our incident rows):",
        chi.incident.len()
    );
    println!(
        "   E_in [eV]      <E'> ours      <E'> OpenMC    d%      med ours      med OpenMC   d%   \
          P(<300k) ours  OpenMC    d(abs)"
    );

    let (mut worst_mean, mut worst_med, mut worst_below) = (0.0f64, 0.0f64, 0.0f64);
    let mut compared = 0usize;
    let mut signed_mean_sum = 0.0f64;

    for &(e_ref, mean_ref, med_ref, below_ref) in OPENMC_MT91 {
        // Match on incident energy; both grids come from the same MF=6, so an
        // exact-to-rounding match is expected. A row we cannot match is
        // reported, not silently skipped -- a silent skip could empty the test.
        let Some(i) = chi
            .incident
            .iter()
            .position(|&x| (x - e_ref).abs() <= 1.0e-3 * e_ref)
        else {
            println!("   {e_ref:.6e}  -- no matching incident row on our side");
            continue;
        };
        let row = &chi.tables[i];
        let Some((mean, med, below)) = moments(&row.e_out, &row.pdf) else {
            println!("   {e_ref:.6e}  -- our row has a non-normalisable pdf");
            continue;
        };
        let d_mean = 100.0 * (mean / mean_ref - 1.0);
        let d_med = 100.0 * (med / med_ref - 1.0);
        let d_below = below - below_ref;
        println!(
            "   {e_ref:.6e}  {mean:.6e}  {mean_ref:.6e}  {d_mean:+6.3}  {med:.6e}  \
             {med_ref:.6e}  {d_med:+6.3}   {below:.6}      {below_ref:.6}  {d_below:+.6}"
        );
        worst_mean = worst_mean.max(d_mean.abs());
        worst_med = worst_med.max(d_med.abs());
        worst_below = worst_below.max(d_below.abs());
        signed_mean_sum += d_mean;
        compared += 1;
    }

    assert!(
        compared >= 15,
        "only {compared} of {} OpenMC rows matched an incident row on our side. The two grids \
         should coincide -- both come from the same MF=6 section -- so this means one side's \
         incident grid is being rebuilt, and the comparison is not the one intended.",
        OPENMC_MT91.len()
    );

    let bias = signed_mean_sum / compared as f64;
    println!(
        "\n  {compared} rows compared. Worst |d| : mean {worst_mean:.4} %, median \
         {worst_med:.4} %, P(E'<300keV) {worst_below:.6} absolute. Mean-of-signed-deviation \
         (a systematic bias would show here, a scatter would not): {bias:+.4} %."
    );

    // Gates sized to the observed agreement, not chosen in advance. If our law
    // carried the op-os8x deficit, the mean would sit systematically HIGH here
    // -- neutrons not moved far enough down -- and `bias` would be positive and
    // of order the residual (~1 %).
    assert!(
        worst_mean < 1.0,
        "worst mean-outgoing-energy disagreement is {worst_mean:.4} %, above the 1 % gate. Our \
         MT=91 transfer law differs from OpenMC's on the same evaluation -- which would make it \
         the cause of op-os8x rather than merely its leading suspect."
    );
    assert!(
        worst_below < 0.01,
        "worst P(E' < 300 keV) disagreement is {worst_below:.6} absolute, above the 0.01 gate. \
         300 keV is where op-os8x's missing flux went, so a disagreement here is the residual."
    );
    assert!(
        bias.abs() < 0.5,
        "the signed mean deviation is {bias:+.4} %, a systematic bias rather than scatter. Our \
         law is moving neutrons {} far on average than OpenMC's.",
        if bias > 0.0 { "less" } else { "more" }
    );
}
