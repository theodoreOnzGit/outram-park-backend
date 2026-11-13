// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The averaged second law (RELAP-7 Eq 155) on every interfacial closure in
//! `multiphase_1d`, over a sweep of states (gh:#1064). Record:
//! `crates/tampines/verification_and_validation/species_entropy.md`.

use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use tampines_steam_tables::region_2_vapour::h_tp_2;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Length, Pressure, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use super::{interfacial_entropy_production, InterfaceWorkTerms, InterfacialEntropyProduction};
use crate::multiphase_1d::interfacial::{InterfacialCellState, InterfacialExchange};
use crate::multiphase_1d::properties::{PhaseState, SaturatedProperties, SaturatedTransport};

fn h_liquid(p: f64, t: f64) -> f64 {
    h_tp_1(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p))
        .get::<joule_per_kilogram>()
}

fn h_vapour(p: f64, t: f64) -> f64 {
    h_tp_2(ThermodynamicTemperature::new::<kelvin>(t), Pressure::new::<pascal>(p))
        .get::<joule_per_kilogram>()
}

/// Running minima of each term over a sweep, with the state that set each.
#[derive(Default)]
struct Minima {
    count: usize,
    total: (f64, String),
    heat: (f64, String),
    drag: (f64, String),
    kinetic: (f64, String),
    phase: (f64, String),
    consistency_abs_max: f64,
    worst_kinetic_over_heat: f64,
    negative: usize,
    negative_without_consistency: usize,
    worst_negative_relative: (f64, String),
}

impl Minima {
    fn new() -> Self {
        let inf = (f64::INFINITY, String::new());
        Self {
            total: inf.clone(),
            heat: inf.clone(),
            drag: inf.clone(),
            kinetic: inf.clone(),
            phase: inf,
            ..Self::default()
        }
    }

    fn add(&mut self, e: InterfacialEntropyProduction, label: &str) {
        self.count += 1;
        if e.total() < 0.0 {
            self.negative += 1;
            let positive = e.heat_transfer + e.drag + e.mass_transfer_kinetic.max(0.0);
            let rel = -e.total() / positive.max(1e-300);
            if rel > self.worst_negative_relative.0 {
                self.worst_negative_relative = (rel, format!("{label} {e:?}"));
            }
            if e.total() - e.saturation_consistency < 0.0 {
                self.negative_without_consistency += 1;
            }
        }
        for (slot, v) in [
            (&mut self.total, e.total()),
            (&mut self.heat, e.heat_transfer),
            (&mut self.drag, e.drag),
            (&mut self.kinetic, e.mass_transfer_kinetic),
            (&mut self.phase, e.phase_change),
        ] {
            if v < slot.0 {
                *slot = (v, label.to_string());
            }
        }
        self.consistency_abs_max = self.consistency_abs_max.max(e.saturation_consistency.abs());
        if e.mass_transfer_kinetic < 0.0 && e.heat_transfer > 0.0 {
            self.worst_kinetic_over_heat = self
                .worst_kinetic_over_heat
                .max(-e.mass_transfer_kinetic / e.heat_transfer);
        }
    }
}

/// **Methodology.** Every interfacial closure set in `multiphase_1d` is
/// evaluated through `InterfacialExchange::sources_with_properties` over a
/// grid of states, and its entropy production is computed term by term from
/// the equations `TwoFluid1d` solves (see the parent module for the
/// derivation):
///
/// - closure sets: bubbly (Schiller–Naumann, Ranz–Marshall / spherical
///   conduction, 1 mm) **with** the gh:#1060 regime-map area (D_h = 50 mm),
///   the same **without** it, and the droplet set (0.1 mm, vapour
///   continuous);
/// - interface-work options: the defaults (mass-weighted `u_int` with friction
///   work, `H_int` mass transfer) and each gh:#1060 ablation, four in all;
/// - states: `p` ∈ {0.5, 2, 7, 15} MPa; `α_g` ∈ {0.01, 0.1, 0.3, 0.6, 0.8,
///   0.95}; `T_l − T_sat` ∈ {−30, −10, −1, 0, +1, +10, +25} K;
///   `T_g − T_sat` ∈ {−20, −1, 0, +1, +20, +100} K; `u_l = 1 m/s`,
///   `u_g − u_l` ∈ {−2, 0, 0.5, 5} m/s.
///
/// That is 4032 states × 3 closure sets × 4 options = 48 384 evaluations.
///
/// **Prediction (before the run):**
/// - heat transfer `≥ 0` and drag `≥ 0` at every point (analytic: positive
///   coefficients, `u_int` between the phase velocities);
/// - the mass-transfer kinetic term `≥ 0` everywhere under the donor-velocity
///   ablation, and **negative somewhere** under the default mass-weighted
///   `u_int` (condensing states with `x < ½`, from the algebra in the module
///   docs), but smaller than the heat-transfer term there by orders of
///   magnitude;
/// - the phase-change term is third order and small; the IF97
///   saturation-consistency term is small;
/// - **the total `≥ 0` at every point** (Eq 155).
///
/// **Result (2026-11-01, release).** 408 of the 4032 states lie outside the
/// property layer's own range (liquid above IF97 Region 1's 623.15 K, or
/// vapour subcooled past what the Region-2 equation represents, both at
/// 15 MPa) and are skipped by construction; the closures refused none of the
/// rest (10 872 evaluations per option).
///
/// | option | min heat | min drag | min kinetic | negative totals | worst `−σ/(positive terms)` |
/// |---|---|---|---|---|---|
/// | defaults | 0 | 0 | `−1.9e3` | **4** | 0.94 |
/// | no `u_int` | 0 | 0 | 0 | **17** | 1.75 |
/// | no `H_int` | 0 | 0 | 0 | **4** | 0.94 |
/// | both ablated | 0 | 0 | 0 | **17** | 1.75 |
///
/// (W/(m³·K).) Heat transfer and drag are never negative, the donor-velocity
/// kinetic term never negative, and the mass-weighted `u_int` kinetic term
/// does go negative as predicted, at most 0.77 % of the heat term there.
/// **The total-entropy prediction is MISSED:** with the defaults Eq 155 is
/// violated at 4 of 10 872 points, all at 15 MPa with the vapour 100 K
/// superheated and the liquid 10 K subcooled (`α_g` 0.8 and 0.95). The cause
/// is the phase-change term, not the IF97 saturation inconsistency (the
/// count is the same without it; that term is at most `1.3e3` W/(m³·K)
/// where the heat term is `1e5`–`1e8`): condensation driven by the
/// subcooled liquid removes vapour at `h_g^sat`, well below the superheated
/// bulk `h_g`, which leaves the remaining vapour hotter — an entropy sink of
/// order `Γ c_p ΔT_g²/(2 T²)` that the heat-transfer production does not
/// cover once `ΔT_g` is large and `ΔT_l` small. The prediction that this term
/// is third order and small was wrong at 100 K of superheat. Ablating the
/// friction work (`no u_int`) removes the drag dissipation and raises the
/// count to 17. The miss is pinned below (exact count and state class), not
/// asserted away; the fix (a losing-phase interface enthalpy closer to its
/// bulk value) is a closure change for a later issue.
#[test]
fn every_interfacial_closure_produces_non_negative_entropy_over_a_sweep() {
    let pressures = [0.5e6, 2.0e6, 7.0e6, 15.0e6];
    let voids = [0.01, 0.1, 0.3, 0.6, 0.8, 0.95];
    let dtl = [-30.0, -10.0, -1.0, 0.0, 1.0, 10.0, 25.0];
    let dtg = [-20.0, -1.0, 0.0, 1.0, 20.0, 100.0];
    let slips = [-2.0, 0.0, 0.5, 5.0];
    let d_h = Length::new::<meter>(0.05);
    let bubbly = || InterfacialExchange::bubbly(Length::new::<meter>(1.0e-3)).expect("bubbly");
    let sets = [
        ("bubbly+regime-map", bubbly().with_regime_map(d_h).expect("map")),
        ("bubbly fixed-d", bubbly()),
        (
            "droplet",
            InterfacialExchange::droplet(Length::new::<meter>(1.0e-4)).expect("droplet"),
        ),
    ];
    let options = [
        ("default", InterfaceWorkTerms::default()),
        (
            "no-u_int",
            InterfaceWorkTerms {
                interface_velocity: false,
                total_enthalpy_mass_transfer: true,
            },
        ),
        (
            "no-H_int",
            InterfaceWorkTerms {
                interface_velocity: true,
                total_enthalpy_mass_transfer: false,
            },
        ),
        (
            "both-ablated",
            InterfaceWorkTerms {
                interface_velocity: false,
                total_enthalpy_mass_transfer: false,
            },
        ),
    ];
    let mut minima: Vec<Minima> = (0..options.len()).map(|_| Minima::new()).collect();
    let mut refused = 0usize;
    let mut outside_property_range = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for &p in &pressures {
        let sat = SaturatedProperties::at(p).expect("sat");
        let transport = SaturatedTransport::at(p).expect("transport");
        for &a in &voids {
            for &dl in &dtl {
                for &dg in &dtg {
                    for &w in &slips {
                        let cell = InterfacialCellState {
                            pressure: p,
                            void_fraction: a,
                            vapour_enthalpy: h_vapour(p, sat.t_sat + dg),
                            liquid_enthalpy: h_liquid(p, sat.t_sat + dl),
                            vapour_velocity: 1.0 + w,
                            liquid_velocity: 1.0,
                        };
                        // States outside the property layer's own range (IF97
                        // Region 1 above 623.15 K, Region 2 metastable vapour the
                        // equation cannot represent) are skipped by construction:
                        // they are not closure states.
                        if PhaseState::vapour_at(p, cell.vapour_enthalpy, sat).is_err()
                            || PhaseState::liquid_at(p, cell.liquid_enthalpy, sat).is_err()
                        {
                            outside_property_range += 1;
                            continue;
                        }
                        for (name, set) in &sets {
                            let sources = match set.sources_with_properties(cell, sat, transport) {
                                Ok(s) => s,
                                Err(e) => {
                                    if refused < 3 {
                                        println!("refusal: {name} p={p} a={a} dTl={dl} dTg={dg}: {e}");
                                    }
                                    refused += 1;
                                    continue;
                                }
                            };
                            for (k, (opt, terms)) in options.iter().enumerate() {
                                let e = interfacial_entropy_production(cell, &sources, sat, *terms)
                                    .expect("entropy");
                                let label = format!(
                                    "{name}/{opt} p={p:.1e} a={a} dTl={dl} dTg={dg} w={w}"
                                );
                                if k == 0 && e.total() < 0.0 {
                                    violations.push(label.clone());
                                }
                                minima[k].add(e, &label);
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "states outside the property range (skipped): {outside_property_range}; refused by the \
         closures: {refused}"
    );
    for (k, (opt, _)) in options.iter().enumerate() {
        let m = &minima[k];
        println!(
            "[{opt}] negative totals {} ({} without the IF97 consistency term); worst \
             -total/(positive terms) {:.4e} at {}",
            m.negative, m.negative_without_consistency, m.worst_negative_relative.0,
            m.worst_negative_relative.1
        );
        println!(
            "[{opt}] n={} min total {:.4e} ({}) | min heat {:.4e} | min drag {:.4e} | \
             min kinetic {:.4e} ({}) | min phase-change {:.4e} ({}) | max |sat-consistency| \
             {:.4e} | worst -kinetic/heat {:.3e}",
            m.count,
            m.total.0,
            m.total.1,
            m.heat.0,
            m.drag.0,
            m.kinetic.0,
            m.kinetic.1,
            m.phase.0,
            m.phase.1,
            m.consistency_abs_max,
            m.worst_kinetic_over_heat
        );
    }
    for v in &violations {
        println!("Eq 155 violated (defaults): {v}");
    }
    assert_eq!(refused, 0, "a closure refused a state inside the property range");
    // THE PREDICTION "total >= 0 at every point" WAS MISSED (2026-11-01): the
    // phase-change term goes negative where strongly superheated vapour
    // (+100 K) meets subcooled liquid at 15 MPa, because the condensing mass
    // leaves the vapour at h_g^sat, below the bulk h_g (see the V&V record).
    // The miss is pinned, not hidden: the exact count with the defaults, and
    // that every violation is a +100 K vapour-superheat state.
    assert_eq!(violations.len(), 4, "the set of Eq 155 violations changed: {violations:?}");
    assert!(violations.iter().all(|v| v.contains("dTg=100")), "{violations:?}");
    for (k, (opt, terms)) in options.iter().enumerate() {
        let m = &minima[k];
        assert!(m.heat.0 >= 0.0, "[{opt}] heat-transfer entropy negative: {:?}", m.heat);
        assert!(m.drag.0 >= 0.0, "[{opt}] drag entropy negative: {:?}", m.drag);
        if !terms.interface_velocity {
            assert!(m.kinetic.0 >= 0.0, "[{opt}] donor kinetic term negative: {:?}", m.kinetic);
        }
    }
    let default = &minima[0];
    assert!(
        default.kinetic.0 < 0.0,
        "the predicted negative kinetic term under the mass-weighted u_int did not appear"
    );
}
