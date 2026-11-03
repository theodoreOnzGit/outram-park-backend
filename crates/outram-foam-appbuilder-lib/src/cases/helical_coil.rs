//! Laminar flow in a helically coiled pipe, solved by [`PimpleFoam`] with
//! [`laminar_pipe`](super::laminar_pipe)'s boundary conditions, and the
//! diagnostics a Dean-flow V&V needs (gh:#1157, the CFD coil section).
//!
//! # Set-up
//!
//! The mesh is `outram_foam_mesh::block_primitives::HelicalPipe` (a pipe
//! O-grid swept along the helix); its patches are `inlet`, `outlet` and the
//! wall, so [`PipeFlow::build_solver`] applies unchanged: a uniform inlet
//! along the inlet's inward normal, p = 0 at the outlet, no-slip walls, from
//! rest. Nothing here is coil-specific except where the diagnostics look.
//!
//! **Developing, not periodic.** A fully developed coil flow is periodic in
//! the arc length, but `PimpleFoam` has no mean-pressure-gradient source
//! (OpenFOAM's `meanVelocityForce`), and one turn of the coil maps onto the
//! next by a translation only on the Frenet frame, which shears the cells
//! (see `HelicalPipe`'s docs). So the case is a few turns with a uniform
//! inlet, and the developed quantities are measured over a turn after the
//! flow has developed (the V&V record measures where that is).
//!
//! # Coordinates
//!
//! This crate does not depend on the mesher, so the caller supplies each
//! cell's coil coordinates ([`CoilCells`]): arc position `s`, the offsets
//! `n` (towards the coil axis, the inner side of the bend) and `b` of its
//! centre from the centreline, and the Frenet frame `T, N, B` there. With
//! `HelicalPipe::locate` that is one call per cell.
//!
//! # Reference values
//!
//! The Dean number `De = Re √(d/D_c)`, `D_c` the curvature diameter (twice
//! the radius of curvature of the centreline). The friction factor is the
//! Darcy `f = (−dp/ds) d / (½ U²)` of [`PipeFlow::friction_factor`]; its
//! straight-pipe value is `64/Re`. The coil correlations (Mishra & Gupta
//! 1979 for laminar flow) are in
//! `tampines::components::helical_coil_otsg::correlations` and are called by
//! the V&V runner, not here (no dependency on `tampines` from this crate).
use outram_foam_basic_lib::prelude::*;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::f64::{Acceleration, Length};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

use super::laminar_pipe::{fit_line, PipeFlow};
use crate::solvers::pimple_foam::PimpleFoam;

/// A laminar coil flow: the pipe flow (radius, developed length, bulk
/// velocity, ν) and the coil's curvature diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoilFlow {
    /// The tube flow; `pipe.length` is the developed (arc) length.
    pub pipe: PipeFlow,
    /// Curvature diameter `D_c` of the centreline.
    pub curvature_diameter: Length,
}

impl CoilFlow {
    /// Reynolds number `U d / ν`.
    pub fn reynolds(&self) -> f64 {
        self.pipe.reynolds()
    }

    /// Curvature ratio `d / D_c`.
    pub fn curvature_ratio(&self) -> f64 {
        2.0 * self.pipe.radius.get::<meter>() / self.curvature_diameter.get::<meter>()
    }

    /// Dean number `Re √(d / D_c)`.
    pub fn dean_number(&self) -> f64 {
        self.reynolds() * self.curvature_ratio().sqrt()
    }
}

/// Per-cell coil coordinates, in the mesh's cell order (see the module docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoilCells {
    /// Arc position of each cell centre \[m\].
    pub s: Vec<f64>,
    /// Offset along N (towards the coil axis) \[m\].
    pub n: Vec<f64>,
    /// Offset along B \[m\].
    pub b: Vec<f64>,
    /// Frenet frame `[T, N, B]` at `s`.
    pub frenet: Vec<[[f64; 3]; 3]>,
}

/// A velocity in a cell's Frenet frame: `[u_t, u_n, u_b]`, axial, towards
/// the coil axis, and binormal.
pub fn frenet_components(u: Vector3, frenet: &[[f64; 3]; 3]) -> [f64; 3] {
    frenet.map(|e| u.x * e[0] + u.y * e[1] + u.z * e[2])
}

/// `−dp/ds` from a volume-weighted p-against-s fit over every cell with
/// `s ∈ [s0, s1]` (positive for flow towards +s). In a developed coil flow p
/// also varies across the section (it balances the centrifugal force); the
/// fit over whole sections averages that out.
pub fn arc_pressure_gradient(sol: &PimpleFoam, cells: &CoilCells, s0: Length, s1: Length) -> Acceleration {
    let (a, b) = (s0.get::<meter>(), s1.get::<meter>());
    let m = &sol.mesh;
    let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
        .filter_map(|c| {
            let s = cells.s[c];
            (s >= a && s <= b).then(|| (s, sol.p.internal[c], m.cell_volumes[c]))
        })
        .collect();
    Acceleration::new::<meter_per_second_squared>(-fit_line(&pts).1)
}

/// What a cross-section shows of the Dean flow, over the cells with
/// `|s − s0| < half_width`; velocities over the bulk velocity U, offsets over
/// the tube radius R.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeanSection {
    /// Cells used.
    pub cells: usize,
    /// `n / R` of the cell with the largest axial velocity: negative is
    /// towards the outer wall (away from the coil axis), where Dean flow
    /// pushes the axial maximum; 0 in a straight pipe.
    pub axial_peak_n: f64,
    /// That largest axial velocity, `u_t / U` (2 for Hagen–Poiseuille).
    pub axial_peak: f64,
    /// Largest in-plane (secondary) speed `√(u_n² + u_b²) / U`.
    pub max_secondary: f64,
    /// Volume-mean `u_n / U` over the central cells (`r < R/3`): negative is
    /// the core flowing outward, the Dean vortices' signature.
    pub core_u_n: f64,
    /// Volume-mean `u_b / U` over the cells with `n > 0` and `b > 0`, minus
    /// that over `n > 0`, `b < 0`. In the classic Dean pair the core flows
    /// outward, the flow returns inward along the upper and lower walls, and
    /// the two return flows meet at the inner wall and turn towards `b = 0`:
    /// `u_b < 0` above the midplane and `> 0` below it on the inner half, so
    /// this is **negative** for that pair (a sign test, not a magnitude).
    pub vortex_pair: f64,
}

/// The [`DeanSection`] of the cells within `half_width` of arc position `s0`
/// (`radius` R and bulk velocity of `flow`).
pub fn dean_section(sol: &PimpleFoam, cells: &CoilCells, flow: &PipeFlow, s0: Length, half_width: Length) -> DeanSection {
    let (s0, hw) = (s0.get::<meter>(), half_width.get::<meter>());
    let r = flow.radius.get::<meter>();
    let u = flow.bulk_velocity.get::<meter_per_second>();
    let m = &sol.mesh;
    let mut out = DeanSection { cells: 0, axial_peak_n: 0.0, axial_peak: f64::NEG_INFINITY, max_secondary: 0.0, core_u_n: 0.0, vortex_pair: 0.0 };
    let (mut core, mut wc) = (0.0, 0.0);
    let (mut up, mut wu, mut dn, mut wd) = (0.0, 0.0, 0.0, 0.0);
    for c in 0..m.n_cells {
        if (cells.s[c] - s0).abs() >= hw {
            continue;
        }
        out.cells += 1;
        let [ut, un, ub] = frenet_components(sol.u.internal[c], &cells.frenet[c]);
        let w = m.cell_volumes[c];
        if ut > out.axial_peak {
            out.axial_peak = ut;
            out.axial_peak_n = cells.n[c] / r;
        }
        out.max_secondary = out.max_secondary.max((un * un + ub * ub).sqrt());
        let (n, b) = (cells.n[c], cells.b[c]);
        if n * n + b * b < r * r / 9.0 {
            core += un * w;
            wc += w;
        }
        if n > 0.0 && b > 0.0 {
            up += ub * w;
            wu += w;
        } else if n > 0.0 && b < 0.0 {
            dn += ub * w;
            wd += w;
        }
    }
    if out.cells == 0 {
        out.axial_peak = 0.0;
        return out;
    }
    out.axial_peak /= u;
    out.max_secondary /= u;
    out.core_u_n = core / wc.max(1e-300) / u;
    out.vortex_pair = (up / wu.max(1e-300) - dn / wd.max(1e-300)) / u;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uom::si::f64::{KinematicViscosity, Time, Velocity};
    use uom::si::kinematic_viscosity::square_meter_per_second;
    use uom::si::time::second;

    #[test]
    fn dean_number_and_frenet_components() {
        let f = CoilFlow {
            pipe: PipeFlow {
                radius: Length::new::<meter>(0.005),
                length: Length::new::<meter>(1.0),
                bulk_velocity: Velocity::new::<meter_per_second>(0.01),
                nu: KinematicViscosity::new::<square_meter_per_second>(1e-6),
            },
            curvature_diameter: Length::new::<meter>(0.1),
        };
        assert!((f.reynolds() - 100.0).abs() < 1e-9);
        assert!((f.curvature_ratio() - 0.1).abs() < 1e-15);
        assert!((f.dean_number() - 100.0 * 0.1_f64.sqrt()).abs() < 1e-9);
        let fr = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(frenet_components(Vector3::new(1.0, 2.0, 3.0), &fr), [2.0, -1.0, 3.0]);
    }

    /// On a 2×2×4 box with a hand-set velocity and pressure, the fits and
    /// section statistics come out as constructed.
    #[test]
    fn gradient_and_section_statistics_on_a_hand_set_field() {
        // 2 × 2 × 4 cells of 1 cm along z (s = z), patches inlet / outlet / walls.
        let m = Length::new::<meter>;
        let bx = outram_foam_mesh::block_primitives::BoxBlock::new([m(0.02), m(0.02), m(0.04)], [2, 2, 4]);
        let mesh = Arc::new(bx.mesh().unwrap().to_fv_mesh().unwrap());
        let flow = PipeFlow {
            radius: Length::new::<meter>(0.01),
            length: Length::new::<meter>(0.04),
            bulk_velocity: Velocity::new::<meter_per_second>(1.0),
            nu: KinematicViscosity::new::<square_meter_per_second>(1e-3),
        };
        let mut sol = flow.build_solver(mesh.clone(), Time::new::<second>(1.0));
        let mut cells = CoilCells::default();
        // Frenet: T = +z, N = −x (the inner side is −x), B = T × N = −y.
        let fr = [[0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]];
        for c in 0..mesh.n_cells {
            let x = mesh.cell_centres[c];
            cells.s.push(x.z);
            cells.n.push(-(x.x - 0.01));
            cells.b.push(-(x.y - 0.01));
            cells.frenet.push(fr);
            sol.p.internal[c] = 5.0 - 2.0 * x.z;
            // Axial peak on the outer side (x > 0.01 ⇒ n < 0); core flow outward (+x).
            let outer = x.x > 0.01;
            sol.u.internal[c] = Vector3::new(0.1, if x.y > 0.01 { 0.05 } else { -0.05 }, if outer { 2.0 } else { 1.0 });
        }
        let g = arc_pressure_gradient(&sol, &cells, Length::new::<meter>(0.0), Length::new::<meter>(0.04));
        assert!((g.get::<meter_per_second_squared>() - 2.0).abs() < 1e-12);
        let d = dean_section(&sol, &cells, &flow, Length::new::<meter>(0.015), Length::new::<meter>(0.0051));
        assert_eq!(d.cells, 4);
        assert!(d.axial_peak_n < 0.0 && (d.axial_peak - 2.0).abs() < 1e-12, "{d:?}");
        assert!((d.max_secondary - (0.1_f64 * 0.1 + 0.05 * 0.05).sqrt()).abs() < 1e-12);
        // Cell centres are 0.005 from the axis, beyond R/3: no core cells.
        assert_eq!(d.core_u_n, 0.0);
        // n > 0 cells are at x < 0.01; there b > 0 ⇔ y < 0.01 where u_b = −u_y = +0.05.
        assert!((d.vortex_pair - 0.1).abs() < 1e-12, "{d:?}");
        let empty = dean_section(&sol, &cells, &flow, Length::new::<meter>(1.0), Length::new::<meter>(0.001));
        assert_eq!((empty.cells, empty.axial_peak), (0, 0.0));
    }
}
