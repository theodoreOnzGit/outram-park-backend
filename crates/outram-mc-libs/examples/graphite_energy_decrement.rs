//! Mean log energy decrement per collision from the graphite S(alpha,beta)
//! kernel — the number that decides whether the ~+1.7 % k offset vs OpenMC is a
//! scattering-kernel effect.
//!
//! # Why this number
//!
//! Both cross-section sets are already verified against NJOY2016 to ~0.1 %
//! (`u238_vs_njoy_pendf.rs`, `graphite_vs_njoy_thermr.rs`), yet the spectrum
//! differs by several percent and in the *soft* direction. `sigma(E)` sets how
//! often a neutron collides; the kernel sets how much energy it loses. So the
//! discriminating quantity is the mean log energy decrement
//!
//! ```text
//! xi = <ln(E / E')>
//! ```
//!
//! Above the bound-scattering regime this must approach the free-gas value for
//! carbon, which is fixed by kinematics alone:
//!
//! ```text
//! alpha = ((A-1)/(A+1))^2 = (11/13)^2 = 0.715976
//! xi_fg = 1 + alpha*ln(alpha)/(1-alpha) = 0.157811   for A = 12
//! ```
//!
//! A kernel that transfers too much energy per collision — the signature of an
//! over-soft spectrum — shows up here as `xi` above the free-gas asymptote at
//! the top of the thermal range. This needs no external oracle: the asymptote
//! is analytic.
//!
//! Bound effects do not vanish instantly, so `xi` is expected to sit *above*
//! `xi_fg` at thermal energies (up-scatter from the moderator's own motion) and
//! fall toward it as `E` rises well above the phonon scale. What would be
//! damning is `xi` failing to trend toward `xi_fg` at all, or overshooting it by
//! a wide margin near the 4 eV cutoff.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example graphite_energy_decrement
//! ```

fn main() {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::thermal::ThermalScattering;

    const TEMP: f64 = 600.0;
    const N: usize = 200_000;

    let a = 12.0_f64;
    let alpha = ((a - 1.0) / (a + 1.0)).powi(2);
    let xi_fg = 1.0 + alpha * alpha.ln() / (1.0 - alpha);

    let tsl = reference_endf("tsl-crystalline-graphite.endf").expect("graphite tape");
    let g = ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 30, TEMP, "C in graphite")
        .expect("thermal law");

    println!("graphite S(a,b) @ {TEMP} K, {N} samples per energy");
    println!("free-gas carbon asymptote: xi_fg = {xi_fg:.6}  (alpha = {alpha:.6})\n");
    println!(
        "{:>10}  {:>10}  {:>10}  {:>10}  {:>9}",
        "E [eV]", "<E'>/E", "xi", "xi/xi_fg", "up-scat %"
    );

    let mut seed = 20_260_911_u64;
    for &e in &[0.0253, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0, 3.9] {
        let (mut sum_ratio, mut sum_xi, mut n_ok, mut n_up) = (0.0, 0.0, 0usize, 0usize);
        for _ in 0..N {
            if let Some((ep, _mu)) = g.sample(e, &mut seed) {
                if ep > 0.0 {
                    sum_ratio += ep / e;
                    sum_xi += (e / ep).ln();
                    if ep > e {
                        n_up += 1;
                    }
                    n_ok += 1;
                }
            }
        }
        if n_ok == 0 {
            println!("{e:>10.4} {:>11}", "no samples");
            continue;
        }
        let k = n_ok as f64;
        let (ratio, xi) = (sum_ratio / k, sum_xi / k);
        println!(
            "{e:>10.4}  {ratio:>10.5}  {xi:>10.5}  {:>10.3}  {:>8.1}",
            xi / xi_fg,
            100.0 * n_up as f64 / k
        );
    }

    println!(
        "\nxi above xi_fg at thermal energies is expected: the moderator's own\n\
         thermal motion up-scatters, which a free-gas-at-rest kinematic bound\n\
         does not include. The diagnostic is the TREND toward xi_fg as E rises\n\
         past the phonon scale, and whether the approach overshoots."
    );
}
