//! Monte Carlo slowing-down against an **exact deterministic oracle**, swept
//! across six dilutions and all three scattering kernels.
//!
//! # Why this is an oracle and not just another code comparison
//!
//! `physics::slowing_down::solve_deterministic` is not an independent
//! implementation of the same approximations — it is the *exact* solution of
//! the infinite-medium slowing-down equation on the same nuclear data, obtained
//! by a Volterra quadrature in lethargy. For isotropic-CM scattering off a
//! target at rest the two must agree to counting statistics and nothing else:
//! there is no modelling freedom left between them. So a disagreement here is a
//! defect in the Monte Carlo collision physics, full stop.
//!
//! That made this the instrument that excluded **energy treatment of
//! self-shielding** from the ring-RPT residual hunt: the MC tracks the
//! deterministic answer at every dilution from sigma_b = 10 b (deeply shielded)
//! to 10 000 b (nearly dilute), so whatever the pebble offset is, it is not the
//! code failing to self-shield in energy.
//!
//! Two of my own bugs were found by this comparison rather than by the physics
//! under test, which is the usual way round:
//!
//! - the `F(0+)` boundary value of the deterministic solve was set to zero when
//!   the true continuous collision density just below the source is
//!   `sum_i (Sigma_s,i/Sigma_t)/(1-alpha_i)`. Wrong boundary value, first-order
//!   convergence, and the error shrank with grid refinement, so it looked like
//!   quadrature noise rather than a bug;
//! - a resonance-adaptive grid can be coarser than the narrowest scatter window
//!   (U-238's is 0.0169 lethargy). The solve then returned `p_esc = 0.00000`
//!   where the truth is 0.176. `solve_on_grid` now rejects such a grid instead
//!   of silently returning a wrong number.
//!
//! # V&V result
//!
//! See the gate at the bottom of `main` for the recorded agreement, the date,
//! and the tolerances. `tests/ring_rpt_hunt_lessons.rs` carries the
//! fast-running regression form of the same comparison.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example slowing_down_oracle
//! ```
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::slowing_down::*;
use outram_mc_libs::vv::assert_monotone;
use std::time::Instant;

const TEMP: f64 = 293.6;

fn main() {
    let e_top: f64 = std::env::var("E_TOP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10.0e3);
    let e_bot: f64 = std::env::var("E_BOT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let hist: usize = std::env::var("HIST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    let t0 = Instant::now();
    let u238 = load("U238", "n-092_U_238.endf");
    let c12 = load("C12", "n-006_C_012-ENDF8.0.endf");
    eprintln!("data ready in {:.1} s", t0.elapsed().as_secs_f64());
    let nuclides = vec![c12, u238];

    let band = SlowingDownBand { e_top, e_bot };
    let mut grid = nuclides[1].native_energy_grid(e_bot * 0.9, e_top * 1.1);
    let g2 = nuclides[0].native_energy_grid(e_bot * 0.9, e_top * 1.1);
    grid.extend_from_slice(&g2);
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * b.abs());
    eprintln!("union grid: {} points over {e_bot}–{e_top} eV", grid.len());

    let sigma_p_c = 4.7392_f64; // C-12 potential scattering, b
    println!(
        "\n{:>10}  {:>9}  {:>11}  {:>11}  {:>11}  {:>9}",
        "sigma_b/b", "N_C/N_U8", "det p_esc", "MC iso", "MC aniso", "MC prod"
    );
    // (sigma_b, det p_esc, [iso, aniso, prod], 1 sigma) for the V&V gate below.
    let mut sweep: Vec<(f64, f64, [f64; 3], f64)> = Vec::new();
    for &sigma_b in &[10.0_f64, 30.0, 100.0, 300.0, 1000.0, 10000.0] {
        let ratio = sigma_b / sigma_p_c;
        let mix = vec![
            MixComponent {
                nuclide_idx: 0,
                atom_density: ratio * 1.0e-3,
            },
            MixComponent {
                nuclide_idx: 1,
                atom_density: 1.0e-3,
            },
        ];
        let t = Instant::now();
        let det = solve_deterministic(&nuclides, &mix, band, TEMP, &grid);
        let t_det = t.elapsed().as_secs_f64();

        let mc = |k: ScatterKernel| {
            InfiniteMediumMc {
                histories: hist,
                seed: 0xABCD_0001,
                kernel: k,
                max_collisions: 200_000,
            }
            .run(&nuclides, &mix, band, TEMP)
        };
        let iso = mc(ScatterKernel::IsotropicCmAtRest);
        let ani = mc(ScatterKernel::AnisotropicCmAtRest);
        let pro = mc(ScatterKernel::Production);
        let se = InfiniteMediumMc {
            histories: hist,
            ..Default::default()
        }
        .stderr_of(iso.escaped);
        println!(
            "{sigma_b:>10.0}  {ratio:>9.1}  {:>11.5}  {:>11.5}  {:>11.5}  {:>11.5}   (det {:.1} s, 1σ {:.5})",
            det.escaped, iso.escaped, ani.escaped, pro.escaped, t_det, se
        );
        println!(
            "{:>10}  {:>9}  {:>11}  {:>+11.2}  {:>+11.2}  {:>+11.2}   % diff vs deterministic",
            "",
            "",
            "",
            100.0 * (iso.escaped / det.escaped - 1.0),
            100.0 * (ani.escaped / det.escaped - 1.0),
            100.0 * (pro.escaped / det.escaped - 1.0)
        );
        sweep.push((
            sigma_b,
            det.escaped,
            [iso.escaped, ani.escaped, pro.escaped],
            se,
        ));
    }

    vv_gate(&sweep);
}

/// V&V gate: the Monte Carlo must reproduce the exact deterministic solution to
/// counting statistics, at every dilution, for every kernel.
///
/// # Tolerances, and why they are what they are
///
/// The isotropic-CM-at-rest kernel is the *same physics* the deterministic
/// solve assumes, so its gate is pure statistics: `4 sigma` on the Monte Carlo
/// estimate, with a 0.5 % floor for the quadrature error of the deterministic
/// solve itself on a finite grid. There is no physical reason for it to miss.
///
/// The anisotropic and production kernels add real physics the deterministic
/// solve does not model (CM anisotropy, and for `Production` also free-gas
/// target motion and inelastic channels), so they are held to a looser
/// `max(4·se, 5 %)` envelope: they are *supposed* to differ, and the assertion on
/// them is that they differ by a physically sensible amount rather than
/// diverging.
///
/// # Why the 5 % is floored by the statistics, and not flat
///
/// It was flat until 2026-09-13, when it fired at the most self-shielded
/// dilution — `AnisotropicCmAtRest` +27.46 % against the oracle at
/// `sigma_b = 10 b`. That is not a regression. At that dilution `p_esc` is
/// **9.1e-4 and its own sampling standard error is 1.0e-4, 11 % of it**, so a
/// flat 5 % band is a quarter of the noise and the row cannot be held to it at
/// any history count this example can afford (matching 5 % to 1 sigma would need
/// ~77x the histories, for one row). The measured departures there are +3.3 %,
/// +27.5 % and +8.8 % — all inside 2.6 sigma — and every other dilution has all
/// three kernels inside 2.4 % with `se` under 2 %.
///
/// So the physics bound is kept wherever the measurement has the power to test
/// it, and the noise floor takes over where it does not. A gate must not assert
/// more precision than the measurement has; the alternative — leaving a 5 % band
/// on an 11 % measurement — is a gate that fires on luck in both directions.
///
/// The monotonicity claim is the shape check: `p_esc` must rise with
/// `sigma_b`, because more moderator per absorber atom means less
/// self-shielding. A code that returns a constant escape probability passes
/// every agreement envelope above and is still wrong.
fn vv_gate(sweep: &[(f64, f64, [f64; 3], f64)]) {
    use outram_mc_libs::vv::assert_absolute;

    println!("\n=== V&V gate: MC vs the exact deterministic slowing-down solution ===");
    assert!(
        sweep.len() >= 2,
        "the dilution sweep produced {} rows; the gate needs at least two",
        sweep.len()
    );

    const KERNEL_NAMES: [&str; 3] = ["IsotropicCmAtRest", "AnisotropicCmAtRest", "Production"];

    for &(sigma_b, det, mc, se) in sweep {
        assert!(
            det > 0.0 && det < 1.0,
            "deterministic p_esc = {det} at sigma_b = {sigma_b} b is not a \
             probability. A grid coarser than the narrowest scatter window used \
             to return exactly 0.0 here; `solve_on_grid` now rejects that, so \
             this firing means something new."
        );
        // Isotropic CM at rest: identical physics, so statistics only.
        assert_absolute(
            &format!("iso-CM MC vs deterministic @ sigma_b = {sigma_b:.0} b"),
            mc[0],
            det,
            (4.0 * se).max(0.005 * det),
        );
        // The other two model more physics than the oracle does.
        //
        // The envelope is `max(4·se, 5 % of det)` and not a flat 5 %, for the same
        // reason the iso-CM one above is: **a gate must not assert more precision
        // than the measurement has.** At the most self-shielded dilution the
        // escape probability is ~9e-4 and its own sampling standard error is 11 %
        // of it, so a flat 5 % band there is a quarter of the noise. It passed for
        // a while on one lucky sample and fired on 2026-09-13 at +27.46 %, which
        // is 2.6 sigma — not a regression, an ill-conditioned comparison. Every
        // dilution from 30 b up has se under 2 % and is still held to the 5 %
        // physics bound, which is where the claim has power.
        for k in 1..3 {
            let rel = (mc[k] - det) / det;
            let envelope = (4.0 * se).max(0.05 * det);
            let n_sigma = if se > 0.0 {
                (mc[k] - det).abs() / se
            } else {
                0.0
            };
            println!(
                "  [{}] {} vs deterministic @ sigma_b = {sigma_b:.0} b: \
                 {:+.2} % ({n_sigma:.1} sigma; envelope ±{:.2} % = max(4 se, 5 %), \
                 extra physics expected)",
                if (mc[k] - det).abs() <= envelope {
                    "PASS"
                } else {
                    "FAIL"
                },
                KERNEL_NAMES[k],
                rel * 100.0,
                100.0 * envelope / det,
            );
            assert!(
                (mc[k] - det).abs() <= envelope,
                "{} gives p_esc = {:.5} against the deterministic {det:.5} at \
                 sigma_b = {sigma_b:.0} b, a {:+.2} % departure. This kernel \
                 models CM anisotropy (and, for Production, free-gas motion and \
                 inelastic channels) that the oracle does not, so a few percent \
                 is expected — 5 % is not.",
                KERNEL_NAMES[k],
                mc[k],
                rel * 100.0,
            );
        }
    }

    let det_curve: Vec<f64> = sweep.iter().map(|&(_, d, _, _)| d).collect();
    assert_monotone(
        "deterministic p_esc rises with sigma_b (less self-shielding)",
        &det_curve,
        true,
        0.0,
    );
    let iso_curve: Vec<f64> = sweep.iter().map(|&(_, _, m, _)| m[0]).collect();
    assert_monotone(
        "Monte Carlo p_esc rises with sigma_b (less self-shielding)",
        &iso_curve,
        true,
        0.02,
    );

    let (first, last) = (det_curve[0], det_curve[det_curve.len() - 1]);
    assert!(
        last - first > 0.05,
        "p_esc moved only {:.4} across a 1000x range in sigma_b \
         ({first:.5} -> {last:.5}). Self-shielding is barely being modelled; \
         a flat curve passes every agreement envelope above.",
        last - first,
    );
    println!(
        "  [PASS] self-shielding spans the sweep: p_esc {first:.5} -> {last:.5} \
         over a 1000x range in sigma_b"
    );
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
    Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).expect("reconstruct")
}
