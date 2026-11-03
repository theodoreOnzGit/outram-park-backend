//! # V&V: separator-dryer and downcomer volumes (gh:#1073, RELAP-7 §6.8–6.9)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Closed network:** a separated volume between pipes walled at their
//!    far ends conserves mass and total energy to < 1e-12 relative.
//! 2. **Ideal separation at steady state:** IF97 water entering at 7 MPa,
//!    quality 0.2, through a separator whose steam outlet draws saturated
//!    vapour and whose liquid outlet is a port at the bottom (its hydrostatic
//!    head regulates the level). At steady state the steam outlet carries
//!    `x_in` of the inlet flow within 1 %, the outlets balance the inlet's mass
//!    and energy within 0.5 %, the steam stream's enthalpy is `h_g(p)` within
//!    1 %, and the level settles inside the vessel.
//! 3. **Uncovered port:** draining a two-phase downcomer from its bottom, a
//!    port at 0.5 m draws liquid while the level is above 0.5 m and vapour
//!    from the first step it is below (Eq 377's switch), and never otherwise.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::pressure::megapascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

fn end(pipe: usize, end: PipeEnd) -> PipeEndRef {
    PipeEndRef { pipe, end }
}

fn water_pipe(len: f64, n: usize, p_mpa: f64, h: f64) -> HemPipe<Iapws97Hem> {
    let geo = HemPipeGeometry::new(Length::new::<meter>(len), Area::new::<square_meter>(0.01), n).unwrap();
    let s = PrimitiveState::PressureEnthalpy {
        pressure: Pressure::new::<megapascal>(p_mpa),
        specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(geo, Iapws97Hem, HemInitialCondition::Uniform(s), HemNumerics::default()).unwrap()
}

fn mix(p_mpa: f64, x: f64) -> (f64, HemThermoState) {
    let eos = Iapws97Hem;
    let p = Pressure::new::<megapascal>(p_mpa);
    let probe = eos.state_p_h(p, AvailableEnergy::new::<joule_per_kilogram>(2.0e6)).unwrap();
    let (l, g) = eos.saturated_phases(&probe).unwrap();
    let (hf, hg) = (l.specific_enthalpy.value, g.specific_enthalpy.value);
    let h = hf + x * (hg - hf);
    (h, eos.state_p_h(p, AvailableEnergy::new::<joule_per_kilogram>(h)).unwrap())
}

/// **Closed separator network conserves.** Three 0.5 m pipes of 7 MPa,
/// quality 0.2 water on a 0.002 m³ separator (A = 0.01 m², so 0.2 m tall),
/// ports Mixture / Vapour / AtHeight(0), walls at the far ends, 200 steps.
/// **Result (2026-11-01):** mass 0, energy 2.2e-16 (after the two port
/// corrections in the V&V doc; the first two versions failed: the volume
/// drained in four steps, then its specific energy ran away). Held.
#[test]
fn a_closed_separator_network_conserves_mass_and_energy() {
    let (h, st) = mix(7.0, 0.2);
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let a = net.add_pipe(water_pipe(0.5, 5, 7.0, h));
    let b = net.add_pipe(water_pipe(0.5, 5, 7.0, h));
    let c = net.add_pipe(water_pipe(0.5, 5, 7.0, h));
    let sep = net
        .add_separated_volume(
            Volume::new::<cubic_meter>(0.002),
            Area::new::<square_meter>(0.01),
            st,
            vec![
                (end(a, PipeEnd::Outlet), 0.0, VolumePort::Mixture),
                (end(b, PipeEnd::Inlet), 0.0, VolumePort::Vapour),
                (end(c, PipeEnd::Inlet), 0.0, VolumePort::AtHeight(0.0)),
            ],
        )
        .unwrap();
    let (m0, e0) = (net.total_mass().value, net.total_energy().value);
    for _ in 0..200 {
        net.step().unwrap();
    }
    let dm = net.total_mass().value / m0 - 1.0;
    let de = net.total_energy().value / e0 - 1.0;
    let level = net.volume_level(sep).unwrap().unwrap().get::<meter>();
    println!("closed separator: mass {dm:e}, energy {de:e}, level {level:.4} m, ports {:?} {:?} {:?}",
        net.port_draw(sep, 0).unwrap(), net.port_draw(sep, 1).unwrap(), net.port_draw(sep, 2).unwrap());
    assert!(dm.abs() < 1e-12 && de.abs() < 1e-12);
    assert_eq!(net.port_draw(sep, 1).unwrap(), Some(PortDraw::Vapour));
    assert_eq!(net.port_draw(sep, 0).unwrap(), Some(PortDraw::Mixture));
    assert_eq!(net.port_draw(sep, 9).unwrap(), None);
    // Refusals.
    assert!(net
        .add_separated_volume(Volume::new::<cubic_meter>(0.002), Area::new::<square_meter>(0.01), st,
            vec![(end(a, PipeEnd::Inlet), 0.0, VolumePort::AtHeight(0.5))])
        .is_err(), "port above the vessel");
    assert!(net
        .add_separated_volume(Volume::new::<cubic_meter>(0.002), Area::new::<square_meter>(0.0), st,
            vec![(end(a, PipeEnd::Inlet), 0.0, VolumePort::Liquid)])
        .is_err());
    assert_eq!(net.volume_level(7).unwrap(), None, "no such junction");
}

/// **Regression: the `(ρ, e)` inversion finds a valid state without a hint.**
/// Found by the separator test (2026-11-01): ρ = 58.835 kg/m³,
/// e = 2.0532 MJ/kg (7.0 MPa, quality 0.601) failed with no hint, because
/// the bracket expansion leapt from 6.7 MPa to 100 MPa, which the flash
/// rejects. The log-grid fallback, reached only on that failure, now finds
/// it. **Result:** without a hint 6.99999986675244 MPa, with a 7 MPa hint
/// the same to 1e-12 relative.
#[test]
fn the_rho_e_inversion_finds_a_valid_state_without_a_hint() {
    use uom::si::mass_density::kilogram_per_cubic_meter;
    let (rho, e) = (58.83539534028867, 2053164.868173995);
    let f = |hint| {
        Iapws97Hem
            .state_rho_e(MassDensity::new::<kilogram_per_cubic_meter>(rho), AvailableEnergy::new::<joule_per_kilogram>(e), hint)
            .unwrap()
            .pressure
            .get::<megapascal>()
    };
    let (a, b) = (f(None), f(Some(Pressure::new::<megapascal>(7.0))));
    println!("inversion: no hint {a:.14} MPa, hint {b:.14} MPa");
    assert!((a / b - 1.0).abs() < 1e-12);
}

/// **Ideal separation at steady state.** Inlet: a 1 m pipe fed by a
/// time-dependent volume at 7.0 MPa with `h` for quality 0.2. Separator
/// 0.002 m³, A = 0.001 m² (2 m tall), initially at quality 0.2 (level
/// 0.33 m). Steam outlet (`Vapour`): a 1 m pipe to 6.99 MPa. Liquid outlet
/// (`AtHeight(0)`): a 1 m pipe to `6.99 MPa + ρ_f g (0.35 m)`, so the level
/// settles where the head drives the liquid flow. Pipes 0.01 m², 5 cells,
/// IF97 wall friction on. Measured over the last 0.1 s of the run from the
/// cumulative boundary outflows.
/// **Result (2026-11-01), 1 s:** in 3.5185 kg/s, steam 0.7041, liquid 2.8113;
/// split 0.2001 (x_in 0.2, +0.05 %); mass balance −9.0e-4, energy −6.7e-4;
/// steam enthalpy h_g to +0.003 %; level 0.313 m and steady. Held. Protocol
/// history: at 0.3 s and 0.5 s without port losses the run was not steady
/// (mass balance −78 %, −65 %); with K = 1000 on the liquid line its loss at
/// the true flow needed ~7 m of head; K = 20 everywhere settles by ~0.6 s.
/// The criteria were not changed.
#[test]
fn an_ideal_separator_splits_by_quality_at_steady_state() {
    let (h_in, st) = mix(7.0, 0.2);
    let eos = Iapws97Hem;
    let (liq, vap) = eos.saturated_phases(&st).unwrap();
    let rho_f = liq.density.value;
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let a = net.add_pipe(water_pipe(1.0, 5, 7.0, h_in));
    let mut pb = water_pipe(1.0, 5, 6.99, vap.specific_enthalpy.value);
    pb.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<megapascal>(6.99))).unwrap();
    let b = net.add_pipe(pb);
    let p_c = 6.99e6 + rho_f * 9.80665 * 0.35;
    let mut pcp = water_pipe(1.0, 5, p_c / 1e6, liq.specific_enthalpy.value);
    pcp.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<uom::si::pressure::pascal>(p_c))).unwrap();
    let c = net.add_pipe(pcp);
    net.add_time_dependent(
        end(a, PipeEnd::Inlet),
        TimeDependentBoundary::Volume { pressure: TimeTable::constant(7.0e6), thermo: TdvThermo::Enthalpy(TimeTable::constant(h_in)) },
    )
    .unwrap();
    let sep = net
        .add_separated_volume(
            Volume::new::<cubic_meter>(0.002),
            Area::new::<square_meter>(0.001),
            st,
            vec![
                (end(a, PipeEnd::Outlet), 20.0, VolumePort::Mixture),
                (end(b, PipeEnd::Inlet), 20.0, VolumePort::Vapour),
                (end(c, PipeEnd::Inlet), 20.0, VolumePort::AtHeight(0.0)),
            ],
        )
        .unwrap();
    let t_end: f64 = std::env::var("SEP_T_END").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let window = 0.1;
    let mut t_mark = 0.0;
    while net.time().value < t_end - window {
        net.step().unwrap();
        if net.time().value >= t_mark {
            let pa = net.pipe(a).unwrap();
            println!(
                "  t {:.2} s: level {:.4} m, p_sep-end {:.5} MPa, inlet {:.4} kg/s",
                net.time().value,
                net.volume_level(sep).unwrap().unwrap().get::<meter>(),
                pa.cell_state(4).unwrap().pressure.get::<megapascal>(),
                pa.cell_state(4).unwrap().density.value * pa.cell_state(4).unwrap().velocity.value * 0.01
            );
            t_mark += 0.1;
        }
    }
    let snap = |net: &HemNetwork<Iapws97Hem>| {
        let (pa, pb, pc) = (net.pipe(a).unwrap(), net.pipe(b).unwrap(), net.pipe(c).unwrap());
        (
            -pa.cumulative_mass_outflow(PipeEnd::Inlet).value,
            -pa.cumulative_energy_outflow(PipeEnd::Inlet).value,
            pb.cumulative_mass_outflow(PipeEnd::Outlet).value,
            pb.cumulative_energy_outflow(PipeEnd::Outlet).value,
            pc.cumulative_mass_outflow(PipeEnd::Outlet).value,
            pc.cumulative_energy_outflow(PipeEnd::Outlet).value,
            net.time().value,
        )
    };
    let s0 = snap(&net);
    let l0 = net.volume_level(sep).unwrap().unwrap().get::<meter>();
    net.run_until(Time::new::<second>(t_end)).unwrap();
    let s1 = snap(&net);
    let l1 = net.volume_level(sep).unwrap().unwrap().get::<meter>();
    let dt = s1.6 - s0.6;
    let (m_in, e_in) = ((s1.0 - s0.0) / dt, (s1.1 - s0.1) / dt);
    let (m_g, e_g) = ((s1.2 - s0.2) / dt, (s1.3 - s0.3) / dt);
    let (m_f, e_f) = ((s1.4 - s0.4) / dt, (s1.5 - s0.5) / dt);
    let x_split = m_g / m_in;
    let h_g = vap.specific_enthalpy.value;
    println!(
        "separator to {t_end} s: in {m_in:.4} kg/s, steam {m_g:.4}, liquid {m_f:.4}; split {x_split:.4} (x_in 0.2); \
         mass balance {:+.3e}, energy balance {:+.3e}; steam h {:.1} kJ/kg vs h_g {:.1} ({:+.3} %); level {l0:.4} -> {l1:.4} m",
        (m_g + m_f) / m_in - 1.0,
        (e_g + e_f) / e_in - 1.0,
        e_g / m_g / 1e3,
        h_g / 1e3,
        100.0 * (e_g / m_g / h_g - 1.0)
    );
    assert!((x_split / 0.2 - 1.0).abs() < 0.01);
    assert!(((m_g + m_f) / m_in - 1.0).abs() < 0.005);
    assert!(((e_g + e_f) / e_in - 1.0).abs() < 0.005);
    assert!((e_g / m_g / h_g - 1.0).abs() < 0.01);
    let h_f = liq.specific_enthalpy.value;
    println!("liquid stream h {:.1} kJ/kg vs h_f {:.1} ({:+.3} %)", e_f / m_f / 1e3, h_f / 1e3, 100.0 * (e_f / m_f / h_f - 1.0));
    assert!((e_f / m_f / h_f - 1.0).abs() < 0.01);
    assert!(l1 > 0.0 && l1 < 2.0);
}

/// **The uncovered-port switch.** A 1 m tall downcomer (0.01 m³,
/// A = 0.01 m²) of 1 MPa water at quality 0.001 (level about 0.85 m: printed),
/// drained from a port at its bottom through a 1 m pipe to 0.98 MPa; a second
/// port at 0.5 m opens onto a 1 m pipe closed at its far end. Both pipes
/// start as saturated liquid at 1 MPa. (A first version started the closed
/// pipe with the vessel's 0.1 % quality mixture: liquid entering it collapsed
/// the vapour and drove a cell above 100 MPa, a condensation water hammer of
/// the test's making. A drain to 0.5 MPa drove the drain pipe's last cell
/// toward 100 MPa against its `FixedPressure` outlet: a defect of that
/// boundary with flashing saturated liquid, filed separately; the drain now
/// runs to 0.98 MPa.) Every step,
/// `port_draw` of the 0.5 m port is checked against the level: liquid while
/// `L > 0.5 m`, vapour once `L ≤ 0.5 m`. Run until the level is 0.3 m (or
/// 3 s). **Result (2026-11-01):** level 0.8528 → 0.300 m in 15 987 steps
/// (0.458 s); the 0.5 m port switched to vapour at 0.2985 s, on the first
/// step with the level at 0.49998 m, and matched the level on every step;
/// 9.894 kg drained; mass closure −3.4e-14, energy −2.7e-14. Held.
#[test]
fn a_downcomer_port_uncovers_when_the_level_passes_it() {
    let (_, st) = mix(1.0, 0.001);
    let eos = Iapws97Hem;
    let (liq, _) = eos.saturated_phases(&st).unwrap();
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let mut drain = water_pipe(1.0, 5, 1.0, liq.specific_enthalpy.value);
    drain.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<megapascal>(0.98))).unwrap();
    let d = net.add_pipe(drain);
    let side = net.add_pipe(water_pipe(1.0, 5, 1.0, liq.specific_enthalpy.value));
    let dc = net
        .add_separated_volume(
            Volume::new::<cubic_meter>(0.01),
            Area::new::<square_meter>(0.01),
            st,
            vec![
                (end(d, PipeEnd::Inlet), 0.0, VolumePort::AtHeight(0.0)),
                (end(side, PipeEnd::Inlet), 0.0, VolumePort::AtHeight(0.5)),
            ],
        )
        .unwrap();
    let l0 = net.volume_level(dc).unwrap().unwrap().get::<meter>();
    let (m0, e0) = (net.total_mass().value, net.total_energy().value);
    let mut uncovered_at = None;
    let mut steps = 0;
    loop {
        let l = net.volume_level(dc).unwrap().unwrap().get::<meter>();
        let draw = net.port_draw(dc, 1).unwrap().unwrap();
        let expect = if l > 0.5 { PortDraw::Liquid } else { PortDraw::Vapour };
        assert_eq!(draw, expect, "level {l} m");
        assert_eq!(net.port_draw(dc, 0).unwrap().unwrap(), if l > 0.0 { PortDraw::Liquid } else { PortDraw::Vapour });
        if draw == PortDraw::Vapour && uncovered_at.is_none() {
            uncovered_at = Some((net.time().value, l));
        }
        if l < 0.3 || net.time().value > 3.0 {
            break;
        }
        net.step().unwrap();
        steps += 1;
    }
    let out = net.pipe(d).unwrap();
    let drained = out.cumulative_mass_outflow(PipeEnd::Outlet).value;
    let e_out = out.cumulative_energy_outflow(PipeEnd::Outlet).value;
    let dm = (net.total_mass().value + drained) / m0 - 1.0;
    let de = (net.total_energy().value + e_out) / e0 - 1.0;
    let l_end = net.volume_level(dc).unwrap().unwrap().get::<meter>();
    println!(
        "downcomer: level {l0:.4} -> {l_end:.4} m in {steps} steps ({:.3} s); 0.5 m port uncovered at {:?}; drained {drained:.3} kg; mass closure {dm:e}, energy closure {de:e}",
        net.time().value,
        uncovered_at
    );
    let (_, l_unc) = uncovered_at.expect("the port uncovers");
    assert!(l_unc <= 0.5);
    assert!(dm.abs() < 1e-12 && de.abs() < 1e-12);
}
