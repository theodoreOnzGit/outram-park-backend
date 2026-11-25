//! # The GeN-Foam multiphysics backend (gh:#1746)
//!
//! [`MultiphysicsBackend::GenFoam`](super::MultiphysicsBackend::GenFoam) for
//! HTR-10: the workspace's GeN-Foam port solving multigroup diffusion on an
//! R-Z mesh of the core, with constants from the Monte Carlo page's level
//! (the four isothermal whole-core MGXS records,
//! [`XsSource::MonteCarlo`](super::XsSource::MonteCarlo)). **Nothing is
//! re-implemented:** this wraps `nee_soon::mgxs_genfoam` exactly as
//! `nee_soon/examples/htr10_mgxs_genfoam_rz.rs` (gh:#1187) runs it:
//!
//! 1. the state points: `MgxsStatePoints::htr10_records()`;
//! 2. the mesh: `Htr10RzNeutronicsMesh::build` over the record model, each
//!    cell's region read from the assembled Monte Carlo geometry;
//!    **coarse** is the record's 20 cm cells, **fine** 10 cm;
//! 3. the constants: `Htr10MgxsRegions::new(&mesh).nuclear_data(&points)`
//!    (4 groups, rebalanced, descending, SI, stacked in `TFuel`, log law);
//! 4. `solve_diffusion` with every cell at 600 K, then at 1200 K.
//!
//! **What a run reports** ([`GenFoamResult`]): `k` at both temperatures
//! beside the Monte Carlo `k` of the same records, the isothermal
//! temperature coefficient `(1/k₆₀₀ − 1/k₁₂₀₀)/600 K` (gh:#1174's secant),
//! and the volume-weighted power peaking at 600 K.
//!
//! **What remains simplified, stated where the result is shown:** this is
//! deterministic neutronics only. Every cell sits at one `TFuel`, with no
//! thermal-hydraulic feedback in the run; the coupled chains are
//! `nee_soon::htr10_coupling` (Picard against the R-Z core) and
//! `htgr_sim_v1`'s coarse-mesh tier, still a placeholder. The constants are
//! the lattice records' (N = 12), not the poured bed's. The result is not
//! sent to the plant: its coefficient is shown beside the plant's set for
//! comparison.
//!
//! Native only (it reads the records and builds the Monte Carlo geometry).
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1746).

use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use nee_soon::htr10_rmc::parametric::RECORD_RINGS;
use nee_soon::mgxs_genfoam::{Htr10MgxsRegions, Htr10RzNeutronicsMesh, MgxsStatePoints};
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::{centimeter, meter};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use super::{Availability, MeshResolution, MultiphysicsBackend, MultiphysicsCase, TwinError, XsSource};

/// The two temperatures every cell is solved at, K (gh:#1174's secant).
pub const STATE_TEMPERATURES_K: [f64; 2] = [600.0, 1200.0];

/// One GeN-Foam run's result.
#[derive(Clone, Debug, PartialEq)]
pub struct GenFoamResult {
    pub resolution: MeshResolution,
    pub cells: usize,
    /// Diffusion `k` at [`STATE_TEMPERATURES_K`].
    pub k: [f64; 2],
    /// The Monte Carlo records' `k` at the same temperatures, with σ.
    pub k_mc: [(f64, f64); 2],
    /// `(1/k₆₀₀ − 1/k₁₂₀₀)/600 K`, 1/K.
    pub isothermal_per_k: f64,
    /// Peak cell power density over the volume-weighted mean, at 600 K.
    pub peaking: f64,
    /// Whether both outer iterations converged.
    pub converged: bool,
}

/// The volume-weighted power peaking of an R-Z field: cell `i + nr j` is
/// the annulus between radial edges `i, i+1` and axial edges `j, j+1`.
/// Cells with no power are left out of the mean. `None` with no power.
pub fn rz_peaking(power: &[f64], r_edges: &[f64], z_edges: &[f64]) -> Option<f64> {
    let nr = r_edges.len().checked_sub(1)?;
    let (mut pv, mut v, mut peak) = (0.0, 0.0, 0.0_f64);
    for (c, &p) in power.iter().enumerate() {
        if p <= 0.0 || nr == 0 {
            continue;
        }
        let (i, j) = (c % nr, c / nr);
        let vol = core::f64::consts::PI * (r_edges[i + 1] * r_edges[i + 1] - r_edges[i] * r_edges[i]) * (z_edges.get(j + 1)? - z_edges.get(j)?).abs();
        pv += p * vol;
        v += vol;
        peak = peak.max(p);
    }
    (v > 0.0).then(|| peak / (pv / v))
}

/// HTR-10 on GeN-Foam, coarse (20 cm) and fine (10 cm).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Htr10GenFoam;

impl Htr10GenFoam {
    /// The mesh cell size for `r`.
    pub fn cell_size(r: MeshResolution) -> Length {
        match r {
            MeshResolution::Coarse => Length::new::<centimeter>(20.0),
            MeshResolution::Fine => Length::new::<centimeter>(10.0),
        }
    }
}

impl MultiphysicsCase for Htr10GenFoam {
    type Result = GenFoamResult;

    fn availability(&self, _r: MeshResolution) -> Availability {
        Availability::Built(MultiphysicsBackend::GenFoam)
    }

    fn cross_sections(&self) -> XsSource {
        XsSource::MonteCarlo
    }

    fn run(&mut self, resolution: MeshResolution) -> Result<GenFoamResult, TwinError> {
        let err = |e: String| TwinError::Multiphysics(e);
        let points = MgxsStatePoints::htr10_records().map_err(|e| err(e.to_string()))?;
        let core = assemble_explicit_triso(RECORD_RINGS, points.layers(), 0);
        let present: Vec<bool> = (0..points.points[0].library.zones.len()).map(|s| points.slot_has_flux(s)).collect();
        let mesh = Htr10RzNeutronicsMesh::build(&core, &present, Self::cell_size(resolution)).map_err(|e| err(e.to_string()))?;
        let nd = Htr10MgxsRegions::new(&mesh).nuclear_data(&points).map_err(|e| err(e.to_string()))?;
        let mut k = [0.0; 2];
        let mut k_mc = [(f64::NAN, f64::NAN); 2];
        let mut converged = true;
        let mut peaking = f64::NAN;
        for (n, &t) in STATE_TEMPERATURES_K.iter().enumerate() {
            let tf = vec![ThermodynamicTemperature::new::<kelvin>(t); mesh.n_cells()];
            let sol = mesh.solve_diffusion(&nd, &tf).map_err(|e| err(e.to_string()))?;
            k[n] = sol.k_eff;
            converged &= sol.converged;
            if let Some(p) = points.points.iter().find(|p| (p.temperature.get::<kelvin>() - t).abs() < 1e-9) {
                k_mc[n] = (p.k_eff.get::<ratio>(), p.k_std.get::<ratio>());
            }
            if n == 0 {
                let re: Vec<f64> = (0..=mesh.mesh.r_edges().n_cells()).map(|i| mesh.mesh.r_edges().edge(i).get::<meter>()).collect();
                let ze: Vec<f64> = (0..=mesh.mesh.z_edges().n_cells()).map(|j| mesh.mesh.z_edges().edge(j).get::<meter>()).collect();
                peaking = rz_peaking(&sol.power_density, &re, &ze).unwrap_or(f64::NAN);
            }
        }
        let dt = STATE_TEMPERATURES_K[1] - STATE_TEMPERATURES_K[0];
        Ok(GenFoamResult {
            resolution,
            cells: mesh.n_cells(),
            k,
            k_mc,
            isothermal_per_k: (1.0 / k[0] - 1.0 / k[1]) / dt,
            peaking,
            converged,
        })
    }

    fn describe(&self, r: &GenFoamResult) -> Vec<String> {
        let mut out = vec![format!(
            "GeN-Foam R-Z diffusion, {} cells ({} cm), constants from the Monte Carlo MGXS records (N = 12 lattice){}",
            r.cells,
            Self::cell_size(r.resolution).get::<centimeter>(),
            if r.converged { "" } else { " — NOT CONVERGED" }
        )];
        for (n, t) in STATE_TEMPERATURES_K.iter().enumerate() {
            let (km, sm) = r.k_mc[n];
            out.push(format!("k at {t:.0} K: {:.5} (Monte Carlo {km:.5} ± {sm:.5}, {:+.0} pcm)", r.k[n], (r.k[n] - km) * 1e5));
        }
        out.push(format!("isothermal coefficient, 600 → 1200 K: {:.2} pcm/K", r.isothermal_per_k * 1e5));
        out.push(format!("power peaking at 600 K (peak / volume mean): {:.3}", r.peaking));
        out.push("Diffusion only: every cell at one TFuel, no thermal-hydraulic feedback in this run; not sent to the plant.".into());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The peaking is the volume-weighted one: a uniform field is 1; a
    /// field doubled in one of two equal-volume cells is 4/3; zero power
    /// gives none.
    #[test]
    fn the_peaking_is_volume_weighted() {
        let r = [0.0, 1.0];
        let z = [0.0, 1.0, 2.0];
        assert!((rz_peaking(&[3.0, 3.0], &r, &z).unwrap() - 1.0).abs() < 1e-15);
        assert!((rz_peaking(&[1.0, 2.0], &r, &z).unwrap() - 4.0 / 3.0).abs() < 1e-15);
        // Two radial annuli of unequal volume (1 and 3 times pi per metre).
        let r2 = [0.0, 1.0, 2.0];
        let p = rz_peaking(&[2.0, 1.0], &r2, &[0.0, 1.0]).unwrap();
        assert!((p - 2.0 / ((2.0 * 1.0 + 1.0 * 3.0) / 4.0)).abs() < 1e-15, "{p}");
        assert_eq!(rz_peaking(&[0.0, 0.0], &r, &z), None);
        assert_eq!(Htr10GenFoam::cell_size(MeshResolution::Fine).get::<centimeter>(), 10.0);
        let g = Htr10GenFoam;
        assert_eq!(g.availability(MeshResolution::Coarse), Availability::Built(MultiphysicsBackend::GenFoam));
        assert_eq!(g.cross_sections(), XsSource::MonteCarlo);
    }

    /// **gh:#1746: the coarse GeN-Foam run on the records.** The record's
    /// own 20 cm mesh and constants (gh:#1187), solved at 600 and 1200 K,
    /// reported beside the Monte Carlo `k`. Asserts convergence, a
    /// negative isothermal coefficient and a peaking above 1; prints the
    /// rest. Not a V&V number: gh:#1187's record is the one to quote.
    ///
    /// **Result (2026-11-25, 1.3 s):** 792 cells; k 0.98867 at 600 K and
    /// 0.93700 at 1200 K, **gh:#1187's recorded GeN-Foam k to every printed
    /// digit** (its README's table: +3463 and +5173 pcm against the Monte
    /// Carlo 0.95405 ± 0.00096 and 0.88527 ± 0.00105). So the backend runs
    /// the record's pipeline unchanged, including its known diffusion
    /// over-prediction. Isothermal secant −9.30 pcm/K (the Monte Carlo
    /// records' own secant is (1/0.95405 − 1/0.88527)/600 = −13.57 pcm/K);
    /// peaking 1.427.
    #[test]
    fn the_coarse_run_reports_against_the_records() {
        let mut g = Htr10GenFoam;
        let r = g.run(MeshResolution::Coarse).expect("coarse run");
        for l in g.describe(&r) {
            println!("{l}");
        }
        assert!(r.converged);
        assert!(r.isothermal_per_k < 0.0, "{}", r.isothermal_per_k);
        assert!(r.peaking > 1.0 && r.peaking.is_finite(), "{}", r.peaking);
        assert!(r.k_mc[0].0.is_finite() && r.k_mc[1].0.is_finite());
    }
}
