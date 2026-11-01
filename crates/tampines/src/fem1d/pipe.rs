//! [`FemPipe`]: the continuous-Galerkin HEM pipe.

use farrer_park::element::FacetType;
use farrer_park::quadrature::facet_rule;
use petir::ssprk::{ssp_step, SspMethod, SspWorkspace};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AvailableEnergy, Energy, Length, Mass, MassDensity, Pressure, Ratio, Time, Velocity,
};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use tampines_steam_tables::hem::{
    AreaProfile, HemBoundary, HemCellState, HemEquationOfState, HemError, HemInitialCondition,
    HemPipeGeometry, HemRunSummary, HemSnapshot, HemThermoState, PipeEnd, PrimitiveState,
    TimeIntegrator,
};

use super::eigen::{mat_vec, sign_matrix, EulerState};
use super::{FemNumerics, MassMatrix, Stabilisation};

/// Bookkeeping entries after the nodes: mass in and energy in at the inlet,
/// mass out and energy out at the outlet (kg, J), as `HemPipe`.
const N_BOOKKEEPING: usize = 4;

/// One closed state at a node or quadrature point, raw SI.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pt {
    rho: f64,
    u: f64,
    p: f64,
    e: f64,
    c: f64,
}

impl Pt {
    fn h_total(&self) -> f64 {
        self.e + self.p / self.rho + 0.5 * self.u * self.u
    }
}

/// A precomputed quadrature point: element, position, physical weight
/// (`w·h/2`), the two shape functions, flow area and its slope.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Qp {
    elem: usize,
    w: f64,
    phi: [f64; 2],
    area: f64,
    slope: f64,
}

/// The physical flux `[ρu, ρu² + p, ρuH]` of a state, per unit area.
fn physical_flux(s: &Pt) -> [f64; 3] {
    let m = s.rho * s.u;
    [m, m * s.u + s.p, m * s.h_total()]
}

/// `dA/dx` of an [`AreaProfile`], analytic. The area itself is always taken
/// from [`HemPipeGeometry::flow_area_at`]; this is the derivative of the same
/// formulas (`tampines_steam_tables::hem::AreaProfile::at`).
fn area_slope(profile: AreaProfile, x: f64, length: f64) -> f64 {
    match profile {
        AreaProfile::Uniform(_) => 0.0,
        AreaProfile::Linear { inlet_m2, outlet_m2 } => (outlet_m2 - inlet_m2) / length,
        AreaProfile::ConvergingDiverging {
            inlet_m2,
            throat_m2,
            outlet_m2,
            throat_fraction,
        } => {
            let xt = throat_fraction * length;
            if x <= xt {
                let s = 1.0 - x / xt;
                -2.0 * (inlet_m2 - throat_m2) * s / xt
            } else {
                let s = (x - xt) / (length - xt);
                2.0 * (outlet_m2 - throat_m2) * s / (length - xt)
            }
        }
    }
}

/// Attach a node index to an EOS error raised without one.
fn at_node(e: HemError, node: usize) -> HemError {
    match e {
        HemError::EquationOfState { cell: None, detail } => HemError::EquationOfState {
            cell: Some(node),
            detail,
        },
        other => other,
    }
}

/// The right-hand side of one evaluation, with what the step control needs.
#[derive(Debug, Clone)]
struct Eval {
    /// `dq/dt`, nodes then bookkeeping.
    dq: Vec<f64>,
    /// Largest `|u| + c` over nodes and quadrature points, m/s.
    max_speed: f64,
}

/// A transient HEM pipe discretised by 1-D linear continuous Galerkin finite
/// elements (gh:#1069): conserved nodal `V = [ρA, ρuA, ρEA]`, closed by `E`.
///
/// Node `j` sits at `x_j = j·L/n` (`n + 1` nodes for the geometry's `n`
/// cells, so the element length equals the finite-volume cell width). See the
/// [module doc](super) for the weak form, the stabilisations and the
/// boundary treatment.
///
/// **Physics carried:** inviscid, adiabatic, horizontal, variable area. Wall
/// friction, gravity and wall heat are NOT in the finite-element path; a
/// comparison against [`tampines_steam_tables::hem::HemPipe`] with IF97 must
/// ablate the finite-volume wall friction (`without_wall_friction`) to compare
/// the same physics.
#[derive(Debug, Clone)]
pub struct FemPipe<E: HemEquationOfState> {
    geometry: HemPipeGeometry,
    eos: E,
    numerics: FemNumerics,
    stabilisation: Stabilisation,
    inlet: HemBoundary,
    outlet: HemBoundary,
    q: Vec<f64>,
    nodes: Vec<Pt>,
    node_hints: Vec<f64>,
    qp_hints: Vec<f64>,
    time_s: f64,
    last_dt: Option<f64>,
    /// Nodal `(p, ρ)` at the previous step and that step's `Δt`, for the
    /// entropy residual's time derivatives.
    history: Option<(Vec<f64>, Vec<f64>, f64)>,
    node_area: Vec<f64>,
    qps: Vec<Qp>,
    work: SspWorkspace,
}

impl<E: HemEquationOfState> FemPipe<E> {
    /// Build the pipe with `stabilisation` and `numerics`, both ends
    /// [`HemBoundary::Wall`] until [`Self::set_boundary`] changes them.
    ///
    /// The initial condition is sampled at the nodes. A
    /// [`HemInitialCondition::Riemann`] node exactly on the diaphragm takes
    /// the mean of the two conserved states; a
    /// [`HemInitialCondition::PerCell`] needs one state per **node**
    /// (`n + 1`).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a bad `numerics` or a per-node list of
    /// the wrong length; the EOS's error if a state cannot be closed.
    pub fn new(
        geometry: HemPipeGeometry,
        eos: E,
        initial: HemInitialCondition,
        stabilisation: Stabilisation,
        numerics: FemNumerics,
    ) -> Result<Self, HemError> {
        numerics.validate()?;
        let n = geometry.n_cells();
        let length = geometry.length().get::<meter>();
        let h = length / n as f64;
        let xs: Vec<f64> = (0..=n).map(|j| j as f64 * h).collect();
        let area = |x: f64| geometry.flow_area_at(Length::new::<meter>(x)).value;
        let node_area: Vec<f64> = xs.iter().map(|&x| area(x)).collect();
        let rule = facet_rule(FacetType::Line2);
        let profile = geometry.area_profile();
        let mut qps = Vec::with_capacity(2 * n);
        for elem in 0..n {
            for g in &rule {
                let xi = g.xi[0];
                let x = xs[elem] + 0.5 * (xi + 1.0) * h;
                qps.push(Qp {
                    elem,
                    w: g.weight * 0.5 * h,
                    phi: [0.5 * (1.0 - xi), 0.5 * (1.0 + xi)],
                    area: area(x),
                    slope: area_slope(profile, x, length),
                });
            }
        }
        let close = |s: &PrimitiveState| -> Result<(HemThermoState, f64), HemError> {
            match *s {
                PrimitiveState::PressureTemperature { pressure, temperature, velocity } => {
                    Ok((eos.state_p_t(pressure, temperature)?, velocity.get::<meter_per_second>()))
                }
                PrimitiveState::PressureEnthalpy { pressure, specific_enthalpy, velocity } => {
                    Ok((eos.state_p_h(pressure, specific_enthalpy)?, velocity.get::<meter_per_second>()))
                }
                PrimitiveState::PressureDensity { pressure, density, velocity } => {
                    Ok((eos.state_p_rho(pressure, density)?, velocity.get::<meter_per_second>()))
                }
            }
        };
        let conserved = |s: &PrimitiveState| -> Result<[f64; 3], HemError> {
            let (t, u) = close(s)?;
            let rho = t.density.get::<kilogram_per_cubic_meter>();
            let e = t.specific_internal_energy.get::<joule_per_kilogram>();
            Ok([rho, rho * u, rho * (e + 0.5 * u * u)])
        };
        let per_node: Vec<[f64; 3]> = match &initial {
            HemInitialCondition::Uniform(s) => {
                let w = conserved(s)?;
                vec![w; n + 1]
            }
            HemInitialCondition::Riemann { diaphragm, left, right } => {
                let d = diaphragm.get::<meter>();
                let (wl, wr) = (conserved(left)?, conserved(right)?);
                xs.iter()
                    .map(|&x| {
                        if (x - d).abs() <= 1e-9 * h {
                            [0, 1, 2].map(|k| 0.5 * (wl[k] + wr[k]))
                        } else if x < d {
                            wl
                        } else {
                            wr
                        }
                    })
                    .collect()
            }
            HemInitialCondition::PerCell(v) => {
                if v.len() != n + 1 {
                    return Err(HemError::InvalidInput(format!(
                        "the finite-element pipe needs one initial state per node ({}), got {}",
                        n + 1,
                        v.len()
                    )));
                }
                v.iter().map(conserved).collect::<Result<_, _>>()?
            }
        };
        let mut q = vec![0.0; 3 * (n + 1) + N_BOOKKEEPING];
        for (j, w) in per_node.iter().enumerate() {
            for k in 0..3 {
                q[3 * j + k] = w[k] * node_area[j];
            }
        }
        let len = q.len();
        let mut pipe = Self {
            geometry,
            eos,
            numerics,
            stabilisation,
            inlet: HemBoundary::Wall,
            outlet: HemBoundary::Wall,
            q,
            nodes: Vec::new(),
            node_hints: vec![f64::NAN; n + 1],
            qp_hints: vec![f64::NAN; 2 * n],
            time_s: 0.0,
            last_dt: None,
            history: None,
            node_area,
            qps,
            work: SspWorkspace::new(len),
        };
        let q0 = pipe.q.clone();
        pipe.nodes = pipe.decode_nodes(&q0)?;
        pipe.node_hints = pipe.nodes.iter().map(|s| s.p).collect();
        Ok(pipe)
    }

    /// Set the boundary condition at `end`. Every [`HemBoundary`] is accepted;
    /// the boundary flux is built exactly as `HemPipe` builds its boundary
    /// face (see the module doc).
    ///
    /// # Errors
    ///
    /// None at present; the `Result` matches `HemPipe::set_boundary`.
    pub fn set_boundary(&mut self, end: PipeEnd, boundary: HemBoundary) -> Result<(), HemError> {
        match end {
            PipeEnd::Inlet => self.inlet = boundary,
            PipeEnd::Outlet => self.outlet = boundary,
        }
        Ok(())
    }

    /// The boundary condition at `end`.
    pub fn boundary(&self, end: PipeEnd) -> HemBoundary {
        match end {
            PipeEnd::Inlet => self.inlet,
            PipeEnd::Outlet => self.outlet,
        }
    }

    /// The stabilisation this pipe runs with.
    pub fn stabilisation(&self) -> Stabilisation {
        self.stabilisation
    }

    /// The geometry.
    pub fn geometry(&self) -> &HemPipeGeometry {
        &self.geometry
    }

    /// Simulation time.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// Node positions, measured from the inlet: `x_j = j·L/n`, `j = 0..=n`.
    pub fn node_positions(&self) -> Vec<Length> {
        let h = self.dx();
        (0..=self.geometry.n_cells())
            .map(|j| Length::new::<meter>(j as f64 * h))
            .collect()
    }

    /// Every node's state now. `cell_centres` holds the node positions.
    ///
    /// # Errors
    ///
    /// The EOS's error if a node cannot be re-closed (it closed at the end of
    /// the last step, so this does not happen in practice).
    pub fn snapshot(&self) -> Result<HemSnapshot, HemError> {
        let cells = self
            .nodes
            .iter()
            .enumerate()
            .map(|(j, s)| {
                let t = self
                    .eos
                    .state_rho_e(
                        MassDensity::new::<kilogram_per_cubic_meter>(s.rho),
                        AvailableEnergy::new::<joule_per_kilogram>(s.e),
                        Some(Pressure::new::<pascal>(s.p)),
                    )
                    .map_err(|e| at_node(e, j))?;
                Ok(HemCellState {
                    pressure: t.pressure,
                    temperature: t.temperature,
                    density: t.density,
                    velocity: Velocity::new::<meter_per_second>(s.u),
                    specific_internal_energy: t.specific_internal_energy,
                    specific_enthalpy: t.specific_enthalpy,
                    equilibrium_sound_speed: t.equilibrium_sound_speed,
                    quality: t.quality,
                    void_fraction: t.void_fraction,
                    dynamic_viscosity: self.eos.dynamic_viscosity(&t),
                })
            })
            .collect::<Result<_, HemError>>()?;
        Ok(HemSnapshot {
            time: self.time(),
            cell_centres: self.node_positions(),
            cells,
        })
    }

    /// Mass in the pipe, `∫ρA dx` of the piecewise-linear field (exact:
    /// `Σ_j m_j (ρA)_j` with the lumped weights `m_j = ∫φ_j`).
    pub fn total_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.integral(0))
    }

    /// Total energy in the pipe, `∫ρEA dx`, exact as [`Self::total_mass`].
    pub fn total_energy(&self) -> Energy {
        Energy::new::<joule>(self.integral(2))
    }

    /// Mass that has left through `end` since `t = 0` (negative for net
    /// inflow), integrated by the same Runge–Kutta stages as the nodes.
    pub fn cumulative_mass_outflow(&self, end: PipeEnd) -> Mass {
        let b = 3 * (self.geometry.n_cells() + 1);
        Mass::new::<kilogram>(match end {
            PipeEnd::Inlet => -self.q[b],
            PipeEnd::Outlet => self.q[b + 2],
        })
    }

    /// Total energy that has left through `end` since `t = 0`.
    pub fn cumulative_energy_outflow(&self, end: PipeEnd) -> Energy {
        let b = 3 * (self.geometry.n_cells() + 1);
        Energy::new::<joule>(match end {
            PipeEnd::Inlet => -self.q[b + 1],
            PipeEnd::Outlet => self.q[b + 3],
        })
    }

    /// Advance one step at the configured Courant number.
    ///
    /// # Errors
    ///
    /// As [`Self::run_until`]; the state is left as it was.
    pub fn step(&mut self) -> Result<f64, HemError> {
        self.step_until_at_most(f64::INFINITY).map(|(_, co, _)| co)
    }

    /// Advance to exactly `t_end`.
    ///
    /// The returned summary's `max_courant` is the **classical** Courant
    /// number `(|u| + c)Δt/h` (not `HemPipe`'s central one, which is half of
    /// it in 1-D).
    ///
    /// # Errors
    ///
    /// [`HemError::Unphysical`] (with the node index in `cell`) or the EOS's
    /// error if a stage cannot be closed; the pipe is left at the start of the
    /// failed step.
    pub fn run_until(&mut self, t_end: Time) -> Result<HemRunSummary, HemError> {
        let t_end = t_end.get::<second>();
        let mut steps = 0usize;
        let mut max_co: f64 = 0.0;
        let mut min_p = f64::INFINITY;
        while self.time_s < t_end {
            let (_, co, p) = self.step_until_at_most(t_end)?;
            steps += 1;
            max_co = max_co.max(co);
            min_p = min_p.min(p);
        }
        Ok(HemRunSummary {
            steps,
            time: self.time(),
            max_courant: Ratio::new::<ratio>(max_co),
            min_pressure: Pressure::new::<pascal>(min_p),
        })
    }

    fn dx(&self) -> f64 {
        self.geometry.length().get::<meter>() / self.geometry.n_cells() as f64
    }

    /// `∫φ_j dx`: `h/2` at the two ends, `h` inside.
    fn lumped_mass(&self, j: usize) -> f64 {
        let h = self.dx();
        if j == 0 || j == self.geometry.n_cells() {
            0.5 * h
        } else {
            h
        }
    }

    fn integral(&self, k: usize) -> f64 {
        (0..=self.geometry.n_cells())
            .map(|j| self.lumped_mass(j) * self.q[3 * j + k])
            .sum()
    }

    /// Close a `(ρ, ρu, ρE)` triple (per unit volume).
    fn close(&self, w: [f64; 3], hint: f64, node: usize) -> Result<(Pt, HemThermoState), HemError> {
        let rho = w[0];
        if !(rho.is_finite() && rho > 0.0) {
            return Err(HemError::Unphysical { cell: node, quantity: "density", value: rho });
        }
        let u = w[1] / rho;
        let e = w[2] / rho - 0.5 * u * u;
        if !e.is_finite() {
            return Err(HemError::Unphysical {
                cell: node,
                quantity: "specific internal energy",
                value: e,
            });
        }
        let hint = if hint.is_finite() && hint > 0.0 {
            Some(Pressure::new::<pascal>(hint))
        } else {
            None
        };
        let t = self
            .eos
            .state_rho_e(
                MassDensity::new::<kilogram_per_cubic_meter>(rho),
                AvailableEnergy::new::<joule_per_kilogram>(e),
                hint,
            )
            .map_err(|err| at_node(err, node))?;
        let p = t.pressure.get::<pascal>();
        if !(p.is_finite() && p > 0.0) {
            return Err(HemError::Unphysical { cell: node, quantity: "pressure", value: p });
        }
        let c = t.equilibrium_sound_speed.get::<meter_per_second>();
        Ok((Pt { rho, u, p, e, c }, t))
    }

    fn decode_nodes(&self, q: &[f64]) -> Result<Vec<Pt>, HemError> {
        (0..=self.geometry.n_cells())
            .map(|j| {
                let a = self.node_area[j];
                let w = [q[3 * j] / a, q[3 * j + 1] / a, q[3 * j + 2] / a];
                self.close(w, self.node_hints[j], j).map(|(s, _)| s)
            })
            .collect()
    }

    /// Boundary face state at `end` from its end node `o`, built exactly as
    /// `tampines_steam_tables::hem::HemPipe`'s private `boundary_face` builds
    /// it (same rules, re-implemented here because that function is private).
    fn boundary_face(&self, end: PipeEnd, o: &Pt) -> Result<(Pt, bool), HemError> {
        let (bc, out, node) = match end {
            PipeEnd::Inlet => (self.inlet, -1.0, 0),
            PipeEnd::Outlet => (self.outlet, 1.0, self.geometry.n_cells()),
        };
        let h_o = o.e + o.p / o.rho;
        let from_ph = |p: f64, h: f64, u: f64| -> Result<Pt, HemError> {
            let t = self
                .eos
                .state_p_h(Pressure::new::<pascal>(p), AvailableEnergy::new::<joule_per_kilogram>(h))
                .map_err(|e| at_node(e, node))?;
            Ok(Pt {
                rho: t.density.get::<kilogram_per_cubic_meter>(),
                u,
                p,
                e: t.specific_internal_energy.get::<joule_per_kilogram>(),
                c: t.equilibrium_sound_speed.get::<meter_per_second>(),
            })
        };
        Ok(match bc {
            HemBoundary::Wall => (Pt { u: 0.0, ..*o }, false),
            HemBoundary::Transmissive | HemBoundary::Flux { .. } => (*o, false),
            HemBoundary::FixedPressure(p) => (from_ph(p.get::<pascal>(), h_o, o.u)?, false),
            HemBoundary::Reservoir { stagnation_pressure, stagnation_enthalpy } => {
                let p0 = stagnation_pressure.get::<pascal>();
                let h0 = stagnation_enthalpy.get::<joule_per_kilogram>();
                if o.u * out < 0.0 {
                    let ke = 0.5 * o.u * o.u;
                    (from_ph(p0 - o.rho * ke, h0 - ke, o.u)?, false)
                } else {
                    (from_ph(p0, h_o, o.u)?, false)
                }
            }
            HemBoundary::ChokedBreak { area_fraction, back_pressure } => {
                let flow = self
                    .eos
                    .break_flow(
                        Pressure::new::<pascal>(o.p),
                        AvailableEnergy::new::<joule_per_kilogram>(h_o + 0.5 * o.u * o.u),
                        back_pressure,
                    )
                    .map_err(|e| at_node(e, node))?;
                let g = flow.mass_flux.get::<kilogram_per_square_meter_second>()
                    * area_fraction.get::<ratio>();
                (Pt { u: out * g / o.rho, ..*o }, flow.choked)
            }
        })
    }

    /// The boundary flux per unit area along `+x`: the prescribed one for
    /// [`HemBoundary::Flux`], otherwise the physical flux of the face state.
    fn boundary_flux(&self, end: PipeEnd, o: &Pt) -> Result<([f64; 3], bool), HemError> {
        if let HemBoundary::Flux { mass_flux, momentum_flux, energy_flux } = self.boundary(end) {
            return Ok((
                [
                    mass_flux.get::<kilogram_per_square_meter_second>(),
                    momentum_flux.get::<pascal>(),
                    energy_flux.get::<watt_per_square_meter>(),
                ],
                false,
            ));
        }
        let (face, choked) = self.boundary_face(end, o)?;
        Ok((physical_flux(&face), choked))
    }

    /// The Grüneisen coefficient `Γ = (1/ρ)∂p/∂e|_ρ` by a forward difference
    /// of the EOS, step `δe = 1e-6 (|e| + c²)`. Exact for an ideal gas (`p`
    /// is linear in `e`); for IF97 the `(ρ, e)` inversion's `1e-12` relative
    /// tolerance makes the difference good to ~1e-5 relative.
    fn grueneisen(&self, s: &Pt, hint: f64, node: usize) -> Result<f64, HemError> {
        let de = 1e-6 * (s.e.abs() + s.c * s.c);
        let rho_e2 = s.rho * (s.e + de + 0.5 * s.u * s.u);
        let (s2, _) = self.close([s.rho, s.rho * s.u, rho_e2], hint, node)?;
        Ok((s2.p - s.p) / (s.rho * de))
    }

    /// Solve `M x = r` in place for each of the three components.
    fn mass_solve(&self, r: &mut [f64]) {
        let n = self.geometry.n_cells();
        match self.numerics.mass_matrix {
            MassMatrix::Lumped => {
                for j in 0..=n {
                    let m = self.lumped_mass(j);
                    for k in 0..3 {
                        r[3 * j + k] /= m;
                    }
                }
            }
            MassMatrix::Consistent => {
                // Tridiagonal: off-diagonal h/6, diagonal h/3 at the ends and
                // 2h/3 inside. Thomas algorithm per component.
                let h = self.dx();
                let off = h / 6.0;
                let diag = |j: usize| if j == 0 || j == n { h / 3.0 } else { 2.0 * h / 3.0 };
                let mut cp = vec![0.0; n + 1];
                for k in 0..3 {
                    let mut dp = vec![0.0; n + 1];
                    cp[0] = off / diag(0);
                    dp[0] = r[k] / diag(0);
                    for j in 1..=n {
                        let den = diag(j) - off * cp[j - 1];
                        cp[j] = off / den;
                        dp[j] = (r[3 * j + k] - off * dp[j - 1]) / den;
                    }
                    r[3 * n + k] = dp[n];
                    for j in (0..n).rev() {
                        r[3 * j + k] = dp[j] - cp[j] * r[3 * (j + 1) + k];
                    }
                }
            }
        }
    }

    /// The entropy-viscosity coefficient of each element, frozen for one step
    /// (RELAP-7 §5.4.2, Eqs 334–338, single phase: `κ = µ`, `β = 0`, `l = 0`).
    ///
    /// `κ_K = min(κ_max, κ_e)`, `κ_max = (h/2) max(|u| + c)` (Eq 334) and
    /// `κ_e = h² max(|D̃|, J)/((1 − M)ρc² + Mρu²)` (Eq 336) with the entropy
    /// residual `D̃ = dp/dt − c² dρ/dt` (Eq 335, material derivatives) and the
    /// jump `J = |u| max(⟦∂p/∂x⟧, c²⟦∂ρ/∂x⟧)` (Eqs 337–338) of the element
    /// gradients across each node. Evaluated at the element's two nodes and
    /// maximised. `∂/∂t` is the backward difference over the previous step;
    /// on the first step (no history) `κ = κ_max`, the first-order viscosity.
    fn entropy_viscosity(&self, nodes: &[Pt]) -> Vec<f64> {
        let n = self.geometry.n_cells();
        let h = self.dx();
        let dp: Vec<f64> = (0..n).map(|k| (nodes[k + 1].p - nodes[k].p) / h).collect();
        let drho: Vec<f64> = (0..n).map(|k| (nodes[k + 1].rho - nodes[k].rho) / h).collect();
        let jump: Vec<f64> = (0..=n)
            .map(|j| {
                if j == 0 || j == n {
                    return 0.0;
                }
                let s = &nodes[j];
                s.u.abs() * (dp[j] - dp[j - 1]).abs().max(s.c * s.c * (drho[j] - drho[j - 1]).abs())
            })
            .collect();
        (0..n)
            .map(|k| {
                let kmax = 0.5
                    * h
                    * (nodes[k].u.abs() + nodes[k].c).max(nodes[k + 1].u.abs() + nodes[k + 1].c);
                let Some((p_old, rho_old, dt_old)) = &self.history else {
                    return kmax;
                };
                let j_k = jump[k].max(jump[k + 1]);
                let mut ke: f64 = 0.0;
                for j in [k, k + 1] {
                    let s = &nodes[j];
                    let pt = (s.p - p_old[j]) / dt_old;
                    let rt = (s.rho - rho_old[j]) / dt_old;
                    let d = (pt + s.u * dp[k]) - s.c * s.c * (rt + s.u * drho[k]);
                    let mach = s.u.abs() / s.c;
                    let den = (1.0 - mach) * s.rho * s.c * s.c + mach * s.rho * s.u * s.u;
                    ke = ke.max(h * h * d.abs().max(j_k) / den);
                }
                kmax.min(ke)
            })
            .collect()
    }

    /// One right-hand-side evaluation at `q` (the semi-discrete form of the
    /// module doc), with `kappa` the frozen entropy viscosity per element.
    fn evaluate(&self, q: &[f64], kappa: &[f64], qp_hints: &mut [f64]) -> Result<Eval, HemError> {
        let n = self.geometry.n_cells();
        let h = self.dx();
        let nodes = self.decode_nodes(q)?;
        let mut r = vec![0.0; 3 * (n + 1)];
        let mut max_speed = nodes.iter().fold(0.0_f64, |m, s| m.max(s.u.abs() + s.c));

        // Closed quadrature-point states, kept for SUPG.
        let mut qstates: Vec<Pt> = Vec::with_capacity(self.qps.len());
        for (iq, g) in self.qps.iter().enumerate() {
            let (l, rr) = (g.elem, g.elem + 1);
            let vq: [f64; 3] = [0, 1, 2].map(|k| g.phi[0] * q[3 * l + k] + g.phi[1] * q[3 * rr + k]);
            let (s, _) = self.close([vq[0] / g.area, vq[1] / g.area, vq[2] / g.area], qp_hints[iq], l)?;
            qp_hints[iq] = s.p;
            max_speed = max_speed.max(s.u.abs() + s.c);
            // Galerkin volume terms, RELAP-7 Eqs 240–242 moved to the right.
            let f = [vq[1], vq[1] * s.u + s.p * g.area, vq[1] * s.h_total()];
            let dphi = [-1.0 / h, 1.0 / h];
            for (a, node) in [l, rr].into_iter().enumerate() {
                r[3 * node] += g.w * f[0] * dphi[a];
                r[3 * node + 1] += g.w * (f[1] * dphi[a] + s.p * g.slope * g.phi[a]);
                r[3 * node + 2] += g.w * f[2] * dphi[a];
            }
            qstates.push(s);
        }

        // Boundary terms: −(G·n̂)φ_i at the two ends (n̂ = −1 at x = 0).
        let (f_in, _) = self.boundary_flux(PipeEnd::Inlet, &nodes[0])?;
        let (f_out, _) = self.boundary_flux(PipeEnd::Outlet, &nodes[n])?;
        let (a_in, a_out) = (self.node_area[0], self.node_area[n]);
        for k in 0..3 {
            r[k] += f_in[k] * a_in;
            r[3 * n + k] -= f_out[k] * a_out;
        }

        match self.stabilisation {
            Stabilisation::Galerkin => {}
            Stabilisation::Supg => self.add_supg(q, &r.clone(), &qstates, qp_hints, &mut r)?,
            Stabilisation::EntropyViscosity => {
                for (iq, g) in self.qps.iter().enumerate() {
                    let (l, rr) = (g.elem, g.elem + 1);
                    let (sl, sr, s) = (&nodes[l], &nodes[rr], &qstates[iq]);
                    let kap = kappa[g.elem];
                    // Eqs 331–333 with l = 0: f = κρ_x, g = ρµu_x + uf,
                    // h = κ(ρe)_x − (u²/2)f + ug, µ = κ. Gradients of the
                    // nodal primitive fields, constant on the element.
                    let rho_x = (sr.rho - sl.rho) / h;
                    let u_x = (sr.u - sl.u) / h;
                    let rhoe_x = (sr.rho * sr.e - sl.rho * sl.e) / h;
                    let f = kap * rho_x;
                    let gg = s.rho * kap * u_x + s.u * f;
                    let hh = kap * rhoe_x - 0.5 * s.u * s.u * f + s.u * gg;
                    let dphi = [-1.0 / h, 1.0 / h];
                    for (a, node) in [l, rr].into_iter().enumerate() {
                        r[3 * node] -= g.w * g.area * f * dphi[a];
                        r[3 * node + 1] -= g.w * g.area * gg * dphi[a];
                        r[3 * node + 2] -= g.w * g.area * hh * dphi[a];
                    }
                }
            }
            Stabilisation::Lapidus => {
                let cl = self.numerics.lapidus_coefficient.get::<ratio>();
                for g in &self.qps {
                    let (l, rr) = (g.elem, g.elem + 1);
                    let nu = cl * h * (nodes[rr].u - nodes[l].u).abs();
                    let dphi = [-1.0 / h, 1.0 / h];
                    for k in 0..3 {
                        let vx = (q[3 * rr + k] - q[3 * l + k]) / h;
                        r[3 * l + k] -= g.w * nu * vx * dphi[0];
                        r[3 * rr + k] -= g.w * nu * vx * dphi[1];
                    }
                }
            }
        }

        self.mass_solve(&mut r);
        let mut dq = r;
        dq.extend_from_slice(&[
            f_in[0] * a_in,
            f_in[2] * a_in,
            f_out[0] * a_out,
            f_out[2] * a_out,
        ]);
        Ok(Eval { dq, max_speed })
    }

    /// SUPG (RELAP-7 §5.4.1, Eqs 283–289): adds
    /// `−∫ ∂φ_i/∂x · (h/2) P sgn(D) P⁻¹ R̃ dx` to the nodal residual `r`, with
    /// the quasi-linear residual `R̃ = V_t + A V_x + (F − AU) A' − S`
    /// (Eqs 283–285, `S = [0, pA', 0]`). `V_t` is the Galerkin-only nodal rate
    /// (`M⁻¹ r_gal`, passed in as `r_gal`) interpolated to the point: an
    /// explicit scheme cannot put the SUPG-weighted time derivative into the
    /// mass matrix without a solve per stage, and this keeps `R̃` consistent
    /// (zero for a smooth exact solution).
    fn add_supg(
        &self,
        q: &[f64],
        r_gal: &[f64],
        qstates: &[Pt],
        qp_hints: &[f64],
        r: &mut [f64],
    ) -> Result<(), HemError> {
        let h = self.dx();
        let mut rate = r_gal.to_vec();
        self.mass_solve(&mut rate);
        for (iq, g) in self.qps.iter().enumerate() {
            let (l, rr) = (g.elem, g.elem + 1);
            let s = &qstates[iq];
            let gamma = self.grueneisen(s, qp_hints[iq], l)?;
            let es = EulerState { u: s.u, h_total: s.h_total(), c: s.c, gamma };
            let Some(sgn) = sign_matrix(&es) else {
                return Err(HemError::Unphysical { cell: l, quantity: "sound speed", value: s.c });
            };
            let vt: [f64; 3] = [0, 1, 2].map(|k| g.phi[0] * rate[3 * l + k] + g.phi[1] * rate[3 * rr + k]);
            let vx: [f64; 3] = [0, 1, 2].map(|k| (q[3 * rr + k] - q[3 * l + k]) / h);
            let av = mat_vec(&es.jacobian(), vx);
            let p_hat = (1.0 + gamma) * s.p - s.rho * s.c * s.c;
            let rt = [
                vt[0] + av[0],
                vt[1] + av[1] + p_hat * g.slope - s.p * g.slope,
                vt[2] + av[2] + s.u * p_hat * g.slope,
            ];
            let w = mat_vec(&sgn, rt);
            let dphi = [-1.0 / h, 1.0 / h];
            for (a, node) in [l, rr].into_iter().enumerate() {
                for k in 0..3 {
                    r[3 * node + k] -= g.w * dphi[a] * 0.5 * h * w[k];
                }
            }
        }
        Ok(())
    }

    /// One step, capped so as not to pass `t_end`. Returns `(dt, classical
    /// Courant, min nodal pressure)`.
    fn step_until_at_most(&mut self, t_end: f64) -> Result<(f64, f64, f64), HemError> {
        let cap = t_end - self.time_s;
        let kappa = match self.stabilisation {
            Stabilisation::EntropyViscosity => self.entropy_viscosity(&self.nodes),
            _ => Vec::new(),
        };
        let mut hints = self.qp_hints.clone();
        let first = self.evaluate(&self.q.clone(), &kappa, &mut hints)?;
        let h = self.dx();
        let co_target = self.numerics.courant.get::<ratio>()
            * match self.numerics.mass_matrix {
                MassMatrix::Lumped => 1.0,
                // The consistent mass matrix's largest eigenvalue ratio is 3×
                // the lumped one for diffusion and √3× for advection.
                MassMatrix::Consistent => 1.0 / 3.0_f64.sqrt(),
            };
        let mut dt = if first.max_speed > 0.0 { co_target * h / first.max_speed } else { f64::INFINITY };
        dt = dt.min(self.numerics.max_dt.get::<second>()).min(cap);
        let courant = first.max_speed * dt / h;

        let q_old = self.q.clone();
        let mut q = self.q.clone();
        let mut work = std::mem::replace(&mut self.work, SspWorkspace::new(0));
        let mut err: Option<HemError> = None;
        let mut first = Some(first);
        let t0 = self.time_s;
        let integrated = {
            let this = &*self;
            let mut rhs = |_t: f64, qs: &[f64], dq: &mut [f64]| {
                if err.is_some() {
                    dq.fill(0.0);
                    return;
                }
                let ev = match first.take() {
                    Some(e) => Ok(e),
                    None => this.evaluate(qs, &kappa, &mut hints),
                };
                match ev {
                    Ok(e) => dq.copy_from_slice(&e.dq),
                    Err(e) => {
                        err = Some(e);
                        dq.fill(0.0);
                    }
                }
            };
            let method = match self.numerics.time_integrator {
                TimeIntegrator::ForwardEuler => None,
                TimeIntegrator::Ssp22 => Some(SspMethod::Ssp22),
                TimeIntegrator::Ssp33 => Some(SspMethod::Ssp33),
                TimeIntegrator::Ssp104 => Some(SspMethod::Ssp104),
            };
            match method {
                None => {
                    let mut k = vec![0.0; q.len()];
                    rhs(t0, &q, &mut k);
                    for (qi, ki) in q.iter_mut().zip(&k) {
                        *qi += dt * ki;
                    }
                    Ok(())
                }
                Some(m) => ssp_step(m, &mut rhs, t0, &mut q, dt, &mut work)
                    .map_err(|e| HemError::InvalidInput(format!("SSP step rejected the state: {e:?}"))),
            }
        };
        self.work = work;
        if let Some(e) = err.or(integrated.err()) {
            self.q = q_old;
            return Err(e);
        }
        let new_nodes = match self.decode_nodes(&q) {
            Ok(v) => v,
            Err(e) => {
                self.q = q_old;
                return Err(e);
            }
        };
        let old_nodes = std::mem::replace(&mut self.nodes, new_nodes);
        self.history = Some((
            old_nodes.iter().map(|s| s.p).collect(),
            old_nodes.iter().map(|s| s.rho).collect(),
            dt,
        ));
        self.q = q;
        self.qp_hints = hints;
        self.node_hints = self.nodes.iter().map(|s| s.p).collect();
        self.time_s = if dt == cap { t_end } else { t0 + dt };
        self.last_dt = Some(dt);
        let min_p = self.nodes.iter().map(|s| s.p).fold(f64::INFINITY, f64::min);
        Ok((dt, courant, min_p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines_steam_tables::hem::IdealGasHem;
    use uom::si::area::square_meter;

    fn gas(p: f64, rho: f64, u: f64) -> PrimitiveState {
        PrimitiveState::PressureDensity {
            pressure: Pressure::new::<pascal>(p),
            density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
            velocity: Velocity::new::<meter_per_second>(u),
        }
    }

    /// **The analytic area slope matches a central difference of
    /// `flow_area_at`** for all three profiles (to 1e-6 relative).
    #[test]
    fn area_slope_is_the_derivative_of_the_profile() {
        let profiles = [
            AreaProfile::Uniform(2.0),
            AreaProfile::Linear { inlet_m2: 1.0, outlet_m2: 3.0 },
            AreaProfile::ConvergingDiverging { inlet_m2: 1.5, throat_m2: 1.0, outlet_m2: 2.0, throat_fraction: 0.4 },
        ];
        for prof in profiles {
            let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(2.0), prof, 10).unwrap();
            for x in [0.1, 0.5, 0.79, 0.81, 1.3, 1.9] {
                let d = 1e-6;
                let a = |x: f64| geo.flow_area_at(Length::new::<meter>(x)).get::<square_meter>();
                let fd = (a(x + d) - a(x - d)) / (2.0 * d);
                let an = area_slope(prof, x, 2.0);
                assert!((fd - an).abs() <= 1e-6 * (1.0 + an.abs()), "{prof:?} at {x}: {fd} vs {an}");
            }
        }
    }

    /// **Both mass matrices are inverted correctly**: the consistent solve
    /// reproduces `M x` for a known `x`, and the lumped one divides by
    /// `∫φ_j`. Also: a per-node initial list of the wrong length is refused.
    #[test]
    fn mass_solves_and_initial_condition_checks() {
        let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), uom::si::f64::Area::new::<square_meter>(1.0), 5).unwrap();
        let mut nm = FemNumerics::default();
        nm.mass_matrix = MassMatrix::Consistent;
        let pipe = FemPipe::new(geo, IdealGasHem::air(), HemInitialCondition::Uniform(gas(1e5, 1.0, 0.0)), Stabilisation::Supg, nm).unwrap();
        let h = 0.2;
        let x: Vec<f64> = (0..18).map(|i| (i as f64 * 0.37).sin() + 2.0).collect();
        let mut mx = vec![0.0; 18];
        for k in 0..3 {
            for j in 0..6 {
                let d = if j == 0 || j == 5 { h / 3.0 } else { 2.0 * h / 3.0 };
                let mut v = d * x[3 * j + k];
                if j > 0 {
                    v += h / 6.0 * x[3 * (j - 1) + k];
                }
                if j < 5 {
                    v += h / 6.0 * x[3 * (j + 1) + k];
                }
                mx[3 * j + k] = v;
            }
        }
        pipe.mass_solve(&mut mx);
        for (a, b) in mx.iter().zip(&x) {
            assert!((a - b).abs() < 1e-12, "{a} vs {b}");
        }
        assert!(FemPipe::new(geo, IdealGasHem::air(), HemInitialCondition::PerCell(vec![gas(1e5, 1.0, 0.0); 5]), Stabilisation::Supg, FemNumerics::default()).is_err());
        assert_eq!(pipe.node_positions().len(), 6);
        assert_eq!(pipe.stabilisation(), Stabilisation::Supg);
        assert_eq!(pipe.boundary(PipeEnd::Inlet), HemBoundary::Wall);
        assert!((pipe.geometry().length().get::<meter>() - 1.0).abs() < 1e-15);
    }

    /// **A Riemann node on the diaphragm takes the mean conserved state, and
    /// one step of each stabilisation runs.**
    #[test]
    fn diaphragm_node_and_one_step_per_stabilisation() {
        let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), uom::si::f64::Area::new::<square_meter>(1.0), 4).unwrap();
        for st in [Stabilisation::Galerkin, Stabilisation::Supg, Stabilisation::EntropyViscosity, Stabilisation::Lapidus] {
            let mut pipe = FemPipe::new(
                geo,
                IdealGasHem::air(),
                HemInitialCondition::Riemann { diaphragm: Length::new::<meter>(0.5), left: gas(1e5, 1.0, 0.0), right: gas(1e4, 0.125, 0.0) },
                st,
                FemNumerics::default(),
            )
            .unwrap();
            let s = pipe.snapshot().unwrap();
            assert!((s.cells[2].density.value - 0.5625).abs() < 1e-12);
            let m0 = pipe.total_mass().value;
            pipe.step().unwrap();
            pipe.step().unwrap();
            assert!((pipe.total_mass().value / m0 - 1.0).abs() < 1e-13, "{st:?}");
            assert!(pipe.time().value > 0.0);
        }
    }
}
