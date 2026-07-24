//! End-to-end **PT (pressure-temperature) flash** of a light-hydrocarbon
//! mixture with the Peng-Robinson property package.
//!
//! This example composes the already-ported DWSIM thermo kernel — the
//! Peng-Robinson cubic EOS ([`outram_park_fork_dwsim_libs::thermo::cubic_eos`]),
//! the Wilson-seeded nested-loops flash
//! ([`outram_park_fork_dwsim_libs::thermo::flash`]), and the property-package
//! glue ([`outram_park_fork_dwsim_libs::thermo::property_package`]) — into a
//! single isothermal-isobaric two-phase vapour-liquid-equilibrium calculation
//! and prints the phase split.
//!
//! ## What it does
//!
//! Flashes an equimolar **methane + ethane** binary at a stated temperature and
//! pressure that lie inside the two-phase envelope, using
//! [`PropertyPackageModel::PengRobinson`]. It prints:
//!
//! - the vapour molar fraction `β` \[-\],
//! - the liquid `x_i`, vapour `y_i`, and equilibrium `K_i = y_i/x_i` \[-\],
//! - the overall mass-balance check `z_i = (1−β) x_i + β y_i` (should recover
//!   the feed to machine precision), and
//! - the same flash re-run with the ideal (Wilson) and SRK packages for
//!   comparison.
//!
//! ## Mixture parameters and their source
//!
//! The critical constants, acentric factors, and molar masses come from the
//! crate's [`reference`](outram_park_fork_dwsim_libs::thermo::component::reference)
//! presets, whose values are the standard tabulated numbers from
//! **Poling, Prausnitz & O'Connell, *The Properties of Gases and Liquids*,
//! 5th ed. (McGraw-Hill, 2001), Appendix A** — public-literature data
//! (workspace `DATA_POLICY`):
//!
//! - Methane: `Tc = 190.56 K`, `Pc = 4.599 MPa`, `ω = 0.011`.
//! - Ethane:  `Tc = 305.32 K`, `Pc = 4.872 MPa`, `ω = 0.099`.
//!
//! ## Honest scope
//!
//! Verification, **not** validation: the flash is EOS-consistent (it satisfies
//! iso-fugacity and the mass balance internally), but the numbers are **not**
//! validated against an experimental / NIST / DECHEMA VLE benchmark. Binary
//! interaction parameters are `k_ij = 0` (geometric-mean rule) and no phase-
//! stability pre-test is run. Pure `f64` compute — no BLAS, no GUI — so this
//! example builds and runs natively on Android/Termux. Independent OUTRAM PARK
//! fork, not the official DWSIM; not for nuclear / safety-critical use.
//!
//! Run with: `cargo run -p outram-park-fork-dwsim-libs --release --example pt_flash`

use outram_park_fork_dwsim_libs::thermo::component::reference;
use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;

fn main() {
    // Equimolar methane(1) / ethane(2) feed at a two-phase state.
    let methane = reference::methane();
    let ethane = reference::ethane();
    let components = [methane.clone(), ethane.clone()];
    let z = [0.5_f64, 0.5_f64];
    let t = 200.0_f64; // K
    let p = 2.0e6_f64; // Pa (20 bar)

    println!("======================================================================");
    println!(" OUTRAM PARK / DWSIM-fork  —  PT (pressure-temperature) flash");
    println!("======================================================================");
    println!(
        " Feed          : {:.3} {} + {:.3} {}  (mole fractions)",
        z[0], components[0].name, z[1], components[1].name
    );
    println!(" Temperature   : {t:.2} K");
    println!(" Pressure      : {:.3e} Pa  ({:.1} bar)", p, p / 1.0e5);
    println!(" k_ij          : 0 (geometric-mean rule)");
    println!();

    // -- Primary result: Peng-Robinson EOS-consistent flash. ------------------
    let pr = PropertyPackageModel::PengRobinson
        .flash_pt(&components, &z, t, p)
        .expect("PR flash converged");

    println!("----------------------------------------------------------------------");
    println!(" Peng-Robinson property package");
    println!("----------------------------------------------------------------------");
    println!(" Vapour molar fraction  beta = {:.6}", pr.beta);
    println!(" Outer iterations            = {}", pr.iterations);
    println!();
    println!(
        "   {:<10} {:>12} {:>12} {:>12} {:>12}",
        "component", "z_i", "x_i (liq)", "y_i (vap)", "K_i = y/x"
    );
    for (i, c) in components.iter().enumerate() {
        println!(
            "   {:<10} {:>12.6} {:>12.6} {:>12.6} {:>12.6}",
            c.name, z[i], pr.x[i], pr.y[i], pr.k[i]
        );
    }
    println!();

    // -- Mass-balance check: z_i = (1-beta) x_i + beta y_i. -------------------
    println!(" Mass balance   z_i = (1-beta) x_i + beta y_i :");
    let mut max_residual = 0.0_f64;
    for (i, c) in components.iter().enumerate() {
        let recon = (1.0 - pr.beta) * pr.x[i] + pr.beta * pr.y[i];
        let residual = (recon - z[i]).abs();
        max_residual = max_residual.max(residual);
        println!(
            "   {:<10} recon = {:>12.9}   z = {:>8.6}   |residual| = {:.2e}",
            c.name, recon, z[i], residual
        );
    }
    println!(" Max mass-balance residual = {max_residual:.2e}  (should be ~0)");

    // Sum-to-one checks.
    let sum_x: f64 = pr.x.iter().sum();
    let sum_y: f64 = pr.y.iter().sum();
    println!(" Sum x_i = {sum_x:.9}   Sum y_i = {sum_y:.9}   (should be 1)");

    // K-ordering: light (methane) must be more volatile than heavy (ethane).
    println!();
    if pr.k[0] > 1.0 && pr.k[1] < 1.0 {
        println!(
            " K-ordering OK: K_methane = {:.4} > 1 > {:.4} = K_ethane  (light more volatile)",
            pr.k[0], pr.k[1]
        );
    } else {
        println!(" WARNING: unexpected K-ordering {:?}", pr.k);
    }

    // -- Cross-model comparison (Ideal/Wilson and SRK). -----------------------
    println!();
    println!("----------------------------------------------------------------------");
    println!(" Cross-model comparison (same feed, T, P)");
    println!("----------------------------------------------------------------------");
    for model in [
        PropertyPackageModel::Ideal,
        PropertyPackageModel::PengRobinson,
        PropertyPackageModel::Srk,
    ] {
        match model.flash_pt(&components, &z, t, p) {
            Ok(fr) => println!(
                "   {:<14} beta = {:.6}   K = [{:.4}, {:.4}]   iters = {}",
                format!("{model:?}"),
                fr.beta,
                fr.k[0],
                fr.k[1],
                fr.iterations
            ),
            Err(e) => println!("   {model:<14?} FAILED: {e}"),
        }
    }
    println!("======================================================================");
    println!(" Note: verification result (EOS-consistent, mass-balance-closing), NOT");
    println!(" validated against experimental VLE. k_ij = 0; no phase-stability test.");
    println!("======================================================================");
}
