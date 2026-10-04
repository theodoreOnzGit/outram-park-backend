//! Resonance escape, measured in an infinite homogeneous medium against its
//! **analytic infinite-dilution limit** — the one piece of physics the FHR
//! pebble exercises and the Godiva benchmark does not.
//!
//! # Why
//!
//! After `op-50vu` (free-gas target motion) and `op-8l2e` (packing fraction) the
//! pebble still sits ~+4000 pcm above its OpenMC reference, and the residual is
//! entirely the non-thermal/thermal flux ratio: like-for-like at 0.625 eV,
//! `p` is +8.5 % and `ε` is −5.1 %. In plain terms **this code captures about
//! 8 % fewer neutrons above 0.625 eV than the reference does**, and above
//! 0.625 eV that is almost all U-238 resonance capture.
//!
//! Everything that feeds it has been measured and is right: the U-238 capture
//! **resonance integral** matches NJOY's own PENDF to +0.00 % over 0.5 eV –
//! 100 keV (`u238_resonance_integral.rs`), `ξ` matches analytic two-body
//! kinematics to 0.5 % across eight nuclides (`epithermal_slowing_down.rs`), and
//! the moderator scattering cross sections sit within a few percent of published
//! free-atom values. So the data and the kernel are not the problem. What has
//! never been checked is whether **transport assembles them into the right
//! absorption rate** — resonance self-shielding, which in a continuous-energy
//! Monte Carlo is supposed to be automatic and therefore never gets tested.
//!
//! # The oracle, which is analytic and needs no library
//!
//! In an infinite homogeneous medium of a moderator plus a *trace* absorber, the
//! slowing-down flux is unperturbed (`φ ∝ 1/E`) and the absorption probability
//! per source neutron is exactly, to first order in the absorber density,
//!
//! ```text
//! P_abs → N_a · RI_∞ / (ξ Σ_s)
//! ```
//!
//! with `RI_∞ = ∫ σ_γ dE/E` over the same band. That integral is not assumed
//! here — it is the **274.637 b** this crate's own reconstruction produces over
//! 0.5 eV – 100 keV, already verified against NJOY. So the test is closed-loop
//! on measured quantities and the only thing under test is the transport.
//!
//! Running the same medium at increasing absorber density then traces
//! self-shielding developing: `RI_eff` must fall away from `RI_∞` monotonically
//! as the dilution cross section `σ_0 = Σ_s/N_a` falls. **A code that
//! over-shields — that is, one that loses resonance absorption it should be
//! making — shows up as `RI_eff/RI_∞ < 1` while still dilute**, which is exactly
//! the shape of the pebble residual.
//!
//! # V&V result (2026-09-11, ENDF/B-VIII.0 @ 600 K)
//!
//! ```text
//!   sigma_0 [b]   P_abs(U8)   RI_eff [b]   RI_eff/RI_inf
//!     3.7958e5      0.00449      272.05       0.9906 +/- 0.0105
//!     3.7958e4      0.04103      253.04       0.9214
//!     3.7958e3      0.23245      159.77       0.5817
//!     3.7958e2      0.60981       56.84       0.2070
//! ```
//!
//! **The transport reproduces the analytic dilute limit to 0.9 sigma**, and
//! self-shielding then develops monotonically to 21 % of `RI_inf` at the densest
//! loading. So resonance self-shielding *in energy* is correct here, and the
//! failure mode this program was written to hunt — `RI_eff/RI` sitting low while
//! still dilute — is **not** present.
//!
//! That is an exclusion, and it is why the ring-RPT search moved on from U-238
//! resonance absorption. The attribution of the residual to self-shielded U-238
//! capture was later **refuted outright** by the LEU-COMP-THERM-008 case
//! differential (`examples/lct008_keff.rs`): three cases sharing one lattice give
//! +2950/+2271/+1713 pcm, a 14-sigma spread that an error in `p` cannot produce.
//!
//! # Re-measured 2026-10-04 (GitHub #524)
//!
//! **The kernel changed in this change, and the old one is kept.** Until
//! 2026-10-04 this program carried its own copy of the collision loop, whose
//! comment claimed "both arms use the same kernels transport uses". That was **not true**
//! of it: it scattered off U-238 without DBRC (`free_gas_elastic_scatter`, no
//! table) and looked cross sections up without URR self-shielding, although
//! both have been transport's defaults since 2026-09-20. The loop now lives in
//! `examples/common/energy_only_slowing_down.rs`, shared with
//! `ugraphite_four_factor.rs`, and applies both by default. `KERNEL=legacy`
//! runs the old kernel, and `TEMP_K` sets the temperature (default 600 K).
//!
//! **Legacy kernel, 600 K, on `develop` at `bfeb81a083` and again through the
//! shared module** (seed 5 150 701, same histories; 272 s and 453 s single
//! core on an i9-13900K, the second run sharing the machine with other jobs):
//! the two runs agree **bit for bit** on every row, which is the check that the
//! move into the shared module changed nothing. Both differ slightly from the
//! 2026-09-11 record above, which predates later changes to the code; the
//! movement is within the counting statistics:
//!
//! ```text
//!   sigma_0 [b]   P_abs(U8)   RI_eff [b]   RI_eff/RI_inf     (2026-09-11)
//!     3.7958e5      0.00450      272.169      0.9910 +/- 0.0105   (0.9906)
//!     3.7958e4      0.04103      253.034      0.9213              (0.9214)
//!     3.7958e3      0.23264      159.918      0.5823              (0.5817)
//!     3.7958e2      0.60998       56.863      0.2070              (0.2070)
//! ```
//!
//! All three gates pass.
//!
//! **Default kernel (DBRC + URR on), and the tutorial's 296 K.** Same seed,
//! same histories; single core of an i9-13900K (16 logical cores, 62 GB, Linux, shared with
//! other jobs), CPU only; 542 s (600 K), 822 s (296 K, sharing its core), and
//! 552 s for the 296 K legacy run. `RI_eff/RI_inf` (1 sigma 0.0105 on the
//! first row, ~0.004 on the second and third):
//!
//! ```text
//!   sigma_0 [b]   600 K legacy   600 K default   296 K legacy   296 K default
//!     3.7958e5       0.9910         0.9948          1.0003         0.9820
//!     3.7958e4       0.9213         0.9249          0.8962         0.9026
//!     3.7958e3       0.5823         0.5863          0.5233         0.5236
//!     3.7958e2       0.2070         0.2095          0.1884         0.1877
//! ```
//!
//! **Quote the default-kernel columns** — they are the physics transport
//! runs. All three gates pass in all four runs. At 600 K turning DBRC + URR on
//! raises `RI_eff` by 0.4-1.2 % on every row, the direction expected of DBRC
//! (resonance up-scatter into the peaks), though no single row resolves it
//! beyond ~1.3 sigma; at 296 K the two kernels agree within statistics, as a
//! smaller Doppler width predicts. The 296 K default dilute row is 0.982, 1.7
//! sigma below the analytic 1; its legacy twin is 1.000. The runs are not
//! paired (the RNG stream decorrelates at the first differing draw), so the
//! difference is 1.2 sigma and is recorded, not explained. `RI_MEASURED_B`
//! is the 600 K integral; Doppler broadening conserves the area, and the
//! 296 K integral was not re-computed here (`Not re-checked`).
//!
//! Colder fuel shields more: at `sigma_0 = 3796 b` the effective integral is
//! 52 % of the dilute value at 296 K against 59 % at 600 K. That is the
//! Doppler effect seen from the resonance-integral side.
//!
//! # What this deliberately leaves out
//!
//! No geometry, no tracking, no majorant: the neutron is followed in **energy
//! only**, sampling the nuclide by `N·σ_t` and the reaction by the same
//! branch order ~~`keff_delta` uses~~ `transport_csg::transport_history_vr`
//! uses (see `common/energy_only_slowing_down.rs`; corrected 2026-10-04). If the answer is right here and wrong in the
//! pebble, the fault is spatial (geometry, delta tracking); if it is wrong here
//! too, it is in the collision physics itself.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example u238_resonance_escape
//! ```

#[path = "common/energy_only_slowing_down.rs"]
mod energy_only_slowing_down;

use energy_only_slowing_down::{slow_down, KernelOptions};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::vv::{assert_absolute, assert_monotone};

/// Medium temperature \[K\]: `TEMP_K` from the environment, default 600 K
/// (the temperature every result recorded here before 2026-10-04 was taken
/// at). The tutorial's room-temperature run is `TEMP_K=296`.
fn temp_k() -> f64 {
    std::env::var("TEMP_K")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600.0)
}

/// The collision kernel: everything transport applies (DBRC, URR) unless
/// `KERNEL=legacy` asks for the pre-2026-10-04 kernel (neither), which is kept
/// so the recorded 2026-09-11 table stays reproducible and the worth of the
/// two terms stays measurable.
fn kernel() -> KernelOptions {
    match std::env::var("KERNEL").as_deref() {
        Ok("legacy") => KernelOptions::legacy(),
        _ => KernelOptions::default(),
    }
}
/// Source energy — above the whole resolved range, so nothing is captured
/// outside the band the resonance integral covers.
const E_SOURCE: f64 = 1.0e5;
/// Cadmium cutoff: a neutron reaching it has escaped the resonances.
const E_CUT: f64 = 0.5;
/// This crate's own measured `∫σ_γ dE/E` over `[E_CUT, E_SOURCE]`, verified
/// against NJOY's PENDF to +0.00 % (`u238_resonance_integral.rs`).
const RI_MEASURED_B: f64 = 274.637;

fn main() {
    let temp = temp_k();
    let opts = kernel();
    let dir = endf_dir();
    let c12 = load("C12", "n-006_C_012-ENDF8.0.endf", &dir, temp);
    let u238 = load("U238", "n-092_U_238.endf", &dir, temp);

    // Graphite-density carbon, free gas: above 4 eV the bound law is off anyway,
    // and leaving it off keeps xi*Sigma_s a number this program can state.
    let n_c = 0.08_f64; // atoms/barn.cm
    let awr_c = c12.awr;
    let alpha_c = ((awr_c - 1.0) / (awr_c + 1.0)).powi(2);
    let xi_c = 1.0 + alpha_c * alpha_c.ln() / (1.0 - alpha_c);
    // Carbon's elastic cross section is flat across the whole band; take it at
    // 1 keV, in the middle of it.
    let sigma_s_c = c12.xs_at_energy(1.0e3, temp).elastic;
    let xi_sigma_s = xi_c * n_c * sigma_s_c;
    // Moderator first: the shared kernel's cumulative nuclide search then makes
    // the same choice, from the same draw, as the two-nuclide loop it replaced.
    let nuclides = vec![c12, u238];

    println!(
        "Infinite homogeneous medium, {temp} K, source {E_SOURCE:.0e} eV, cutoff {E_CUT} eV, \
         kernel: DBRC {} URR {}",
        opts.dbrc, opts.urr
    );
    println!(
        "moderator: C-12 at N = {n_c} /b.cm, sigma_s = {sigma_s_c:.4} b, xi = {xi_c:.6} \
         -> xi*Sigma_s = {xi_sigma_s:.6} /cm"
    );
    println!("absorber : U-238, RI over this band (measured, = NJOY) = {RI_MEASURED_B} b\n");
    println!(
        "{:>12} {:>12} {:>12} {:>12} {:>12} {:>10} {:>10}",
        "N_U238",
        "sigma_0 [b]",
        "P_abs(U8)",
        "P_abs(dilute)",
        "RI_eff [b]",
        "RI_eff/RI",
        "P_abs(C)"
    );

    let mut seed = 5_150_701_u64;
    // (sigma_0, RI_eff/RI_inf, 1-sigma on that ratio) for the V&V gate below.
    let mut shielding: Vec<(f64, f64, f64)> = Vec::new();
    for &n_a in &[1.0e-6_f64, 1.0e-5, 1.0e-4, 1.0e-3] {
        // Fewer histories are needed when absorption is likely; keep the
        // absolute uncertainty on P_abs roughly constant.
        let n_hist = if n_a <= 1.0e-5 { 2_000_000 } else { 400_000 };
        let counts = slow_down(
            &nuclides,
            &[n_c, n_a],
            temp,
            E_SOURCE,
            E_CUT,
            n_hist,
            &mut seed,
            opts,
        );
        let (abs_m, abs_a) = (counts.absorbed_by[0], counts.absorbed_by[1]);
        // Only the absorber's own captures belong in RI_eff. The moderator has
        // its own 1/v capture -- carbon's is worth N_C * 1.5e-3 b / (xi*Sigma_s)
        // ~ 0.2 % of source neutrons, which is a third of the signal at the most
        // dilute point and would otherwise read as a 50 % over-absorption.
        let p_abs = abs_a as f64 / n_hist as f64;
        let p_mod = abs_m as f64 / n_hist as f64;
        let p_dilute = n_a * RI_MEASURED_B / xi_sigma_s;
        // Invert the first-order relation. Using -ln(1-P) rather than P itself
        // removes the leading depletion term, so the ratio stays meaningful once
        // absorption is no longer small.
        let ri_eff = -(1.0 - p_abs).ln() * xi_sigma_s / n_a;
        let sigma_0 = n_c * sigma_s_c / n_a;
        println!(
            "{n_a:>12.1e} {sigma_0:>12.4e} {p_abs:>12.5} {p_dilute:>12.5} {ri_eff:>12.3} \
             {:>10.4} {p_mod:>10.5}",
            ri_eff / RI_MEASURED_B
        );
        // Binomial 1-sigma on P_abs, propagated through RI_eff = -ln(1-P)*xi*Sigma_s/N_a.
        // d/dP of -ln(1-P) is 1/(1-P), so sigma_RI/RI = sigma_P / ((1-P) * -ln(1-P)).
        let sigma_p = (p_abs * (1.0 - p_abs) / n_hist as f64).sqrt();
        let rel_sigma = sigma_p / ((1.0 - p_abs) * -(1.0 - p_abs).ln());
        shielding.push((sigma_0, ri_eff / RI_MEASURED_B, rel_sigma));
    }

    println!(
        "\nHow to read this.\n\
         - The top row is effectively infinite dilution (sigma_0 ~ 4e5 b, against a\n\
           peak resonance sigma of ~1e4 b): `RI_eff/RI`\n\
           there must be **1.000**. It is a closed loop on quantities this repo has\n\
           already verified, so a departure is a transport defect, not a data one.\n\
         - Going down the rows, sigma_0 falls and self-shielding sets in, so\n\
           `RI_eff/RI` must fall **monotonically** below 1. That is physics.\n\
         - The failure mode being hunted is `RI_eff/RI` sitting low while still\n\
           dilute: resonance absorption that transport is not making."
    );

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // Measured 2026-09-11, ENDF/B-VIII.0 @ 600 K, seed 5_150_701, C-12 at
    // N = 0.08 /b.cm (xi*Sigma_s = 0.060393 /cm), RI_inf = 274.637 b:
    //
    //   sigma_0 [b]   P_abs(U8)   RI_eff [b]   RI_eff/RI_inf
    //     3.7958e5      0.00449      272.05        0.9906 +/- 0.0105
    //     3.7958e4      0.04103      253.04        0.9214
    //     3.7958e3      0.23245      159.77        0.5817
    //     3.7958e2      0.60981       56.84        0.2070
    //
    // The dilute limit is 0.9 sigma from unity -- the transport reproduces the
    // analytic first-order result. Self-shielding then develops monotonically to
    // 21 % of the infinite-dilution integral at the densest loading.
    //
    // The gate derives every tolerance from THIS run's own counting statistics
    // rather than from a number written here, so the numbers above are a record
    // and not something that can go stale into a false pass.
    //
    // Three separate claims, because two of them can pass while the physics is
    // wrong:
    //
    //   1. the dilute limit is unity        — the analytic oracle
    //   2. shielding is monotone in sigma_0 — the shape
    //   3. shielding is actually *present*  — without this, a code that returns
    //      a flat ratio of 1.000 at every dilution passes claims 1 and 2 and has
    //      no self-shielding at all
    println!("\n=== V&V gate: self-shielding against the analytic dilute limit ===");

    let (sigma_0_dilute, ratio_dilute, rel_sigma) = shielding[0];
    // 4 sigma of this run's own counting statistics, plus 1 % for the fact that
    // "infinite dilution" here is a finite sigma_0 and RI_MEASURED_B is itself a
    // quadrature of a resonance structure.
    let gate = 4.0 * rel_sigma + 0.01;
    println!(
        "  most dilute point: sigma_0 = {sigma_0_dilute:.3e} b, \
         RI_eff/RI_inf = {ratio_dilute:.4} (1 sigma = {:.4})",
        rel_sigma
    );
    assert_absolute(
        "dilute-limit RI_eff/RI_inf (analytic oracle: exactly 1)",
        ratio_dilute,
        1.0,
        gate,
    );

    let ratios: Vec<f64> = shielding.iter().map(|&(_, r, _)| r).collect();
    // sigma_0 falls down the rows, so the ratio must not rise. 1 % of slack for
    // the counting noise between adjacent rows.
    assert_monotone(
        "RI_eff/RI_inf falls as sigma_0 falls (self-shielding)",
        &ratios,
        false,
        0.01,
    );

    let deepest = *ratios.last().expect("four dilutions were run");
    assert!(
        deepest < 0.90,
        "self-shielding is not being modelled at all: at sigma_0 = {:.3e} b the \
         effective resonance integral is still {:.1} % of the infinite-dilution \
         value, where physics demands a visible depression.\n\
         A code with no self-shielding returns a flat 1.000 at every dilution and \
         passes both of the gates above. This is the one that catches it.",
        shielding.last().unwrap().0,
        deepest * 100.0,
    );
    println!(
        "  [PASS] self-shielding is present: RI_eff/RI_inf = {deepest:.4} at the \
         densest loading (must be < 0.90)"
    );
}

fn endf_dir() -> std::path::PathBuf {
    njoy_outram_park_fork::reference_data::reference_endf("n-006_C_012-ENDF8.0.endf")
        .expect("reference-data/endf/")
        .parent()
        .expect("parent")
        .to_path_buf()
}

fn load(name: &str, file: &str, dir: &std::path::Path, temp: f64) -> Nuclide {
    let p = dir.join(file);
    eprint!("  reconstructing {name:<6} … ");
    let t0 = std::time::Instant::now();
    let n = Nuclide::from_endf_file(&p, name, temp, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}
