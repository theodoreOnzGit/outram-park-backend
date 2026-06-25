use std::sync::Arc;
use openfoam_basic_lib::prelude::*;
use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Transonic/supersonic compressible solver — sonicFoam.
///
/// Uses the compressibility ψ = ρ/p as the primary thermodynamic closure.
/// The pressure equation is:
///
///   ∂(ψp)/∂t + ∇·(ψ_d p) − ∇·(ρ·rAU·∇p) = 0
///
/// where ψ_d = ψ·U is the "density" face velocity field.  The `fvm::div`
/// implicit scalar-convection operator is not yet in this library, so the
/// convective term ∇·(ψ_d p) is treated explicitly via `fvc::div`.
///
/// C++ solver: `applications/solvers/compressible/sonicFoam/`
pub struct SonicFoam {
    pub mesh:     Arc<FvMesh>,
    pub control:  ControlDict,
    pub schemes:  FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u:   VolVectorField,
    /// Pressure [Pa]
    pub p:   VolScalarField,
    /// Density [kg/m³]
    pub rho: VolScalarField,
    /// Specific internal energy e [J/kg]
    pub e:   VolScalarField,
    /// Compressibility ψ = ρ/p [s²/m²]
    pub psi: VolScalarField,
    /// Dynamic viscosity μ [Pa·s]
    pub mu:  VolScalarField,
    /// Mass flux φ = ρ U·Sf [kg/s]
    pub phi: SurfaceScalarField,
}

impl SonicFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u   = VolVectorField::zero("U",  mesh.clone());
        let p   = VolScalarField::uniform("p",  mesh.clone(), 1.0e5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let e   = VolScalarField::zeros("e", mesh.clone());
        let psi = VolScalarField::uniform("psi", mesh.clone(), 1.0e-5);
        let mu  = VolScalarField::uniform("mu",  mesh.clone(), 1.8e-5);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        Self { mesh, control, schemes, solution, u, p, rho, e, psi, mu, phi }
    }

    /// Advance one time step.
    ///
    /// Algorithm (single PISO pass — no outer PIMPLE correctors):
    ///   1. Solve UEqn with explicit pressure gradient  (momentum predictor)
    ///   2. Assemble ψ-based pressure equation and solve
    ///   3. Correct flux φ and velocity U
    ///   4. Update ρ = ψ·p
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.control.delta_t;
        let settings = SolverSettings::default();
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let u_old   = self.u.clone();
        let rho_old = self.rho.clone();
        let p_old   = self.p.clone();

        // ── rhoEqn: explicit continuity ρ_new = ρ_old − dt · ∇·φ ────────────
        let div_phi = fvc::div_flux(&self.phi);
        self.rho = rho_old.clone() + (-dt) * div_phi;
        for c in 0..n { if self.rho.internal[c] < 1e-4 { self.rho.internal[c] = 1e-4; } }

        // ── UEqn: ∂(ρU)/∂t + ∇·(ρUU) − ∇·(μ∇U) ────────────────────────────
        let mut u_eqn = fvm::ddt_coeff_vec(&self.rho, &self.u, &u_old, dt, mesh.clone())
            + fvm::div_vec(&self.phi, &self.u, mesh.clone())
            - fvm::laplacian_vec(&self.mu, &self.u, mesh.clone());

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

        // Momentum predictor: −V·∇p → source, solve, restore
        let gp = fvc::grad(&self.p);
        for c in 0..n {
            u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
        }
        let (u_pred, _) = u_eqn.solve("U", settings);
        for c in 0..n {
            u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
        }
        self.u = u_pred;

        let h = u_eqn.h_field(&self.u);
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

        // rhor AUf = ρ_f · rAU_f  [s] — face coefficient for pressure Laplacian
        let rho_f  = fvc::interpolate(&self.rho);
        let rauf   = fvc::interpolate(&rau);
        let rho_rauf = rho_f.clone() * rauf.clone();

        // phiHbyA = ρ_f · flux(HbyA) [kg/s]
        let vol_hbya = fvc::flux(&hbya);
        let phi_hbya = rho_f * vol_hbya;

        // Raw mass-flux divergence Σ_f phi_hbya_f for RHS [kg/s]
        let source_p_base = {
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

        // ψ face flux: phid = psi_f · vol_phi = ψ·U·Sf [s²/m² · m³/s = s/m]
        // Used for explicit convection term ∇·(ψ·U·p): phid·p is a density flux
        let psi_f = fvc::interpolate(&self.psi);
        let vol_phi = fvc::flux(&self.u);             // U·Sf [m³/s]
        let phid   = psi_f * vol_phi;                 // ψ_f · U·Sf [s/m · m³/s = m²/s... units: s²/m² * m³/s = s/m]

        // Explicit ∇·(ψ_d p): contributes to RHS as convective term for p
        let div_phid_p = fvc::div(&phid, &self.p);   // ∇·(phid·p)/V, per-unit-volume

        // ── PISO inner pressure correctors ────────────────────────────────────
        // pEqn: ψ·V/dt · p − ∇·(ρ·rAU·∇p) = ψ·V/dt · p_old − Σ_f phi_hbya_f − V·∇·(ψ_d p)
        for _ in 0..n_inner {
            let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p);
            let psi_sl   = self.psi.internal.as_slice();
            let p_old_sl = p_old.internal.as_slice();
            let ddp_sl   = div_phid_p.internal.as_slice();
            let mut src = source_p_base.clone();
            for c in 0..n {
                let pvdt = psi_sl[c] * mesh.cell_volumes[c] / dt;
                // Diagonal: implicit ∂(ψp)/∂t term
                p_eqn.ldu.diag[c] += pvdt;
                // Source: ψ·V/dt · p_old (old-time term)
                src[c] += pvdt * p_old_sl[c];
                // Source: −V · ∇·(ψ_d p) (explicit convective correction, subtract)
                src[c] -= mesh.cell_volumes[c] * ddp_sl[c];
            }
            p_eqn.source = Field::new(src);
            let (p_new, _) = p_eqn.solve("p", settings);
            self.p = p_new;
        }

        // ── Corrections ───────────────────────────────────────────────────────
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
        self.u = hbya - rau * fvc::grad(&self.p);

        // Update ρ = ψ · p
        {
            let psi_sl = self.psi.internal.as_slice();
            let p_sl   = self.p.internal.as_slice();
            for c in 0..n {
                self.rho.internal[c] = (psi_sl[c] * p_sl[c]).max(1e-4);
            }
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
