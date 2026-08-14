# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `outram_park_fork_thermochimica`

# outram-park-fork-thermochimica

Independent pure-Rust fork/translation of ORNL Thermochimica (BSD-3) — molten-salt Gibbs-energy-minimisation thermochemistry (fission-product speciation, redox, solubility) for the MSRE digital twin. SCAFFOLD: no human V&V. Not affiliated with ORNL or the Thermochimica project.

> **⚠️ Scaffold — unverified until validated.** Skeleton crate; the port is
> in progress (MSRE digital-twin epic `op-6w0`). No human V&V. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork.

## Modules

- [`gem`] — the CALPHAD **Gibbs-energy-minimisation core**: an
  element-potential / Lagrange-multiplier minimiser over multiple phases
  with ideal and binary Redlich-Kister solution models. This is the first
  ported piece of Thermochimica (bead `op-6w0.1`); the ChemSage `.dat`
  parser and the sublattice / quasichemical solution models are not yet
  ported (see the module's scope notes).

## Modules

## Module `gem`

# `gem` — CALPHAD Gibbs-energy-minimisation core

A pure-Rust Gibbs-energy **minimiser** (GEM) in the CALPHAD framing that
ORNL's Thermochimica uses: given a set of **phases**, each holding chemical
**species** with a standard molar Gibbs energy `g°_i(T,P)` plus an activity
(ideal or Redlich-Kister non-ideal) term, and a feed of chemical **elements**
(atom abundances), it finds the equilibrium phase amounts and within-phase
species mole fractions that minimise the total Gibbs energy `G` subject to
element mass-balance constraints — the **element-potential / Lagrange-
multiplier** method (`µ_i = Σ_j a_ij λ_j`). Phases may **vanish** (a
candidate that is not thermodynamically stable collapses toward zero).

This crate is a sibling of
`outram-park-fork-dwsim-libs::thermo::gibbs_multiphase` (the DWSIM-derived
multi-phase minimiser); it re-implements the same *numerical* idea in the
CALPHAD idiom of Thermochimica, and **does not depend on** that crate.

## The minimisation problem

Minimise the dimensionless total Gibbs energy over `P` phases and their
species (raw `f64`, SI; see *Units* below):

```text
G/RT = Σ_p Σ_s n_ps · (µ_ps / RT),
µ_ps/RT = g°_ps/(RT) + ln x_ps + ln γ_ps  [+ ln(P/P°) if phase p is a gas]
```

where `n_ps` is the moles of species `s` in phase `p` \[mol\],
`x_ps = n_ps / Σ_l n_pl` its within-phase mole fraction \[-\], `g°_ps` its
standard molar Gibbs energy in that phase \[J/mol\], and `γ_ps` its activity
coefficient \[-\] from the phase's [`SolutionModel`] (`γ = 1` for an ideal
phase; a Redlich-Kister term otherwise). The atom matrix `a_ks` (atoms of
element `k` per formula unit of species `s`) is a property of the species.
Subject to, for every element `k`,

```text
Σ_p Σ_s a_ks · n_ps = b_k     (atom balance over ALL phases),   n_ps ≥ 0,
```

with atom targets `b_k` set by the feed. The Lagrange stationarity condition
is the **element-potential** relation — one potential `π_k = λ_k/RT` per
element, shared by every phase:

```text
µ_ps/RT = Σ_k a_ks · π_k     for every species s present in phase p.
```

Equating this across two phases sharing a species gives equal chemical
potential (phase equilibrium); for a species over its own single-species
condensed phase it gives the classic activity/vapour relation.

## The element-potential Newton iteration (pure Rust, no BLAS)

At a strictly-positive estimate `n_ps` with phase totals `n_p = Σ_s n_ps`,
the `(M+P)×(M+P)` saddle-point system (upstream `GEMNewton.f90`) for the `M`
element potentials `π_k` and `P` per-phase log-total corrections
`u_p = Δ ln n_p` is

```text
Σ_j R_kj π_j + Σ_p Q_kp u_p = (b_k − q_k) + S_k     (one row per element k)
Σ_j Q_jp π_j                = F_p                   (one row per phase p)
```

with `q_k = Σ_p Σ_s a_ks n_ps`, `Q_kp = Σ_s a_ks n_ps`,
`R_kj = Σ_p Σ_s a_ks a_js n_ps`, `S_k = Σ_p Σ_s a_ks n_ps (µ_ps/RT)`, and
`F_p = Σ_s n_ps (µ_ps/RT)`. The species correction is

```text
δ_ps = Σ_k a_ks π_k + u_p − µ_ps/RT,
n_ps ← n_ps · exp(ω · δ_ps),   ω ≤ min(1, max_step / max|δ|).
```

The `[[R, Q],[Qᵀ, 0]]` block structure uses the **ideal** (`ln x`) Hessian.
The composition-dependent activity coefficient `ln γ_ps` enters only through
the right-hand side (`µ_ps/RT` carries it) and is **held frozen within each
linearisation**, then recomputed from the updated composition next step — a
successive-substitution / quasi-Newton treatment of non-ideality. Because
`δ_ps = 0` at the fixed point forces `µ_ps/RT = Σ_k a_ks π_k` (the exact
stationarity condition **including** `ln γ`), the converged answer is correct
regardless of the frozen-`γ` Hessian approximation; only the convergence
*rate* is affected (upstream uses the full analytic Hessian; this first pass
does not — documented scope below).

**Monotone merit line search** (upstream `GEMLineSearch.f90`). ω is reduced
by backtracking on the merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` (penalty
[`MERIT_PENALTY`]): the largest halving of the damped `ω₀` whose trial `Φ`
does not rise is taken, so `Φ` is non-increasing by construction and, since
its atom-violation term → 0 at convergence, `Φ → G/RT`. The multiplicative
update keeps every `n_ps > 0` automatically; the `(b_k − q_k)` residual
drives atom balance back onto target each step.

**Vanishing phases.** A phase whose total falls below
[`GemOptions::phase_floor`] is *frozen* (its row `M+p` becomes `u_p = 0`) to
keep the saddle system non-singular; its species still update from the shared
`π` and regrow if they become favourable, else stay near zero. This is how a
candidate phase that is not stable collapses out (V&V `collapse_*` test).

## Units — documented raw `f64` (SI)

Raw `f64` in SI throughout the inner loop: temperature `T` \[K\], pressure
`P` and reference `P°` \[Pa\], standard Gibbs energies `g°_ps` and
Redlich-Kister coefficients `L_v` \[J/mol\], moles \[mol\]; mole fractions,
activity coefficients, and element potentials are dimensionless. `R` is the
CODATA molar gas constant [`R`] \[J/(mol·K)\].

## Design (workspace CLAUDE.md)

Enum dispatch, **no `dyn` / `Box` / lifetime params**: the per-phase mixing
model is the closed enum [`SolutionModel`]; the system, options, result, and
errors are plain owned structs / enums. `#![forbid(unsafe_code)]` (crate
level). Raw `f64` maths inside; the `(M+P)` linear system is solved by
in-crate Gaussian elimination ([`solve_linear`]) — no BLAS/LAPACK/FFI, so
this compiles on Android / Termux like the rest of the crate.

## Honest scope — untrusted AI-assisted draft, pending human V&V

This is the **GEM core only**, not the whole of Thermochimica. Deliberately
**out of scope** in this first pass:
- **The ChemSage / FactSage `.dat` file parser** (upstream `parser/`). Data
  is supplied programmatically via [`GemSystem::new`] + [`GemSystem::minimize`]
  (hand-coded `g°`, atom matrix, feed). No file format is read.
- **Solution models beyond ideal + binary Redlich-Kister.** Upstream's QKTO,
  SUBG/SUBI/SUBL/SUBM (sublattice / modified-quasichemical) models, magnetic
  contributions, and Muggianu/Kohler ternary interpolation are **not**
  ported. [`SolutionModel::RedlichKister`] handles binary interaction terms
  only (ternary+ interaction coefficients are not implemented).
- **The full analytic non-ideal Hessian** and automatic miscibility-gap /
  new-phase detection (upstream `CheckMiscibilityGap.f90`,
  `Subminimization.f90`). The candidate phase set is caller-supplied; an
  unlisted phase is never discovered. Non-ideality uses a frozen-`γ`
  quasi-Newton step (above).
- **Leveling / initial phase-assemblage estimation** (upstream `setup/`,
  `InitGEMSolver.f90`): here the feed seeds the iteration directly.
- **Phase-assemblage management** (upstream `CheckPhaseAssemblage.f90`,
  `AddSolnPhase.f90`, `RemPureConPhase.f90`, …). The number of
  *simultaneously active* phases must not exceed the number of **independent
  components** (linearly independent element rows) — the Gibbs phase rule as
  it appears in the element-potential saddle system, whose phase block
  otherwise loses column rank. Over-determined candidate sets return
  [`GemError::SingularSystem`] rather than being repaired by adding/removing
  phases from the assemblage. A candidate phase that is simply *unstable*
  still collapses out (vanishing phases, above); what is not handled is more
  *stable* candidate phases coexisting than the component count allows.

**Verified, not validated.** The V&V tests below are analytic
**verification** (pure-component trivial limit, ideal Nernst partition,
Redlich-Kister activity-coefficient identity, and a molten-fluoride
LiF-BeF₂ ideal-mixing mass/charge-balance identity) against closed-form
results, **not** validation against a measured multi-phase equilibrium
dataset. AI-assisted draft material, **untrusted until human-reviewed** per
the crate `CLAUDE.md`. Not for nuclear facility operation, reactor control,
safety-critical, or licensing decisions. Independent OUTRAM PARK fork.

```rust
pub mod gem { /* ... */ }
```

### Types

#### Struct `BinaryInteraction`

One binary Redlich-Kister interaction term between two species of a phase.

Contributes to the molar excess Gibbs energy of mixing (upstream
`CompExcessGibbsEnergyRKMP.f90`, binary term):

```text
g^ex_pair = x_i · x_j · Σ_{v=0}^{N} L_v · (x_i − x_j)^v      [J/mol]
```

where `x_i`, `x_j` are the within-phase mole fractions \[-\] of the two
interacting species and `L_v` \[J/mol\] is the order-`v` mixing coefficient.
`v = 0` alone is a **regular** solution (symmetric); adding `v = 1` makes it
**subregular** (asymmetric); higher orders extend the polynomial. Coefficients
are treated as constants at the solve temperature — a caller wanting
`L_v(T) = a + bT` evaluates it before building the model (temperature-
dependent `L` storage is out of scope here).

```rust
pub struct BinaryInteraction {
    pub species_i: usize,
    pub species_j: usize,
    pub l_coeffs: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `species_i` | `usize` | Within-phase index of the first interacting species `i` (0-based). |
| `species_j` | `usize` | Within-phase index of the second interacting species `j` (0-based),<br>`j ≠ i`. |
| `l_coeffs` | `Vec<f64>` | Redlich-Kister coefficients `[L_0, L_1, …, L_N]` \[J/mol\], lowest order<br>first. Empty ⇒ no contribution. `L_0` is the regular term; `L_1` the<br>first subregular correction. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BinaryInteraction { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BinaryInteraction) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SolutionModel`

Mixing (solution) model of one phase — the composition term in each species'
chemical potential (enum dispatch, no `dyn`; the set is closed).

All variants give `µ_ps/RT = g°_ps/RT + ln x_ps + ln γ_ps [+ ln(P/P°) if
gas]`; they differ in `γ_ps` (activity coefficient) and whether the gas
pressure term is present:
- ideal models have `γ = 1` (activity `= x`);
- [`SolutionModel::RedlichKister`] adds a non-ideal `ln γ` from its binary
  interaction terms.

A **single-species** ideal or Redlich-Kister phase has `x = 1`, hence unit
activity — an exact **pure condensed** (stoichiometric) substance, the
CALPHAD "pure condensed phase".

```rust
pub enum SolutionModel {
    IdealGas,
    IdealSolution,
    RedlichKister {
        interactions: Vec<BinaryInteraction>,
    },
}
```

##### Variants

###### `IdealGas`

Ideal **gas** solution: `γ = 1`, plus the pressure term `ln(P/P°)`. Drives
the pressure response of any mole-changing reaction.

###### `IdealSolution`

Ideal **condensed** solution (molten salt / liquid / solid solution):
`γ = 1`, no pressure term (condensed molar volume neglected).

###### `RedlichKister`

Non-ideal condensed solution with binary **Redlich-Kister** excess terms.
`γ ≠ 1`; no pressure term. Ternary+ interactions and Muggianu
interpolation are not modelled (see module scope notes).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `interactions` | `Vec<BinaryInteraction>` | Binary interaction terms (any number; may target overlapping pairs). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SolutionModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SolutionModel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PhaseInput`

One phase's static description supplied to [`GemSystem::new`]: its mixing
model, species labels, and the atom sub-matrix mapping its species onto the
*global* element list.

The atom matrix has `M` rows (one per global element, in the system's element
order) and `n_species` columns; `atom_matrix[k][s]` is the number of atoms of
element `k` in one formula unit of species `s` of this phase (non-negative,
finite; typically small integers or simple fractions).

```rust
pub struct PhaseInput {
    pub model: SolutionModel,
    pub species_names: Vec<String>,
    pub atom_matrix: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `model` | `SolutionModel` | Mixing model for the phase. |
| `species_names` | `Vec<String>` | Species labels (identification only). Length = the phase's `n_species`. |
| `atom_matrix` | `Vec<Vec<f64>>` | `M` rows × `n_species` columns: atoms of each global element per formula<br>unit of each species. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PhaseInput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PhaseInput) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GemOptions`

Convergence / iteration controls for [`GemSystem::minimize`].

All tolerances are dimensionless. [`Default`]: `tol = 1e-10`,
`max_iter = 5000`, `max_step = 2.0`, `mole_floor = 1e-12`,
`phase_floor = 1e-11`.

```rust
pub struct GemOptions {
    pub tol: f64,
    pub max_iter: usize,
    pub max_step: f64,
    pub mole_floor: f64,
    pub phase_floor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tol` | `f64` | Convergence tolerance \[-\]. Stops when both the KKT stationarity residual<br>`max|δ_ps|` (complementarity-aware) and the worst relative atom-balance<br>residual fall below this value. |
| `max_iter` | `usize` | Maximum Newton iterations before [`GemError::NotConverged`]. Larger than<br>the ideal-only sibling's default because the frozen-`γ` step converges<br>more slowly for strongly non-ideal phases. |
| `max_step` | `f64` | Maximum per-step log-correction `max|ω·δ_ps|` \[-\]. `ω ≤<br>min(1, max_step/max|δ|)` caps how far any species moves per step in<br>`ln n` space. Typical `2.0` (factor `e² ≈ 7.4`). |
| `mole_floor` | `f64` | Lower floor \[mol\] on the *initial* working moles of any (phase, species)<br>the feed gives as zero, so `ln n` is finite at the start. Must be > 0. |
| `phase_floor` | `f64` | Phase-total floor \[mol\]. A phase below this is *frozen* (`u_p = 0`) to<br>keep the saddle system non-singular as it vanishes; its species still<br>update from the shared potentials. Must be > 0 and ≥ `mole_floor`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GemOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GemOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GemResult`

Result of a converged Gibbs-energy minimisation.

Per-phase quantities are indexed `[phase][species]` in the build order;
element potentials are an `M`-vector in element order.

```rust
pub struct GemResult {
    pub moles: Vec<Vec<f64>>,
    pub mole_fractions: Vec<Vec<f64>>,
    pub phase_totals: Vec<f64>,
    pub activity_coefficients: Vec<Vec<f64>>,
    pub element_potentials: Vec<f64>,
    pub gibbs_energy_rt: f64,
    pub gibbs_energy: f64,
    pub descent_merit_history: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `moles` | `Vec<Vec<f64>>` | Equilibrium species amounts `n_ps` \[mol\], `moles[p][s]`. A collapsed<br>phase reports species amounts near `mole_floor` or below. |
| `mole_fractions` | `Vec<Vec<f64>>` | Within-phase mole fractions `x_ps = n_ps / Σ_l n_pl` \[-\],<br>`mole_fractions[p][s]`. For a collapsed phase (total < `phase_floor`)<br>these are reported as `0`. |
| `phase_totals` | `Vec<f64>` | Total moles in each phase `n_p = Σ_s n_ps` \[mol\], one per phase. |
| `activity_coefficients` | `Vec<Vec<f64>>` | Activity coefficients `γ_ps` \[-\], `activity_coefficients[p][s]` (`1` for<br>ideal phases). Activity of species `s` is `γ_ps · x_ps`. |
| `element_potentials` | `Vec<f64>` | Shared element potentials `π_k = λ_k/RT` \[-\], one per element. At the<br>solution every present species satisfies `µ_ps/RT = Σ_k a_ks π_k`. |
| `gibbs_energy_rt` | `f64` | Dimensionless total Gibbs energy `G/RT = Σ_p Σ_s n_ps (µ_ps/RT)` \[mol\]<br>at the returned composition (the minimised objective). |
| `gibbs_energy` | `f64` | Total Gibbs energy `G = RT · (G/RT)` \[J\], relative to the supplied<br>`g°_ps` reference states. |
| `descent_merit_history` | `Vec<f64>` | Line-search merit `Φ = G/RT + μ·Σ_k|b_k − q_k|` \[mol\] at each iteration<br>plus the final value. **Monotonically non-increasing by construction**<br>(the V&V monotone-descent witness); `Φ → G/RT` as the violation → 0. |
| `iterations` | `usize` | Number of Newton iterations performed. |
| `converged` | `bool` | `true` if both tolerances in [`GemOptions`] were met. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GemResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GemResult) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `GemError`

Errors from constructing or solving a [`GemSystem`].

```rust
pub enum GemError {
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    InvalidInput {
        what: &'static str,
        value: f64,
        positive: bool,
    },
    SingularSystem,
    NotConverged {
        iterations: usize,
        max_correction: f64,
        atom_residual: f64,
    },
}
```

##### Variants

###### `DimensionMismatch`

A supplied slice / matrix had the wrong length for the system dimensions.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was mis-sized. |
| `expected` | `usize` | Expected length. |
| `got` | `usize` | Actual length. |

###### `InvalidInput`

An input value was non-finite, or a required-positive value was ≤ 0.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input was invalid. |
| `value` | `f64` | Offending value. |
| `positive` | `bool` | Whether the field additionally had to be strictly positive. |

###### `SingularSystem`

The saddle-point system was singular even after freezing vanished phases —
typically a rank-deficient atom matrix (a duplicated element row, an
element no species contains, or two phases proportional in element space).

###### `NotConverged`

The iteration did not meet both tolerances within `max_iter`. Carries the
worst residuals so the caller can judge how close it got.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed (`= max_iter`). |
| `max_correction` | `f64` | Final complementarity-aware `max|δ_ps|` \[-\]. |
| `atom_residual` | `f64` | Final worst relative atom-balance residual \[-\]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GemError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GemError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GemSystem`

A multi-phase reacting system: a shared element list plus `P` phases, each
with its own species, mixing model, and atom sub-matrix.

Carries only the *combinatorial* data (which phases exist, made of what);
the thermodynamics (`g°`, `T`, `P`, feed) are passed per solve to
[`Self::minimize`], so one system can be reused across states.

```rust
pub struct GemSystem {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(element_symbols: &[&str], phases: &[PhaseInput]) -> Result<Self, GemError> { /* ... */ }
  ```
  Build a system from the global element list and one [`PhaseInput`] per

- ```rust
  pub fn n_elements(self: &Self) -> usize { /* ... */ }
  ```
  Number of chemical elements `M`.

- ```rust
  pub fn n_phases(self: &Self) -> usize { /* ... */ }
  ```
  Number of phases `P`.

- ```rust
  pub fn n_species(self: &Self, p: usize) -> usize { /* ... */ }
  ```
  Number of species in phase `p`. Panics if `p` is out of range.

- ```rust
  pub fn element_symbols(self: &Self) -> &[String] { /* ... */ }
  ```
  Element symbols, in element order.

- ```rust
  pub fn species_names(self: &Self, p: usize) -> &[String] { /* ... */ }
  ```
  Species names of phase `p`, in species order. Panics if `p` out of range.

- ```rust
  pub fn element_abundance(self: &Self, moles: &[&[f64]]) -> Result<Vec<f64>, GemError> { /* ... */ }
  ```
  Total moles of each element supplied by a per-phase composition:

- ```rust
  pub fn minimize(self: &Self, gibbs_formation: &[&[f64]], temperature: f64, feed: &[&[f64]], pressure: f64, p_ref: f64, options: &GemOptions) -> Result<GemResult, GemError> { /* ... */ }
  ```
  Minimise `G/RT` over all phases subject to shared atom balance, returning

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> GemSystem { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GemSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `R`

CODATA-2018 molar gas constant `R` \[J/(mol·K)\].

Used to non-dimensionalise Gibbs energies (`g°/RT`) throughout the solver.
Exact by the 2019 SI redefinition (`R = N_A · k_B`).

```rust
pub const R: f64 = 8.314_462_618_153_24;
```

