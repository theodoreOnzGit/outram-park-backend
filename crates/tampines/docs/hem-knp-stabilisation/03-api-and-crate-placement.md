# Epic #896: HEM/KNP API and crate placement (planning slice)

Read-only survey, 2026-10-16. The repo was not edited. Every claim below was
checked against the tree at that date, unless it is marked **unverified**.

> **Status update, 2026-10-16 (gh #901):** steps **M1–M4 have landed.** The KNP
> flux is one kernel, `outram_foam_basic_lib::fv_operators::fvc::central_upwind`;
> copies A, B and C call it; and the equilibrium sound speeds are public (M2).
> §2's inventory (line counts, `pub(crate)`, "three copies") describes the tree
> *before* that change and is kept as the survey record. Two deviations from
> §5: `KnpFaceState::u` is the velocity **vector** (plus a face normal), not a
> scalar normal velocity, because all three callers transport 3-D momentum;
> and B's and C's `velocity_component` stay, because their vendored field
> types are not basic-lib's (that dedup waits for M10).

---

## 1. Dependency graph

Source: `cargo metadata --format-version 1 --no-deps`, internal (path) edges only.

| crate | normal deps (internal) | dev deps (internal) |
|---|---|---|
| `petir` 0.0.1 | none | none |
| `outram-foam-basic-lib` 0.1.4 | `petir` | none |
| `tampines-steam-tables` 0.2.9 | `outram-foam-basic-lib`, `petir` | `chem-eng…`, `teh-o-prke`, `tuas_boussinesq_solver` |
| `outram-park-fork-coolprop` 0.1.2 | `outram-foam-basic-lib`, `petir` | none |
| `outram-foam-appbuilder-lib` 0.1.4 | `outram-foam-basic-lib`, `outram-foam-multiphase`, `outram-foam-turbulence-lib` | `outram-foam-mesh` |
| `tampines` 0.0.2 | `tampines-steam-tables`, `outram-park-fork-coolprop`, `outram-foam-basic-lib`, `outram-foam-multiphase`, `boon-lay`, `chem-eng…`, `outram-park-fork-dwsim-libs`, `raffles`, `tuas_boussinesq_solver` | `nee_soon`, `teh-o-prke` |

```
petir ◄── outram-foam-basic-lib ◄──┬── tampines-steam-tables ◄──┐
  ▲                                ├── outram-park-fork-coolprop ◄┤
  └────────────────────────────────┤                              ├── tampines
                                   └── outram-foam-appbuilder-lib  │   (not a dep of tampines today)
                                         outram-foam-multiphase ◄──┘
```

**Answers:**

- **tampines → tampines-steam-tables** is a normal dependency. The reverse
  edge does not exist.
- `petir` sits at the bottom, and `outram-foam-basic-lib` sits on top of it.
  Both property crates depend on both. Per the epic body, basic-lib uses petir
  only in `math/inc_gamma.rs` and `math/inv_inc_gamma.rs`.
- **Exporting the solver from tampines creates no cycle on its own.** tampines
  already depends on every crate the solver needs (both EOS backends,
  basic-lib, petir via basic-lib). It does not list petir directly, so add
  `petir.workspace = true` to `tampines`.
- **There is one cycle, and only if Edwards (in steam-tables) calls the
  tampines API.** That needs `tampines-steam-tables [dev] → tampines`, which
  closes `tampines → tampines-steam-tables → (dev) tampines`. See §4.3 for the
  fix and the measurement.
- **appbuilder:** no crate in the HEM chain depends on it, and it depends on
  nothing in that chain except basic-lib. `tampines → appbuilder` would be
  cycle-free, but it would pull in appbuilder's `ControlDict`/`FvSchemes`/I/O
  and turbulence stack just to borrow one flux kernel. **Recommendation: do
  not add that edge.** Move the shared kernel *down* into basic-lib, which all
  four crates already depend on (§2.3).

---

## 2. KNP inventory

### 2.1 The three KNP copies, plus the 1-D two-phase solvers already in tampines

| # | location | what it is | conserved vars | EOS / sound speed | time / coupling | status |
|---|---|---|---|---|---|---|
| A | `crates/outram-foam-appbuilder-lib/src/solvers/rho_central_foam/mod.rs` (510 lines) | Stand-alone **density-based explicit KNP** (`RhoCentralFoam`), a rhoCentralFoam port with van Leer MUSCL | `[ρ, ρU, ρE]`, total energy, conservative | **Hard-coded calorically perfect gas**, `const GAMMA = 1.4`, `p=(γ−1)ρe`, `c=√(γp/ρ)` | Forward Euler, fixed `dt`. `adjustTimeStep`/`maxCo` "**not implemented**" (its own doc) | Validated: Sod (1978) Table II, max 0.96 % of peak at faithful points (`tests/sod_shock_tube_validation/`, 2026-07-06). Uses positivity **clamps** (`.max(1e-10)`, `.max(0.0)`, `rho_new.max(1e-6)`). Boundary faces always use zero-gradient extrapolation, whatever the BC. |
| B | `crates/tampines-steam-tables/src/openfoam_algorithms/rhoPimpleFoam/central_upwind.rs` (491) + `mod.rs` (3265) | KNP face flux used **only as Mach-blended deferred-correction dissipation** on the pressure-based PIMPLE array `TampinesSteamArray` (`SolverMode::HybridAllMach`, opt-in; default `Pimple`) | `[ρ, ρU, ρ·he]`, **static enthalpy**, so it does not conserve total energy | IAPWS-IF97 `(p,h)` flash. `hem_sound_speed_ph` = Kieffer eq. 28 in the dome, region `w_tp_*` outside, clamped at `C_MIN_MPS = 1.0`. **`pub(crate)`** | Implicit PIMPLE + explicit KNP dissipation, fixed `dt` | Edwards gated only under `EnergyConvectionScheme::Linear`. With the van Leer default it panics at the 273.15 K floor (gh:#406). |
| C | `crates/outram-park-fork-coolprop/src/openfoam_algorithms/rhoPimpleFoam/central_upwind.rs` (549) + `mod.rs` (2526) | Twin of B on `OPCPFluidArray` | same as B | CoolProp Helmholtz + Maxwell VLE. `hem_sound_speed_ph(fluid, p, h, c_min)` uses a finite-difference HEM speed in the dome. **`pub(crate)`** | same as B | same pattern |

**Measured duplication:**

- **B's and C's `face_flux_impl` are code-identical.** With comments stripped,
  `diff` prints nothing.
- **A's inline flux is the same formula**, written out per component, with
  `ρE` and total enthalpy in place of `ρ·he`.
- `velocity_component` is copied in A and in C (and in B's mod).
- `fvc/muscl.rs` exists three times: basic-lib, steam-tables and coolprop
  `openfoam_source`. The steam-tables and coolprop copies are byte-identical.
  They differ from basic-lib only in `use` paths, because #492 deduplicated
  `FvMesh` but not the field types.

**Other implementations.** I grepped for `kurganov|central.upwind|knp|rusanov|hll|riemann|godunov|lax.friedrichs`
across `crates/`. There is no other flux kernel. The only other Riemann code
is an **ideal-gas exact Riemann solver (Toro ch. 4)** inside
`outram-foam-appbuilder-lib/tests/sod_shock_tube_validation/main.rs`, which is
reusable for the Sod test (§5).

**Existing 1-D two-phase solvers in tampines.** These are not KNP, but they
are what the new solver must sit beside and reuse:

- `tampines::multiphase_1d::{DriftFlux1d (4-eq), TwoFluid1d (6-eq)}`. Both use
  a semi-implicit, pressure-based march with donor-cell fluxes and a
  tridiagonal pressure solve (`multiphase_1d::thomas_solve`, local code; petir
  has `linalg::tridiag`, which is GSL symmetric).
- The pieces they share, and which the HEM solver should reuse rather than
  copy:
  - `Pipe1d` (uom constructor, `circular()`)
  - `properties::{SaturatedProperties, TwoPhaseState::flash(p,h)}` (IF97 HEM flash)
  - `AxialBoundary::{Closed, PrescribedVelocity, ChokedOutlet}` (break via the
    HEM critical-flow dispatcher)
  - `TampinesError` (its policy: "Reported rather than clamped")
- The `multiphase_1d` module doc lists **`crate::hem` as the 3-equation rung
  of the ladder**. `tampines::hem` is only
  `pub use …TampinesSteamTableCV as HemSteamCv` and contains no solver. The new
  solver fills exactly that gap.

### 2.2 What each copy takes as EOS

- **A:** none. It is a constant.
- **B:** free functions on `(p, h)` in SI `f64`. The face state is
  `FaceState{rho, u, he, p, c}`.
- **C:** the same, plus a `Fluid` argument.
- **Inversion functions that exist** (needed by a density-based solver, which
  carries ρ and ρE and must recover `p`):
  - IF97 `p_rho_h_eqm` / `tpx_rho_h_eqm`
    (`interfaces/functional_programming/rho_h_flash_eqm`): an accurate, bracketed
    1-D root in `p` over `v_ph_eqm`, covering **all regions including the
    dome**. Its root finder is in-house bisection plus a fallback; it does not
    use `petir::roots`.
  - CoolProp `state_hrho` / `temperature_hrho`: **single-phase only**. Its own
    doc says it returns `NonConvergent` "inside the two-phase dome, which this
    crate does not model". It is in-house Newton plus bisection, not petir.
  - **There is no `(ρ, e)` flash anywhere.** For a total-energy solver it can
    be built generically: `p` solves `ρ·v_ph(p, e + p/ρ) − 1 = 0`. That is one
    bracketed `petir::roots` Brent solve per cell over any backend's `(p,h)`
    flash (CoolProp `state_ph` does handle the dome via Maxwell VLE).

**WARNING:** the public sound-speed getters are the wrong physics for HEM.

- `TampinesSteamTableCV::get_speed_of_sound` (re-exported as
  `tampines::hem::HemSteamCv`) returns **`w_ph_wood_wallis`, the frozen speed**
  (`getter_methods.rs:65`).
- The equilibrium speeds (`hem_sound_speed_ph` in both crates) are
  `pub(crate)`.

A tampines EOS wrapper written against today's public API would silently use
Wood–Wallis. B's own module doc says that speed misplaces the characteristics.
The equilibrium speeds must be made public first (step M2).

### 2.3 One generic kernel, parameterised by an EOS enum

Yes, in two layers. This respects the rules: enum dispatch, no `dyn`, no
`Box`, no lifetimes, and `uom` at API boundaries.

1. **An EOS-free flux kernel in `outram-foam-basic-lib`**, as a new module
   `fvc::central_upwind`, raw SI `f64` like the rest of basic-lib.
   - The KNP flux only consumes `(ρ, U, p, c)` plus one energy pair per side.
     So it takes the energy *density* `w_e` and the energy *flux per unit mass
     flux* `f_e`:
     - A passes `w_e = ρE`, `f_e = H`.
     - B and C pass `w_e = ρ·he`, `f_e = he`.
   - It also carries `knp_face_flux`, `central_face_flux`, `mach_blend` and the
     `a_max` needed for the Courant number.
   - Every crate in the chain already depends on basic-lib, so there is no new
     edge.
   - B, C and A then delete their copies and call this one. That is three
     copies down to one.
2. **An EOS enum in `tampines`.** It can only live there or above, because it
   must name IF97 *and* CoolProp, and only tampines depends on both.

   ```rust
   pub enum HemEquationOfState { Iapws97, CoolProp(CoolPropFluid), IdealGas { gamma: Ratio, gas_constant: SpecificHeatCapacity } }
   ```

   - Each variant forwards to its backend's public `(p,h)` flash, its public
     **equilibrium** sound speed (after M2), and a shared petir-Brent `(ρ,e)→p`
     inversion.
   - `IdealGas` is closed-form. It makes the Sod limit run through the *same*
     kernel and driver as water.
   - A compile-time `trait HemEos` on each concrete closure struct is optional
     (the rust-design-rules pattern), not required.
   - **Reuse the existing `tampines::fluids::TampinesFluid`** (`Steam |
     CoolProp(Fluid)`) for selecting the backend. Either add an `IdealGas`
     variant, or have the EOS enum wrap it:
     `HemEquationOfState::Fluid(TampinesFluid) | IdealGas{..}`. Do not mint a
     third substance list.

**Rejected alternative: a generic `Knp1d<E: HemEos>` in basic-lib, with impls
in each property crate.** It would let steam-tables run Edwards without
tampines, avoiding the cycle. But it puts a type parameter on the user-facing
type (the api-design doc forbids "extra type parameters … if it raises the
mental context load"). It also scatters the EOS impls across three crates.
Only reconsider it if the maintainer rejects the dev-dependency in §4.3.

---

## 3. Proposed public API in `tampines`

**Module:** `tampines::multiphase_1d::hem`, the 3-equation rung beside
`drift_flux` and `two_fluid`, sharing `Pipe1d`. Re-export it as
`tampines::hem::{HemPipe, …}` so that a user looking for "HEM" finds it in
`tampines::hem` too. Update `hem/mod.rs`'s `//!` to point at it.

```rust
//! tampines::multiphase_1d::hem — Homogeneous Equilibrium Model (3-equation)
//! transient pipe flow, explicit density-based KNP central-upwind scheme.
//! Solves ∂W/∂t + ∂F/∂x = S for W = [ρ, ρu, ρE] (TOTAL energy, conservative),
//! closed by an equilibrium EOS (both phases at T_sat(p), no slip).
//! Not in here: slip (see drift_flux), thermal non-equilibrium (two_fluid).

use uom::si::f64::{Pressure, ThermodynamicTemperature, AvailableEnergy, Velocity,
                   MassDensity, Time, Ratio, Mass, Energy, MassRate, Length};
pub use crate::multiphase_1d::Pipe1d;            // reused, not new
pub use crate::fluids::TampinesFluid;            // reused, not new

/// Equation of state closing the HEM system. Every variant supplies the
/// EQUILIBRIUM sound speed (never frozen/Wood–Wallis) — that is what places
/// the KNP characteristics correctly through a flashing front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HemEquationOfState {
    /// IAPWS-IF97 water/steam (tampines-steam-tables). Valid 273.15–1073.15 K,
    /// p ≤ 100 MPa (Region 5 to 2273.15 K, p ≤ 50 MPa).
    Iapws97,
    /// CoolProp Helmholtz EOS + Maxwell VLE for any `CoolPropFluid`.
    CoolProp(CoolPropFluid),
    /// Calorically perfect gas, p = (γ−1)ρe. For verification (Sod limit).
    IdealGas { gamma: Ratio, specific_gas_constant: SpecificHeatCapacity },
}

/// One cell's thermodynamic state, uom-typed. Returned by snapshots and by
/// `HemEquationOfState::state_rho_e` (public so a caller can test the closure
/// on its own).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemCellState {
    pub pressure: Pressure,
    pub temperature: ThermodynamicTemperature,
    pub density: MassDensity,
    pub velocity: Velocity,
    pub specific_enthalpy: AvailableEnergy,
    /// Equilibrium vapour mass fraction; `None` outside the dome.
    pub quality: Option<Ratio>,
    pub void_fraction: Ratio,
    pub equilibrium_sound_speed: Velocity,
}

/// Initial condition. Constructors cover what callers actually have.
#[derive(Debug, Clone, PartialEq)]
pub enum HemInitialCondition {
    /// Uniform (p, T, u). Errors inside the dome (T does not fix the state).
    UniformPressureTemperature { pressure: Pressure, temperature: ThermodynamicTemperature, velocity: Velocity },
    /// Uniform (p, h, u) — valid in the dome.
    UniformPressureEnthalpy { pressure: Pressure, specific_enthalpy: AvailableEnergy, velocity: Velocity },
    /// Riemann problem: left state for x < `diaphragm`, right state beyond.
    Riemann { diaphragm: Length, left: PrimitiveState, right: PrimitiveState },
    /// Per-cell (p, T) profile, length = pipe.n_cells() (Edwards' Hendrie profile).
    PressureTemperatureProfile { pressure: Vec<Pressure>, temperature: Vec<ThermodynamicTemperature> },
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveState {
    PressureTemperature { pressure: Pressure, temperature: ThermodynamicTemperature, velocity: Velocity },
    PressureEnthalpy   { pressure: Pressure, specific_enthalpy: AvailableEnergy, velocity: Velocity },
    PressureDensity    { pressure: Pressure, density: MassDensity, velocity: Velocity }, // ideal-gas Sod
}

/// Which pipe end a boundary applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeEnd { Inlet /* x = 0 */, Outlet /* x = L */ }

/// End boundary condition (applied through a ghost state in the KNP flux).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HemBoundary {
    /// Reflective rigid wall, u = 0 (Edwards closed end). Conserves mass/energy.
    Wall,
    /// Zero-gradient (transmissive) — Sod's open ends.
    Transmissive,
    /// Stagnation reservoir (p0, h0) feeding/receiving the pipe.
    Reservoir { stagnation_pressure: Pressure, stagnation_enthalpy: AvailableEnergy },
    /// Fixed static back-pressure.
    FixedPressure(Pressure),
    /// HEM choked break of area `area_fraction × A` discharging to
    /// `back_pressure` — mass flux from the existing, Moody/Zaloudek-verified
    /// `crate::critical_flow` dispatcher. `Iapws97` only; other EOS →
    /// `TampinesError::InvalidInput`. Change `area_fraction` between steps to
    /// ramp a rupture disc.
    ChokedBreak { area_fraction: Ratio, back_pressure: Pressure },
}

/// Time-step control. Default: adaptive at Courant 0.4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeStepControl {
    /// dt = courant × min_f(Δx / a_max,f), a_max the KNP local wave speed
    /// (rhoCentralFoam's `amaxSf`), capped at `max_dt`.
    AdaptiveCourant { courant: Ratio, max_dt: Time },
    /// Fixed dt; `step` returns `CourantViolation` rather than running unstable.
    Fixed(Time),
}

/// Numerical options. `Default` is the validated configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemNumerics {
    pub time_step: TimeStepControl,
    pub limiter: Limiter,            // re-export of outram_foam_basic_lib fvc::Limiter, default VanLeer
    pub time_integrator: TimeIntegrator, // ForwardEuler (rhoCentralFoam) | SspRk2 (see §7, decision D3)
}

/// The solver. Owns its state; no lifetimes, no Box, no dyn.
#[derive(Debug, Clone)]
pub struct HemPipe { /* pipe, eos, numerics, W = [ρ, ρu, ρE] Vec<f64>s, bcs, time */ }

impl HemPipe {
    /// Build and flash the initial state. Errors: bad geometry/IC, EOS out of range.
    pub fn new(pipe: Pipe1d, eos: HemEquationOfState, initial: HemInitialCondition,
               numerics: HemNumerics) -> Result<Self, TampinesError>;
    pub fn set_boundary(&mut self, end: PipeEnd, bc: HemBoundary);
    /// One step; returns what happened (dt taken, max Courant, choked?).
    pub fn step(&mut self) -> Result<HemStepReport, TampinesError>;
    /// Step until `t_end` (last step shortened to land exactly).
    pub fn run_until(&mut self, t_end: Time) -> Result<HemRunSummary, TampinesError>;
    pub fn time(&self) -> Time;
    pub fn cell_state(&self, cell: usize) -> Result<HemCellState, TampinesError>;
    pub fn snapshot(&self) -> HemSnapshot;          // owned Vec<HemCellState> + time
    pub fn total_mass(&self) -> Mass;               // Σ ρ V  — conservation checks
    pub fn total_energy(&self) -> Energy;           // Σ ρE V
    pub fn boundary_mass_flow(&self, end: PipeEnd) -> MassRate;
    pub fn pipe(&self) -> &Pipe1d;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemStepReport { pub time: Time, pub dt: Time, pub max_courant: Ratio,
                           pub outlet_choked: bool, pub min_pressure: Pressure }
#[derive(Debug, Clone, PartialEq)]
pub struct HemSnapshot { pub time: Time, pub cells: Vec<HemCellState>, pub cell_centres: Vec<Length> }
```

**Errors.** Reuse `TampinesError`. Do not add a parallel error enum. Two new
variants:

- `CourantViolation { courant: f64, limit: f64 }`
- `EquationOfState { cell: usize, detail: String }`, raised when a `(ρ,e)` flash
  fails or leaves the IF97 range.

Everything else maps to the existing `InvalidInput`, `Unphysical` and
`Numerical`. **No clamps**, unlike A: negative density or pressure is
`Unphysical`, per `TampinesError`'s own policy.

**Haiku-test notes.**

- The task "Sod in water" is reachable from rust-analyzer as
  `HemPipe::new(Pipe1d::circular(..), HemEquationOfState::Iapws97, HemInitialCondition::Riemann{..}, HemNumerics::default())`
  followed by `run_until`.
- Every quantity is uom. Raw `f64` stays inside the step loop.
- There is one entry type, enums carry the choices, and there are no generics
  on the public type.
- The fixed-dt CFL guard turns a silent blow-up into a named error.
- tampines has no maturity statement in its `CLAUDE.md`, so the dogfooding
  rule is **not yet binding** on it. The maintainer asked for the test anyway;
  run a baseline Haiku session once the API compiles (step M7).

---

## 4. Test plan

### 4.1 Unit and verification tests in `tampines`

Location: `crates/tampines/src/multiphase_1d/hem_tests.rs`, plus `tests/` for
the long ones.

1. **Ideal-gas Sod limit, cross-code against appbuilder A.**
   - Setup: `IdealGas{γ=1.4}`, 100 cells on [−5, 5] m, Sod ICs,
     `t = 6.3246e-3 s`, `Fixed(1e-6 s)`, ForwardEuler, van Leer. These are A's
     exact settings.
   - Gate 1: vs A's profile at round-off on a **closed-end or transmissive**
     pair, which requires the shared basic-lib kernel.
   - Gate 2: vs the Toro ch. 4 exact solution, moved from A's test into a
     shared test helper (or `outram-foam-basic-lib` dev module). Same < 5 %
     peak criterion at the faithful Table II points.
   - Note: A clamps and B/C do not, so expect a bit-identical match only while
     no clamp fires. State that in the test doc.
2. **Positivity.**
   - Ideal-gas strong double rarefaction (Toro test 2, `u = ∓2` with
     `p = 0.4`): ρ > 0 and p > 0 at every step, at Courant ≤ the documented
     limit.
   - IF97 water double rarefaction (below) reaching the dome: p stays above
     the triple point, and `HemCellState` never errors.
   - The **negative** test: Courant 1.5 under `Fixed` must return
     `CourantViolation`, not NaN.
3. **Conservation.**
   - `Wall` at both ends.
   - Set up a non-uniform IF97 state (subcooled plus two-phase halves) and run
     N steps.
   - Gate: |ΔΣρV|/ΣρV and |ΔΣρEV|/ΣρEV ≤ 1e-12 per 1000 steps (flux form, so
     this is round-off).
   - This test is why the solver must carry `ρE`, not `ρ·he` (B/C conserve
     neither total energy nor, with the folded `phi`, the energy split).
4. **Galilean/symmetry invariants.**
   - A mirror-symmetric IC stays symmetric to round-off.
   - A uniform state with uniform `u` and transmissive ends is stationary.
5. **Default pinned.** Following `outram-mc-libs/tests/correct_physics_is_default.rs`,
   assert that `HemNumerics::default()` gives `AdaptiveCourant` and `VanLeer`,
   and that `Iapws97` uses the **equilibrium**, not Wood–Wallis, sound speed.
   Compare against Kieffer at one dome state.

### 4.2 HEM verification case with a semi-analytical answer

**Proposal: the HEM water double-rarefaction ("expansion tube").**

- **IC:** liquid water at `(p0, T0)` subcooled, with `u = −u0` left and `+u0`
  right of the centre.
- **The exact HEM solution** is two symmetric rarefactions along the
  isentrope `s0`. The centre state follows from the Riemann invariant
  `u(p) = u0 − ∫_p^{p0} dp'/(ρ c_eq)` with `u(p*) = 0`.
- **Method:** one quadrature (`petir::integration::qags`) plus one root
  (`petir::roots` Brent) over IF97 `(p,s)` flashes. This is semi-analytical
  and uses only petir numerics.
- Choose `u0` large enough that `p*` falls inside the dome. The equilibrium
  sound speed then drops discontinuously at saturation (about 1000 → tens of
  m/s). The fan splits into a liquid fan, a constant-state plateau and a
  two-phase fan. That is the HEM-specific structure a Wood–Wallis or
  ideal-gas code gets wrong, so it tests the closure and not just the scheme.
- **It avoids shocks,** and so avoids the non-convex Hugoniot issues at the
  phase boundary.
- **Predict the plateau `p*` before running** (CLAUDE.md "predictions written
  before runs").

**Candidate citations.** **Unverified.** None of these are in the kovan
corpus. The maintainer supplies the corpus, so they need reading before being
cited in a doc comment.

- Menikoff, R. & Plohr, B. J. (1989), "The Riemann problem for fluid flow of
  real materials", *Rev. Mod. Phys.* 61(1), 75–130. Covers general-EOS wave
  curves, including phase transitions.
- Müller, S. & Voss, A. (2006), "The Riemann problem for the Euler equations
  with nonconvex and nonsmooth equation of state: construction of wave
  curves", *SIAM J. Sci. Comput.* 28(2), 651–681.
- Colella, P. & Glaz, H. M. (1985), "Efficient solution algorithms for the
  Riemann problem for real gases", *J. Comput. Phys.* 59, 264–289.
- Saurel, R., Petitpas, F. & Abgrall, R. (2008), "Modelling phase transition
  in metastable liquids: application to cavitating and flashing flows",
  *J. Fluid Mech.* 607, 313–350. Has water expansion-tube tests.
- Lund, H. (2012), "A hierarchy of relaxation models for two-phase flow",
  *SIAM J. Appl. Math.* 72(6), 1713–1741. HEM sound speed in the hierarchy.
- Kurganov, A., Noelle, S. & Petrova, G. (2001), *SIAM J. Sci. Comput.* 23(3),
  707–740. Greenshields, C. J. et al. (2010), *Int. J. Numer. Meth. Fluids*
  63, 1–21 (the rhoCentralFoam paper).
- Toro, E. F. (2009), *Riemann Solvers and Numerical Methods for Fluid
  Dynamics*, 3rd ed., Springer, ch. 4. Already cited by A's Sod test.

**Secondary case, cross-code: a single-phase liquid-water shock tube.** No
dome. Compare the IF97 solution against the `CoolProp(Water)` solution through
the same kernel; the EOS difference bounds the agreement. This is consistent
with the "cross-code comparison" evidence class. Do **not** gate it as
validation.

### 4.3 Edwards stays in `tampines-steam-tables`, calling the tampines API

**The direction does not allow it as-is.** It needs
`tampines-steam-tables [dev-dependencies] tampines`, a dev-only back-edge on
top of the normal `tampines → tampines-steam-tables` edge.

**Measured on a toy two-crate workspace (scratchpad `cyc/`, `cargo test --release`):**

- Cargo **accepts** a normal-dep plus dev-dep cycle.
- **Integration tests (`tests/*.rs`) work.** The test links the same `a` lib
  that `b` links, so `a::State` unifies. `test t ... ok`.
- **Unit tests in `src/` (`#[cfg(test)]`) break.** `a` is compiled twice, and
  rustc reports `E0308 mismatched types: expected a::State, found State`.

**Fix (recommended):**

1. Add a **dev-dependency `tampines` to `tampines-steam-tables`**, inside the
   existing Android/wasm-gated dev-dep block.
2. Keep the Edwards HEM case strictly in `tests/edwards_blowdown.rs`, which
   already is an integration test. **Never** call tampines from steam-tables'
   `src/` tests.
3. Declare it path-only (`tampines = { path = "../tampines" }`, no version) so
   `cargo publish` strips it. Otherwise steam-tables could not be published
   before tampines, which itself needs steam-tables.
4. Record the edge in `docs/claude-md/members-and-dependencies.md`'s edge list.
   It is a maintainer-approval item, like the 2026-10-02 edges there.

**Costs:**

- `cargo test -p tampines-steam-tables` now builds tampines' whole tree:
  raffles, boon-lay, dwsim, coolprop, multiphase.
- Kovan's code map gains a back-edge.

**Fallback if the maintainer rejects the back-edge:** move Edwards-HEM to
`crates/tampines/tests/edwards_hem_blowdown.rs`. tampines already depends on
steam-tables, so there is no cycle. This mirrors the drift-flux Edwards in
`tampines/src/multiphase_1d/edwards_tests.rs`. Steam-tables would then keep
only the choked-flow (Moody/Zaloudek) gates.

**Shared data either way.** The digitised GS-1 data, the Hendrie profile and
the break ramp are duplicated today: `GS1_DATA_PSIA` was "transcribed from the
HEM test file" into `edwards_tests.rs`. Export them once as
`tampines::multiphase_1d::edwards_case`, a `pub` fixture module with
provenance. Then both the drift-flux and HEM Edwards tests read one copy.

---

## 5. Migration steps (candidate child issues, in order)

| step | deliverable | moves / changes | deletes / deprecates |
|---|---|---|---|
| **M1** | Shared KNP kernel in basic-lib | New `outram_foam_basic_lib::fv_operators::fvc::central_upwind`: `KnpFaceState{rho,u,p,c,w_e,f_e}`, `knp_face_flux`, `central_face_flux`, `mach_blend`, `local_wave_speed_max`; plus `velocity_component`. Unit tests ported from B's `central_upwind::tests`. Cite KNP 2001 and the upstream rhoCentralFoam file. | none yet |
| **M2** | Public equilibrium sound speeds | Promote `hem_sound_speed_ph` to public API in steam-tables (e.g. `ph_flash_eqm::w_ph_eqm_hem`) and in coolprop (`flash::hem_sound_speed_ph`), with uom wrappers. Fix the `TampinesSteamTableCV::get_speed_of_sound` doc to say it is **frozen (Wood–Wallis)**, and add `get_equilibrium_speed_of_sound`. | none |
| **M3** | Re-point B and C at the kernel | B's and C's `central_upwind.rs` keep only the EOS-specific face-state assembly and call M1. Gate: Edwards `HybridAllMach` numbers **bit-identical** before and after (pure refactor). | Delete the duplicated `face_flux_impl` and `velocity_component` in B and C. |
| **M4** | Re-point appbuilder A at the kernel | `RhoCentralFoam::step` calls M1 with `w_e = ρE`, `f_e = H`. Gate: Sod test unchanged. Optionally implement `maxCo` adaptive dt, reading upstream `setDeltaT.H` / `centralCourantNo.H` first. | Delete A's inline flux. |
| **M5** | petir-based `(ρ,e)` closure | In tampines: `HemEquationOfState` plus `state_rho_e` via a `petir::roots` Brent bracket on `p` over the backend `(p,h)` flash, with `petir::mathf` for transcendentals. Coordinate with the petir-audit slice on migrating `p_rho_h_eqm` and CoolProp `temperature_hrho` to petir roots. | none |
| **M6** | `HemPipe` solver plus tests §4.1–4.2 | New `tampines::multiphase_1d::hem` (re-exported in `tampines::hem`). Reuse `Pipe1d`, `TampinesFluid`, the `critical_flow` dispatcher and `TampinesError`. Add `petir` to tampines deps. Move the Toro exact-Riemann helper to a shared place. V&V doc with methodology plus results. | none |
| **M7** | Haiku API baseline | Fresh-context small-model run on the §3 API. Log every exception. | none |
| **M8** | Edwards on the tampines API | Per §4.3: dev-dep back-edge (or the fallback), and the shared `edwards_case` fixture. Edwards-HEM rerun on `HemPipe`, with results recorded beside the historical PIMPLE numbers. Do not overwrite them ("fix the instruction, never the receipt"). | Deprecate GS-1 and Hendrie copies; the old PIMPLE Edwards pins stay as a historical save point until the maintainer retires them (gh:#406). |
| **M9** | Decide the fate of `HybridAllMach` | Once `HemPipe` is stable, propose to the maintainer either retiring `SolverMode::HybridAllMach` (B and C) or keeping it as the low-Mach network-component option. **`TampinesSteamArray` and `OPCPFluidArray` themselves must stay**: they have consumers in `tampines::components`, `gas_phase::pipe`, `teh-o-prke`, the digital-twin examples and `htgr_sim_v1`. | Possibly `HybridAllMach` and `KnpFaceClosure` (maintainer call) |
| **M10** | MUSCL dedup (optional, separate) | The steam-tables and coolprop `openfoam_source/fv_operators/fvc/muscl.rs` are byte-identical to each other and differ from basic-lib only in `use` paths. Deduplicate once their field types are unified (#492's follow-on). | Two `muscl.rs` copies |

The end state is **one** KNP flux (basic-lib) and **one** HEM solver
(tampines). The PIMPLE arrays stay, but they consume the shared kernel and do
not copy it.

---

## 6. Doc claims the code contradicts

I found these while surveying. I am read-only, so they are reported here. The
hard rule says to fix them in the change that touches them.

1. **`crates/tampines/src/fluids/mod.rs` (`TampinesFluid` doc)** says
   steam-tables' `tests/edwards_blowdown.rs` "exercises the transient pipe
   solver in [`crate::multiphase_1d`]". **False:** it drives
   `TampinesSteamArray` (`edwards_blowdown.rs:393`). multiphase_1d's own
   Edwards test is the drift-flux one. The same doc says the Edwards benchmark
   "is genuinely gated -- 2 active, passing tests", but the file's own header
   (gh:#406) now says it is valid only under the non-default Linear scheme.
2. **`crates/tampines/src/multiphase_1d/mod.rs`** lists `[crate::hem]` as a
   3-equation transient solver in the ladder table. `tampines::hem` contains
   no solver; it is only a state-type re-export.
3. **`docs/claude-md/members-and-dependencies.md`** says "`tampines` dev-deps
   → `{tuas, teh-o-prke, chem-eng}` (the **library** itself is TUAS-free)".
   **False:** `tuas_boussinesq_solver` and `chem-eng…` are **normal** deps of
   tampines (Cargo.toml, `cargo metadata`), and `nee_soon` is a dev-dep that is
   not listed.
4. **`crates/tampines/src/critical_flow/mod.rs`** puts the Marviken data in
   `marviken_tests.rs` in the steam-tables context. A
   `tampines/src/multiphase_1d/marviken_tests.rs` also exists. **Not
   re-checked** which file the claim means. Verify it before M6 touches that
   doc.
5. The root `Cargo.toml` pins `tampines-steam-tables` at `version = "0.2.8"`
   while the crate is 0.2.9. That is semver-compatible, so it is not a defect;
   noted only for the publish step in §4.3.

---

## 7. Open decisions for the maintainer

- **D1 — scheme.** This plan assumes the stable solver is a **density-based
  explicit KNP on `[ρ, ρu, ρE]`**, with adaptive CFL, the A lineage. The
  alternative is the current PIMPLE plus KNP hybrid (B). The instability
  diagnosis slice should confirm this. The API in §3 is scheme-agnostic except
  for `TimeStepControl`.
- **D2 — the dev-dependency back-edge** `tampines-steam-tables (dev) →
  tampines`, or Edwards-HEM as a tampines integration test (§4.3).
- **D3 — time integrator.** Upstream rhoCentralFoam is forward Euler. An
  SSP-RK2 (Shu–Osher) would be the usual stabiliser for MUSCL, but **petir has
  only `rk4_step` / `rkf45`**. GSL has no SSP scheme, and petir's rule forbids
  AI-written numerics. So an SSP integrator needs a named upstream to port
  from, or a maintainer exception.
- **D4 — CoolProp in the dome.** `state_hrho` refuses two-phase states. The
  generic petir `(ρ,e)→p` root over `state_ph` covers it, but that path is
  new and unverified for CoolProp HEM.
