//! # V&V: continuous-Galerkin FEM against the finite-volume HEM pipe (gh:#1069)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! The same case, the same EOS and the same boundary conditions, run through
//! [`Pipe1d`] on `Discretisation::FiniteVolume` (`HemPipe`, KNP, van Leer,
//! SSP22, the default) and on `Discretisation::FiniteElement(s)` for each
//! stabilisation `s` (SUPG, entropy viscosity, Lapidus; lumped mass, SSP33,
//! classical Courant 0.4 = `HemPipe`'s central 0.2). Methodology and the full
//! result tables: `verification_and_validation/fem1d_vs_fv.md`.
//!
//! ## Predictions (written 2026-11-01, BEFORE the first run of any case)
//!
//! **Sod** (ideal gas, γ = 1.4, 100–1600 cells, t = 6.32 ms):
//!
//! - P1. FV has the lowest L1(ρ) error at every mesh; entropy viscosity (EV)
//!   is second, within 1.5× of FV; SUPG third; Lapidus last (it puts no
//!   viscosity on the contact, where `u_x = 0`, so the contact oscillates).
//! - P2. Observed L1(ρ) order: FV 0.7–0.9, EV 0.6–0.9, SUPG 0.5–0.8,
//!   Lapidus ≤ 0.6.
//! - P3. Shock width (points between 10 % and 90 % of the jump): FV 2–3;
//!   EV 3–5 (its shock viscosity is the first-order `(h/2)(|u| + c)`, i.e.
//!   Rusanov-like); SUPG 2–4; Lapidus 2–4.
//! - P4. Density overshoot (beyond the local exact range, % of ρ_L − ρ_R):
//!   FV ≤ 1 %, EV ≤ 3 %, SUPG 5–15 % (SUPG is not monotone without a
//!   discontinuity-capturing term, which RELAP-7 §5.4.1 does not add),
//!   Lapidus 5–20 % (at the contact).
//! - P5. Global mass and energy conserved to round-off (< 1e-12) by all
//!   four: the waves do not reach the ends, and both schemes telescope.
//! - P6. Cost: an FEM step evaluates the EOS at 2 quadrature points per
//!   element plus the nodes, over 3 stages, against FV's 1 per cell over 2
//!   stages, so ~4–5× FV per step at the same mesh (SUPG ~2× more for its
//!   Grüneisen difference). At equal L1(ρ), FV is fastest by ≥ 5×.
//!
//! **IF97 water double rarefaction** (the existing `HemPipe` IF97 Riemann
//! test: 5 MPa, 450 K, ∓5 m/s, 100 cells, 20 ms; no water *shock* tube with a
//! reference exists in the HEM V&V, so this is the one used):
//!
//! - P7. Every method's saturated-liquid plateau pressure within 1 % of
//!   `p_b`. At the flashing centre (the equilibrium sound speed drops from
//!   ~1100 to ~10 m/s) FV and EV meet the existing 3 kPa centre gate; SUPG and
//!   Lapidus oscillate at the phase boundary and may miss it or fail to close
//!   the EOS.
//!
//! **Quasi-1-D nozzle** (#1058 case, 200 cells, reservoir 1e5 Pa / 300 K):
//!
//! - P8. Subsonic (`p_e/p0 = 0.95`, 0.2 s): all four within 1 % of the
//!   isentropic pressure; SUPG and EV more accurate than FV (smooth steady
//!   flow, where linear Galerkin is second order and the limiter is not).
//!   Mass-flux spread < 1 % for all, larger for FEM (no local conservation).
//! - P9. Shocked (`p_e/p0 = 0.75`, 0.1 s): shock within 3 cells of the
//!   analytic position for all; post-shock pressure within 3 %; SUPG shows
//!   post-shock oscillations.
//!
//! **Edwards–O'Brien** (24 cells, IF97, choked break, frictionless on both,
//! run to 0.30 s, the end of the digitised GS-1 data):
//!
//! - P10. GS-1 plateau within ±15 psia of FV's and RMSE vs data within
//!   ±10 psia of FV's for the FEM runs that complete (same HEM physics; the
//!   plateau is thermodynamic). EV completes; SUPG and Lapidus may fail at
//!   the flashing front. Mass including the break outflow conserved to
//!   1e-12. FEM run time 3–6× FV.
//!
//! Gates asserted here were fixed with the predictions and are honest ones
//! only (completion, conservation, convergence, the FV reference reproducing
//! its own recorded numbers). The predictions are judged in the V&V doc, not
//! asserted: a miss is a finding.
//!
//! **PCICE (gh:#1068), added after the FEM runs:** run on Sod by
//! [`pcice_sod`] with the same `L1(ρ)`; no prediction was written for it
//! here, so its row is a measurement. The other cases print why PCICE was not
//! run on them ([`pcice_hook`]). **Result (2026-11-01):** PCICE L1(ρ)
//! 6.941e-2 → 6.424e-3 over 100–1600 cells (order 0.81–0.91), within
//! 0.4–10 % of FV; wall time at FV-400 accuracy 0.036 s against FV's 0.042 s
//! on the same run.

mod common;

use common::edwards_case::*;
use common::hem_rarefaction;
use common::toro_exact::{exact_state, right_shock_speed, star_pressure, star_velocity, GasState};
use petir::mathf::RealMath;
use std::time::Instant;
use tampines::fem1d::{Discretisation, Pipe1d, Pipe1dNumerics, Stabilisation};
use tampines_steam_tables::hem::{
    AreaProfile, HemBoundary, HemEquationOfState, HemInitialCondition, HemPipeGeometry,
    Iapws97Hem, IdealGasHem, PipeEnd, PrimitiveState,
};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// The methods compared, FV first.
fn methods() -> Vec<Discretisation> {
    vec![
        Discretisation::FiniteVolume,
        Discretisation::FiniteElement(Stabilisation::Supg),
        Discretisation::FiniteElement(Stabilisation::EntropyViscosity),
        Discretisation::FiniteElement(Stabilisation::Lapidus),
        // PCICE (gh:#1068): add its variant here once it exists.
    ]
}

/// Where the PCICE result of a case goes (gh:#1068). Sod is run in
/// [`pcice_sod`] with the same instrument; for the other cases PCICE has
/// not been set up on this file's configurations, and the hook says where
/// its own record is.
fn pcice_hook(case: &str) -> Option<String> {
    match case {
        "water" => Some("not run on this double rarefaction; PCICE's IF97 shock tube is within 0.37 % of HemPipe's star pressure (verification_and_validation/pcice.md)".into()),
        "nozzle-subsonic" | "nozzle-shock" => Some("not run: PCICE's variable area is verified only for a duct at rest (pcice.md)".into()),
        "edwards" => Some("not run: PCICE on a flashing IF97 blowdown is untested (gh:#1087)".into()),
        _ => None,
    }
}

/// **PCICE on this file's Sod** (gh:#1068, added after the FEM runs): the
/// `pcice::cases::sod_ideal_gas` setup is this file's Sod (10 m, the same
/// states and end time), closed at both ends (the waves do not reach them),
/// with PCICE's own numerics (material Courant 0.5, acoustic cap 0.5). The
/// same `L1(ρ)` with cell-width weights as FV. Returns `(cells, L1, wall s)`.
fn pcice_sod(n: usize) -> (usize, f64, f64) {
    let case = tampines::pcice::cases::sod_ideal_gas(n).unwrap();
    let wall = Instant::now();
    let mut pipe = case.build_pcice().unwrap();
    pipe.run_until(case.end_time).unwrap();
    let secs = wall.elapsed().as_secs_f64();
    let t = case.end_time.get::<second>();
    let snap = pipe.snapshot();
    let h = 10.0 / n as f64;
    let l1: f64 = snap
        .cell_centres
        .iter()
        .zip(&snap.cells)
        .map(|(x, c)| h * (c.density.get::<kilogram_per_cubic_meter>() - exact_state(SOD_L, SOD_R, x.get::<meter>() - 5.0, 0.0, t).rho).abs())
        .sum();
    (n, l1, secs)
}

/// The numerics every case uses: the defaults, except that the diagnostic
/// environment knob `FEM_CFL` (unset in the recorded runs) overrides the FEM
/// classical Courant number, to tell a time-step instability from a spatial
/// one.
fn numerics() -> Pipe1dNumerics {
    let mut n = Pipe1dNumerics::default();
    if let Some(c) = std::env::var("FEM_CFL").ok().and_then(|s| s.parse::<f64>().ok()) {
        n.finite_element.courant = Ratio::new::<ratio>(c);
    }
    n
}

fn name(d: Discretisation) -> &'static str {
    match d {
        Discretisation::FiniteVolume => "FV (HemPipe KNP)",
        Discretisation::FiniteElement(Stabilisation::Supg) => "FEM SUPG",
        Discretisation::FiniteElement(Stabilisation::EntropyViscosity) => "FEM entropy viscosity",
        Discretisation::FiniteElement(Stabilisation::Lapidus) => "FEM Lapidus",
        Discretisation::FiniteElement(Stabilisation::Galerkin) => "FEM Galerkin (no stabilisation)",
    }
}

fn gas(p: f64, rho: f64, u: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(u),
    }
}

/// Integration weights of the solution points: `h` per FV cell; the
/// trapezoid (lumped) weights `h/2, h, …, h, h/2` per FEM node.
fn weights(d: Discretisation, n: usize, h: f64) -> Vec<f64> {
    match d {
        Discretisation::FiniteVolume => vec![h; n],
        Discretisation::FiniteElement(_) => (0..=n).map(|j| if j == 0 || j == n { 0.5 * h } else { h }).collect(),
    }
}

/// Global conservation error `max(|ΔM|/M0, |ΔE|/E0)` counting what left
/// through both ends.
fn conservation<E: HemEquationOfState>(p: &Pipe1d<E>, m0: f64, e0: f64) -> (f64, f64) {
    let out_m = p.cumulative_mass_outflow(PipeEnd::Inlet).value + p.cumulative_mass_outflow(PipeEnd::Outlet).value;
    let out_e = p.cumulative_energy_outflow(PipeEnd::Inlet).value + p.cumulative_energy_outflow(PipeEnd::Outlet).value;
    (
        ((p.total_mass().value + out_m - m0) / m0).abs(),
        ((p.total_energy().value + out_e - e0) / e0).abs(),
    )
}

/// Points of `vals` (with positions `xs` inside `[x0, x1]`) strictly between
/// the 10 % and 90 % levels from `lo` to `hi`.
fn width(xs: &[f64], vals: &[f64], x0: f64, x1: f64, lo: f64, hi: f64) -> usize {
    let (a, b) = (lo + 0.1 * (hi - lo), lo + 0.9 * (hi - lo));
    let (a, b) = (a.min(b), a.max(b));
    xs.iter()
        .zip(vals)
        .filter(|(x, v)| **x >= x0 && **x <= x1 && **v > a && **v < b)
        .count()
}

/// Largest excursion of `vals` above / below the exact solution's local
/// range over `±3h` around each point, in units of `scale`.
fn over_under<F: Fn(f64) -> f64>(xs: &[f64], vals: &[f64], h: f64, exact: F, scale: f64) -> (f64, f64) {
    let (mut over, mut under) = (0.0_f64, 0.0_f64);
    for (x, v) in xs.iter().zip(vals) {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in -12..=12 {
            let e = exact(x + k as f64 * h / 4.0);
            lo = lo.min(e);
            hi = hi.max(e);
        }
        over = over.max(v - hi);
        under = under.max(lo - v);
    }
    (over / scale, under / scale)
}

// ───────────────────────────── Sod ─────────────────────────────

const SOD_L: GasState = GasState { rho: 1.0, u: 0.0, p: 1.0e5 };
const SOD_R: GasState = GasState { rho: 0.125, u: 0.0, p: 1.0e4 };

#[derive(Debug, Clone, Copy)]
struct SodResult {
    n: usize,
    l1_rho: f64,
    shock_pts: usize,
    contact_pts: usize,
    over_rho: f64,
    under_rho: f64,
    over_u: f64,
    under_u: f64,
    cons: (f64, f64),
    secs: f64,
    steps: usize,
}

/// Sod's tube in SI as `tampines-steam-tables`' `run_sod` (appbuilder's
/// case): `[0, 10]` m, diaphragm 5 m, air, `1e5 | 1e4` Pa, `1 | 0.125`
/// kg/m³, transmissive ends, `t = 0.2·10/√1e5 = 6.32456e-3` s.
fn run_sod(d: Discretisation, n: usize) -> SodResult {
    let geo = HemPipeGeometry::new(Length::new::<meter>(10.0), Area::new::<square_meter>(1.0), n).unwrap();
    let wall = Instant::now();
    let mut pipe = Pipe1d::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(5.0),
            left: gas(1.0e5, 1.0, 0.0),
            right: gas(1.0e4, 0.125, 0.0),
        },
        d,
        numerics(),
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive).unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive).unwrap();
    let (m0, e0) = (pipe.total_mass().value, pipe.total_energy().value);
    let t = 0.2 * 10.0 / (1.0e5_f64).sqrt();
    let sum = pipe
        .run_until(Time::new::<second>(t))
        .unwrap_or_else(|e| panic!("{} Sod {n}: {e}", name(d)));
    let secs = wall.elapsed().as_secs_f64();
    let snap = pipe.snapshot().unwrap();
    let xs: Vec<f64> = snap.cell_centres.iter().map(|x| x.get::<meter>() - 5.0).collect();
    let rho: Vec<f64> = snap.cells.iter().map(|c| c.density.get::<kilogram_per_cubic_meter>()).collect();
    let u: Vec<f64> = snap.cells.iter().map(|c| c.velocity.get::<meter_per_second>()).collect();
    let h = 10.0 / n as f64;
    let w = weights(d, n, h);
    let ex = |x: f64| exact_state(SOD_L, SOD_R, x, 0.0, t);
    let l1_rho: f64 = xs.iter().zip(&rho).zip(&w).map(|((x, r), wi)| wi * (r - ex(*x).rho).abs()).sum();
    // Exact wave positions.
    let ps = star_pressure(SOD_L, SOD_R);
    let us = star_velocity(SOD_L, SOD_R, ps);
    let x_shock = right_shock_speed(SOD_R, ps) * t;
    let x_contact = us * t;
    let c_l = (1.4 * SOD_L.p / SOD_L.rho).sqrt();
    let c_sl = c_l * (ps / SOD_L.p).r_powf(0.4 / 2.8);
    let x_tail = (us - c_sl) * t;
    let rho_sl = ex(x_contact - 1e-6).rho;
    let rho_sr = ex(x_contact + 1e-6).rho;
    let mid = 0.5 * (x_contact + x_shock);
    let shock_pts = width(&xs, &rho, mid, 5.0, SOD_R.rho, rho_sr);
    let contact_pts = width(&xs, &rho, 0.5 * (x_tail + x_contact), mid, rho_sr, rho_sl);
    let (over_rho, under_rho) = over_under(&xs, &rho, h, |x| ex(x).rho, SOD_L.rho - SOD_R.rho);
    let (over_u, under_u) = over_under(&xs, &u, h, |x| ex(x).u, us);
    SodResult {
        n,
        l1_rho,
        shock_pts,
        contact_pts,
        over_rho,
        under_rho,
        over_u,
        under_u,
        cons: conservation(&pipe, m0, e0),
        secs,
        steps: sum.steps,
    }
}

/// Wall time at the target error, by log-log interpolation of `(L1, time)`
/// between meshes (extrapolated from the nearest two, and flagged, outside).
fn time_at(results: &[SodResult], target: f64) -> (f64, bool) {
    let pts: Vec<(f64, f64)> = results.iter().map(|r| (r.l1_rho.r_ln(), r.secs.max(1e-6).r_ln())).collect();
    let lt = target.r_ln();
    for w in pts.windows(2) {
        let ((e0, t0), (e1, t1)) = (w[0], w[1]);
        if (lt <= e0 && lt >= e1) || (lt >= e0 && lt <= e1) {
            return ((t0 + (lt - e0) * (t1 - t0) / (e1 - e0)).r_exp(), false);
        }
    }
    let k = pts.len();
    let ((e0, t0), (e1, t1)) = if lt < pts[k - 1].0 { (pts[k - 2], pts[k - 1]) } else { (pts[0], pts[1]) };
    ((t0 + (lt - e0) * (t1 - t0) / (e1 - e0)).r_exp(), true)
}

/// **Sod on FV and the three FEM stabilisations, 100–1600 cells.**
///
/// Methodology: see `run_sod`; exact solution Toro (2009) ch. 4 (the copied
/// `common::toro_exact`), evaluated at each method's own solution points.
/// Measures: `L1(ρ) = ∫|ρ − ρ_exact| dx` with each method's own quadrature
/// (cell widths for FV, trapezoid for FEM nodes); observed order between
/// successive meshes; shock and contact widths (10–90 % points); over- and
/// undershoot beyond the exact local range (±3h) in % of `ρ_L − ρ_R` and
/// of `u*`; global conservation; wall time and steps; wall time at FV's
/// 400-cell error.
///
/// Gates (fixed with the predictions): every run completes; mass and energy
/// conserved to 1e-12; each method's L1(ρ) at 1600 cells below its 100-cell
/// value; FV's 100-cell L1(ρ) close to its recorded L2-era behaviour is not
/// asserted (different norm).
///
/// Results: `verification_and_validation/fem1d_vs_fv.md`, Sod tables.
/// **Results (2026-11-01):** FV best at every mesh (L1(ρ) 6.91e-2 → 5.85e-3,
/// order 0.84–0.94); SUPG 1.05e-1 → 1.02e-2 (0.81–0.86); EV 9.77e-2 →
/// 1.25e-2 (0.72–0.76); Lapidus 1.18e-1 → 1.68e-2 (0.68–0.73). Velocity
/// overshoot FV < 1 %, SUPG 13–15 %, EV and Lapidus 10–12 % of u*. Wall time
/// at FV-400 accuracy: FV 0.045 s, EV 0.32, SUPG 0.49, Lapidus 0.62.
/// Conservation ≤ 8e-14. All gates met.
#[test]
fn sod_fv_against_fem_stabilisations() {
    let meshes = [100usize, 200, 400, 800, 1600];
    let mut all: Vec<(Discretisation, Vec<SodResult>)> = vec![];
    for d in methods() {
        let rs: Vec<SodResult> = meshes.iter().map(|&n| run_sod(d, n)).collect();
        all.push((d, rs));
    }
    let target = all[0].1[2].l1_rho;
    println!("\nSOD | method | cells | L1(rho) kg/m2 | order | shock pts | contact pts | rho over % | rho under % | u over % | u under % | mass err | energy err | steps | wall s");
    for (d, rs) in &all {
        for (k, r) in rs.iter().enumerate() {
            let order = if k == 0 { f64::NAN } else { (rs[k - 1].l1_rho / r.l1_rho).r_ln() / 2.0_f64.r_ln() };
            println!(
                "SOD | {} | {} | {:.4e} | {:.2} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.1e} | {:.1e} | {} | {:.3}",
                name(*d), r.n, r.l1_rho, order, r.shock_pts, r.contact_pts,
                100.0 * r.over_rho, 100.0 * r.under_rho, 100.0 * r.over_u, 100.0 * r.under_u,
                r.cons.0, r.cons.1, r.steps, r.secs
            );
        }
    }
    println!("\nSOD-COST | method | wall s at L1(rho) = {target:.4e} (FV, 400 cells)");
    for (d, rs) in &all {
        let (t, ex) = time_at(rs, target);
        println!("SOD-COST | {} | {:.4}{}", name(*d), t, if ex { " (extrapolated)" } else { "" });
    }
    let pc: Vec<(usize, f64, f64)> = meshes.iter().map(|&n| pcice_sod(n)).collect();
    for (k, (n, l1, secs)) in pc.iter().enumerate() {
        let order = if k == 0 { f64::NAN } else { (pc[k - 1].1 / l1).r_ln() / 2.0_f64.r_ln() };
        println!("SOD | PCICE (gh:#1068) | {n} | {l1:.4e} | {order:.2} | | | | | | | | | | {secs:.3}");
    }
    let pts: Vec<SodResult> = pc.iter().map(|&(n, l1, secs)| SodResult { n, l1_rho: l1, secs, ..all[0].1[0] }).collect();
    let (tp, exp) = time_at(&pts, target);
    println!("SOD-COST | PCICE (gh:#1068) | {:.4}{}", tp, if exp { " (extrapolated)" } else { "" });
    assert!(pc[4].1 < pc[0].1, "PCICE Sod error falls with the mesh");
    for (d, rs) in &all {
        for r in rs {
            assert!(r.cons.0 < 1e-12 && r.cons.1 < 1e-12, "{} {}: conservation {:?}", name(*d), r.n, r.cons);
        }
        assert!(rs[4].l1_rho < rs[0].l1_rho, "{} does not converge", name(*d));
    }
}

// ─────────────────── IF97 water double rarefaction ───────────────────

const DR_P0: f64 = 5.0e6;
const DR_T0: f64 = 450.0;
const DR_U0: f64 = 5.0;

/// **The HEM water double rarefaction on FV and FEM** (the case of
/// `tampines-steam-tables`' `hem_water_double_rarefaction_matches_the_semi_analytical_plateaus`:
/// 1 m, 100 cells, transmissive, IF97, to 20 ms, frictionless on both).
///
/// Measures, against the semi-analytical solution (copied
/// `common::hem_rarefaction`): plateau (`0.30 ≤ |x| ≤ 0.45 m`) mean `p`
/// and `|u|`, centre (`|x| ≤ 0.10 m`) mean `p` and `ρ`, against the existing
/// gates (1 %, 5 %, `0.25(p_b − p*)`, 5 %); L1 of `p` over the pipe;
/// conservation; wall time. A failed FEM run is recorded, not asserted.
///
/// Gates: FV completes and meets its four existing gates (it must reproduce
/// its own record); every run that completes conserves mass to 1e-12.
/// **Results (2026-11-01):** FV meets all four gates (−0.003 %, −0.05 %,
/// −4 Pa, −0.02 %). All three FEM stabilisations fail to close the EOS
/// (SUPG at 1e-5 s, EV at 4.3 ms, Lapidus at 0.42 ms at the transmissive
/// end); recorded, not asserted (prediction P7 missed for EV).
#[test]
fn if97_double_rarefaction_fv_against_fem() {
    let pred = hem_rarefaction::solve(DR_P0, DR_T0, DR_U0);
    let n = 100;
    let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), Area::new::<square_meter>(1e-3), n).unwrap();
    let state = |u: f64| PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<pascal>(DR_P0),
        temperature: ThermodynamicTemperature::new::<kelvin>(DR_T0),
        velocity: Velocity::new::<meter_per_second>(u),
    };
    let t_end = 0.02;
    println!("\nWATER | method | plateau p err % | plateau |u| err % | centre p - p* Pa (gate {:.0}) | centre rho err % | L1(p) Pa m | mass err | steps | wall s", 0.25 * (pred.p_bubble - pred.p_star));
    for d in methods() {
        let wall = Instant::now();
        let mut pipe = Pipe1d::new(
            geo,
            Iapws97Hem,
            HemInitialCondition::Riemann { diaphragm: Length::new::<meter>(0.5), left: state(-DR_U0), right: state(DR_U0) },
            d,
            numerics(),
        )
        .unwrap()
        .without_wall_friction();
        pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive).unwrap();
        pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive).unwrap();
        let (m0, e0) = (pipe.total_mass().value, pipe.total_energy().value);
        let res = pipe.run_until(Time::new::<second>(t_end));
        let secs = wall.elapsed().as_secs_f64();
        let sum = match res {
            Ok(s) => s,
            Err(e) => {
                println!("WATER | {} | FAILED at t = {:.5} s: {e} | | | | | | | {:.2}", name(d), pipe.time().get::<second>(), secs);
                assert!(d != Discretisation::FiniteVolume, "FV must complete");
                continue;
            }
        };
        let snap = pipe.snapshot().unwrap();
        let w = weights(d, n, 0.01);
        let (mut pl, mut ce, mut l1) = ((0.0, 0.0, 0usize), (0.0, 0.0, 0usize), 0.0);
        for ((x, c), wi) in snap.cell_centres.iter().zip(&snap.cells).zip(&w) {
            let xr = x.get::<meter>() - 0.5;
            let p = c.pressure.get::<pascal>();
            if (0.30..=0.45).contains(&xr.abs()) {
                pl = (pl.0 + p, pl.1 + c.velocity.get::<meter_per_second>().abs(), pl.2 + 1);
            }
            if xr.abs() <= 0.10 {
                ce = (ce.0 + p, ce.1 + c.density.get::<kilogram_per_cubic_meter>(), ce.2 + 1);
            }
            let (pe, _) = hemsample(&pred, xr, t_end);
            l1 += wi * (p - pe).abs();
        }
        let ep = pl.0 / pl.2 as f64 / pred.p_bubble - 1.0;
        let eu = pl.1 / pl.2 as f64 / -pred.u_bubble - 1.0;
        let dpc = ce.0 / ce.2 as f64 - pred.p_star;
        let erc = ce.1 / ce.2 as f64 / pred.rho_star - 1.0;
        let cons = conservation(&pipe, m0, e0);
        println!(
            "WATER | {} | {:+.3} | {:+.2} | {:+.0} | {:+.2} | {:.1} | {:.1e} | {} | {:.2}",
            name(d), 100.0 * ep, 100.0 * eu, dpc, 100.0 * erc, l1, cons.0, sum.steps, secs
        );
        assert!(cons.0 < 1e-12, "{} mass {:e}", name(d), cons.0);
        if d == Discretisation::FiniteVolume {
            assert!(ep.abs() <= 0.01 && eu.abs() <= 0.05);
            assert!(dpc.abs() <= 0.25 * (pred.p_bubble - pred.p_star) && erc.abs() <= 0.05);
        }
    }
    println!("WATER | PCICE (gh:#1068) | {}", pcice_hook("water").unwrap_or_else(|| "pending".into()));
}

/// Semi-analytical `(p, u)` at `xr` (measured from the centre) at `t`.
fn hemsample(pred: &hem_rarefaction::DoubleRarefaction, xr: f64, t: f64) -> (f64, f64) {
    let (p, u) = hem_rarefaction::sample_left(pred, -xr.abs() / t);
    (p, -u * xr.signum())
}

// ───────────────────────────── Nozzle ─────────────────────────────

const GAMMA: f64 = 1.4;
const NZ: usize = 200;

fn nozzle() -> AreaProfile {
    AreaProfile::ConvergingDiverging { inlet_m2: 1.5, throat_m2: 1.0, outlet_m2: 1.5, throat_fraction: 0.5 }
}

fn area_ratio(m: f64) -> f64 {
    let k = (GAMMA + 1.0) / (2.0 * (GAMMA - 1.0));
    (1.0 / m) * ((2.0 / (GAMMA + 1.0)) * (1.0 + 0.5 * (GAMMA - 1.0) * m * m)).r_powf(k)
}

fn mach_from_area(target: f64, supersonic: bool) -> f64 {
    let (mut lo, mut hi) = if supersonic { (1.0, 10.0) } else { (1e-6, 1.0) };
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if (area_ratio(mid) > target) != supersonic {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn p_over_p0(m: f64) -> f64 {
    (1.0 + 0.5 * (GAMMA - 1.0) * m * m).r_powf(-GAMMA / (GAMMA - 1.0))
}

fn mach_from_p(p_ratio: f64) -> f64 {
    ((2.0 / (GAMMA - 1.0)) * (p_ratio.r_powf(-(GAMMA - 1.0) / GAMMA) - 1.0)).sqrt()
}

/// The #1058 nozzle run on `d`: reservoir `(1e5 Pa, 300 K)` inlet, fixed
/// exit pressure, from rest at the exit state. Returns positions, pressures,
/// mass fluxes `ρuA`, conservation, steps and wall time; `Err` text if the
/// run failed.
#[allow(clippy::type_complexity)]
fn run_nozzle(d: Discretisation, p_exit: f64, t_end: f64) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, (f64, f64), usize, f64), String> {
    let air = IdealGasHem::air();
    let r = air.specific_gas_constant.value;
    let h0 = GAMMA / (GAMMA - 1.0) * r * 300.0;
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), NZ).unwrap();
    let wall = Instant::now();
    let mut pipe = Pipe1d::new(geo, air, HemInitialCondition::Uniform(gas(p_exit, p_exit / (r * 300.0), 0.0)), d, numerics()).unwrap();
    pipe.set_boundary(
        PipeEnd::Inlet,
        HemBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(1.0e5),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h0),
        },
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<pascal>(p_exit))).unwrap();
    let (m0, e0) = (pipe.total_mass().value, pipe.total_energy().value);
    let sum = pipe.run_until(Time::new::<second>(t_end)).map_err(|e| format!("FAILED at t = {:.5} s: {e}", pipe.time().get::<second>()))?;
    let secs = wall.elapsed().as_secs_f64();
    let snap = pipe.snapshot().unwrap();
    let x: Vec<f64> = snap.cell_centres.iter().map(|c| c.get::<meter>()).collect();
    let p: Vec<f64> = snap.cells.iter().map(|c| c.pressure.get::<pascal>()).collect();
    let g: Vec<f64> = snap
        .cells
        .iter()
        .zip(&snap.cell_centres)
        .map(|(c, xc)| c.density.value * c.velocity.value * geo.flow_area_at(*xc).get::<square_meter>())
        .collect();
    Ok((x, p, g, conservation(&pipe, m0, e0), sum.steps, secs))
}

fn area_at(x: f64) -> f64 {
    HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), NZ)
        .unwrap()
        .flow_area_at(Length::new::<meter>(x))
        .get::<square_meter>()
}

/// **Subsonic nozzle** (`p_e/p0 = 0.95`, 0.2 s, 200 cells): worst and L1
/// relative pressure error against the isentropic area–Mach solution, the
/// mass-flux spread `(max − min)/max` over the points, conservation
/// (inflow counted), wall time. Gate: FV within its recorded 1 % / 1 %.
/// **Results (2026-11-01):** worst p error FV 0.31 %, SUPG / EV / Lapidus
/// 0.19 %; mass-flux spread FV 0.74 %, FEM 0.0009–0.025 %. FEM wins here.
#[test]
fn subsonic_nozzle_fv_against_fem() {
    let a_star = 1.5 / area_ratio(mach_from_p(0.95));
    println!("\nNOZZLE-SUB | method | worst p err % | mean |p err| % | mass-flux spread % | mass err | energy err | steps | wall s");
    for d in methods() {
        match run_nozzle(d, 0.95e5, 0.2) {
            Err(e) => println!("NOZZLE-SUB | {} | {e}", name(d)),
            Ok((x, p, g, cons, steps, secs)) => {
                let errs: Vec<f64> = x
                    .iter()
                    .zip(&p)
                    .map(|(xi, pi)| (pi / (1.0e5 * p_over_p0(mach_from_area(area_at(*xi) / a_star, false))) - 1.0).abs())
                    .collect();
                let worst = errs.iter().fold(0.0_f64, |a, b| a.max(*b));
                let mean = errs.iter().sum::<f64>() / errs.len() as f64;
                let (lo, hi) = g.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| (l.min(*v), h.max(*v)));
                let spread = (hi - lo) / hi;
                println!(
                    "NOZZLE-SUB | {} | {:.4} | {:.4} | {:.4} | {:.1e} | {:.1e} | {} | {:.2}",
                    name(d), 100.0 * worst, 100.0 * mean, 100.0 * spread, cons.0, cons.1, steps, secs
                );
                if d == Discretisation::FiniteVolume {
                    assert!(worst < 0.01 && spread < 0.01);
                }
            }
        }
    }
    println!("NOZZLE-SUB | PCICE (gh:#1068) | {}", pcice_hook("nozzle-subsonic").unwrap_or_else(|| "pending".into()));
}

/// **Shocked nozzle** (`p_e/p0 = 0.75`, 0.1 s, 200 cells): shock position
/// (largest pressure rise in the diverging half) against the analytic one
/// in cells; post-shock pressure 5 cells downstream; L1 relative pressure
/// error over the pipe against the analytic profile; shock width (10–90 %
/// points of the jump within ±5 cells of the measured shock); pressure over/undershoot beyond the local exact range
/// in % of the jump; conservation; wall time. Gate: FV within its recorded
/// 3 cells and 3 %.
/// **Results (2026-11-01):** shock offset FV +0.92 cells, FEM +1.42 (nodes vs
/// centres); post-shock within 1.3 % for all; p overshoot FV 1.4 %, SUPG
/// 7.5 %, EV 1.1 %, Lapidus 1.1 % of the jump.
#[test]
fn shocked_nozzle_fv_against_fem() {
    let (at, ae, pr) = (1.0, 1.5, 0.75);
    let p02 = |m1: f64| {
        let a = ((GAMMA + 1.0) * m1 * m1 / ((GAMMA - 1.0) * m1 * m1 + 2.0)).r_powf(GAMMA / (GAMMA - 1.0));
        let b = ((GAMMA + 1.0) / (2.0 * GAMMA * m1 * m1 - (GAMMA - 1.0))).r_powf(1.0 / (GAMMA - 1.0));
        a * b
    };
    let exit_p = |a_s: f64| {
        let r0 = p02(mach_from_area(a_s / at, true));
        r0 * p_over_p0(mach_from_area(ae / (at / r0), false))
    };
    let (mut lo, mut hi) = (at * 1.000_001, ae);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if exit_p(mid) > pr {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a_s = 0.5 * (lo + hi);
    let x_s = 0.5 + 0.5 * ((a_s - at) / (ae - at)).sqrt();
    let m1 = mach_from_area(a_s / at, true);
    let r0 = p02(m1);
    let a_star2 = at / r0;
    let exact = |x: f64| -> f64 {
        let a = area_at(x.clamp(0.0, 1.0));
        if x < 0.5 {
            1.0e5 * p_over_p0(mach_from_area(a / at, false))
        } else if x < x_s {
            1.0e5 * p_over_p0(mach_from_area(a / at, true))
        } else {
            1.0e5 * r0 * p_over_p0(mach_from_area(a / a_star2, false))
        }
    };
    let p1 = 1.0e5 * p_over_p0(m1);
    let p2 = 1.0e5 * r0 * p_over_p0(mach_from_area(a_s / a_star2, false));
    let dx = 1.0 / NZ as f64;
    println!("\nanalytic shock at x = {x_s:.4} m, M1 = {m1:.3}, p {p1:.0} -> {p2:.0} Pa");
    println!("NOZZLE-SHOCK | method | shock offset cells | post-shock p err % | L1 |p err| % | shock pts | p over % jump | p under % jump | mass err | energy err | steps | wall s");
    for d in methods() {
        match run_nozzle(d, pr * 1.0e5, 0.1) {
            Err(e) => println!("NOZZLE-SHOCK | {} | {e}", name(d)),
            Ok((x, p, _, cons, steps, secs)) => {
                let npts = x.len();
                let start = x.iter().position(|xi| *xi > 0.5).unwrap();
                let (k, _) = (start..npts - 1)
                    .map(|i| (i, p[i + 1] - p[i]))
                    .fold((0, f64::NEG_INFINITY), |b, c| if c.1 > b.1 { c } else { b });
                let x_num = 0.5 * (x[k] + x[k + 1]);
                let j = x.iter().position(|xi| *xi > x_s + 5.0 * dx).unwrap();
                let e_post = p[j] / exact(x[j]) - 1.0;
                let l1 = x.iter().zip(&p).map(|(xi, pi)| (pi / exact(*xi) - 1.0).abs()).sum::<f64>() / npts as f64;
                let pts = width(&x, &p, x_num - 5.0 * dx, x_num + 5.0 * dx, p1, p2);
                let (ov, un) = over_under(&x, &p, dx, exact, p2 - p1);
                println!(
                    "NOZZLE-SHOCK | {} | {:+.2} | {:+.2} | {:.3} | {} | {:.2} | {:.2} | {:.1e} | {:.1e} | {} | {:.2}",
                    name(d), (x_num - x_s) / dx, 100.0 * e_post, 100.0 * l1, pts, 100.0 * ov, 100.0 * un, cons.0, cons.1, steps, secs
                );
                if d == Discretisation::FiniteVolume {
                    assert!((x_num - x_s).abs() <= 3.0 * dx && e_post.abs() < 0.03);
                }
            }
        }
    }
    println!("NOZZLE-SHOCK | PCICE (gh:#1068) | {}", pcice_hook("nozzle-shock").unwrap_or_else(|| "pending".into()));
}

/// **A fluid at rest in the nozzle on FEM, and walls conserving mass and
/// energy in a varying area.**
///
/// Prediction (before the run): the Galerkin pressure terms `∫(pA'φ +
/// pAφ')` integrate `p(Aφ)'` exactly with 2-point Gauss, so a uniform-pressure
/// rest state is a discrete steady state, `|u| < 1e-9` m/s after 2 ms on every
/// stabilisation; and a Riemann problem between walls conserves mass and
/// energy to 1e-12.
///
/// **Result (2026-11-01): the rest-state prediction MISSED** — 200 cells,
/// Galerkin: `max|u| = 2.4e-3 m/s` after 2 ms. The assumption it rested on
/// was wrong: the nodal unknown is `ρA` (RELAP-7 Eqs 243–245), so at a
/// quadrature point `ρ = V^h/A` is not the uniform `ρ` when `A` is quadratic,
/// and neither is `p`. The 1e-9 gate is kept as the record of the miss, not
/// asserted; this test now asserts what was measured to hold: the residual
/// motion falls when the mesh is doubled (a consistency error, added after
/// the miss and labelled so), and the wall-conservation gate as registered.
/// Numbers per stabilisation: V&V doc, "Rest state".
#[test]
fn fem_rest_state_and_wall_conservation_in_the_nozzle() {
    let rest_umax = |n: usize, d: Discretisation| -> f64 {
        let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), n).unwrap();
        let mut rest = Pipe1d::new(geo, IdealGasHem::air(), HemInitialCondition::Uniform(gas(1.0e5, 1.2, 0.0)), d, numerics()).unwrap();
        rest.run_until(Time::new::<second>(2.0e-3)).unwrap();
        rest.snapshot().unwrap().cells.iter().map(|c| c.velocity.value.abs()).fold(0.0, f64::max)
    };
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), NZ).unwrap();
    for st in [Stabilisation::Galerkin, Stabilisation::Supg, Stabilisation::EntropyViscosity, Stabilisation::Lapidus] {
        let d = Discretisation::FiniteElement(st);
        let (u1, u2, u4) = (rest_umax(NZ, d), rest_umax(2 * NZ, d), rest_umax(4 * NZ, d));
        let mut walls = Pipe1d::new(
            geo,
            IdealGasHem::air(),
            HemInitialCondition::Riemann { diaphragm: Length::new::<meter>(0.3), left: gas(1.0e5, 1.0, 0.0), right: gas(1.0e4, 0.125, 0.0) },
            d,
            numerics(),
        )
        .unwrap();
        let (m0, e0) = (walls.total_mass().value, walls.total_energy().value);
        let ok = walls.run_until(Time::new::<second>(2.0e-3));
        let cons = conservation(&walls, m0, e0);
        println!(
            "REST | {} | max |u| at rest, 200/400/800 cells: {u1:.2e} / {u2:.2e} / {u4:.2e} m/s (order {:.2}, {:.2}) | walls: run {}, mass {:.1e}, energy {:.1e}",
            name(d),
            (u1 / u2).r_ln() / 2.0_f64.r_ln(),
            (u2 / u4).r_ln() / 2.0_f64.r_ln(),
            if ok.is_ok() { "ok" } else { "FAILED" },
            cons.0,
            cons.1
        );
        println!("REST | {} | registered gate |u| < 1e-9 m/s: {}", name(d), if u1 < 1e-9 { "met" } else { "MISSED" });
        assert!(u2 < u1 && u4 < u2, "{st:?}: rest-state motion does not fall with the mesh");
        if st != Stabilisation::Galerkin {
            ok.unwrap();
            assert!(cons.0 < 1e-12 && cons.1 < 1e-12, "{st:?} {cons:?}");
        }
    }
    let fv = rest_umax(NZ, Discretisation::FiniteVolume);
    println!("REST | FV (HemPipe KNP) | max |u| at rest, 200 cells: {fv:.2e} m/s");
}

// ───────────────────────────── Edwards ─────────────────────────────

/// **Edwards–O'Brien on FV and FEM, 24 cells, to 0.30 s** (the last
/// digitised GS-1 point; the 0.6 s FV record is in tampines-steam-tables).
///
/// Setup as `tampines-steam-tables/tests/hem_pipe_edwards.rs`: 7 MPa, Hendrie
/// temperature at each solution point, wall at `x = 0`, `ChokedBreak` to
/// 0.1 MPa ramped 0 → 0.87 over 1 ms (area re-set every 50 µs during the
/// ramp, every 1 ms after), IF97. **Frictionless on both**: the FEM carries
/// no friction, so FV's default friction is ablated to compare the same
/// physics (the recorded FV Edwards numbers were also frictionless).
/// GS-1 is the point nearest 12.890 ft (a node for FEM, a cell centre for
/// FV). Measures: GS-1 plateau mean over 0.02–0.06 s; RMSE against the
/// data points up to 0.30 s; mass conservation counting the break outflow;
/// wall time. Gates: FV completes, conserves mass to 1e-12 and its RMSE is
/// within 2 psia of its recorded 44.2 psia (frictionless, 24 cells); FEM runs
/// that complete conserve mass to 1e-12. `EDW_CMP_TEND` overrides the end
/// time.
/// **Results (2026-11-01):** FV completes, plateau 368.0 psia, RMSE 44.2
/// psia, 8 s (reproduces its record). At the registered FEM Courant 0.4 all
/// three FEM runs fail to close the EOS (SUPG 0.3 ms, EV 52 ms, Lapidus 37
/// ms). Diagnostic `FEM_CFL=0.1`: EV completes (372.2 / 48.1 psia, 93 s),
/// Lapidus completes (372.7 / 46.3 psia, 88 s), SUPG fails at 0.4 ms.
#[test]
fn edwards_fv_against_fem() {
    let t_end: f64 = std::env::var("EDW_CMP_TEND").ok().and_then(|s| s.parse().ok()).unwrap_or(0.30);
    let n = 24;
    let geo = HemPipeGeometry::new(Length::new::<meter>(PIPE_LENGTH_M), Area::new::<square_meter>(PIPE_AREA_M2), n).unwrap();
    println!("\nEDWARDS | method | t reached s | GS-1 plateau psia | RMSE vs data psia (pts) | mass err | steps | wall s");
    for d in methods() {
        let pts = d.solution_points(&geo);
        let init: Vec<PrimitiveState> = pts
            .iter()
            .map(|x| PrimitiveState::PressureTemperature {
                pressure: Pressure::new::<pascal>(P_INIT_PA),
                temperature: ThermodynamicTemperature::new::<kelvin>(hendrie_temperature_k(x.get::<meter>())),
                velocity: Velocity::new::<meter_per_second>(0.0),
            })
            .collect();
        let gs1 = (0..pts.len())
            .min_by(|&a, &b| {
                let x = GS1_POS_FT * FT_TO_M;
                (pts[a].get::<meter>() - x).abs().partial_cmp(&(pts[b].get::<meter>() - x).abs()).unwrap()
            })
            .unwrap();
        let wall = Instant::now();
        let mut pipe = Pipe1d::new(geo, Iapws97Hem, HemInitialCondition::PerCell(init), d, numerics())
            .unwrap()
            .without_wall_friction();
        let m0 = pipe.total_mass().value;
        let mut series: Vec<(f64, f64)> = vec![(0.0, P_INIT_PA * PA_TO_PSI)];
        let (mut t_next, mut steps, mut max_err, mut failed) = (0.0_f64, 0usize, 0.0_f64, None);
        while t_next < t_end - 1e-12 {
            let dts = if t_next < DISC_OPEN_TIME_S { 5.0e-5 } else { 1.0e-3 };
            t_next = (t_next + dts).min(t_end);
            let t_mid = 0.5 * (pipe.time().get::<second>() + t_next);
            pipe.set_boundary(
                PipeEnd::Outlet,
                HemBoundary::ChokedBreak {
                    area_fraction: Ratio::new::<ratio>(break_area_fraction(t_mid)),
                    back_pressure: Pressure::new::<pascal>(P_AMBIENT_PA),
                },
            )
            .unwrap();
            match pipe.run_until(Time::new::<second>(t_next)) {
                Ok(s) => steps += s.steps,
                Err(e) => {
                    failed = Some(format!("{e}"));
                    break;
                }
            }
            let out = pipe.cumulative_mass_outflow(PipeEnd::Outlet).value + pipe.cumulative_mass_outflow(PipeEnd::Inlet).value;
            max_err = max_err.max((pipe.total_mass().value + out - m0).abs() / m0);
            let p = pipe.snapshot().unwrap().cells[gs1].pressure.get::<pascal>() * PA_TO_PSI;
            series.push((pipe.time().get::<second>(), p));
        }
        let secs = wall.elapsed().as_secs_f64();
        let t_reached = pipe.time().get::<second>();
        let ts: Vec<f64> = series.iter().map(|s| s.0).collect();
        let ps: Vec<f64> = series.iter().map(|s| s.1).collect();
        let used: Vec<&(f64, f64)> = GS1_DATA_PSIA.iter().filter(|(t, _)| *t <= t_reached + 1e-12).collect();
        let rmse = (used.iter().map(|(t, dd)| (lerp(&ts, &ps, *t) - dd).r_powi(2)).sum::<f64>() / used.len() as f64).sqrt();
        let plateau: Vec<f64> = series.iter().filter(|s| s.0 >= 0.02 && s.0 <= 0.06).map(|s| s.1).collect();
        let plat = plateau.iter().sum::<f64>() / plateau.len().max(1) as f64;
        println!(
            "EDWARDS | {} | {:.4}{} | {:.1} | {:.1} ({}) | {:.1e} | {} | {:.1}",
            name(d),
            t_reached,
            failed.as_ref().map(|e| format!(" FAILED: {e}")).unwrap_or_default(),
            plat,
            rmse,
            used.len(),
            max_err,
            steps,
            secs
        );
        let samples: Vec<String> = [0.02, 0.06, 0.1, 0.2, 0.3]
            .iter()
            .filter(|t| **t <= t_reached + 1e-12)
            .map(|t| format!("{t:.2}s:{:.0}", lerp(&ts, &ps, *t)))
            .collect();
        println!("EDWARDS-TRACE | {} | {}", name(d), samples.join(" "));
        assert!(max_err < 1e-12, "{} mass {max_err:e}", name(d));
        if d == Discretisation::FiniteVolume {
            assert!(failed.is_none(), "FV must complete");
            if (t_end - 0.30).abs() < 1e-12 {
                assert!((rmse - 44.2).abs() < 2.0, "FV RMSE {rmse} vs recorded 44.2 psia");
            }
        }
    }
    println!("EDWARDS | PCICE (gh:#1068) | {}", pcice_hook("edwards").unwrap_or_else(|| "pending".into()));
}
