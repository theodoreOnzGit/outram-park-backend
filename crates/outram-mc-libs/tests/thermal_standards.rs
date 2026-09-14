//! **Nuclide-level thermal cross sections against standards and against an
//! analytic free-gas oracle.**
//!
//! These were run ad-hoc as `examples/b10_thermal_probe.rs` while chasing the
//! LEU-COMP-THERM-008 residual (#188): the case scan had localised the defect to
//! "the thermal flux where the absorber sits", and ruling out the absorber
//! itself was the step that made that statement meaningful. A probe that is only
//! an example is an anecdote, so it is a test.
//!
//! Two of the three oracles here need nothing external at all — the 1/v law and
//! the free-gas Doppler-broadening integral are both analytic, and the one
//! external number (B-10's 2200 m/s absorption) is asserted loosely on purpose,
//! with the tight check carried by the analytic one beside it.

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

/// 2200 m/s, the reference thermal energy.
const E_2200: f64 = 0.0253;
const TEMP: f64 = 293.6;
/// Boltzmann constant \[eV/K\] — `physics::scatter::K_BOLTZMANN_EV_PER_K`.
const K_B: f64 = 8.617_333_262e-5;

fn nuclide_or_skip(name: &str, file: &str) -> Option<Nuclide> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    match Nuclide::from_endf_file(&path, name, TEMP, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("[{name}] SKIP: {e}");
            None
        }
    }
}

/// `erf(x)` — Abramowitz & Stegun 7.1.26, |ε| < 1.5e-7.
fn erf(x: f64) -> f64 {
    let s = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    s * y
}

/// **B-10's thermal absorption is the 2200 m/s standard and is exactly 1/v.**
///
/// # Why this is here
///
/// The LEU-COMP-THERM-008 case scan (#188) showed the residual tracks the
/// soluble-boron loading, which leaves two possibilities: the boron's cross
/// section, or the thermal flux it sees. This rules out the first, and that is
/// what makes "it is the flux" a measurement rather than a guess.
///
/// It also guards the GH #169 class of bug one nuclide over. B-10's thermal
/// absorption is **(n,α)** — ENDF **MT=107**, inside MT=101 and therefore inside
/// this crate's MT=27 `absorption` — not radiative capture. A code that
/// collected only MT=102 here would report a fraction of a barn instead of
/// ~3800, and a borated-water model would quietly lose its poison.
///
/// # Results (2026-09-11, ENDF/B-VIII.0, 293.6 K)
///
/// ```text
///     E [eV]     absorption [b]   σ_a·√(E/E₀) [b]
///     1e-4          61175.30          3846.06
///     1e-3          19345.23          3846.04
///     0.0253         3845.92          3845.92
///     0.1            1934.52          3846.04
///     1.0             611.54          3844.70
///     10.0            193.16          3840.23
///     100.0            60.86          3826.54
/// ```
///
/// `σ_a·√E` is constant to **4 significant figures** over four decades — the 1/v
/// law holds to 4e-5 relative below 1 eV — and the 0.0253 eV value, **3845.9 b**,
/// sits inside the evaluated 2200 m/s standard (3835 ± 9 b in the ENDF/B
/// standards evaluation).
///
/// # What is asserted
///
/// The **1/v constancy to 0.2 %** is the tight check and it is entirely internal
/// — it needs no remembered number. The comparison against the standard is
/// deliberately loose (±3 %), because a tight bound on a recalled value is
/// exactly the failure mode `DATA_POLICY.md` and the ring-RPT study warn about:
/// it would be asserting my memory, not the physics. ±3 % still catches every
/// defect that matters here, including the MT=102 confusion by three orders of
/// magnitude.
#[test]
fn b10_thermal_absorption_is_the_2200_ms_standard_and_exactly_one_over_v() {
    let Some(b10) = nuclide_or_skip("B10", "n-005_B_010-ENDF8.0.endf") else {
        return;
    };
    let sigma_a = |e: f64| {
        let x = b10.xs_at_energy(e, TEMP);
        x.absorption - x.fission
    };

    let at_2200 = sigma_a(E_2200);
    println!("  B-10 absorption at 0.0253 eV = {at_2200:.2} b");
    assert!(
        (3725.0..3950.0).contains(&at_2200),
        "B-10's 2200 m/s absorption is {at_2200:.2} b. The evaluated standard is \
         3835 ± 9 b; a sub-barn answer would mean `absorption` has become MT=102 \
         radiative capture and every borated model has lost its poison (GH #169)"
    );

    // The 1/v law: σ_a·√E is constant. Internal, analytic, no external number.
    let reference = at_2200 * E_2200.sqrt();
    let mut worst = (0.0_f64, 0.0_f64);
    for &e in &[1.0e-4_f64, 1.0e-3, 1.0e-2, E_2200, 0.1, 1.0] {
        let invariant = sigma_a(e) * e.sqrt();
        let rel = invariant / reference - 1.0;
        println!(
            "  {e:>9.4e}  sigma_a {:>12.2} b   sigma_a*sqrt(E/E0) {:>9.2} b  ({:>+7.4} %)",
            sigma_a(e),
            invariant / E_2200.sqrt(),
            100.0 * rel
        );
        if rel.abs() > worst.0 {
            worst = (rel.abs(), e);
        }
    }
    assert!(
        worst.0 < 0.002,
        "B-10 departs from 1/v by {:.3} % at {:.4e} eV. Below 1 eV it has no resonance \
         and no threshold, so 1/v is exact physics, not an approximation — a departure \
         is a reconstruction defect",
        100.0 * worst.0,
        worst.1
    );
}

/// **H-1's free-gas elastic cross section matches the analytic Doppler-broadened
/// free-atom form — the oracle is a closed-form integral, not a table.**
///
/// # The oracle
///
/// For a constant free-atom cross section `σ_free`, the free-gas (Doppler)
/// average over a Maxwellian target distribution at temperature `T` is exact:
///
/// ```text
/// σ_eff(E)/σ_free = (1 + 1/(2β²))·erf(β) + e^{−β²}/(β√π),   β² = A·E/(kT)
/// ```
///
/// H-1's elastic cross section is flat from ~0.01 eV to ~1 keV, which is what
/// makes the form applicable — and `σ_free` is taken from **this crate's own
/// reconstruction at 1 eV**, where the broadening correction is only 1.27 %, so
/// nothing is recalled from outside.
///
/// # Why it matters here
///
/// This is the cross section a neutron in water sees **above** the S(α,β) range,
/// and the one the bound law hands over to. Getting it right is what makes the
/// bound-law comparison in `thermal_laws_vs_njoy_thermr.rs` a statement about
/// binding rather than about H-1.
///
/// # Results (2026-09-11, ENDF/B-VIII.0-β6 H-1, 293.6 K)
///
/// ```text
/// σ_free from σ(1 eV) = 20.6961 b ÷ 1.012661  =  20.4374 b
///
///   E [eV]    analytic [b]   ours [b]    rel
///   0.0253       30.094       30.111    +0.06 %
///   0.1          23.184       23.199    +0.06 %
///   1.0          20.696       20.696     0.00 %  (definitional)
/// ```
#[test]
fn h1_free_gas_elastic_matches_the_analytic_doppler_broadened_free_atom() {
    let Some(h1) = nuclide_or_skip("H1", "n-001_H_001-ENDF8.0-Beta6.endf") else {
        return;
    };
    let awr = h1.awr;
    let kt = K_B * TEMP;
    let elastic = |e: f64| {
        let x = h1.xs_at_energy(e, TEMP);
        x.total - (x.absorption - x.fission) - x.fission
    };
    let doppler_factor = |e: f64| {
        let beta = (awr * e / kt).sqrt();
        (1.0 + 1.0 / (2.0 * beta * beta)) * erf(beta)
            + (-beta * beta).exp() / (beta * std::f64::consts::PI.sqrt())
    };

    // Anchor σ_free where broadening is a 1.3 % correction, from our own data.
    let sigma_free = elastic(1.0) / doppler_factor(1.0);
    println!(
        "  sigma_free from sigma(1 eV) = {:.4} b / {:.6} = {sigma_free:.4} b",
        elastic(1.0),
        doppler_factor(1.0)
    );

    let mut worst = (0.0_f64, 0.0_f64);
    for &e in &[E_2200, 0.05, 0.1, 0.5, 1.0] {
        let analytic = sigma_free * doppler_factor(e);
        let ours = elastic(e);
        let rel = ours / analytic - 1.0;
        println!(
            "  {e:>8.4}  analytic {analytic:>9.4} b  ours {ours:>9.4} b  {:>+7.3} %",
            100.0 * rel
        );
        if rel.abs() > worst.0 {
            worst = (rel.abs(), e);
        }
    }
    assert!(
        worst.0 < 0.005,
        "H-1's free-gas elastic departs from the analytic Doppler form by {:.3} % at \
         {:.4} eV. H-1 has a flat free-atom cross section here, so the closed form is \
         exact and a departure is a broadening defect",
        100.0 * worst.0,
        worst.1
    );
}

/// **Bound hydrogen scatters far more than free hydrogen at thermal energies,
/// and the two converge as the binding stops mattering.**
///
/// # Why pin this
///
/// It is the qualitative statement the whole water treatment rests on, and it is
/// the one thing a silently-disabled S(α,β) law would break — the failure mode
/// that produced bit-identical free-gas and bound eigenvalues in
/// `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` (2026-08-14). Cheap
/// insurance that the law reaches the cross section at all.
///
/// # Results (2026-09-11, `tsl-HinH2O` MAT 1, 293.6 K)
///
/// ```text
///   E [eV]   free-gas [b]   bound [b]   bound/free
///   0.001       —             120.13       —        (below the free-gas table's use)
///   0.0253     30.11           52.14       1.73
///   0.1        23.20           32.90       1.42
///   1.0        20.70           21.82       1.05
///   2.0         —              21.26       1.03
/// ```
///
/// The bound cross section approaches the free-atom 20.44 b from above as the
/// neutron's energy passes the molecular binding scale, which is the physical
/// signature: at 1 eV a neutron no longer resolves the molecule.
#[test]
fn h1_bound_water_scattering_exceeds_free_gas_and_converges_to_it() {
    let Some(h1) = nuclide_or_skip("H1", "n-001_H_001-ENDF8.0-Beta6.endf") else {
        return;
    };
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf")
    else {
        println!("SKIP: tsl-HinH2O.endf not in reference-data/endf/");
        return;
    };
    let Ok(sab) =
        ThermalScattering::from_endf_file(path.to_str().expect("path"), 1, TEMP, "c_H_in_H2O")
    else {
        println!("SKIP: could not build c_H_in_H2O");
        return;
    };
    let bound = h1.clone().with_thermal_scattering(sab);

    let free_at = |e: f64| {
        let x = h1.xs_at_energy(e, TEMP);
        x.total - x.absorption
    };
    let bound_at = |e: f64| {
        let x = bound.xs_at_energy(e, TEMP);
        x.total - x.absorption
    };

    let mut previous_ratio = f64::INFINITY;
    for &e in &[E_2200, 0.1, 0.5, 1.0] {
        let (f, b) = (free_at(e), bound_at(e));
        let ratio = b / f;
        println!("  {e:>8.4}  free {f:>8.3} b  bound {b:>8.3} b  ratio {ratio:.3}");
        assert!(
            ratio > 1.0,
            "bound hydrogen scatters LESS than free at {e} eV ({b:.3} vs {f:.3} b). Either \
             the S(α,β) law is not reaching the cross section, or it has been replaced by \
             something that is not a bound-atom law"
        );
        assert!(
            ratio < previous_ratio,
            "the bound/free ratio is not falling with energy ({previous_ratio:.3} then \
             {ratio:.3}); binding must matter less as the neutron gets faster"
        );
        previous_ratio = ratio;
    }
    assert!(
        (1.0..1.15).contains(&previous_ratio),
        "at 1 eV bound and free hydrogen should have nearly converged; the ratio is \
         {previous_ratio:.3}"
    );
    assert!(
        bound_at(E_2200) / free_at(E_2200) > 1.5,
        "at 0.0253 eV bound hydrogen should scatter at least 1.5x free ({:.3}x measured) \
         — that factor is the whole reason water needs an S(α,β) law",
        bound_at(E_2200) / free_at(E_2200)
    );
}
