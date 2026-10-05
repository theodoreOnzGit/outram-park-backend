//! Step 10's spatial neutronics and its Picard coupling to the porous core
//! (gh:#591).
//!
//! ```text
//!   Step 8 nuclearData (TFuel, ln T)          Step 7 meshes + maps
//!            │                                        │
//!            ▼                                        ▼
//!   ┌──────────────────────────┐  q'''_N   ┌────────────────────┐  q'''_TH  ┌─────────────────────┐
//!   │ neutronics mesh (14k     │──────────▶│ TH polyMesh (bed + │──────────▶│ r-z ring grid       │
//!   │ polyhedra): GeN-Foam     │  N→TH map │ cavity), per cell  │ sampled,  │ (porous_core march, │
//!   │ DiffusionNeutronics,     │           │                    │ conserva- │  40 × 200 nodes)    │
//!   │ XS at each cell's TFuel  │◀──────────│                    │◀──────────│                     │
//!   └──────────────────────────┘  TH→N map └────────────────────┘ T at the  └─────────────────────┘
//!            ▲  TFuel_N               T_TH  (cavity: inlet T)    centroid   T_pebble,avg per node
//!            └── reflector cells outside the TH mesh: held at the inlet helium temperature
//! ```
//!
//! **What is solved.** Each Picard iteration:
//!
//! 1. **Neutronics.** The GeN-Foam port's multigroup diffusion k-eigenvalue
//!    (`outram_foam_appbuilder_lib::genfoam::neutronics::DiffusionNeutronics`,
//!    a port of upstream `diffusionNeutronics`) on Step 7's neutronics
//!    `polyMesh`, built with `new_with_cell_parameters`: Step 8's
//!    `nuclearData` is evaluated at **each cell's own** `TFuel` with the
//!    upstream polyharmonic-spline interpolation in the file's law (`log`:
//!    linear in ln T between state points, linear extrapolation outside
//!    them, as upstream). The outer boundary is a Marshak vacuum (albedo
//!    `gamma = 1/2`, [`NeutronBoundary`]). The flux and `k` of the last
//!    iteration seed the next (warm start). The power density
//!    (`sum_g phi_g sigmaPow_g`) is scaled to the thermal power.
//! 2. **Neutronics → TH.** Step 7's `MeshMapping` neutronics → TH (upstream
//!    `mapTgtToSrc`, volume-weighted) gives the power density on the TH mesh;
//!    each bed cell's power `q V` is then shared among the ring-grid nodes
//!    in proportion to how many of the nodes' sample points (2 radii × 64
//!    azimuths per node) have that cell as their nearest bed
//!    cell, which keeps the bed power exactly; a cell no sample reached gives
//!    its power to the node at its centroid. Power the neutronics puts in TH
//!    cavity cells or outside the TH mesh (region stair-stepping, gh:#594) is
//!    reported, and the node power is rescaled to the thermal power (all of
//!    it is deposited in the bed: no gamma heating of the reflectors).
//! 3. **Relaxation.** The node power is under-relaxed (Step 9's
//!    `power_relaxation`) and handed to the march
//!    ([`crate::porous_core::PorousCore::set_node_power`]), which then does
//!    one outer iteration (march + flow split).
//! 4. **TH → neutronics.** Each TH bed cell takes the fuel-pebble volume
//!    average temperature of the node at its centroid; cavity cells the
//!    inlet helium temperature; Step 7's TH → neutronics map gives `TFuel`
//!    on the neutronics cells it covers; the rest (the reflectors, the
//!    conus, the discharge tube) are held at the inlet helium temperature.
//!
//! Converged when the march's own criteria hold (ring Δp spread, ΔT) **and**
//! the unrelaxed node power and `k` stopped moving (Step 9's
//! `power_tolerance`, `k_tolerance`).
//!
//! **What this is not.** One `TFuel` drives every material's constants (the
//! Step 8 state points are isothermal, gh:#595); there is no separate
//! moderator or coolant feedback, no reflector heat balance, no delayed
//! neutrons (steady state only), and the constants are P0, `D = 1/(3 Σ_t)`
//! (gh:#595) on stair-stepped regions (gh:#594).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use dhoby_ghaut::workbench::meshes::MeshRole;
use dhoby_ghaut::workbench::multiphysics::{MultiphysicsInputs, MultiphysicsSetup};
use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::AlbedoLinearisation;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};

use crate::porous_core::{Fields, IterationReport, PorousCore, Summary};

/// The outer boundary of the neutronics mesh (the Monte Carlo model's
/// vacuum boundary).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutronBoundary {
    /// **Default.** Marshak vacuum: albedo `gamma = 1/2` (GeN-Foam
    /// `albedoSP3`), with the exact face closure `J = gamma phi_face`
    /// ([`AlbedoLinearisation::FaceValue`]), which stays bounded on the
    /// 30 cm cells at the reflector's outer surface.
    MarshakFace,
    /// Ablation: upstream's linearisation, `J = gamma phi_cell`
    /// ([`AlbedoLinearisation::CellValue`]).
    MarshakCell,
    /// Ablation: zero flux on the physical boundary (no extrapolation
    /// distance).
    ZeroFlux,
}

impl NeutronBoundary {
    /// Human label.
    pub fn label(self) -> &'static str {
        match self {
            Self::MarshakFace => "Marshak vacuum (albedo 1/2, face closure)",
            Self::MarshakCell => "Marshak vacuum (albedo 1/2, GeN-Foam cell-value linearisation)",
            Self::ZeroFlux => "zero flux on the boundary",
        }
    }
}

/// One neutronics solve's result.
#[derive(Clone, Debug)]
pub struct NeutronicsSolution {
    /// The eigenvalue.
    pub k: f64,
    /// Outer (power) iterations it took.
    pub outer_iterations: usize,
    /// Power density per neutronics cell \[W/m³\], summing to the target
    /// power over the mesh.
    pub q_w_m3: Vec<f64>,
    /// Group flux per cell (arbitrary amplitude).
    pub flux: Vec<Vec<f64>>,
    /// Production rate density `sum_g nu Sigma_f phi_g` per cell (same
    /// amplitude as `flux`).
    pub production: Vec<f64>,
    /// Absorption rate density per cell (removal less out-scatter).
    pub absorption: Vec<f64>,
    /// Cells whose `TFuel` was outside the state points (extrapolated).
    pub extrapolated_cells: usize,
    /// Wall-clock seconds.
    pub seconds: f64,
}

/// The diffusion model on Step 7's neutronics mesh with Step 8's constants.
pub struct Neutronics {
    mesh: Arc<FvMesh>,
    xs: Arc<CrossSectionData>,
    zone_of_cell: Vec<usize>,
    boundary: NeutronBoundary,
    settings: DiffusionSettings,
    /// Last converged flux per group (warm start).
    flux: Option<Vec<Vec<f64>>>,
    k: f64,
    /// State-point temperature range \[K\].
    t_range: [f64; 2],
}

fn s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl Neutronics {
    /// Read the neutronics `polyMesh` and its cellZones and Step 8's
    /// `nuclearData`, and check that the zones the solver will use are the
    /// regions Step 7 assigned.
    ///
    /// # Errors
    ///
    /// A missing or inconsistent input, named.
    pub fn new(inputs: &MultiphysicsInputs, boundary: NeutronBoundary) -> Result<Self, String> {
        inputs.check_for_neutronics()?;
        let ms = &inputs.meshes;
        let n = ms.mesh(MeshRole::Neutronics).ok_or("no neutronics mesh")?;
        let dir = PathBuf::from(n.polymesh_dir.clone().ok_or("neutronics mesh not written")?);
        let mesh = read_poly_mesh(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        if mesh.n_cells != n.cells {
            return Err(format!(
                "the neutronics polyMesh has {} cells, Step 7 reported {}",
                mesh.n_cells, n.cells
            ));
        }
        let nd_path = PathBuf::from(
            inputs
                .mgxs
                .nuclear_data_path
                .clone()
                .ok_or("Step 8 wrote no nuclearData")?,
        );
        let nd = read_nuclear_data(&nd_path).map_err(|e| format!("{}: {e}", nd_path.display()))?;
        let xs = CrossSectionData::from_input(&nd).map_err(|e| format!("nuclearData: {e:?}"))?;
        if xs.variable_names() != ["TFuel"] {
            return Err(format!(
                "Step 10 drives the cross sections with TFuel only; the nuclearData declares {:?}",
                xs.variable_names()
            ));
        }
        let names: Vec<String> = (0..xs.zone_count())
            .map(|i| xs.zone(i).map(|z| z.name().to_string()).unwrap_or_default())
            .collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let zones: Vec<_> = read_cell_zones(&dir)
            .map_err(s)?
            .into_iter()
            .filter(|z| !z.cells.is_empty())
            .collect();
        let zone_of = zone_of_cell(&zones, &name_refs, mesh.n_cells).map_err(s)?;
        // The cellZones on disk must say what Step 7's cell_region says.
        let regions = &ms.plan.regions.regions;
        for (c, &z) in zone_of.iter().enumerate() {
            let want = &regions[n.cell_region[c]].id;
            if names[z] != *want {
                return Err(format!(
                    "neutronics cell {c}: cellZones say {}, Step 7's region map says {want}",
                    names[z]
                ));
            }
        }
        let temps: Vec<f64> = inputs.mgxs.states.iter().map(|s| s.temperature_k).collect();
        let t_range = [
            temps.iter().copied().fold(f64::MAX, f64::min),
            temps.iter().copied().fold(f64::MIN, f64::max),
        ];
        Ok(Self {
            mesh,
            xs: Arc::new(xs),
            zone_of_cell: zone_of,
            boundary,
            settings: DiffusionSettings::default(),
            flux: None,
            k: 1.0,
            t_range,
        })
    }

    /// The neutronics mesh.
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// Forget the warm start (the next solve starts from a flat flux).
    pub fn reset(&mut self) {
        self.flux = None;
        self.k = 1.0;
    }

    /// Solve the k-eigenvalue with each cell's constants at `t_cell_k` \[K\]
    /// and scale the power density to `power_w`.
    ///
    /// # Errors
    ///
    /// A cross-section evaluation or eigenvalue failure.
    pub fn solve(&mut self, t_cell_k: &[f64], power_w: f64) -> Result<NeutronicsSolution, String> {
        let t0 = Instant::now();
        let n = self.mesh.n_cells;
        if t_cell_k.len() != n {
            return Err(format!("{} temperatures for {n} cells", t_cell_k.len()));
        }
        let extrapolated_cells = t_cell_k
            .iter()
            .filter(|&&t| t < self.t_range[0] - 1e-9 || t > self.t_range[1] + 1e-9)
            .count();
        let bc = match self.boundary {
            NeutronBoundary::ZeroFlux => BoundaryCondition::FixedValue(0.0),
            _ => BoundaryCondition::ZeroGradient,
        };
        let flux_bc = vec![bc; self.mesh.patches.len()];
        let mut model = DiffusionNeutronics::new_with_cell_parameters(
            self.mesh.clone(),
            &self.xs,
            &self.zone_of_cell,
            t_cell_k,
            &flux_bc,
            self.settings,
        )
        .map_err(|e| format!("diffusion set-up: {e}"))?;
        let lin = match self.boundary {
            NeutronBoundary::MarshakFace => Some(AlbedoLinearisation::FaceValue),
            NeutronBoundary::MarshakCell => Some(AlbedoLinearisation::CellValue),
            NeutronBoundary::ZeroFlux => None,
        };
        if let Some(lin) = lin {
            for p in 0..self.mesh.patches.len() {
                model.set_albedo_boundary(p, 0.5, lin);
            }
        }
        if let Some(prev) = &self.flux {
            model.set_initial_guess(prev, self.k);
        }
        let rep = model
            .solve_eigenvalue()
            .map_err(|e| format!("diffusion eigenvalue: {e}"))?;
        let st = model.state();
        self.flux = Some(
            st.flux()
                .iter()
                .map(|f| f.internal.as_slice().to_vec())
                .collect(),
        );
        self.k = rep.k_eff;
        // Neutron balance per cell, for the region-by-region comparison with
        // the Monte Carlo tallies: production and absorption rates (the
        // latter as removal less out-scatter, the solver's own absorption).
        let xf = model.xs_fields();
        let ng = st.flux().len();
        let mut prod = vec![0.0; n];
        let mut absr = vec![0.0; n];
        for g in 0..ng {
            let ph = st.flux()[g].internal.as_slice();
            let nf = xf.nu_sigma_f[g].internal.as_slice();
            let sr = xf.sigma_removal[g].internal.as_slice();
            for c in 0..n {
                let out: f64 = (0..ng)
                    .filter(|&h| h != g)
                    .map(|h| xf.scattering[g][h].internal.as_slice()[c])
                    .sum();
                prod[c] += nf[c] * ph[c];
                absr[c] += (sr[c] - out) * ph[c];
            }
        }
        let flux_groups: Vec<Vec<f64>> = st
            .flux()
            .iter()
            .map(|f| f.internal.as_slice().to_vec())
            .collect();
        let pd = st.power_density().internal.as_slice();
        let total: f64 = pd.iter().zip(&self.mesh.cell_volumes).map(|(q, v)| q * v).sum();
        if !(total > 0.0) {
            return Err("the diffusion solution carries no fission power".into());
        }
        let scale = power_w / total;
        Ok(NeutronicsSolution {
            k: rep.k_eff,
            outer_iterations: rep.outer_iterations,
            q_w_m3: pd.iter().map(|q| q * scale).collect(),
            flux: flux_groups,
            production: prod,
            absorption: absr,
            extrapolated_cells,
            seconds: t0.elapsed().as_secs_f64(),
        })
    }
}

/// Where the neutronics power landed, as fractions of the thermal power.
#[derive(Clone, Copy, Debug, Default)]
pub struct PowerSplit {
    /// In TH bed cells (handed to the march).
    pub bed: f64,
    /// In TH cavity cells (dropped; the cavity has no fuel, but on the 30 cm
    /// neutronics mesh its region holds bed cells, gh:#594).
    pub cavity: f64,
    /// Outside the TH mesh (the conus, the tube, reflector cells holding
    /// stair-stepped bed; dropped).
    pub outside: f64,
}

/// The transfers between the neutronics mesh, the TH mesh and the ring grid.
pub struct Transfer {
    n_cells_n: usize,
    th_vol_m3: Vec<f64>,
    th_bed: Vec<bool>,
    /// Per TH bed cell: `(node, share)` with shares summing to 1.
    cell_share: Vec<Vec<(usize, f64)>>,
    /// Node containing each TH bed cell's centroid.
    cell_node: Vec<usize>,
    n_nodes: usize,
}

/// Uniform-grid nearest-point search over a few thousand centroids.
struct Nearest {
    lo: [f64; 3],
    h: f64,
    dims: [usize; 3],
    buckets: Vec<Vec<usize>>,
    pts: Vec<[f64; 3]>,
}

impl Nearest {
    fn new(pts: Vec<[f64; 3]>, h: f64) -> Self {
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for p in &pts {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let dims = [0, 1, 2].map(|a| (((hi[a] - lo[a]) / h).floor() as usize + 1).max(1));
        let mut buckets = vec![Vec::new(); dims[0] * dims[1] * dims[2]];
        for (i, p) in pts.iter().enumerate() {
            let b = Self::cell_of(lo, h, dims, *p);
            buckets[b[0] + dims[0] * (b[1] + dims[1] * b[2])].push(i);
        }
        Self {
            lo,
            h,
            dims,
            buckets,
            pts,
        }
    }

    fn cell_of(lo: [f64; 3], h: f64, dims: [usize; 3], p: [f64; 3]) -> [usize; 3] {
        [0, 1, 2].map(|a| (((p[a] - lo[a]) / h).floor().max(0.0) as usize).min(dims[a] - 1))
    }

    /// Index of the nearest point (exact: shells grow until the best found is
    /// closer than any unsearched bucket).
    fn nearest(&self, p: [f64; 3]) -> usize {
        let c = Self::cell_of(self.lo, self.h, self.dims, p);
        let mut best = (f64::MAX, 0usize);
        let max_shell = *self.dims.iter().max().unwrap_or(&1);
        for shell in 0..=max_shell {
            let sh = shell as i64;
            for dz in -sh..=sh {
                for dy in -sh..=sh {
                    for dx in -sh..=sh {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != sh {
                            continue;
                        }
                        let q = [c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz];
                        if (0..3).any(|a| q[a] < 0 || q[a] >= self.dims[a] as i64) {
                            continue;
                        }
                        let b = q[0] as usize
                            + self.dims[0] * (q[1] as usize + self.dims[1] * q[2] as usize);
                        for &i in &self.buckets[b] {
                            let d: f64 = (0..3).map(|a| (self.pts[i][a] - p[a]).powi(2)).sum();
                            if d < best.0 {
                                best = (d, i);
                            }
                        }
                    }
                }
            }
            // Every point in an unsearched shell is at least `shell * h` away.
            if best.0 < f64::MAX && best.0.sqrt() <= shell as f64 * self.h {
                break;
            }
        }
        best.1
    }
}

impl Transfer {
    /// Build the sampling between the TH mesh's bed cells and the ring grid
    /// of `core`. The grid is laid on the bed of Step 7's R-Z domain (its
    /// radius and height); `note` gets a line if that differs from Step 9's.
    ///
    /// # Errors
    ///
    /// A missing mesh or a TH mesh with no bed cells.
    pub fn new(
        inputs: &MultiphysicsInputs,
        core: &PorousCore,
        note: &mut Vec<String>,
    ) -> Result<Self, String> {
        let ms = &inputs.meshes;
        let th = ms.mesh(MeshRole::ThermalHydraulics).ok_or("no TH mesh")?;
        let n = ms.mesh(MeshRole::Neutronics).ok_or("no neutronics mesh")?;
        let dir = PathBuf::from(th.polymesh_dir.clone().ok_or("TH mesh not written")?);
        let fv = read_poly_mesh(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        if fv.n_cells != th.cells || th.cell_region.len() != th.cells {
            return Err("the TH polyMesh does not match Step 7's TH summary".into());
        }
        let regions = &ms.plan.regions;
        let th_bed: Vec<bool> = th.cell_region.iter().map(|&g| regions.is_bed(g)).collect();
        let d = &ms.domain;
        let r_bed = d.core_radius / 100.0;
        let (z_bot, z_top) = (d.conus_top / 100.0, d.bed_top / 100.0);
        let h_bed = z_top - z_bot;
        let f = &core.setup().foam;
        if (f.core_radius_cm - d.core_radius).abs() > 0.5
            || (f.core_height_cm - 100.0 * h_bed).abs() > 0.5
        {
            note.push(format!(
                "the ring grid is laid on Step 7's bed ({:.1} cm radius, {:.1} cm high); Step 9 says {:.1} x {:.1} cm: the march uses Step 9's dimensions",
                d.core_radius,
                100.0 * h_bed,
                f.core_radius_cm,
                f.core_height_cm
            ));
        }
        let fields = core.fields();
        let (n_r, n_z) = (fields.n_r, fields.n_z);
        let r_frac: Vec<f64> = fields
            .ring_outer_m
            .iter()
            .map(|r| (r / fields.ring_outer_m[n_r - 1]).powi(2))
            .collect();
        // Nearest bed-cell search.
        let bed_cells: Vec<usize> = (0..th.cells).filter(|&c| th_bed[c]).collect();
        if bed_cells.is_empty() {
            return Err("the TH mesh holds no pebble-bed cells".into());
        }
        let pts: Vec<[f64; 3]> = bed_cells
            .iter()
            .map(|&c| {
                let p = fv.cell_centres[c];
                [p.x, p.y, p.z]
            })
            .collect();
        let near = Nearest::new(pts, 0.1);
        let mut hits: Vec<Vec<(usize, f64)>> = vec![Vec::new(); th.cells];
        const NR: usize = 2;
        const NT: usize = 64;
        const NZ: usize = 1;
        for i in 0..n_r {
            let a0 = if i == 0 { 0.0 } else { r_frac[i - 1] };
            let a1 = r_frac[i];
            for j in 0..n_z {
                let k = i * n_z + j;
                for ir in 0..NR {
                    let r = r_bed * (a0 + (a1 - a0) * (ir as f64 + 0.5) / NR as f64).sqrt();
                    for it in 0..NT {
                        let th_ang = (it as f64 + 0.5 + 0.5 * ir as f64) * std::f64::consts::TAU
                            / NT as f64;
                        for iz in 0..NZ {
                            let depth = (j as f64 + (iz as f64 + 0.5) / NZ as f64) / n_z as f64;
                            let z = z_top - depth * h_bed;
                            let c = bed_cells[near.nearest([r * th_ang.cos(), r * th_ang.sin(), z])];
                            match hits[c].last_mut() {
                                Some(last) if last.0 == k => last.1 += 1.0,
                                _ => hits[c].push((k, 1.0)),
                            }
                        }
                    }
                }
            }
        }
        let mut cell_node = vec![usize::MAX; th.cells];
        let mut unreached = 0;
        for &c in &bed_cells {
            let p = fv.cell_centres[c];
            let rr = (p.x * p.x + p.y * p.y).sqrt();
            let a = (rr / r_bed).powi(2);
            let i = r_frac.iter().position(|&q| a <= q).unwrap_or(n_r - 1);
            let depth = ((z_top - p.z) / h_bed).clamp(0.0, 1.0 - 1e-12);
            let j = ((depth * n_z as f64).floor() as usize).min(n_z - 1);
            cell_node[c] = i * n_z + j;
            let tot: f64 = hits[c].iter().map(|h| h.1).sum();
            if tot > 0.0 {
                for h in &mut hits[c] {
                    h.1 /= tot;
                }
            } else {
                hits[c] = vec![(cell_node[c], 1.0)];
                unreached += 1;
            }
        }
        if unreached > 0 {
            note.push(format!(
                "{unreached} of {} TH bed cells were nearest to no node sample; each gives its power to the node at its centroid",
                bed_cells.len()
            ));
        }
        Ok(Self {
            n_cells_n: n.cells,
            th_vol_m3: fv.cell_volumes.clone(),
            th_bed,
            cell_share: hits,
            cell_node,
            n_nodes: n_r * n_z,
        })
    }

    /// Neutronics power density \[W/m³\] → node power \[W\], before the
    /// rescale; with where the power went. `p_total` is the neutronics total.
    pub fn power_to_nodes_raw(
        &self,
        inputs: &MultiphysicsInputs,
        q_n: &[f64],
        p_total: f64,
    ) -> Result<(Vec<f64>, PowerSplit, f64), String> {
        let map = inputs
            .meshes
            .mapping(MeshRole::Neutronics, MeshRole::ThermalHydraulics)
            .ok_or("no neutronics -> TH map")?;
        let mut q_th = vec![0.0; self.th_vol_m3.len()];
        map.map(q_n, &mut q_th);
        let mut p_nodes = vec![0.0; self.n_nodes];
        let (mut bed, mut cav) = (0.0, 0.0);
        for (c, (&q, &v)) in q_th.iter().zip(&self.th_vol_m3).enumerate() {
            let p = q * v;
            if self.th_bed[c] {
                bed += p;
                for &(k, w) in &self.cell_share[c] {
                    p_nodes[k] += p * w;
                }
            } else {
                cav += p;
            }
        }
        let split = PowerSplit {
            bed: bed / p_total,
            cavity: cav / p_total,
            outside: 1.0 - (bed + cav) / p_total,
        };
        Ok((p_nodes, split, bed))
    }

    /// Node fuel-pebble temperatures → `TFuel` on the neutronics cells \[K\].
    /// TH cavity cells, and neutronics cells the TH mesh does not cover,
    /// take `t_outside_k`.
    pub fn temperatures_to_neutronics(
        &self,
        inputs: &MultiphysicsInputs,
        t_node_k: &[f64],
        t_outside_k: f64,
    ) -> Result<Vec<f64>, String> {
        let map = inputs
            .meshes
            .mapping(MeshRole::ThermalHydraulics, MeshRole::Neutronics)
            .ok_or("no TH -> neutronics map")?;
        let t_th: Vec<f64> = (0..self.th_vol_m3.len())
            .map(|c| {
                if self.th_bed[c] {
                    t_node_k[self.cell_node[c]]
                } else {
                    t_outside_k
                }
            })
            .collect();
        let mut t_n = vec![t_outside_k; self.n_cells_n];
        map.map(&t_th, &mut t_n);
        Ok(t_n)
    }
}

/// The spatial half of one coupling iteration, streamed with the march's
/// [`IterationReport`].
#[derive(Clone, Debug)]
pub struct SpatialReport {
    /// Diffusion eigenvalue.
    pub k_eff: f64,
    /// |Δk| since the last iteration.
    pub k_change: f64,
    /// Largest unrelaxed change of a node's power / largest node power.
    pub power_change: f64,
    /// Outer iterations of this eigenvalue solve.
    pub outer_iterations: usize,
    /// Where the neutronics power landed.
    pub split: PowerSplit,
    /// Neutronics cells whose temperature was outside the state points.
    pub extrapolated_cells: usize,
    /// Seconds in the neutronics this iteration.
    pub neutronics_s: f64,
}

/// What the spatial run leaves for drawing and the record.
#[derive(Clone, Debug, Default)]
pub struct SpatialFields {
    /// Power density per neutronics cell \[W/m³\].
    pub q_n_w_m3: Vec<f64>,
    /// `TFuel` per neutronics cell \[K\].
    pub t_n_k: Vec<f64>,
}

/// The node temperature the cross sections see: the fuel pebbles' volume
/// average, mixed with unfuelled pebbles (at the helium temperature) by
/// count, as the lumped feedback of the ablation does.
pub fn node_feedback_temperature(f: &Fields, fuel_fraction: f64) -> Vec<f64> {
    f.t_pebble_avg_k
        .iter()
        .zip(&f.t_helium_k)
        .map(|(tp, th)| fuel_fraction * tp + (1.0 - fuel_fraction) * th)
        .collect()
}

/// Run the coupled case with the power from diffusion: Picard iteration
/// between [`Neutronics`] and the porous-core march, through [`Transfer`].
/// `each` sees every iteration; `started` the set-up notes.
///
/// # Errors
///
/// A set-up, neutronics or march failure.
pub fn solve(
    setup: &MultiphysicsSetup,
    inputs: &MultiphysicsInputs,
    boundary: NeutronBoundary,
    stop: impl Fn() -> bool,
    mut started: impl FnMut(&[String]),
    mut each: impl FnMut(&IterationReport, &Fields),
) -> Result<(Summary, Fields, SpatialFields), String> {
    let mut core = PorousCore::new(setup)?;
    let mut notes = vec![format!("neutronics boundary: {}", boundary.label())];
    let mut neut = Neutronics::new(inputs, boundary)?;
    let tr = Transfer::new(inputs, &core, &mut notes)?;
    started(&notes);
    let p_w = setup.neutronics.thermal_power_mw * 1e6;
    let t_in = setup.foam.inlet_temperature_c + 273.15;
    let c = &setup.coupling;
    let w = c.power_relaxation.clamp(0.05, 1.0);
    let mut t_n = vec![t_in; neut.mesh().n_cells];
    // Start from the march's uniform power, so even the first hand-over is
    // relaxed (the cold-state shape is the most peaked one the loop sees).
    let mut p_prev: Option<Vec<f64>> = Some(core.node_power_w().to_vec());
    let mut k_prev = f64::NAN;
    let mut converged = false;
    let mut q_n = Vec::new();
    for _ in 0..c.max_iterations.max(1) {
        let sol = neut.solve(&t_n, p_w)?;
        let (raw, split, bed) = tr.power_to_nodes_raw(inputs, &sol.q_w_m3, p_w)?;
        let scale = p_w / bed;
        let new: Vec<f64> = raw.iter().map(|p| p * scale).collect();
        let (p_nodes, power_change) = match &p_prev {
            None => (new, f64::INFINITY),
            Some(prev) => {
                let pmax = new.iter().copied().fold(0.0, f64::max);
                let change = new
                    .iter()
                    .zip(prev)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0, f64::max)
                    / pmax;
                (
                    prev.iter()
                        .zip(&new)
                        .map(|(b, a)| (1.0 - w) * b + w * a)
                        .collect(),
                    change,
                )
            }
        };
        core.set_node_power(&p_nodes);
        p_prev = Some(p_nodes);
        let mut r = core.iterate()?;
        let t_node = node_feedback_temperature(core.fields(), setup.foam.fuel_pebble_fraction);
        t_n = tr.temperatures_to_neutronics(inputs, &t_node, t_in)?;
        let k_change = (sol.k - k_prev).abs();
        k_prev = sol.k;
        r.spatial = Some(SpatialReport {
            k_eff: sol.k,
            k_change,
            power_change,
            outer_iterations: sol.outer_iterations,
            split,
            extrapolated_cells: sol.extrapolated_cells,
            neutronics_s: sol.seconds,
        });
        q_n = sol.q_w_m3;
        each(&r, core.fields());
        if core.converged(&r) && power_change < c.power_tolerance && k_change < c.k_tolerance {
            converged = true;
            break;
        }
        if stop() {
            break;
        }
    }
    let mut sum = core.summary(converged)?;
    sum.k_eff = Some(k_prev);
    Ok((
        sum,
        core.fields().clone(),
        SpatialFields {
            q_n_w_m3: q_n,
            t_n_k: t_n,
        },
    ))
}

/// One region's share of the neutron balance.
#[derive(Clone, Debug)]
pub struct RegionBalance {
    /// Region id.
    pub region: String,
    /// Share of the system's production.
    pub production: f64,
    /// Share of the system's absorption.
    pub absorption: f64,
    /// Volume-integrated flux per group, scaled so the system's production
    /// equals the Monte Carlo's (per source neutron) for comparison.
    pub flux: Vec<f64>,
}

/// The isothermal comparison at one state point.
#[derive(Clone, Debug)]
pub struct IsothermalK {
    /// State temperature \[K\].
    pub t_k: f64,
    /// Diffusion eigenvalue.
    pub k_diffusion: f64,
    /// Step 8's Monte Carlo k and its σ.
    pub k_mc: f64,
    /// σ of `k_mc`.
    pub k_mc_sigma: f64,
    /// Diffusion: production / absorption over the whole mesh (no leakage
    /// term), and leakage through the outer boundary as a fraction of
    /// production / k.
    pub kinf_system_diffusion: f64,
    /// Monte Carlo: production / absorption over all regions, from Step 8's
    /// tallied fluxes and constants.
    pub kinf_system_mc: f64,
    /// Diffusion region balance.
    pub diffusion: Vec<RegionBalance>,
    /// Monte Carlo region balance (from the tallies).
    pub mc: Vec<RegionBalance>,
}

/// The diffusion eigenvalue with every neutronics cell at one temperature,
/// for each Step 8 state point, against the Monte Carlo `k` and region
/// balance at the same state. The code-to-code comparison of the record.
///
/// # Errors
///
/// As [`Neutronics::solve`].
pub fn isothermal_k(
    inputs: &MultiphysicsInputs,
    boundary: NeutronBoundary,
) -> Result<Vec<IsothermalK>, String> {
    let mut neut = Neutronics::new(inputs, boundary)?;
    let n = neut.mesh().n_cells;
    let nm = inputs
        .meshes
        .mesh(MeshRole::Neutronics)
        .ok_or("no neutronics mesh")?;
    let regions = &inputs.meshes.plan.regions.regions;
    let ng = inputs.mgxs.n_groups();
    let vols = neut.mesh().cell_volumes.clone();
    let mut out = Vec::new();
    for st in &inputs.mgxs.states {
        neut.reset();
        let sol = neut.solve(&vec![st.temperature_k; n], 1.0)?;
        // Monte Carlo balance from the tallies.
        let mut mc: Vec<RegionBalance> = st
            .regions
            .iter()
            .map(|r| RegionBalance {
                region: r.region.clone(),
                production: (0..ng).map(|g| r.nu_fission[g] * r.flux[g]).sum(),
                absorption: (0..ng).map(|g| r.absorption[g] * r.flux[g]).sum(),
                flux: r.flux.clone(),
            })
            .collect();
        let p_mc: f64 = mc.iter().map(|r| r.production).sum();
        let a_mc: f64 = mc.iter().map(|r| r.absorption).sum();
        // Diffusion balance per region. Flux in 1/m² per m³ cell → ∫φ dV in
        // m⁻²·m³ = m; the MC flux is a track length per source neutron in cm.
        let mut dif: Vec<RegionBalance> = mc
            .iter()
            .map(|r| RegionBalance {
                region: r.region.clone(),
                production: 0.0,
                absorption: 0.0,
                flux: vec![0.0; ng],
            })
            .collect();
        for c in 0..n {
            let id = &regions[nm.cell_region[c]].id;
            let Some(b) = dif.iter_mut().find(|b| &b.region == id) else {
                continue;
            };
            b.production += sol.production[c] * vols[c];
            b.absorption += sol.absorption[c] * vols[c];
            for g in 0..ng {
                b.flux[g] += sol.flux[g][c] * vols[c];
            }
        }
        let p_d: f64 = dif.iter().map(|r| r.production).sum();
        let a_d: f64 = dif.iter().map(|r| r.absorption).sum();
        // Scale the diffusion flux so its production (1/m per unit flux
        // amplitude × m) matches the MC production per source neutron.
        let scale = p_mc / p_d;
        for b in &mut dif {
            for f in &mut b.flux {
                *f *= scale;
            }
        }
        for b in dif.iter_mut() {
            b.production /= p_d;
            b.absorption /= a_d;
        }
        for b in mc.iter_mut() {
            b.production /= p_mc;
            b.absorption /= a_mc;
        }
        out.push(IsothermalK {
            t_k: st.temperature_k,
            k_diffusion: sol.k,
            k_mc: st.k,
            k_mc_sigma: st.k_sigma,
            kinf_system_diffusion: p_d / a_d,
            kinf_system_mc: p_mc / a_mc,
            diffusion: dif,
            mc,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use dhoby_ghaut::workbench::meshes::MeshRole;
    use dhoby_ghaut::workbench::mgxs::{InterpLaw, MgxsSet, RegionXs, StateXs};
    use dhoby_ghaut::workbench::multiphysics::MultiphysicsInputs;
    use nee_soon::mgxs::{GroupStructure, MgxsLibrary, ZoneMgxs};

    use super::*;

    /// Step 7 at a coarse size (neutronics 40 cm, TH 25 cm, structural
    /// 60 cm), built once for every test here.
    fn meshes() -> &'static (crate::meshing::BuiltMeshes, std::path::PathBuf) {
        static M: OnceLock<(crate::meshing::BuiltMeshes, std::path::PathBuf)> = OnceLock::new();
        M.get_or_init(|| {
            let r = crate::preset::htr10();
            let mut engine = crate::engine::Engine::default();
            let a = crate::headless::assemble_for_test(&mut engine, &r).expect("assembly");
            let d = crate::meshing::domain_from(&a);
            let mut plan = dhoby_ghaut::workbench::meshes::MeshPlan::default_for(&d);
            plan.roles[0].cell_size_cm = 40.0;
            plan.roles[1].cell_size_cm = 25.0;
            plan.roles[2].cell_size_cm = 60.0;
            let dir = std::env::temp_dir()
                .join(format!("dhoby_ghaut_spatial_test_{}", std::process::id()));
            let b = crate::meshing::build(&d, &plan, &dir, &mut |_| {}).expect("meshes");
            (b, dir)
        })
    }

    /// Two-group constants, group 0 fast, cm⁻¹, with `Σ_t = Σ_a + Σ_s row`
    /// (as Step 8's `rebalanced`).
    #[derive(Clone, Copy)]
    struct Two {
        sa: [f64; 2],
        nf: [f64; 2],
        s01: f64,
        s00: f64,
        s11: f64,
    }

    impl Two {
        fn total(&self) -> [f64; 2] {
            [self.sa[0] + self.s00 + self.s01, self.sa[1] + self.s11]
        }
        fn zone(&self, name: &str) -> ZoneMgxs {
            let t = self.total();
            ZoneMgxs {
                name: name.into(),
                flux: vec![1.0, 1.0],
                total: t.to_vec(),
                absorption: self.sa.to_vec(),
                nu_fission: self.nf.to_vec(),
                kappa_fission: self.nf.iter().map(|v| v / 2.43 * 3.2e-11).collect(),
                scatter: vec![vec![self.s00, self.s01], vec![0.0, self.s11]],
                chi: if self.nf[1] > 0.0 {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 0.0]
                },
            }
        }
        fn region(&self, name: &str) -> RegionXs {
            let z = self.zone(name);
            RegionXs {
                region: name.into(),
                volume_cm3: 1.0,
                flux: z.flux.clone(),
                flux_rel_sigma: vec![0.0; 2],
                total: z.total.clone(),
                absorption: z.absorption.clone(),
                nu_fission: z.nu_fission.clone(),
                kappa_fission: z.kappa_fission.clone(),
                chi: z.chi.clone(),
                scatter: z.scatter.clone(),
                rel_sigma_total: vec![0.0; 2],
                rel_sigma_absorption: vec![0.0; 2],
                rel_sigma_nu_fission: vec![0.0; 2],
            }
        }
    }

    /// Write a 2-group `nuclearData` with constants `xs(region, T)` for every
    /// region on the neutronics mesh, at the given temperatures, through the
    /// same writer Step 8 uses; return the hand-off.
    fn inputs(tag: &str, temps: &[f64], xs: impl Fn(&str, f64) -> Two) -> MultiphysicsInputs {
        let (b, dir) = meshes();
        let set = &b.set;
        let nm = set.mesh(MeshRole::Neutronics).unwrap();
        let ids: Vec<String> = set
            .plan
            .regions
            .regions
            .iter()
            .enumerate()
            .filter(|(g, _)| nm.cell_region.contains(g))
            .map(|(_, r)| r.id.clone())
            .collect();
        let groups = GroupStructure::new(vec![1e-5, 0.625, 2e7]).unwrap();
        let mut nds = Vec::new();
        let mut states = Vec::new();
        for &t in temps {
            let lib = MgxsLibrary {
                groups: groups.clone(),
                zones: ids.iter().map(|id| xs(id, t).zone(id)).collect(),
            };
            nds.push((t, nee_soon::genfoam_xs::to_nuclear_data_input(&lib).unwrap()));
            states.push(StateXs {
                temperature_k: t,
                k: 1.0,
                k_sigma: 0.0,
                histories: 0,
                data_s: 0.0,
                transport_s: 0.0,
                regions: ids.iter().map(|id| xs(id, t).region(id)).collect(),
            });
        }
        let nd = crate::mgxs_run::stack_states(&nds, temps[0], InterpLaw::LnT);
        let case = dir.join(tag);
        let path = crate::mgxs_run::write_nuclear_data_checked(&nd, "test", &case).unwrap();
        MultiphysicsInputs {
            meshes: set.clone(),
            mgxs: MgxsSet {
                edges_ev_desc: vec![2e7, 0.625, 1e-5],
                law: InterpLaw::LnT,
                states,
                nuclear_data_path: Some(path.display().to_string()),
                notes: vec![],
            },
        }
    }

    /// **Verification, neutronics on the Step 7 mesh** (the analytic check
    /// gh:#591 asked for before the coupled run). One homogeneous 2-group
    /// medium fills the whole neutronics mesh (every region the same), zero
    /// flux on the boundary; the eigenvalue must match the bare cylinder
    /// `k = [νΣf1 (Σa2 + D2 B²) + νΣf2 Σ12] / [(Σr1 + D1 B²)(Σa2 + D2 B²)]`,
    /// `B² = (2.405/R)² + (π/H)²`, `R` the radius of the circle with the
    /// inscribed 32-gon's area. It goes through the whole hand-off: polyMesh,
    /// cellZones, and a `nuclearData` written by Step 8's writer.
    ///
    /// Graphite-like constants with long diffusion lengths (`L1 ≈ 18 cm`,
    /// `L2 ≈ 47 cm`) so the 40 cm cells resolve the flux. Tolerance 1 %;
    /// the measured deviation is printed (recorded in the Step 10 V&V record).
    #[test]
    fn bare_cylinder_k_on_the_neutronics_mesh_matches_the_analytic_value() {
        let m = Two {
            sa: [0.0002, 0.0004],
            nf: [0.0, 0.00065],
            s01: 0.0028,
            s00: 0.30,
            s11: 0.38,
        };
        let inp = inputs("bare", &[300.0], |_, _| m);
        let d = &inp.meshes.domain;
        let mut n = Neutronics::new(&inp, NeutronBoundary::ZeroFlux).expect("neutronics");
        let sol = n.solve(&vec![300.0; n.mesh().n_cells], 1.0).expect("solve");
        let t = m.total();
        let (d1, d2) = (1.0 / (3.0 * t[0]), 1.0 / (3.0 * t[1]));
        let poly = 32.0 / 2.0 * (2.0 * std::f64::consts::PI / 32.0).sin() / std::f64::consts::PI;
        let r = d.r_outer * poly.sqrt();
        let h = d.z_top - d.z_bottom;
        let b2 = (2.404_825_557_7 / r).powi(2) + (std::f64::consts::PI / h).powi(2);
        let sr1 = m.sa[0] + m.s01;
        let k = (m.nf[0] * (m.sa[1] + d2 * b2) + m.nf[1] * m.s01)
            / ((sr1 + d1 * b2) * (m.sa[1] + d2 * b2));
        let rel = sol.k / k - 1.0;
        eprintln!(
            "bare cylinder R {r:.2} cm H {h:.1} cm: diffusion k {:.5}, analytic {k:.5}, {:+.0} pcm, {} outers",
            sol.k,
            rel * 1e5,
            sol.outer_iterations
        );
        assert!(rel.abs() < 0.01, "k {} vs analytic {k}", sol.k);
        let p: f64 = sol
            .q_w_m3
            .iter()
            .zip(&n.mesh().cell_volumes)
            .map(|(q, v)| q * v)
            .sum();
        assert!((p - 1.0).abs() < 1e-9);
    }

    /// **Verification, the coupled loop.** On the coarse Step 7 meshes with
    /// synthetic constants (a fissile bed whose thermal νΣ_f falls with
    /// ln T, graphite elsewhere) at 3 MW:
    ///
    /// - **power is conserved across the mapping**: the neutronics power sums
    ///   to the thermal power; what the neutronics → TH map puts on the TH
    ///   mesh equals `Σ_s q_s ov_s`, with `ov_s` each neutronics cell's
    ///   overlap with the TH mesh recovered from the weights (an independent
    ///   path through the same data), and no cell deposits more than its
    ///   volume; the nodes receive exactly the bed cells' power (1e-12); the
    ///   march carries the thermal power away (energy balance 1e-6);
    /// - **the loop converges** (march residuals, node power 1e-4, k 1e-6)
    ///   within Step 9's iteration limit, and the hot `k` is below the first
    ///   (cold) one: negative feedback through the constants.
    #[test]
    fn the_coupled_loop_conserves_power_across_the_maps_and_converges() {
        let bed_cold = Two {
            sa: [0.00045, 0.0047],
            nf: [0.00034, 0.0088],
            s01: 0.0030,
            s00: 0.357,
            s11: 0.406,
        };
        let graphite = Two {
            sa: [0.00005, 0.0006],
            nf: [0.0, 0.0],
            s01: 0.0030,
            s00: 0.30,
            s11: 0.38,
        };
        let inp = inputs("coupled", &[300.0, 1200.0], |id, t| {
            if id == "pebble_bed" {
                let mut b = bed_cold;
                b.nf[1] *= 1.0 - 0.12 * (t / 300.0).ln();
                b
            } else {
                graphite
            }
        });
        let r = crate::preset::htr10();
        let mut setup = crate::mp_preset::htr10(300.15);
        setup.foam.radial_rings = 6;
        setup.foam.axial_nodes = 30;
        setup.neutronics.thermal_power_mw = 3.0;
        let (setup, _) = crate::mp_preset::on_built_core(setup, &inp.meshes.domain, &r);
        let p_w = 3.0e6;
        // The maps, piece by piece.
        let mut neut = Neutronics::new(&inp, NeutronBoundary::MarshakFace).unwrap();
        let core = crate::porous_core::PorousCore::new(&setup).unwrap();
        let mut notes = Vec::new();
        let tr = Transfer::new(&inp, &core, &mut notes).unwrap();
        let sol = neut.solve(&vec![500.0; neut.mesh().n_cells], p_w).unwrap();
        let vn = neut.mesh().cell_volumes.clone();
        let p_n: f64 = sol.q_w_m3.iter().zip(&vn).map(|(q, v)| q * v).sum();
        assert!((p_n / p_w - 1.0).abs() < 1e-12);
        let (raw, split, bed) = tr.power_to_nodes_raw(&inp, &sol.q_w_m3, p_w).unwrap();
        let map = inp
            .meshes
            .mapping(MeshRole::Neutronics, MeshRole::ThermalHydraulics)
            .unwrap();
        let mut ov = vec![0.0; vn.len()];
        for (t, row) in map.weights.iter().enumerate() {
            for &(s, w) in row {
                ov[s] += w * tr.th_vol_m3[t];
            }
        }
        for (s, (&o, &v)) in ov.iter().zip(&vn).enumerate() {
            assert!(o <= v * (1.0 + 1e-3), "neutronics cell {s} deposits {o} m3 > its {v} m3");
        }
        let deposited: f64 = sol.q_w_m3.iter().zip(&ov).map(|(q, o)| q * o).sum();
        let on_th = (split.bed + split.cavity) * p_w;
        assert!(
            (on_th - deposited).abs() < 1e-9 * p_w,
            "TH receives {on_th} W, neutronics deposits {deposited} W"
        );
        let nodes: f64 = raw.iter().sum();
        assert!((nodes - bed).abs() < 1e-12 * p_w, "nodes {nodes} vs bed cells {bed}");
        eprintln!(
            "power split bed {:.4} cavity {:.4} outside {:.4}; notes {notes:?}",
            split.bed, split.cavity, split.outside
        );
        assert!(split.bed > 0.8, "{split:?}");
        // The loop.
        let mut k_hist = Vec::new();
        let (sum, _, sf) = solve(
            &setup,
            &inp,
            NeutronBoundary::MarshakFace,
            || false,
            |_| {},
            |r, _| k_hist.push(r.spatial.as_ref().map_or(f64::NAN, |s| s.k_eff)),
        )
        .expect("coupled run");
        eprintln!(
            "converged {} in {} iterations: k {:.5} (first {:.5}), peak kernel {:.1} C, node peak/mean {:.3}, energy balance {:.1e}",
            sum.converged,
            sum.iterations,
            sum.k_eff.unwrap_or(f64::NAN),
            k_hist[0],
            sum.max_kernel_c,
            sum.node_peak_to_mean,
            sum.energy_balance_rel
        );
        assert!(sum.converged, "did not converge in {} iterations", sum.iterations);
        assert!(sum.energy_balance_rel.abs() < 1e-6);
        assert!(sum.k_eff.unwrap() < k_hist[0], "hot k should be below the first (cold) k");
        assert!(sf.t_n_k.iter().all(|t| t.is_finite() && *t > 500.0));
    }
}
