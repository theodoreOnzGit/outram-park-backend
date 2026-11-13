//! A laminar internal-flow case on any mesh with patches named `inlet`,
//! `outlet` and walls, solved by [`PimpleFoam`], with the diagnostics a
//! developing-pipe-flow V&V needs (gh:#1155, Foam Studio stage 1).
//!
//! Promoted from the stage 0 harness
//! (`verification_and_validation/wasm_pimple_3d_pipe/src/lib.rs`, gh:#1154):
//! the same boundary conditions, solver settings and fits, now shared by the
//! V&V test `tests/laminar_pipe_hagen_poiseuille.rs` and the browser demo
//! `dhoby-ghaut/examples/foam_studio_web`.
//!
//! # Boundary conditions
//!
//! - patch `inlet`: fixed uniform velocity `U_b` along the patch's mean
//!   inward normal, zeroGradient p;
//! - patch `outlet`: zeroGradient U, fixed kinematic p = 0;
//! - every other patch: no-slip wall (fixed U = 0), zeroGradient p.
//!
//! PISO, 1 outer and 2 correctors; Euler; upwind convection; `Gauss linear
//! corrected` Laplacians (the `PimpleFoam` defaults). The flow starts from rest.
//!
//! # Reference values
//!
//! For a circular pipe of diameter D, bulk velocity U and viscosity ν:
//! Hagen–Poiseuille gives the developed centreline velocity `2U` and
//! kinematic pressure gradient `−dp/dz = 32 ν U / D²`, i.e. the Darcy
//! friction factor `f = 64/Re`. The hydrodynamic development length (to 99 %
//! of the developed centreline velocity, uniform inlet) is taken from Durst
//! et al. (2005), see [`PipeFlow::durst_development_length`].
use petir::mathf::RealMath;
use std::sync::Arc;

use outram_foam_basic_lib::prelude::*;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::f64::{Acceleration, KinematicViscosity, Length, Time, Velocity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use crate::solvers::pimple_foam::PimpleFoam;

/// The physical set-up of a straight circular pipe along +z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeFlow {
    /// Pipe radius.
    pub radius: Length,
    /// Pipe length (inlet at z = 0 of the pipe).
    pub length: Length,
    /// Uniform inlet = bulk velocity.
    pub bulk_velocity: Velocity,
    /// Kinematic viscosity.
    pub nu: KinematicViscosity,
}

impl PipeFlow {
    /// The stage 0 case: D = 20 mm, L = 200 mm, U = 5 mm/s, ν = 1e-6 m²/s
    /// (Re = 100).
    pub fn stage0() -> Self {
        Self {
            radius: Length::new::<meter>(0.01),
            length: Length::new::<meter>(0.2),
            bulk_velocity: Velocity::new::<meter_per_second>(5.0e-3),
            nu: KinematicViscosity::new::<square_meter_per_second>(1.0e-6),
        }
    }

    fn d(&self) -> f64 {
        2.0 * self.radius.get::<meter>()
    }

    fn u(&self) -> f64 {
        self.bulk_velocity.get::<meter_per_second>()
    }

    fn nu_si(&self) -> f64 {
        self.nu.get::<square_meter_per_second>()
    }

    /// Reynolds number `U D / ν`.
    pub fn reynolds(&self) -> f64 {
        self.u() * self.d() / self.nu_si()
    }

    /// Hagen–Poiseuille developed kinematic pressure gradient, `−dp/dz =
    /// 32 ν U / D²` (positive).
    pub fn hagen_poiseuille_gradient(&self) -> Acceleration {
        Acceleration::new::<meter_per_second_squared>(32.0 * self.nu_si() * self.u() / (self.d() * self.d()))
    }

    /// Hagen–Poiseuille kinematic wall shear stress `τ_w/ρ = 4 ν U / R`
    /// \[m²/s²\] (the force balance `τ_w = (−dp/dz) R / 2`).
    pub fn hagen_poiseuille_wall_shear(&self) -> f64 {
        4.0 * self.nu_si() * self.u() / self.radius.get::<meter>()
    }

    /// Darcy friction factor from a kinematic pressure gradient:
    /// `f = (−dp/dz) D / (½ U²)`; Hagen–Poiseuille gives `64/Re`.
    pub fn friction_factor(&self, gradient: Acceleration) -> f64 {
        gradient.get::<meter_per_second_squared>() * self.d() / (0.5 * self.u() * self.u())
    }

    /// Hydrodynamic development length to 99 % of the developed centreline
    /// velocity for a uniform inlet, Durst, Ray, Ünsal & Bayoumi (2005), "The
    /// development lengths of laminar pipe and channel flows", J. Fluids Eng.
    /// 127, 1154–1160, doi:10.1115/1.2063088, eq. (8):
    /// `L/D = [0.619^1.6 + (0.0567 Re)^1.6]^(1/1.6)`.
    /// At Re = 100 this is 5.772 D.
    pub fn durst_development_length(&self) -> Length {
        let re = self.reynolds();
        let ld = (0.619_f64.r_powf(1.6) + (0.0567 * re).r_powf(1.6)).r_powf(1.0 / 1.6);
        Length::new::<meter>(ld * self.d())
    }

    /// Time step for an axial Courant number `co` on the developed centreline
    /// (`Δt = co Δz / 2U`).
    pub fn courant_time_step(&self, dz: Length, co: f64) -> Time {
        Time::new::<second>(co * dz.get::<meter>() / (2.0 * self.u()))
    }

    /// A [`PimpleFoam`] on `mesh` with this case's boundary conditions, at a
    /// fixed time step `dt`, starting from rest.
    pub fn build_solver(&self, mesh: Arc<FvMesh>, dt: Time) -> PimpleFoam {
        let control = ControlDict {
            start: StartControl::StartTime(0.0),
            stop: StopControl::EndTime(1.0e9),
            delta_t: dt.get::<second>(),
            ..ControlDict::default()
        };
        let mut solution = FvSolution::default();
        solution.pimple.n_outer_correctors = 1;
        solution.pimple.n_correctors = 2;
        let mut s = PimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);
        let n = mesh.n_cells;
        let mut ub = Vec::new();
        let mut pb = Vec::new();
        for p in &mesh.patches {
            match p.name.as_str() {
                "inlet" => {
                    let mut sf = Vector3::ZERO;
                    for fi in 0..p.size {
                        sf = sf + mesh.face_area_vectors[p.start + fi];
                    }
                    let inward = if sf.mag() > 0.0 { sf * (-1.0 / sf.mag()) } else { Vector3::new(0.0, 0.0, 1.0) };
                    ub.push(PatchField::fixed_value_vec(p.size, inward * self.u()));
                    pb.push(PatchField::zero_gradient(p.size));
                }
                "outlet" => {
                    ub.push(PatchField::zero_gradient_vec(p.size));
                    pb.push(PatchField::fixed_value(p.size, 0.0));
                }
                _ => {
                    ub.push(PatchField::fixed_value_vec(p.size, Vector3::ZERO));
                    pb.push(PatchField::zero_gradient(p.size));
                }
            }
        }
        s.u = VolVectorField::new("U", mesh.clone(), Field::new(vec![Vector3::ZERO; n]), ub);
        s.p = VolScalarField::new("p", mesh.clone(), Field::new(vec![0.0; n]), pb);
        s.nu = VolScalarField::uniform("nu", mesh, self.nu_si());
        s
    }

    /// One time step; returns the steady-state metric used by stage 0: the
    /// largest cell velocity change relative to U, per flow-through time L/U
    /// (dimensionless). Below 1e-3 the run is called steady.
    ///
    /// # Errors
    /// Whatever [`PimpleFoam::step`] returns.
    pub fn step(&self, s: &mut PimpleFoam) -> Result<f64, AppBuilderError> {
        let before = s.u.internal.as_slice().to_vec();
        s.step()?;
        // A NaN cell must not read as "no change": `f64::max` drops NaN, so a
        // diverged field used to report 0, i.e. steady (found on the
        // unstructured pipe, gh:#1202). Any non-finite change is returned as NaN.
        let du = before.iter().zip(s.u.internal.as_slice()).fold(0.0_f64, |a, (b, c)| {
            let d = (*c - *b).mag();
            if a.is_nan() || !d.is_finite() {
                f64::NAN
            } else {
                a.max(d)
            }
        });
        let u = self.u();
        Ok(du / u * (self.length.get::<meter>() / u) / s.control.delta_t)
    }
}

/// Volume-weighted least-squares fit `y ≈ a + b x`; returns `(a, b)`.
pub fn fit_line(samples: &[(f64, f64, f64)]) -> (f64, f64) {
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(x, y, w) in samples {
        sw += w;
        sx += w * x;
        sy += w * y;
        sxx += w * x * x;
        sxy += w * x * y;
    }
    let det = sw * sxx - sx * sx;
    if det.abs() < 1e-300 {
        return (if sw > 0.0 { sy / sw } else { 0.0 }, 0.0);
    }
    let b = (sw * sxy - sx * sy) / det;
    ((sy - b * sx) / sw, b)
}

/// Centreline axial velocity in the slab `|z − z0| < hz`: the intercept of a
/// `u_z = a + b r²` fit over the cells with r < R/2 (exact for a parabola).
/// The pipe axis is the z axis through `(x0, y0)`.
pub fn centreline_velocity(s: &PimpleFoam, flow: &PipeFlow, axis: (f64, f64), z0: f64, hz: f64) -> Velocity {
    let r = flow.radius.get::<meter>();
    let m = &s.mesh;
    let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
        .filter_map(|c| {
            let x = m.cell_centres[c];
            let r2 = (x.x - axis.0).r_powi(2) + (x.y - axis.1).r_powi(2);
            ((x.z - z0).abs() < hz && r2 < 0.25 * r * r).then(|| (r2, s.u.internal[c].z, m.cell_volumes[c]))
        })
        .collect();
    Velocity::new::<meter_per_second>(fit_line(&pts).0)
}

/// The distinct cell-centre z values (to 1e-9 m), sorted: the axial layers of
/// a structured mesh.
pub fn axial_layers(mesh: &FvMesh) -> Vec<f64> {
    let mut zs: Vec<f64> = mesh.cell_centres.iter().map(|c| (c.z * 1e9).round() / 1e9).collect();
    zs.sort_by(f64::total_cmp);
    zs.dedup();
    zs
}

/// Centreline velocity of every axial layer, `(z [m], u_c [m/s])`.
pub fn centreline_profile(s: &PimpleFoam, flow: &PipeFlow, axis: (f64, f64)) -> Vec<(f64, f64)> {
    axial_layers(&s.mesh)
        .into_iter()
        .map(|z| (z, centreline_velocity(s, flow, axis, z, 1e-7).get::<meter_per_second>()))
        .collect()
}

/// Development length from a centreline profile: the first z (linearly
/// interpolated between layers, measured from `z_inlet`) at which `u_c`
/// reaches `frac · u_dev`. `None` if it never does.
pub fn development_length(profile: &[(f64, f64)], u_dev: f64, frac: f64, z_inlet: f64) -> Option<Length> {
    let target = frac * u_dev;
    let mut prev: Option<(f64, f64)> = None;
    for &(z, u) in profile {
        if u >= target {
            let zz = match prev {
                Some((z0, u0)) if u > u0 => z0 + (target - u0) / (u - u0) * (z - z0),
                _ => z,
            };
            return Some(Length::new::<meter>(zz - z_inlet));
        }
        prev = Some((z, u));
    }
    None
}

/// `−dp/dz` from a volume-weighted p-against-z fit over every cell with
/// `z ∈ [z0, z1]` (positive for flow toward +z).
pub fn pressure_gradient(s: &PimpleFoam, z0: f64, z1: f64) -> Acceleration {
    let m = &s.mesh;
    let pts: Vec<(f64, f64, f64)> = (0..m.n_cells)
        .filter_map(|c| {
            let z = m.cell_centres[c].z;
            (z >= z0 && z <= z1).then(|| (z, s.p.internal[c], m.cell_volumes[c]))
        })
        .collect();
    Acceleration::new::<meter_per_second_squared>(-fit_line(&pts).1)
}

/// Volume-weighted RMS of `(u_z − 2U(1 − r²/R²)) / 2U` over the cells whose
/// centres lie in `z ∈ [z0, z1]`: the developed-profile error of the gh:#1155
/// record, as a function so that any mesh (O-grid or polyhedral, gh:#1202)
/// is measured the same way. The axis is the z axis through `axis`.
pub fn profile_rms(s: &PimpleFoam, flow: &PipeFlow, axis: (f64, f64), z0: f64, z1: f64) -> f64 {
    let (r, u2) = (flow.radius.get::<meter>(), 2.0 * flow.u());
    let m = &s.mesh;
    let (mut sw, mut se) = (0.0, 0.0);
    for c in 0..m.n_cells {
        let x = m.cell_centres[c];
        if x.z < z0 || x.z > z1 {
            continue;
        }
        let rr = ((x.x - axis.0).r_powi(2) + (x.y - axis.1).r_powi(2)) / (r * r);
        let e = (s.u.internal[c].z - u2 * (1.0 - rr)) / u2;
        sw += m.cell_volumes[c];
        se += m.cell_volumes[c] * e * e;
    }
    if sw > 0.0 {
        (se / sw).sqrt()
    } else {
        f64::NAN
    }
}

/// The radial profile of `u_z` by **binning cell centres in r** (gh:#1202):
/// the cells whose centres lie within `|z − z_mid| < hz` are put in `n_bins`
/// equal bins of r/R ∈ [0, 1], and each bin gives its volume-weighted mean
/// r/R and mean `u_z / U`. Empty bins are left out. This works on any mesh;
/// on an unstructured one a single plane cuts cells whose centres sit off
/// the plane, so a slab is used instead of interpolating to the plane.
/// Returns `(r/R, u_z/U, cells in the bin)`.
pub fn radial_profile_binned(s: &PimpleFoam, flow: &PipeFlow, axis: (f64, f64), z_mid: f64, hz: f64, n_bins: usize) -> Vec<(f64, f64, usize)> {
    let (r, u) = (flow.radius.get::<meter>(), flow.u());
    let n_bins = n_bins.max(1);
    let mut acc = vec![(0.0, 0.0, 0.0, 0usize); n_bins];
    let m = &s.mesh;
    for c in 0..m.n_cells {
        let x = m.cell_centres[c];
        if (x.z - z_mid).abs() >= hz {
            continue;
        }
        let rr = ((x.x - axis.0).r_powi(2) + (x.y - axis.1).r_powi(2)).sqrt() / r;
        let k = ((rr * n_bins as f64) as usize).min(n_bins - 1);
        let w = m.cell_volumes[c];
        acc[k].0 += w;
        acc[k].1 += w * rr;
        acc[k].2 += w * s.u.internal[c].z / u;
        acc[k].3 += 1;
    }
    acc.into_iter().filter(|a| a.3 > 0).map(|a| (a.1 / a.0, a.2 / a.0, a.3)).collect()
}

/// Mean kinematic wall shear stress `τ_w/ρ` \[m²/s²\] on the faces of every
/// patch other than `inlet` and `outlet` whose centres lie in `z ∈ [z0, z1]`,
/// area-weighted: per face `ν |U_P,t| / d`, the tangential velocity of the
/// wall cell over the distance from its centre to the face along the face
/// normal. This is OpenFOAM's `wallShearStress` with the wall's own
/// `snGrad` (no non-orthogonal correction, first order in d), as its
/// `wallShearStress` function object computes it for a laminar no-slip wall.
/// Hagen–Poiseuille gives `4 ν U / R`; [`PipeFlow::hagen_poiseuille_wall_shear`].
pub fn wall_shear_stress(s: &PimpleFoam, z0: f64, z1: f64) -> f64 {
    let m = &s.mesh;
    let nu = s.nu.internal[0];
    let (mut sa, mut st) = (0.0, 0.0);
    for p in &m.patches {
        if p.name == "inlet" || p.name == "outlet" {
            continue;
        }
        for fi in 0..p.size {
            let f = p.start + fi;
            let fc = m.face_centres[f];
            if fc.z < z0 || fc.z > z1 {
                continue;
            }
            let a = m.face_areas[f];
            let n = m.face_area_vectors[f] * (1.0 / a.max(1e-300));
            let c = m.owner[f];
            let d = (fc - m.cell_centres[c]).dot(n).abs().max(1e-300);
            let up = s.u.internal[c];
            let ut = up - n * up.dot(n);
            sa += a;
            st += a * nu * ut.mag() / d;
        }
    }
    if sa > 0.0 {
        st / sa
    } else {
        f64::NAN
    }
}

/// Net face flux through the named patch as stored in `phi` \[m³/s\],
/// outward positive; 0 if there is no such patch.
pub fn patch_flux(s: &PimpleFoam, name: &str) -> f64 {
    s.mesh
        .patches
        .iter()
        .position(|p| p.name == name)
        .map(|i| s.phi.boundary[i].values.as_slice().iter().sum())
        .unwrap_or(0.0)
}

/// Worst cell continuity error `max |Σ_f φ_f|` over the inlet flow rate.
pub fn max_continuity_error(s: &PimpleFoam) -> f64 {
    let m = &s.mesh;
    let mut div = vec![0.0; m.n_cells];
    for f in 0..m.n_internal_faces {
        div[m.owner[f]] += s.phi.internal[f];
        div[m.neighbour[f]] -= s.phi.internal[f];
    }
    for (pi, p) in m.patches.iter().enumerate() {
        for fi in 0..p.size {
            div[m.owner[p.start + fi]] += s.phi.boundary[pi].values[fi];
        }
    }
    let q = -patch_flux(s, "inlet");
    div.iter().fold(0.0_f64, |a, d| a.max(d.abs())) / q.abs().max(1e-300)
}

/// Physical time after which `xs` (time, value) stays within `tol`
/// (relative) of its last value; 0 if it always did.
pub fn settling_time(xs: &[(f64, f64)], tol: f64) -> f64 {
    let last = xs.last().map_or(0.0, |x| x.1);
    xs.iter().rev().find(|x| (x.1 - last).abs() > tol * last.abs()).map_or(0.0, |x| x.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_values_of_the_stage0_case() {
        let f = PipeFlow::stage0();
        assert!((f.reynolds() - 100.0).abs() < 1e-9);
        let g = f.hagen_poiseuille_gradient();
        assert!((g.get::<meter_per_second_squared>() - 4.0e-4).abs() < 1e-15);
        assert!((f.friction_factor(g) - 0.64).abs() < 1e-12);
        let ld = f.durst_development_length().get::<meter>() / 0.02;
        assert!((ld - 5.771_884).abs() < 1e-5, "L/D {ld}");
        let dt = f.courant_time_step(Length::new::<meter>(0.0025), 4.0);
        assert!((dt.get::<second>() - 1.0).abs() < 1e-12);
    }

    /// The profile instruments on an exact Hagen–Poiseuille field set at the
    /// cell centres of a coarse O-grid (gh:#1202): the RMS is round-off and
    /// every bin sits on 2(1 − r²) to within its width. The wall shear is
    /// checked on a plane channel instead (below): on the O-grid the parabola
    /// vanishes on the circle, not on the polygon's flat faces, so the
    /// one-sided estimate there reads high (1.28 on this 16-gon, first run
    /// 2026-11-04) for a reason that is the test field's, not the
    /// instrument's.
    #[test]
    fn profile_instruments_on_an_exact_parabola() {
        use outram_foam_mesh::block_primitives::PipeOGrid;
        let f = PipeFlow { length: Length::new::<meter>(0.05), ..PipeFlow::stage0() };
        let g = PipeOGrid::new(f.radius, f.length, 4, 3, 10);
        let mesh = Arc::new(g.mesh().unwrap().to_fv_mesh().unwrap());
        let mut s = f.build_solver(mesh.clone(), Time::new::<second>(0.1));
        let (r, u2) = (0.01, 2.0 * 5.0e-3);
        for c in 0..mesh.n_cells {
            let x = mesh.cell_centres[c];
            s.u.internal[c] = Vector3::new(0.0, 0.0, u2 * (1.0 - (x.x * x.x + x.y * x.y) / (r * r)));
        }
        assert!(profile_rms(&s, &f, (0.0, 0.0), 0.0, 0.05) < 1e-12);
        let bins = radial_profile_binned(&s, &f, (0.0, 0.0), 0.025, 0.01, 5);
        assert!(bins.len() >= 4, "{bins:?}");
        for (rr, u, n) in &bins {
            assert!(*n > 0 && (u - 2.0 * (1.0 - rr * rr)).abs() < 0.15, "{rr} {u}");
        }
        assert!((f.hagen_poiseuille_wall_shear() - 2.0e-6).abs() < 1e-18);
    }

    /// `wall_shear_stress` on plane Poiseuille flow `u_z = 4U y (H − y) / H²`
    /// between walls y = 0 and y = H of a box (the x and z sides named
    /// `inlet` / `outlet`, so only the two walls count): the one-sided
    /// `ν u_P / d` with d = Δy/2 is exactly `4νU/H · (1 − d/H)`.
    #[test]
    fn wall_shear_is_the_one_sided_gradient_on_a_channel() {
        use outram_foam_mesh::block_primitives::BoxBlock;
        let (h, ny) = (0.01, 10);
        let mut b = BoxBlock::new([0.01, h, 0.02].map(Length::new::<meter>), [2, ny, 3]);
        b.patch_names = ["inlet", "outlet", "walls", "walls", "inlet", "outlet"].map(String::from);
        let mesh = Arc::new(b.mesh().unwrap().to_fv_mesh().unwrap());
        let f = PipeFlow::stage0();
        let mut s = f.build_solver(mesh.clone(), Time::new::<second>(0.1));
        let u = 5.0e-3;
        for c in 0..mesh.n_cells {
            let y = mesh.cell_centres[c].y;
            s.u.internal[c] = Vector3::new(0.0, 0.0, 4.0 * u * y * (h - y) / (h * h));
        }
        let d = 0.5 * h / ny as f64;
        let want = 4.0 * 1.0e-6 * u / h * (1.0 - d / h);
        let got = wall_shear_stress(&s, -1.0, 1.0);
        assert!((got / want - 1.0).abs() < 1e-12, "{got} vs {want}");
    }

    /// A diverged (NaN) velocity reads as a NaN change, never as steady.
    #[test]
    fn a_nan_field_is_not_steady() {
        use outram_foam_mesh::block_primitives::PipeOGrid;
        let f = PipeFlow { length: Length::new::<meter>(0.05), ..PipeFlow::stage0() };
        let mesh = Arc::new(PipeOGrid::new(f.radius, f.length, 2, 1, 4).mesh().unwrap().to_fv_mesh().unwrap());
        let mut s = f.build_solver(mesh, Time::new::<second>(0.1));
        s.u.internal[0] = Vector3::new(f64::NAN, 0.0, 0.0);
        assert!(!matches!(f.step(&mut s), Ok(v) if v.is_finite()));
    }

    #[test]
    fn fits_and_lengths() {
        let (a, b) = fit_line(&[(0.0, 1.0, 1.0), (1.0, 3.0, 2.0), (2.0, 5.0, 1.0)]);
        assert!((a - 1.0).abs() < 1e-12 && (b - 2.0).abs() < 1e-12);
        assert_eq!(fit_line(&[(1.0, 4.0, 1.0)]), (4.0, 0.0));
        let prof = [(0.0, 0.5), (1.0, 0.9), (2.0, 1.0)];
        let l = development_length(&prof, 1.0, 0.95, 0.0).unwrap().get::<meter>();
        assert!((l - 1.5).abs() < 1e-12);
        assert!(development_length(&prof, 2.0, 0.99, 0.0).is_none());
        assert_eq!(settling_time(&[(1.0, 2.0), (2.0, 1.0), (3.0, 1.0)], 0.01), 1.0);
    }
}
