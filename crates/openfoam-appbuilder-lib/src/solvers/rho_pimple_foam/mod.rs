use std::sync::Arc;
use openfoam_basic_lib::prelude::*;
use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Compressible transient PIMPLE solver — rhoPimpleFoam.
///
/// Solves:
///   ∂ρ/∂t  + ∇·(ρU)     = 0          (continuity)
///   ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ  (momentum)
///   ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt      (energy, h-form, adiabatic closure)
///   ρ = ψ·p                            (EOS approximation)
///
/// Pressure equation includes the compressibility term ψ·∂p/∂t so that the
/// system is consistent with the linearised continuity equation.
///
/// C++ solver: `applications/solvers/compressible/rhoPimpleFoam/`
pub struct RhoPimpleFoam {
    pub mesh:     Arc<FvMesh>,
    pub control:  ControlDict,
    pub schemes:  FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u:       VolVectorField,
    /// Pressure field [Pa]
    pub p:       VolScalarField,
    /// Density field [kg/m³]
    pub rho:     VolScalarField,
    /// Temperature field [K]
    pub t:       VolScalarField,
    /// Specific enthalpy [J/kg]
    pub he:      VolScalarField,
    /// Dynamic viscosity μ [Pa·s]
    pub mu:      VolScalarField,
    /// Effective thermal diffusivity αh = κ/Cp [kg/(m·s)]
    pub alpha_h: VolScalarField,
    /// Compressibility ψ = ∂ρ/∂p|_T = ρ/p [s²/m²]
    pub psi:     VolScalarField,
    /// Mass flux φ = ρ U·Sf [kg/s]
    pub phi:     SurfaceScalarField,
}

impl RhoPimpleFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u       = VolVectorField::zero("U",       mesh.clone());
        let p       = VolScalarField::uniform("p",    mesh.clone(), 1.0e5);
        let rho     = VolScalarField::uniform("rho",  mesh.clone(), 1.0);
        let t       = VolScalarField::uniform("T",    mesh.clone(), 300.0);
        let he      = VolScalarField::zeros("he",     mesh.clone());
        let mu      = VolScalarField::uniform("mu",   mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi     = VolScalarField::uniform("psi",  mesh.clone(), 1.0e-5);
        let phi     = SurfaceScalarField::zeros("phi", mesh.clone());
        Self { mesh, control, schemes, solution, u, p, rho, t, he, mu, alpha_h, psi, phi }
    }

    /// Advance one time step with compressible PIMPLE.
    ///
    /// The pressure equation includes the compressibility diagonal term
    /// ψ·V/dt so the system is consistent with ρ = ψ·p.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.control.delta_t;
        let settings = SolverSettings::default();
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let u_old   = self.u.clone();
        let p_old   = self.p.clone();
        let he_old  = self.he.clone();

        for _ in 0..n_outer {
            // ── rhoEqn: explicit continuity ρ_new = ρ_old − dt · ∇·(ρU) ─────
            let div_phi = fvc::div_flux(&self.phi);   // ∇·φ per unit volume [1/s]
            self.rho = self.rho.clone() + (-dt) * div_phi;
            // Clamp density to a physical minimum
            for c in 0..n {
                if self.rho.internal[c] < 1e-4 { self.rho.internal[c] = 1e-4; }
            }

            // ── UEqn: ∂(ρU)/∂t + ∇·(ρUU) − ∇·(μ∇U) ────────────────────────
            let mut u_eqn = fvm::ddt_coeff_vec(&self.rho, &self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                - fvm::laplacian_vec(&self.mu, &self.u, mesh.clone());

            // A [kg/s];  rAU = V/A [m³·s/kg]
            let a = u_eqn.a_field();
            let rau = {
                let a_sl = a.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_sl[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU", mesh.clone(), Field::new(vals),
                    mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect(),
                )
            };

            // Momentum predictor with explicit −V·∇p
            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            let (u_pred, _) = u_eqn.solve("U", settings);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            // H(U_pred) from clean source [m·kg/s²]
            let h = u_eqn.h_field(&self.u);

            // HbyA = H/A [m/s]
            let hbya = {
                let h_sl = h.internal.as_slice();
                let a_sl = a.internal.as_slice();
                let vals: Vec<Vector3> = (0..n)
                    .map(|c| h_sl[c] * (1.0 / a_sl[c].max(1e-30)))
                    .collect();
                VolVectorField::new(
                    "HbyA", mesh.clone(), Field::new(vals),
                    mesh.patches.iter().map(|p| PatchField::zero_gradient_vec(p.size)).collect(),
                )
            };

            // Interpolated fields at faces
            let rho_f  = fvc::interpolate(&self.rho);   // ρ_f [kg/m³]
            let rauf   = fvc::interpolate(&rau);         // rAU_f [m³·s/kg]
            // ρ_f · rAU_f → face coefficient for pressure Laplacian [m³·s/m³ = s... no, m³·s/kg · kg/m³ = s]
            let rho_rauf = rho_f.clone() * rauf.clone(); // [s]

            // phi_hbya = ρ_f · flux(HbyA): mass flux from HbyA [kg/s]
            let vol_hbya = fvc::flux(&hbya);     // [m³/s]
            let phi_hbya = rho_f * vol_hbya;     // [kg/s]

            // Raw mass-flux divergence for pressure source [kg/s]
            let source_p = {
                let mut s = vec![0.0_f64; n];
                let phi_int = phi_hbya.internal.as_slice();
                for f in 0..mesh.n_internal_faces {
                    s[mesh.owner[f]]     += phi_int[f];
                    s[mesh.neighbour[f]] -= phi_int[f];
                }
                for (pi, patch) in mesh.patches.iter().enumerate() {
                    let phi_bc = phi_hbya.boundary[pi].values.as_slice();
                    for fi in 0..patch.size {
                        s[mesh.owner[patch.start + fi]] += phi_bc[fi];
                    }
                }
                s
            };

            // Compressibility source terms: ψ·V/dt · p_old (for diagonal ψ·V/dt added to p_eqn)
            let mut psi_vdt = vec![0.0_f64; n];
            {
                let psi_sl   = self.psi.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                for c in 0..n {
                    psi_vdt[c] = psi_sl[c] * mesh.cell_volumes[c] / dt;
                    // source will be augmented in the loop below
                    let _ = p_old_sl; // used below
                }
            }

            // ── PISO inner pressure correctors ────────────────────────────────
            for _ in 0..n_inner {
                // p_eqn: ∇·(ρ_f·rAU_f·∇p) + ψ·V/dt·p = Σ_f phi_hbya_f + ψ·V/dt·p_old
                let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p);
                let psi_sl   = self.psi.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                let mut full_source = source_p.clone();
                for c in 0..n {
                    let pvdt = psi_sl[c] * mesh.cell_volumes[c] / dt;
                    p_eqn.ldu.diag[c] += pvdt;
                    full_source[c] += pvdt * p_old_sl[c];
                }
                p_eqn.source = Field::new(full_source);
                let (p_new, _) = p_eqn.solve("p", settings);
                self.p = p_new;
            }

            // ── Final corrections ─────────────────────────────────────────────
            // Correct mass flux: phi = phi_hbya − ρ_f·rAU_f·snGrad(p)·|Sf|
            let sng = fvc::sn_grad(&self.p);
            {
                let sng_sl      = sng.internal.as_slice();
                let rho_rauf_sl = rho_rauf.internal.as_slice();
                let mut phi_corr = phi_hbya;
                for f in 0..mesh.n_internal_faces {
                    phi_corr.internal[f] -= rho_rauf_sl[f] * sng_sl[f] * mesh.face_areas[f];
                }
                self.phi = phi_corr;
            }

            // U = HbyA − rAU · ∇p  [m/s]
            self.u = hbya - rau * fvc::grad(&self.p);

            // Update density from EOS: ρ = ψ · p
            {
                let psi_sl = self.psi.internal.as_slice();
                let p_sl   = self.p.internal.as_slice();
                for c in 0..n {
                    self.rho.internal[c] = (psi_sl[c] * p_sl[c]).max(1e-4);
                }
            }

            // ── Energy equation (semi-implicit, explicit convection) ──────────
            // ∂(ρh)/∂t + ∇·(φh) − ∇·(αh∇h) = dp/dt
            let conv_he     = fvc::div(&self.phi, &self.he);  // explicit ∇·(φh)/V
            let alpha_h_f   = fvc::interpolate(&self.alpha_h);
            let dp_dt       = (self.p.clone() - p_old.clone()) * (1.0 / dt);

            let mut e_eqn = fvm::ddt_coeff(&self.rho, &self.he, &he_old, dt)
                - fvm::laplacian(&alpha_h_f, &self.he);
            {
                let conv_sl = conv_he.internal.as_slice();
                let dpdt_sl = dp_dt.internal.as_slice();
                for c in 0..n {
                    let v = mesh.cell_volumes[c];
                    e_eqn.source[c] -= v * conv_sl[c];  // explicit convection
                    e_eqn.source[c] += v * dpdt_sl[c];  // dp/dt source
                }
            }
            let (he_new, _) = e_eqn.solve("he", settings);
            self.he = he_new;
        }

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        let start = match self.control.start {
            StartControl::StartTime(t) => t,
            _ => 0.0,
        };
        let end = match self.control.stop {
            StopControl::EndTime(t) => t,
            _ => return Ok(()),
        };
        let dt = self.control.delta_t;
        let mut time = start;
        while time < end {
            self.step()?;
            time += dt;
        }
        Ok(())
    }
}
