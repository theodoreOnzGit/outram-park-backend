//! The helical coil section (gh:#1157): a few turns of a pipe coil meshed by
//! `outram_foam_mesh::block_primitives::HelicalPipe`, solved by `PimpleFoam`
//! on `cases::laminar_pipe`'s boundary conditions, measured with
//! `cases::helical_coil`, and compared with Mishra & Gupta (1979), called
//! from `tampines::components::helical_coil_otsg::correlations`.
//!
//! **The physics is not here**: this file sizes the grids, picks the
//! measuring windows fixed in the V&V record
//! (`crates/outram-foam-appbuilder-lib/verification_and_validation/helical_coil_dean_gh1157.md`)
//! and steps the solver. The demo engine and the `--coil-run` V&V command
//! both use it, so the numbers in the record and on the page come from one
//! code path.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::double_pipe::pseudo_time_step;
use outram_foam_appbuilder_lib::cases::helical_coil::{arc_pressure_gradient, dean_section, CoilCells, CoilFlow, DeanSection};
use outram_foam_appbuilder_lib::cases::laminar_pipe::PipeFlow;
use outram_foam_appbuilder_lib::io::fv_schemes::DivScheme;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_mesh::block_primitives::HelicalPipe;
use tampines::components::helical_coil_otsg::correlations::darcy_friction_laminar_mishra_gupta;
use uom::si::f64::{KinematicViscosity, Length, Velocity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Bulk velocity of the coil runs \[m/s\] (Re is set through ν).
pub const U_COIL: f64 = 5.0e-3;

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

/// A coil in metres: tube radius, coil diameter at the tube centreline,
/// pitch, turns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoilSpec {
    pub tube_radius: f64,
    pub coil_diameter: f64,
    pub pitch: f64,
    pub turns: f64,
}

impl CoilSpec {
    /// The V&V coil: d = 10 mm, D_w = 100 mm, h = 20 mm, 3 turns
    /// (d/D_c = 0.0996).
    pub fn vv() -> Self {
        Self { tube_radius: 0.005, coil_diameter: 0.1, pitch: 0.02, turns: 3.0 }
    }

    /// The straight-pipe limit of the V&V record: D_w = 10 m, 0.01 turns
    /// (31.4 d), pitch 0.
    pub fn straight_limit() -> Self {
        Self { tube_radius: 0.005, coil_diameter: 10.0, pitch: 0.0, turns: 0.01 }
    }

    /// The coil's mesh with `n`, `nr` and `layers`.
    pub fn grid(&self, n: usize, nr: usize, layers: usize) -> HelicalPipe {
        HelicalPipe::new(m(self.tube_radius), m(self.coil_diameter), m(self.pitch), self.turns, n, nr, layers)
    }

    /// Arc length \[m\].
    pub fn length(&self) -> f64 {
        self.grid(1, 1, 1).arc_length().get::<meter>()
    }

    /// The V&V family: `n = nr = 2m`, `round(m L/d)` layers (Δs = d/m).
    pub fn level(&self, lvl: usize) -> HelicalPipe {
        let d = 2.0 * self.tube_radius;
        let layers = ((lvl as f64 * self.length() / d).round() as usize).max(1);
        self.grid(2 * lvl, 2 * lvl, layers)
    }

    /// The demo's grid for a cell budget: the cross-section `(n, n − 2)` of
    /// the demo pipe (its thicker ring cells keep the viscous number down,
    /// `laminar_pipe_gh1155.md`), with as many layers as the budget allows
    /// up to 3 per diameter; the largest `n ≥ 3` whose layers are then still
    /// no longer than `d`.
    pub fn for_budget(&self, target: usize) -> HelicalPipe {
        let d = 2.0 * self.tube_radius;
        let l = self.length();
        let make = |n: usize| {
            let sec = n * n + 4 * n * (n - 2).max(1);
            let fit = (target as f64 / sec as f64).floor() as usize;
            let layers = fit.min((3.0 * l / d).round() as usize).max(1);
            self.grid(n, (n - 2).max(1), layers)
        };
        let mut best = make(3);
        for n in 3..32 {
            let g = make(n);
            if g.n_layers as f64 * d / l < 1.0 - 1e-9 && n > 3 {
                break;
            }
            best = g;
        }
        best
    }
}

/// Each cell's coil coordinates (`HelicalPipe::locate` at its centre).
pub fn coil_cells(grid: &HelicalPipe, fv: &FvMesh) -> CoilCells {
    let mut c = CoilCells::default();
    for x in &fv.cell_centres {
        let q = grid.locate([x.x, x.y, x.z]);
        c.s.push(q.s);
        c.n.push(q.n);
        c.b.push(q.b);
        c.frenet.push(q.frenet);
    }
    c
}

/// A coil being solved.
pub struct CoilRun {
    pub grid: HelicalPipe,
    pub solver: PimpleFoam,
    pub flow: CoilFlow,
    pub cells: CoilCells,
    /// Axial cell length \[m\].
    pub ds: f64,
    pub steps: usize,
}

/// What a run measured (see the V&V record's instruments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoilReport {
    pub reynolds: f64,
    pub dean: f64,
    /// f·Re/64 on the last measuring window (turn 3 of the V&V coil) and on
    /// the one before it (the development check).
    pub f_re_64: f64,
    pub f_re_64_before: f64,
    /// Mishra & Gupta's f_c/f_s at this Re and d/D_c (from tampines).
    pub mishra_gupta: f64,
    pub section: DeanSection,
    /// Where the section was taken \[m of arc\].
    pub section_s: f64,
}

impl CoilRun {
    /// The case on `fv` (the mesh of `grid`) at `reynolds`, with convection
    /// `scheme` (the V&V record's choice is `GaussLinear`), Courant number
    /// `co` on the axial cell length capped at viscous number `max_viscous`,
    /// one non-orthogonal corrector.
    pub fn on_mesh(grid: HelicalPipe, fv: Arc<FvMesh>, reynolds: f64, scheme: DivScheme, co: f64, max_viscous: f64) -> Self {
        let r = grid.radius.get::<meter>();
        let l = grid.arc_length();
        let pipe = PipeFlow {
            radius: grid.radius,
            length: l,
            bulk_velocity: Velocity::new::<meter_per_second>(U_COIL),
            nu: KinematicViscosity::new::<square_meter_per_second>(U_COIL * 2.0 * r / reynolds.max(1e-3)),
        };
        let flow = CoilFlow { pipe, curvature_diameter: grid.curvature_diameter() };
        let ds = l.get::<meter>() / grid.n_layers as f64;
        let dt = pseudo_time_step(&fv, pipe.bulk_velocity, pipe.nu, m(ds), co, max_viscous);
        let cells = coil_cells(&grid, &fv);
        let mut solver = pipe.build_solver(fv, dt);
        solver.solution.pimple.n_non_orthogonal_correctors = 1;
        solver.schemes.default_div = scheme;
        Self { grid, solver, flow, cells, ds, steps: 0 }
    }

    /// One time step; the `laminar_pipe` steady metric.
    ///
    /// # Errors
    /// Divergence, as text.
    pub fn step(&mut self) -> Result<f64, String> {
        let c = self.flow.pipe.step(&mut self.solver).map_err(|e| e.to_string())?;
        self.steps += 1;
        if c.is_finite() {
            Ok(c)
        } else {
            Err("the solve diverged".into())
        }
    }

    /// The measuring window width: one turn, at most half the tube.
    fn window(&self) -> f64 {
        let lt = self.grid.arc_length().get::<meter>() / self.grid.turns.max(1e-9);
        lt.min(0.5 * self.grid.arc_length().get::<meter>())
    }

    /// The report: f·Re/64 on `[L − w, L − 2d]` and `[L − 2w, L − w]`
    /// (w one turn, at most L/2), the Dean section on the layer at
    /// `L − w/2`, and Mishra & Gupta.
    pub fn report(&self) -> CoilReport {
        let l = self.grid.arc_length().get::<meter>();
        let d = 2.0 * self.grid.radius.get::<meter>();
        let w = self.window();
        let p = &self.flow.pipe;
        let f = |a: f64, b: f64| p.friction_factor(arc_pressure_gradient(&self.solver, &self.cells, m(a), m(b))) * p.reynolds() / 64.0;
        let s_sec = ((l - 0.5 * w) / self.ds).floor() * self.ds + 0.5 * self.ds;
        let re = self.flow.reynolds();
        let ratio = self.flow.curvature_ratio();
        CoilReport {
            reynolds: re,
            dean: self.flow.dean_number(),
            f_re_64: f(l - w, l - 2.0 * d),
            f_re_64_before: f(l - 2.0 * w, l - w),
            mishra_gupta: darcy_friction_laminar_mishra_gupta(re, ratio) * re / 64.0,
            section: dean_section(&self.solver, &self.cells, p, m(s_sec), m(0.5 * self.ds)),
            section_s: s_sec,
        }
    }
}

/// Run `grid` at `reynolds` (Courant number `co`, 4 in the record) until the steady metric is below `tol` (the
/// record's criterion is `engine::STEADY`, 1e-3; a smaller one checks that
/// the reported numbers do not depend on it) or `max_steps`; returns the
/// report, the run itself (its fields, for pictures) and the wall seconds of
/// the solve.
///
/// # Errors
/// Meshing or divergence, as text.
pub fn run_to_steady(grid: HelicalPipe, reynolds: f64, scheme: DivScheme, co: f64, tol: f64, max_steps: usize, mut on_step: impl FnMut(&CoilRun, f64)) -> Result<(CoilReport, CoilRun, f64), String> {
    let fv = Arc::new(grid.mesh().map_err(|e| e.to_string())?.to_fv_mesh().map_err(|e| e.to_string())?);
    let mut run = CoilRun::on_mesh(grid, fv, reynolds, scheme, co, crate::engine::MAX_VISCOUS_DEFAULT);
    let t0 = bugis::web_demo::platform::now_s();
    for _ in 0..max_steps {
        let c = run.step()?;
        on_step(&run, c);
        if c < tol {
            break;
        }
    }
    let wall = bugis::web_demo::platform::now_s() - t0;
    Ok((run.report(), run, wall))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_are_sized_as_the_record_says() {
        let s = CoilSpec::vv();
        assert!((s.length() - 0.9444).abs() < 1e-4, "{}", s.length());
        let [g2, g3, g4] = [2, 3, 4].map(|k| s.level(k));
        assert_eq!([g2.n_cells(), g3.n_cells(), g4.n_cells()], [15_120, 50_940, 120_960]);
        let st = CoilSpec::straight_limit();
        assert!((st.length() / 0.01 - 31.4159).abs() < 1e-3);
        assert_eq!(st.level(3).n_layers, 94);
        let b = s.for_budget(20_000);
        assert!(b.n_cells() <= 20_000, "{}", b.n_cells());
        assert!(b.n_layers as f64 * 0.01 >= s.length() - 1e-9, "layers no longer than d");
    }

    /// A coarse half-turn coil at Re = 100 runs to steady: the core flows
    /// outward, the axial maximum moves outward, and the friction factor is
    /// above the straight pipe's. (Regression of the code path, not V&V.)
    #[test]
    fn a_coarse_coil_shows_dean_flow() {
        let spec = CoilSpec { tube_radius: 0.005, coil_diameter: 0.04, pitch: 0.0, turns: 0.5 };
        let g = spec.grid(4, 2, 24);
        let (r, run, _) = run_to_steady(g, 200.0, DivScheme::GaussLinear, 4.0, crate::engine::STEADY, 2000, |_, _| {}).unwrap();
        let steps = run.steps;
        assert!(steps < 2000, "steady in {steps}");
        assert!(r.section.core_u_n < 0.0, "{r:?}");
        assert!(r.section.axial_peak_n < 0.0, "{r:?}");
        assert!(r.section.vortex_pair < 0.0, "{r:?}");
        assert!(r.f_re_64 > 1.0, "{r:?}");
        assert!(r.mishra_gupta > 1.0);
    }
}
