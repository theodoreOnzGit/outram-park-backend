//! # V&V: Edwards–O'Brien pipe blowdown on the density-based `HemPipe`
//!
//! Issue gh #902 (epic gh #896). The same experiment as
//! `tests/edwards_blowdown.rs`, which runs the pressure-based PIMPLE array
//! `TampinesSteamArray`; this file runs the density-based KNP HEM solver
//! [`HemPipe`] on it. The two are kept side by side; neither overwrites the
//! other's numbers.
//!
//! ## Methodology
//!
//! - **Geometry**: horizontal pipe, `L = 4.096 m`, `D = 0.073 m`, closed at
//!   `x = 0`, break at `x = L` (B-T-3271 Table 1). Uniform mesh of 24 cells,
//!   one per RELAP volume, as the PIMPLE case (`EDW_HEM_CELLS` overrides it).
//! - **Initial condition**: 7.00 MPa uniform, Hendrie (1973) non-isothermal
//!   temperature profile interpolated to the cell centres, at rest; IF97
//!   `(p, T)` subcooled liquid.
//! - **Boundaries**: `Wall` at `x = 0`. `ChokedBreak` at `x = L` discharging to
//!   0.1 MPa, its area fraction ramped linearly 0 → 0.87 over 1 ms (the
//!   rupture disc). The break mass flux is this crate's HEM critical-flow
//!   dispatcher at the break cell's stagnation state, the same model the
//!   PIMPLE case feeds its outlet velocity from.
//! - **Numerics**: `HemNumerics::default()` (adaptive central Courant 0.2,
//!   van Leer, SSP22). Inviscid and frictionless: the PIMPLE case has no wall
//!   friction either.
//! - **Reference**: the digitised Edwards GS-1 "Data" curve (16 points,
//!   0–0.30 s). GS-1 is the cell whose centre is nearest 12.890 ft.
//! - **Pass criterion (honest gates only, fixed before the run):** the run
//!   completes to `t_end` with no error, and mass is conserved to round-off
//!   counting the break outflow: `|M(t) + M_out(t) − M(0)| ≤ 1e-12·M(0)`.
//!   The GS-1 RMSE and the 0.02–0.06 s plateau are **reported, not gated**.
//!   Nothing was tuned to them.
//!
//! ## Results (measured 2026-10-16, release, default numerics)
//!
//! | | HemPipe, 24 cells | HemPipe, 96 cells | PIMPLE record (24 cells, `Linear` he) |
//! |---|---|---|---|
//! | completes 0.6 s | yes, 4014 steps, 20.6 s | yes, 12602 steps, 175 s | yes |
//! | GS-1 plateau, mean 0.02–0.06 s | 364.9 psia | 372.1 psia | 359.0 psia |
//! | GS-1 RMSE vs data, 16 pts 0–0.30 s | 44.1 psia | 45.2 psia | 42.8 psia |
//! | mass conservation incl. break outflow | 4.2e-15 | 3.4e-15 | n/a |
//!
//! **CORRECTED 2026-10-16:** the four rows above were recorded from an
//! earlier state of this code. Re-measured on the committed code (identical
//! after the #900 and #903 merges): 24 cells gives 4025 steps, a 368.0 psia
//! plateau, 44.2 psia RMSE and 5.2e-15 mass error. 96 cells gives 12518 steps,
//! 372.4 psia, 45.1 psia and 4.9e-15. The PIMPLE record after #900 is 360.5 /
//! 17.2 psia. Full table: `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`.
//!
//! The experimental plateau is ~350–367 psia. Like the PIMPLE run, the HEM
//! pipe holds a plateau near the saturation pressure of the break-end liquid
//! and then depressurises too slowly: from 0.08 s on it is 20–70 psia above
//! the data (e.g. 383 vs 343 psia at 0.08 s, 258 vs 190 psia at 0.30 s). That
//! is the known direction of an equilibrium model's error for a subcooled
//! blowdown (no thermal non-equilibrium), and nothing was tuned to reduce it.
//! Past ~0.55 s GS-1 drops below the 0.1 MPa back pressure (11.4 psia at
//! 0.6 s on 24 cells): the break carries no backflow, so once the pipe is
//! below ambient the break acts as a closed end. The PIMPLE numbers are kept
//! exactly as `tests/edwards_blowdown.rs` records them; this file does not
//! change them. Full record:
//! `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`.

use petir::mathf::RealMath;
mod common;

use common::edwards_case::*;
use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// The PIMPLE Edwards numbers this run is reported beside
/// (`tests/edwards_blowdown.rs`, Linear scheme, 24 cells, 30 µs).
/// ~~359.0 / 42.8 psia~~ **updated 2026-10-16** to the values recorded after
/// gh:#900 changed that solver's defaults: 360.5 / 17.2 psia.
const PIMPLE_PLATEAU_PSIA: f64 = 360.5;
const PIMPLE_GS1_RMSE_PSIA: f64 = 17.2;

/// The full 0.6 s Edwards transient on `HemPipe` (see the module doc).
///
/// Wall clock measured 2026-10-16, release, 24 cells: ~~20.6 s (4014 steps)~~ 16.3 s (4025 steps, re-measured), so
/// it is not behind the `long-tests` feature (the 5-minute rule). The 96-cell
/// mesh-sensitivity run (`EDW_HEM_CELLS=96`) took 175 s and is not part of the
/// default run.
#[test]
fn edwards_obrien_blowdown_on_hem_pipe() {
    let wall_clock = std::time::Instant::now();
    let n_cells: usize = std::env::var("EDW_HEM_CELLS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let t_end: f64 = std::env::var("EDW_HEM_TEND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.6);

    let geo = HemPipeGeometry::new(
        Length::new::<meter>(PIPE_LENGTH_M),
        Area::new::<square_meter>(PIPE_AREA_M2),
        n_cells,
    )
    .unwrap();
    let cells: Vec<PrimitiveState> = (0..n_cells)
        .map(|i| PrimitiveState::PressureTemperature {
            pressure: Pressure::new::<pascal>(P_INIT_PA),
            temperature: ThermodynamicTemperature::new::<kelvin>(hendrie_temperature_k(
                geo.cell_centre(i).get::<meter>(),
            )),
            velocity: Velocity::new::<meter_per_second>(0.0),
        })
        .collect();
    let mut pipe: HemPipe = HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::PerCell(cells),
        HemNumerics::default(),
    )
    .expect("Edwards initial state");
    let back = Pressure::new::<pascal>(P_AMBIENT_PA);
    let gs1 = (0..n_cells)
        .min_by(|&a, &b| {
            let x = GS1_POS_FT * FT_TO_M;
            let da = (geo.cell_centre(a).get::<meter>() - x).abs();
            let db = (geo.cell_centre(b).get::<meter>() - x).abs();
            da.partial_cmp(&db).unwrap()
        })
        .unwrap();
    let m0 = pipe.total_mass().get::<kilogram>();

    // Sample every 1 ms; inside the 1 ms disc ramp, re-set the break area every 50 µs.
    let mut series: Vec<(f64, f64, f64, f64)> = vec![]; // (t, p_gs1 psia, void_gs1, break kg/s)
    let mut steps = 0usize;
    let mut max_co: f64 = 0.0;
    let mut last_m_out = 0.0;
    let mut last_t = 0.0;
    let mut t_next = 0.0;
    let mut max_mass_err: f64 = 0.0;
    let record = |pipe: &HemPipe, series: &mut Vec<(f64, f64, f64, f64)>, mdot: f64| {
        let c = pipe.cell_state(gs1).unwrap();
        series.push((
            pipe.time().get::<second>(),
            c.pressure.get::<pascal>() * PA_TO_PSI,
            c.void_fraction.get::<ratio>(),
            mdot,
        ));
    };
    record(&pipe, &mut series, 0.0);
    while t_next < t_end - 1e-12 {
        let dt_sample = if t_next < DISC_OPEN_TIME_S {
            5.0e-5
        } else {
            1.0e-3
        };
        t_next = (t_next + dt_sample).min(t_end);
        // Area at the middle of the interval (the ramp is linear).
        let t_mid = 0.5 * (pipe.time().get::<second>() + t_next);
        pipe.set_boundary(
            PipeEnd::Outlet,
            HemBoundary::ChokedBreak {
                area_fraction: Ratio::new::<ratio>(break_area_fraction(t_mid)),
                back_pressure: back,
            },
        )
        .unwrap();
        let r = pipe
            .run_until(Time::new::<second>(t_next))
            .unwrap_or_else(|e| {
                panic!(
                    "HemPipe Edwards failed near t = {:.5} s: {e}",
                    pipe.time().get::<second>()
                )
            });
        steps += r.steps;
        max_co = max_co.max(r.max_courant.get::<ratio>());
        let m_out = pipe
            .cumulative_mass_outflow(PipeEnd::Outlet)
            .get::<kilogram>();
        let t = pipe.time().get::<second>();
        let mdot = (m_out - last_m_out) / (t - last_t);
        last_m_out = m_out;
        last_t = t;
        let m = pipe.total_mass().get::<kilogram>();
        let inlet = pipe
            .cumulative_mass_outflow(PipeEnd::Inlet)
            .get::<kilogram>();
        max_mass_err = max_mass_err.max((m + m_out + inlet - m0).abs() / m0);
        if dt_sample >= 1.0e-3 || t_next >= DISC_OPEN_TIME_S {
            record(&pipe, &mut series, mdot);
        }
    }

    // GS-1 RMSE vs the digitised data (series sampled on the same 1 ms grid).
    let p_at = |t: f64| {
        let ts: Vec<f64> = series.iter().map(|s| s.0).collect();
        let ps: Vec<f64> = series.iter().map(|s| s.1).collect();
        lerp(&ts, &ps, t)
    };
    let used: Vec<&(f64, f64)> = GS1_DATA_PSIA
        .iter()
        .filter(|(t, _)| *t <= t_end + 1e-12)
        .collect();
    let rmse = (used
        .iter()
        .map(|(t, d)| (p_at(*t) - d).r_powi(2))
        .sum::<f64>()
        / used.len() as f64)
        .sqrt();
    let plateau: Vec<f64> = series
        .iter()
        .filter(|s| s.0 >= 0.02 && s.0 <= 0.06)
        .map(|s| s.1)
        .collect();
    let plateau_mean = plateau.iter().sum::<f64>() / plateau.len().max(1) as f64;
    let peak_mdot = series.iter().map(|s| s.3).fold(0.0, f64::max);

    println!("HemPipe Edwards: {n_cells} cells, GS-1 = cell {gs1}, t_end = {t_end} s, {steps} steps, max central Courant {max_co:.3}");
    println!("  wall clock {:.1} s", wall_clock.elapsed().as_secs_f64());
    println!("  GS-1 plateau (0.02-0.06 s mean) = {plateau_mean:.1} psia   [PIMPLE record {PIMPLE_PLATEAU_PSIA} psia; data ~350-367 psia]");
    println!("  GS-1 RMSE vs data ({} pts) = {rmse:.1} psia   [PIMPLE record {PIMPLE_GS1_RMSE_PSIA} psia]", used.len());
    println!("  peak break flow = {peak_mdot:.2} kg/s; mass conservation (incl. break outflow) max rel err = {max_mass_err:.3e}");
    println!("  t [s]   p_GS1 [psia]  data [psia]  void_GS1");
    for (t, d) in &used {
        let s = series
            .iter()
            .min_by(|a, b| (a.0 - t).abs().partial_cmp(&(b.0 - t).abs()).unwrap())
            .unwrap();
        println!(
            "  {:.3}   {:8.1}      {:8.1}     {:.3}",
            t,
            p_at(*t),
            d,
            s.2
        );
    }
    for s in series
        .iter()
        .filter(|s| ((s.0 * 1000.0).round() as i64) % 50 == 0 && s.0 > 0.3)
    {
        println!("  {:.3}   {:8.1}         -         {:.3}", s.0, s.1, s.2);
    }
    let csv: String = std::iter::once("t_s,p_gs1_psia,void_gs1,break_kg_s".to_string())
        .chain(
            series
                .iter()
                .map(|s| format!("{:.6},{:.4},{:.5},{:.4}", s.0, s.1, s.2, s.3)),
        )
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/verification_and_validation/hem_pipe_edwards_gs1.csv"
        ),
        csv,
    );

    assert!(
        (pipe.time().get::<second>() - t_end).abs() < 1e-12,
        "run did not reach t_end"
    );
    assert!(
        max_mass_err <= 1e-12,
        "mass not conserved: {max_mass_err:e}"
    );
}
