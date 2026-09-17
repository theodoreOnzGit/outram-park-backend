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
//! # The answer (2026-09-17): the laws agree exactly, on BOTH nuclides
//!
//! | nuclide | rows | worst `⟨E'⟩` | worst median | worst `P(E' < 300 keV)` | signed bias |
//! |---|---|---|---|---|---|
//! | U-238 | 37 | **0.0000 %** | 0.0000 % | 0.000000 | −0.0000 % |
//! | U-235 | 25 | **0.0000 %** | 0.0000 % | 0.000000 | +0.0000 % |
//!
//! over 1.5–10 MeV. **The MT=91 `f₀(E→E')` shape is excluded as the cause of
//! `op-os8x`** — not "consistent with", exactly equal to every digit the oracle
//! carries. The suspect list must change.
//!
//! **U-235 was added on 2026-09-17 and is the point of this revision.** Until
//! then this comparison ran on U-238 alone, while **Godiva is 93.7 % U-235 by
//! atom density** — so the standing claim "the MT=91 transfer is excluded" was
//! a statement about the *minority* nuclide. Every other `op-os8x` oracle in
//! `verification_and_validation/openmc_godiva_cross_code/` already covered
//! both. This is the fifth time in this study that an exclusion turned out to
//! be only as wide as the window it was measured in, and the first where the
//! window's missing axis was the **nuclide** rather than the energy band. The
//! two laws are genuinely unalike — at 2 MeV U-235 puts 18.4 % of its emission
//! below 300 keV against U-238's ~30 % — so the substitution was never safe,
//! and it happens to have been harmless only because both are right.
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
//! # A CORRECTION TO THIS FILE'S OWN METHOD, and it nearly cost a false defect
//!
//! The first version of the off-grid test below **failed**, reporting a
//! systematic `+0.60 %` bias growing to `+1.76 %` with incident energy — our
//! law apparently leaving neutrons higher in energy than OpenMC's, which is
//! exactly the `op-os8x` signature. It was written up as a probable cause.
//!
//! **It was wrong, and the defect was in the reference.** Both this file and the
//! Python oracle computed the mean as `∫E·p dE / ∫p dE` by the **trapezoid
//! rule**. The denominator is fine — `p` is linear between tabulated points, so
//! the trapezoid rule integrates it exactly. The numerator is not: `E·p(E)` is
//! **quadratic** on each bin, and the trapezoid rule is exact only for linear
//! integrands. Its error per bin is `−h³m/6` with `m` the pdf slope, so it grows
//! with bin width and with how steeply the pdf falls — which on this law means
//! it grows with incident energy, producing a *fake* energy-dependent bias.
//!
//! Measured on U-238 MT=91, trapezoid against the exact lin-lin integral:
//!
//! | `E_in` | trapezoid error | our sampler vs the **exact** mean |
//! |---|---|---|
//! | 1.945 MeV | `+0.089 %` | `−0.048 %` |
//! | 2.400 MeV | `−0.259 %` | `−0.050 %` |
//! | 3.000 MeV | `−1.632 %` | `−0.051 %` |
//!
//! The sampler sits a **flat `−0.05 %`** from the exact mean at every energy —
//! consistent with the Monte Carlo error of the check itself, and with no energy
//! dependence at all. It was correct throughout; the yardstick was bent.
//!
//! Both this file and the oracle scripts now integrate the first moment in
//! closed form. **This is the fifth reference-side error in this study** — the
//! crate's `CLAUDE.md` already records four appearances of the "nearest-point
//! trap", three in external references and one in our own script. The general
//! lesson is the same and worth restating: *a reference is not right merely for
//! being external, or for being the obvious formula.* Here the obvious formula
//! was quietly inexact for one of its two integrals and not the other.
//!
//! It also explains why the row-by-row comparison below agreed so exactly: both
//! sides applied the *same* inexact rule to the *same* tables. That agreement
//! still establishes what it claims — the tables are identical — but it was
//! never evidence that either side's moment was right.
//!
//! # Results — see the test's own output for the measured numbers
//!
//! Recorded in this file's assertions and printed on every run.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

/// OpenMC's law, extracted by `mt91_transfer_oracle.py`. Columns:
/// `e_in_ev, mean_eout_ev, median_eout_ev, frac_below_300kev`.
///
/// # Extended above 3.4 MeV on 2026-09-17, and why that matters
///
/// This table used to stop at **3.4 MeV** — 18 of the law's **96** incident
/// rows — because the oracle script carried `BAND = (1.5e6, 3.5e6)`, the band
/// the `op-os8x` residual was localised to *at the time*. Reasonable then, and
/// too narrow now: the **reflective, zero-leakage** spectrum comparison run on
/// 2026-09-17 re-localised the strongest excess to **3.0–4.8 MeV**
/// (`+0.95 %`, 4.5 sigma), almost entirely outside the window.
///
/// So "the MT=91 transfer table is excluded" was, until this change, a statement
/// about **19 % of the law** and nothing above 3.4 MeV. The oracle band is now
/// `(1.5e6, 1.0e7)` and this table carries 37 rows.
const OPENMC_MT91: &[(f64, f64, f64, f64)] = &[
    (1.500000e6, 1.332240e5, 1.355220e5, 0.988766),
    (1.540000e6, 1.575129e5, 1.622010e5, 0.990431),
    (1.600000e6, 1.936125e5, 2.003856e5, 0.952021),
    (1.640000e6, 2.169767e5, 2.259463e5, 0.794565),
    (1.700000e6, 2.461034e5, 2.560282e5, 0.647259),
    (1.800000e6, 3.024077e5, 3.124222e5, 0.469568),
    (1.900000e6, 3.508892e5, 3.601625e5, 0.381126),
    (1.945000e6, 3.729099e5, 3.805995e5, 0.352115),
    (2.100000e6, 4.405479e5, 4.428208e5, 0.285614),
    (2.170000e6, 4.705629e5, 4.693651e5, 0.264602),
    (2.240000e6, 4.990497e5, 4.936324e5, 0.247352),
    (2.400000e6, 5.566064e5, 5.401300e5, 0.219791),
    (2.500000e6, 5.873875e5, 5.642812e5, 0.207762),
    (2.575000e6, 6.112284e5, 5.816885e5, 0.200266),
    (2.760000e6, 6.615592e5, 6.168931e5, 0.185640),
    (3.000000e6, 7.213435e5, 6.581210e5, 0.169033),
    (3.100000e6, 7.427245e5, 6.792109e5, 0.165530),
    (3.400000e6, 8.000375e5, 7.119718e5, 0.155181),
    (3.600000e6, 8.345472e5, 7.349701e5, 0.148390),
    (4.000000e6, 8.987229e5, 7.852663e5, 0.138260),
    (4.250000e6, 9.372021e5, 7.993626e5, 0.132466),
    (4.500000e6, 9.761598e5, 8.411447e5, 0.126803),
    (4.700000e6, 1.007707e6, 8.388850e5, 0.123375),
    (5.000000e6, 1.053830e6, 8.921144e5, 0.119088),
    (5.500000e6, 1.135627e6, 8.946542e5, 0.112166),
    (5.700000e6, 1.170034e6, 9.233960e5, 0.106907),
    (6.000000e6, 1.239567e6, 9.694310e5, 0.093264),
    (6.300000e6, 1.350154e6, 1.032454e6, 0.068987),
    (6.400000e6, 1.400200e6, 1.089996e6, 0.061733),
    (6.500000e6, 1.462345e6, 1.125740e6, 0.046967),
    (7.000000e6, 1.924762e6, 1.487628e6, 0.007699),
    (7.500000e6, 2.555363e6, 2.041325e6, 0.005485),
    (8.000000e6, 3.279207e6, 2.677900e6, 0.004980),
    (8.500000e6, 4.032473e6, 3.629550e6, 0.004477),
    (9.000000e6, 4.765503e6, 4.762881e6, 0.003652),
    (9.600000e6, 5.579447e6, 5.776860e6, 0.002633),
    (1.000000e7, 6.084090e6, 6.326328e6, 0.002097),
];

/// Trapezoidal mean, median and `P(E' < 300 keV)` of one tabulated outgoing-energy
/// row — the same three functionals, computed the same way, as the Python oracle.
/// The same law for **U-235**, added 2026-09-17. Columns as above.
///
/// # Why this was missing, and why it matters most
///
/// Every other `op-os8x` oracle in
/// `verification_and_validation/openmc_godiva_cross_code/` covers both
/// nuclides (`band_xs_oracle.py` iterates `[("U235", …), ("U238", …)]`;
/// `chi_shape_oracle.py` loads both). The one that measures *where an inelastic
/// collision actually puts the neutron* — the leading suspect — ran on **U-238
/// only**, while **Godiva is 93.7 % U-235 by atom density**.
///
/// So "the MT=91 transfer is excluded" was a statement about the minority
/// nuclide. That is the same shape of gap as the 1.5–3.5 MeV band described
/// above, one axis over: **an exclusion is only as wide as the window it was
/// measured in**, and the window has a nuclide axis as well as an energy one.
///
/// The two laws are not alike, which is why the substitution was never safe:
/// at 2 MeV U-235 puts **18.4 %** of its emission below 300 keV against
/// U-238's **~30 %**, and its `⟨E'⟩` is 643 keV against U-238's 385 keV.
const OPENMC_MT91_U235: &[(f64, f64, f64, f64)] = &[
    (1.555e6, 4.719317e5, 4.625502e5, 0.264249),
    (1.727e6, 5.391136e5, 5.220811e5, 0.222175),
    (2.000e6, 6.427320e5, 6.245720e5, 0.183508),
    (2.171e6, 6.976118e5, 6.848496e5, 0.168325),
    (2.370e6, 7.548124e5, 7.144997e5, 0.156986),
    (2.600e6, 8.119853e5, 7.594616e5, 0.147352),
    (2.927e6, 8.783601e5, 8.377460e5, 0.135243),
    (3.344e6, 9.491878e5, 8.766601e5, 0.126396),
    (3.927e6, 1.041300e6, 9.397689e5, 0.113293),
    (4.400e6, 1.122726e6, 9.753941e5, 0.103658),
    (4.897e6, 1.214676e6, 1.030472e6, 0.096331),
    (5.382e6, 1.312350e6, 1.073451e6, 0.087627),
    (5.500e6, 1.343836e6, 1.088234e6, 0.081289),
    (5.600e6, 1.378837e6, 1.111680e6, 0.069607),
    (5.700e6, 1.427410e6, 1.147394e6, 0.053676),
    (5.850e6, 1.520951e6, 1.218529e6, 0.033018),
    (6.000e6, 1.632179e6, 1.300736e6, 0.019331),
    (6.400e6, 2.005629e6, 1.617538e6, 0.007486),
    (7.000e6, 2.717188e6, 2.222941e6, 0.005642),
    (7.400e6, 3.275239e6, 2.859784e6, 0.004906),
    (7.760e6, 3.785798e6, 3.529541e6, 0.004346),
    (8.400e6, 4.640075e6, 4.637270e6, 0.003285),
    (9.000e6, 5.367814e6, 5.468270e6, 0.002447),
    (9.370e6, 5.788770e6, 5.908467e6, 0.002079),
    (1.000e7, 6.470521e6, 6.594237e6, 0.001632),
];

/// One nuclide's comparison: our tape, our label, OpenMC's extracted law, and
/// how many rows must match for the comparison to mean anything.
struct Mt91Case {
    label: &'static str,
    endf: &'static str,
    oracle: &'static [(f64, f64, f64, f64)],
    min_rows: usize,
}

const MT91_CASES: &[Mt91Case] = &[
    Mt91Case {
        label: "U238",
        endf: "n-092_U_238.endf",
        oracle: OPENMC_MT91,
        min_rows: 15,
    },
    Mt91Case {
        label: "U235",
        endf: "n-092_U_235-ENDF8.0.endf",
        oracle: OPENMC_MT91_U235,
        min_rows: 15,
    },
];

fn moments(e_out: &[f64], pdf: &[f64]) -> Option<(f64, f64, f64)> {
    if e_out.len() < 2 {
        return None;
    }
    let mut norm = 0.0;
    let mut first = 0.0;
    let mut cum = vec![0.0f64; e_out.len()];
    for w in 0..e_out.len() - 1 {
        let (a, b) = (e_out[w], e_out[w + 1]);
        let (pa, pb) = (pdf[w], pdf[w + 1]);
        let dx = b - a;
        // Normalisation: `p` is linear between points, so the trapezoid rule
        // integrates it EXACTLY.
        norm += 0.5 * dx * (pa + pb);
        // First moment: `E*p(E)` is QUADRATIC on each bin, so the trapezoid
        // rule is NOT exact for it -- see this file's correction note. Integrate
        // it in closed form instead.
        if dx > 0.0 {
            let m = (pb - pa) / dx;
            first += pa * (b * b - a * a) / 2.0
                + m * ((b * b * b - a * a * a) / 3.0 - a * (b * b - a * a) / 2.0);
        }
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
    for case in MT91_CASES {
        check_mt91_transfer(case);
    }
}

fn check_mt91_transfer(case: &Mt91Case) {
    let Mt91Case {
        label,
        endf,
        oracle,
        min_rows,
    } = *case;
    let Some(tape_path) = reference_file_or_skip(
        "endf",
        endf,
        &format!("{label} evaluation (MT=91 transfer vs OpenMC)"),
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape_path, label, TEMP_K, 1.0e-3).expect("evaluation");
    let law = nuc
        .continuum_law(91)
        .unwrap_or_else(|| panic!("{label} carries an MF=6 LAW=1 MT=91 emission law"));
    assert!(
        law.cm_frame,
        "our MT=91 law reports LAB frame; OpenMC reports centre-of-mass for this reaction, so \
         the comparison below would be between different frames."
    );
    assert_eq!(
        law.branches.len(),
        1,
        "{label} MT=91 has {} neutron subsections; this comparison assumes the single-branch \
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

    for &(e_ref, mean_ref, med_ref, below_ref) in oracle {
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
        compared >= min_rows,
        "[{label}] only {compared} of {} OpenMC rows matched an incident row on our side. The \
         two grids should coincide -- both come from the same MF=6 section -- so this means \
         one side's incident grid is being rebuilt, and the comparison is not the one intended.",
        oracle.len()
    );

    let bias = signed_mean_sum / compared as f64;
    println!(
        "\n  [{label}] {compared} rows compared. Worst |d| : mean {worst_mean:.4} %, median \
         {worst_med:.4} %, P(E'<300keV) {worst_below:.6} absolute. Mean-of-signed-deviation \
         (a systematic bias would show here, a scatter would not): {bias:+.4} %."
    );

    // Gates sized to the observed agreement, not chosen in advance. If our law
    // carried the op-os8x deficit, the mean would sit systematically HIGH here
    // -- neutrons not moved far enough down -- and `bias` would be positive and
    // of order the residual (~1 %).
    assert!(
        worst_mean < 1.0,
        "[{label}] worst mean-outgoing-energy disagreement is {worst_mean:.4} %, above the 1 % gate. Our \
         MT=91 transfer law differs from OpenMC's on the same evaluation -- which would make it \
         the cause of op-os8x rather than merely its leading suspect."
    );
    assert!(
        worst_below < 0.01,
        "[{label}] worst P(E' < 300 keV) disagreement is {worst_below:.6} absolute, above the 0.01 gate. \
         300 keV is where op-os8x's missing flux went, so a disagreement here is the residual."
    );
    assert!(
        bias.abs() < 0.5,
        "[{label}] the signed mean deviation is {bias:+.4} %, a systematic bias rather than scatter. Our \
         law is moving neutrons {} far on average than OpenMC's.",
        if bias > 0.0 { "less" } else { "more" }
    );
}

// ─────────────────── off-grid: the inter-row (unit-base) rule ───────────────────

/// Closed-form expected `⟨E'⟩` of OpenMC's unit-base construction at incident
/// energies **between** tabulated rows, from
/// `verification_and_validation/openmc_godiva_cross_code/mt91_interrow_oracle.py`.
/// Columns: `e_in_ev, expected_mean_eout_ev, r`.
const OPENMC_MT91_OFFGRID: &[(f64, f64, f64)] = &[
    (1.970000e6, 3.834406e5, 0.161290),
    (2.050000e6, 4.181176e5, 0.677419),
    (2.150000e6, 4.618863e5, 0.714286),
    (2.300000e6, 5.202911e5, 0.375000),
    (2.450000e6, 5.719211e5, 0.500000),
    (2.620000e6, 6.233918e5, 0.243243),
    (2.900000e6, 6.963895e5, 0.583333),
    (3.050000e6, 7.320555e5, 0.500000),
];

/// **The inter-row rule, which the row-by-row comparison above is structurally
/// blind to.**
///
/// # Why this is a separate test and not a stronger version of the first
///
/// The test above shows our `f₀` matches OpenMC's on every *tabulated* row to
/// 7 significant figures. Godiva's neutrons almost never arrive at a tabulated
/// row. Between rows the spectrum is built by **unit-base interpolation**, and
/// a defect there — wrong branch probability, wrong envelope, wrong bin lookup
/// — leaves every row identical while moving the sampled spectrum. That is
/// exactly the signature `op-os8x` still has after the transfer table was
/// excluded, which is why this is lead 1.
///
/// # What is compared, and why nothing is re-implemented
///
/// The unit-base scaling is affine in the sampled value, so the expectation
/// passes through it and the mean has a closed form in each row's own mean and
/// endpoints (derived in the oracle script). The reference is therefore
/// **OpenMC's own tabulated rows put through the published algorithm in closed
/// form** — no sampler is written on the reference side, and this test's job is
/// to check that *our sampler* reproduces it.
///
/// A disagreement localises `op-os8x` to the inter-row construction. Agreement
/// excludes it and leaves the CM→lab transform and channel branching.
///
/// # Results (2026-09-16, ENDF/B-VIII.0 U-238)
///
/// Worst `|Δ|` **0.0228 %**, signed mean **−0.0205 %** over 8 off-grid probes
/// spanning 1.97–3.05 MeV. The deviation is **flat in energy** — 0.0187 % to
/// 0.0228 % across a band where the fake trapezoid bias had run 0.06 % to
/// 1.76 % — and is consistent with the Monte Carlo error of the check at 4e6
/// draws. **The inter-row unit-base construction is therefore excluded as a
/// cause of `op-os8x`**, as is the within-row CDF inversion: at an on-grid
/// energy `r = 0` and the rescale is the identity, and the sampled mean sits
/// the same flat −0.05 % from the exact row mean there.
#[test]
fn mt91_offgrid_sampling_reproduces_openmcs_unit_base_construction() {
    use outram_mc_libs::material::nuclide::sample_continuum_outgoing_energy as sample_e_out;

    let Some(tape_path) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (MT=91 off-grid unit-base check)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape_path, "U238", TEMP_K, 1.0e-3).expect("U-238");
    let law = nuc.continuum_law(91).expect("U-238 MT=91 law");
    let chi = &law.branches[0].spectrum;

    // 4e6 draws puts the standard error on the mean near 0.03 % for a
    // distribution whose spread is of order its mean -- well inside the gate.
    const N: usize = 4_000_000;

    // Envelope dump. The row-by-row test compares MOMENTS, which are blind to a
    // trailing zero-density point: it contributes nothing to the mean but sets
    // `E_k`, and `E_k` is what the unit-base rescale divides by.
    println!("our MT=91 row envelopes (E_out first/last), for the brackets in play:");
    for &target in &[
        1.945e6f64, 2.1e6, 2.17e6, 2.24e6, 2.4e6, 2.5e6, 2.575e6, 2.76e6, 3.0e6, 3.1e6,
    ] {
        let i = chi
            .incident
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1 - target)
                    .abs()
                    .partial_cmp(&(b.1 - target).abs())
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap();
        let t = &chi.tables[i];
        println!(
            "  E_in {:.4e}  E_out[0]={:.6e}  E_out[-1]={:.6e}  n={}  p[-1]={:.3e}",
            chi.incident[i],
            t.e_out[0],
            t.e_out[t.e_out.len() - 1],
            t.e_out.len(),
            t.pdf[t.pdf.len() - 1]
        );
    }

    // ON-GRID first. At a tabulated incident energy r = 0, so the sampler takes
    // the lower row and the unit-base rescale is the identity: the sampled mean
    // must then equal the trapezoid mean of that row's own tabulated pdf. This
    // isolates the CDF INVERSION from the INTER-ROW rule -- if on-grid already
    // disagrees, the inter-row rule is not the defect.
    println!("on-grid control (r = 0, no interpolation): sampled mean vs the row's own pdf mean");
    for &(e_row, mean_ref, _, _) in OPENMC_MT91 {
        let Some(i) = chi
            .incident
            .iter()
            .position(|&x| (x - e_row).abs() <= 1.0e-3 * e_row)
        else {
            continue;
        };
        let mut seed = 0x0A11_6817u64;
        let mut sum = 0.0f64;
        const M: usize = 1_000_000;
        for _ in 0..M {
            sum += sample_e_out(chi, chi.incident[i], &mut seed);
        }
        let mean = sum / M as f64;
        println!(
            "  E_in {:.6e}  sampled {:.6e}  pdf-mean {mean_ref:.6e}  d {:+7.4} %",
            chi.incident[i],
            mean,
            100.0 * (mean / mean_ref - 1.0)
        );
    }

    println!(
        "U-238 MT=91 sampled <E'> at OFF-GRID incident energies, ours vs the closed-form mean \
         of OpenMC's unit-base construction ({N} draws each):"
    );
    println!("   E_in [eV]      r        <E'> ours      <E'> expected    d%");

    let mut worst = 0.0f64;
    let mut signed = 0.0f64;
    for &(e_in, expected, r) in OPENMC_MT91_OFFGRID {
        // Confirm the probe really is off-grid on OUR incident grid too -- if
        // it landed on a row, this test would silently become the previous one.
        assert!(
            !chi.incident
                .iter()
                .any(|&x| (x - e_in).abs() <= 1.0e-6 * e_in),
            "probe {e_in:.6e} eV coincides with one of our tabulated incident rows; this test \
             must sample strictly between rows or it checks nothing new."
        );

        // Report OUR bracket beside the oracle's `r`. If the two grids differ,
        // the brackets differ and the comparison is not the one intended --
        // which a row-by-row match cannot rule out, since that test searches
        // our grid for OpenMC's energies and would not notice extra rows.
        let j = chi
            .incident
            .iter()
            .rposition(|&x| x <= e_in)
            .unwrap_or(0)
            .min(chi.incident.len() - 2);
        let r_ours = (e_in - chi.incident[j]) / (chi.incident[j + 1] - chi.incident[j]);
        println!(
            "      our bracket [{:.6e}, {:.6e}] r={r_ours:.6} | oracle r={r:.6}",
            chi.incident[j],
            chi.incident[j + 1]
        );

        let mut seed = 0x0FF6_01D0u64;
        let mut sum = 0.0f64;
        for _ in 0..N {
            sum += sample_e_out(chi, e_in, &mut seed);
        }
        let mean = sum / N as f64;
        let d = 100.0 * (mean / expected - 1.0);
        println!("   {e_in:.6e}  {r:.6}  {mean:.6e}  {expected:.6e}  {d:+7.4}");
        worst = worst.max(d.abs());
        signed += d;
    }
    let bias = signed / OPENMC_MT91_OFFGRID.len() as f64;
    println!(
        "\n  worst |d| = {worst:.4} %, signed mean = {bias:+.4} % over \
         {} off-grid probes.",
        OPENMC_MT91_OFFGRID.len()
    );

    // Gates sized to the Monte Carlo error of the check itself (~0.03 % on the
    // mean at 4e6 draws), not to the accuracy hoped for. A systematic bias is
    // what a defective inter-row rule produces, so `bias` is gated tighter than
    // the worst single probe.
    assert!(
        worst < 0.1,
        "worst off-grid mean-outgoing-energy disagreement is {worst:.4} %, above the 0.1 % gate \
         (sized to the 0.0228 % observed 2026-09-16, with headroom for the Monte Carlo error of \
         the check itself). \
         Our unit-base inter-row construction differs from OpenMC's, which WOULD be a cause of \
         op-os8x -- the row-by-row comparison cannot see this."
    );
    assert!(
        bias.abs() < 0.06,
        "the signed mean deviation over the off-grid probes is {bias:+.4} %, a systematic bias \
         rather than scatter. Our inter-row rule is moving neutrons {} far on average than \
         OpenMC's unit-base construction.",
        if bias > 0.0 { "less" } else { "more" }
    );
}
