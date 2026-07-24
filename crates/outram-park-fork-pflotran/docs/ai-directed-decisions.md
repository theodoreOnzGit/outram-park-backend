# AI-directed decisions — `outram-park-fork-pflotran`

> **Status: awaiting human review.** This document records engineering
> decisions made **autonomously by an AI assistant** (Claude) while building the
> v1 vertical slice of the PFLOTRAN Rust fork, under an explicit
> "make-the-best-decisions-you-can and flag them" directive from the maintainer
> (2026-07-22). Every entry here is an AI judgement call that a human has **not
> yet** reviewed or approved. Per the workspace `RESPONSIBLE_USE.md`, all of
> this is **untrusted draft material** until a maintainer signs off (see the
> crate README "Bookkeeping status" block — both axes remain ❌).
>
> Nothing here has been validated against published PFLOTRAN reference results.
> Verification tests (MMS, analytical) check *internal correctness only*.

## How to use this document

Each decision has: **what** was decided, **why**, **alternatives considered**,
and an **open question / review ask** for the human. Reviewers: search for
`REVIEW:` to find the explicit asks.

---

## D1 — Reuse `outram-foam-basic-lib` linear algebra; add a pure-Rust `krylov` module rather than translating OpenBLAS

- **Decision.** The Newton–Krylov linear backbone reuses
  `outram-foam-basic-lib`'s face-addressed sparse `LduMatrix` (its `multiply`
  is the SpMV) and dense `SquareMatrix` LU. The pieces foam-basic-lib lacks for
  an **asymmetric** Richards Jacobian — BiCGStab, GMRES(m), ILU(0) and Jacobi
  preconditioners, and BLAS-1 vector helpers — are added as a **new pure-Rust
  `krylov` module inside foam-basic-lib**, then imported here.
- **Why.** The maintainer directed "use outram-foam basic libs for matrix
  algebra … otherwise translate OpenBLAS … only if needed", and later "upgrade
  outram-foam basic libs some capabilities from OpenBLAS, as a separate module,
  then import them in pflotran". foam-basic-lib already ships SPD solvers
  (DIC-PCG, GAMG) but only Gauss–Seidel for asymmetric systems, which is too
  weak as a Newton inner solve. A first-principles pure-Rust Krylov module is
  the minimal capability gap, and keeps everything Android-clean (no system
  BLAS, satisfying the workspace Android rule).
- **Alternatives.** (a) Translate OpenBLAS/LAPACK wholesale — rejected as
  vastly out of scope and Android-hostile. (b) Use only Gauss–Seidel —
  rejected, poor convergence for advection-dominated / anisotropic systems.
- **REVIEW:** confirm the new capability belongs in `outram-foam-basic-lib`
  (making it a solver dependency of pflotran) rather than living inside the
  pflotran crate. Confirm the `krylov` public API shape.

## D2 — Parallelism: rayon CPU + Android-gated wgpu addon (op-v6s.14)

- **CPU (rayon), trusted default.** The hot per-cell loops are parallelised with
  rayon (pure Rust, Android-clean): RICHARDS residual + numerical Jacobian, the
  two-phase multiphase block residual/Jacobian, and the reactive-transport
  per-cell speciation. All writes land in disjoint indices (parallel map →
  serial scatter), so results are **bit-identical to serial** — proven by every
  existing test still passing.
- **GPU (wgpu), acceleration only.** A demonstrator `gpu` module (batched van
  Genuchten `Se(pc)` compute kernel) is **target-gated OFF Android** (verified:
  no wgpu in the `aarch64-linux-android` dep graph) with a mandatory CPU
  fallback (`probe()` → `None` on no-GPU hosts). GPU runs `f32`; the `f64` CPU
  path is authoritative.
- **REVIEW (important):** the **GPU dispatch path was NOT executed** in the dev
  environment (no `/dev/dri`); only the CPU fallback ran. The wgpu kernel must be
  validated on a GPU-equipped host, and it is not yet wired into the solver's
  hot property-evaluation loop (it is a standalone accelerator for now).
  Precision (`f32` GPU vs `f64` CPU) and whether to offload a bigger kernel
  (SpMV, residual batch) are open.

## D3 — v1 scope (unchanged from bead op-v6s.2, restated for the build)

- RICHARDS (variably-saturated single-phase) · structured Cartesian FV ·
  two-point flux · serial pure-Rust Newton–Krylov · minimal card-deck I/O +
  CSV/VTK. **Out of v1:** unstructured grids, MPI, HDF5, multiphase (GENERAL),
  energy (TH), solute transport, geochemistry (GIRT).

---

## Verification & validation posture (important — read before trusting any number)

- **Verification (implemented correctly?)** is done with the **Method of
  Manufactured Solutions (MMS)** and closed-form analytical cases (steady 1D
  saturated flow). These check that the discretisation converges at its design
  order and reproduces exact solutions — internal correctness only.
- **Validation (matches physical reality / published reference?)** is **NOT**
  done. The transient Richards cases (e.g. a Celia-1990-style infiltration) are
  currently **regression** tests: they record the *current* output as a frozen
  baseline so future changes are caught, but that baseline has **not** been
  compared to published PFLOTRAN gold-files or experimental data. Do not cite
  any of these numbers as validated (bead op-v6s.9).
- No result in this crate's tests was hand-written; every recorded number is
  produced by actually running the code. Where a reference value is used, its
  source is cited in the test doc comment.

---

## Decision log (appended as work proceeds)

<!-- New decisions are appended below with the next Dn index. -->

### D4 — `krylov` module in `outram-foam-basic-lib` (fleet-built)

- **GMRES uses RIGHT preconditioning** (solves `A M^-1 u = b`, `x = M^-1 u`) so
  the Givens residual estimate equals the true residual; it also recomputes the
  true residual each restart as the authoritative stopping test.
- **ILU(0) is a genuine incomplete-LU** (IKJ elimination on a CSR view built
  from the LDU coefficients, restricted to A's sparsity), exact for tridiagonal
  matrices — *not* a Jacobi fallback. Only safeguard is a small-pivot floor
  (`1e-300`, sign-preserving).
- Verification only: solvers cross-checked against dense `SquareMatrix` LU
  (rel err < 1e-6) and analytic tridiagonal LU. No physical validation.
- **REVIEW:** the crate's own `CLAUDE.md` porting workflow asks for a README
  "Ported items" row and a `cargo test --doc` run — done in the bookkeeping
  step; confirm the `krylov` module belongs at Layer 1b of that crate.

### D5 — Grid conventions

- Cell ordering **x-fastest**: `index = i + nx*(j + ny*k)`. Connection/LDU face
  order is the same traversal (each cell emits +x, +y, +z faces), so LDU face
  `f` ↔ `connections()[f]`, and `owner < neighbour` holds by construction.
- `cell_index`/`cell_ijk` use `debug_assert!` bounds (no release-mode cost) —
  out-of-range in release is UB-adjacent (panics downstream), a deliberate
  hot-path choice. **REVIEW:** confirm acceptable.

### D6 — Property models and their known singularities

- **Model pairing:** van Genuchten retention ↔ **Mualem** relative permeability;
  Brooks–Corey retention ↔ **Burdine** relative permeability. These are the
  classical self-consistent pairings and match PFLOTRAN defaults. VG+Burdine is
  not offered in v1.
- **EOS:** slightly-compressible exponential `rho(p) = rho_ref*exp(c*(p-p_ref))`
  (strictly positive, derivative exactly `c*rho` for a consistent Jacobian);
  constant viscosity.
- **Known non-smoothness a Newton solver must handle:** VG–Mualem
  `dkr/dSe -> +inf` as `Se -> 1` (returned as `f64::INFINITY`, intrinsic to the
  model); Brooks–Corey `dSe/dpc` is discontinuous at the air-entry pressure
  `pc = 1/alpha`. **REVIEW:** these are physical properties of the models, but
  the RICHARDS Jacobian/line-search must be robust to them (see D8).

### D7 — Input-deck format is an AI-designed subset (NOT real PFLOTRAN syntax)

- The `io` card grammar (`GRID`/`MATERIAL`/`CHARACTERISTIC_CURVES`/
  `BOUNDARY_CONDITION`/`TIME` blocks, `#`/`!` comments, `END`/`/` terminators)
  is an AI-invented minimal subset. It is **not** compatible with genuine
  PFLOTRAN input decks and makes no fidelity claim.
- **REVIEW (high priority):** a human must decide whether to (a) keep this
  lite format for v1 verification and document it as non-PFLOTRAN, or (b)
  replace it with a real PFLOTRAN input-deck parser. Flagged in `io/mod.rs`.

### D8 — Newton solver strategy

- Backtracking **Armijo** line search (`||F(x+λdx)|| <= (1 - 1e-4 λ)||F(x)||`,
  λ halved from 1 up to `max_backtracks`); if none pass, take the least-residual
  trial λ to guarantee progress.
- **Inexact Newton:** a non-converged inner Krylov solve is tolerated (its
  `converged` flag is ignored and the direction still used); only a non-finite
  `dx` is fatal. **REVIEW:** whether to surface inner-solver non-convergence in
  `NewtonReport` (currently not a field) is a maintainer decision.

### D9 — RICHARDS discretisation choices (the physics core)

- **Time:** backward (implicit) Euler, first-order. **Space:** cell-centred
  two-point flux (TPFA) finite volume. Primary unknown: liquid pressure per cell.
- **Mobility upstream-weighted** per face (kr from the upstream cell by flow
  potential) — standard and the source of Jacobian asymmetry. Density is the
  arithmetic face mean; permeability isotropic homogeneous in v1.
- **Jacobian is assembled NUMERICALLY** by local finite differences of the
  residual over the two-point stencil (O(N·stencil), not O(N²)). Rationale: it
  matches the residual by construction (good Newton convergence), and it
  side-steps the singular analytical `dk_r/dSe → ∞` of van Genuchten–Mualem at
  full saturation (D6). **REVIEW:** an analytical Jacobian would be faster and is
  worth a follow-up bead, but numerical is the safer first cut. Perturbation
  `h = 1e-8·(1+|p|)`.
- **Capillary pressure** `p_c = p_gas − p_l` with a fixed reference gas pressure
  (default atmospheric). **Gravity** default `9.80665 m/s²` in `−z`; potential
  `Φ = p + ρ_face·g·z`. Unspecified boundaries are **no-flow**.
- **Adaptive timestepping:** grow ×1.5 on a converged step (capped at `max_dt`),
  cut ×0.5 and retry on nonlinear failure, abort below `min_dt = 1e-6·initial_dt`.
  **REVIEW:** these factors are AI-chosen defaults, not tuned/validated.
- **Verification done:** closed-form saturated 1D steady state (linear profile,
  matches to < 5 Pa) and a stationary closed no-flow box. **Validation NOT done.**

### D10 — `NeumannFlux` sign convention

- `BoundaryConditionKind::NeumannFlux(q)`: `q` is the normal Darcy velocity
  (m/s), **positive = inflow** into the domain. `q = 0` (or an unspecified
  boundary) is a no-flow wall. **REVIEW:** confirm this matches the intended
  PFLOTRAN-style convention before any deck is shared as "PFLOTRAN-compatible".

---

### D11 — Solute transport translation (op-v6s.11)

- **Segregated / sequential coupling.** Transport is solved on a *frozen* flow
  field (Darcy face fluxes + water content) exported from a RICHARDS solve
  (`RichardsProblem::flow_field`). The passive solute does not feed back into
  flow (conservative, non-reactive) — so the transport system is **linear** in
  concentration and is assembled + solved directly with one BiCGStab call, no
  Newton. **REVIEW:** fully-coupled flow+transport is a later option; segregated
  is the standard first cut.
- **First-order upwind advection.** Simple and monotone (no over/undershoot),
  but adds numerical diffusion. TVD / higher-order flux limiting is deferred
  (the bead title mentions TVD) — flag for a follow-up bead. Dispersion is a
  symmetric two-point Fickian term with effective coefficient
  `D_face = D_molecular + alpha_L * |v_darcy|`.
- **Implicit Euler in time**, matching the flow discretisation.
- Verification (not validation): steady 1D advection–diffusion has a closed-form
  Peclet profile `c(x) = (1 - e^{Pe x/L})/(1 - e^{Pe})`; pure diffusion is linear;
  closed domains conserve solute mass. Deferred: the Celia/validation cases.
- **`InflowConcentration` is a true Dirichlet concentration** (dispersive
  coupling to `c_bc` applied regardless of flux sign; advection still upwinded),
  not an advection-only inflow. **REVIEW:** this strengthens the original spec —
  it is the Danckwerts-vs-Dirichlet inlet choice; confirm the intended inlet
  condition. The default no-BC boundary stays advective-outflow / zero-gradient.
- Longitudinal dispersion only (no transverse); `bc_by_location` is a
  `[Option<f64>;6]` that must widen if more transport BC kinds are added.
- **Coupling verified:** `RichardsProblem::flow_field` exports volumetric Darcy
  face fluxes + water content; an integration test runs RICHARDS then transports
  a tracer through the exported field (bounded, advecting, mass gained).

### D12 — TH energy transport (op-v6s.10)

- **One-way (weak) coupling** for v1: flow (RICHARDS) is solved first and its
  Darcy flow field is *frozen* while the energy equation is transported. The
  temperature does **not** feed back into flow (no buoyancy, no `mu(T)` effect on
  the flow). **REVIEW:** two-way coupling (density/viscosity feedback → buoyant
  flow) is the natural follow-up; the `properties::thermal` `rho(p,T)`/`mu(T)`
  needed for it already exist.
- Energy equation mirrors the `transport` module: volumetric heat capacity
  `C_v = theta_w rho_w c_w + (1-phi) rho_r c_r`, upwind advected enthalpy,
  two-point conduction with **`kappa_eff = phi kappa_w + (1-phi) kappa_r`**
  (saturated arithmetic mean — partial-saturation/series-parallel mixing
  deferred). Linear → one BiCGStab solve.
- Thermal properties are AI-fitted correlations, **not IAPWS-IF97** (real
  steam/water is `tampines-steam-tables`): density `rho0 exp(c dp - beta dT)`,
  viscosity Andrade `mu0 exp(B(1/T-1/T_ref))` with `B=1800 K`. Verification-only;
  constants are order-of-magnitude, not from a cited benchmark.

### D13 — Aqueous geochemistry (op-v6s.12)

- Equilibrium speciation only: law of mass action + component mass balance,
  log-concentration Newton with an analytic dense Jacobian (foam-basic-lib
  `SquareMatrix` LU). Verified against a closed-form weak-acid dissociation.
- **Ideal activities (gamma = 1)** — Debye-Hückel/Davies deferred. **No mineral
  phases, no kinetics, no charge-balance constraint, no H2O activity.** These are
  documented and are the obvious follow-ups for a real GIRT capability.
- **Not yet coupled to transport** — the reaction network is a standalone solver;
  the operator-split reactive-transport loop (transport step → per-cell
  speciation) is a follow-up.

### D14 — Upstream-parity wave: standalone gap modules (op-v6s.15.*, 2026-07-23)

Following the maintainer directive *"I believe there is still work to be done
compared to upstream — file beads and spawn agent fleets to continue,"* a
capability-gap epic (**op-v6s.15**) was opened against the full PFLOTRAN feature
set and worked as a fleet of **standalone new modules**, each built by an agent
that writes only its own `src/<module>/` directory (no edits to `lib.rs`,
`Cargo.toml`, or existing modules), with the main loop wiring + compiling
centrally. Rationale: disjoint new directories cannot collide, so the fleet is
safe to run in parallel; the main loop owns integration and is the single
compile authority.

Landed this wave (each **verification-only**, unit-tested, no human V&V):

- **`activity`** (op-v6s.15.1) — Ideal / Debye–Hückel / Davies coefficients.
- **`sorption`** (op-v6s.15.2) — Kd/Langmuir/Freundlich isotherms + Gaines–Thomas
  ion exchange; **linear Kd wired into `transport`** (retardation).
- **`decay`** (op-v6s.15.3) — Bateman chains + `exp(A·dt)` scaling-and-squaring;
  first-order decay **wired into `transport`**.
- **`microbial`** (op-v6s.15.4) — Monod / dual-Monod biodegradation (RKF45).
- **`eos_real`** (op-v6s.15.7) — IAPWS-IF97 liquid water via `tampines-steam-tables`.
- **`wells`** (op-v6s.15.12) — Peaceman well index (iso/anisotropic), BHP + rate
  control, source/sink, and Hydrostatic / SeepageFace / TimeVarying BCs.
- **`deck`** (op-v6s.15.10) — parser for a documented **subset of genuine
  PFLOTRAN keyword-block syntax** (Fortran `1.d-12` floats, time-unit
  normalisation), superseding the AI-invented `io` lite format of D7 for
  real-deck compatibility. Still a subset — unsupported cards are enumerated in
  the module header.

**Scope-splitting decision.** Three parity beads named advanced extensions
beyond what the core module implements; rather than leave them perpetually
"in progress", the implemented core was closed and each extension was filed as
its own follow-up bead:
- op-v6s.15.1 core closed → **op-s1h** Pitzer (high-ionic-strength brines).
- op-v6s.15.2 core closed → **op-gg7** surface complexation (CCM / diffuse-layer).
- op-v6s.15.7 core closed → **op-1y6** CO2 (Redlich–Kwong) + NaCl-brine EOS.

**Second fleet — all landed (verification-only, unit-tested):**
- **`pitzer`** (op-s1h) — Pitzer ion-interaction virial activity for brines;
  matches Pitzer & Mayorga (1973) tabulations to ~0.001 (8 tests).
- **`unstructured`** (op-v6s.15.8) — unstructured polyhedral FV grid with
  projected-normal TPFA transmissibility (18 tests); K-orthogonality limit
  documented.
- **`eos_co2_brine`** (op-1y6) — Redlich–Kwong CO2 (honest −14% near-critical vs
  Span–Wagner) + Batzle–Wang NaCl brine density/viscosity (10 tests).
- **`surface_complexation`** (op-gg7) — amphoteric protonation + metal binding
  with NEM / constant-capacitance / diffuse-layer electrostatics (9 tests). One
  test's premise was corrected (bare surface for the DLM potential-sign check —
  the metal complex's positive charge legitimately offsets deprotonation at high
  pH); model code unchanged.

Whole-crate state after both fleets: **224 lib tests green in release**, the
crate's `--lib` cross-compiles to `aarch64-linux-android`.

**Third fleet — the coupled-physics gaps, done as NEW composed modules** (not
edits to `flow`/`energy`/`multiphase`, which stay as-is — the same disjoint-module
discipline, each new module *reads* the existing solver/property APIs and *reuses*
them read-only):
- **`general_mode`** (op-v6s.15.5) — PFLOTRAN GENERAL mode as a non-isothermal
  nb=3 (p_l, s_l, T) block system extending the isothermal two-phase solver with
  an energy balance; T couples back through rho_l(T)/mu_l(T). Water/gas/energy
  conservation, isothermal-limit reduction, and thermal coupling verified (6
  tests). Simplified GENERAL (no inter-phase partitioning / phase change / latent
  heat), flagged.
- **`thermal_convection`** (op-v6s.15.6) — two-way buoyancy: Boussinesq rho(T)
  drives Darcy flow which advects heat (nb=2 p,T). The conductive limit
  (beta=0 → no flow), the Rayleigh-number formula, Newton-under-heating, and
  input validation verify (4 tests). **The two strongly-convecting HRL onset
  tests are `#[ignore]`d honestly**: the finite-difference block Jacobian does
  not robustly converge the buoyancy-coupled system at the high permeability
  (k~1.4e-9) supercritical Rayleigh numbers need — the residual stalls near
  ‖F‖~4e-2. This is a real solver-robustness limit, not a tuning typo (it recurs
  even in the stably-stratified case), so rather than fabricate a green it was
  split to a follow-up bead (**op-3tt**, analytic Jacobian / adaptive timestep).

Whole crate after three fleets: **234 lib tests pass (2 ignored)**, `--lib`
cross-compiles to `aarch64-linux-android`.

**Remaining parity gaps, all outside the pure-Rust / Android-buildable envelope
or a documented follow-up:** op-v6s.15.9 (MPI scale-out) and op-v6s.15.11 (HDF5
I/O) — both conflict with the Android / no-C-toolchain rule and are tracked as
known-deferred; **op-3tt** (robust convecting solve) is the one open
implementation follow-up from this wave. The extension beads op-s1h (Pitzer),
op-gg7 (surface complexation), and op-1y6 (CO2/brine) all landed in the second
fleet. The integration follow-ups (activity → geochemistry speciation;
eos_real → RICHARDS water density) remain open main-loop tasks.

**Integration follow-ups still open** (the standalone modules exist but are not
yet threaded into the hot loops): `activity` → geochemistry speciation
(currently ideal `gamma = 1`, needs per-species charges — a public-API change);
`eos_real` → RICHARDS water density. Both are numerically load-bearing and are
left as bounded main-loop tasks with their own commits.

### D15 — MPI scale-out via a new pure-Rust crate (op-v6s.15.9, 2026-07-23/24)

Maintainer directive: *"take the mpi upstream and start converting it to rust"* —
MPICH subset + shared-memory transport chosen (via `AskUserQuestion`). Rather than
bind a C MPI (which would break the Android/no-C rule), a **new workspace crate
`outram-park-mpi`** was created: a pure-Rust translation of an MPICH subset over a
shared-memory *threads-as-ranks* transport (own epic **op-erl**). It provides the
MPI-3 surface pflotran needs — communicators (world + `dup`/`split`), datatypes,
point-to-point (`send`/`recv`/`isend`/`irecv`), and collectives
(`barrier`/`broadcast`/`reduce`/`all_reduce`/`scatter`/`gather`/`all_gather` with
`ReduceOp`) — all Android-clean (std threads only), ~28 tests.

pflotran's **`decomposition`** module (op-v6s.15.9, first slice) then depends on
`outram-park-mpi` and demonstrates the real pattern: a balanced 1-D
[`Decomposition1D`] partition + nearest-neighbour [`exchange_halo`], with a
distributed Jacobi stencil that is **bit-identical to the serial reference across
rank counts {1,2,3,4,6}**. This proves the MPI transport drives a correct
distributed stencil end-to-end.

**Deliberately scoped as a first slice (flagged for review):** the halo exchange
is the foundation, **not** a fully MPI-parallel Newton solve — the implicit
RICHARDS/transport Jacobian is still assembled and solved serially per rank.
Distributing the global linear solve (parallel matrix-vector products + a
distributed Krylov method) and multi-dimensional / unstructured partitioning are
follow-ups. Groups/topologies for the MPI crate are bead **op-er2**; optimised
collective algorithms and a TCP multi-node transport remain under op-erl.

## Deferred to next week (2026-07-22 maintainer directive)

All **Celia-1990 and validation-case work is paused** this week; the focus is
PFLOTRAN **translation**. Deferred beads (noted in the store):
- op-v6s.9 (V&V strategy), op-v6s.9.2 (Celia benchmark case), op-v6s.9.3
  (source reference + validation gate), op-v6s.9.4 (mass-conservation
  diagnostic — first cut already landed).
Verification tests (analytical/MMS) continue as normal — they are part of
translation quality, distinct from the deferred *validation-case* work.

## v1 completion status (2026-07-22)

**Done and tested (verification-only):** the RICHARDS vertical slice — grid
(op-v6s.5), properties (op-v6s.7), I/O (op-v6s.6), Newton–Krylov solver
(op-v6s.4) + the foam-basic-lib `krylov` module, and the RICHARDS flow mode
end-to-end (op-v6s.8). Test suite: 50 unit + 3 integration + 1 regression + 3
verification tests, all green in release; both crates cross-compile to Android;
the dependent foam crates still build.

**Verification evidence (measured, not fabricated):**
- MMS spatial convergence: **observed order 2.000** on the saturated operator.
- Hydrostatic equilibrium under gravity: analytical match to **9.8e-5 Pa**.
- Saturated steady state: exact linear profile to machine zero.

**Explicitly NOT done — the human-review / follow-up backlog:**
1. **Validation (op-v6s.9, the big one).** No comparison to published PFLOTRAN
   gold-files or experimental data has been done — only verification. A
   canonical transient benchmark (e.g. Celia et al. 1990 infiltration) with a
   *sourced* reference solution is required before any validity claim. The
   reference data must come from open literature per the workspace data policy;
   it was not available to the AI in this environment.
2. **Upstream license byte-for-byte re-check (op-v6s.1)** before publish.
3. **Real PFLOTRAN input-deck syntax** — the current `io` format is an
   AI-designed lite subset (D7).
4. **Analytical Jacobian** as a faster alternative to the numerical one (D9);
   **wgpu acceleration** (D2), Android-gated, once the CPU path is profiled.
5. **Later flow modes (op-v6s.10–.14)**: TH (water+energy), conservative solute
   transport, reactive geochemistry (GIRT), GENERAL multiphase, parallelism —
   all out of v1 scope, not started.
6. **Human sign-off** on both README "Bookkeeping status" axes — still ❌; an AI
   must not flip them (RESPONSIBLE_USE.md). Every entry above is untrusted AI
   draft until then.
