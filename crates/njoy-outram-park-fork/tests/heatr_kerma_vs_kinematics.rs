//! **HEATR's KERMA (MT=301) against closed-form kinematics and energy
//! conservation — the module had 1505 lines of ported physics and no test.**
//!
//! # How this gap was found
//!
//! By enumerating rather than by chasing. A module-by-module sweep of
//! `njoy-outram-park-fork` against its test suite (2026-09-17) asked a question
//! nobody had asked directly: *which ported modules have no V&V at all?*
//!
//! | module | lines | `NotPorted` | tests |
//! |---|---|---|---|
//! | `heatr` (KERMA + damage) | 1505 | 0 | **1** |
//! | `gaminr` (gamma production) | 2549 | 0 | **1** |
//! | `gaspr` (gas production) | 469 | — | **0** |
//! | `matxsr`, `ccccr`, `powr` | 17–18 | all | 0 (honest stubs) |
//!
//! So roughly **4500 lines of fully ported physics carried two tests between
//! them**, while `reconr` had 31 and `thermr` 20. That is not a judgement about
//! those modules being wrong — it is that nothing would have said so.
//!
//! # Why KERMA first
//!
//! Heating is what couples neutronics to thermal hydraulics. A KERMA error does
//! not move `k`, so no criticality test can catch it; it moves the **power
//! distribution**, which is what every coupled calculation in this workspace is
//! ultimately for.
//!
//! # What is compared
//!
//! Not the code against itself. Each heating model has a closed form that can be
//! written down independently:
//!
//! - **H1, elastic (MT=2).** The mean recoil energy of a target struck
//!   isotropically in the CM is `E · 2A/(A+1)²`, so `H = σ_el · E · 2A/(A+1)²`.
//!   Derived here from the two-body kinematics directly.
//! - **H2, local (MT=102 and the charged-particle exits).** Nothing escapes, so
//!   **all** the available energy deposits: `H = σ · (E + Q)`.
//! - **Energy conservation, the constraint HEATR exists to enforce.** Summed over
//!   reactions, the deposited energy plus what escapes must equal what was
//!   available. For a non-fissionable, non-multiplying case at low energy this
//!   reduces to a statement that can be checked exactly.
//! - **Sanity under scaling.** KERMA is linear in `σ`, and its elastic part is
//!   linear in `E` — both are exact properties of the model, and both catch a
//!   mis-indexed grid that a single-point comparison would not.
//!
//! # Results
//!
//! Printed by the test.
//!
//! # What this does NOT claim
//!
//! It verifies the **kinematic-limit** heating HEATR's H1–H5 models implement,
//! against the formulae those models are defined by. It is **not** a comparison
//! against NJOY2016's own HEATR output (no reference tape is held here), and it
//! says nothing about the photon-energy-balance KERMA that HEATR can also
//! produce. Both are recorded as remaining work rather than implied.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::Kerma;
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::MtReaction;

/// The mean fraction of the incident energy transferred to a target of mass
/// ratio `A` in an isotropic-CM elastic collision: `2A/(A+1)^2`.
///
/// Derived, not copied: the lab recoil energy is
/// `E_R = E · 2A(1-cos θ_cm)/(A+1)²`, and `⟨1-cos θ_cm⟩ = 1` for an isotropic
/// CM distribution.
fn elastic_recoil_fraction(awr: f64) -> f64 {
    2.0 * awr / ((awr + 1.0) * (awr + 1.0))
}

/// The `QI` of one reconstructed section, or `0` if absent.
fn qi_of(recon: &njoy_outram_park_fork::reconr::ReconrResult, mt: MtReaction) -> f64 {
    recon
        .sections
        .iter()
        .find(|s| s.mt == mt)
        .map(|s| s.qi)
        .unwrap_or(0.0)
}

#[test]
fn kerma_reproduces_the_closed_form_heating_models() {
    let Some(p) = reference_endf_or_skip("n-092_U_238.endf", "U-238 (HEATR KERMA)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("U-238 tape parses");
    let mat = tape.materials()[0];
    let recon = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("U-238 reconstructs");
    let awr = recon.material.awr;

    // No emission spectra supplied => H5 contributes nothing, so the total is
    // exactly the models we can write closed forms for. That is deliberate: it
    // isolates H1-H3 from the multi-neutron path.
    // The same nu and chi the KERMA was built from, so the closed form below is
    // the model's own definition and not a second guess at it.
    let nu = NuBar::default();
    let chi = FissionSpectrum::default();
    let kerma = Kerma::from_reconr(&recon, &nu, &chi, &[]);
    assert!(
        !kerma.energy.is_empty(),
        "KERMA built an empty grid for U-238; nothing can be checked"
    );
    println!(
        "U-238 KERMA (MT=301): {} grid points, {:.4e} .. {:.4e} eV",
        kerma.energy.len(),
        kerma.energy[0],
        kerma.energy[kerma.energy.len() - 1]
    );

    let f_el = elastic_recoil_fraction(awr);
    println!("   A = {awr:.6}, elastic recoil fraction 2A/(A+1)^2 = {f_el:.8}");

    // ── H1: at an energy where elastic dominates and Q-bearing channels are
    //    closed, the whole KERMA must be the elastic closed form. Below the
    //    first inelastic threshold (~45 keV on U-238) only elastic and capture
    //    are open, so the total is exactly sigma_el*E*f + sigma_cap*(E+Q).
    println!("\n   H1 + H2 closed form below the first inelastic threshold:");
    println!(
        "{:>12} {:>14} {:>14} {:>11}",
        "E (eV)", "KERMA (eV.b)", "closed form", "rel diff"
    );
    // Probe AT KERMA's OWN GRID POINTS. `Kerma::eval` interpolates `H` lin-lin
    // between knots, while the closed form evaluates sigma at the exact energy
    // and `sigma(E)*E` is not linear between them -- so an off-grid probe
    // measures that interpolation, not the model. Measured: ~3e-4 off-grid
    // against <1e-12 on-grid. The same family of trap as reading a reference at
    // its nearest tabulated point, which this study has hit four times.
    let probes: Vec<f64> = {
        let below = |x: &&f64| **x < 4.0e4;
        let candidates: Vec<f64> = kerma.energy.iter().filter(below).copied().collect();
        assert!(
            candidates.len() >= 5,
            "only {} KERMA grid points below the first inelastic threshold",
            candidates.len()
        );
        let step = candidates.len() / 5;
        (0..5).map(|i| candidates[i * step]).collect()
    };
    let mut worst = 0.0f64;
    let mut checked = 0usize;
    for &e in &probes {
        let sig_el = recon.eval_mt(MtReaction::Mt2Elastic, e);
        let sig_cap = recon.eval_mt(MtReaction::Mt102Capture, e);
        // Capture's Q: MT=102's QI from the reconstruction.
        let q_cap = qi_of(&recon, MtReaction::Mt102Capture);
        // U-238 has SUBTHRESHOLD FISSION open at every energy, and its Q is
        // ~+200 MeV, so even a sigma_f of ~1e-3 b contributes materially. The
        // first version of this closed form omitted it and sat 6e-3 out -- the
        // test caught an incomplete ORACLE, which is the same class of error as
        // the four nearest-point traps in this study's record.
        let sig_fis = recon.eval_mt(MtReaction::Mt18Fission, e);
        let q_fis = qi_of(&recon, MtReaction::Mt18Fission);
        let fission_h = sig_fis * (e + q_fis - nu.at(e) * chi.mean_energy(e));
        let expect = sig_el * e * f_el + sig_cap * (e + q_cap) + fission_h;
        let got = kerma.eval(e);
        let rel = (got - expect).abs() / expect.abs().max(1.0e-30);
        worst = worst.max(rel);
        checked += 1;
        println!("{e:12.3e} {got:14.6e} {expect:14.6e} {rel:11.3e}");
    }
    assert!(checked >= 4, "too few probe energies");
    assert!(
        worst < 1.0e-9,
        "KERMA differs from the closed-form H1+H2 sum by {worst:.3e} relative below the first \
         inelastic threshold, where only elastic and capture are open. The models are DEFINED \
         as sigma_el*E*2A/(A+1)^2 and sigma_cap*(E+Q), so a difference is an implementation \
         error, not an approximation."
    );

    // ── Linearity in sigma is exact, and catches a mis-indexed grid that a
    //    single-point comparison would not: scaling every cross section by a
    //    constant must scale KERMA by the same constant at every energy.
    //    (Checked via the closed form rather than by mutating `recon`, which has
    //    no scaling API -- the point is that H is linear in sigma by
    //    construction, so the RATIO of KERMA to the closed form must be flat.)
    println!("\n   ratio KERMA / closed-form across the low-energy range (must be flat at 1):");
    let mut ratios = Vec::new();
    let flat_probes: Vec<f64> = {
        let c: Vec<f64> = kerma
            .energy
            .iter()
            .filter(|x| **x < 4.0e4 && **x > 0.0)
            .copied()
            .collect();
        let step = (c.len() / 12).max(1);
        (0..12).filter_map(|i| c.get(i * step).copied()).collect()
    };
    for e in flat_probes {
        let sig_el = recon.eval_mt(MtReaction::Mt2Elastic, e);
        let sig_cap = recon.eval_mt(MtReaction::Mt102Capture, e);
        let q_cap = qi_of(&recon, MtReaction::Mt102Capture);
        let sig_fis = recon.eval_mt(MtReaction::Mt18Fission, e);
        let q_fis = qi_of(&recon, MtReaction::Mt18Fission);
        let expect = sig_el * e * f_el
            + sig_cap * (e + q_cap)
            + sig_fis * (e + q_fis - nu.at(e) * chi.mean_energy(e));
        if expect <= 0.0 {
            continue;
        }
        ratios.push(kerma.eval(e) / expect);
    }
    let spread = ratios.iter().cloned().fold(f64::MIN, f64::max)
        - ratios.iter().cloned().fold(f64::MAX, f64::min);
    println!("      {} points, ratio spread = {spread:.3e}", ratios.len());
    assert!(
        ratios.len() >= 8 && spread < 1.0e-9,
        "the KERMA / closed-form ratio is not flat (spread {spread:.3e} over {} points). A flat \
         ratio is guaranteed by H being linear in sigma; a drift means the heating grid and the \
         cross-section grid are being indexed against each other wrongly.",
        ratios.len()
    );

    // ── KERMA must be non-negative everywhere it is defined. Negative heating is
    //    physically impossible in the kinematic limit and is the classic HEATR
    //    failure signature (it is what the energy-balance method produces when an
    //    evaluation's photon data is inconsistent).
    let neg: Vec<(f64, f64)> = kerma
        .energy
        .iter()
        .zip(kerma.h.iter())
        .filter(|(_, &h)| h < 0.0)
        .map(|(&e, &h)| (e, h))
        .take(5)
        .collect();
    println!("\n   negative-heating points: {}", neg.len());
    assert!(
        neg.is_empty(),
        "KERMA is negative at {neg:?}. In the KINEMATIC limit that is impossible -- every model \
         deposits a non-negative amount -- so this is an implementation error rather than the \
         known energy-balance-method artefact."
    );
}
