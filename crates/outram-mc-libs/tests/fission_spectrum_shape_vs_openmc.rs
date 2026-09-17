//! **The fission spectrum's SHAPE against OpenMC's — a new `op-os8x` hypothesis,
//! because the mean cannot see the thing the residual is.**
//!
//! # Why this, and why now
//!
//! `op-os8x` is a spectral residual against OpenMC on Godiva: **+0.42 % too much
//! flux at 1.9–3.0 MeV** against **1.2–1.9 % too little at 67–174 keV**, on
//! identical data, while `k` agrees. Every candidate the cross-code study
//! carried has now been measured and excluded — the cross sections
//! (≤0.06 % flux-weighted), both angular laws, the MT=91 transfer table, the
//! within-row CDF inversion, the inter-row unit-base rule, the CM→lab transform
//! (machine precision against Galilean addition) and channel branching
//! (bit-identical partitions, ≤1.23 σ sampled).
//!
//! **χ was cleared too — but only on its mean.**
//! `tests/fission_spectrum_vs_openmc.rs` compares `⟨E_out⟩` and finds
//! 0.008–0.018 %, and that is a real result. It is also insensitive to exactly
//! the thing `op-os8x` is: a **redistribution** that moves probability out of
//! ~100 keV and into the MeV window conserves `⟨E⟩` to first order, because the
//! 10 MeV tail compensates. A 0.4 % shift between two bands holding 2.3 % and
//! 21 % of χ moves the mean by far less than the 0.1 % Monte Carlo error that
//! comparison runs at.
//!
//! And χ is the **largest single source of neutrons in a bare fast assembly** —
//! every neutron is born from it. If its shape is off in these bands, that is
//! the residual, with no transport mechanism needed at all.
//!
//! # Methodology
//!
//! U-235 (94 % of Godiva), ENDF/B-VIII.0. Sample
//! [`Nuclide::sample_fission_energy`] 4 000 000 times at each of five incident
//! energies and bin into nine bands, two of which are precisely the residual's:
//! **67–174 keV** (the deficit) and **1.9–3.0 MeV** (the excess).
//!
//! The oracle is OpenMC 0.15.3's own χ table for the same evaluation, read from
//! `work/U235.h5` and **integrated analytically** — not sampled — so it carries
//! no Monte Carlo noise of its own
//! (`verification_and_validation/openmc_godiva_cross_code/chi_shape_oracle.py`,
//! regenerable). It is interpolated in incident energy, because OpenMC picks
//! table `i` or `i+1` with probability `r` and the *expectation* of that draw is
//! the interpolation — reading the nearest table instead is the trap this study
//! has hit three times.
//!
//! # Results
//!
//! Printed by the test.
//!
//! # Reading the outcome
//!
//! - **A difference of the right sign and size in the two residual bands** would
//!   make χ's shape the explanation for `op-os8x`.
//! - **Agreement** excludes the last large candidate and says the residual is
//!   not in any secondary-energy law at all, which would be a genuinely new
//!   constraint rather than another suspicion.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
const DRAWS: usize = 4_000_000;

/// The nine bands, matching `chi_shape_oracle.py`'s `BANDS` exactly.
const BANDS: &[(f64, f64)] = &[
    (1.0e-3, 1.0e3),
    (1.0e3, 6.7e4),
    (6.7e4, 1.74e5),
    (1.74e5, 1.0e6),
    (1.0e6, 1.9e6),
    (1.9e6, 3.0e6),
    (3.0e6, 6.0e6),
    (6.0e6, 1.0e7),
    (1.0e7, 3.0e7),
];

/// The two bands `op-os8x` is localised to, as indices into [`BANDS`].
const DEFICIT_BAND: usize = 2; // 67-174 keV
const EXCESS_BAND: usize = 5; // 1.9-3.0 MeV

/// OpenMC's χ band fractions, `[incident energy] × [band]`, from
/// `chi_shape_oracle.py` (analytic integration of its own tabulated pdf,
/// interpolated in incident energy).
const OPENMC_CHI: &[(f64, [f64; 9])] = &[
    (
        1.0e3,
        [
            0.00001, 0.00762, 0.02301, 0.27665, 0.27060, 0.21032, 0.18594, 0.02447, 0.00138,
        ],
    ),
    (
        1.0e5,
        [
            0.00001, 0.00757, 0.02294, 0.27616, 0.27085, 0.21043, 0.18597, 0.02468, 0.00137,
        ],
    ),
    (
        1.0e6,
        [
            0.00001, 0.00734, 0.02248, 0.27229, 0.27073, 0.21104, 0.18828, 0.02643, 0.00140,
        ],
    ),
    (
        2.0e6,
        [
            0.00001, 0.00724, 0.02216, 0.26858, 0.26844, 0.21131, 0.19244, 0.02821, 0.00159,
        ],
    ),
    (
        5.0e6,
        [
            0.00001, 0.00732, 0.02236, 0.25978, 0.26133, 0.21096, 0.20275, 0.03328, 0.00222,
        ],
    ),
];

#[test]
fn fission_spectrum_shape_matches_openmc_band_by_band() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (chi shape vs OpenMC)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape, "U235", TEMP_K, 1.0e-3).expect("U-235 reconstructs");

    let mut worst_z = 0.0f64;
    let mut worst_label = String::new();
    let mut deficit_shifts = Vec::new();
    let mut excess_shifts = Vec::new();

    for &(e_in, theirs) in OPENMC_CHI {
        let mut counts = [0u64; 9];
        let mut seed = 0x5EED_0F15u64 ^ (e_in as u64);
        for _ in 0..DRAWS {
            let e_out = nuc.sample_fission_energy(e_in, &mut seed);
            for (b, &(lo, hi)) in BANDS.iter().enumerate() {
                if e_out >= lo && e_out < hi {
                    counts[b] += 1;
                    break;
                }
            }
        }
        let n = DRAWS as f64;
        println!("\nE_in = {e_in:.3e} eV, {DRAWS} draws:");
        println!(
            "{:>10} {:>10} {:>10} {:>10} {:>9} {:>8}",
            "band lo", "band hi", "ours", "OpenMC", "diff", "sigma"
        );
        for (b, &(lo, hi)) in BANDS.iter().enumerate() {
            let ours = counts[b] as f64 / n;
            let ref_f = theirs[b];
            // Binomial error on our sampled fraction; the oracle is analytic and
            // contributes none.
            let sem = (ours * (1.0 - ours) / n).sqrt().max(1.0e-12);
            let z = (ours - ref_f).abs() / sem;
            let rel = if ref_f > 0.0 {
                (ours - ref_f) / ref_f * 100.0
            } else {
                0.0
            };
            if ref_f > 1.0e-4 && z > worst_z {
                worst_z = z;
                worst_label = format!("E_in={e_in:.2e} band {lo:.2e}-{hi:.2e}");
            }
            if b == DEFICIT_BAND {
                deficit_shifts.push(rel);
            }
            if b == EXCESS_BAND {
                excess_shifts.push(rel);
            }
            println!(
                "{lo:10.2e} {hi:10.2e} {:10.6} {ref_f:10.6} {rel:+8.3}% {z:8.2}",
                ours
            );
        }
    }

    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!("\n--- the two bands op-os8x lives in ---");
    println!(
        "   67-174 keV  (flux DEFICIT of 1.2-1.9 %): chi shift {:+.3} % (per incident E: {:?})",
        mean(&deficit_shifts),
        deficit_shifts
            .iter()
            .map(|x| format!("{x:+.3}"))
            .collect::<Vec<_>>()
    );
    println!(
        "   1.9-3.0 MeV (flux EXCESS of +0.42 %):   chi shift {:+.3} % (per incident E: {:?})",
        mean(&excess_shifts),
        excess_shifts
            .iter()
            .map(|x| format!("{x:+.3}"))
            .collect::<Vec<_>>()
    );
    println!("   worst band deviation = {worst_z:.2} sigma   at {worst_label}");

    // The gate: every band with a meaningful share must agree within 4 sigma of
    // our own sampling error. The oracle is analytic, so all of the error is on
    // this side and the comparison is as sharp as DRAWS allows.
    assert!(
        worst_z < 4.0,
        "chi's SHAPE differs from OpenMC's by {worst_z:.2} sigma at {worst_label}. If the sign \
         and size line up with op-os8x -- too much in 1.9-3.0 MeV, too little in 67-174 keV -- \
         then the fission spectrum's shape IS the residual, and the mean comparison in \
         fission_spectrum_vs_openmc.rs missed it because a mean cannot see a redistribution \
         that the 10 MeV tail compensates for."
    );
}
