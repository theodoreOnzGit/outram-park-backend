//! The calls behind chapter 4's particle-failure pages (Fission products,
//! lessons 33-38): **boon-lay fuel failure** (`boon_lay::fuel_failure`), a
//! clean-room, agentic implementation of the equations published in the
//! PANAMA-I report (Verfondern & Nabielek, HTA-IB-03/90). It is not a port:
//! the PANAMA source code was not available. Every number here is boon-lay
//! fuel failure's, never "PANAMA's".
//!
//! The particle is `fuel_failure::htr10::particle`, HTR-10 applied to the
//! model: an EXTRAPOLATION outside the report's 1600-2500 °C German-TRISO
//! range, with two stand-in inputs taken from the report's HTR-Module case
//! (SiC strength and modulus, fast fluence; see that module's docs). The
//! irradiation temperature T_B = 776 °C is the report's HTR-Module average,
//! the value that module sweeps around; it is not HTR-10's.
//!
//! Release from failed particles (lesson 37) uses TRISO-ATOPS's transient
//! Booth (`booth_transient` on the time integral of D/a^2, a = kernel
//! radius) with the kernel D of `diffusion_coefficient`. Public API only.
//! Shared by `main.rs` and `tests/lesson_barriers.rs` through `#[path]`.
#![allow(dead_code)]

use boon_lay::fuel_failure::htr10;
use boon_lay::fuel_failure::{
    advance_action_integral, advance_grain_boundary_exposure, advance_thinning_factor, corroded_weibull_modulus,
    corrosion::corrosion_rate_with, corrosion::FIGURE_PREFACTOR, corrosion::PRINTED_PREFACTOR, corrosion_rate,
    decomposition_rate_constant, dimensionless_time, grain_boundary_corrosion_rate, induced_stress_with_thinning_factor,
    internal_gas_pressure, irradiated_strength, irradiated_weibull_modulus, molar_volume, reduced_diffusion_coefficient,
    released_gas_fraction, thermal_decomposition_failure_fraction, total_failure_fraction, weibull_failure_fraction,
    AccidentHistory, AccidentStep, DecompositionCalibration, FailureProgress, GrainBoundaryCorrosion, ParticleState,
    AS_MANUFACTURED_TARGET,
};
use boon_lay::triso_atops_fork::diffusion::{integrate_diffusion_over_time, DiffusionMaterial};
use boon_lay::triso_atops_fork::release_models::transient::booth_transient;
use uom::si::area::square_meter;
use uom::si::f64::{Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::{meter, micrometer};
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{day, hour, second};
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

/// The irradiation temperature every failure page uses, °C (the report's
/// HTR-Module average, as `htr10`'s docs say; not HTR-10's).
pub const T_B_C: f64 = 776.0;

pub fn temp(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}
pub fn r(x: f64) -> Ratio {
    Ratio::new::<ratio>(x)
}

/// The HTR-10 particle at `T_B_C`, grain-boundary corrosion as asked.
pub fn particle(gb: bool) -> ParticleState {
    let mut p = htr10::particle(temp(T_B_C));
    p.grain_boundary = if gb { GrainBoundaryCorrosion::Enabled } else { GrainBoundaryCorrosion::Disabled };
    p
}

/// The particle at another burnup (FIMA fraction), all else as `particle`.
pub fn particle_at_burnup(fima: f64) -> ParticleState {
    htr10::particle_with(
        temp(T_B_C),
        r(fima),
        Time::new::<day>(htr10::RESIDENCE_FULL_POWER_DAYS),
        htr10::STAND_IN_FLUENCE_E25_PER_M2,
    )
}

/// The end-of-irradiation phi_1 at `T_B_C`: the accident's starting value.
pub fn phi1_start() -> f64 {
    eoi(T_B_C)
}

/// The end-of-irradiation phi_1 at irradiation temperature `t_b_c` °C.
pub fn eoi(t_b_c: f64) -> f64 {
    htr10::end_of_irradiation_failure(temp(t_b_c)).get::<ratio>()
}

/// The geometry the model uses: (r_i, r_a, cube-root mean radius, d_o) [m],
/// kernel volume V_k and free volume V_f [m^3].
pub fn geometry() -> (f64, f64, f64, f64, f64, f64) {
    let l = htr10::sic_layer();
    (
        l.inner_radius().get::<meter>(),
        l.outer_radius().get::<meter>(),
        l.cbrt_mean_radius().get::<meter>(),
        l.initial_thickness().get::<meter>(),
        htr10::kernel_volume().get::<cubic_meter>(),
        htr10::free_volume().get::<cubic_meter>(),
    )
}

/// The pieces of the internal pressure at accident temperature `t_c` after
/// `hours` of accident: (F_d, OPF, p total, p from fission gas alone, p from
/// CO alone) with pressures in Pa. Each piece is `internal_gas_pressure` with
/// the other source set to zero, so the two add to the total exactly (the
/// equation is linear in `F_d F_f + OPF`).
pub fn pressure_parts(p: &ParticleState, t_c: f64, hours: f64) -> (f64, f64, f64, f64, f64) {
    let t = temp(t_c);
    let ds = reduced_diffusion_coefficient(p.diffusion_kernel, t, p.burnup);
    let tau_a = dimensionless_time(ds, Time::new::<hour>(hours));
    let f_d = released_gas_fraction(p.dimensionless_irradiation_time, tau_a);
    let opf = p.oxygen.oxygen_per_fission(t);
    let pr = |fd: Ratio, o: Ratio| {
        internal_gas_pressure(fd, p.stable_gas_yield, o, p.burnup, p.free_volume, p.kernel_volume, molar_volume(p.compound), t)
            .get::<pascal>()
    };
    (f_d.get::<ratio>(), opf.get::<ratio>(), pr(f_d, opf), pr(f_d, r(0.0)), pr(r(0.0), opf))
}

/// The driver's own pressure at the same point (`AccidentHistory::pressure_at`).
pub fn pressure_at(p: &ParticleState, t_c: f64, hours: f64) -> f64 {
    AccidentHistory::new(*p, r(0.0)).pressure_at(Time::new::<hour>(hours), temp(t_c)).get::<pascal>()
}

/// Thin-shell SiC stress [Pa] at pressure `p_pa` and thinning factor `fkor`.
pub fn stress(p_pa: f64, fkor: f64) -> f64 {
    induced_stress_with_thinning_factor(&htr10::sic_layer(), Pressure::new::<pascal>(p_pa), r(fkor)).get::<pascal>()
}

/// Irradiated median strength [Pa] and Weibull modulus at fluence `gamma`
/// (1e25 m^-2 EDN) and `T_B_C`, from the stand-in unirradiated values.
pub fn strength_at(gamma: f64) -> (f64, f64) {
    (
        irradiated_strength(Pressure::new::<megapascal>(htr10::STAND_IN_STRENGTH_MPA), gamma, temp(T_B_C)).get::<pascal>(),
        irradiated_weibull_modulus(htr10::STAND_IN_WEIBULL_MODULUS, gamma, temp(T_B_C)),
    )
}

/// Eq (1): phi_1 at stress `s` [Pa], median strength `s0` [Pa], modulus `m`.
pub fn weibull(s: f64, s0: f64, m: f64) -> f64 {
    weibull_failure_fraction(Pressure::new::<pascal>(s), Pressure::new::<pascal>(s0), m).get::<ratio>()
}

/// FKOR after `hours` at `t_c` °C, advanced in `n` equal steps.
pub fn thinning(t_c: f64, hours: f64, n: usize) -> f64 {
    let d_o = htr10::sic_layer().initial_thickness();
    let dt = Time::new::<hour>(hours / n as f64);
    let mut f = r(1.0);
    for _ in 0..n {
        f = advance_thinning_factor(f, d_o, temp(t_c), dt);
    }
    f.get::<ratio>()
}

/// SiC corrosion rate [m/s] at `t_c` °C: (figure-consistent default, as printed).
pub fn corrosion(t_c: f64) -> (f64, f64) {
    (
        corrosion_rate(temp(t_c)).get::<meter_per_second>(),
        corrosion_rate_with(PRINTED_PREFACTOR, temp(t_c)).get::<meter_per_second>(),
    )
}
pub fn prefactors() -> (f64, f64) {
    (FIGURE_PREFACTOR, PRINTED_PREFACTOR)
}

/// Decomposition: rate constant k [1/s] at `t_c`, and (zeta, phi_2 sphere,
/// phi_2 loose) after `hours`.
pub fn decomposition(t_c: f64, hours: f64) -> (f64, f64, f64, f64) {
    let d_o = htr10::sic_layer().initial_thickness();
    let k = decomposition_rate_constant(d_o, temp(t_c)).get::<hertz>();
    let z = advance_action_integral(r(0.0), d_o, temp(t_c), Time::new::<hour>(hours));
    (
        k,
        z.get::<ratio>(),
        thermal_decomposition_failure_fraction(z, DecompositionCalibration::ParticlesInSphere).get::<ratio>(),
        thermal_decomposition_failure_fraction(z, DecompositionCalibration::LooseParticles).get::<ratio>(),
    )
}

/// Grain-boundary attack: the rate [1/s] at `t_c`, and the corroded Weibull
/// modulus after `hours` from the end-of-irradiation modulus `m0`.
pub fn grain_boundary(t_c: f64, hours: f64, m0: f64) -> (f64, f64) {
    let x = advance_grain_boundary_exposure(r(0.0), temp(t_c), Time::new::<hour>(hours));
    (grain_boundary_corrosion_rate(temp(t_c)).get::<hertz>(), corroded_weibull_modulus(m0, x))
}

/// One row of an accident history.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub hours: f64,
    pub t_c: f64,
    pub fkor: f64,
    pub zeta: f64,
    pub p_pa: f64,
    pub sigma_pa: f64,
    pub phi1: f64,
    pub phi2: f64,
    pub total: f64,
}

/// Step `AccidentHistory` along `(duration h, mean T °C)` intervals from the
/// end-of-irradiation phi_1, recording each step; the pressure and stress
/// are the driver's own `pressure_at` at the step's end and mean temperature.
pub fn run_history(p: &ParticleState, steps: &[(f64, f64)]) -> Vec<Row> {
    let mut h = AccidentHistory::new(*p, r(phi1_start()));
    let mut rows = Vec::with_capacity(steps.len() + 1);
    let push = |rows: &mut Vec<Row>, g: &FailureProgress, t_c: f64, h: &AccidentHistory| {
        let pr = h.pressure_at(g.elapsed, temp(t_c)).get::<pascal>();
        rows.push(Row {
            hours: g.elapsed.get::<hour>(),
            t_c,
            fkor: g.thinning_factor.get::<ratio>(),
            zeta: g.action_integral.get::<ratio>(),
            p_pa: pr,
            sigma_pa: stress(pr, g.thinning_factor.get::<ratio>()),
            phi1: g.pressure_vessel.get::<ratio>(),
            phi2: g.thermal_decomposition.get::<ratio>(),
            total: g.total.get::<ratio>(),
        });
    };
    let g0 = h.progress();
    push(&mut rows, &g0, steps.first().map_or(T_B_C, |s| s.1), &h);
    for &(dh, t_c) in steps {
        let g = h.step(AccidentStep { duration: Time::new::<hour>(dh), mean_temperature: temp(t_c) });
        push(&mut rows, &g, t_c, &h);
    }
    rows
}

/// An isothermal hold at `t_c` for `hours` in `n` steps.
pub fn hold(t_c: f64, hours: f64, n: usize) -> Vec<(f64, f64)> {
    vec![(hours / n as f64, t_c); n]
}

/// An ILLUSTRATIVE heat-up history (prescribed, not from a plant model):
/// linear from `T_B_C` to `peak` over 50 h, held 50 h, then down to
/// `T_B_C` over 200 h, in 5 h steps at each interval's mean temperature.
pub fn ramp(peak: f64) -> Vec<(f64, f64)> {
    let mut s = Vec::new();
    let lin = |a: f64, b: f64, n: usize, out: &mut Vec<(f64, f64)>| {
        for i in 0..n {
            let t0 = a + (b - a) * i as f64 / n as f64;
            let t1 = a + (b - a) * (i + 1) as f64 / n as f64;
            out.push((5.0, 0.5 * (t0 + t1)));
        }
    };
    lin(T_B_C, peak, 10, &mut s);
    lin(peak, peak, 10, &mut s);
    lin(peak, T_B_C, 40, &mut s);
    s
}

/// The total combination, Eq. on page -480-, for the page's phi_o toggle.
pub fn total(phi_o: f64, phi1: f64, phi2: f64) -> f64 {
    total_failure_fraction(r(phi_o), r(phi1), r(phi2)).get::<ratio>()
}
pub fn as_manufactured_target() -> f64 {
    AS_MANUFACTURED_TARGET
}

/// Kernel release fraction by TRISO-ATOPS's transient Booth for element `z`
/// along `rows` (the history's own times and temperatures): the trapezoidal
/// time integral of D (`integrate_diffusion_over_time`, kernel material)
/// divided by the kernel radius squared, then `booth_transient`.
pub fn kernel_release(z: u32, rows: &[Row]) -> Vec<f64> {
    let a = htr10::KERNEL_RADIUS_UM * 1e-6;
    let times: Vec<Time> = rows.iter().map(|w| Time::new::<hour>(w.hours)).collect();
    let temps: Vec<ThermodynamicTemperature> = rows.iter().map(|w| temp(w.t_c)).collect();
    integrate_diffusion_over_time(z, &times, &temps, DiffusionMaterial::Kernel)
        .iter()
        .map(|i| booth_transient(r(i.get::<square_meter>() / (a * a))).get::<ratio>())
        .collect()
}

pub fn kernel_radius_um() -> f64 {
    htr10::KERNEL_RADIUS_UM
}
pub fn sic_um() -> (f64, f64) {
    (htr10::SIC_INNER_RADIUS_UM, htr10::SIC_OUTER_RADIUS_UM)
}
pub fn micrometres(m: f64) -> f64 {
    Length::new::<meter>(m).get::<micrometer>()
}
pub fn seconds(h: f64) -> f64 {
    Time::new::<hour>(h).get::<second>()
}
