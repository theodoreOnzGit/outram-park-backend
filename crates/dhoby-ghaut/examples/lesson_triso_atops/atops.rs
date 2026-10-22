//! The calls behind chapter 5, "Integrating it in TRISO-ATOPS" (Fission
//! products, lessons 39-46): boon-lay's port of INL's TRISO-ATOPS
//! (`boon_lay::triso_atops_fork`, upstream de374c8, MIT; code-to-code
//! verified in `crates/boon-lay/docs/triso-atops-code-to-code.md`), with the
//! plant constants and failure fractions of the TRISO-ATOPS paper's MHTGR
//! Case A as `crates/boon-lay/verification_and_validation/mhtgr_stoyer/
//! constants.csv` records them (Stoyer, Raichart & Petti, Nucl. Technol.
//! 2026, Tables 2 and 3, printed p. 7). Public API only. Shared by `main.rs`
//! and `tests/lesson_triso_atops.rs` through `#[path]`.
#![allow(dead_code)]

use boon_lay::triso_atops_fork::accident::{coolant_release, mean_temperature_rate, release_activity, AccidentFractions, NormalOperationNode, ReleaseMaterial as AccMaterial};
use boon_lay::triso_atops_fork::activities::live_pools::{step as pools_step, PoolRates, PrimaryPools};
use boon_lay::triso_atops_fork::activities::source_terms::FailureFractions;
use boon_lay::triso_atops_fork::activities::becquerels_from_curies;
use boon_lay::triso_atops_fork::diffusion::{diffusion_coefficient, diffusion_coefficient_sic_ag, integrate_diffusion_over_time, DiffusionMaterial};
use boon_lay::triso_atops_fork::normal_operation::{normal_operation_node, NodalActivities, NodeState, ParentPools, PlantConstants};
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::{find_nuclide, supported_nuclides};
use boon_lay::triso_atops_fork::nuclide_model::{ElementGroup, TrisoAtopsNuclide};
use boon_lay::triso_atops_fork::release_models::steady_state::{attenuation_factor, booth_longlived, breakthrough_model, BOOTH_SERIES_TERMS};
use boon_lay::triso_atops_fork::release_models::transient::booth_transient;
use boon_lay::triso_atops_fork::release_models::{rb_fail, release_fraction_transient, ReleaseMaterial};
use boon_lay::triso_atops_fork::run_file::TimeUnit;
use boon_lay::triso_atops_fork::run_selection::{select_nuclides, ParentDecayPolicy};
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Frequency, Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::pressure::kilopascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{hour, second};

pub fn temp(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}
pub fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
/// Years as upstream counts them (a 365-day year, `TimeUnit::Year`).
pub fn years(y: f64) -> Time {
    TimeUnit::Year.to_time(y)
}

/// MHTGR Case A plant constants (constants.csv; paper Table 3, p. 7).
pub fn case_a() -> PlantConstants {
    PlantConstants {
        k_plate: Frequency::new::<hertz>(7.5e-5),
        k_clean: Frequency::new::<hertz>(8.77e-5),
        graphite_thickness: m(4.5e-3),
        grain_size: m(1.0e-5),
        sic_thickness: m(3.5e-5),
        kernel_radius: m(2.13e-4),
        run_time: years(40.0),
        irradiation_time: years(3.0),
        variant: boon_lay::triso_atops_fork::TrisoAtopsVariant::Upstream,
    }
}
/// MHTGR Case A failure fractions (constants.csv; paper Table 2, p. 7).
pub fn fractions_a() -> FailureFractions {
    FailureFractions { heavy_metal: 2.0e-5, sic: 1.0e-4, incremental: 2.3e-5, incremental_sic: 3.6e-5 }
}
pub fn accident_fractions_a() -> AccidentFractions {
    AccidentFractions { heavy_metal: 2.0e-5, sic: 1.0e-4, incremental: 2.3e-5, incremental_sic: 3.6e-5, incremental_accident: 6.6e-5, incremental_sic_accident: 1.7e-4 }
}

pub fn group_name(g: ElementGroup) -> &'static str {
    match g {
        ElementGroup::NobleGas => "NobleGas",
        ElementGroup::Halogen => "Halogen",
        ElementGroup::SpecialMetal => "SpecialMetal",
        ElementGroup::Silver => "Silver",
        ElementGroup::Other => "Other",
    }
}

/// The 84 nuclides with their group and the port's short-lived flag at Case
/// A's 3-year irradiation (`select_nuclides`, upstream's ratio 0.2).
pub fn nuclides() -> Vec<(TrisoAtopsNuclide, bool)> {
    let all = supported_nuclides();
    let names: Vec<&str> = all.iter().map(|n| n.name).collect();
    let (sel, skipped) = select_nuclides(&names, case_a().irradiation_time, None, ParentDecayPolicy::default());
    assert!(skipped.is_empty(), "every database name selects: {skipped:?}");
    sel.into_iter().map(|s| (s.nuclide, s.short_lived)).collect()
}

/// Kernel and graphite D [m^2/s] of element `z` at `t_c` (both at `t_c`).
pub fn d(z: u32, t_c: f64) -> (f64, f64) {
    let k = diffusion_coefficient(z, temp(t_c), temp(t_c));
    (k.kernel.get::<square_meter_per_second>(), k.graphite.get::<square_meter_per_second>())
}
pub fn d_sic_ag(t_c: f64) -> f64 {
    diffusion_coefficient_sic_ag(temp(t_c)).get::<square_meter_per_second>()
}

/// The highest temperature on `grid` (ascending) below which `f` is
/// constant (equal to its value at the grid's first point): the clamp, read
/// from the output alone. `None` if `f` changes from the first step.
pub fn flat_below(grid: &[f64], f: impl Fn(f64) -> f64) -> Option<f64> {
    let v0 = f(grid[0]);
    let mut last = None;
    for &t in grid {
        if f(t) == v0 {
            last = Some(t);
        } else {
            break;
        }
    }
    last.filter(|&t| t > grid[0])
}

/// `rb_fail` for a nuclide at `t_c` with Case A's constants: the release
/// model the port picks by group.
pub fn rb(n: &TrisoAtopsNuclide, short_lived: bool, t_c: f64) -> f64 {
    let p = case_a();
    let (dk, _) = d(n.z, t_c);
    rb_fail(n.z, short_lived, n.decay_constant(), temp(t_c), p.irradiation_time, p.grain_size, p.sic_thickness, p.kernel_radius, DiffusionCoefficient::new::<square_meter_per_second>(dk)).get::<ratio>()
}

/// The Booth equivalent-sphere radius the port uses: sqrt(2 a_grain r)
/// (as in `rb_fail`), with Case A's constants [m].
pub fn a_booth() -> f64 {
    let p = case_a();
    (p.grain_size.get::<meter>() * 2.0 * p.kernel_radius.get::<meter>()).sqrt()
}

/// `booth_longlived` at reduced time x = D t / a^2 (a = 1 m, t = 1 s).
pub fn booth_long(x: f64) -> f64 {
    booth_longlived(DiffusionCoefficient::new::<square_meter_per_second>(x), Time::new::<second>(1.0), m(1.0)).get::<ratio>()
}
/// `booth_transient` on the same reduced variable.
pub fn booth_trans(x: f64) -> f64 {
    booth_transient(Ratio::new::<ratio>(x)).get::<ratio>()
}
pub fn booth_terms() -> usize {
    BOOTH_SERIES_TERMS
}

/// Silver breakthrough through Case A's SiC at `t_c` over the 3-year
/// irradiation (no decay scaling).
pub fn breakthrough(t_c: f64) -> f64 {
    let p = case_a();
    breakthrough_model(diffusion_coefficient_sic_ag(temp(t_c)), p.irradiation_time, p.sic_thickness, p.kernel_radius).get::<ratio>()
}

/// Graphite attenuation factor for element `z` at `t_c` after `t` in Case
/// A's 4.5 mm graphite.
pub fn attenuation(z: u32, t_c: f64, t: Time) -> f64 {
    let (_, dg) = d(z, t_c);
    attenuation_factor(DiffusionCoefficient::new::<square_meter_per_second>(dg), t, case_a().graphite_thickness).get::<ratio>()
}

/// One node in normal operation, 1 Ci of inventory, at `t_c` fuel and
/// graphite temperature, with `fractions` and the clean-up system as asked.
pub fn node(n: &TrisoAtopsNuclide, short_lived: bool, t_c: f64, fractions: FailureFractions, hps: bool) -> NodalActivities {
    normal_operation_node(n, short_lived, becquerels_from_curies(1.0), fractions, case_a(), NodeState { core_temperature: temp(t_c), graphite_temperature: temp(t_c) }, hps, ParentPools::none())
}

/// boon-lay's live pools (NOT a port) stepped from empty for `t_s` seconds
/// in `n` steps with source `s` [atoms/s] and the given rates [1/s]; returns
/// (circulating, plate-out, clean-up, leaked).
pub fn live(s: f64, decay: f64, plate: f64, clean: f64, leak: f64, t_s: f64, n: usize) -> (f64, f64, f64, f64) {
    let rates = PoolRates { decay, plate_out: plate, clean_up: clean, leak };
    let mut p = PrimaryPools { circulating: 0.0, plate_out: 0.0, clean_up: 0.0, leaked: 0.0 };
    for _ in 0..n {
        p = pools_step(p, s, rates, t_s / n as f64).0;
    }
    (p.circulating, p.plate_out, p.clean_up, p.leaked)
}

/// An accident at one node: the time samples [h], temperatures [°C], the
/// kernel and graphite release fractions from `release_fraction_transient`
/// on the trapezoidal integral of D, and the venting fraction from
/// `coolant_release` (defined only at samples where the core heats; `None`
/// elsewhere).
pub struct Accident {
    pub hours: Vec<f64>,
    pub t_c: Vec<f64>,
    pub rf_kernel: Vec<f64>,
    pub rf_graphite: Vec<f64>,
    pub vent: Vec<Option<f64>>,
}
pub fn accident(n: &TrisoAtopsNuclide, hours: &[f64], t_c: &[f64]) -> Accident {
    let p = case_a();
    let times: Vec<Time> = hours.iter().map(|&h| Time::new::<hour>(h)).collect();
    let temps: Vec<ThermodynamicTemperature> = t_c.iter().map(|&t| temp(t)).collect();
    let group = n.element_group();
    let ik = integrate_diffusion_over_time(n.z, &times, &temps, DiffusionMaterial::Kernel);
    let ig = integrate_diffusion_over_time(n.z, &times, &temps, DiffusionMaterial::Graphite);
    let rf_kernel = ik.iter().map(|i| release_fraction_transient(n.z, group, *i, p.kernel_radius, Some(p.sic_thickness), ReleaseMaterial::Kernel).get::<ratio>()).collect();
    let rf_graphite = ig.iter().map(|i| release_fraction_transient(n.z, group, *i, p.graphite_thickness, None, ReleaseMaterial::Graphite).get::<ratio>()).collect();
    let dtdt = mean_temperature_rate(&times, &[temps.clone()]);
    let (frac, vt) = coolant_release(&times, &dtdt, &temps, Pressure::new::<kilopascal>(101.325));
    let mut vent = vec![None; times.len()];
    for (f, t) in frac.iter().zip(vt.iter()) {
        let i = times.iter().position(|x| x == t).expect("a venting time is a sample time");
        vent[i] = Some(*f);
    }
    Accident { hours: hours.to_vec(), t_c: t_c.to_vec(), rf_kernel, rf_graphite, vent }
}

/// The atoms available for accident release at the node of `node`
/// (`release_activity`), for kernel release fraction `rf`.
pub fn released_atoms(n: &TrisoAtopsNuclide, act: &NodalActivities, rf: f64) -> f64 {
    let inv_atoms = becquerels_from_curies(1.0).get::<hertz>() / n.decay_constant().get::<hertz>();
    let nd = NormalOperationNode {
        kernel_inventory_atoms: inv_atoms,
        release_rate: act.release_rate,
        source_rate: act.source_rate,
        graphite_activity: act.graphite_activity,
        circulating_activity: act.circulating_activity,
        plate_out_activity: act.plate_out_activity,
        clean_up_activity: act.clean_up_activity,
    };
    release_activity(n.element_group(), accident_fractions_a(), nd, rf, false, AccMaterial::Kernel, false, n.z)
}

/// The same ILLUSTRATIVE heat-up as chapter 4's lesson 36 (prescribed, not
/// a plant calculation): from `t0` up to `peak` over 50 h, held 50 h, down to
/// `t0` over 200 h, sampled every 5 h.
pub fn heatup(t0: f64, peak: f64) -> (Vec<f64>, Vec<f64>) {
    let mut h = vec![];
    let mut t = vec![];
    for i in 0..=60 {
        let hh = 5.0 * i as f64;
        let tc = if hh <= 50.0 { t0 + (peak - t0) * hh / 50.0 } else if hh <= 100.0 { peak } else { peak + (t0 - peak) * (hh - 100.0) / 200.0 };
        h.push(hh);
        t.push(tc);
    }
    (h, t)
}

pub fn nuclide(name: &str) -> TrisoAtopsNuclide {
    find_nuclide(name).expect("in the TRISO-ATOPS table")
}
