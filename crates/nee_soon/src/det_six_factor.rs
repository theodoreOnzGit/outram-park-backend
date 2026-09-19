//! Six-factor decomposition of a **deterministic** (diffusion / SP3) solve.
//!
//! # What this is
//!
//! [`outram_mc_libs::prelude::run_keff_reactor_physics`] produces a three-group,
//! leakage-resolved six-factor decomposition (η, f, p, ε, `P_FNL`, `P_TNL`) from
//! a Monte Carlo run. This module produces the **same decomposition** from a
//! deterministic solve, so the two can be compared term by term and mapped
//! against the same parameters.
//!
//! # Why it calls the Monte Carlo crate's assembly instead of its own
//!
//! `p` and `ε` are **not convention-free**. `outram-mc-libs`' own module docs
//! record a case where two differently-defined `p` values looked 8.8 % apart
//! while their *product* differed by 2.2 % — the fingerprint of a definitional
//! mismatch being read as physics. Writing a second set of formulas here
//! against the same six names would rebuild exactly that trap.
//!
//! So this module does **no** six-factor algebra. It forms the group-resolved
//! reaction rates from the deterministic flux and the multigroup cross
//! sections, collapses them onto the same three bands, and hands them to
//! [`outram_mc_libs::prelude::assemble_six_factors`]. One definition, two
//! solvers.
//!
//! # Units and normalisation
//!
//! The six factors are **ratios**, so any consistent flux normalisation gives
//! the same answer — a deterministic eigenvalue solve fixes the flux only up to
//! a constant, and that is sufficient here. Rates are
//! `sum_cells Sigma_x,g,zone(cell) * phi_g,cell * V_cell`, in whatever units
//! the flux carries.
//!
//! A deterministic solve has no statistical uncertainty, so every
//! [`Estimate`] carries `std = 0` and the propagated `1σ` fields are zero.
//! **That is not a claim of accuracy** — it records that the *sampling* error
//! is zero, while the discretisation and condensation errors, which are the
//! large ones here, are not represented at all.

use crate::mgxs::MgxsLibrary;
use outram_mc_libs::prelude::{assemble_six_factors, Estimate, SixFactors};

/// Which of the three bands a fine group falls in: `0 = thermal`,
/// `1 = resonance`, `2 = fast` — the index order
/// [`outram_mc_libs::prelude::SixFactors`] uses.
fn band_of(upper_ev: f64, thermal_cutoff_ev: f64, resonance_upper_ev: f64) -> usize {
    if upper_ev <= thermal_cutoff_ev {
        0
    } else if upper_ev <= resonance_upper_ev {
        1
    } else {
        2
    }
}

/// Group-resolved reaction rates collapsed onto the three six-factor bands.
///
/// Public because a caller mapping a parameter sweep usually wants the rates
/// as well as the factors — the rates are what make a change in `p` or `f`
/// attributable to a band rather than merely observed.
#[derive(Debug, Clone, Default)]
pub struct BandRates {
    /// Absorption rate per band `[thermal, resonance, fast]`.
    pub absorption: [f64; 3],
    /// `nu`-fission production rate per band.
    pub production: [f64; 3],
    /// Thermal absorption restricted to the fuel zones.
    pub thermal_absorption_fuel: f64,
}

/// Collapse a deterministic solve's group fluxes into three-band reaction rates.
///
/// # Arguments
///
/// - `lib` — the multigroup library **in the same group ordering as
///   `group_flux`**. Pass the ascending-energy library with an ascending flux,
///   or the descending one with a descending flux; this function reads the
///   group edges from `lib` and does not reorder anything.
/// - `zone_of_cell` — zone index per mesh cell, as handed to the solver.
/// - `cell_volume` — per-cell volume. A uniform mesh may pass all ones; the
///   factors are ratios and a constant cancels.
/// - `group_flux` — `[group][cell]` scalar flux from the converged solve.
/// - `fuel_zone` — per-zone flag: does this zone contain fuel? Drives `f` and
///   `η`, and getting it wrong silently changes both.
/// - `thermal_cutoff_ev`, `resonance_upper_ev` — the two band edges.
///
/// Groups whose index exceeds `lib`'s group count, and cells whose zone is out
/// of range, are skipped rather than panicking: a truncated or mismatched solve
/// should yield visibly wrong rates, not abort a parameter sweep mid-map.
#[must_use]
pub fn band_rates(
    lib: &MgxsLibrary,
    zone_of_cell: &[usize],
    cell_volume: &[f64],
    group_flux: &[Vec<f64>],
    fuel_zone: &[bool],
    thermal_cutoff_ev: f64,
    resonance_upper_ev: f64,
) -> BandRates {
    let edges = lib.groups.edges();
    let mut out = BandRates::default();

    for (g, flux_g) in group_flux.iter().enumerate() {
        // Upper edge of group g in the library's own ordering.
        let upper = match (edges.get(g), edges.get(g + 1)) {
            (Some(&a), Some(&b)) => a.max(b),
            _ => continue,
        };
        let band = band_of(upper, thermal_cutoff_ev, resonance_upper_ev);

        for (c, &phi) in flux_g.iter().enumerate() {
            let Some(&z) = zone_of_cell.get(c) else {
                continue;
            };
            let Some(zone) = lib.zones.get(z) else {
                continue;
            };
            let v = cell_volume.get(c).copied().unwrap_or(1.0);
            let w = phi * v;

            let sa = zone.absorption.get(g).copied().unwrap_or(0.0);
            let sf = zone.nu_fission.get(g).copied().unwrap_or(0.0);

            out.absorption[band] += sa * w;
            out.production[band] += sf * w;
            if band == 0 && fuel_zone.get(z).copied().unwrap_or(false) {
                out.thermal_absorption_fuel += sa * w;
            }
        }
    }
    out
}

/// The six factors of a deterministic solve, using
/// [`outram_mc_libs::prelude::assemble_six_factors`] — the same assembly the
/// Monte Carlo path uses.
///
/// `leakage_by_group` is `[thermal, resonance, fast]` in the same units as the
/// rates. **A zero-leakage (reflective) solve must pass `[0.0; 3]`**, which
/// makes `P_FNL = P_TNL = 1` exactly; passing a leakage a reflective solve did
/// not have is the single easiest way to make this disagree with a `k_inf`
/// reference for a reason that has nothing to do with the physics.
#[must_use]
pub fn six_factors_from_deterministic(
    rates: &BandRates,
    leakage_by_group: [f64; 3],
    thermal_cutoff_ev: f64,
    resonance_upper_ev: f64,
) -> SixFactors {
    let a = [
        Estimate::exact(rates.absorption[0]),
        Estimate::exact(rates.absorption[1]),
        Estimate::exact(rates.absorption[2]),
    ];
    let p = [
        Estimate::exact(rates.production[0]),
        Estimate::exact(rates.production[1]),
        Estimate::exact(rates.production[2]),
    ];
    let l = [
        Estimate::exact(leakage_by_group[0]),
        Estimate::exact(leakage_by_group[1]),
        Estimate::exact(leakage_by_group[2]),
    ];
    assemble_six_factors(
        a,
        Estimate::exact(rates.thermal_absorption_fuel),
        p,
        l,
        (thermal_cutoff_ev, resonance_upper_ev),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mgxs::{GroupStructure, ZoneMgxs};

    /// A hand-computable two-zone, four-group case.
    ///
    /// # Methodology
    ///
    /// Four groups with edges `[1e-5, 1.0, 1e3, 1e5, 2e7]` eV and bands split at
    /// `thermal <= 1.0 eV` and `resonance <= 1e5 eV`, so groups map
    /// `g0 -> thermal`, `g1, g2 -> resonance`, `g3 -> fast`. Two cells, one per
    /// zone, unit volume, unit flux everywhere. Zone 0 is fuel, zone 1 is not.
    ///
    /// # Result
    ///
    /// Rates are exact sums of the tabulated cross sections, and with zero
    /// leakage `P_FNL` and `P_TNL` are both exactly 1, so
    /// `k_from_factors` must equal `total production / total absorption`. Both
    /// are asserted to 1e-12.
    #[test]
    fn deterministic_six_factors_telescope_to_production_over_absorption() {
        let lib = MgxsLibrary {
            groups: GroupStructure::new(vec![1.0e-5, 1.0, 1.0e3, 1.0e5, 2.0e7])
                .expect("edges ascend"),
            zones: vec![
                ZoneMgxs {
                    name: "fuel".into(),
                    flux: vec![1.0; 4],
                    total: vec![1.0; 4],
                    absorption: vec![0.10, 0.02, 0.01, 0.005],
                    nu_fission: vec![0.20, 0.01, 0.005, 0.02],
                    kappa_fission: vec![0.0; 4],
                    scatter: vec![vec![0.0; 4]; 4],
                    chi: vec![0.0, 0.0, 0.0, 1.0],
                },
                ZoneMgxs {
                    name: "reflector".into(),
                    flux: vec![1.0; 4],
                    total: vec![1.0; 4],
                    absorption: vec![0.004, 0.001, 0.001, 0.001],
                    nu_fission: vec![0.0; 4],
                    kappa_fission: vec![0.0; 4],
                    scatter: vec![vec![0.0; 4]; 4],
                    chi: vec![0.0; 4],
                },
            ],
        };

        let zone_of_cell = [0usize, 1usize];
        let volume = [1.0, 1.0];
        let flux: Vec<Vec<f64>> = (0..4).map(|_| vec![1.0, 1.0]).collect();
        let fuel = [true, false];

        let r = band_rates(&lib, &zone_of_cell, &volume, &flux, &fuel, 1.0, 1.0e5);

        // thermal = g0 over both zones
        assert!((r.absorption[0] - (0.10 + 0.004)).abs() < 1.0e-12);
        // resonance = g1 + g2 over both zones
        assert!((r.absorption[1] - (0.02 + 0.01 + 0.001 + 0.001)).abs() < 1.0e-12);
        // fast = g3
        assert!((r.absorption[2] - (0.005 + 0.001)).abs() < 1.0e-12);
        // thermal absorption in fuel only
        assert!((r.thermal_absorption_fuel - 0.10).abs() < 1.0e-12);

        let sf = six_factors_from_deterministic(&r, [0.0; 3], 1.0, 1.0e5);

        // Zero leakage => both non-leakage probabilities are exactly one.
        assert!((sf.p_fnl.mean - 1.0).abs() < 1.0e-12, "P_FNL = {}", sf.p_fnl.mean);
        assert!((sf.p_tnl.mean - 1.0).abs() < 1.0e-12, "P_TNL = {}", sf.p_tnl.mean);

        let a_tot: f64 = r.absorption.iter().sum();
        let p_tot: f64 = r.production.iter().sum();
        assert!(
            (sf.k_from_factors.mean - p_tot / a_tot).abs() < 1.0e-12,
            "factors telescope to {} but P/A is {}",
            sf.k_from_factors.mean,
            p_tot / a_tot
        );

        // A deterministic solve carries no sampling uncertainty.
        assert_eq!(sf.k_from_factors.std, 0.0);
    }
}
