//! # V&V: Edwards–O'Brien pipe blowdown on `TampinesSteamArray`
//!
//! ## Methodology
//!
//! This is a verification & validation tutorial case that drives OUR current
//! 1-D compressible PIMPLE pipe array ([`TampinesSteamArray`], the
//! rhoPimpleFoam-derived array with a **real IAPWS-IF97 `(p,h)` two-phase
//! flash**, OpenFOAM-style pressure bounding, and panic-over-stale-state in
//! `correct_thermo`) through the classic Edwards–O'Brien horizontal-pipe
//! depressurisation experiment.
//!
//! - **Geometry** (Tomlinson & Aumiller B-T-3271, Table 1 = the modified
//!   RELAP5-3D nodalisation; Edwards & O'Brien 1970 experiment): a horizontal
//!   pipe `L = 4.096 m`, inside diameter `D = 0.073 m`, flow area
//!   `A = pi D^2 / 4 = 4.185e-3 m^2`. Closed at `x = 0`; a rupture disc at
//!   `x = L` (RELAP volume 24) opens to containment. The break flow area is
//!   `0.87 A` (13 % glass-disc residue) and opens over a `1.0 ms` linear ramp.
//! - **Mesh**: OUR array builds a *uniform* mesh; this case uses `24` cells,
//!   one per RELAP volume, so the seven pressure gauge stations GS-7 … GS-1
//!   (GS-7 at the closed end, GS-1 nearest the break) map cleanly by closest
//!   axial location.
//! - **Initial conditions**: `p = 7.00 MPa` (1000 psig) uniform; temperature is
//!   the **Hendrie (1973) non-isothermal axial profile** interpolated onto the
//!   cell centres (NOT isothermal — the single most important modelling detail
//!   per the benchmark). Enthalpy/density/`psi` follow from OUR `(p,T)`
//!   subcooled-liquid flash (`set_temperature_vector`).
//! - **Break boundary**: a choked (critical) outlet. Each step the last cell's
//!   `(p0,h0)` is fed to the crate's existing HEM critical-flow dispatcher
//!   [`get_critical_pressure_and_mass_flux_multiphase_ph`] (the same machinery
//!   validated against Moody / Zaloudek / Marviken), giving the throat mass
//!   flux `G*` and throat pressure. The break mass flow is
//!   `m_dot = G* * A_break(t)` and the equivalent full-face outlet velocity
//!   `U = m_dot / (rho * A)` is imposed via `set_outlet_velocity`. The closed
//!   end is `U = 0`; both ends keep a `ZeroGradient` pressure BC so the
//!   pressure floats on the array's compressibility diagonal (a blowdown has no
//!   Dirichlet pressure anywhere — the mass depletion sets `p`).
//! - **Transient**: the full 600 ms is integrated with `dt = 1.0e-4 s`
//!   (the benchmark's stipulated `dt_max`; acoustic CFL ~= 0.6 on this mesh).
//! - **Reference / tolerances**: gauge pressures are compared against the
//!   digitised Edwards experimental "Data" curve at GS-1 (below), reported as an
//!   RMSE in psia. Following the module V&V contract, assertions are
//!   **sanity-only** (finiteness, physical bounds) — HEM is a work in progress,
//!   so the deliverable is the reported error metric for the paper's V&V
//!   section, not a hard tolerance gate.
//!
//! ## Results (measured 2026-07-16, IAPWS-IF97 tables, 24 cells, dt = 30 us,
//! PIMPLE 4 outer / 4 inner PISO correctors, alpha_p = alpha_u = 1.0)
//!
//! The full 600 ms transient completes (20 000 steps, ~180 s wall) with no NaN
//! and all void fractions in [0, 1]. Headline numbers (printed with
//! `--nocapture`):
//!
//! - **Break flow**: peak 127.7 lbm/s (57.9 kg/s), choked. With the corrected
//!   energy closure (below) the break-cell enthalpy is no longer over-drained,
//!   so the HEM critical flux is higher than the earlier ~52 lbm/s and now sits
//!   near the ~96–111 lbm/s Henry–Fauske / experimental band (slightly above).
//!   The break unchokes (`choked = 0`) once the pipe reaches ambient.
//! - **Rarefaction wave / axial ordering** — captured: GS-1 (nearest break)
//!   leads GS-7 (closed end) in both depressurisation and void growth. Void
//!   grows *progressively* across the pipe (e.g. GS-3 ~0.65 by t = 58 ms) rather
//!   than jumping to 1.0 — physical flashing propagation.
//! - **Flashing plateau (GS-1)**: measured mean over 0.02–0.06 s ≈ **393 psia**,
//!   recovering the ~350–367 psia experimental plateau (now ~8 % high rather
//!   than the former ~20× under-prediction). GS-1 drops 1015 → ~374 psia by
//!   t = 1 ms and HOLDS ~380–394 psia through ~0.11 s at/near the saturation
//!   line `p_sat(T0(GS-1) = 498 K) ~ 354 psia`, then declines. The fluid no
//!   longer decompresses past saturation.
//! - **GS-1 pressure RMSE vs digitised experimental Data**: ≈ **60 psia** over
//!   16 points (0–0.30 s), down from ≈ 276 psia. Reported, not gated.
//!
//! ### Solver-physics fixes that recover the plateau (2026-07-16)
//!
//! Two coupled corrections, both purely thermodynamic (no clamps/floors):
//!
//! 1. **Conservative energy time derivative with the continuity density.**
//!    The energy equation's `d(rho h)/dt` is now discretised as
//!    `(rho_cont*h - rho_old*h_old)/dt` where `rho_cont = rho_old - dt*div(phi)`
//!    is rebuilt from the final mass flux — NOT the EOS `(p,h)` density that
//!    `correct_thermo` writes each inner corrector. Only the continuity density
//!    makes `(rho_cont - rho_old)/dt = -div(phi)` hold exactly, which cancels
//!    the `h*div(phi)` outflow term-for-term against `div(phi h)` and leaves the
//!    material derivative `rho Dh/Dt = dp/dt`. The former `fvm::ddt_coeff`
//!    (which reused the current rho for both time levels) solved
//!    `rho*dh/dt + div(phi h) = dp/dt`, whose un-cancelled `h*div(phi)`
//!    over-drained enthalpy during the flash and drove the bulk liquid subcooled
//!    (bead op-21g.14).
//! 2. **Compressibility `psi = d(rho)/dp|_h` (fixed enthalpy).** The pressure
//!    equation's compliance is now the finite-difference `d(rho)/dp` of the real
//!    `(p,h)` flash at fixed enthalpy, the correct linearisation for a
//!    segregated solve (he is frozen during the pressure correction). The former
//!    frozen quality-weighted isothermal `rho*kappa_T` omitted the flashing term
//!    `(v_g - v_f)*dx/dp` and was ~100x too small in the two-phase dome, so the
//!    pressure overshot straight through the plateau. The fixed-enthalpy
//!    compressibility carries the flashing compliance that pins a two-phase cell
//!    on the saturation line as it depressurises — the plateau.
//!
//! ### Artificial-cooling research question — ANSWERED: the artefact is absent
//!
//! Ethan's explicit Python mirror truncates at t = 0.413 s because, once the
//! pipe empties, its density/energy bookkeeping drifts, the recovered specific
//! internal energy collapses, the solver non-physically "cools the steam to
//! ~1 °C", and the gauge pressures **rebound** by hundreds of psia. OUR
//! pressure-correction (PIMPLE) array carries the full 600 ms **without** that
//! artefact:
//!
//! - Minimum temperature anywhere past t = 0.42 s is **~388 K (~115 °C)** at
//!   GS-1/GS-4/GS-7 — nowhere near the ~1 °C (274 K) collapse. Steam stays hot.
//! - GS-1 pressure past 0.42 s declines smoothly to ~24 psia; the tail "rebound"
//!   (tail-max minus value at 0.42 s) is **+0.0 psia** — no non-physical rebound
//!   (contrast: Ethan's tail rebounds by hundreds of psia).
//! - Void fraction stays in a physical two-phase range rather than collapsing
//!   back to cold dense liquid.
//!
//! So the pressure-based semi-implicit update cures the explicit scheme's
//! empty-pipe cooling/rebound tail — the run is physically stable across the
//! whole 600 ms. The test prints the GS-1/GS-4/GS-7 tail temperatures and
//! pressures so the reader can inspect the tail directly.
//!
//! ## Runtime knobs (V&V exploration, not part of the pass criterion)
//! Environment overrides: `EDW_DT_US` (timestep in microseconds, default 30),
//! `EDW_TEND` (end time in seconds, default 0.6), `EDW_DBG` (per-100-step
//! break/GS-1 trace to stderr). Smaller `EDW_DT_US` (e.g. 10) reduces the
//! acoustic-CFL overshoot at the initial rarefaction.

use tampines_steam_tables::{SolverMode, TampinesSteamArray};

use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    ph_flash_region, x_ph_flash,
};
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use tampines_steam_tables::region_1_subcooled_liquid::v_tp_1;
use tampines_steam_tables::region_2_vapour::v_tp_2;
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;
use tampines_steam_tables::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, Length, Pressure, ThermodynamicTemperature, Time, Velocity,
};
use uom::si::length::meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

// ─────────────────────────────────────────────────────────────────────────────
//  Geometry constants — Edwards–O'Brien pipe (B-T-3271 Table 1)
// ─────────────────────────────────────────────────────────────────────────────

/// Total pipe length [m] (13.44 ft).
const PIPE_LENGTH_M: f64 = 4.096;
/// Inside diameter [m] (2.88 in).
const PIPE_ID_M: f64 = 0.073;
/// Pipe cross-sectional flow area [m^2] = pi D^2 / 4.
const PIPE_AREA_M2: f64 = std::f64::consts::PI * PIPE_ID_M * PIPE_ID_M / 4.0;
/// Break flow area fraction (13 % reduction, glass-disc residue).
const BREAK_AREA_FRACTION: f64 = 0.87;
/// Rupture-disc opening time [s] (1.0 ms linear ramp).
const DISC_OPEN_TIME_S: f64 = 1.0e-3;
/// Initial pipe pressure [Pa abs] — 1000 psig.
const P_INIT_PA: f64 = 7.0e6;
/// Containment / ambient back-pressure [Pa abs].
const P_AMBIENT_PA: f64 = 1.0e5;

const FT_TO_M: f64 = 0.3048;
const PA_TO_PSI: f64 = 1.0 / 6894.757_293;
const KG_TO_LBM: f64 = 2.204_622_6;

/// Volume-centre axial position [ft from closed end] and initial temperature
/// [degF] — the Hendrie non-isothermal profile transcribed from B-T-3271
/// Table 1 (edwards_blowdown_relap.ods), volumes 1..24. GS markers noted in
/// prose: GS-7=vol 1 (closed end) … GS-1=vol 23 (nearest break).
const NODE_CENTRE_FT: [f64; 24] = [
    0.260, 0.780, 1.300, 1.820, 2.385, 3.000, 3.615, 4.215, 4.820, 5.425, 6.025, 6.640, 7.255,
    7.820, 8.340, 8.940, 9.630, 10.320, 10.920, 11.440, 11.905, 12.370, 12.890, 13.295,
];
const NODE_T_DEGF: [f64; 24] = [
    447.5, 448.4, 449.4, 450.3, 451.4, 452.5, 451.7, 450.8, 450.0, 449.2, 448.3, 447.5, 448.0,
    448.5, 448.9, 449.4, 450.0, 446.5, 443.4, 440.8, 438.4, 436.0, 437.0, 437.8,
];

/// Gauge-station axial positions [ft from the closed end], GS-1 … GS-7.
/// GS-1 is nearest the break (x = L); GS-7 is at the closed end (x = 0).
const GS_POS_FT: [(&str, f64); 7] = [
    ("GS-1", 12.890),
    ("GS-2", 12.370),
    ("GS-3", 9.630),
    ("GS-4", 6.640),
    ("GS-5", 4.820),
    ("GS-6", 3.000),
    ("GS-7", 0.260),
];

/// Digitised Edwards experimental "Data" pressure at GS-1 [(t_s, psia)],
/// traced from Tomlinson & Aumiller B-T-3271 Figure 3 (graphreader.com),
/// subsampled every 0.02 s. Provenance: `benchmark_digitised.csv`, series
/// "Data". Used to report a pressure RMSE — NOT a hard tolerance gate.
const GS1_DATA_PSIA: [(f64, f64); 16] = [
    (0.000, 985.0),
    (0.020, 350.777),
    (0.040, 364.038),
    (0.060, 367.358),
    (0.080, 343.024),
    (0.100, 312.239),
    (0.120, 298.234),
    (0.140, 296.0),
    (0.160, 288.68),
    (0.180, 288.394),
    (0.200, 289.0),
    (0.220, 283.112),
    (0.240, 271.433),
    (0.260, 252.96),
    (0.280, 228.0),
    (0.300, 190.093),
];

fn degf_to_k(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0 + 273.15
}

/// Piecewise-linear interpolation with flat extrapolation outside the range.
fn lerp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    for i in 0..xs.len() - 1 {
        if x >= xs[i] && x <= xs[i + 1] {
            let w = (x - xs[i]) / (xs[i + 1] - xs[i]);
            return ys[i] * (1.0 - w) + ys[i + 1] * w;
        }
    }
    *ys.last().unwrap()
}

/// Homogeneous (slip-free) void fraction from a `(p, h)` state, using OUR
/// IAPWS-IF97 flash. Subcooled → 0, superheated → 1, two-phase → x*v_g/v with
/// saturated specific volumes from the region-1/region-2 forward equations.
fn void_fraction(p_pa: f64, h_jkg: f64) -> f64 {
    let p = Pressure::new::<pascal>(p_pa);
    let h = AvailableEnergy::new::<joule_per_kilogram>(h_jkg);
    match ph_flash_region(p, h) {
        FwdEqnRegion::Region1 | FwdEqnRegion::Region3 => 0.0,
        FwdEqnRegion::Region2 | FwdEqnRegion::Region5 => 1.0,
        FwdEqnRegion::Region4 => {
            let x = x_ph_flash(p, h);
            let t_sat = sat_temp_4(p);
            let vf = v_tp_1(t_sat, p).get::<cubic_meter_per_kilogram>();
            let vg = v_tp_2(t_sat, p).get::<cubic_meter_per_kilogram>();
            let v = vf + x * (vg - vf);
            (x * vg / v).clamp(0.0, 1.0)
        }
    }
}

/// Break flow area [m^2] at simulation time `t` — linear rupture-disc ramp over
/// `DISC_OPEN_TIME_S`, then held at `BREAK_AREA_FRACTION * A_pipe`.
fn break_area(t: f64) -> f64 {
    let f = (t / DISC_OPEN_TIME_S).clamp(0.0, 1.0);
    f * BREAK_AREA_FRACTION * PIPE_AREA_M2
}

/// One choked-break sample: throat mass flux, throat pressure, choked flag.
struct BreakSample {
    mdot_kg_s: f64,
    p_throat_pa: f64,
    choked: bool,
}

/// Evaluate the critical (choked) break flow from the break-cell stagnation
/// state `(p0, h0)` at time `t`, via the crate's HEM critical-flow dispatcher.
fn choked_break(p0_pa: f64, h0_jkg: f64, rho0: f64, t: f64) -> (BreakSample, f64) {
    // No discharge until the pipe pressure exceeds ambient.
    if p0_pa <= P_AMBIENT_PA * 1.001 {
        return (
            BreakSample {
                mdot_kg_s: 0.0,
                p_throat_pa: p0_pa,
                choked: false,
            },
            0.0,
        );
    }
    let p0 = Pressure::new::<pascal>(p0_pa);
    let h0 = AvailableEnergy::new::<joule_per_kilogram>(h0_jkg);
    let (p_throat, g) = get_critical_pressure_and_mass_flux_multiphase_ph(p0, h0);
    let g = g.get::<kilogram_per_square_meter_second>().max(0.0);
    let p_throat_pa = p_throat.get::<pascal>();

    let a_break = break_area(t);
    let mdot = g * a_break; // [kg/s]
                            // Equivalent velocity on the FULL pipe face so m_dot is conserved through
                            // the smaller break area: U = m_dot / (rho * A_pipe).
    let u_face = mdot / (rho0.max(1e-4) * PIPE_AREA_M2);
    let choked = p_throat_pa > P_AMBIENT_PA * 1.001;
    (
        BreakSample {
            mdot_kg_s: mdot,
            p_throat_pa,
            choked,
        },
        u_face,
    )
}

#[test]
fn edwards_obrien_pipe_blowdown_600ms() {
    // ── Case parameters (kept tight for a release-mode test) ────────────────
    let n_cells: i64 = 24; // one uniform cell per RELAP volume
    let dt_us: f64 = std::env::var("EDW_DT_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30.0); // 30 us default → acoustic CFL ~0.25 in subcooled water
    let dt = Time::new::<second>(dt_us * 1.0e-6);
    let t_end = std::env::var("EDW_TEND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.600_f64);
    let dbg = std::env::var("EDW_DBG").is_ok();
    let sample_every = 0.001_f64; // record gauge series every 1 ms

    let mut array = TampinesSteamArray::new(
        Length::new::<meter>(PIPE_LENGTH_M),
        Area::new::<square_meter>(PIPE_AREA_M2),
        n_cells,
        dt,
    )
    .expect("valid Edwards pipe geometry");

    // Fast transient: use a genuine PISO configuration (no under-relaxation) so
    // each corrector takes its full step in real time. Four outer correctors,
    // four inner pressure correctors, alpha_p = alpha_u = 1.0. SIMPLE-style
    // under-relaxation (alpha_p < 1) was found to only marginally lift the
    // flashing plateau (~17 -> ~36 psia); the plateau is recovered instead by
    // the two solver-physics fixes documented in the module header (conservative
    // energy ddt + fixed-enthalpy compressibility), not by the relaxation knob.
    array.set_pimple_algorithm(
        4,
        4,
        uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(1.0),
        uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(1.0),
    );

    // Opt-in all-Mach hybrid mode for figure regeneration (EDW_HYBRID=1). The
    // default (unset) is pure PIMPLE, so this test's headline numbers above are
    // the PIMPLE baseline; the Pimple-vs-Hybrid comparison lives in the
    // dedicated `edwards_hybrid_damps_ringing_vs_pimple` test below.
    if std::env::var("EDW_HYBRID").is_ok() {
        array.set_solver_mode(SolverMode::HybridAllMach);
    }

    let n = array.mesh.n_cells;

    // ── Initial conditions ──────────────────────────────────────────────────
    // 1) uniform 7.00 MPa
    for c in 0..n {
        array.p.internal[c] = P_INIT_PA;
    }
    // 2) Hendrie non-isothermal temperature profile → sets he/rho/T/psi via the
    //    real (p,T) subcooled-liquid flash at the just-set 7 MPa.
    let centres_m: Vec<f64> = NODE_CENTRE_FT.iter().map(|&ft| ft * FT_TO_M).collect();
    let temps_k: Vec<f64> = NODE_T_DEGF.iter().map(|&f| degf_to_k(f)).collect();
    let cell_temps: Vec<ThermodynamicTemperature> = (0..n)
        .map(|c| {
            let xc = array.mesh.cell_centres[c].x;
            ThermodynamicTemperature::new::<kelvin>(lerp(&centres_m, &temps_k, xc))
        })
        .collect();
    array
        .set_temperature_vector(cell_temps)
        .expect("temperature profile length matches cell count");

    // ── Boundary conditions ─────────────────────────────────────────────────
    // Closed end = "left" patch (x = 0): U = 0. Break = "right" patch (x = L):
    // outlet velocity updated each step from the choked-flow solution. Both
    // pressure BCs stay ZeroGradient (constructor default) so p floats.
    array.set_inlet_velocity(Velocity::new::<meter_per_second>(0.0));

    // ── Map gauge stations onto cells (closest axial centre) ─────────────────
    let gs_cells: Vec<usize> = GS_POS_FT
        .iter()
        .map(|&(_, ft)| {
            let x = ft * FT_TO_M;
            (0..n)
                .min_by(|&a, &b| {
                    let da = (array.mesh.cell_centres[a].x - x).abs();
                    let db = (array.mesh.cell_centres[b].x - x).abs();
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap()
        })
        .collect();

    // ── Time-history buffers ────────────────────────────────────────────────
    let mut times: Vec<f64> = Vec::new();
    let mut p_gs: Vec<[f64; 7]> = Vec::new(); // psia
    let mut a_gs: Vec<[f64; 7]> = Vec::new(); // void fraction
    let mut t_gs: Vec<[f64; 7]> = Vec::new(); // K (for the cooling diagnostic)
    let mut break_hist: Vec<(f64, f64, f64, bool)> = Vec::new(); // (t, mdot, p_throat_pa, choked)

    let dt_s = dt.get::<second>();
    let n_steps = (t_end / dt_s).round() as usize;
    let sample_stride = (sample_every / dt_s).round().max(1.0) as usize;

    let mut time_s = 0.0_f64;

    // Record the initial state (t = 0).
    record(
        &array, &gs_cells, time_s, &mut times, &mut p_gs, &mut a_gs, &mut t_gs,
    );
    {
        let c = n - 1;
        let (bs, _u) = choked_break(
            array.p.internal[c],
            array.he.internal[c],
            array.rho.internal[c],
            time_s,
        );
        break_hist.push((time_s, bs.mdot_kg_s, bs.p_throat_pa, bs.choked));
    }

    // ── Transient loop ──────────────────────────────────────────────────────
    for step in 1..=n_steps {
        // Break BC from the current break-cell stagnation state.
        let c = n - 1;
        let (bs, u_face) = choked_break(
            array.p.internal[c],
            array.he.internal[c],
            array.rho.internal[c],
            time_s,
        );
        array.set_outlet_velocity(Velocity::new::<meter_per_second>(u_face));

        if dbg && step % 100 == 0 {
            let cb = n - 1;
            let cg1 = gs_cells[0];
            eprintln!(
                "step {step:5} t={time_s:.4} | break: p={:.0}kPa T={:.1}K a={:.3} | GS-1: p={:.0}psia T={:.1}K a={:.3} | mdot={:.2} u={:.2}",
                array.p.internal[cb] / 1e3,
                array.t.internal[cb],
                void_fraction(array.p.internal[cb], array.he.internal[cb]),
                array.p.internal[cg1] * PA_TO_PSI,
                array.t.internal[cg1],
                void_fraction(array.p.internal[cg1], array.he.internal[cg1]),
                bs.mdot_kg_s,
                u_face,
            );
        }

        array.step();
        time_s += dt_s;

        if step % sample_stride == 0 {
            record(
                &array, &gs_cells, time_s, &mut times, &mut p_gs, &mut a_gs, &mut t_gs,
            );
            // Break flow recomputed from the just-advanced state.
            let (bs2, _) = choked_break(
                array.p.internal[c],
                array.he.internal[c],
                array.rho.internal[c],
                time_s,
            );
            break_hist.push((time_s, bs2.mdot_kg_s, bs2.p_throat_pa, bs2.choked));
        }
        // Silence unused-warning for the pre-step sample's fields.
        let _ = &bs;
    }

    // ── Sanity assertions (finiteness / physical bounds only) ───────────────
    for c in 0..n {
        assert!(
            array.p.internal[c].is_finite(),
            "cell {c} pressure went non-finite"
        );
        assert!(
            array.he.internal[c].is_finite() && array.t.internal[c].is_finite(),
            "cell {c} enthalpy/temperature went non-finite"
        );
    }
    for row in &a_gs {
        for &al in row {
            assert!(
                (0.0..=1.0).contains(&al) && al.is_finite(),
                "void fraction out of [0,1]: {al}"
            );
        }
    }
    for row in &p_gs {
        for &pp in row {
            assert!(
                pp.is_finite() && pp > 0.0,
                "gauge pressure non-physical: {pp}"
            );
        }
    }

    // ── Write CSVs (Ethan's plot_figures.py wide format) ─────────────────────
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation");
    std::fs::create_dir_all(out_dir).unwrap();
    write_pressure_csv(out_dir, &times, &p_gs);
    write_void_csv(out_dir, &times, &a_gs);
    write_break_csv(out_dir, &break_hist);

    // ── Reported metrics ────────────────────────────────────────────────────
    let gs1 = 0usize; // GS-1 column
                      // Interpolate sim GS-1 pressure onto the digitised Data timestamps.
    let sim_t: Vec<f64> = times.clone();
    let sim_p_gs1: Vec<f64> = p_gs.iter().map(|r| r[gs1]).collect();
    let mut sq = 0.0;
    let mut cnt = 0.0;
    for &(td, pd) in GS1_DATA_PSIA.iter() {
        let ps = lerp(&sim_t, &sim_p_gs1, td);
        sq += (ps - pd) * (ps - pd);
        cnt += 1.0;
    }
    let rmse_gs1 = (sq / cnt).sqrt();

    // Flashing plateau: mean GS-1 pressure over 0.02–0.06 s.
    let plateau: f64 = {
        let vals: Vec<f64> = times
            .iter()
            .zip(sim_p_gs1.iter())
            .filter(|(&t, _)| (0.02..=0.06).contains(&t))
            .map(|(_, &p)| p)
            .collect();
        if vals.is_empty() {
            f64::NAN
        } else {
            vals.iter().sum::<f64>() / vals.len() as f64
        }
    };

    let mdot_peak_lbm = break_hist
        .iter()
        .map(|&(_, m, _, _)| m * KG_TO_LBM)
        .fold(0.0_f64, f64::max);
    let mdot_t0_lbm = break_hist[0].1 * KG_TO_LBM;

    // ── Artificial-cooling diagnostic: temperatures/pressures past 0.42 s ────
    // GS-1 (near break), GS-4 (mid), GS-7 (closed end) minima and end-state.
    let idx_042 = times.iter().position(|&t| t >= 0.42).unwrap_or(times.len());
    let tail_min_t: [f64; 3] = [gs1, 3, 6].map(|col| {
        t_gs[idx_042..]
            .iter()
            .map(|r| r[col])
            .fold(f64::INFINITY, f64::min)
    });
    let tail_end_p: [f64; 3] = [gs1, 3, 6].map(|col| *p_gs.last().unwrap().get(col).unwrap());
    // Does GS-1 pressure rebound in the tail (a jump > 50 psia above a prior
    // local level after 0.42 s)? Compare tail max against the value at 0.42 s.
    let p_at_042 = p_gs[idx_042.min(p_gs.len() - 1)][gs1];
    let tail_max_p_gs1 = p_gs[idx_042..]
        .iter()
        .map(|r| r[gs1])
        .fold(0.0_f64, f64::max);
    let rebound = tail_max_p_gs1 - p_at_042; // >0 and large ⇒ non-physical rebound

    println!("\n================ Edwards–O'Brien blowdown — reported metrics ================");
    println!(
        "mesh: {n} uniform cells, dt = {:.1e} s, t_end = {:.3} s",
        dt_s, t_end
    );
    println!(
        "break flow at t=0        : {:.2} kg/s ({:.1} lbm/s)",
        break_hist[0].1, mdot_t0_lbm
    );
    println!("break flow peak          : {:.1} lbm/s", mdot_peak_lbm);
    println!("GS-1 flashing plateau    : {plateau:.1} psia (mean 0.02–0.06 s)");
    println!("GS-1 pressure RMSE vs Data: {rmse_gs1:.1} psia (16 pts, 0–0.30 s)");
    println!("GS-1 p @ t_end           : {:.1} psia", tail_end_p[0]);
    println!("--- artificial-cooling tail check (t > 0.42 s) ---");
    println!(
        "min T past 0.42 s [K]    : GS-1 {:.1}, GS-4 {:.1}, GS-7 {:.1}",
        tail_min_t[0], tail_min_t[1], tail_min_t[2]
    );
    println!(
        "           [degC]        : GS-1 {:.1}, GS-4 {:.1}, GS-7 {:.1}",
        tail_min_t[0] - 273.15,
        tail_min_t[1] - 273.15,
        tail_min_t[2] - 273.15
    );
    println!(
        "end p [psia]             : GS-1 {:.1}, GS-4 {:.1}, GS-7 {:.1}",
        tail_end_p[0], tail_end_p[1], tail_end_p[2]
    );
    println!("GS-1 tail p rebound      : {rebound:+.1} psia (tail max − p@0.42s)");
    println!("=============================================================================\n");

    // Sanity: the artificial-cooling artefact would drive T toward ~274 K
    // (~1 degC). We only report it (no hard gate), but flag it loudly.
    let cold = tail_min_t.iter().any(|&tk| tk < 275.0);
    println!(
        "artificial-cooling verdict: {}",
        if cold {
            "COLD tail present (T < 2 degC somewhere past 0.42 s)"
        } else {
            "NO cold tail — steam stays hot past 0.42 s (artefact absent)"
        }
    );
}

/// Sample every gauge station into the history buffers.
fn record(
    array: &TampinesSteamArray,
    gs_cells: &[usize],
    time_s: f64,
    times: &mut Vec<f64>,
    p_gs: &mut Vec<[f64; 7]>,
    a_gs: &mut Vec<[f64; 7]>,
    t_gs: &mut Vec<[f64; 7]>,
) {
    times.push(time_s);
    let mut pr = [0.0; 7];
    let mut ar = [0.0; 7];
    let mut tr = [0.0; 7];
    for (i, &c) in gs_cells.iter().enumerate() {
        let p = array.p.internal[c];
        let h = array.he.internal[c];
        pr[i] = p * PA_TO_PSI;
        ar[i] = void_fraction(p, h);
        tr[i] = array.t.internal[c];
    }
    p_gs.push(pr);
    a_gs.push(ar);
    t_gs.push(tr);
}

fn write_pressure_csv(dir: &str, times: &[f64], p_gs: &[[f64; 7]]) {
    use std::io::Write;
    let mut f = std::fs::File::create(format!("{dir}/sim_pressure_gs.csv")).unwrap();
    writeln!(f, "time_s,GS-1,GS-2,GS-3,GS-4,GS-5,GS-6,GS-7").unwrap();
    for (t, row) in times.iter().zip(p_gs) {
        write!(f, "{t:.6}").unwrap();
        for v in row {
            write!(f, ",{v:.4}").unwrap();
        }
        writeln!(f).unwrap();
    }
}

fn write_void_csv(dir: &str, times: &[f64], a_gs: &[[f64; 7]]) {
    use std::io::Write;
    let mut f = std::fs::File::create(format!("{dir}/sim_void_gs.csv")).unwrap();
    writeln!(f, "time_s,GS-1,GS-2,GS-3,GS-4,GS-5,GS-6,GS-7").unwrap();
    for (t, row) in times.iter().zip(a_gs) {
        write!(f, "{t:.6}").unwrap();
        for v in row {
            write!(f, ",{v:.5}").unwrap();
        }
        writeln!(f).unwrap();
    }
}

fn write_break_csv(dir: &str, hist: &[(f64, f64, f64, bool)]) {
    use std::io::Write;
    let mut f = std::fs::File::create(format!("{dir}/sim_break_flow.csv")).unwrap();
    writeln!(f, "time_s,mdot_kg_s,mdot_lbm_s,p_throat_psia,choked").unwrap();
    for &(t, m, p_throat, choked) in hist {
        writeln!(
            f,
            "{t:.6},{m:.5},{:.5},{:.4},{}",
            m * KG_TO_LBM,
            p_throat * PA_TO_PSI,
            if choked { 1 } else { 0 }
        )
        .unwrap();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Pimple-vs-Hybrid all-Mach comparison (bead op-21g.15.6)
// ─────────────────────────────────────────────────────────────────────────────

/// Drive the Edwards pipe for `t_end_s` in the given [`SolverMode`], returning
/// `(times [s], gauge pressures [psia])` sampled every 1 ms. Identical geometry,
/// IC, break BC and PISO settings to `edwards_obrien_pipe_blowdown_600ms`; the
/// *only* difference between the two calls in the comparison test is `mode`.
fn run_gauge_pressures(mode: SolverMode, t_end_s: f64, dt_us: f64) -> (Vec<f64>, Vec<[f64; 7]>) {
    let n_cells: i64 = 24;
    let dt = Time::new::<second>(dt_us * 1.0e-6);

    let mut array = TampinesSteamArray::new(
        Length::new::<meter>(PIPE_LENGTH_M),
        Area::new::<square_meter>(PIPE_AREA_M2),
        n_cells,
        dt,
    )
    .expect("valid Edwards pipe geometry");
    array.set_pimple_algorithm(
        4,
        4,
        uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(1.0),
        uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(1.0),
    );
    array.set_solver_mode(mode);

    let n = array.mesh.n_cells;
    for c in 0..n {
        array.p.internal[c] = P_INIT_PA;
    }
    let centres_m: Vec<f64> = NODE_CENTRE_FT.iter().map(|&ft| ft * FT_TO_M).collect();
    let temps_k: Vec<f64> = NODE_T_DEGF.iter().map(|&f| degf_to_k(f)).collect();
    let cell_temps: Vec<ThermodynamicTemperature> = (0..n)
        .map(|c| {
            let xc = array.mesh.cell_centres[c].x;
            ThermodynamicTemperature::new::<kelvin>(lerp(&centres_m, &temps_k, xc))
        })
        .collect();
    array.set_temperature_vector(cell_temps).unwrap();
    array.set_inlet_velocity(Velocity::new::<meter_per_second>(0.0));

    let gs_cells: Vec<usize> = GS_POS_FT
        .iter()
        .map(|&(_, ft)| {
            let x = ft * FT_TO_M;
            (0..n)
                .min_by(|&a, &b| {
                    let da = (array.mesh.cell_centres[a].x - x).abs();
                    let db = (array.mesh.cell_centres[b].x - x).abs();
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap()
        })
        .collect();

    let dt_s = dt.get::<second>();
    let n_steps = (t_end_s / dt_s).round() as usize;
    let sample_stride = (0.001_f64 / dt_s).round().max(1.0) as usize;

    let mut times: Vec<f64> = Vec::new();
    let mut p_gs: Vec<[f64; 7]> = Vec::new();
    let mut time_s = 0.0_f64;

    let record =
        |array: &TampinesSteamArray, times: &mut Vec<f64>, p_gs: &mut Vec<[f64; 7]>, t: f64| {
            times.push(t);
            let mut pr = [0.0; 7];
            for (i, &c) in gs_cells.iter().enumerate() {
                pr[i] = array.p.internal[c] * PA_TO_PSI;
            }
            p_gs.push(pr);
        };
    record(&array, &mut times, &mut p_gs, time_s);

    for step in 1..=n_steps {
        let c = n - 1;
        let (_bs, u_face) = choked_break(
            array.p.internal[c],
            array.he.internal[c],
            array.rho.internal[c],
            time_s,
        );
        array.set_outlet_velocity(Velocity::new::<meter_per_second>(u_face));
        array.step();
        time_s += dt_s;
        if step % sample_stride == 0 {
            record(&array, &mut times, &mut p_gs, time_s);
        }
    }
    (times, p_gs)
}

/// Excess total variation of a series over the window `[t0, t1]` — the "wiggle"
/// (ringing) amplitude on top of the monotone blowdown trend:
/// `Σ|p[i+1]−p[i]|  −  |p_first − p_last|`. A perfectly monotone decay scores 0;
/// oscillation adds twice each overshoot. Units follow the input (psia here).
fn excess_total_variation(times: &[f64], series: &[f64], t0: f64, t1: f64) -> f64 {
    let idx: Vec<usize> = (0..times.len())
        .filter(|&i| times[i] >= t0 && times[i] <= t1)
        .collect();
    if idx.len() < 2 {
        return 0.0;
    }
    let mut tv = 0.0;
    for w in idx.windows(2) {
        tv += (series[w[1]] - series[w[0]]).abs();
    }
    let net = (series[*idx.last().unwrap()] - series[idx[0]]).abs();
    (tv - net).max(0.0)
}

/// # V&V: all-Mach hybrid KNP dissipation damps the near-sonic ringing
///
/// ## Methodology
///
/// The Edwards–O'Brien pipe blowdown ([`edwards_obrien_pipe_blowdown_600ms`]) is
/// run twice on the identical 24-cell mesh, IC (7 MPa + Hendrie axial temperature
/// profile), choked-break BC and 4-outer/4-inner PISO settings — the *only*
/// difference is [`SolverMode`]: pure `Pimple` (`β = 0` everywhere) versus
/// `HybridAllMach` (Mach-blended KNP central-upwind dissipation, blend window
/// `lo = 0.3`, `hi = 1.0`, sound speed the **HEM equilibrium** speed). The window
/// is `0–0.15 s` at `dt = 30 µs`, which spans the initial rarefaction ringing and
/// the `0.02–0.06 s` flashing plateau.
///
/// - **Ringing metric**: excess total variation (wiggle on top of the monotone
///   blowdown) of each gauge's pressure trace over `0–0.15 s`
///   ([`excess_total_variation`]). Summed over GS-1…GS-7. Lower ⇒ less ringing.
/// - **Plateau metric**: mean GS-1 pressure over `0.02–0.06 s` — must be retained
///   (the flashing plateau / no-subcooling result the PIMPLE fix recovered).
///
/// ## Results (measured 2026-07-16, IAPWS-IF97 tables, 24 cells, dt = 30 µs,
/// window 0–0.15 s; full trail in
/// `collaboration/edwards_tampines_regen/hybrid_debug_log.md`)
///
/// The Mach-blended KNP dissipation **damps the near-sonic ringing** while
/// **retaining the flashing plateau**. Excess total variation (ringing) per
/// gauge, Pimple → Hybrid (drop %):
///
/// ```text
/// gauge   excess-TV Pimple   excess-TV Hybrid   drop %
/// GS-1              454.7             182.4      59.9
/// GS-2             2169.2             714.9      67.0
/// GS-3             1027.9             303.3      70.5
/// GS-4              441.7             442.0      -0.1
/// GS-5              267.6             262.1       2.1
/// GS-6               22.6              23.1      -2.3
/// GS-7              286.0             151.6      47.0
/// SUM              4669.6            2079.3      55.5   ← 55.5 % less ringing
/// ```
///
/// - **Ringing**: summed excess-TV drops **4670 → 2079 psia (−55.5 %)**; the
///   largest drops are at the near-break/interior gauges GS-2 (−67 %) and GS-3
///   (−70 %) where the near-sonic flashing front rings hardest, and GS-1 (−60 %)
///   nearest the break. GS-4/5/6 (in less-oscillatory zones) are unchanged to
///   ±2 %.
/// - **Plateau**: GS-1 mean over 0.02–0.06 s is **392.7 psia (Pimple) → 387.7
///   psia (Hybrid)**, a 1.3 % change — the flashing plateau / no-subcooling
///   result is retained (the min-Ma-gated hybrid tracks the pure-PIMPLE
///   trajectory in the subcooled bulk essentially exactly).
///
/// The design that achieves this: continuity + momentum KNP dissipation, the
/// blend gated on `min(Ma)` of the two adjacent cells (low-Mach scaling; without
/// it the liquid-side acoustic speed at a liquid/two-phase interface makes the
/// viscosity ~35× the physical mass flux and over-drains enthalpy out of the
/// `(p,h)` validity range). No separate energy source — the enthalpy
/// shock-capturing rides on the continuity flux through the EEqn `∇·(φh)` so the
/// plateau-fix cancellation is preserved. HEM equilibrium sound speed
/// throughout (Kieffer eq. 28 in the dome, `w_tp_1/2/3/5` single-phase).
///
/// The assertions below (finiteness; ringing must not grow; plateau retained
/// within 15 %) are sanity gates per the module V&V contract; the reported
/// numbers are the deliverable.
///
/// ## Full-transient hybrid stability (bug op-21g.15.7 — FIXED)
///
/// `HybridAllMach` is stable over the **full 600 ms** transient. The earlier
/// late-time instability past `t ≈ 0.18 s` — as the pipe empties, a rarefying
/// near-sonic cell's continuity density `ρ_cont` collapses to its floor, the
/// segregated EEqn diagonal `ρ_cont·V/dt` vanishes, and the explicit KNP deferred
/// correction over-drives that nearly-empty cell across the IAPWS-IF97 273.15 K
/// `(p,h)` validity edge (`correct_thermo` correctly panics) — is removed by the
/// **rarefied-tail density taper** on the KNP blend (`assemble_hybrid_dissipation`):
/// the dissipation is scaled to zero below `ρ = 50 kg/m³` and full above
/// `ρ = 100 kg/m³`. The minimum dissipated-face density over the 0–0.15 s ringing
/// window is ≈ 106.5 kg/m³, so the taper is inert there — the ~55 % ringing
/// reduction and ≈ 388 psia plateau are unchanged — while the late-time emptying
/// tail (ρ → few kg/m³, where the HEM closure degrades and there is no flashing
/// shock to capture) reverts to pure PIMPLE, which is stable.
///
/// Measured (2026-07-16, 24 cells, dt = 30 µs, full 600 ms in `HybridAllMach`):
/// plateau 387.7 psia, GS-1 RMSE vs Data 30.6 psia, no cold tail (min tail
/// T ≈ 372 K). The `EDW_HYBRID=1` full-run figures are in
/// `collaboration/edwards_tampines_regen/figures_hybrid_stable/`.
#[test]
fn edwards_hybrid_damps_ringing_vs_pimple() {
    let t_end = std::env::var("EDW_CMP_TEND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.15_f64);
    let dt_us = std::env::var("EDW_DT_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30.0_f64);

    let (t_p, p_p) = run_gauge_pressures(SolverMode::Pimple, t_end, dt_us);
    let (t_h, p_h) = run_gauge_pressures(SolverMode::HybridAllMach, t_end, dt_us);

    // Per-gauge excess-TV (ringing) and the sum across all seven stations.
    let mut tv_p = [0.0_f64; 7];
    let mut tv_h = [0.0_f64; 7];
    for g in 0..7 {
        let sp: Vec<f64> = p_p.iter().map(|r| r[g]).collect();
        let sh: Vec<f64> = p_h.iter().map(|r| r[g]).collect();
        tv_p[g] = excess_total_variation(&t_p, &sp, 0.0, t_end);
        tv_h[g] = excess_total_variation(&t_h, &sh, 0.0, t_end);
    }
    let sum_p: f64 = tv_p.iter().sum();
    let sum_h: f64 = tv_h.iter().sum();

    // Flashing plateau (mean GS-1 over 0.02–0.06 s) for each mode.
    let plateau = |times: &[f64], p_gs: &[[f64; 7]]| -> f64 {
        let vals: Vec<f64> = times
            .iter()
            .zip(p_gs.iter())
            .filter(|(&t, _)| (0.02..=0.06).contains(&t))
            .map(|(_, r)| r[0])
            .collect();
        if vals.is_empty() {
            f64::NAN
        } else {
            vals.iter().sum::<f64>() / vals.len() as f64
        }
    };
    let plat_p = plateau(&t_p, &p_p);
    let plat_h = plateau(&t_h, &p_h);

    println!("\n===== Edwards Pimple vs HybridAllMach ({t_end:.3} s, dt = {dt_us} us) =====");
    println!("gauge |   excess-TV Pimple |   excess-TV Hybrid |   drop %");
    for (g, name) in GS_POS_FT.iter().map(|(n, _)| n).enumerate() {
        let drop = if tv_p[g] > 1e-9 {
            100.0 * (tv_p[g] - tv_h[g]) / tv_p[g]
        } else {
            0.0
        };
        println!(
            "{name:>5} | {:>18.2} | {:>18.2} | {:>8.1}",
            tv_p[g], tv_h[g], drop
        );
    }
    let sum_drop = if sum_p > 1e-9 {
        100.0 * (sum_p - sum_h) / sum_p
    } else {
        0.0
    };
    println!("  SUM | {sum_p:>18.2} | {sum_h:>18.2} | {sum_drop:>8.1}");
    println!(
        "GS-1 flashing plateau (0.02-0.06 s): Pimple {plat_p:.1} psia, Hybrid {plat_h:.1} psia"
    );
    println!("==============================================================================\n");

    // Sanity (finiteness) — per the module V&V contract.
    for g in 0..7 {
        assert!(
            tv_p[g].is_finite() && tv_h[g].is_finite(),
            "excess-TV non-finite"
        );
    }
    assert!(
        plat_p.is_finite() && plat_h.is_finite(),
        "plateau non-finite"
    );
    // Ringing must not grow, and the flashing plateau must be retained.
    assert!(
        sum_h <= sum_p * 1.02 + 1.0,
        "hybrid ringing grew: sum_h = {sum_h:.2} vs sum_p = {sum_p:.2}"
    );
    assert!(
        (plat_h - plat_p).abs() <= 0.15 * plat_p,
        "flashing plateau not retained: Pimple {plat_p:.1} vs Hybrid {plat_h:.1} psia"
    );
}
