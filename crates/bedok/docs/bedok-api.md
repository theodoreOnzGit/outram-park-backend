# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `bedok`

**BEDOK** — 3-D nodal-diffusion neutronics coupled to thermal hydraulics.

A Rust translation of Than Yan Ren's (SNRSI) MATLAB implementation, ported
from the `main_exec_diff3d_standalone` snapshot.

# How this crate is laid out

**One module per `.m` file, flat, named after the original.** That is the
whole organising principle and it is deliberate: `th_solverxyz.rs` is
`th_solverxyz.m`, function for function. There is no `nodal/`, no `th/`, no
`coupling/` — the MATLAB has no such folders, and the point of this
translation is that a reader can hold the `.m` file and the `.rs` file side
by side.

Two consequences worth knowing before you go looking for things:

- **Module names are `snake_case` even where the original was not.**
  `makegradDxyz.m` becomes `makegrad_dxyz`, because Rust warns on
  non-snake-case module names. The original filename is always named in the
  module's own doc comment.
- **Three modules have no `.m` counterpart** — [`matlab`], [`types`] and
  [`error`]. They carry the MATLAB container semantics, the loosely-typed
  structs, and the `error(...)` conditions respectively. Each says so at the
  top. Everything else is a translation.

# Indexing — read this before editing any module

**The port is 0-based.** The reference's 1-based index arithmetic is
converted, so

```text
idx = (g-1)*es + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz     % MATLAB
```

becomes

```text
idx =  g*es    +  ix*maxiy*maxiz    +  iy*maxiz    + iz     // Rust
```

Storage stays **column-major** in [`matlab::Array2`], [`matlab::Array3`] and
[`matlab::Array4`], because the reference does linear indexing into
multi-dimensional arrays and the layout is therefore observable.

## Two places the conversion is not mechanical

Both are documented where they occur; they are the things to check first if
an index bug shows up.

- **[`convert_grid3d`] used `0` as a "no material here" sentinel**, which
  only worked because MATLAB indices start at 1. Going 0-based makes `0` a
  valid index. That map is now `Option<usize>` — `None` for absent — and the
  two sites that tested `== 0` test `is_none()`.
- **[`convertindexc2d`] keeps 1-based arithmetic internally.** It maps
  between two index spaces whose definition — the `(2n+1)` half-index grid
  interleaving cell centres and edges — is stated in 1-based terms, where the
  offsets are load-bearing rather than incidental. It converts at the
  boundary instead, so callers still see 0-based indices.

Material identifiers are a third thing worth knowing about, though it is not
an indexing change: `whichsigma` stores **1-based material numbers with `0`
meaning void**, straight from the benchmark composition CSVs. A node holding
material `m` reads row `m - 1` of the cross-section table. See
[`calcdiffvalues3d`].

# Translation policy — no silent repairs

The MATLAB is unfinished, and the snapshot is terminal. Defects are
translated **as they are**, with the defect described in the doc comment of
the item that carries it and a test that pins the wrong behaviour so a later
fix is a visible, deliberate change. Repairs belong in a separate stage with
before/after numbers, never mixed into translation. The reasoning is in
the crate README, "Translation policy": a translation carrying well-meant fixes
cannot be debugged against a benchmark, because a disagreement can no longer
be attributed.

Defects recorded so far live in `docs/bedok-reference-defects.md` and in
these places in the code:

- [`convertindexc2d`] — the mode 1 → mode 2 → mode 1 round trip is **not**
  the identity; the two directions disagree by an off-by-one in the forward
  row calculation. Found by running the tests, not by reading the code.
- [`handle3dcoords`] — the generic branch assigns `params.maxix` to
  `maxi3`.
- [`convert_grid3d`] — precursor indices collide for `Nc > 1`.
- [`geometry_ends3d`] — only the first contiguous run per grid line is
  found.
- [`convertsparsekey3d`] — the diagnostic decode is hard-coded to a
  17×17×19 grid.
- [`calc_bucklingxyz`] — the cache fingerprint is three sums and three
  non-zero counts, which cannot separate every distinct cross-section set;
  a collision silently reuses the wrong cached coefficients.
- [`calc_abefghxyz`] — `abefgh` loses precision to cancellation as `alpha`
  goes to zero, with no series fallback.
- [`makesigmadfxyz`] — in half-index mode the `iz` loop bound is `maxiz`
  where the other two axes use `m*max…`, so the upper half of the core
  silently gets no cross sections. Latent: every call site passes mode 1.
- [`makegrad_dxyz`] — a **fuelled** node outside its line's `[low, high]`
  bounds is skipped by the `z` pass, keeps the pre-filled identity `1`, and
  then has `y` and `x` accumulated on top, leaving a spurious `+1` on its
  diagonal. Reachable via [`geometry_ends3d`]'s first-contiguous-run
  limitation. Confirmed by test.
- [`calc_sanodalxyz`] — the same root cause with the opposite symptom: the
  `y`/`x` passes accumulate into a diagonal slot the `z` pass was supposed
  to create, so a node `z` missed **aborts** rather than computing something
  wrong. Confirmed by test.
- [`sanodaldiffusion_solverxyz`] — a nodal-update interval of **1 does not
  converge**, and the built-in default *is* 1 for any mesh whose three
  extents sum to 10 or fewer. Confirmed by test at three mesh sizes.
- [`diffusion_solverxyz`] — the empty-grid compaction is unreachable
  (`keychange` is a hard-coded `0`), and four `writematrix` CSV dumps run
  unconditionally on every call.
- Both flux solvers — a bailed-out iteration returns the **previous**
  pass's `k_eff` and residuals, and says nothing about having bailed. The
  translation adds a `Termination` value rather than reproducing the
  silence.

# Two deliberate departures from the reference, both in the flux solvers

Everything else in this crate reproduces the reference exactly. These two do
not, and the reasoning is recorded in `docs/bedok-reference-defects.md`
alongside defects D1-D7:

- **The diagnostic CSV writes are returned, not written.** Both solvers
  compute the same symmetry maps the reference dumps to disk and hand them
  back in a `Diagnostics` struct. A library that writes files as a side
  effect of being called cannot be used concurrently or tested cleanly, and
  the physics is identical either way.
- **The `gmres`/`ilu` branch is not translated.** It is selected at
  `philenf >= 50_000_000`, which is unreachable for any runnable problem, so
  an ILU and a restarted GMRES written for it could never be verified
  against the reference. Both solvers return
  [`error::BedokError::IterativeSolveNotTranslated`] instead, which names
  the threshold.

# Status — INCOMPLETE, but building and tested

This is a rewrite in progress. **all 50 `.m` files are translated**: the
utility and indexing layer, all fourteen SANM nodal files, both flux
solvers, the whole thermal-hydraulics layer, **both** coupling drivers
(steady and transient), and five benchmark cases ([`iaea3ds`],
[`neacrpd1`], [`neacrpd1t`], [`neacrpa2`], [`neacrpa2t`] and
[`neacrpa1t`]), the critical-boron search, and the 2-D legacy case. [`iapws_if97`] is partially done — regions
1, 2 and 4 with the backward and transport entry points, but **not region
3**, which caps everything at 16.5292 MPa.

**Three of the fifty do not land as modules**, and each says why in its own
header:

- `main_exec_diff3d.m` and `run_neacrpd1t.m` are **scripts**, not functions.
  They become `examples/`, which is also the entry point the workspace's
  human-interface rule asks for.
- `plotreactor3dcolour.m` is half data preparation and half MATLAB figure
  emission. The first half is [`plotreactor3dcolour`]; the rendering is not
  translated, on the same reasoning as the CSV policy.

One more is translated but **cannot be run**: [`geom2dxycase1`] builds a 2-D
case, and every solver in the snapshot is 3-D. Its own call site in
`main_exec_diff3d.m` is commented out.

# An open disagreement with the reference

[`criticalboron_xyz`] finds case A2 critical at **1253.29 ppm** where the
MATLAB finds 1139.01 — about **1100 pcm** apart, cause **not established**.
See that module and `docs/bedok-reference-defects.md`, "Open discrepancies".

# A defect worth knowing before running the PWR cases

[`makegrad_dxyz`]'s face coupling is **only consistent on a uniform mesh**
(defect G1). The NEACRP PWR cases grade their axial mesh, and on
[`neacrpa2`]'s worst joint — 30 cm against 7.7 cm, at the bottom of the
core — the coupling is misstated by **+144.8%**. It is pinned by test and
deliberately not repaired. See `docs/bedok-reference-defects.md`.

**Both the steady and the transient paths now run end to end on real
benchmark cases.** [`iaea3ds`] matches a published `k_eff` to -1.1 pcm;
[`neacrpd1`] drives the coupled loop to a joint fixed point in 12 outer
passes; [`neacrpd1t`] marches the case-D cold-water injection through
[`thdiffusion_solvertimexyz`] with six delayed-neutron families. **No
transient result has been compared to a published curve** — the NEACRP
specification is not in the literature archive, so the transient tests
assert structure and cross-scheme agreement only.

[`iapws_if97`] is the one module that is **not** Than Yan Ren's code: it
translates a third-party BSD-2-Clause implementation by Mark Mikofski, whose
terms are reproduced in the crate `NOTICE`. BSD-2-Clause is
GPL-3.0-compatible.

The crate builds clean under clippy and rustdoc, and its 135 unit tests pass
(rustc 1.97.1, release profile, 2026-08-13).

**What that does and does not establish.** The tests cover the translated
utility layer and pin the reference defects listed above; [`iapws_if97`]'s
region-1 functions are checked against the published IAPWS-IF97 verification
values and agree to ~3e-9 relative, which is the tabulated values' own
precision, and region 4's saturation line agrees with Tables 35 and 36 to
**1.752e-9**, with the two directions inverting each other to 4.263e-15.

[`w3chf`]'s unit-conversion chain — seven constants with the British-unit
conversions folded in — is confirmed by the correlation landing at
2.7-3.4 MW/m² at PWR conditions, the expected magnitude. That verifies the
transcription, not the correlation.

The two flux solvers are exercised on a **uniform leaking cube** — one
energy group, vacuum on all six faces, an analytically known
`k_inf = 2.5` — where they converge to a positive, centre-peaked
fundamental mode below `k_inf`, and where the nodal correction moves the
eigenvalue -299 pcm off finite difference on a 4x4x4 mesh. That checks the
assembly, the factorisation and the iteration hang together and produce a
physically-signed answer.

**The one benchmark comparison is [`iaea3ds`].** The IAEA 3-D PWR
benchmark — 17x17x19 quarter core, two groups — gives `k_eff = 1.029084`
against 1.029096 (PARCS) and 1.029082 (ADPRES), so **-1.1 pcm and +0.2
pcm**, closer than the two reference codes are to each other. Measured
2026-08-18; see that module and the README for the full statement, and
`src/data/PROVENANCE.md` for where the two reference numbers come from.

That is the crate's **only** validation evidence, and its scope is narrow:
pure neutronics, no thermal-hydraulics, no coupling, no transient, and no
comparison against the benchmark's published assembly powers. The coupled
driver [`thdiffusion_solverxyz`] is **not** shown to converge on any case —
read its "Verification status" before using it. Per `RESPONSIBLE_USE.md`
everything here remains AI-assisted draft material pending human review;
"the tests pass" is not the same claim as "the physics is right".

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Permission:** given by the author for open-source release under OUTRAM
  PARK, with institutional approval; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

## Modules

## Module `driftflux6_solverstatic3d`

Multichannel wrapper for the staggered six-equation two-fluid solver.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `driftflux6_solverstatic3d.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# Read this first: the solver this wraps is missing from the handover

Every channel's actual solve is delegated to **`driftflux6_solverstatic1d.m`,
which is not in the snapshot** — one of the five referenced-but-absent files
`docs/bedok-reference-defects.md` records. So this file cannot do its job as
shipped, in MATLAB or here.

What it does instead is not undefined, though, and that is why the module is
translated rather than skipped. The reference wraps each channel solve in a
`try`/`catch` that keeps the channel's previous state and warns. In MATLAB,
calling a missing function raises `Undefined function` — which that `catch`
swallows. **So the shipped snapshot's real behaviour is: every powered
channel fails, warns, and retains its previous state, after which the
derived-field tail runs over those unchanged states.** This translation
reproduces exactly that, and reports it through
[`ChannelOutcome::SolverMissing`] so a caller cannot mistake it for a
converged solve.

The consequence for the layer above: `th_solverxyz.m` chooses between this
and [`crate::singleflow1devap`] on `params.th_model`, and **only the `'hem'`
branch can actually run**. The NEACRP D1 BWR case sets `th_model = 'hem'`,
so the benchmark path is unaffected.

# What *is* translated and does work

- The channel sharding and the previous-state defaults.
- The warm-start admission policy (below).
- **The whole derived-field recovery tail** — pressures, phase densities,
  mixture density and velocity, enthalpies, quality and the three liquid
  transport properties, all from the IAPWS layer. This is real, testable
  code and it runs over whatever states the channels hold.

# The warm-start policy, which is the interesting part

A channel's previous solution is reused as a starting guess **only if both**
hold: that solve converged (`relerr < 1e-3`), and the wall heat flux has
moved less than 20% since. The reference's own comment explains why — an
unconverged mid-march state is a "poisoned seed", and under a hard flux ramp
the evaporation seed rebuilt from the *current* flux tracks the problem
better than a stale converged one. A seed is likewise only *stored* from a
converged solve.

# What is deliberately not reproduced

**The `parfor` sharding.** The reference runs the channels over a MATLAB
process pool, with `params.stag6_par` and `params.stag6_nworkers` to control
it and an automatic serial fallback. Channels are independent, so this is a
pure performance choice with no effect on results; the translation runs them
serially. Re-introducing parallelism here is a free change whenever it is
worth making.

**The `evalc` log capture.** The reference wraps the channel call in `evalc`
purely to swallow the JFNK solver's per-iteration printing, which it notes
would otherwise flood the coupled log at ~2 MB/cycle. Nothing here prints.

```rust
pub mod driftflux6_solverstatic3d { /* ... */ }
```

### Types

#### Enum `ChannelOutcome`

What happened to one channel.

The reference tracks this in the `warm` / `fail` flag arrays and prints a
summary line; returning it lets a caller act on the fact that nothing was
solved, which a printed line does not.

```rust
pub enum ChannelOutcome {
    Unpowered,
    SolverMissing,
}
```

##### Variants

###### `Unpowered`

The column carries no power, so it was skipped and keeps its previous
state. This is the reference's `if ~any(pwch); return; end`.

###### `SolverMissing`

The channel is powered and would have been solved, but
`driftflux6_solverstatic1d.m` is absent from the snapshot. The previous
state is retained, reproducing the reference's `catch`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChannelOutcome { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ChannelOutcome) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ChannelReport`

The per-channel bookkeeping the wrapper returns alongside the updated state.

```rust
pub struct ChannelReport {
    pub outcomes: Vec<ChannelOutcome>,
    pub powered: usize,
    pub warm_eligible: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `outcomes` | `Vec<ChannelOutcome>` | One outcome per channel, in `ix * maxiy + iy` order. |
| `powered` | `usize` | How many channels carried power and so attempted a solve. |
| `warm_eligible` | `usize` | How many would have been given a warm start, had the solver existed.<br><br>Computed from the reference's admission policy against the incoming<br>`stag6_*` store, so it is a faithful count even though no solve runs. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChannelReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ChannelReport { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `driftflux6_solverstatic3d`

`th = driftflux6_solverstatic3d(params, geometry, th, pwrdens)`.

# Arguments

- `params` — the three extents.
- `geometry` — needs `Lz`.
- `th` — the incoming T-H state; its coolant fields supply the
  previous-state defaults, and `stag6_ustag` / `stag6_qref` /
  `stag6_relerr` the warm-start store.
- `pwrdens` — power density per node; a column with none is skipped.

# Returns

`(th, report)` — the updated state with every derived field recovered, and
the per-channel [`ChannelReport`].

# This does not solve anything

See the module docs. Every powered channel reports
[`ChannelOutcome::SolverMissing`] and keeps its previous state; the derived
fields are then recovered over those states, which is exactly what the
shipped MATLAB does. A caller wanting a channel model that works should use
[`crate::singleflow1devap`].

# Errors

Never — the missing solver is reported per channel, not as a call failure,
because that is how the reference behaves. [`missing_solver`] is provided
for a caller that would rather have the error value.

# Panics

If `pwrdens` or `geometry.lz` is shorter than the node count.

```rust
pub fn driftflux6_solverstatic3d(params: &crate::types::Params, geometry: &crate::types::Geometry, th: &crate::types::Th, pwrdens: &[f64]) -> (crate::types::Th, ChannelReport) { /* ... */ }
```

#### Function `missing_solver`

The error value for the absent single-channel solver.

Provided for a caller that would rather fail than continue on stale state —
the wrapper itself does not return it, because the reference catches and
continues.

```rust
pub fn missing_solver() -> crate::error::BedokError { /* ... */ }
```

## Module `error`

Error type for the translation.

# Why this module exists

No `.m` counterpart. The reference signals failure with `error(...)`, which
aborts the whole run; the closest faithful Rust is a `Result` carrying the
same condition and message. Where the reference *prints* rather than errors
(`convertsparsekey3d.m`'s debug block, for instance), the translation logs
and continues — the behaviour, not the mechanism, is what is preserved.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `BedokError`

Failure conditions raised by the translated reference.

Each variant names the `.m` file whose `error(...)` call it stands in for,
so a failure can be traced back to the MATLAB line that produced it.

```rust
pub enum BedokError {
    NanEncountered,
    UnexpectedComplex,
    NoCoordinateBranch,
    ReferenceFileMissing {
        file: &'static str,
        referenced_from: &'static str,
    },
    UninitialisedRodLevel {
        ix: usize,
        iy: usize,
        bank: usize,
    },
    BoronSearchDiverged {
        k_eff: f64,
        boron: f64,
        phase: &'static str,
    },
    IterativeSolveNotTranslated {
        philenf: usize,
        threshold: usize,
    },
}
```

##### Variants

###### `NanEncountered`

`pauseonnan.m` — `error('NaN occured')`.

The reference's spelling of the message is preserved deliberately; it is
what a user grepping the MATLAB will search for.

###### `UnexpectedComplex`

`pauseonnan.m` — `error('Unexpected  complex number')`.

The doubled space is in the reference and is kept. This variant is
currently unreachable: the translation works in `f64` throughout, so
there is no complex value for `~isreal` to detect. It exists to record
that the reference performs the check.

###### `NoCoordinateBranch`

A coordinate branch in `handle2dcoords.m` / `handle3dcoords.m` matched
no populated field set, leaving the reference's outputs unassigned.

###### `ReferenceFileMissing`

A `.m` file the snapshot references but does not contain.

The handover is incomplete — `docs/bedok-reference-defects.md` lists
five referenced-but-absent files. Where a translated module's control
flow reaches one of them, this reports which file and from where, rather
than silently producing a default.

Note the reference itself often *catches* the resulting MATLAB
"undefined function" error and continues on a fallback path; where it
does, the translation reproduces that fallback and surfaces this as a
per-item outcome rather than failing the whole call. See
[`crate::driftflux6_solverstatic3d`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `&'static str` | The absent `.m` file. |
| `referenced_from` | `&'static str` | The file that calls it. |

###### `UninitialisedRodLevel`

`sigmavalupd3d_handler.m` — defect C1, on a lattice position with no
previous value to inherit.

The rod-level search leaves `rodlvl` unassigned when a bank's tip sits
at or above the top of its column. Later positions silently reuse the
previous one's value; the **first** has none, and MATLAB raises
`Undefined function or variable 'rodlvl'`. There is no defensible
substitute, so this reports the position rather than inventing one.

See [`crate::sigmavalupd3d_handler`] for why this case is not exotic:
it is a fully withdrawn bank.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ix` | `usize` | The 0-based `x` index of the lattice position. |
| `iy` | `usize` | The 0-based `y` index. |
| `bank` | `usize` | The control-rod bank number. |

###### `BoronSearchDiverged`

The critical-boron search produced an eigenvalue outside a sane range.

`criticalboron_xyz.m` raises `criticalboron_xyz:badeig` whenever a
search eigensolve returns a `k_eff` outside `[0.8, 1.2]`, and
`criticalboron_xyz:badboot` for `[0.5, 1.5]` during the Phase-0
bootstrap. Both abort rather than feeding a garbage value into the
secant — the reference's comment records boron diverging past 1e5 ppm
when an earlier version did not check.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | The offending eigenvalue. |
| `boron` | `f64` | The boron concentration it was computed at, ppm. |
| `phase` | `&'static str` | Which phase raised it: `"eigensolve"` or `"bootstrap"`. |

###### `IterativeSolveNotTranslated`

The flux solvers' preconditioned-GMRES branch, which is **not
translated**.

This is the one place the translation declines to reproduce the
reference rather than reproducing it faithfully, so it is worth being
precise about the scope.

`diffusion_solverxyz.m` and `sanodaldiffusion_solverxyz.m` both switch
from a direct factorisation to `gmres(LHS, RHS, 100, tol, 20, L, U, x0)`
with an `ilu` preconditioner once `philenf >= 50_000_000`. Reaching that
needs 50 million unknowns — for two energy groups, a mesh of 25 million
nodes — whose sparse operators alone would not fit in the memory of any
machine this code runs on. The branch is unreachable for every case in
the snapshot and for anything a user could plausibly build.

Translating it would mean writing an ILU factorisation and a restarted
GMRES that could never be exercised, and therefore never verified,
against the reference. An explicit error is the honest alternative: it
is visible, it names the threshold, and it cannot be mistaken for the
direct path having run.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `philenf` | `usize` | The problem size that selected the branch. |
| `threshold` | `usize` | The reference's `sizethresh`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Type Alias `Result`

Result alias for the translated reference.

```rust
pub type Result<T> = std::result::Result<T, BedokError>;
```

## Module `matlab`

MATLAB container semantics — the one module with no `.m` counterpart.

# Why this module exists

MATLAB's array language supplies things Rust does not: multi-dimensional
column-major arrays, a sparse triplet form that sums duplicates, `find`,
`nnz`. The reference leans on all of them. They live here so the translated
modules read as physics rather than as plumbing.

# Indexing — 0-based

**These containers are 0-based**, and the translated index formulas are
converted from the reference's 1-based arithmetic accordingly. The usual
shape of the change is that `(ix-1)*stride` becomes `ix*stride` and a
trailing `+ iz` becomes `+ iz` with `iz` already 0-based, so

```text
idx = (g-1)*es + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz     % MATLAB, 1-based
```

becomes

```text
idx =  g*es    +  ix*maxiy*maxiz    +  iy*maxiz    + iz     // Rust, 0-based
```

Storage stays **column-major**, matching MATLAB, because the reference does
linear indexing into multi-dimensional arrays in places and the layout is
therefore observable.

# The sentinel that does not survive the conversion

One thing is *not* a mechanical reindex, and it is worth knowing about
before reading `convert_grid3d`. The reference uses **`key(idx) == 0` to
mean "this node carries no material"**, which works only because `0` is not
a valid 1-based index. Going 0-based makes `0` a perfectly good index and
destroys the sentinel.

Rather than invent a different magic number, the translation carries that
map as `Option<usize>` — `None` for absent, `Some(i)` for the compacted
index. It is the same information with the ambiguity removed, and the two
call sites that tested `== 0` test `is_none()` instead.

# What belongs here

Only container semantics and MATLAB built-ins. Physics belongs in the module
named after the `.m` file it came from.

```rust
pub mod matlab { /* ... */ }
```

### Types

#### Struct `Array2`

Column-major 2-D array — the translation's stand-in for a MATLAB matrix.

Indices are **0-based**. Element `(i, j)` lives at linear offset
`j * rows + i`, which is MATLAB's layout.

```rust
pub struct Array2<T> {
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
  pub fn zeros(rows: usize, cols: usize) -> Self { /* ... */ }
  ```
  `zeros(rows, cols)`.

- ```rust
  pub fn rows(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 1)`.

- ```rust
  pub fn cols(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 2)`.

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> T { /* ... */ }
  ```
  Element `(i, j)`, 0-based.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, value: T) { /* ... */ }
  ```
  Set element `(i, j)`, 0-based.

- ```rust
  pub fn as_slice(self: &Self) -> &[T] { /* ... */ }
  ```
  The backing store, column-major, for bulk operations.

- ```rust
  pub fn get_linear_column_major(self: &Self, k: usize) -> T { /* ... */ }
  ```
  Read by **linear** index in column-major order — MATLAB's `a(k)` on a

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Array2<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Array2<T>) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Array3`

Column-major 3-D array — the `(ix, iy, iz)` grids the reference uses for
`whichsigma`, diffusion coefficients and the like. 0-based.

```rust
pub struct Array3<T> {
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
  pub fn zeros(rows: usize, cols: usize, pages: usize) -> Self { /* ... */ }
  ```
  `zeros(rows, cols, pages)`.

- ```rust
  pub fn rows(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 1)`.

- ```rust
  pub fn cols(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 2)`.

- ```rust
  pub fn pages(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 3)`.

- ```rust
  pub fn get(self: &Self, i: usize, j: usize, k: usize) -> T { /* ... */ }
  ```
  Element `(i, j, k)`, 0-based.

- ```rust
  pub fn as_slice(self: &Self) -> &[T] { /* ... */ }
  ```
  The backing store, column-major, for bulk operations such as a whole-

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, k: usize, value: T) { /* ... */ }
  ```
  Set element `(i, j, k)`, 0-based.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Array3<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Array3<T>) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Array4`

Column-major 4-D array — the `(ix, iy, iz, g)` group-wise quantities such
as `diffvalues`. 0-based.

```rust
pub struct Array4<T> {
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
  pub fn zeros(d1: usize, d2: usize, d3: usize, d4: usize) -> Self { /* ... */ }
  ```
  `zeros(d1, d2, d3, d4)`.

- ```rust
  pub fn get(self: &Self, i: usize, j: usize, k: usize, l: usize) -> T { /* ... */ }
  ```
  Element `(i, j, k, l)`, 0-based.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, k: usize, l: usize, value: T) { /* ... */ }
  ```
  Set element `(i, j, k, l)`, 0-based.

- ```rust
  pub fn groups(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 4)` — the group count, in every current use.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Array4<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Array4<T>) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Triplet`

One structural non-zero of a sparse matrix.

Row and column indices are **0-based**, unlike MATLAB's `[i, j, v] =
find(mat)` which returns them 1-based.

```rust
pub struct Triplet {
    pub i: usize,
    pub j: usize,
    pub v: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `i` | `usize` | 0-based row index. |
| `j` | `usize` | 0-based column index. |
| `v` | `f64` | The stored value. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Triplet { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Triplet) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SparseMatrix`

Sparse matrix in triplet (coordinate) form, mirroring MATLAB's `sparse`.

# Semantics carried over from MATLAB

- `sparse(i, j, v, m, n)` **sums duplicate `(i, j)` pairs** rather than
  overwriting. [`SparseMatrix::assemble`] reproduces that.
- Explicitly stored zeros are dropped, so `nnz` counts structural
  non-zeros only.
- [`SparseMatrix::find`] returns entries in **column-major order**, the
  order MATLAB's `find` produces on a sparse matrix.

```rust
pub struct SparseMatrix {
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
  pub fn zeros(rows: usize, cols: usize) -> Self { /* ... */ }
  ```
  An all-zero `rows`-by-`cols` sparse matrix — `sparse(m, n)`.

- ```rust
  pub fn assemble(i: &[usize], j: &[usize], v: &[f64], rows: usize, cols: usize) -> Self { /* ... */ }
  ```
  `sparse(i, j, v, rows, cols)` with **0-based** index slices, summing

- ```rust
  pub fn rows(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 1)`.

- ```rust
  pub fn cols(self: &Self) -> usize { /* ... */ }
  ```
  `size(a, 2)`.

- ```rust
  pub fn add(self: &mut Self, i: usize, j: usize, value: f64) { /* ... */ }
  ```
  Accumulate `value` into entry `(i, j)`, 0-based.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, value: f64) { /* ... */ }
  ```
  Overwrite entry `(i, j)`, 0-based — `mat(i, j) = value`.

- ```rust
  pub fn nnz(self: &mut Self) -> usize { /* ... */ }
  ```
  `nnz(mat)` — the count of structural non-zeros.

- ```rust
  pub fn find(self: &mut Self) -> Vec<Triplet> { /* ... */ }
  ```
  `[i, j, v] = find(mat)` — non-zeros in column-major order, 0-based.

- ```rust
  pub fn diagonal(self: &mut Self) -> Vec<f64> { /* ... */ }
  ```
  `diag(mat)` — the leading diagonal as a dense vector, `min(rows, cols)`

- ```rust
  pub fn column_sums(self: &mut Self) -> Vec<f64> { /* ... */ }
  ```
  `sum(mat)` — the **column** sums, `cols()` long.

- ```rust
  pub fn combine(terms: &[(&SparseMatrix, f64)]) -> Self { /* ... */ }
  ```
  A linear combination `sum_k factor_k * term_k` — the `gradD + nodal +

- ```rust
  pub fn from_diagonal(v: &[f64]) -> Self { /* ... */ }
  ```
  `spdiags(v, 0, n, n)` — a square matrix carrying `v` on its diagonal.

- ```rust
  pub fn scale_columns(self: &Self, d: &[f64]) -> Self { /* ... */ }
  ```
  `mat * spdiags(d, 0, n, n)` — scale **column** `j` by `d[j]`.

- ```rust
  pub fn mul_vec(self: &mut Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  `mat * x` — sparse matrix times dense vector.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SparseMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SparseMatrix { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `Decomposition`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

`decomposition(A)` — a reusable sparse factorisation, MATLAB's own idiom for
"factorise once, then apply `\` many times".

Both flux solvers factorise their `LHS` outside the power iteration and
solve against it every pass; [`crate::sanodaldiffusion_solverxyz`]
refactorises whenever the nodal correction is updated. That reuse is the
whole reason the reference calls `decomposition` rather than `A\b`.

The factorisation is an LU with partial pivoting, matching what MATLAB
selects for a general (unsymmetric, non-triangular) sparse matrix — the
nodal-diffusion `LHS` is unsymmetric, so no Cholesky path applies.

# A singular matrix yields `NaN`, it does not error

MATLAB's `decomposition` **warns** on a singular matrix and its `\` then
propagates `Inf`/`NaN`; it does not abort. That behaviour is load-bearing
here, because both solvers are written to catch it downstream —
`sanodaldiffusion_solverxyz` runs the result through
[`crate::fixinfnan::fixinfnan`], and both break their power iteration on
`isnan(k_eff)`. So a failed factorisation is represented as
[`Decomposition::Singular`], whose [`Decomposition::solve`] returns all
`NaN`, rather than as an error that would skip those guards.

```rust
pub enum Decomposition {
    Factorised(faer::sparse::linalg::solvers::Lu<usize, f64>),
    Singular,
}
```

##### Variants

###### `Factorised`

A successful LU factorisation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `faer::sparse::linalg::solvers::Lu<usize, f64>` |  |

###### `Singular`

The factorisation failed — see the type-level note on why this is not an
error.

##### Implementations

###### Methods

- ```rust
  pub fn new(a: &mut SparseMatrix) -> Self { /* ... */ }
  ```
  Factorise a square sparse matrix.

- ```rust
  pub fn solve(self: &Self, rhs: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  `dA \ b`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve_dense`

`A \ b` for a small **dense** system — Gaussian elimination with partial
pivoting.

The reference uses MATLAB's `mldivide` on the tiny per-node systems the
nodal expansion produces: `G`-by-`G` at a boundary face and `2G`-by-`2G` at
an interior one, with `G` typically 2. At that size a hand-written
elimination is clearer than a linear-algebra dependency, and the pivoting
makes it as stable as `mldivide` is on the same input.

This is **not** for the large sparse systems elsewhere in the solver; those
want a real sparse factorisation.

# Arguments

- `a` — the `n`-by-`n` matrix, **row-major**: element `(i, j)` at
  `a[i * n + j]`.
- `b` — the right-hand side, `n` long.
- `n` — the system size.

# Returns

The solution vector. A **singular** matrix yields `NaN` entries rather than
an error, which is the closest match to MATLAB's behaviour — it warns and
returns non-finite values rather than aborting. Callers that care must check.

# Panics

If `a` is not `n * n` long or `b` is not `n` long.

```rust
pub fn solve_dense(a: &[f64], b: &[f64], n: usize) -> Vec<f64> { /* ... */ }
```

#### Function `norm1`

`norm(v, 1)` — the sum of absolute values.

The two flux solvers use this for the fission-source integral that drives
the `keff` update. Note [`crate::sanodaldiffusion_solverxyz`] uses a plain
`sum` in two of the three places where [`crate::diffusion_solverxyz`] uses
this; the two differ whenever the fission source has a negative entry, which
a diverging solve can produce. That inconsistency is the reference's — see
defect N10.

```rust
pub fn norm1(v: &[f64]) -> f64 { /* ... */ }
```

#### Function `norm2`

`norm(v)` — the Euclidean (2-)norm.

```rust
pub fn norm2(v: &[f64]) -> f64 { /* ... */ }
```

#### Function `min_abs_finite`

`min(abs(v))` over the entries that have a finite magnitude.

MATLAB's `min` skips `NaN`, and `abs(Inf)` is `Inf`, which can only be the
minimum when every entry is non-finite. This reproduces both behaviours and
returns `None` for that degenerate case, where MATLAB would return `Inf`.
The one caller, `fixinfnan`, only reaches it when at least one entry is
non-finite.

```rust
pub fn min_abs_finite(v: &[f64]) -> Option<f64> { /* ... */ }
```

## Module `thdiffusion_solverxyz`

The steady coupled driver — neutronics and thermal-hydraulics to a joint
fixed point.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `thdiffusion_solverxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this is

The top of the steady solver stack, and the point of the whole crate. One
outer iteration is:

1. **Rebuild the cross sections** from the current T-H state, through
   [`crate::sigmavalupd3d_handler`].
2. **Solve the eigenvalue problem** with
   [`crate::sanodaldiffusion_solverxyz`], warm-started from the previous
   pass's flux and `k_eff`.
3. **Solve the thermal-hydraulics** on the resulting power, through
   [`crate::th_solverxyz`].
4. **Under-relax** the four feedback fields, and test three convergence
   criteria.

It exits when the fission-source residual, the `k_eff` residual **and** the
fuel-temperature change are all under tolerance.

# The under-relaxation is load-bearing, not a nicety

Steps 2 and 3 are each convergent on their own; their composition is not.
The reference damps four fields — coolant density, Doppler temperature,
`fueltempavg` and wall heat flux — with a weight of 0.5, and says why:
without it the strong BWR void/Doppler feedback "oscillates undamped between
cold/dense and boiling/void states". Raising [`crate::types::Params::threlax`]
to 1 removes the damping entirely.

# The inner tolerance follows the outer residual

An Eisenstat-Walker style schedule sets

```text
innertol = clamp(eta * max(fs_residual, keff_residual), 1e-6, 1e-3)
```

with `eta = 0.001`. While the outer loop is far from converged an
over-tight inner solve is wasted, because the cross sections move again next
pass. The reference's comment makes a sharper point than mere economy,
though, and it is worth repeating: a loose inner solve **biases the coupled
fixed point**, not just the final readout — loose flux gives wrong power
gives wrong fuel temperature gives wrong Doppler. So the schedule
self-tightens to the 1e-6 floor in the tail, where the outer residual is
~1e-3.

This is the only consumer of [`crate::types::Params::innertol`], the switch
[`crate::sanodaldiffusion_solverxyz`] reads.

# Verification status

**The coupled loop converges on a real benchmark case.** Run on
[`crate::neacrpd1`] — NEACRP case D, a 17x17x14 two-group LWR core with
fuel-temperature and coolant-density feedback — it reaches a joint fixed
point in **12 outer passes**, meeting all three criteria: fission-source
residual 2.645e-5, `k_eff` residual 8.270e-6, and a fuel-temperature
residual of 0.4744 K against a 0.5 K tolerance. On the HEM
thermal-hydraulic path it converges in 29 passes. Measured 2026-08-18; the
full numbers and their interpretation are in that module's tests.

**It does not converge on the synthetic 3x3x6 one-group fixture below**,
and the NEACRP result identifies that as a property of the fixture rather
than of this module. A hand-made one-group cross-section set on a 3x3x6
mesh is not necessarily a well-posed coupled problem, and this one is not:
the inner [`crate::sanodaldiffusion_solverxyz`] solve converges on the first
two outer passes and then hits its 5000-iteration cap, regardless of the
sign or magnitude of the feedback slope — a ten-fold weaker table and a
flipped void coefficient both fail at the same pass.

The warm-start renormalisation was named as the prime suspect while the
fixture and the port were still indistinguishable. It is **exonerated**:
the NEACRP case exercises it on all 12 passes.

The three fixture-dependent tests below are therefore left `#[ignore]`d
rather than deleted or weakened to pass. They state what should hold, and
the honest fix is to rebuild that fixture as a well-posed problem — not to
relax them. **The claim this module supports is the NEACRP one above; do
not extend it to the transient path, which has no such evidence.**

```rust
pub mod thdiffusion_solverxyz { /* ... */ }
```

### Modules

## Module `defaults`

The reference's default outer tolerances and caps.

```rust
pub mod defaults { /* ... */ }
```

### Constants and Statics

#### Constant `FUELTEMP_TOL`

`fueltemp.tol` — max-norm fuel-temperature change, K.

```rust
pub const FUELTEMP_TOL: f64 = 0.5;
```

#### Constant `FLUX_TOL`

`flux.tol` — fission-source and `k_eff` residual tolerance.

```rust
pub const FLUX_TOL: f64 = 1e-4;
```

#### Constant `MAX_ITER`

`maxiter` — outer iteration cap.

```rust
pub const MAX_ITER: usize = 50;
```

#### Constant `RELAX`

`wrelax` — Picard under-relaxation weight.

```rust
pub const RELAX: f64 = 0.5;
```

#### Constant `ETA`

`eta` — the inexact-inner forcing factor.

```rust
pub const ETA: f64 = 0.001;
```

#### Constant `INNERTOL_FLOOR`

The inner tolerance floor.

```rust
pub const INNERTOL_FLOOR: f64 = 1e-6;
```

#### Constant `INNERTOL_CAP`

The inner tolerance cap.

```rust
pub const INNERTOL_CAP: f64 = 1e-3;
```

### Types

#### Enum `Termination`

Why the coupled iteration stopped.

```rust
pub enum Termination {
    Converged,
    NonPositiveKeff,
    NanKeff,
    IterationCap,
}
```

##### Variants

###### `Converged`

All three criteria met.

###### `NonPositiveKeff`

`k_eff <= 0` — a non-physical eigenvalue.

###### `NanKeff`

`k_eff` came back `NaN`.

###### `IterationCap`

The outer iteration cap was reached.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Termination { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CoupledOutput`

`output` — what the reference returns, plus what it computes and discards.

```rust
pub struct CoupledOutput {
    pub k_eff: f64,
    pub residual: f64,
    pub k_eff_residual: f64,
    pub fueltemp_residual: f64,
    pub fueltemp_residual_history: Vec<f64>,
    pub k_eff_history: Vec<f64>,
    pub scalar_flux: crate::matlab::Array2<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub th: crate::types::Th,
    pub iterations: usize,
    pub termination: Termination,
    pub fueltemp_converged: bool,
    pub chf: crate::w3chf::Chf,
    pub chf_channel: crate::w3chfhottest::HottestChannel,
    pub rodfraction: crate::sigmavalupd3d_handler::RodFraction,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | `output.k_eff` — the converged multiplication factor. |
| `residual` | `f64` | `output.residual` — the final fission-source residual. |
| `k_eff_residual` | `f64` | `output.k_eff_residual` — the final `k_eff` residual. |
| `fueltemp_residual` | `f64` | `output.fueltemp_residual` — the final fuel-temperature change, K. |
| `fueltemp_residual_history` | `Vec<f64>` | `output.fueltemp_residual_history` — one entry per outer iteration. |
| `k_eff_history` | `Vec<f64>` | `output.k_eff_history` — one entry per outer iteration. |
| `scalar_flux` | `crate::matlab::Array2<f64>` | `output.scalar_flux` — the converged flux history, renormalised. |
| `fission_source` | `Vec<f64>` | `output.fission_source` — renormalised to the initial integral. |
| `pwrdens` | `Vec<f64>` | `output.pwrdens` — `fission_source .* Vi`. |
| `th` | `crate::types::Th` | `output.th` — the converged thermal-hydraulic state. |
| `iterations` | `usize` | How many outer iterations ran. |
| `termination` | `Termination` | Why the loop stopped. Not in the reference's `output`. |
| `fueltemp_converged` | `bool` | Whether the fuel-temperature criterion was actually met.<br><br>The reference prints `[converged]` or `[NOT converged]` for this alone,<br>separately from the other two; a caller cannot otherwise tell, because<br>the loop can exit on the iteration cap with this still large. |
| `chf` | `crate::w3chf::Chf` | The critical-heat-flux result — **which the reference computes and<br>throws away**.<br><br>Defect C3: `chf = w3chfhottest(params, geometry, th)` runs on the last<br>line before the output block, and `chf` never appears in `output`. The<br>work is done and discarded. Returned here rather than discarded, on the<br>same reasoning as the CSV dumps elsewhere in this crate — the<br>computation is unchanged and the caller gains the answer. |
| `chf_channel` | `crate::w3chfhottest::HottestChannel` | Which channel that CHF belongs to. See [`HottestChannel`] — defect C2<br>means it may not be the limiting one. |
| `rodfraction` | `crate::sigmavalupd3d_handler::RodFraction` | The final rod-fraction map from the last feedback rebuild. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoupledOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `thdiffusion_solverxyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`output = thdiffusion_solverxyz(geometry, params, th, sigmavalues, whichsigma, initial_k_eff)`.

# Arguments

- `geometry`, `params` — as the solvers they drive.
- `th` — the incoming T-H state. **Its feedback fields are overwritten**
  before the first iteration with the uniform values
  `params.fueltempavg`, `params.cooltempavg` and `params.cooldenavg`, so
  only the case-file constants on it survive.
- `sigmavaluesref`, `feedback`, `whichsigmaref` — the unperturbed cross
  sections and the feedback tables, held fixed and re-perturbed each pass.
- `initial_k_eff` — `varargin{1}`; `None` is the reference's default of `1`.

# Returns

[`CoupledOutput`].

# Convergence — three criteria, and the loop exits only when all three pass

```text
while fs_residual >= fluxtol || keff_residual >= fluxtol
                             || fueltemp_error >= fueltemptol
```

The fuel-temperature criterion is a **max-norm over the core**, in kelvin,
on the change in `fueltempavg` between passes — and it is taken *after*
under-relaxation, so the damping weight also sets how fast this criterion
can be met.

# Defects carried here

- **C3 — the CHF result is computed and discarded.** Returned here; see
  [`CoupledOutput::chf`].
- **The final renormalisation pairs mismatched vectors on an early break**,
  exactly as [`crate::sanodaldiffusion_solverxyz`]'s does (defect D5):
  `norm_factor` comes from the last pass's `fission_source_new` while the
  scaling is applied to `fission_source`, which on a `break` is one pass
  older.
- **Seven `writematrix` dumps** run unconditionally at the end, outside any
  `debugdump` guard. Not reproduced — the histories they contain are in
  [`CoupledOutput`].
- **The break increments the iteration counter first**, unlike the flux
  solvers, so the reported histories include the failing pass.

# Errors

Whatever the inner solvers raise — notably
[`crate::error::BedokError::UninitialisedRodLevel`] from the feedback
rebuild.

# Panics

If the geometry vectors are shorter than the node count.

```rust
pub fn thdiffusion_solverxyz(geometry: &crate::types::Geometry, params: &crate::types::Params, th: &crate::types::Th, sigmavaluesref: &crate::types::SigmaValues, feedback: &crate::sigmavalupd3d_handler::FeedbackTables, whichsigmaref: &crate::matlab::Array3<usize>, initial_k_eff: Option<f64>) -> crate::Result<CoupledOutput> { /* ... */ }
```

## Module `th_solvertimexyz`

The transient thermal-hydraulics driver — one implicit-Euler step.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `th_solvertimexyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What differs from the steady driver

Structurally this is [`crate::th_solverxyz`] with three substitutions and
nothing else. The power normalisation, the Dittus-Boelter coefficient, the
fuel-temperature clamp, the Doppler weight, the wall-flux recovery and the
`NaN` rescue are all identical, line for line.

| | steady | transient |
|---|---|---|
| coolant | [`crate::singleflow1devap`] **or** the two-fluid wrapper | [`crate::singleflow1devaptime`], always |
| rods | [`crate::fuelrodheat_1dcylnd`] | [`crate::fuelrodheattime_1dcylnd`] |
| extra inputs | — | `thold`, `dt` |

# There is no channel-model gate here, and that explains the steady one

`th_solverxyz.m` chooses between the homogeneous-equilibrium march and the
two-fluid wrapper on `params.th_model`. **This file has no such choice** —
it always marches HEM.

That asymmetry is the reason the steady driver has a `'hem'` option at all.
Its own comment says so: a transient run needs its `t = 0` steady state from
the *same* model it will be marched with, because a two-fluid steady state
has less void than HEM at the same conditions, and handing that to the
transient would be a density mismatch — a spurious reactivity step at
`t = 0`. So `neacrpd1t` sets `th_model = 'hem'` to keep the two consistent.

# The steady driver is this one at `dt = infinity`

Both substituted solvers reduce to their steady counterparts as `dt` grows,
each verified in its own module. It follows that this driver reduces to
[`crate::th_solverxyz`] in HEM mode, and that is checked here — a
cross-check across two independently transcribed drivers and four
independently transcribed solvers.

```rust
pub mod th_solvertimexyz { /* ... */ }
```

### Functions

#### Function `th_solvertimexyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`th = th_solvertimexyz(params, geometry, th, whichsigma, pwrdens, thold, dt)`.

# Arguments

As [`crate::th_solverxyz::th_solverxyz`], plus:

- `thold` — the **converged state of the previous time step**. Supplies the
  capacity terms: `thold.coolant.enth` and `.dens` for the coolant march,
  `thold.fueltemp` for the rod conduction.
- `dt` — the time step, **seconds**.

Note `th` and `thold` are different things and both are needed. `th` is the
current Picard iterate within this time step — its `heatflux` is the lagged
wall flux and its `fueltemp` the property iterate — while `thold` is the
previous time level and carries the physics of both time derivatives.

**`th.powratio` must already carry the current relative core power**, as the
reference's header states; this function does not ramp it.

# Returns

`(th, report)` — the updated state and the per-node
[`crate::th_solverxyz::RodReport`].

# Reference defects carried here

All of [`crate::th_solverxyz`]'s, since the surrounding code is the same:
the subarea recomputed rather than read (T12), the Doppler two-point weight
aliased to `fueltempavg` (T13), the unfuelled-column skip decided on the
bottom node alone, and the dead reads (T14). See that module for each.

# Panics

If `pwrdens` is shorter than `G*es`, or `thold.fueltemp` is not shaped like
`th.fueltemp`.

```rust
pub fn th_solvertimexyz(params: &crate::types::Params, geometry: &crate::types::Geometry, th: &crate::types::Th, whichsigma: &crate::matlab::Array3<usize>, pwrdens: &[f64], thold: &crate::types::Th, dt: f64) -> (crate::types::Th, crate::th_solverxyz::RodReport) { /* ... */ }
```

## Module `th_solverxyz`

The steady thermal-hydraulics driver — coolant, then rods, then feedback.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `th_solverxyz.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this is

The hub of the thermal-hydraulics layer. Given a power distribution from the
neutronics, it runs the whole steady T-H pass in four stages:

1. **Normalise and collapse** the power density — divide by its 1-norm, then
   sum over energy groups.
2. **Solve the coolant**, through whichever channel model
   [`crate::types::ThModel`] selects.
3. **Solve every fuelled rod** with [`crate::fuelrodheat_1dcylnd`], using a
   Dittus-Boelter heat-transfer coefficient as the boundary condition.
4. **Produce the feedback quantities** the neutronics needs back: the
   Doppler fuel temperature, the coolant density, and the wall heat flux
   that closes the loop into the next coolant solve.

Stage 4 is why this matters. `fueltempdoppler` drives the fuel-temperature
cross-section feedback and `dens` the moderator-density feedback, so an
error here moves reactivity.

# The wall heat flux is lagged, and that is the coupling

`heatflux` enters stage 2 as the *previous* pass's value and is recomputed
in stage 3. So one call is one Picard sweep of the coolant/rod coupling, and
the caller iterates. That is why the channel models take `th.heatflux` as an
input rather than deriving it.

```rust
pub mod th_solverxyz { /* ... */ }
```

### Types

#### Enum `NodeOutcome`

What happened at one node during the rod pass.

The reference signals the last of these with a `warning` and carries on;
returning them lets a caller count how much of the core needed rescuing,
which a warning stream does not.

```rust
pub enum NodeOutcome {
    Solved,
    Skipped,
    Rescued,
}
```

##### Variants

###### `Solved`

The rod was solved and its temperatures used.

###### `Skipped`

The node carries no pin power, or its column is unfuelled. Skipped.

###### `Rescued`

The rod solve returned `NaN`. The coolant temperature (or
`params.cooltempavg` if that is not finite either) was substituted and
the wall heat flux zeroed.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodeOutcome { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeOutcome) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `RodReport`

Per-node bookkeeping from the rod pass.

```rust
pub struct RodReport {
    pub solved: usize,
    pub skipped: usize,
    pub rescued: usize,
    pub clamped_low: usize,
    pub clamped_high: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solved` | `usize` | How many nodes were solved. |
| `skipped` | `usize` | How many were skipped as unpowered or unfuelled. |
| `rescued` | `usize` | How many needed the `NaN` rescue. **Any non-zero count here means the<br>feedback is running on substituted temperatures.** |
| `clamped_low` | `usize` | How many nodes had at least one temperature raised to the **coolant<br>floor**.<br><br>Not tracked by the reference, and **it is not an anomaly counter**: on<br>any rod with a fuel-cladding gap this equals [`RodReport::solved`],<br>every time. The gap node is a dummy pinned at exactly 1 K (defect T7),<br>which is always below the coolant temperature, so the floor clamp<br>always fires on it.<br><br>That is worth knowing rather than hiding: the clamp was added as a guard<br>against ill-conditioned conduction solves, but because it is<br>unconditionally active it cannot serve as a signal that one occurred.<br>[`RodReport::clamped_high`] is the one to watch. |
| `clamped_high` | `usize` | How many nodes had a temperature cut down to `tmaxfuel`.<br><br>**This one is a genuine warning.** A rod at the melting-point ceiling<br>either is genuinely melting or, more likely, came out of an<br>ill-conditioned conduction solve. Unlike<br>[`RodReport::clamped_low`] it should be zero in a well-posed case. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RodReport { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `th_solverxyz`

`th = th_solverxyz(params, geometry, th, whichsigma, pwrdens)`.

# Arguments

- `params` — `G`, the extents, `params.fuel`, and the optional `th_model`,
  `tmaxfuel` and `cooltempavg`.
- `geometry` — `Lz`, the `zlows`/`zhis` bounds, and the whole
  `geometry.fuel` rod description.
- `th` — the incoming state. `heatflux` is read as the **lagged** wall flux;
  `fueltemp` is read as the property iterate for the rod solves and
  overwritten with the result.
- `whichsigma` — the material map, used only to skip unfuelled columns.
- `pwrdens` — the power density from the flux solver, `G*es` long. Consumed
  normalised and group-collapsed.

# Returns

`(th, report)` — the updated state and the per-node [`RodReport`].

# The heat-transfer coefficient

A Dittus-Boelter correlation on the subchannel:

```text
subarea  = pitch^2 - pi Rtot^2
hydia    = 4 subarea / (2 pi Rtot + 4 pitch - 8 Rtot)
Re       = vm hydia / kvis
Nu       = 0.023 Pr^0.4 Re^0.8
hcoeff   = tcon Nu / hydia
```

with the exponent 0.4 being the heating form. Lengths in cm, so `hcoeff` is
W/(cm²·K) and the rod boundary condition `bc = hcoeff * Rtot` is W/(cm·K).

**`Pr^0.4` and `Re^0.8` are wrapped in `real()` in the reference**, which
only matters if either goes negative — a non-physical state that the
fractional power would otherwise turn complex. Reproduced here by taking the
power of the absolute value where the base is negative, which is what
`real(x^0.4)` gives for the principal branch only when `x >= 0`; see the
note on defect T11.

# `subarea` and `hydia` are recomputed, not read

This driver derives both from `pitch` and `Rtot` rather than reading
[`crate::types::FuelGeometry::subarea`] and `hydia`, which the case files
also set and [`crate::w3chf`] does read. If a case file's stored values
disagree with `pitch^2 - pi Rtot^2`, the two modules will silently use
different subchannel geometry. Recorded as defect T12.

# The Doppler temperature is a two-point weight, not a volume average

```text
fueltempdoppler = (1 - alpha) * T(centre) + alpha * T(pellet surface)
fueltempavg     = fueltempdoppler
```

The pellet surface is unknown index `fueln + 1` (1-based), which is the
interface-duplicate node [`crate::fuelrodheat_1dcylnd`] creates — **not**
the gap dummy, which sits one further out. The commented-out line directly
above computes a genuine `Vi`-weighted average over the fuel nodes; it is
disabled, and `fueltempavg` is simply aliased to the Doppler value. So a
reader expecting an average gets a two-point weight. Recorded as defect T13.

# Dead reads

The reference loads `Lx`, `Ly`, `Lr`, `Vi` (and `repmat`s it to `G`
groups), `Vif`, `whichf`, `whichg` and `maxir`, and computes
`subflow = flowrate * subarea` — **none of which it then uses**. All are
residue of the commented-out inline conduction assembly. Not parameters
here. Recorded as defect T14.

# Panics

If `pwrdens` is shorter than `G*es`, or the geometry vectors are shorter
than the node count.

```rust
pub fn th_solverxyz(params: &crate::types::Params, geometry: &crate::types::Geometry, th: &crate::types::Th, whichsigma: &crate::matlab::Array3<usize>, pwrdens: &[f64]) -> (crate::types::Th, RodReport) { /* ... */ }
```

### Constants and Statics

#### Constant `TMAX_FUEL_DEFAULT`

`tmaxfuel` — the reference's default fuel-temperature ceiling, K.

The UO2 melting point.

```rust
pub const TMAX_FUEL_DEFAULT: f64 = 3100.0;
```

## Module `types`

The MATLAB structs — `params`, `geometry`, `th`, `constants`, `results`.

# Why this module exists

Like [`crate::matlab`], this has no `.m` counterpart. The reference passes
four loosely-typed structs through nearly every function signature, built up
field by field by the case files (`neacrpd1.m`, `iaea3ds.m`, …) and read
back with `isfield` guards. Rust has no equivalent of an open struct, so the
fields are collected here and the `isfield(params, 'x')` tests become
`Option::is_some`.

# Growing this module

The field set is **deliberately incomplete** and grows as modules are
ported. Only fields an already-translated `.m` file actually reads appear
here — inventing the rest up front would mean guessing at the reference,
which is exactly what the translation is supposed to avoid. Each field
records the `.m` file it was introduced by.

```rust
pub mod types { /* ... */ }
```

### Types

#### Enum `CoordinateMode`

Geometry discretisation mode, as selected by which coordinate fields the
case file populated.

The reference expresses this as a chain of `isfield` tests in
`handle2dcoords.m` / `handle3dcoords.m` rather than as a value; this enum is
only a way of documenting the three cases the reference recognises.

```rust
pub enum CoordinateMode {
    Cylindrical,
    Cartesian,
    Generic,
}
```

##### Variants

###### `Cylindrical`

`maxir` / `maxitheta` / `maxiz` — cylindrical.

###### `Cartesian`

`maxix` / `maxiy` / `maxiz` — Cartesian.

###### `Generic`

`maxi1` / `maxi2` / `maxi3` — generic, already-resolved extents.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoordinateMode { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoordinateMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Params`

The `params` struct — run controls and discretisation extents.

Set up by the user block at the top of `main_exec_diff3d.m` and then
extended by whichever case file runs (`neacrpd1.m` and friends).

# Units

The reference carries no units. Extents are node counts; `tend` and `tgrid`
are seconds. Fields are documented individually where a unit applies.

```rust
pub struct Params {
    pub maxir: Option<usize>,
    pub maxitheta: Option<usize>,
    pub maxix: Option<usize>,
    pub maxiy: Option<usize>,
    pub maxiz: Option<usize>,
    pub maxi1: Option<usize>,
    pub maxi2: Option<usize>,
    pub maxi3: Option<usize>,
    pub g: usize,
    pub nc: Option<usize>,
    pub max_num_cycles: usize,
    pub nodalupd: usize,
    pub fsexp: usize,
    pub evap_c0: Option<f64>,
    pub evap_homog: bool,
    pub innertol: Option<f64>,
    pub fuel: FuelParams,
    pub th_model: ThModel,
    pub tmaxfuel: Option<f64>,
    pub cooltempavg: f64,
    pub boron: f64,
    pub fueltempavg: f64,
    pub cooldenavg: f64,
    pub fueltemptol: Option<f64>,
    pub fluxtol: Option<f64>,
    pub thmaxiter: Option<usize>,
    pub threlax: Option<f64>,
    pub inexactinner: Option<bool>,
    pub inexacteta: Option<f64>,
    pub stop: usize,
    pub verb: i32,
    pub plotfig: i32,
    pub plot3d: i32,
    pub debugdump: i32,
    pub tend: Option<f64>,
    pub tgrid: Option<Vec<f64>>,
    pub timepicard: Option<usize>,
    pub nodalupdtime: Option<usize>,
    pub crittol: Option<f64>,
    pub velocities: Vec<f64>,
    pub beta_dnp: Vec<f64>,
    pub lambda_dnp: Vec<f64>,
    pub ejectduration: Option<f64>,
    pub timescheme: TimeScheme,
    pub freqiter: Option<usize>,
    pub freqmode: FreqMode,
    pub jfnkprecon: i32,
    pub jfnkrel: f64,
    pub jfnkverb: i32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `maxir` | `Option<usize>` | Radial node count, cylindrical cases. `isfield(params,'maxir')`. |
| `maxitheta` | `Option<usize>` | Azimuthal node count, cylindrical cases. |
| `maxix` | `Option<usize>` | `x` node count, Cartesian cases. |
| `maxiy` | `Option<usize>` | `y` node count, Cartesian cases. |
| `maxiz` | `Option<usize>` | `z` node count — shared by the cylindrical and Cartesian branches. |
| `maxi1` | `Option<usize>` | Generic dim-1 extent, used when neither named branch applies. |
| `maxi2` | `Option<usize>` | Generic dim-2 extent. |
| `maxi3` | `Option<usize>` | Generic dim-3 extent. |
| `g` | `usize` | `G` — number of energy groups. |
| `nc` | `Option<usize>` | `Nc` — number of delayed-neutron precursor families.<br><br>`convert_grid3d.m` guards this with `isfield` and substitutes `0`; the<br>other readers assume it is present. |
| `max_num_cycles` | `usize` | Outer power-iteration cycle cap. |
| `nodalupd` | `usize` | Cycles per SA-nodal correction update; `0` selects the built-in default.<br><br>Read by [`crate::sanodaldiffusion_solverxyz`], whose default is<br>`ceil((maxix + maxiy + maxiz) / 10)`. **A value of `1` destabilises the<br>solver** — see that module, and defect N1 in<br>`docs/bedok-reference-defects.md`. |
| `fsexp` | `usize` | Source iterations between fission-source extrapolations; `0` selects the<br>built-in default of 5.<br><br>`isfield(params, 'fsexp')` in `sanodaldiffusion_solverxyz.m`, guarded<br>the same `~= 0` way as [`Params::nodalupd`]. |
| `evap_c0` | `Option<f64>` | `params.evap_C0` — the Zuber-Findlay distribution parameter in the<br>void-quality closure, dimensionless.<br><br>`None` selects the reference's default of **1.2**, quoted there as the<br>round-tube value. Read only by [`crate::singleflow1devap`]. |
| `evap_homog` | `bool` | `params.evap_homog` — force the homogeneous limit.<br><br>When set, the closure uses `C0 = 1` and `Vgj = 0`, so the phases move<br>together and the void fraction follows the quality directly. The<br>reference tests `params.evap_homog == 1`. |
| `innertol` | `Option<f64>` | Inexact inner convergence tolerance for the flux solve, dimensionless.<br><br>Set by an outer coupling loop (`thdiffusion_solverxyz.m`) to avoid<br>over-solving while the T-H feedback is still moving. `None` — and, per<br>the reference's `params.innertol > 0` test, any non-positive value —<br>selects the tight built-in `1e-6`. Read only by<br>[`crate::sanodaldiffusion_solverxyz`]; [`crate::diffusion_solverxyz`]<br>has no such switch and is always tight. |
| `fuel` | `FuelParams` | `params.fuel` — the fuel-rod radial mesh sizes. |
| `th_model` | `ThModel` | `params.th_model` — which channel model the steady T-H driver uses. |
| `tmaxfuel` | `Option<f64>` | `params.tmaxfuel` — ceiling for the fuel-temperature clamp, **K**.<br><br>`None` selects the reference's default of **3100 K**, the UO2 melting<br>point. The clamp guards an ill-conditioned rod-conduction solve from<br>injecting non-physical temperatures into the Doppler feedback. |
| `cooltempavg` | `f64` | `params.cooltempavg` — core-average coolant temperature, **K**.<br><br>Used only as the last-resort substitute when a node's own coolant<br>temperature is itself non-finite and the rod solve returned `NaN`. |
| `boron` | `f64` | `params.boron` — soluble boron concentration, ppm.<br><br>The feedback variable for the boron cross-section table; a scalar over<br>the whole core. Read by [`crate::sigmavalupd3d_handler`], and the<br>quantity the critical-boron search (`criticalboron_xyz.m`, not yet<br>translated) varies. |
| `fueltempavg` | `f64` | `params.fueltempavg` — the fuel temperature the coupled loop starts<br>from, **K**, applied uniformly across the core. |
| `cooldenavg` | `f64` | `params.cooldenavg` — the coolant density the coupled loop starts from,<br>**g/cm³**, applied uniformly. |
| `fueltemptol` | `Option<f64>` | `params.fueltemptol` — outer convergence tolerance on the fuel<br>temperature, **K**, as a max-norm over the core.<br><br>`None` selects the reference's **0.5 K**. Its comment records that this<br>was relaxed from 0.01 K because "a max-norm fuel temperature criterion<br>that tight is unrealistic for a coupled BWR steady state — the hot nodes<br>limit-cycle ~1 K". |
| `fluxtol` | `Option<f64>` | `params.fluxtol` — outer convergence tolerance on the fission-source and<br>`k_eff` residuals, dimensionless.<br><br>`None` selects the reference's **1e-4**, relaxed from 1e-5 because "even<br>exact inner solves floor the outer fission-source residual near ~1e-4". |
| `thmaxiter` | `Option<usize>` | `params.thmaxiter` — cap on coupled outer iterations. `None` selects 50. |
| `threlax` | `Option<f64>` | `params.threlax` — Picard under-relaxation weight on the feedback<br>fields, dimensionless on `(0, 1]`.<br><br>`None` selects the reference's **0.5**. A weight of 1 is no damping;<br>the reference notes the neutronics/T-H feedback "otherwise oscillates<br>undamped between cold/dense and boiling/void states". |
| `inexactinner` | `Option<bool>` | `params.inexactinner` — whether to scale the inner flux tolerance by the<br>outer residual. The reference tests `~= 0`, so `None` means enabled. |
| `inexacteta` | `Option<f64>` | `params.inexacteta` — the forcing factor in that schedule.<br><br>`None` selects the reference's **0.001**. See<br>[`crate::thdiffusion_solverxyz`] for why it is that small. |
| `stop` | `usize` | Force stop after this many cycles; `0` disables. |
| `verb` | `i32` | Verbosity. |
| `plotfig` | `i32` | Whether to produce figures. |
| `plot3d` | `i32` | Whether to produce the 3-D power plot. |
| `debugdump` | `i32` | Debug dump toggle. |
| `tend` | `Option<f64>` | End of transient, seconds. Set by the case file. |
| `tgrid` | `Option<Vec<f64>>` | Explicit time grid, seconds. Absent means uniform 10 ms steps over<br>`0..tend`. |
| `timepicard` | `Option<usize>` | T-H feedback Picard passes per time step. |
| `nodalupdtime` | `Option<usize>` | SA-nodal correction update interval in steps; `0` freezes it. |
| `crittol` | `Option<f64>` | `params.crittol` — tolerance on `|k_eff - 1|` for the critical state.<br><br>Read only by [`crate::criticalboron_xyz`]; defaults to 1e-5. |
| `velocities` | `Vec<f64>` | `params.velocities` — prompt neutron group velocities, cm/s.<br><br>One per energy group. The transient driver uses the reciprocals as the<br>inverse-velocity vector multiplying the flux time derivative; an empty<br>vector means no kinetics data and the transient cannot run. |
| `beta_dnp` | `Vec<f64>` | `params.beta_dnp` — delayed neutron fractions, dimensionless.<br><br>Six families in every case in the snapshot, summing to `betatot`. |
| `lambda_dnp` | `Vec<f64>` | `params.lambda_dnp` — delayed neutron precursor decay constants, 1/s.<br><br>Same length and ordering as [`Params::beta_dnp`]. |
| `ejectduration` | `Option<f64>` | `params.ejectduration` — control-assembly ejection time, seconds.<br><br>The bank moves linearly from its steady position to<br>[`Geometry::crodejectto`] over this interval, then stays put. |
| `timescheme` | `TimeScheme` | `params.timescheme` — which kinetics discretisation to march. |
| `freqiter` | `Option<usize>` | `params.freqiter` — flux solves per step under<br>[`TimeScheme::ExponentialTransform`]: one predictor plus<br>`freqiter - 1` frequency correctors. Clamped to at least 1. |
| `freqmode` | `FreqMode` | `params.freqmode` — how the exponential-transform frequencies are taken. |
| `jfnkprecon` | `i32` | JFNK preconditioner flag.<br><br>**Read by nothing in this snapshot.** `main_exec_diff3d.m` sets it, but<br>its only consumer — `driftflux_solverstatic1d.m` — is absent from the<br>handover. Translated so the driver stays faithful; see<br>`docs/bedok-reference-defects.md`. |
| `jfnkrel` | `f64` | JFNK relaxation factor. Inert, as [`Params::jfnkprecon`]. |
| `jfnkverb` | `i32` | JFNK verbosity. Inert, as [`Params::jfnkprecon`]. |

##### Implementations

###### Methods

- ```rust
  pub fn coordinate_mode_2d(self: &Self) -> Option<CoordinateMode> { /* ... */ }
  ```
  `[maxi1, maxi2] = handle2dcoords(params)` — which coordinate branch the

- ```rust
  pub fn coordinate_mode_3d(self: &Self) -> Option<CoordinateMode> { /* ... */ }
  ```
  `[maxi1, maxi2, maxi3] = handle3dcoords(params)` — which coordinate

- ```rust
  pub fn nc_or_zero(self: &Self) -> usize { /* ... */ }
  ```
  `Nc`, defaulting to `0` when the field is absent.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Params { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Params { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Geometry`

The `geometry` struct — physical extents and the per-column active-region
bounds computed by `geometry_ends3d.m`.

```rust
pub struct Geometry {
    pub xtot: f64,
    pub ytot: f64,
    pub xlows: Option<crate::matlab::Array2<usize>>,
    pub xhis: Option<crate::matlab::Array2<usize>>,
    pub ylows: Option<crate::matlab::Array2<usize>>,
    pub yhis: Option<crate::matlab::Array2<usize>>,
    pub zlows: Option<crate::matlab::Array2<usize>>,
    pub zhis: Option<crate::matlab::Array2<usize>>,
    pub lx: Vec<f64>,
    pub ly: Vec<f64>,
    pub lz: Vec<f64>,
    pub crodbanks: Option<crate::matlab::Array2<usize>>,
    pub crod: Vec<f64>,
    pub crodstep: f64,
    pub crodbtm: f64,
    pub crodeject: Option<usize>,
    pub crodejectto: f64,
    pub zscale: usize,
    pub fuel: FuelGeometry,
    pub vi: Vec<f64>,
    pub xmin: BoundaryCondition,
    pub xmax: BoundaryCondition,
    pub ymin: BoundaryCondition,
    pub ymax: BoundaryCondition,
    pub zmin: BoundaryCondition,
    pub zmax: BoundaryCondition,
    pub adf: Option<crate::matlab::Array2<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xtot` | `f64` | Total `x` extent of the modelled quadrant. Units follow the case file. |
| `ytot` | `f64` | Total `y` extent of the modelled quadrant. |
| `xlows` | `Option<crate::matlab::Array2<usize>>` | `geometry.xlows(iy, iz)` — first `ix` with material present. |
| `xhis` | `Option<crate::matlab::Array2<usize>>` | `geometry.xhis(iy, iz)` — last `ix` with material present. |
| `ylows` | `Option<crate::matlab::Array2<usize>>` | `geometry.ylows(ix, iz)` — first `iy` with material present. |
| `yhis` | `Option<crate::matlab::Array2<usize>>` | `geometry.yhis(ix, iz)` — last `iy` with material present. |
| `zlows` | `Option<crate::matlab::Array2<usize>>` | `geometry.zlows(ix, iy)` — first `iz` with material present. |
| `zhis` | `Option<crate::matlab::Array2<usize>>` | `geometry.zhis(ix, iy)` — last `iz` with material present. |
| `lx` | `Vec<f64>` | `geometry.Lx` — node width in `x`, one entry per node.<br><br>Length `maxix*maxiy*maxiz`, ordered `ix*maxiy*maxiz + iy*maxiz + iz`.<br>The reference `repmat`s this to `G` groups at each use site rather than<br>storing it per group. Units follow the case file, typically cm. |
| `ly` | `Vec<f64>` | `geometry.Ly` — node width in `y`. As [`Geometry::lx`]. |
| `lz` | `Vec<f64>` | `geometry.Lz` — node height in `z`. As [`Geometry::lx`]. |
| `crodbanks` | `Option<crate::matlab::Array2<usize>>` | `geometry.crodbanks(ix, iy)` — which control-rod bank sits over each<br>lattice position; `0` for none.<br><br>Bank numbers are 1-based and index [`Geometry::crod`]. |
| `crod` | `Vec<f64>` | `geometry.crod(bank)` — each bank's withdrawal, in **steps**. |
| `crodstep` | `f64` | `geometry.crodstep` — the height of one control-rod step, cm. |
| `crodbtm` | `f64` | `geometry.crodbtm` — the axial position of a fully inserted rod tip, cm,<br>measured from the bottom of the core.<br><br>A bank's tip sits at `crodbtm + crod(bank) * crodstep`; nodes **above**<br>that are rodded. |
| `crodeject` | `Option<usize>` | `geometry.crodeject` — which bank is ejected, 1-based; `None` (or the<br>reference's `0`) means the case has no rod motion. |
| `crodejectto` | `f64` | `geometry.crodejectto` — the ejected bank's final position, in steps. |
| `zscale` | `usize` | `geometry.zscale` — mesh layers per axial *block* of the benchmark model.<br><br>`maxiz / <the case's block count>`. Only the transient driver's radial<br>power maps read it, to turn an active-core block number into the mesh<br>layers it spans. |
| `fuel` | `FuelGeometry` | `geometry.fuel` — the fuel-rod radial mesh and materials.<br><br>One rod description shared by the whole core; see [`FuelGeometry`]. |
| `vi` | `Vec<f64>` | `geometry.Vi` — node volume, one entry per node.<br><br>Length `maxix*maxiy*maxiz`, in the same `ix*maxiy*maxiz + iy*maxiz + iz`<br>order as [`Geometry::lx`], and typically cm³ where the case file works<br>in cm. The two flux solvers `repmat` it to `G` groups and multiply the<br>converged fission source by it to get the power density.<br><br>Note [`crate::makegrad_dxyz`] reads `geometry.Vi` and never uses it —<br>that is dead code in the reference and is not why this field exists. |
| `xmin` | `BoundaryCondition` | `geometry.xmin` — boundary condition on the low-`x` face. |
| `xmax` | `BoundaryCondition` | `geometry.xmax` — boundary condition on the high-`x` face. |
| `ymin` | `BoundaryCondition` | `geometry.ymin` — boundary condition on the low-`y` face. |
| `ymax` | `BoundaryCondition` | `geometry.ymax` — boundary condition on the high-`y` face. |
| `zmin` | `BoundaryCondition` | `geometry.zmin` — boundary condition on the low-`z` face. |
| `zmax` | `BoundaryCondition` | `geometry.zmax` — boundary condition on the high-`z` face. |
| `adf` | `Option<crate::matlab::Array2<f64>>` | `geometry.adf` — assembly discontinuity factors, `philen` by **6**.<br><br>Same `(minus, plus)` per-axis column layout as `gradterms`: `0, 1` for<br>`x`, `2, 3` for `y`, `4, 5` for `z`. Dimensionless; `1` everywhere means<br>no discontinuity.<br><br>The reference guards this with `isfield` and substitutes<br>`ones(philen, 6)` when absent, which `None` reproduces. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Geometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Geometry { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `Conductivity`

A temperature-dependent thermal conductivity, W/(cm·K).

# Why this is an enum rather than a function pointer

The reference carries these as a **cell array of anonymous function
handles**, `geometry.fuel.tcon{m}`, built by each case file and invoked as
`tcon{whichk(i)}(T)`. The set of correlations the snapshot actually ships is
closed — two of them, both in `neacrpd1.m` and `neacrpa2.m` — so an enum
gives exhaustive dispatch and keeps the workspace's no-trait-objects rule.
A new correlation is a new variant and a compile error at every `match`.

# The cell array is heterogeneous, and that is not reproduced

`tcon` is sized `max(whichk) + 1`, and its **last** element is not a
function at all: it is a bare scalar gap conductance, used as
`tcon{end} * <length>` and never called. `whichk` only ever takes values
`0`, `1`, `2` — so the last slot is unreachable by the indexed lookup and
exists purely to be read as `tcon{end}`.

Conflating a W/(cm·K) conductivity with a W/(cm²·K) conductance in one
container is the reference's own doing. Here they are split: the
correlations live in [`FuelGeometry::tcon`] and the gap conductance in
[`FuelGeometry::gap_conductance`], which have different units and different
meanings. This is a type-level restructuring in the same spirit as
[`BoundaryCondition`] replacing the reference's strings; it changes no
behaviour.

```rust
pub enum Conductivity {
    Uo2Fuel,
    ZircaloyClad,
    Constant(f64),
}
```

##### Variants

###### `Uo2Fuel`

UO2 fuel: `(1.05 + 2150/(T - 73.15)) / 100`, W/(cm·K), `T` in K.

From `neacrpd1.m` and `neacrpa2.m`. **Singular at `T = 73.15 K`** and
negative below it; the reference does not guard this and neither does
the evaluation here. Fuel temperatures are hundreds of K above it.

###### `ZircaloyClad`

Zircaloy cladding:
`(7.51 + 2.09e-2 T - 1.45e-5 T^2 + 7.67e-9 T^3) / 100`, W/(cm·K).

From `neacrpd1.m` and `neacrpa2.m`.

###### `Constant`

A temperature-independent conductivity, W/(cm·K).

Not used by any case file in the snapshot; provided so a caller can
supply a constant-property material without inventing a correlation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Evaluate at temperature `t` in **K**, returning W/(cm·K).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Conductivity { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Conductivity) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `VolumetricHeatCapacity`

A temperature-dependent **volumetric** heat capacity, J/(cm³·K).

`geometry.fuel.rhocp` in the reference, a `cell(2,1)` of function handles
built by `neacrpa1t.m` — the transient driver, and the only file that sets
it. As [`Conductivity`], the closed set becomes an enum.

# This is `rho * cp`, already multiplied out

Both correlations are written as `density * specific_heat / 1000`: the
density in g/cm³, the specific heat in J/(kg·K), and the `/1000` converting
the product to J/(cm³·K). Nothing downstream ever needs the two factors
separately.

# It is indexed differently from `tcon`

[`FuelGeometry::tcon`] has `max(whichk) + 1` entries, the last being the gap
conductance. `rhocp` has exactly `max(whichk)` — **the gap carries no heat
capacity**, and the transient stencil skips it rather than looking one up.

```rust
pub enum VolumetricHeatCapacity {
    Uo2Fuel,
    ZircaloyClad,
    Constant(f64),
}
```

##### Variants

###### `Uo2Fuel`

UO2 fuel at 98.752% of theoretical density:
`10.412 * (1 - 0.01248) * (162.3 + 0.3038 T - 2.391e-4 T^2
+ 6.404e-8 T^3) / 1000`, J/(cm³·K), `T` in K.

From `neacrpa1t.m`. The leading `10.412` is the UO2 density in g/cm³ and
the `(1 - 0.01248)` its porosity correction.

###### `ZircaloyClad`

Zircaloy cladding: `6.6 * (252.54 + 0.11474 T) / 1000`, J/(cm³·K).

From `neacrpa1t.m`; `6.6` g/cm³ is the Zircaloy density.

###### `Constant`

A temperature-independent volumetric heat capacity, J/(cm³·K).

Not used by any case file in the snapshot; provided so a caller can
supply a constant-property material.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Evaluate at temperature `t` in **K**, returning J/(cm³·K).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VolumetricHeatCapacity { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VolumetricHeatCapacity) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelParams`

`params.fuel` — the fuel-rod radial mesh sizes.

The reference passes this sub-struct where a function's signature says
`params`, so `makeheatlaplacian_1dcylnd(params.fuel, geometry.fuel, ...)`
reads `params.maxir` and means `params.fuel.maxir`.

```rust
pub struct FuelParams {
    pub maxir: usize,
    pub fueln: usize,
    pub gapn: usize,
    pub cladn: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `maxir` | `usize` | `params.fuel.maxir` — total radial node count, fuel + gap + cladding. |
| `fueln` | `usize` | `params.fuel.fueln` — radial nodes inside the fuel pellet. |
| `gapn` | `usize` | `params.fuel.gapn` — radial nodes across the fuel-cladding gap. |
| `cladn` | `usize` | `params.fuel.cladn` — radial nodes through the cladding. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelParams { /* ... */ }
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
    fn default() -> FuelParams { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelGeometry`

`geometry.fuel` — the 1-D cylindrical fuel-rod discretisation.

One radial mesh, shared by every axial node of every channel: the rod
geometry does not vary across the core in any case the snapshot ships. The
per-node quantities that *do* vary (power, coolant temperature) are passed
to the conduction solver as scalars.

# Units

Lengths cm, areas cm², volumes cm³ — the whole reference works in cm.

```rust
pub struct FuelGeometry {
    pub lr: Vec<f64>,
    pub ctr: Vec<f64>,
    pub vi: Vec<f64>,
    pub whichk: Vec<usize>,
    pub tcon: Vec<Conductivity>,
    pub rhocp: Vec<VolumetricHeatCapacity>,
    pub gap_conductance: f64,
    pub fuelrad: f64,
    pub rtot: f64,
    pub pitch: f64,
    pub subarea: f64,
    pub hydia: f64,
    pub doppleralpha: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lr` | `Vec<f64>` | `geometry.fuel.Lr(ir)` — radial node thickness, cm. Length `maxir`. |
| `ctr` | `Vec<f64>` | `geometry.fuel.Ctr(ir)` — radius of each node **centre**, cm:<br>`sum(Lr(1:ir)) - 0.5*Lr(ir)`. |
| `vi` | `Vec<f64>` | `geometry.fuel.Vi(ir)` — node volume per unit length, cm³/cm.<br><br>The innermost is `pi*Lr(1)^2`; the rest are annular shells. |
| `whichk` | `Vec<usize>` | `geometry.fuel.whichk(ir)` — which material occupies node `ir`.<br><br>**`0` means the gap**, `1` the fuel, `2` the cladding. A non-zero value<br>`m` selects `tcon[m - 1]`; `0` selects [`FuelGeometry::gap_conductance`]<br>instead, and marks a node the conduction stencil bridges rather than<br>solves through. |
| `tcon` | `Vec<Conductivity>` | The per-material conductivity correlations, indexed by `whichk - 1`.<br><br>See [`Conductivity`] for why this is not the reference's cell array. |
| `rhocp` | `Vec<VolumetricHeatCapacity>` | `geometry.fuel.rhocp` — volumetric heat capacity per material,<br>J/(cm³·K), indexed by `whichk - 1`.<br><br>Read only by [`crate::fuelrodheattime_1dcylnd`]; the steady conduction<br>solver has no time term and never touches it. **Exactly<br>`max(whichk)` entries** — unlike [`FuelGeometry::tcon`], there is no<br>trailing gap element, because the gap carries no heat capacity. |
| `gap_conductance` | `f64` | The fuel-cladding **gap conductance**, W/(cm²·K) — `tcon{end}`.<br><br>`0.35` in `neacrpd1.m`, attributed there to the NEACRP benchmark. Note<br>the units differ from [`FuelGeometry::tcon`]'s: this is a conductance<br>across a gap of unresolved width, not a conductivity. |
| `fuelrad` | `f64` | `geometry.fuel.fuelrad` — the fuel pellet radius, cm. |
| `rtot` | `f64` | `geometry.fuel.Rtot` — the outer cladding radius, cm. |
| `pitch` | `f64` | `geometry.fuel.pitch` — the lattice pitch, cm. |
| `subarea` | `f64` | `geometry.fuel.subarea` — coolant flow area per pin, cm².<br><br>`th_solverxyz.m` recomputes this as `pitch^2 - pi*Rtot^2` rather than<br>reading the field, so the two can disagree; `w3chf.m` reads the field. |
| `hydia` | `f64` | `geometry.fuel.hydia` — subchannel hydraulic diameter, cm.<br><br>As [`FuelGeometry::subarea`], `th_solverxyz.m` recomputes rather than<br>reads. |
| `doppleralpha` | `f64` | `geometry.fuel.doppleralpha` — the weight on the pellet-surface<br>temperature in the Doppler average, dimensionless on `[0, 1]`.<br><br>`Tdoppler = (1 - alpha)*T_centre + alpha*T_surface`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FuelGeometry { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `AxisField`

A quantity carried per axis, one `philen` vector each.

The reference builds several of these as bare structs with `.x`, `.y` and
`.z` fields — `A2` and `A4` from the nodal expansion among them. Structurally
identical to [`crate::calc_transleakagexyz::Leakage`]; kept separate because
the two mean different things and mixing them up would type-check.

```rust
pub struct AxisField {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | The `x` component. |
| `y` | `Vec<f64>` | The `y` component. |
| `z` | `Vec<f64>` | The `z` component. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisField { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> AxisField { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `BoundaryCondition`

Outer boundary condition on one face of the core.

The reference carries these as the strings `'vacuum'`, `'zeroflux'` and
`'reflective'`, dispatched on with `switch`.

# `Vacuum` and `ZeroFlux` are not distinguished

Every `switch` in the translated code groups them — `case {'vacuum',
'zeroflux'}` — so they produce identical coefficients. They are kept as
separate variants because the case files set them separately and the
distinction may matter to code not yet translated.

# An unrecognised string silently gives zero

The reference's `switch` statements have no `otherwise` branch, so a
boundary condition that is none of the three leaves the preallocated `0` in
place — a silently absent boundary term rather than an error. The enum makes
that unrepresentable, which narrows the input domain rather than changing
behaviour for any valid input.

```rust
pub enum BoundaryCondition {
    Vacuum,
    ZeroFlux,
    Reflective,
}
```

##### Variants

###### `Vacuum`

`'vacuum'` — no incoming current.

###### `ZeroFlux`

`'zeroflux'` — flux forced to zero at the face. Treated identically to
[`BoundaryCondition::Vacuum`] everywhere in the translated code.

###### `Reflective`

`'reflective'` — zero net current, a symmetry plane.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryCondition { /* ... */ }
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
    fn default() -> BoundaryCondition { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryCondition) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Coolant`

`th.coolant` — the coolant thermodynamic state, one entry per core node.

Every vector is `maxix*maxiy*maxiz` long in the usual
`ix*maxiy*maxiz + iy*maxiz + iz` order, except the two inlet scalars.

# Units — cm-g-s, not SI

The reference works in centimetres and grams throughout, and mixes in MPa
for pressure and kJ/kg for enthalpy because that is what its IAPWS
implementation returns. Each field states its own unit; the ones that catch
people out are density in **g/cm³** (not kg/m³) and velocity in **cm/s**.

# Growing this struct

As [`Params`], the field set is deliberately incomplete and grows as the
thermal-hydraulics modules are ported. Only fields a translated `.m` file
actually reads appear here.

```rust
pub struct Coolant {
    pub inlettemp: f64,
    pub inletpress: f64,
    pub inletvoid: f64,
    pub press: Vec<f64>,
    pub temps: Vec<f64>,
    pub enth: Vec<f64>,
    pub enthface: Vec<f64>,
    pub quality: Vec<f64>,
    pub alphag: Vec<f64>,
    pub vm: Vec<f64>,
    pub ldens: Vec<f64>,
    pub gdens: Vec<f64>,
    pub dens: Vec<f64>,
    pub kvis: Vec<f64>,
    pub pran: Vec<f64>,
    pub tcon: Vec<f64>,
    pub vliq: Vec<f64>,
    pub vgas: Vec<f64>,
    pub tempsliq: Vec<f64>,
    pub tempsgas: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inlettemp` | `f64` | `th.coolant.inlettemp` — inlet temperature, **K**. Scalar. |
| `inletpress` | `f64` | `th.coolant.inletpress` — inlet pressure, **MPa**. Scalar. |
| `inletvoid` | `f64` | `th.coolant.inletvoid` — inlet void fraction, dimensionless. Scalar.<br><br>Read by [`crate::driftflux6_solverstatic3d`] to set the inlet mixture<br>density; zero for a subcooled inlet. |
| `press` | `Vec<f64>` | `th.coolant.press` — pressure per node, **MPa**. |
| `temps` | `Vec<f64>` | `th.coolant.temps` — bulk temperature per node, **K**. |
| `enth` | `Vec<f64>` | `th.coolant.enth` — bulk specific enthalpy per node, **kJ/kg**.<br><br>Cell-**centred**: in the transient scheme it is the mean of the node's<br>two face values. |
| `enthface` | `Vec<f64>` | `th.coolant.enthface` — cell-**face** specific enthalpy, **kJ/kg**.<br><br>Written only by [`crate::singleflow1devaptime`], which solves for the<br>faces and derives the centres from them. The steady solver leaves it<br>empty. |
| `quality` | `Vec<f64>` | `th.coolant.quality` — thermodynamic equilibrium quality, mass<br>fraction. Negative in subcooled liquid, which the W-3 correlation<br>relies on. |
| `alphag` | `Vec<f64>` | `th.coolant.alphag` — void fraction, dimensionless on `[0, 1]`. |
| `vm` | `Vec<f64>` | `th.coolant.vm` — mixture velocity, **cm/s**. |
| `ldens` | `Vec<f64>` | `th.coolant.ldens` — saturated **liquid** density, g/cm³. |
| `gdens` | `Vec<f64>` | `th.coolant.gdens` — saturated **vapour** density, g/cm³. |
| `dens` | `Vec<f64>` | `th.coolant.dens` — mixture density, g/cm³. |
| `kvis` | `Vec<f64>` | `th.coolant.kvis` — kinematic viscosity, cm²/s. |
| `pran` | `Vec<f64>` | `th.coolant.pran` — Prandtl number, dimensionless. |
| `tcon` | `Vec<f64>` | `th.coolant.tcon` — coolant thermal conductivity, W/(cm·K).<br><br>Distinct from [`FuelGeometry::tcon`], which is a set of correlations for<br>solid materials; this is an already-evaluated per-node value. |
| `vliq` | `Vec<f64>` | `th.coolant.vliq` — **liquid** phase velocity, cm/s.<br><br>The six-equation two-fluid model tracks the phases separately, so this<br>and [`Coolant::vgas`] replace the single [`Coolant::vm`] that the<br>homogeneous model uses. `vm` is still filled, as their mass-weighted<br>mean. |
| `vgas` | `Vec<f64>` | `th.coolant.vgas` — **vapour** phase velocity, cm/s. |
| `tempsliq` | `Vec<f64>` | `th.coolant.tempsliq` — **liquid** phase temperature, K.<br><br>The two-fluid model allows the phases to be at different temperatures,<br>so neither is `Tsat` in general. `temps` is set equal to this one for<br>compatibility with the downstream code, which expects a single<br>temperature. |
| `tempsgas` | `Vec<f64>` | `th.coolant.tempsgas` — **vapour** phase temperature, K. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Coolant { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Coolant { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `ThModel`

Which channel model `th_solverxyz.m` uses for the coolant.

The reference selects on the string `params.th_model`, testing
`strcmpi(params.th_model, 'hem')` and defaulting to the two-fluid path.

# Only one of these can actually run

[`ThModel::TwoFluid`] routes to [`crate::driftflux6_solverstatic3d`], whose
per-channel solver is **absent from the snapshot** — so it retains the
previous state rather than solving. [`ThModel::Hem`] routes to
[`crate::singleflow1devap`], which works. The NEACRP D1 BWR case sets
`'hem'`.

```rust
pub enum ThModel {
    TwoFluid,
    Hem,
}
```

##### Variants

###### `TwoFluid`

`'twofluid'` — the staggered six-equation per-channel wrapper. The
reference's default, and the branch taken by any unrecognised string.

###### `Hem`

`'hem'` — the homogeneous-equilibrium enthalpy march.

The reference's comment explains why this exists: the transient driver
marches the HEM model, so a transient needs its `t = 0` steady state
from the **same** model. A two-fluid steady state has less void than
HEM at the same conditions, and handing that to the transient would be a
density mismatch — a spurious reactivity step at `t = 0`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThModel { /* ... */ }
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
    fn default() -> ThModel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `FlowDirection`

Which way the coolant flows along `z`.

The reference carries this as `th.flowdir`, an integer tested `== -1`. Any
other value means upward, so the two-variant enum loses nothing.

```rust
pub enum FlowDirection {
    Up,
    Down,
}
```

##### Variants

###### `Up`

Increasing `z` — the inlet is at `zlow`. Every value except `-1`.

###### `Down`

Decreasing `z` — the inlet is at `zhi`. The reference's `flowdir == -1`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FlowDirection { /* ... */ }
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
    fn default() -> FlowDirection { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FlowDirection) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `MassFlux`

`th.flowrate` — coolant mass flux, **g/(s·cm²)**.

The reference accepts either a scalar or a per-node vector and expands the
scalar with `if isscalar(flowrate)`. An enum keeps that choice visible
rather than making every caller pre-expand.

```rust
pub enum MassFlux {
    Uniform(f64),
    PerNode(Vec<f64>),
}
```

##### Variants

###### `Uniform`

One value for the whole core.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `PerNode`

One value per node, in the usual flattened order.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  The mass flux at node `i`, **g/(s·cm²)**.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MassFlux { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Th`

The `th` struct — the thermal-hydraulic state passed through the coupling.

# Growing this struct

As [`Coolant`], deliberately incomplete.

```rust
pub struct Th {
    pub coolant: Coolant,
    pub heatflux: Vec<f64>,
    pub maxpow: f64,
    pub powratio: f64,
    pub nfuelpin: f64,
    pub coolheatfrac: f64,
    pub flowrate: MassFlux,
    pub flowdir: FlowDirection,
    pub stag6_ustag: crate::matlab::Array2<f64>,
    pub stag6_qref: crate::matlab::Array2<f64>,
    pub stag6_relerr: Vec<f64>,
    pub fueltemp: crate::matlab::Array2<f64>,
    pub fueltempavg: Vec<f64>,
    pub fueltempdoppler: Vec<f64>,
    pub linpwrdens: Vec<f64>,
    pub modtemp: Vec<f64>,
    pub inlettemp_t: InletForcing,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `coolant` | `Coolant` | The coolant state. |
| `heatflux` | `Vec<f64>` | `th.heatflux` — wall heat flux per node, **W/cm²**. |
| `maxpow` | `f64` | `th.maxpow` — total core thermal power, **W**.<br><br>Multiplies the normalised power density to give absolute power. |
| `powratio` | `f64` | `th.powratio` — fraction of rated power the case runs at,<br>dimensionless. |
| `nfuelpin` | `f64` | `th.nfuelpin` — fuel pins per node.<br><br>The reference wraps this in `double(...)`, so a case file may supply it<br>as an integer type. |
| `coolheatfrac` | `f64` | `th.coolheatfrac` — fraction of fission power deposited **directly in<br>the coolant** rather than in the fuel, dimensionless.<br><br>The complement, `1 - coolheatfrac`, is what heats the pins. |
| `flowrate` | `MassFlux` | `th.flowrate` — coolant mass flux, g/(s·cm²). |
| `flowdir` | `FlowDirection` | `th.flowdir` — which way the coolant flows along `z`. |
| `stag6_ustag` | `crate::matlab::Array2<f64>` | `th.stag6_Ustag` — the per-channel state vector the staggered solver<br>reuses as a warm start, `6*maxiz` rows by `maxix*maxiy` channels.<br><br>Threaded through the coupled Picard loop in `th` rather than returned<br>separately, because the coupling layer under-relaxes only a few named<br>fields and this survives intact between cycles. |
| `stag6_qref` | `crate::matlab::Array2<f64>` | `th.stag6_qref` — the wall heat flux each stored warm start was computed<br>at, `maxiz` by channels. A seed is only reused while the flux has not<br>moved much. |
| `stag6_relerr` | `Vec<f64>` | `th.stag6_relerr` — the relative residual each channel's last solve<br>reached, one per channel. `NaN` where a channel has never been solved. |
| `fueltemp` | `crate::matlab::Array2<f64>` | `th.fueltemp` — the radial temperature profile at each core node, **K**.<br><br>`maxix*maxiy*maxiz` rows by `maxid` columns, where `maxid` is the<br>fuel-rod unknown count [`crate::fuelrodheat_1dcylnd`] describes. Row<br>`idx` is one rod's profile from centre to cladding surface. |
| `fueltempavg` | `Vec<f64>` | `th.fueltempavg` — the fuel temperature fed to the cross-section<br>feedback, **K**, one per node.<br><br>Despite the name this is **not** a volume average: `th_solverxyz.m`<br>assigns it equal to [`Th::fueltempdoppler`], with the volume-averaging<br>line commented out. See that module. |
| `fueltempdoppler` | `Vec<f64>` | `th.fueltempdoppler` — the Doppler-weighted fuel temperature, **K**.<br><br>`(1 - alpha) * T_centre + alpha * T_pellet_surface`, with `alpha` from<br>[`FuelGeometry::doppleralpha`]. |
| `linpwrdens` | `Vec<f64>` | `th.linpwrdens` — linear power density, **W/cm** per node. |
| `modtemp` | `Vec<f64>` | `th.modtemp` — moderator temperature, **K**, one per node.<br><br>Distinct from `coolant.temps` in a design where the moderator and the<br>coolant are different fluids. For the LWR cases in the snapshot they<br>coincide, but the cross-section tables address them separately. |
| `inlettemp_t` | `InletForcing` | `th.inlettemp_t` — a prescribed time-dependent inlet temperature.<br><br>The reference stores a MATLAB function handle here and the transient<br>driver evaluates it at the start of every step, overwriting<br>`coolant.inlettemp`. Function handles cannot cross into Rust and this<br>workspace forbids trait objects, so the forcing is an **enum** of the<br>shapes the snapshot actually uses; see [`InletForcing`]. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Th { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Th { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `InletForcing`

A prescribed time-dependent coolant inlet condition.

Replaces the reference's `th.inlettemp_t` function handle. Adding a new
forcing law means adding a variant, which the compiler then forces every
match site to handle — the reason this workspace prefers enums to trait
objects.

```rust
pub enum InletForcing {
    Steady,
    ExponentialSubcooling {
        pressure: f64,
        dh0: f64,
        rate: f64,
    },
}
```

##### Variants

###### `Steady`

No forcing: the inlet stays at `coolant.inlettemp` throughout.

###### `ExponentialSubcooling`

NEACRP case D1's cold-water injection, benchmark Fig. 6.1.

The inlet enthalpy sits `dh(t)` below the saturated-liquid value at
`pressure`, with the subcooling growing from `dh0` to `2*dh0`:

```text
dh(t) = dh0 * (2 - exp(-rate * t))     kJ/kg
```

At `t = 0` this is exactly `dh0`, so it is continuous with the steady
inlet the case file sets. The temperature is recovered through the
IF97 backward equation at the (constant) core pressure.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Core pressure, MPa. |
| `dh0` | `f64` | The steady-state subcooling, kJ/kg. |
| `rate` | `f64` | The approach rate, 1/s. |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, t: f64) -> Option<f64> { /* ... */ }
  ```
  The inlet temperature at time `t` in **K**, or `None` when the case

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> InletForcing { /* ... */ }
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
    fn default() -> InletForcing { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InletForcing) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `TimeScheme`

Which kinetics discretisation the transient driver marches.

The reference selects with the integer `params.timescheme`.

```rust
pub enum TimeScheme {
    ExponentialTransform,
    ImplicitEuler,
}
```

##### Variants

###### `ExponentialTransform`

`1`, the reference's default — exponential-transform implicit Euler for
the flux with analytic precursor integration over a linearly varying
transformed fission source.

The scheme of the nodal program Ants (A. Rintala, U. Lauranto, *Ann.
Nucl. Energy* **190** (2023) 109868, Eqs. (3)-(13)).

###### `ImplicitEuler`

`0` — plain implicit Euler for both flux and precursors. First order,
and described in the reference as the legacy scheme.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TimeScheme { /* ... */ }
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
    fn default() -> TimeScheme { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TimeScheme) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `FreqMode`

How the exponential-transform frequencies are taken.

```rust
pub enum FreqMode {
    Global,
    Node,
}
```

##### Variants

###### `Global`

`'global'`, the reference's default — one amplitude frequency per
energy group, uniform in space, from the volume-integrated group flux.

Robust: it captures the stiff point-kinetics-like exponential rise
exactly, which is what a super-prompt rod ejection needs.

###### `Node`

`'node'` — per-node, per-group frequencies as written in the Ants paper.

Slightly more accurate for shape transients, and **unstable in
super-prompt rod ejections**: the reference's own comment records that
node-wise frequency noise near the ejected channel feeds back through
the nearly singular prompt operator.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FreqMode { /* ... */ }
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
    fn default() -> FreqMode { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FreqMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SigmaValues`

The `sigmavalues` struct — per-**material** cross-section data, as read
from the benchmark case files.

This is the *input* to [`crate::makesigmadfxyz::makesigmadfxyz`], which
expands it onto the spatial mesh to produce [`Sigma`]. Material rows are
0-based here; the identifiers stored in `whichsigma` are 1-based with `0`
for void, so a node holding material `m` reads row `m - 1`.

```rust
pub struct SigmaValues {
    pub tot: crate::matlab::Array2<f64>,
    pub f: crate::matlab::Array2<f64>,
    pub s: crate::matlab::Array3<f64>,
    pub nu: crate::matlab::Array2<f64>,
    pub chi: crate::matlab::Array2<f64>,
    pub fp: Option<crate::matlab::Array2<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tot` | `crate::matlab::Array2<f64>` | `sigmavalues.tot(material, g)` — total cross section, cm<sup>-1</sup>. |
| `f` | `crate::matlab::Array2<f64>` | `sigmavalues.f(material, g)` — fission cross section, cm<sup>-1</sup>. |
| `s` | `crate::matlab::Array3<f64>` | `sigmavalues.s(material, gt, g)` — scattering from group `g` **into**<br>group `gt`. Note the destination index comes first. |
| `nu` | `crate::matlab::Array2<f64>` | `sigmavalues.nu(material, g)` — neutrons per fission.<br><br>The reference accepts a scalar here and expands it; see<br>[`crate::makesigmadfxyz::makesigmadfxyz`] for how, and for the<br>inconsistent indexing that follows. |
| `chi` | `crate::matlab::Array2<f64>` | `sigmavalues.chi(material, gt)` — fission spectrum, the fraction of<br>fission neutrons born into group `gt`. Dimensionless, sums to 1 over<br>`gt`. |
| `fp` | `Option<crate::matlab::Array2<f64>>` | `sigmavalues.fp(material, g)` — prompt fission cross section.<br><br>Optional in the reference, which substitutes zeros when the field is<br>absent. `None` reproduces that. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaValues { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SigmaValues { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Sigma`

The `sigma` struct — the multigroup cross-section **operators**, expanded
onto the spatial mesh.

Each matrix is `philenf` square over the flattened `(group, node)` index
space, so a single matrix carries both the within-group and the
group-to-group coupling. Produced by
[`crate::makesigmadfxyz::makesigmadfxyz`].

```rust
pub struct Sigma {
    pub tot: crate::matlab::SparseMatrix,
    pub s: crate::matlab::SparseMatrix,
    pub f: crate::matlab::SparseMatrix,
    pub fp: crate::matlab::SparseMatrix,
    pub fb: crate::matlab::SparseMatrix,
    pub sd: crate::matlab::SparseMatrix,
    pub nu: Vec<f64>,
    pub chi: crate::matlab::Array2<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tot` | `crate::matlab::SparseMatrix` | `sigma.tot` — total cross section, diagonal. Units cm<sup>-1</sup>. |
| `s` | `crate::matlab::SparseMatrix` | `sigma.s` — scattering, including the group-to-group off-diagonals. |
| `f` | `crate::matlab::SparseMatrix` | `sigma.f` — fission production `chi * nu * Sigma_f`, divided by `keff`<br>where it enters the buckling. |
| `fp` | `crate::matlab::SparseMatrix` | `sigma.fp` — the prompt part of `sigma.f`, built as `chi * Sigma_fp`.<br><br>Note this carries **no** `nu` factor, where [`Sigma::f`] does. |
| `fb` | `crate::matlab::SparseMatrix` | `sigma.fb` — bare fission cross section on the diagonal, without the<br>`chi` or `nu` factors. |
| `sd` | `crate::matlab::SparseMatrix` | `sigma.sd` — the within-group scattering `Sigma_s(g -> g)` on the<br>diagonal only. |
| `nu` | `Vec<f64>` | `sigma.nu` — neutrons per fission, one entry per `(group, node)`. |
| `chi` | `crate::matlab::Array2<f64>` | `sigma.chi` — fission spectrum, `G` rows by `philen` columns. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Sigma { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Sigma { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `calc_1sttransleakagexyz`

First-moment transverse leakages — the linear term of the quadratic
transverse-leakage fit.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_1sttransleakagexyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_1sttransleakagexyz { /* ... */ }
```

### Functions

#### Function `calc_1sttransleakagexyz`

`Leakage = calc_1sttransleakagexyz(params, geometry, Leakzero, diffvalues)`.

Fits the **first moment** of the transverse leakage on each axis from the
zeroth-moment leakages of the other two.

# The transverse coupling

The source on each axis is the sum of the leakages on the **other two**:

```text
Ssource.x = Leakzero.y + Leakzero.z
Ssource.y = Leakzero.x + Leakzero.z
Ssource.z = Leakzero.x + Leakzero.y
```

That is what makes the leakage *transverse* — the 1-D nodal equation along
`x` is driven by what leaks out through the `y` and `z` faces.

# Arguments

- `params` — supplies `G` and the extents.
- `geometry` — node widths, per-line active bounds, and the six face
  boundary conditions.
- `leakzero` — the zeroth-moment leakages from
  [`crate::calc_transleakagexyz::calc_transleakagexyz`].
- `diffvalues` — **flat `philen` vector**, as elsewhere in this chain.

# Returns

[`Leakage`] — three `philen` vectors of first-moment coefficients. Entries
for nodes outside the core stay zero.

# Interior stencil

With mesh ratios `tp = L_plus / L` and `tm = L_minus / L`:

```text
h  = 2 (tp + 1)(tm + 1)(tm + tp + 1)
LL = [ (tm+1)(2tm+1)(S_plus - S) + (tp+1)(2tp+1)(S - S_minus) ] / h
```

then scaled by `0.25 * L^2 / D`. On a uniform mesh `tp = tm = 1` and this
collapses to the centred difference `(S_plus - S_minus) / 4`, scaled the
same way.

# Boundary faces

One-sided, with `h = 4 (t + 1)(t + 2)`:

- `Vacuum` / `ZeroFlux` — `(S_plus - S) / (t + 1)`
- `Reflective` — `6 (S_plus - S) / h`

and the mirror image at the high face. Both are then scaled by
`0.25 * L^2 / D` exactly as the interior is.

# The `diffvalues` test differs from `calc_transleakagexyz`

Worth knowing when comparing the two files. At a boundary face this
reference tests `diffvalues(idx)` **with** the group offset, whereas
`calc_transleakagexyz.m` tests the bare node index — group 1 only. The
interior selection is the bare node index in both.

For cross sections that make every group of a node void together — which is
what `calcdiffvalues3d` produces — the two tests agree. They would diverge
only for a node void in some groups and not others. Translated as written in
each file rather than harmonised.

# Panics

If a boundary node's neighbour index falls outside the vector — the same
two-node-minimum constraint documented on
[`crate::calc_transleakagexyz::calc_transleakagexyz`].

```rust
pub fn calc_1sttransleakagexyz(params: &crate::types::Params, geometry: &crate::types::Geometry, leakzero: &crate::calc_transleakagexyz::Leakage, diffvalues: &[f64]) -> crate::calc_transleakagexyz::Leakage { /* ... */ }
```

## Module `calc_2ndtransleakagexyz`

Second-moment transverse leakages — the quadratic term of the
transverse-leakage fit.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_2ndtransleakagexyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_2ndtransleakagexyz { /* ... */ }
```

### Functions

#### Function `calc_2ndtransleakagexyz`

`Leakage = calc_2ndtransleakagexyz(params, geometry, Leakzero, diffvalues)`.

Fits the **second moment** of the transverse leakage on each axis. The
transverse coupling is identical to
[`crate::calc_1sttransleakagexyz::calc_1sttransleakagexyz`] — each axis is
driven by the sum of the other two axes' zeroth-moment leakages — and so are
the arguments, returns and the two-node-minimum constraint.

# How it differs from the first moment

Three changes, all in the formulas:

**Interior.** The mesh-ratio weights lose their `(2t + 1)` factors, and the
minus term reverses sign:

```text
first:   LL = [ (tm+1)(2tm+1)(S_p - S) + (tp+1)(2tp+1)(S - S_m) ] / h
second:  LL = [ (tm+1)        (S_p - S) + (tp+1)        (S_m - S) ] / h
```

with the same `h = 2 (tp+1)(tm+1)(tm+tp+1)` and the same `0.25 L^2 / D`
scaling. On a uniform mesh the second-moment stencil collapses to
`(S_p + S_m - 2S) / 12` — a discrete second derivative, which is **exactly
zero for a linear source**. That is the sense in which it is the quadratic
term.

**Vacuum and zero-flux faces contribute nothing.** Where the first-moment
version computes a one-sided difference, this one's `switch` runs
`continue`, leaving the preallocated zero in place. Only a reflective face
gets a value.

**Reflective faces use `2/h` rather than `6/h`**, and the high face takes
`S_minus - S` rather than `S - S_minus` — matching the reversed interior
sign convention.

# Panics

If a boundary node's neighbour index falls outside the vector — the same
two-node-minimum constraint documented on
[`crate::calc_transleakagexyz::calc_transleakagexyz`].

```rust
pub fn calc_2ndtransleakagexyz(params: &crate::types::Params, geometry: &crate::types::Geometry, leakzero: &crate::calc_transleakagexyz::Leakage, diffvalues: &[f64]) -> crate::calc_transleakagexyz::Leakage { /* ... */ }
```

## Module `calc_a1234_expansionxyz`

The full `A1`–`A4` semi-analytic nodal expansion.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_a1234_expansionxyz.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_a1234_expansionxyz { /* ... */ }
```

### Types

#### Struct `A3`

The `A3` coefficients — same six-field shape as
[`crate::calc_a1_expansionxyz::A1`], because `A3` is built from `A1` and
inherits its `*first` variants.

```rust
pub struct A3 {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub xfirst: Vec<f64>,
    pub yfirst: Vec<f64>,
    pub zfirst: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | `A3.x`. |
| `y` | `Vec<f64>` | `A3.y`. |
| `z` | `Vec<f64>` | `A3.z`. |
| `xfirst` | `Vec<f64>` | `A3.xfirst`, from `A1.xfirst`. |
| `yfirst` | `Vec<f64>` | `A3.yfirst`. |
| `zfirst` | `Vec<f64>` | `A3.zfirst`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> A3 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> A3 { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Expansion`

All four expansion coefficients, as the reference's
`[A1, A2, A3, A4]` return.

```rust
pub struct Expansion {
    pub a1: crate::calc_a1_expansionxyz::A1,
    pub a2: crate::types::AxisField,
    pub a3: A3,
    pub a4: crate::types::AxisField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a1` | `crate::calc_a1_expansionxyz::A1` | First-order coefficient, with `*first` boundary variants. |
| `a2` | `crate::types::AxisField` | Second-order coefficient. |
| `a3` | `A3` | Third-order coefficient, with `*first` boundary variants. |
| `a4` | `crate::types::AxisField` | Fourth-order coefficient. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Expansion { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Expansion { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `calc_a1234_expansionxyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`[A1,A2,A3,A4] = calc_a1234_expansionxyz(params, geometry, phivec, sigma, diffvaluesD, gradterms, nodaltermsold, keff)`.

The driver of the semi-analytic nodal expansion. It calls the leakage and
buckling routines, solves for `A2`, builds `A4` from it, delegates `A1`, and
finally builds `A3` from `A1`.

# Order of operations

1. `Leakage` — [`calc_transleakagexyz`]
2. `Buck` — [`calc_bucklingxyz`]
3. `Leakage1`, `Leakage2` — the first and second moments
4. `A2` from `(diag(Ee)·Buck + 3I) A2 = Buck·phi - Ee·Leakage2 + Ssource`
5. `A4 = Bb · (Buck·A2 + Leakage2)`
6. `A1` — [`calc_a1_expansionxyz`]
7. `A3 = Aa · (Buck·A1 + Leakage1)`, and likewise for the `*first` variants

# Arguments

- `params`, `geometry` — as elsewhere.
- `coeffs` — `Aa`/`Bb`/`Ee` here, plus `Ff`/`Gg`/`Hh` passed through to
  [`calc_a1_expansionxyz`]. **The reference reads these from
  `geometry.nodalcoeffs`**; passed explicitly for the reason given on that
  function.
- `phivec` — the flux, `philen` long.
- `sigma` — cross-section operators, for the buckling.
- `diffvalues_d` — the **flat `philen`** diffusion vector.
- `gradterms`, `nodaltermsold` — `philen` by 6.
- `keff` — current eigenvalue estimate.
- `buck_cache` — carried across calls; see [`BucklingCache`].

# The `A2` solve is block-diagonal, not a general sparse solve

The reference writes

```text
Atemp.x = spdiags(Ee.x,0,philen,philen)*Buck.x + 3*speye(philen);
A2.x    = Atemp.x \ btemp.x;
```

which looks like a `philen`-square sparse solve. It is not, in substance:
`Buck` couples energy groups **only at the same spatial node** — the
reference states this itself in `calc_a1_expansionxyz.m` — so `Atemp` is
block-diagonal with one `G`-by-`G` block per node, and scaling by a diagonal
and adding `3I` preserves that. The system therefore decomposes exactly into
`es` independent `G`-by-`G` solves.

This translation solves it that way, via [`crate::matlab::solve_dense`].
**The decomposition is exact, not an approximation**, so no sparse-solver
dependency is needed here.

The one caveat worth stating: MATLAB's `mldivide` would factor the whole
sparse matrix, so its rounding differs from a per-block factorisation at the
last-bits level. The results agree to round-off, not bit-for-bit. If a
future parity check needs bit equality against a MATLAB run, this is the
place it will show up first.

# `diffvaluesDfix` — division-by-zero guard on one term only

The reference makes a **second copy** of the diffusion vector with zeros
replaced by `1000000`, and uses it **only** for the `Ssource` division:

```text
diffvaluesDfix=diffvaluesD;
diffvaluesDfix(diffvaluesDfix==0)=1000000;
```

Every other consumer — the leakage trio, the buckling, `calc_a1_expansion` —
receives the unmodified vector with genuine zeros intact. So a void node
contributes `Ssource ≈ 0` here (a large denominator) rather than `Inf`,
while remaining a true void everywhere else. The magic number is the
reference's; it is a guard, not a physical diffusion coefficient.

# Returns

[`Expansion`] — all four coefficients.

```rust
pub fn calc_a1234_expansionxyz(params: &crate::types::Params, geometry: &crate::types::Geometry, coeffs: &crate::calc_abefghxyz::Coeffs, phivec: &[f64], sigma: &mut crate::types::Sigma, diffvalues_d: &[f64], gradterms: &crate::matlab::Array2<f64>, nodaltermsold: &crate::matlab::Array2<f64>, keff: f64, buck_cache: &mut crate::calc_bucklingxyz::BucklingCache) -> Expansion { /* ... */ }
```

## Module `calc_a1_expansionxyz`

The `A1` coefficient of the semi-analytic nodal expansion.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_a1_expansionxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_a1_expansionxyz { /* ... */ }
```

### Types

#### Struct `A1`

The `A1` expansion coefficients.

Six vectors, not three. The `*first` variants are computed at the **low**
boundary face of each grid line and are consumed separately by
`calc_a1234_expansionxyz` to build `A3.xfirst`/`yfirst`/`zfirst`.

```rust
pub struct A1 {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub xfirst: Vec<f64>,
    pub yfirst: Vec<f64>,
    pub zfirst: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | `A1.x` — interior faces and the high-`x` boundary. |
| `y` | `Vec<f64>` | `A1.y`. |
| `z` | `Vec<f64>` | `A1.z`. |
| `xfirst` | `Vec<f64>` | `A1.xfirst` — the low-`x` boundary face only. |
| `yfirst` | `Vec<f64>` | `A1.yfirst`. |
| `zfirst` | `Vec<f64>` | `A1.zfirst`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> A1 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> A1 { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `buckling_blocks`

Collapse a block-diagonal operator to its dense per-node group blocks.

The buckling operators couple energy groups only **at the same spatial
node**, so row `idx` has non-zeros only in the `G` columns
`g2 * es + (idx % es)`. This gathers those into a `philen`-by-`G` dense
array, so `Buck.d(idx, idxvec)` becomes a row read.

The reference does the same and says why: it "replaces ~2e5 expensive
sparse-matrix slice extractions per call with cheap dense row reads (the
dominant cost in this function)". Here it matters for a second reason —
a triplet-scan lookup per access would be far worse than MATLAB's sparse
indexing.

# Arguments

- `m` — the operator, `philen` square and block-diagonal in groups.
- `philen` — `G * es`.
- `es` — nodes per group.
- `groups` — `G`.

# Returns

A `philen`-by-`groups` array; entry `(idx, g2)` is the coupling from group
`g2` into `idx`, at `idx`'s node. Structural zeros read back as `0`.

```rust
pub fn buckling_blocks(m: &mut crate::matlab::SparseMatrix, philen: usize, es: usize, groups: usize) -> crate::matlab::Array2<f64> { /* ... */ }
```

#### Function `calc_a1_expansionxyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`A1 = calc_a1_expansionxyz(params, geometry, phivec, A2, A4, Leakone, diffvalues, Buck)`.

Solves for the first expansion coefficient on each axis, by imposing current
and flux continuity across every node face.

# Two kinds of system

- **Interior faces** — a `2G`-by-`2G` solve per face, coupling the node on
  each side. The top `G` rows impose current continuity, the bottom `G`
  impose flux continuity weighted by the assembly discontinuity factors.
  Only the first `G` components of the solution are kept; they belong to the
  node on the low side.
- **Boundary faces** — a `G`-by-`G` solve per face, with a different
  right-hand side per boundary condition.

The reference batches the interior solves with `pagemldivide`; here they are
a loop over independent small systems, which is the same arithmetic.

# Arguments

- `params` — supplies `G` and the extents.
- `geometry` — per-line bounds, face boundary conditions, and `adf`.
- `coeffs` — the `Aa`/`Ff`/`Gg`/`Hh` coefficients from
  [`crate::calc_abefghxyz::calc_abefghxyz`]. **The reference reads these
  from `geometry.nodalcoeffs`**; passing them explicitly keeps
  [`crate::types`] from having to depend on a translated module. Behaviour
  is unchanged.
- `phivec` — the flux, `philen` long.
- `a2`, `a4` — the second and fourth expansion coefficients.
- `leakone` — first-moment transverse leakages from
  [`crate::calc_1sttransleakagexyz::calc_1sttransleakagexyz`].
- `diffvalues` — **flat `philen` vector**, as elsewhere in this chain.
- `buck` — the buckling operators from
  [`crate::calc_bucklingxyz::calc_bucklingxyz`].

# `Buck` is block-diagonal, and that is exploited

Energy groups couple only at the same spatial node, so `Buck.d` is
block-diagonal and `Buck.d(idx, idxvec)` — the `G` group entries at `idx`'s
node — is just one dense row. The reference pre-extracts those into
`philen`-by-`G` arrays, noting it "replaces ~2e5 expensive sparse-matrix
slice extractions per call with cheap dense row reads (the dominant cost in
this function)". This translation does the same, for the same reason: a
triplet-scan lookup per access would be far worse.

# Reference asymmetry — the high-face `zeroflux` sign

At a **high** face the two non-reflective branches differ in a way the low
face does not mirror:

```text
vacuum:   btemp = ... - adf(idx,plus)*(A2 + A4 + phivec + Aa*Leakone)
zeroflux: btemp =       -adf(idx,plus)*(A2 + A4 + phivec - Aa*Leakone)
```

The `Aa*Leakone` term flips sign between them. At the low face both the
`vacuum` and `zeroflux` branches use `- Aa*Leakone`, so the high-face
`zeroflux` line is the odd one out. Verified against the source rather than
inferred, and translated as written per the no-silent-repairs rule in
the crate README, "Translation policy". Whether it is deliberate or a slip is a
physics question this translation does not attempt to settle.

# `zeroflux` is not grouped with `vacuum` here

Every other translated module treats them identically
(`case {'vacuum','zeroflux'}`). In this file all three boundary conditions
have distinct formulas. Do not carry the grouping over from the leakage
modules.

# Singular systems

A node with `diffvalues == 0` in every group leaves its row block untouched
by the per-group loop, so only the unconditional diagonal term survives. If
that diagonal is also zero the system is singular and
[`crate::matlab::solve_dense`] returns `NaN`, mirroring MATLAB's `mldivide`
warning-and-propagate behaviour rather than aborting.

```rust
pub fn calc_a1_expansionxyz(params: &crate::types::Params, geometry: &crate::types::Geometry, coeffs: &crate::calc_abefghxyz::Coeffs, phivec: &[f64], a2: &crate::types::AxisField, a4: &crate::types::AxisField, leakone: &crate::calc_transleakagexyz::Leakage, diffvalues: &[f64], buck: &mut crate::calc_bucklingxyz::Buckling) -> A1 { /* ... */ }
```

## Module `calc_abefghxyz`

The A, B, E, F, G, H coefficients of the semi-analytic nodal update.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_ABEFGHxyz.m`, `main_exec_diff3d_standalone`
  snapshot. The Rust module is `calc_abefghxyz` because Rust warns on
  non-snake-case module names.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_abefghxyz { /* ... */ }
```

### Types

#### Struct `AxisCoeffs`

The six coefficients along one axis, one entry per `(group, node)`.

Each vector is `philen = G * maxix * maxiy * maxiz` long, ordered
`g*es + ix*maxiy*maxiz + iy*maxiz + iz`. Entries outside the core stay zero.

```rust
pub struct AxisCoeffs {
    pub aa: Vec<f64>,
    pub bb: Vec<f64>,
    pub ee: Vec<f64>,
    pub ff: Vec<f64>,
    pub gg: Vec<f64>,
    pub hh: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `aa` | `Vec<f64>` | `Aa` — coefficient of the second-order flux moment. |
| `bb` | `Vec<f64>` | `Bb` — coefficient of the third-order flux moment. |
| `ee` | `Vec<f64>` | `Ee` — surface-to-average flux ratio term. |
| `ff` | `Vec<f64>` | `Ff` — surface-current term for the even moment. |
| `gg` | `Vec<f64>` | `Gg` — odd-moment current ratio. |
| `hh` | `Vec<f64>` | `Hh` — even-moment current ratio. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AxisCoeffs { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> AxisCoeffs { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Coeffs`

`Coeffs` — the six coefficients on all three axes.

```rust
pub struct Coeffs {
    pub x: AxisCoeffs,
    pub y: AxisCoeffs,
    pub z: AxisCoeffs,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `AxisCoeffs` | Coefficients along `x`. |
| `y` | `AxisCoeffs` | Coefficients along `y`. |
| `z` | `AxisCoeffs` | Coefficients along `z`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Coeffs { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Coeffs { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `calc_abefghxyz`

`Coeffs = calc_ABEFGHxyz(params, geometry, sigma, diffvalues)`.

Computes the semi-analytic nodal coefficients for every in-core
`(group, node)` on all three axes.

# Arguments

- `params` — supplies `G` and the extents.
- `geometry` — supplies the per-node widths `lx`, `ly`, `lz`.
- `sigma` — supplies `tot` and `s`; only their **diagonals** are read, as
  the removal cross section `Sigma_r = Sigma_tot - Sigma_s`.
- `diffvalues` — diffusion coefficients from
  [`crate::calcdiffvalues3d::calcdiffvalues3d`], indexed
  `(ix, iy, iz, g)`.

# Returns

[`Coeffs`], with entries left at zero for every node outside the core.

# Which nodes are "in core"

The reference selects on `dvec ~= 0` — a node is in-core exactly when its
diffusion coefficient is non-zero. That is the same convention
`calcdiffvalues3d` establishes by leaving void nodes at zero, so the two
agree by construction. It does mean a genuine zero `D` would be read as
"outside the core", but `D = 1/(3*Sigma_tot)` cannot be zero for finite
`Sigma_tot`.

# Flattening

The reference writes `reshape(permute(diffvalues,[3 2 1 4]), philen, 1)`,
which reorders `(ix, iy, iz, g)` to `(iz, iy, ix, g)` and then reads it
column-major. That lands each element at
`g*es + ix*maxiy*maxiz + iy*maxiz + iz`, which is the ordering everything
else in the solver uses. Here the index is written out directly rather than
going through a permute.

```rust
pub fn calc_abefghxyz(params: &crate::types::Params, geometry: &crate::types::Geometry, sigma: &mut crate::types::Sigma, diffvalues: &crate::matlab::Array4<f64>) -> Coeffs { /* ... */ }
```

## Module `calc_bucklingxyz`

The buckling operators of the semi-analytic nodal update.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_bucklingxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_bucklingxyz { /* ... */ }
```

### Types

#### Struct `Buckling`

The buckling operator on each axis, `philen` square.

```rust
pub struct Buckling {
    pub x: crate::matlab::SparseMatrix,
    pub y: crate::matlab::SparseMatrix,
    pub z: crate::matlab::SparseMatrix,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `crate::matlab::SparseMatrix` | `Buck.x`. |
| `y` | `crate::matlab::SparseMatrix` | `Buck.y`. |
| `z` | `crate::matlab::SparseMatrix` | `Buck.z`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Buckling { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Buckling { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `BucklingCache`

The cached `keff`-independent part of the buckling assembly.

# Why this exists as a struct

The reference holds this in MATLAB `persistent` variables — function-scoped
state that survives between calls for the lifetime of the process. Rust has
no equivalent that is not global mutable state, so the cache is an explicit
value the caller owns and passes by `&mut`.

**The deviation, stated plainly:** MATLAB's cache is *per process and shared
by every caller*; this one is *per `BucklingCache` value*. Two solvers
running in sequence share one cache in MATLAB and would have separate ones
here unless the same value is threaded through. That cannot produce a wrong
answer — the fingerprint guards correctness either way, and a fresh cache
simply rebuilds on first use — but it does mean the *number of rebuilds* can
differ from the reference. Nothing downstream depends on that count.

Create with [`BucklingCache::new`] and keep it alive across the `keff`
iterations of a solve; that is where it pays.

```rust
pub struct BucklingCache {
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
  pub fn new() -> Self { /* ... */ }
  ```
  An empty cache, which rebuilds on its first use.

- ```rust
  pub fn is_populated(self: &Self) -> bool { /* ... */ }
  ```
  Whether the cache currently holds a built assembly.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BucklingCache { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> BucklingCache { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `calc_bucklingxyz`

`Buck = calc_bucklingxyz(params, geometry, sigma, diffvalues, keff)`.

Assembles the three buckling operators. Each entry is

$$ \left(\Sigma_{tot} - \Sigma_s - \frac{\Sigma_f}{k_{eff}}\right) \cdot \frac{L^2}{4 D} $$

evaluated at the node's width `L` along that axis and its diffusion
coefficient `D`, over the `(group, group)` block at each node.

# Arguments

- `cache` — the `keff`-independent assembly; see [`BucklingCache`].
- `params` — supplies `G` and the extents.
- `geometry` — supplies the per-node widths `lx`, `ly`, `lz`.
- `sigma` — `tot`, `s` and `f`, all `philen` square.
- `diffvalues` — **a flat `philen` vector**, not the 4-D array. See below.
- `keff` — the current eigenvalue estimate.

# `diffvalues` is flat here, unlike in `calc_ABEFGHxyz`

This is the one thing to get right at a call site. The reference indexes
`diffvalues` linearly with no `permute`, and its fingerprint uses
`sum(diffvalues)` as a scalar — both only work if the argument is already a
`philen` vector.

The sole caller, `calc_a1234_expansionxyz.m`, passes `diffvaluesD`, which is
exactly that: the 4-D array flattened to
`g*es + ix*maxiy*maxiz + iy*maxiz + iz`. A commented-out block at the top of
that file shows the loop that used to build it.

Its sibling [`crate::calc_abefghxyz::calc_abefghxyz`] takes the **4-D**
array and flattens internally. The asymmetry is the reference's.

Note also that the flattening in that commented-out block substituted
`1000000` for zero entries "to prevent division by 0 later". That
substitution is **not** applied to the vector reaching this function — the
caller keeps genuine zeros and applies the substitution separately, to a
copy named `diffvaluesDfix`, used only for a different division. So zeros
arrive here intact, and the void-skip below is what keeps them out of the
denominator.

# Which nodes are skipped

A node is skipped when `diffvalues[node] == 0`, testing the **group-1**
entry only — the index carries no group offset. A node whose first group has
a non-zero `D` but some later group has zero would pass the test and then
divide by that zero, yielding an infinite entry. `calcdiffvalues3d` fills
all groups of a node together, so the mixed case cannot arise from it.

# Returns

[`Buckling`] — three `philen`-square sparse matrices.

# Panics

If the assembled entry count exceeds `philen * G`, reproducing the
reference's `error('Error in calc_buckling')`. This is defensive in both:
the count is at most `(number of live nodes) * G * G <= philen * G`.

```rust
pub fn calc_bucklingxyz(cache: &mut BucklingCache, params: &crate::types::Params, geometry: &crate::types::Geometry, sigma: &mut crate::types::Sigma, diffvalues: &[f64], keff: f64) -> Buckling { /* ... */ }
```

## Module `calc_relpower3d`

Collapse a 3-D power-density vector to a normalised radial (x-y) map.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_relpower3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_relpower3d { /* ... */ }
```

### Functions

#### Function `calc_relpower3d`

`pwrdens_out = calc_relpower3d(params, pwrdens)`.

Sums the power density over energy groups (when the input still carries
them) and over the axial direction, then normalises the resulting `x`-`y`
map so its **mean over the fuelled nodes is 1**. This is the
relative-power map `main_exec_diff3d.m` writes to `rel_power.csv`, and the
quantity the NEACRP and IAEA-3D benchmarks report assembly-wise.

# Arguments

- `params` — supplies `G` and the three extents via `handle3dcoords`.
- `pwrdens` — power density, flattened. Either `maxix*maxiy*maxiz` long
  (already group-summed) or `G` times that. Units are whatever the solver
  produced; the normalisation makes the output dimensionless.

# Returns

A `maxix`-by-`maxiy` map, dimensionless, scaled so the mean over non-zero
entries is 1.

# The normalisation, precisely

The reference computes `nzero = nnz(pwrdensxy)` and
`nsum = sum(pwrdensxy, "all")`, then scales by `nzero / nsum`. `nzero`
counts only **non-zero** nodes while `nsum` sums **all** of them, so the
result averages to 1 over the fuelled region rather than over the full
rectangle — reflector nodes are excluded from the average but not from the
sum. That is the convention benchmark relative-power maps use.

# Indexing

The reference's 1-based `(ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz` is
converted to the 0-based `ix*maxiy*maxiz + iy*maxiz + iz`.

# Panics

If `pwrdens` is shorter than `maxix*maxiy*maxiz`, or shorter than
`G*maxix*maxiy*maxiz` when the group-collapse branch is taken.

# Division by zero

With an all-zero `pwrdens`, `nsum` is `0` and every output entry is `NaN`.
The reference does not guard this and neither does the translation.

```rust
pub fn calc_relpower3d(params: &crate::types::Params, pwrdens: &[f64]) -> crate::matlab::Array2<f64> { /* ... */ }
```

## Module `calc_sanodalxyz`

The semi-analytic nodal correction operator and its face terms.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_sanodalxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_sanodalxyz { /* ... */ }
```

### Types

#### Struct `SaNodal`

The nodal correction operator and the face terms it was built from.

```rust
pub struct SaNodal {
    pub operator: crate::matlab::SparseMatrix,
    pub terms: crate::matlab::Array2<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `operator` | `crate::matlab::SparseMatrix` | `nodal` — the correction operator, `philen` square. Assembled only;<br>nothing in the reference solves against it. |
| `terms` | `crate::matlab::Array2<f64>` | `nodalterms` — `philen` by 6, `(minus, plus)` per axis: columns `0, 1`<br>for `x`, `2, 3` for `y`, `4, 5` for `z`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SaNodal { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SaNodal { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `calc_sanodalxyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`[nodal, nodalterms] = calc_sanodalxyz(params, geometry, phivec, sigma, diffvalues, gradterms, nodaltermsold, keff)`.

Runs the full expansion, converts it into per-face nodal corrections, and
assembles those into a correction operator that sits alongside the
finite-difference `gradD`.

# Arguments

- `coeffs` — `Hh` and `Gg` here, plus the rest passed through to
  [`calc_a1234_expansionxyz`]. **The reference reads these from
  `geometry.nodalcoeffs`**; passed explicitly, as in the other expansion
  modules.
- `diffvalues` — the **4-D** `(ix, iy, iz, g)` array. This is the **third**
  consumer of that shape, alongside
  [`crate::calc_abefghxyz::calc_abefghxyz`] and
  [`crate::makegrad_dxyz::makegrad_dxyz`]; it flattens internally before
  calling the expansion.
- `gradterms` — face coefficients from
  [`crate::makegrad_dxyz::makegrad_dxyz`].
- `nodaltermsold` — the previous iteration's `nodalterms`, fed to the
  transverse-leakage chain.
- `buck_cache` — carried across calls.

# The ill-conditioning guard

The reference computes `phi_eps = 1e-8 * max(abs(phivec))` and skips any
nodal correction whose denominator is smaller than that, leaving the term at
zero. Its own comment explains why: near-zero or sign-cancelling flux makes
the expansion ill-conditioned, and the fallback is a **pure
finite-difference** correction of zero. `max(abs(phivec)) == 0` substitutes
`1`.

This is the reference's own defensive addition, not part of the underlying
method, and it is preserved as written.

# Two different interior ranges — do not conflate them

The face-term loop runs `low ..= high-1` (each **face**, owned by the node
on its low side), while the assembly loop runs `low+1 ..= high-1` (each
strictly **interior node**, since the boundary nodes get their own blocks).
The reference uses `zlow:zhi-1` in one and `zlow+1:zhi-1` in the other; that
difference is real and is preserved.

# The neighbour copy is unconditional

In the face loop:

```text
if abs(denom_z) > phi_eps
    nodalterms(idx,6)=...
end
nodalterms(idxplus,5)=nodalterms(idx,6);
```

The copy sits **outside** the guard, so when the guard suppresses the
update, the neighbour still receives whatever `nodalterms(idx,6)` already
held — zero on the first pass. Preserved.

# The void test always reads group 1

Every skip test is `diffvalues(..., 1)` — group 1 — regardless of the `g`
being processed. A node void in group 1 but not in others would be skipped
for all groups. `calcdiffvalues3d` fills all groups of a node together, so
the case does not arise from it.

# Reference defect — a fuelled node outside the bounds crashes here

The `z` pass **creates** each node's diagonal triplet slot and records it in
`counteridx`; the `y` and `x` passes then accumulate into
`nodalele(counteridx(idx))`. If `z` never created a slot, `counteridx(idx)`
is `0` and MATLAB raises `Index must be a positive integer`.

`z` skips a node when it is void **or when it falls outside
`[zlow, zhi]`** — and the latter is reachable, because
[`crate::geometry_ends3d`] finds only the first contiguous run per grid
line, so material after an internal axial gap is fuelled yet out of bounds.

This is the same root cause as the defect in
[`crate::makegrad_dxyz::makegrad_dxyz`], with the opposite symptom: there it
silently leaves a spurious `+1` on the diagonal, here it aborts. Translated
as written — the panic below carries the same meaning as MATLAB's index
error — and pinned by a test.

# Panics

If a `y` or `x` pass reaches a node the `z` pass did not create a slot for
(see above), or if the triplet count exceeds `philen*10`, reproducing
`error('Error in calc_sanodal')`.

```rust
pub fn calc_sanodalxyz(params: &crate::types::Params, geometry: &crate::types::Geometry, coeffs: &crate::calc_abefghxyz::Coeffs, phivec: &[f64], sigma: &mut crate::types::Sigma, diffvalues: &crate::matlab::Array4<f64>, gradterms: &crate::matlab::Array2<f64>, nodaltermsold: &crate::matlab::Array2<f64>, keff: f64, buck_cache: &mut crate::calc_bucklingxyz::BucklingCache) -> SaNodal { /* ... */ }
```

## Module `calc_transleakagexyz`

Transverse leakages — the base leakage operators applied to the flux.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calc_transleakagexyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calc_transleakagexyz { /* ... */ }
```

### Types

#### Struct `Leakage`

The transverse leakage on each axis, one entry per `(group, node)`.

```rust
pub struct Leakage {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | `Leakage.x`. |
| `y` | `Vec<f64>` | `Leakage.y`. |
| `z` | `Vec<f64>` | `Leakage.z`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Leakage { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Leakage { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `calc_transleakagexyz`

`Leakage = calc_transleakagexyz(params, geometry, phivec, diffvalues, gradterms, nodalterms)`.

Assembles a leakage operator on each axis and applies it to the flux,
returning the three transverse leakage vectors the nodal expansion needs.

# Arguments

- `params` — supplies `G` and the extents.
- `geometry` — node widths, the per-line active bounds from
  [`crate::geometry_ends3d::geometry_ends3d`], and the six face boundary
  conditions.
- `phivec` — the flux, `philen` long.
- `diffvalues` — **flat `philen` vector**, not the 4-D array. Same
  convention as [`crate::calc_bucklingxyz::calc_bucklingxyz`]; see that
  module for why the two shapes coexist.
- `gradterms`, `nodalterms` — `philen` rows by **6** columns. Columns pair
  up per axis as `(minus, plus)`: `0, 1` for `x`, `2, 3` for `y`, `4, 5` for
  `z`.

# Returns

[`Leakage`] — three `philen` vectors, each `L * phivec` for that axis.

# Structure

Per axis and per node, three coefficients:

- **diagonal** — `(grad_minus + grad_plus + nodal_minus - nodal_plus) / L`
- **plus neighbour** — `-(grad_plus + nodal_plus) / L_plus`
- **minus neighbour** — `-(grad_minus - nodal_minus) / L_minus`

At a boundary face the corresponding neighbour term is dropped and the
diagonal changes: under [`BoundaryCondition::Reflective`] it keeps only the
*inward* pair, while `Vacuum` and `ZeroFlux` keep the full interior form.

# Node widths are indexed by the global index, and wrap

The reference `repmat`s the per-node widths to `philen` and then indexes
them at `idx +/- stride`, which this translation reproduces as
`l[(idx +/- stride) % es]`. For an interior node the neighbour stays within
the same node column so the wrap never triggers. At a boundary face it can:
`idx + stride` for a node at the top of a group block reads the width of a
node in the *next* group block, which for uniform-in-group widths is the
same number. Faithful to the reference either way.

# At least two nodes are needed in every direction

The stencil cannot be assembled on a direction that is one node thick. Such
a node is simultaneously the low and the high face, so the high-face branch
asks for a minus neighbour that is off the end of the vector. The reference
fails on the same geometry from the other side — its low-face `idxplus`
runs past `philen` and `sparse` rejects the subscript.

This is a property of the discretisation rather than a defect, but it is
worth knowing because the failure is an index error rather than a
diagnostic.

# Reference quirk — only the `x` counter is bounds-checked

The reference preallocates `philen*5` entries per axis but tests only
`counterx`, with `error('Error in calc_transleakage')`. The `y` and `z`
counters are never checked. Reproduced: the assertion below covers `x`
alone.

# Panics

If the `x` triplet count exceeds `philen*5`, or if a boundary node's
minus-neighbour index would be negative — the latter mirrors MATLAB's
index-zero error.

```rust
pub fn calc_transleakagexyz(params: &crate::types::Params, geometry: &crate::types::Geometry, phivec: &[f64], diffvalues: &[f64], gradterms: &crate::matlab::Array2<f64>, nodalterms: &crate::matlab::Array2<f64>) -> Leakage { /* ... */ }
```

## Module `calcdiffvalues3d`

Diffusion coefficients from total cross sections.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `calcdiffvalues3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod calcdiffvalues3d { /* ... */ }
```

### Functions

#### Function `calcdiffvalues3d`

`diffvalues = calcdiffvalues3d(params, sigmatotvalues, whichsigma)` and its
`mode`-carrying form.

Fills a per-node, per-group diffusion coefficient array from the material
total cross sections:

$$ D = \frac{n}{(2n + 1)\,\Sigma_{tot}} $$

with `n` the `mode` argument. `mode = 1` gives the familiar
$D = 1/(3\Sigma_{tot})$; higher values correspond to the higher P-N closure
the reference leaves available but never calls with.

# Arguments

- `params` — supplies `G` and the extents.
- `sigmatotvalues` — total macroscopic cross section, **0-based**
  `(material_row, group)`. Units are the case file's, typically
  cm<sup>-1</sup>.
- `whichsigma` — see the material-numbering note below.
- `mode` — the P-N order. `None` selects the reference's default of `1`,
  matching its `isempty(varargin)` branch.

# Material numbering — 1-based values in a 0-based array

`whichsigma` is a 0-based **array** whose stored **values** are 1-based
material identifiers, with `0` meaning "no material". That split is
deliberate: the identifiers come straight out of the benchmark composition
CSVs, where `0` is the void marker and materials count from 1, so
renumbering them would mean rewriting the input data.

The consequence is one visible `- 1`: a node holding material `m` reads row
`m - 1` of `sigmatotvalues`.

# Returns

`(maxix, maxiy, maxiz, G)` diffusion coefficients, in the reciprocal of
`sigmatotvalues`' units (cm where the input is cm<sup>-1</sup>).

**Nodes with `whichsigma == 0` are left at zero**, not filled — the
reference `continue`s past them. Downstream code must read a zero `D` as
"absent material" rather than as a physical value.

# Panics

If a `whichsigma` entry indexes past the end of `sigmatotvalues`.

# Division by zero

A material with `sigmatotvalues == 0` yields an infinite `D`. The reference
does not guard this; the translation does not either.

```rust
pub fn calcdiffvalues3d(params: &crate::types::Params, sigmatotvalues: &crate::matlab::Array2<f64>, whichsigma: &crate::matlab::Array3<usize>, mode: Option<f64>) -> crate::matlab::Array4<f64> { /* ... */ }
```

## Module `convert_grid3d`

Build the compaction map between the full rectangular grid and the
fuelled-node-only unknown vector.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `convert_grid3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod convert_grid3d { /* ... */ }
```

### Functions

#### Function `convert_grid3d`

`[key, reversekey] = convert_grid3d(params, whichsigma)`.

The solvers assemble on the full `(G + Nc) * maxix * maxiy * maxiz`
rectangular grid but only want to *solve* on the nodes that carry material.
This builds the two lookup tables that move between the numberings:

- `key[full_index]` → `Some(compacted_index)`, or `None` if that node
  carries no material.
- `reversekey[compacted_index]` → the full-grid index it came from.

# Why `key` is `Option<usize>` and not a plain index

The reference stores `0` to mean "no material here", which is unambiguous
only because MATLAB indices start at 1. Once the translation moved to
0-based indexing, `0` became a perfectly valid compacted index and the
sentinel stopped working.

Rather than pick a different magic value, the map carries `Option<usize>`.
The two places the reference wrote `key(...) == 0` now read `is_none()`.
Same information, no ambiguity — and it is the one place in this port where
the reindexing was not a mechanical rewrite.

`reversekey` needs no such treatment: it is only read below the fuelled-node
count, and is zero-padded above it exactly as the reference leaves it.

# Arguments

- `params` — supplies `G`, `Nc` (defaulting to `0` when absent — this is
  the one site that guards the field) and the extents.
- `whichsigma` — material index per node, `0` meaning no material.

# Returns

`(key, reversekey)`, both of length `(G + Nc) * maxix * maxiy * maxiz`.
Dimensionless indices.

# Reference defect — precursor indices collide when `Nc > 1`

Inside the precursor loop the reference computes

```text
idx=(G+Nc-1)*energyindexstep+(ix-1)*xstep+(iy-1)*maxiz+iz;
```

The expression does not depend on the loop variable `nn`, so **every**
precursor family in a node maps to the same full-grid index. Each pass
overwrites the entry with a fresh counter, and the `reversekey` entries for
the earlier families point at an index whose `key` no longer refers back to
them. From the surrounding code the intent was `(G+nn-1)`.

**This is harmless at `Nc == 1`** — both expressions give the same block —
and every benchmark case in this snapshot that populates `Nc` uses a single
family, which is presumably why it went unnoticed. It corrupts the map for
any `Nc > 1`.

Translated as written, per the no-silent-repairs rule in
the crate README, "Translation policy". A fix belongs in stage 2 with
before/after numbers, not here.

```rust
pub fn convert_grid3d(params: &crate::types::Params, whichsigma: &crate::matlab::Array3<usize>) -> (Vec<Option<usize>>, Vec<usize>) { /* ... */ }
```

## Module `convertindexc2d`

Convert sparse-matrix indices between the plain and half-index (diamond
difference) numberings.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `convertindexc2d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod convertindexc2d { /* ... */ }
```

### Types

#### Enum `IndexMode`

The two index numberings the reference converts between.

The MATLAB passes these as the bare integers `1` and `2`, documented in
`convertsparseformat2d.m` as "mode 1 normal" and "mode 2 diamond difference
(half indices)".

```rust
pub enum IndexMode {
    Plain,
    DiamondDifference,
}
```

##### Variants

###### `Plain`

Mode 1 — one index per node.

###### `DiamondDifference`

Mode 2 — the `(2n+1)` grid carrying cell edges as well as centres.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> IndexMode { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &IndexMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `convertindexc2d`

`newvec = convertindexc2d(params, vec, frommode, tomode)`.

Maps a list of linear indices from one numbering to the other, routing
through mode 1 as the intermediate: the reference converts *to* mode 1
first, then *from* mode 1 to the requested output.

# Arguments

- `params` — supplies `G` and the extents.
- `vec` — **0-based** linear indices to convert.
- `frommode`, `tomode` — the source and destination numberings.

# Returns

The converted **0-based** indices, as `f64` — see below.

# This is the one place the arithmetic stays 1-based

The rest of the port converts index formulas to 0-based. This function does
not, and the exception is deliberate.

What it computes is a *mapping between two index spaces* whose definition —
the `(2n+1)` half-index grid interleaving cell centres and edges — is stated
in 1-based terms. The `+1`/`-1` offsets are not incidental there; they set
where centres fall relative to edges. Rewriting the interior in 0-based
means re-deriving that interleaving, which is error-prone for no gain.

So the boundary converts and the interior is transcribed verbatim: `+ 1.0`
on entry, the reference's formulas unchanged, `- 1.0` on exit. Callers see
0-based indices throughout, consistent with everything else.

# Why the return type is `f64`

The mode-2 → mode-1 branch computes

```text
ix=ceil(mod((vec(i)-1),energystep2)/xstep2)/2;
iy=(mod(mod((vec(i)-1),energystep2),xstep2)+1)/2;
```

Both end in a division by 2 that is **not** rounded. For an index sitting on
a cell edge rather than a centre, `ix` and `iy` come out half-integer and
the result is fractional. MATLAB carries this silently in double precision;
returning an integer type here would quietly round it and change behaviour.

The one caller, `convertsparseformat2d`, feeds the result into a sparse
assembly that rejects non-integer subscripts, so a fractional result
surfaces as an error there — exactly as in MATLAB.

# Reference defect — the two directions are not inverse

**A mode 1 → mode 2 → mode 1 round trip does not return the original
indices.** This was found by running the translation, not by reading it.

With `G = 1`, `maxi1 = maxi2 = 2` (so `energystep1 = 4`, `xstep1 = 2`,
`energystep2 = 25`, `xstep2 = 5`), the 1-based indices `1, 2, 3, 4` map
forward to `2, 14, 12, 24` and back to `2, 5, 4, 7`.

The cause is in the forward direction. It computes the row as

```text
ix = ceil(mod(t-1, energystep1) / xstep1) * 2
```

but `ceil(local / xstep1)` is the row of a *1-based* position, while `local`
is 0-based. The two agree only when `local` is not a multiple of `xstep1`:
at `local = 0` it yields row `0`, where `floor(local / xstep1) + 1` would
give row `1`. So the first node of every row lands one row low, off the
even-numbered centre positions the `(2n+1)` grid reserves for node centres.
The reverse direction, which divides by 2 instead of multiplying, does not
make the same error, so the two do not compose to the identity.

Translated as written, per the no-silent-repairs rule in
the crate README, "Translation policy". The test below pins the wrong behaviour
so that correcting it is a visible, deliberate change with before/after
numbers.

**Blast radius is unknown and worth establishing before this is relied on.**
The only caller is [`crate::convertsparseformat2d`], which is not yet
reached by any translated code path, so nothing currently depends on the
mapping being right.

# Reference quirks carried over

- **Extents are read directly**, as `params.maxi1` and `params.maxi2`, *not*
  through `handle2dcoords`. Its caller `convertsparseformat2d.m` does use
  `handle2dcoords`. A `params` carrying `maxix`/`maxiy` but no
  `maxi1`/`maxi2` therefore passes the check in the caller and fails here.
- **`philenf1` and `philenf2` are computed and never used** — dead code in
  the reference, omitted from the body but noted so a reader diffing against
  the `.m` file is not surprised.

# Panics

If `params.maxi1` or `params.maxi2` is absent, mirroring MATLAB's
`Reference to non-existent field`.

```rust
pub fn convertindexc2d(params: &crate::types::Params, vec: &[f64], frommode: IndexMode, tomode: IndexMode) -> Vec<f64> { /* ... */ }
```

## Module `convertsparseformat2d`

Re-index a whole sparse matrix between the plain and half-index numberings.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `convertsparseformat2d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod convertsparseformat2d { /* ... */ }
```

### Functions

#### Function `convertsparseformat2d`

`newmat = convertsparseformat2d(params, mat, frommode, tomode)`.

Applies [`convertindexc2d`] to a matrix's row and column indices, leaving
the values alone, and reassembles at whatever length the destination
numbering implies.

# Arguments

- `params` — supplies `G`, `Nc` and the extents.
- `mat` — the matrix to re-index.
- `frommode`, `tomode` — source and destination numberings.

# Returns

The re-indexed matrix, square at `philenf1` (mode 1) or `philenf2`
(mode 2).

# Errors

Propagates [`crate::error::BedokError::NoCoordinateBranch`] from
`handle2dcoords`.

# Values are not converted — and the reference says so

The line above the live one in the `.m` file is a commented-out version that
also passed `v` through `convertindexc2d`:

```text
%newmat=sparse(convertindexc2d(...i...),convertindexc2d(...j...),convertindexc2d(params,v,frommode,tomode),len,len);
```

Converting *values* with an *index* mapping would have been wrong, and the
author evidently caught it. The comment is preserved here because it
documents a deliberate correction rather than leftover scaffolding.

# Extent lookup differs from its callee

This function resolves extents through `handle2dcoords`, but
[`convertindexc2d`] reads `params.maxi1`/`maxi2` directly. A `params`
carrying only `maxix`/`maxiy` therefore passes the check here and then
panics inside the callee. The inconsistency is the reference's; see
[`convertindexc2d`] for the detail.

# Panics

If a converted index is fractional or negative — the 0-based equivalent of
MATLAB's `Subscripts must be either integers 1 to (2^63)-1 or logicals`
from the `sparse` call.

```rust
pub fn convertsparseformat2d(params: &crate::types::Params, mat: &mut crate::matlab::SparseMatrix, frommode: crate::convertindexc2d::IndexMode, tomode: crate::convertindexc2d::IndexMode) -> crate::error::Result<crate::matlab::SparseMatrix> { /* ... */ }
```

## Module `convertsparsekey3d`

Compact a sparse matrix from the full grid onto the fuelled-node numbering.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `convertsparsekey3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod convertsparsekey3d { /* ... */ }
```

### Functions

#### Function `convertsparsekey3d`

`newmat = convertsparsekey3d(mat, key, lennew)`.

Renumbers a matrix assembled on the full rectangular grid onto the compacted
fuelled-node numbering, using the `key` produced by
[`crate::convert_grid3d::convert_grid3d`].

# Arguments

- `mat` — assembled on the full grid.
- `key` — full-grid index → compacted index, `None` for absent nodes. See
  [`crate::convert_grid3d::convert_grid3d`] for why this is an `Option`
  rather than the reference's `0` sentinel.
- `lennew` — the compacted dimension; the result is `lennew` square.

# Returns

The compacted matrix.

# The skip rule

Entries are dropped when **all three** of these hold:

```text
key(i(k))==0 && i(k)==j(k) && v(k)==1
```

That is: a unit diagonal on a node with no material. The solvers place a
`1` on the diagonal of every absent node to keep the full-grid matrix
non-singular, and this discards exactly those placeholders. An absent node
carrying anything *else* is **not** skipped — it falls through to the
diagnostic branch below and then fails.

# Panics

If a surviving entry maps through an absent key. MATLAB reaches the same end
one step later, via `sparse`'s rejection of a zero subscript.

# The reference's diagnostic branch

After recording an entry the reference tests `key(i(k))<=0` and, if so,
prints `k`, `i(k)`, `j(k)`, `v(k)`, a decoded `(ix, iy, iz)` and the key —
with no trailing semicolons, so MATLAB echoes each to the console. Since
`key` is non-negative by construction, this fires exactly when an absent
node survived the skip rule, i.e. immediately before the assembly would
reject it. It is a "print what went wrong, then die" guard.

**The decode is hard-coded to one geometry.** It uses the literals `19` and
`17`:

```text
iz=rem(i(k)-1,19)+1
iy=rem(i(k)-iz,19*17)/19+1
ix=rem(i(k)-(iy-1)*19-iz,19*17*17)/19/17+1
```

so the `(ix, iy, iz)` it reports is meaningful only for a 17×17×19 grid and
is misleading for any other case — including the 17×17×18 grid
`main_exec_diff3d.m` currently configures. The arithmetic is reproduced on a
1-based row index (`t.i + 1`) so it yields the same numbers the reference
prints, and the output line says what it is worth.

```rust
pub fn convertsparsekey3d(mat: &mut crate::matlab::SparseMatrix, key: &[Option<usize>], lennew: usize) -> crate::matlab::SparseMatrix { /* ... */ }
```

## Module `criticalboron_xyz`

Critical-boron search for the coupled neutronics / thermal-hydraulic
steady state.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `criticalboron_xyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What it does, in three phases

Finds the boron concentration at which the **coupled** steady state is
critical. The reference's header records that this file was rewritten in
June 2026 after the obvious approach failed, and the failure is worth
knowing because the structure exists entirely to avoid it:

> The previous implementation wrapped a secant iteration around full
> **cold-started** coupled solves. The cold-start T-H Picard can go chaotic
> at off-nominal boron — `k_eff` transients into the hundreds — and either
> trips the solver's not-converging exit, returning a garbage `k_eff` that
> poisons the secant (**observed: boron diverging past 1e5 ppm**), or
> settles into a spurious coupled state.

So the rewrite never cold-starts the thermal-hydraulics away from the
starting boron:

1. **Phase 0** — one coupled steady solve at the starting boron. If the
   standard solver diverges from its cold start, a bootstrap loop recovers
   a usable coupled state using frozen-nodal eigensolves.
2. **Phase 1** — a guarded secant on **static** eigensolves at the frozen
   Phase-0 T-H state. Cheap, and it measures the boron worth slope.
3. **Phase 2** — a warm-started coupled loop: one static eigensolve per
   outer iteration, a boron correction using the measured slope, and one
   under-relaxed static T-H update. Boron, flux and feedback converge
   together.

# Why there are two different eigensolvers

This is the subtlest part of the file, and the reference documents it at
length because both halves were established by experiment.

`eigsolve_boron` delegates to [`crate::sanodaldiffusion_solverxyz`] — the
same eigensolver the steady and transient drivers use, so the reported
`k_eff` stays consistent across the whole search. That is safe **only**
because the flux is warm-started from a good shape: the solver's continuous
nodal updates then act on a good flux at every update and stay stable.

`eigsolve_cold` exists because from a **flat** cold flux they do not. The
reference records two verified findings:

- `sanodaldiffusion`'s continuous nodal updates use the still-bad
  mid-iteration flux on a cold start and **diverge to `k_eff` around 5e4**
  on the heavily-rodded configuration. Freezing them via a huge `nodalupd`
  does stabilise the cold solve in isolation — but
- `sanodaldiffusion` builds its *initial* nodal correction from a **flat**
  flux (hardcoded ones), so a frozen call returns a roughly **25 pcm-biased,
  flatter seed**, and that poorer seed then destabilised a near-critical
  Phase-1 warm solve (**`k_eff` to 377**).

`eigsolve_cold` therefore builds the nodal correction from the **warm**
flux, freezes it, and power-iterates — stable cold *and* an accurate seed,
which the production solver cannot be made to do through its parameters.

# One deliberate departure from the reference

**The `.mat` steady-state cache is not translated.** As in
[`crate::thdiffusion_solvertimexyz`], `params.steadyfile` becomes an
explicit `initial_steady` argument so the caller owns cache invalidation.
The reference additionally *validates* its cache — discarding it with a
warning if the stored `k_eff` is outside `[0.8, 1.2]` — which an explicit
argument makes unnecessary.

# What this can be checked against

[`crate::neacrpa2t`] records the only published NEACRP number in the
snapshot: case A2's critical boron is **1160.6 ppm** (PANTHER,
NEA/NSC/DOC(93)25 Table 3.1), against the **1139.01 ppm** the reference
computes for itself. This module is what would produce that second number —
but the search that originally produced it, `test_critboron3.m`, is **not in
the snapshot**, so its settings are unknown. See
`docs/bedok-reference-defects.md`, "Missing files".

# OPEN DISCREPANCY — read before using this module's answer

Run on [`crate::neacrpa2`], this port converges to **1253.29 ppm**
(`k_eff` = 1.000001), against the reference's 1139.01 ppm. At the measured
boron worth of -9.62 pcm/ppm that gap is about **1100 pcm** — far beyond
round-off. **This port computes a materially more reactive core than the
MATLAB does.**

**The cause is now narrowed.** Running the identical search on
[`crate::neacrpa1t`] — the same core at hot zero power — reproduces *that*
case's reference value to **0.03%** (551.14 against 551.31 ppm, under 2 pcm),
and on that run Phase 0 did **not** need the bootstrap. A1 and A2 share the
cross sections, the feedback chain, the eigensolver and this whole search, so
a mistranslation in any of them is very unlikely.

What differs is the Phase-0 path: **A2 fell back to the bootstrap and A1 did
not.** The open question is therefore either that the bootstrap converges a
different coupled state, or that [`crate::thdiffusion_solverxyz`] fails on A2
where the MATLAB succeeds — in which case the bootstrap is merely exposing a
defect in the coupled driver.

Measured 2026-08-18; the full breakdown is in that module test. **Until this
is settled, do not trust this module's answer on a case that reports
`bootstrapped == true`.**

```rust
pub mod criticalboron_xyz { /* ... */ }
```

### Modules

## Module `defaults`

The reference's defaults for the search.

```rust
pub mod defaults { /* ... */ }
```

### Constants and Statics

#### Constant `CRIT_TOL`

`crittol` — tolerance on `|k_eff - 1|` for the critical state.

```rust
pub const CRIT_TOL: f64 = 1e-5;
```

#### Constant `FUELTEMP_TOL`

`fueltemptol` — fuel-temperature convergence tolerance, K.

```rust
pub const FUELTEMP_TOL: f64 = 0.5;
```

#### Constant `RELAX`

`threlax` — T-H Picard under-relaxation factor.

```rust
pub const RELAX: f64 = 0.5;
```

#### Constant `SLOPE_SEED`

`slopedefault` — a typical PWR boron worth, `dk/db` per ppm, used to
seed the secant before any slope has been measured.

```rust
pub const SLOPE_SEED: f64 = -9e-5;
```

#### Constant `MAX_OUTER`

`maxout` — Phase-2 outer iterations.

```rust
pub const MAX_OUTER: usize = 40;
```

#### Constant `MAX_SECANT`

The Phase-1 secant iteration cap.

```rust
pub const MAX_SECANT: usize = 12;
```

#### Constant `MAX_BOOTSTRAP`

The Phase-0 bootstrap iteration cap.

```rust
pub const MAX_BOOTSTRAP: usize = 30;
```

#### Constant `COLD_POWER_ITER`

Power iterations inside `eigsolve_cold`.

```rust
pub const COLD_POWER_ITER: usize = 8000;
```

#### Constant `COLD_NODAL_REFINE`

Nodal refinements `eigsolve_cold` applies before freezing.

```rust
pub const COLD_NODAL_REFINE: usize = 3;
```

#### Constant `SEARCH_INNER_TOL`

The tight inner tolerance the search eigensolves use, for a
sub-ppm-accurate critical `k_eff`.

```rust
pub const SEARCH_INNER_TOL: f64 = 1e-8;
```

#### Constant `SECANT_TOL`

The Phase-1 secant's own convergence test on `|k_eff - 1|`.

Looser than [`CRIT_TOL`] because Phase 2 refines it afterwards.

```rust
pub const SECANT_TOL: f64 = 2e-6;
```

### Types

#### Struct `BoronOutput`

What the search returns.

```rust
pub struct BoronOutput {
    pub boron: f64,
    pub k_eff: f64,
    pub boron_history: Vec<f64>,
    pub k_eff_history: Vec<f64>,
    pub slope_pcm_per_ppm: f64,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub th: crate::types::Th,
    pub secant_iterations: usize,
    pub coupled_iterations: usize,
    pub converged: bool,
    pub bootstrapped: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `boron` | `f64` | `output.boron` — the critical concentration, ppm. |
| `k_eff` | `f64` | `output.k_eff` at that concentration. |
| `boron_history` | `Vec<f64>` | `output.boronhist` — every concentration tried, in order. |
| `k_eff_history` | `Vec<f64>` | `output.keffhist` — the matching eigenvalues. |
| `slope_pcm_per_ppm` | `f64` | `output.slope_pcm_per_ppm` — the measured boron worth.<br><br>Negative: boron is an absorber, so more of it lowers `k_eff`. |
| `scalar_flux` | `Vec<f64>` | `output.scalar_flux` at the critical state. |
| `fission_source` | `Vec<f64>` | `output.fission_source` — `sigma.f * phi`. |
| `pwrdens` | `Vec<f64>` | `output.pwrdens` — `fission_source .* Vi`. |
| `th` | `crate::types::Th` | `output.th` — the coupled state at the critical boron. |
| `secant_iterations` | `usize` | How many Phase-1 secant iterations ran. |
| `coupled_iterations` | `usize` | How many Phase-2 coupled iterations ran. |
| `converged` | `bool` | Whether both criteria — `|k_eff - 1|` and the fuel temperature — were<br>met. The reference prints `[converged]` / `[NOT converged]` for this. |
| `bootstrapped` | `bool` | Whether Phase 0 had to fall back to the bootstrap loop.<br><br>Not in the reference's output, which only warns. A caller otherwise has<br>no way to know the standard solver failed. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoronOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `criticalboron_xyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`output = criticalboron_xyz(geometry, params, th, sigmavalues, whichsigma, varargin)`.

# Arguments

- `initial_steady` — a precomputed Phase-0 coupled state. `None` runs
  Phase 0. Replaces the reference's `params.steadyfile` `.mat` cache.
- `initial_k_eff` — `varargin{1}`; the reference defaults it to 1.

# Errors

[`BedokError::BoronSearchDiverged`] if any eigensolve leaves the sane range,
plus whatever the operator chain and the coupled solver raise.

```rust
pub fn criticalboron_xyz(geometry: &crate::types::Geometry, params: &crate::types::Params, th: &crate::types::Th, sigmavaluesref: &crate::types::SigmaValues, feedback: &crate::sigmavalupd3d_handler::FeedbackTables, whichsigmaref: &crate::matlab::Array3<usize>, initial_steady: Option<&crate::thdiffusion_solverxyz::CoupledOutput>, initial_k_eff: Option<f64>) -> crate::error::Result<BoronOutput> { /* ... */ }
```

## Module `diffusion_solverxyz`

Finite-difference multigroup diffusion — the plain power iteration.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `diffusion_solverxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this is, and what it is not

This is the **reference solver without the nodal correction**: a mesh-centred
finite-difference discretisation solved by source iteration. Its companion,
[`crate::sanodaldiffusion_solverxyz`], adds the semi-analytic nodal (SANM)
correction operator on top of the same `gradD` and is the one the benchmark
drivers actually call. This one is the baseline the nodal answer is judged
against — `docs/bedok-reference-defects.md` N1 quotes a "-103 pcm of finite
difference" comparison, and this is the finite difference it means.

The two are deliberately **not** factored into a shared solver here. They
differ in the operator split, in three separate normalisation choices, in
their acceleration, and in their iteration caps; merging them would hide
exactly the inconsistencies the defect register is trying to keep visible.
One module per `.m` file, as everywhere else in this crate.

```rust
pub mod diffusion_solverxyz { /* ... */ }
```

### Types

#### Enum `Termination`

Why the source iteration stopped.

The reference distinguishes these only by which `break` fired, and reports
nothing about it; returning the reason lets a caller tell a converged answer
from a bailed-out one, which the reference's own output cannot do. Defect C7
records that silent non-convergence as a problem in the coupling layer, so
not reproducing the silence is worth the small addition.

```rust
pub enum Termination {
    Converged,
    NonPositiveKeff,
    NanKeff,
    IterationCap,
}
```

##### Variants

###### `Converged`

Both residuals fell below [`TOL`] — the `while` condition went false.

###### `NonPositiveKeff`

`k_eff <= 0`, i.e. the eigenvalue update produced a non-physical value.

###### `NanKeff`

`k_eff` became `NaN` — in practice a singular or diverging solve.

###### `IterationCap`

The iteration count passed [`MAX_ITER`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Termination { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Diagnostics`

Diagnostic asymmetry maps, the quantities the reference writes to CSV.

# Why these are returned rather than written

`diffusion_solverxyz.m` calls `writematrix` **unconditionally** — three
symmetry maps before the iteration and `rel_power_inner.csv` after it — so
every single call scribbles four files into the working directory. (Its
nodal counterpart puts the equivalent dumps behind `params.debugdump`; this
one does not, which is defect D3.)

A library that writes files as a side effect of being called is not
something this translation is willing to reproduce: it would make the solver
unusable from two threads, untestable without a temp directory, and
surprising to every caller. The quantities are computed exactly as the
reference computes them and handed back instead, so a caller that wants the
files can write them and the physics is unchanged either way.

Each map is `maxix` by `maxiy` and dimensionless.

```rust
pub struct Diagnostics {
    pub sigmaf_asymmetry: crate::matlab::Array2<f64>,
    pub sigmas_asymmetry: crate::matlab::Array2<f64>,
    pub sigmatot_asymmetry: crate::matlab::Array2<f64>,
    pub rel_power: crate::matlab::Array2<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sigmaf_asymmetry` | `crate::matlab::Array2<f64>` | `sigmafxy - sigmafxy.'` — the antisymmetric part of the collapsed<br>fission diagonal. Written as `sigmafxy.csv`. |
| `sigmas_asymmetry` | `crate::matlab::Array2<f64>` | `sigmasxy - sigmasxy.'`, from the scattering diagonal. Written as<br>`sigmasxy.csv`. |
| `sigmatot_asymmetry` | `crate::matlab::Array2<f64>` | `sigmatxy - sigmatxy.'`, from the total-cross-section diagonal. Written<br>as `sigmatxy.csv`. |
| `rel_power` | `crate::matlab::Array2<f64>` | `rel_power` — the normalised assembly power map. Written as<br>`rel_power_inner.csv`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Diagnostics { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Diagnostics { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DiffusionOutput`

`output` — what the reference returns, plus the provenance it does not.

Deliberately **not** `Default`: there is no honest default for
[`DiffusionOutput::termination`], and a zero-valued `k_eff` is not a
meaningful starting point for anything.

```rust
pub struct DiffusionOutput {
    pub k_eff: f64,
    pub residual: f64,
    pub k_eff_residual: f64,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub phi_plot: crate::matlab::Array2<f64>,
    pub iterations: usize,
    pub termination: Termination,
    pub diagnostics: Diagnostics,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | `output.k_eff` — the multiplication factor, dimensionless. |
| `residual` | `f64` | `output.residual` — the relative fission-source change, dimensionless. |
| `k_eff_residual` | `f64` | `output.k_eff_residual` — the relative `k_eff` change, dimensionless. |
| `scalar_flux` | `Vec<f64>` | `output.scalar_flux` — the converged flux, `philenf` long, normalised so<br>its fission-source 1-norm equals the flat guess's. |
| `fission_source` | `Vec<f64>` | `output.fission_source` — `sigma.f * scalar_flux`, same length and<br>normalisation. |
| `pwrdens` | `Vec<f64>` | `output.pwrdens` — `fission_source .* Vi`, the power density per node. |
| `phi_plot` | `crate::matlab::Array2<f64>` | `phi_plot` — the flux summed over groups on the `zplot = 1` axial plane,<br>`maxix` by `maxiy`.<br><br>The reference computes this whether or not `params.plotfig` is set, and<br>then only uses it to draw `figure(6)`. Returned rather than plotted,<br>since a library cannot open a figure window; `params.plotfig` is<br>consequently not read here at all. |
| `iterations` | `usize` | The source-iteration count the reference prints as<br>`Diffusion iteration`. This is `iteration - 1`. |
| `termination` | `Termination` | Why the iteration stopped. Not in the reference's `output`. |
| `diagnostics` | `Diagnostics` | The unconditional CSV dumps, returned instead of written. See<br>[`Diagnostics`]. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DiffusionOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `diffusion_solverxyz`

`output = diffusion_solverxyz(geometry, params, sigmavalues, whichsigma, initial_k_eff)`.

Assembles the finite-difference diffusion operator and runs a source
iteration on it to convergence, returning the fundamental-mode flux and
eigenvalue.

# Arguments

- `geometry` — needs `Vi`, the per-node volumes, plus everything
  [`makegrad_dxyz`] reads (the `[low, high]` bounds, the node widths and the
  six boundary conditions).
- `params` — `G`, `Nc` and the three extents.
- `sigmavalues` — per-material cross sections.
- `whichsigma` — the 1-based material map, `0` for void.
- `initial_k_eff` — `varargin{1}`; `None` is the reference's default of `1`.

# The operator split, and why the scattering term appears twice

The reference builds

```text
LHS = gradD + sigma.tot - sigma.sd
RHS = fission_source/k_eff + (sigma.s - sigma.sd)*scalar_flux
```

`sigma.sd` is the **within-group** scattering diagonal and `sigma.s` is the
full scattering operator, so the two lines together are
`(gradD + sigma.tot - sigma.s) phi = fission_source / k_eff` with the
within-group part treated implicitly and the group-to-group part lagged one
iteration. That is an ordinary source iteration over energy, and it is why
this solver's `LHS` differs from the nodal solver's — that one puts the
whole of `sigma.s` on the left and carries no scattering source at all.

# Normalisation

Every fission-source integral here is a **1-norm**, `norm(x, 1)`. The flux
and source are rescaled each pass so the source integral holds at whatever
the flat initial guess produced. The comment in the reference says "fission
source integration = 1", which is not what the code does — see defect N10,
which raises the same point against the nodal solver.

Note `k_eff` is updated from the **un-rescaled** `fission_source_new`, before
the rescale; since the update is a ratio of successive integrals and both
are rescaled by the same factor, that choice does not change the result.

# The empty-grid compaction is dead code

Lines 60-76 and 160-174 of the reference compact the operators onto the
occupied nodes with [`crate::convert_grid3d`] and
[`crate::convertsparsekey3d`], and expand the answer back afterwards. Both
blocks are guarded by `keychange == 1` where `keychange` is the literal `0`
assigned four lines earlier, so **neither ever runs**. It is not translated:
there is nothing to reproduce, and writing an untested compaction path would
be inventing behaviour rather than porting it. The two functions it would
call are translated and tested in their own right. Recorded as defect D1.

# On a non-convergence break, the reported state lags by one iteration

The `break` fires before `iteration` is incremented, so `k_eff(iteration)`,
`residual(iteration)` and `k_eff_residual(iteration)` in the output are the
**previous** pass's values — the offending `k_eff(iteration+1)` that
triggered the break is computed, tested and then discarded. Preserved;
[`DiffusionOutput::termination`] is how a caller can tell this happened.
Recorded as defect D2.

# `Nc > 0` does not work

`Vi` is replicated to `G` groups, giving `G*es` entries, while the fission
source is `philenf = (G+Nc)*es` long. MATLAB's `.*` errors on the mismatch.
This is the same conformance gap as defects C11 and N2; all four benchmark
cases set `Nc = 0`. Reproduced as a panic.

# Errors

- [`BedokError::IterativeSolveNotTranslated`] if `philenf >= 50_000_000`.
- Whatever [`makegrad_dxyz`] raises.

# Panics

If `geometry.vi` is shorter than `maxix*maxiy*maxiz`, or if `Nc > 0` (see
above).

```rust
pub fn diffusion_solverxyz(geometry: &crate::types::Geometry, params: &crate::types::Params, sigmavalues: &crate::types::SigmaValues, whichsigma: &crate::matlab::Array3<usize>, initial_k_eff: Option<f64>) -> crate::Result<DiffusionOutput> { /* ... */ }
```

### Constants and Statics

#### Constant `SIZE_THRESH`

`sizethresh` — above this many unknowns the reference switches to
preconditioned GMRES. See [`BedokError::IterativeSolveNotTranslated`] for
why that branch is not translated.

```rust
pub const SIZE_THRESH: usize = 50_000_000;
```

#### Constant `TOL`

`diffusion.tol` — the convergence tolerance on both residuals.

Unlike [`crate::sanodaldiffusion_solverxyz`], this solver has no
`params.innertol` override; it is always tight.

```rust
pub const TOL: f64 = 1e-6;
```

#### Constant `MAX_ITER`

`maxiter` — the source-iteration cap.

Note this is **10000** where the nodal solver uses 5000.

```rust
pub const MAX_ITER: usize = 10_000;
```

## Module `fiss_src_extrapolatexyz`

Fission-source extrapolation, to accelerate the outer power iteration.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `fiss_src_extrapolatexyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# Method reference

The reference cites p. 51 of B. R. Bandini, *A three-dimensional transient
neutronics routine for the TRAC-PF1 reactor thermal hydraulic computer
code*, PhD thesis, Pennsylvania State University, 1990. The citation is
carried over as provenance for the method; the thesis itself is not in this
repository.

```rust
pub mod fiss_src_extrapolatexyz { /* ... */ }
```

### Types

#### Enum `Extrapolation`

Whether the extrapolation was applied, and if not, why.

The reference signals this only through `verbose` printing; returning it
lets a caller log or count outcomes without parsing stdout. The
`verbose = 0` default means the reference prints nothing at all, so no
behaviour is lost by not printing here.

```rust
pub enum Extrapolation {
    Applied,
    ZeroNorm,
    NotAsymptotic,
}
```

##### Variants

###### `Applied`

Applied, with the weight used.

###### `ZeroNorm`

Skipped: a zero norm in the dominance-ratio denominator. The reference's
comment attributes this to a stagnant fission source, typically in early
iterations.

###### `NotAsymptotic`

Skipped: the two dominance-ratio estimates disagree by more than 10%,
so the iteration is not yet in its asymptotic regime.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Extrapolation { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Extrapolation) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `fiss_src_extrapolatexyz`

`[phirecord, fs] = fiss_src_extrapolatexyz(sigmaf, phirecord)`.

Estimates the dominance ratio from three successive fission-source
differences and, if the iteration looks asymptotic, extrapolates both the
fission source and the current flux along that direction.

# Arguments

- `sigmaf` — the fission production operator, `philen` square.
- `phirecord` — flux history, `philen` rows by **4** columns: current,
  previous, the one before, and the one before that. Column 0 is
  overwritten when the extrapolation is applied.

# Returns

`(fs, outcome)` — the fission source (extrapolated if it was applied), and
which branch was taken.

# The guards, and why they are there

Three of the four safety checks carry explanatory comments in the reference,
which is unusual for this codebase and suggests they were added in response
to real failures:

- **Zero norms.** A stagnant fission source makes the dominance-ratio
  denominator zero. Returns early with `fs` already computed.
- **Clamping the ratio to `[0, 0.99]`.** A ratio at or above 1 means the
  iteration is not converging asymptotically; the weight `w = r/(1-r)` would
  be negative or infinite and would overshoot catastrophically.
- **Capping the weight at 5.** Guards against a ratio that survives the
  clamp but sits close to 1.
- **The 10% agreement test.** Uses an absolute difference rather than a
  relative one, to avoid dividing by a near-zero weight, with a `1e-14`
  floor.

# `fs` is computed before the guards

The early return still hands back a valid, un-extrapolated `fs`, because the
reference assigns `fs = sigmaf*phi` before testing anything. A caller can
use the result unconditionally.

```rust
pub fn fiss_src_extrapolatexyz(sigmaf: &mut crate::matlab::SparseMatrix, phirecord: &mut crate::matlab::Array2<f64>) -> (Vec<f64>, Extrapolation) { /* ... */ }
```

## Module `fuelrodheat_1dcylnd`

Steady 1-D cylindrical fuel-rod conduction — the live path.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `fuelrodheat_1dcylnd.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this computes, and why it matters

The radial temperature profile through one fuel rod at one axial node:
pellet centre out through the fuel, across the fuel-cladding gap, through
the cladding, and into the coolant through a convective boundary condition.
`th_solverxyz.m` calls it once per fuelled node, and its output drives the
**Doppler feedback** — so an error here moves reactivity, not just a
reported temperature.

The whole integrated heat equation is divided through by `2*pi`, as the
reference's own header line states. That is why the source terms are
`0.5*q*(r_out^2 - r_in^2)` rather than `pi*q*(...)`.

# The interface-node doubling — read this before the code

The radial mesh has `maxir` nodes, but the linear system has
**`maxid = maxir + surfcount`** unknowns, where `surfcount` counts
solid/void transitions. The extra unknowns are *surface* temperatures at
material interfaces, where the profile has a kink the cell-centred nodes
cannot represent.

The loop therefore walks two counters: `ir` over the radial mesh and `id`
over the unknowns. A `surf` flag makes it visit one `ir` **twice**, emitting
two unknowns. Worked through for the NEACRP mesh — 5 fuel, 1 gap, 2 clad,
so `whichk = [1,1,1,1,1,0,2,2]`, `maxir = 8`, `surfcount = 2`,
`maxid = 10`:

| `ir` | `id` | what it is |
|---|---|---|
| 1-4 | 1-4 | fuel interior |
| 5 | 5 | last fuel node |
| 5 (again) | 6 | **fuel outer surface** |
| 6 | 7 | the gap — a dummy row, see below |
| 7 | 8 | cladding |
| 8 | 9 | last cladding node |
| — | 10 | **cladding outer surface**, carrying the coolant BC |

Conduction across the gap links `id = 6` directly to `id = 8`, skipping the
dummy. Note the count works out for a reason that is not the obvious one:
`surfcount = 2` counts the fuel/gap *and* gap/clad transitions, but only the
first produces a doubled node — the second extra unknown is the outer
surface, created by the `ir == maxir` branch. The arithmetic is right for
this configuration; it is not obviously right for every configuration.

```rust
pub mod fuelrodheat_1dcylnd { /* ... */ }
```

### Types

#### Enum `Solve`

The conduction solve's outcome, alongside the profile.

The reference returns only the temperature vector; when it contains `NaN` it
*displays* `laplc`, `bvec` and `pwr` and returns anyway. Callers cannot
distinguish that from a good solve, and `th_solverxyz.m` has a whole
`any(isnan(...))` recovery block downstream to cope. Returning the fact
explicitly is cheaper than reproducing the print.

```rust
pub enum Solve {
    Ok,
    NotFinite,
}
```

##### Variants

###### `Ok`

Every temperature is finite.

###### `NotFinite`

The solve produced at least one `NaN` — a singular or near-singular
operator. The reference dumps diagnostics to the console here.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Solve { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Solve) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `fuelrodheat_1dcylnd`

`results = fuelrodheat_1dcylnd(params, geometry, temps, pwr, bc, modtemp)`.

# Arguments

- `fuel` — needs `whichk`, `tcon`, `gap_conductance`, `lr` and `ctr`.
- `maxir` — radial node count, the reference's `params.fuel.maxir`.
- `temps` — the **previous** temperature profile, `maxid` long, in **K**.
  Used only to evaluate the temperature-dependent conductivities, which is
  what makes the whole solve a Picard iteration when the caller feeds its
  own output back. Note it is indexed by `id`, not `ir`.
- `pwr` — volumetric power density in the pellet, **W/cm³**.
- `bc` — outer boundary conductance, **W/(cm·K)**; `hcoeff * Rtot` in the
  live path.
- `modtemp` — coolant (moderator) temperature, **K**, the sink the boundary
  condition drives towards.

# Returns

`(temperatures, outcome)` — the profile over the `maxid` unknowns in **K**,
and whether it came back finite.

# Reference defects carried here

- **The gap row is a dummy that reads as `T = 1`.** A node with
  `whichk == 0` gets `bvec(id) = 1` and keeps the preallocated diagonal
  `1`, so its temperature solves to exactly `1` — in K, a physically absurd
  value. Conduction bypasses it, so it does not corrupt the profile, and
  `th_solverxyz.m` clamps it up to the coolant temperature immediately
  after. But it is in the returned vector, and any caller that averages over
  the profile without knowing this gets a wrong answer. Recorded as T7.
- **A missing branch leaves a stale conductance (T8).** In the `surf == 1`
  pass the body is guarded by `if whichk(ir+1) == 0` with **no `else`**. If
  two *different, both solid* materials are adjacent — fuel directly against
  cladding, no gap — that pass emits no forward link at all and
  `laplcele(id) = kminus + kplus` uses the previous pass's `kplus`,
  producing a row with an inflated diagonal and a missing off-diagonal. The
  row no longer balances, so the operator silently stops conserving energy.
  Unreachable for the benchmark meshes, which always put a gap between fuel
  and cladding.
- **`temps` is read at `id + 1` before that unknown exists.** Every
  conductivity pair reads `temps(id)` and `temps(id+1)`, including at
  `id = maxid - 1`. The vector must therefore be `maxid` long even though
  the mesh has `maxir` nodes — a caller sizing it from `maxir` reads past
  the end. Asserted here.
- **The `ir == maxir` branch uses one material for both sides.** It reads
  `tcon{whichk(ir)}` for `cond` *and* `condplus`, where every other branch
  reads `whichk(ir+1)` for the second. Deliberate — there is no `ir+1` — but
  it means the outer surface conductance is a self-harmonic-mean, i.e. just
  `cond`, evaluated at two different temperatures.
- **A doubled interface conductance.** The `whichk(ir+1) == 0` branch
  multiplies its harmonic mean by an extra `2`, with the un-doubled line
  commented out directly above. No derivation is given.

# Panics

If `temps` is shorter than the `maxid` the mesh implies, if a `whichk`
value has no matching conductivity, or if the mesh has fewer than two nodes.

```rust
pub fn fuelrodheat_1dcylnd(fuel: &crate::types::FuelGeometry, maxir: usize, temps: &[f64], pwr: f64, bc: f64, modtemp: f64) -> (Vec<f64>, Solve) { /* ... */ }
```

## Module `fuelrodheattime_1dcylnd`

Transient 1-D cylindrical fuel-rod conduction — one implicit-Euler step.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `fuelrodheattime_1dcylnd.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this adds to the steady version

One implicit-Euler step of

```text
rho cp dT/dt = (1/r) d/dr ( k r dT/dr ) + q'''
```

The discretisation, the interface-node doubling, the gap bridge and the
boundary treatment are **identical** to [`crate::fuelrodheat_1dcylnd`] —
read that module's docs for how the `ir`/`id` walk works, because none of it
is repeated here. The only additions are a capacity term on each diagonal
and its matching source contribution:

```text
cap_id = rho_cp(T_old_id) * (r_cur^2 - r_prev^2) / 2 / dt
```

with `cap_id * T_old_id` added to the right-hand side. `[r_prev, r_cur]` is
the radial interval that solution node represents.

# The steady solver is this one at `dt = infinity`

Setting `cap = 0` recovers `fuelrodheat_1dcylnd` **exactly** — same
diagonal, same source, same off-diagonals. That is not a loose analogy; it
is checked by a test here, and it is worth knowing because the two files
were transcribed independently. A disagreement between them would mean one
of the two transcriptions is wrong, and the test says which.

They are nevertheless kept as separate modules rather than one
parameterised solver: the reference ships two files, the capacity terms are
interleaved through the loop rather than separable the way
`singleflow1devap`'s stage 2 was, and collapsing them would mean editing
already-verified code to accommodate the new one.

# Semi-implicit, not fully implicit

Thermal properties are evaluated at the **previous** temperatures:
conductivity at `temps` (the current Picard iterate) and heat capacity at
`tempsold` (the previous time step). So the matrix is linear in the unknown
temperature and the non-linearity is lagged. A caller wanting the fully
implicit answer iterates, feeding each result back in as `temps`.

```rust
pub mod fuelrodheattime_1dcylnd { /* ... */ }
```

### Functions

#### Function `fuelrodheattime_1dcylnd`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`results = fuelrodheattime_1dcylnd(params, geometry, temps, tempsold, pwr, bc, modtemp, dt)`.

# Arguments

As [`crate::fuelrodheat_1dcylnd::fuelrodheat_1dcylnd`], plus:

- `tempsold` — temperatures at the **previous time step**, `maxid` long, in
  **K**. Used for the capacity terms, both to evaluate `rho*cp` and as the
  source `cap * T_old`.
- `dt` — the time step, **seconds**.

and `fuel` additionally needs [`crate::types::FuelGeometry::rhocp`].

Note `temps` and `tempsold` are different vectors and mean different things:
`temps` is the current Picard iterate, used only for the conductivities;
`tempsold` is the previous time level, and it carries the physics of the
time derivative.

# Returns

`(temperatures, outcome)` — the profile over the `maxid` unknowns in **K**,
and whether it came back finite.

# The radial intervals, which are not the obvious ones

Each solution node owns a radial interval `[r_prev, r_cur]`, and the
bookkeeping is worth spelling out because it is not simply "the node's own
cell":

| Node | Interval |
|---|---|
| innermost | `[0, Ctr(1)]` — only the **inner half** of the first mesh cell |
| a gap node | none; `r_prev` jumps to `sumLr(ir)` and no capacity is added |
| a surface node (`surf`) | `[Ctr(ir), sumLr(ir)]` — the **outer half** of that cell |
| an ordinary node | `[r_prev, Ctr(ir)]` |
| outermost | `[r_prev, sumLr(maxir)]` |

So the capacity is distributed over half-cells at the interfaces, which is
consistent with the interface doubling: the two unknowns that share a mesh
cell split its mass between them.

# Reference defects carried here

The same set as [`crate::fuelrodheat_1dcylnd`], since the loop is the same:
the gap dummy row pinned at `T = 1` (T7), the missing `else` that leaves a
stale conductance when two different solid materials touch (T8), `temps`
being read at `id + 1`, the self-harmonic-mean at `ir == maxir`, and the
doubled interface conductance. See that module for each.

One is specific to this file: **the gap dummy row is `1*T = 1` here too**,
and because it has no capacity term it stays exactly 1 regardless of `dt`,
`tempsold` or anything else.

# Panics

If `temps` or `tempsold` is shorter than the `maxid` the mesh implies, if a
`whichk` value has no matching conductivity or heat capacity, or if the mesh
has fewer than two nodes.

```rust
pub fn fuelrodheattime_1dcylnd(fuel: &crate::types::FuelGeometry, maxir: usize, temps: &[f64], tempsold: &[f64], pwr: f64, bc: f64, modtemp: f64, dt: f64) -> (Vec<f64>, Solve) { /* ... */ }
```

### Re-exports

#### Re-export `Solve`

```rust
pub use crate::fuelrodheat_1dcylnd::Solve;
```

## Module `fixinfnan`

Replace non-finite entries of a vector.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `fixinfnan.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod fixinfnan { /* ... */ }
```

### Functions

#### Function `fixinfnan`

`newvector = fixinfnan(vector)` and `fixinfnan(vector, anything)`.

Replaces every `Inf`, `-Inf` and `NaN` entry with either `0` (the default)
or `min(abs(vector))` (the special mode the reference selects by passing any
extra argument at all — the value is never inspected, only its presence).

The MATLAB `varargin` test becomes the `use_min_abs` flag: `false` is
`fixinfnan(v)`, `true` is `fixinfnan(v, _)`.

# Arguments

- `vector` — values to clean, in whatever units the caller works in; this
  function is unit-agnostic.
- `use_min_abs` — `false` substitutes `0`, `true` substitutes the smallest
  finite magnitude.

# Substitution value in the special mode

The reference evaluates `min(abs(vector))` on the **original** vector, so
the substitute is computed before any replacement happens. MATLAB's `min`
skips `NaN`, and `abs(Inf)` is `Inf`, so in practice the result is the
smallest finite magnitude — which is what
[`crate::matlab::min_abs_finite`] returns.

The one case where the two differ is a vector with **no** finite entry at
all: MATLAB would yield `Inf` (or `NaN` for an all-`NaN` input) and
propagate it, whereas `min_abs_finite` returns `None`. This translation
substitutes `0` there. That case cannot arise from the reference's own call
sites, and the divergence is recorded rather than hidden.

```rust
pub fn fixinfnan(vector: &[f64], use_min_abs: bool) -> Vec<f64> { /* ... */ }
```

## Module `fixnegativematrix`

Zero the negative entries of a sparse matrix.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `fixnegativematrix.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod fixnegativematrix { /* ... */ }
```

### Functions

#### Function `fixnegativematrix_dense`

`mat = fixnegativematrix(mat)`.

Walks the structural non-zeros and sets every negative one to zero, leaving
positives untouched. The reference uses this to clamp a coefficient matrix
that has picked up negative entries the downstream solve cannot tolerate.

# Arguments

- `mat` — a sparse matrix, modified in place. Unit-agnostic.

# Note on the reference's variable naming

The MATLAB destructures with `[i, j, k] = find(mat)`, so its `k` is the
**value** vector, not a third index. That is only a naming quirk; the
behaviour is `mat(i(n), j(n)) = 0` wherever the value is negative.

# Cost

The reference re-indexes the sparse matrix once per negative entry, which is
quadratic in the worst case. The translation keeps that structure rather
than filtering in one pass, since the no-optimisation rule in
the crate README's "Translation policy" covers exactly this kind of rewrite. It is a
candidate for a stage-2 change, not a translation-time one.
The same clamp for a **dense** matrix — negatives to zero, everywhere.

# Why this exists separately

The reference has one `fixnegativematrix.m`, applied to both sparse
operators and the dense per-material cross-section tables that
[`crate::sigmavalupd3d_handler`] passes it. Its `find(mat)` walk visits only
stored non-zeros, which is defect C12 — a real trap for a sparse argument,
where a structural zero standing in for a negative would be missed.

**For a dense argument the two are equivalent**, because every entry is
stored and the ones `find` skips are exactly the zeros, which need no
clamping. So this is the same function, not a repair: it clamps every
negative entry to zero and leaves the rest alone.

```rust
pub fn fixnegativematrix_dense(mat: &mut crate::matlab::Array2<f64>) { /* ... */ }
```

#### Function `fixnegativematrix`

```rust
pub fn fixnegativematrix(mat: &mut crate::matlab::SparseMatrix) { /* ... */ }
```

## Module `geom2dxycase1`

A 2-D x-y test case: a UO2 square encased in moderator.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `geom2dxycase1.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# NOTHING IN THE SNAPSHOT CAN RUN THIS CASE

Read this before reaching for it. `geom2dxycase1.m` builds a **two
dimensional** problem:

- the geometry has `Lx` and `Ly` but **no `Lz`**, and `Vi` is an *area*;
- the boundaries are named `left`, `right`, `top`, `bottom`, not
  `xmin`/`xmax`/`ymin`/`ymax`/`zmin`/`zmax`;
- `whichsigma` is `maxix` by `maxiy`, with no third index.

Every solver in the snapshot is 3-D and takes the `xyz` boundary names, so
none of them can consume this. `main_exec_diff3d.m` confirms it: the call is
there, **commented out**, alongside the live 3-D cases. It is a legacy case
for a 2-D solver that was not shipped.

It is translated because it is part of the snapshot and its data is worth
keeping, not because it can be run. It returns its own [`Case2d`] rather
than the crate's 3-D [`crate::types::Geometry`], because forcing it into a
3-D type would mean inventing an axial dimension the reference does not
have — an interpretation, not a translation.

# The problem

A 8 x 8 cm UO2 square centred in a 24 x 24 cm moderator block, vacuum on all
four sides. One energy group, two materials, and both share
`Sigma_tot = 5` and `Sigma_s = 0.9 * Sigma_tot`; only the fuel fissions,
with `Sigma_f = 0.05 * Sigma_tot` and `nu = 1`.

# Its own quoted result

The file's header records:

> `k_eff = 0.487` at `Lux = Luy = Lpx = Lpy = 8`

which is the configuration built here. [`REFERENCE_K_EFF`] carries it. **It
has not been reproduced** — there is no solver to reproduce it with — and it
is quoted from a comment, not from a publication. It is recorded so that
whoever writes a 2-D solver has a target waiting.

```rust
pub mod geom2dxycase1 { /* ... */ }
```

### Types

#### Struct `Case2d`

A 2-D case, as the reference builds it.

Deliberately **not** [`crate::types::Geometry`]; see the module docs.

```rust
pub struct Case2d {
    pub xtot: f64,
    pub ytot: f64,
    pub lx: Vec<f64>,
    pub ly: Vec<f64>,
    pub vi: Vec<f64>,
    pub ctr: Vec<(f64, f64)>,
    pub whichsigma: Vec<Vec<usize>>,
    pub tot: [f64; 2],
    pub f: [f64; 2],
    pub s: [f64; 2],
    pub nu: [f64; 2],
    pub chi: [f64; 2],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xtot` | `f64` | `geometry.Xtot` — `Lux + 2*Lpx`, cm. |
| `ytot` | `f64` | `geometry.Ytot` — `Luy + 2*Lpy`, cm. |
| `lx` | `Vec<f64>` | `geometry.Lx` — cell width in `x`, one per cell. |
| `ly` | `Vec<f64>` | `geometry.Ly` — cell width in `y`, one per cell. |
| `vi` | `Vec<f64>` | `geometry.Vi` — the **area** of each cell, cm². Not a volume. |
| `ctr` | `Vec<(f64, f64)>` | `geometry.Ctr` — each cell's centre `(x, y)`, cm. |
| `whichsigma` | `Vec<Vec<usize>>` | `whichsigma(ix, iy)` — **1** for fuel, **2** for moderator.<br><br>Note this case has no void: every cell carries a material, so unlike the<br>3-D cases `0` never appears. |
| `tot` | `[f64; 2]` | `sigmavalues.tot` per material, cm⁻¹. |
| `f` | `[f64; 2]` | `sigmavalues.f` per material, cm⁻¹. Only the fuel fissions. |
| `s` | `[f64; 2]` | `sigmavalues.s(m, 1, 1)` — within-group scattering per material, cm⁻¹. |
| `nu` | `[f64; 2]` | `constants.nu` per material. |
| `chi` | `[f64; 2]` | `constants.chi` per material.<br><br>**Both entries are 1**, not a normalised spectrum — with one energy<br>group there is nowhere else for a fission neutron to go. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Case2d { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Case2d) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `geom2dxycase1`

`[params, geometry, constants, whichsigma, sigmavalues] = geom2dxycase1(params)`.

Builds the 2-D case on a `maxix` by `maxiy` mesh. All four boundaries are
vacuum, so they are not carried on [`Case2d`] — there is nothing to choose.

# Panics

If `maxix` or `maxiy` is zero.

```rust
pub fn geom2dxycase1(maxix: usize, maxiy: usize) -> Case2d { /* ... */ }
```

### Constants and Statics

#### Constant `REFERENCE_K_EFF`

The `k_eff` `geom2dxycase1.m`'s header quotes for this configuration.

**Not reproduced**: nothing in the snapshot can solve a 2-D case. Quoted
from the file's own comment.

```rust
pub const REFERENCE_K_EFF: f64 = 0.487;
```

#### Constant `L`

The fuel square's half-extent and the moderator margin, cm.

The reference sets `Lux = Luy = Lpx = Lpy = 8`, so the fuel is 8 cm across
and sits in an 8 cm margin on every side.

```rust
pub const L: f64 = 8.0;
```

## Module `geometry_ends3d`

Locate the first and last fuelled node along each grid line.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `geometry_ends3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod geometry_ends3d { /* ... */ }
```

### Functions

#### Function `geometry_ends3d`

`geometry = geometry_ends3d(params, geometry, whichsigma)`.

For every grid line in each of the three directions, records the index of
the first node with material present and the index of the last one. The
nodal solvers use these to apply the outer boundary condition at the real
edge of the reactor rather than at the edge of the bounding box.

Six fields are written onto `geometry`: `xlows`/`xhis` indexed `(iy, iz)`,
`ylows`/`yhis` indexed `(ix, iz)`, and `zlows`/`zhis` indexed `(ix, iy)`.
All stored values are **0-based node indices**.

# Arguments

- `params` — supplies the extents.
- `geometry` — modified in place, gaining the six bound arrays.
- `whichsigma` — material index per node, `0` meaning no material.

# Defaults when a line is empty or full

The reference pre-fills `lows` with the first index and `his` with the last,
then overwrites. A grid line with **no** material therefore reports the full
span rather than an empty range — the caller cannot distinguish "entirely
fuelled" from "entirely empty" from these arrays alone.

# Reference limitation — a single contiguous run per line

The scan stops at the first empty node after material is found (`break`). A
grid line with material, then a gap, then material again — an internal void
or a re-entrant boundary — has everything past the gap silently excluded,
with no warning. The benchmark geometries in this snapshot are convex, so
the case does not arise there, but the limitation is real and is translated
as written rather than generalised.

```rust
pub fn geometry_ends3d(params: &crate::types::Params, geometry: &mut crate::types::Geometry, whichsigma: &crate::matlab::Array3<usize>) { /* ... */ }
```

## Module `iapws_if97`

IAPWS-IF97 water and steam properties — translated from `IAPWS_IF97.m`.

# Provenance — third-party, BSD-2-Clause

Unlike every other module in this crate, this one is **not** Than Yan Ren's
code. It is a translation of a third-party MATLAB implementation that the
BEDOK snapshot vendored in.

- **Upstream project:** IAPWS_IF97, <https://github.com/mikofski/IAPWS_IF97>
- **Source file:** `IAPWS_IF97.m` (no tagged release; taken as vendored
  into the `main_exec_diff3d_standalone` snapshot)
- **Copyright:** Copyright (c) 2013, Mark Mikofski
- **Licence:** BSD-2-Clause, reproduced in full in the crate `NOTICE`
- **Compatibility:** BSD-2-Clause is GPL-3.0-compatible, so the combined
  work ships GPL-3.0-only while these terms continue to govern this module.

The vendored copy renders the copyright line as "Mark Mifofski"; the
upstream repository and its `license.txt` both give **Mikofski**, which is
used here as the correct spelling.

# What it implements

27 basic property functions over the IAPWS Industrial Formulation 1997,
plus IAPWS-IF97-S01, -S03rev, -S04, -S05, the 2008 Revised Advisory Note
No. 3 on thermodynamic derivatives, the 2008 viscosity release, and the
2008 revised release of the 1985 thermal-conductivity formulation.

# Naming — the `<property>_<arguments>` convention

The reference dispatches on a string built from a property symbol, an
underscore, and the symbols it depends on: `k_pT` is thermal conductivity
as a function of pressure and temperature. Derivatives prefix `d` and
suffix `d<symbol>`: `dTdp_ph`. Saturation suffixes `sat`, saturated liquid
`L`, saturated vapour `V`. The one irregular name is `cp_ph`, which is
`1/dTdh_ph`.

Function names are carried over verbatim, lowercased for Rust:
`dgammadtau1_pT` becomes `dgammadtau1_pt`.

# Units — these are not SI, and that matters

The reference works in the formulation's own units throughout, and so does
this translation:

| Symbol | Quantity | Unit |
|---|---|---|
| `p` | pressure | MPa |
| `T` | temperature | K |
| `h` | specific enthalpy | kJ/kg |
| `v` | specific volume | m³/kg |
| `x` | quality | mass fraction |
| `k` | thermal conductivity | W/m/K |
| `mu` | viscosity | Pa·s |
| `cp` | isobaric specific heat | kJ/kg/K |

**Pressure in MPa and enthalpy in kJ/kg** are the two that catch people out.
Nothing here carries `uom` types, matching the rest of the translation.

# Scalar, where the reference is vectorised

`IAPWS_IF97.m` is vectorised across inputs, and the entry function spends
its first 110 lines on shape juggling — transposing row vectors, expanding
a scalar against a matrix, bailing to `NaN` for rank > 2. That is MATLAB
array-language plumbing, not physics, and it is not reproduced: the
functions here are scalar, and a caller wanting elementwise behaviour maps
over a slice.

The one behaviour worth noting as *lost*: mismatched input shapes return
`NaN` in the reference rather than raising. A caller relying on that will
need to handle the mismatch itself.

# Exponentiation

The residual sums raise to integer-valued exponents held as `f64`. These use
[`f64::powf`], not `powi`, because MATLAB's `.^` on doubles calls the
platform `pow()`. The two can differ in the last bits, and `powf` is the
closer match.

# Status — partial

`IAPWS_IF97.m` is 3,361 lines and 107 subfunctions. Translated so far:

| Part | Status |
|---|---|
| Region 1 (`gamma` derivatives) | done — [`region1`] |
| Region 2 (`gamma` derivatives) | done — [`region2`] |
| Region 3 (`phi` derivatives) | not started |
| Region 4 (saturation line) | done — [`region4`] |
| Backward equations `T*_ph` | regions 1 and 2 done — [`backward`]; region 3 not started |
| Region boundaries `TB23_p`, `h2bc_p` | done — [`backward`] |
| Viscosity and thermal conductivity | not started |
| Basic properties | partial — [`basic`] has `h`, `v` and `cp` for regions 1 and 2, plus `hL_p`, `hV_p`, `vL_p`, `vV_p` |
| Quality `x_ph`, `x_hT`, `x_pv`, `x_vT` | not started |

**One gap is load-bearing and worth knowing about, and it is the same gap
everywhere:** region 3 is not translated, so anything that would need it
returns `NaN` rather than a wrong number. That caps [`basic::hl_p`],
[`basic::hv_p`], [`basic::vl_p`], [`basic::vv_p`] and [`backward::t_ph`] at
the region 1/3 boundary, **16.5292 MPa**. Both BEDOK operating points — a
PWR at 15.5 MPa and a BWR at 7 MPa — sit below it, so the benchmark cases
are unaffected. The individual sub-equations ([`backward::t2b_ph`],
[`backward::t2c_ph`], …) carry no such cap and are verified well above it.

The BEDOK thermal hydraulics calls this almost entirely through the `_ph`
entry points — `T_ph`, `v_ph`, `cp_ph`, `x_ph` — so those and their
dependency chains are the critical path.

# Verification so far

Both translated regions are checked against the published verification
values, measured 2026-08-12:

- **Region 1** — IAPWS-IF97 Table 5, worst case **2.810e-9** relative.
- **Region 2** — IAPWS-IF97 Table 15, worst case **1.841e-9** relative.
- **Region 4** — IAPWS-IF97 Tables 35 and 36, worst case **1.752e-9**
  relative (added 2026-08-13).
- **[`basic`]** — the dimensioned `h1_pT` and `h2_pT` reproduce their
  regions' figures exactly, confirming the `R * Tstar` scaling adds nothing.
  The saturated-enthalpy chain `hL_p -> Tsat_p -> h1_pT` puts the normal
  boiling point at **373.1243 K = 99.974 °C** and the latent heat at
  **2256.54 kJ/kg**, both textbook.

Per-state numbers are in the test doc comments in [`region1`] and
[`region2`]. The published tables carry 9 significant figures, so this is
agreement at the reference's own precision.

- **Backward equations** — IAPWS-IF97 Table 7 (region 1) to **6.590e-10**
  and Table 24 (regions 2a, 2b, 2c, nine states) to **4.603e-9**
  (added 2026-08-13).

The backward equations get a second, independent check that uses no
published value: round-tripping `T -> h -> T` against the forward equations.
Over 122 region-1 states the worst error is **23.2 mK**, and over the
vapour sweep through the [`backward::t_ph`] dispatcher **15.0 mK** — both
inside the formulation's stated 25 mK backward tolerance. Landing *near*
that tolerance rather than far below it is the expected signature of a
correct transcription: the fits are designed to be exactly this accurate.
The dispatcher sweep additionally verifies the 2a/2b/2c sub-region
selection, since the three fits agree only on their shared boundaries — and
those boundaries are themselves checked, `T2b` against `T2c` on
`h2bc_p` to within 7.5 mK.

Region 4 additionally cross-checks its two directions against each other:
`Tsat_p(psat_T(T))` returns to within **4.263e-15** relative over the whole
line. That is machine precision, five orders tighter than the agreement with
the published tables — which localises the ~1e-9 table residual to the
printed values' own rounding rather than to either expression.

Region 2 additionally cross-checks `dgammadpitau` against a finite
difference of `dgammadpi`. Those two are transcribed from separate
expressions, each with its own copy of the 43-term coefficient table, so
their agreement is an independent check on both — the published-value test
alone would flag a mistyped coefficient without localising it.

No other region has been translated, so no other region is verified.

```rust
pub mod iapws_if97 { /* ... */ }
```

### Modules

## Module `backward`

Backward equations — temperature from pressure and enthalpy.

# Provenance — third-party, BSD-2-Clause

As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
Source functions `T_ph`, `T1_ph`, `T2a_ph`, `T2b_ph`, `T2c_ph`, `h2bc_p`,
`TB23_p`.

# What a backward equation is, and why it exists

The IF97 region equations are explicit in `(p, T)`. Thermal-hydraulics
marches **enthalpy**, so it needs the inverse, `T(p, h)` — and inverting the
forward equation numerically at every node of every iteration is far too
slow. The formulation therefore publishes separate fitted polynomials for
the inverse, accurate to within the forward equation's own tolerance but
evaluable in one pass.

The cost is that the `(p, h)` plane has to be **subdivided** more finely
than the `(p, T)` plane: region 2 alone needs three sub-regions, 2a, 2b and
2c, with boundaries of their own. Most of `T_ph` is deciding which
polynomial applies.

# Units

Pressure MPa, enthalpy kJ/kg, temperature K.

```rust
pub mod backward { /* ... */ }
```

### Functions

#### Function `h2bc_p`

`h = h2bc_p(p)` — the enthalpy on the boundary between sub-regions 2b and
2c, kJ/kg, from pressure in MPa.

The 2b/2c divide follows the 5.85 kJ/(kg·K) isentrope, which the formulation
fits as a square root in pressure.

```rust
pub fn h2bc_p(p: f64) -> f64 { /* ... */ }
```

#### Function `tb23_p`

`T = TB23_p(p)` — the temperature on the region 2 / region 3 boundary, K,
from pressure in MPa.

```rust
pub fn tb23_p(p: f64) -> f64 { /* ... */ }
```

#### Function `t1_ph`

`T = T1_ph(p, h)` — region 1 (compressed liquid), K.

Twenty terms in `pi^I * (eta + 1)^J` with `eta = h / 2500`. **No range
check**, matching the reference: outside region 1 this is an extrapolation.
[`t_ph`] does the region selection.

```rust
pub fn t1_ph(p: f64, h: f64) -> f64 { /* ... */ }
```

#### Function `t2a_ph`

`T = T2a_ph(p, h)` — region 2a (superheated vapour, p <= 4 MPa), K.

```rust
pub fn t2a_ph(p: f64, h: f64) -> f64 { /* ... */ }
```

#### Function `t2b_ph`

`T = T2b_ph(p, h)` — region 2b (superheated vapour, the middle band), K.

```rust
pub fn t2b_ph(p: f64, h: f64) -> f64 { /* ... */ }
```

#### Function `t2c_ph`

`T = T2c_ph(p, h)` — region 2c (superheated vapour, high pressure), K.

Note the **negative** `I` exponents on `(pi + 25)`, which is why the
exponent arrays here are signed.

```rust
pub fn t2c_ph(p: f64, h: f64) -> f64 { /* ... */ }
```

#### Function `t_ph`

`T = T_ph(p, h)` — temperature of liquid, vapour or mixture, K.

# Arguments

- `p` — pressure, **MPa**.
- `h` — specific enthalpy, **kJ/kg**.

# Returns

Temperature in **K**, or `NaN` where this translation does not cover the
state — see the range note below.

# How the region is chosen

The `(p, h)` plane is divided by comparing `h` against boundary enthalpies
computed at the given pressure:

| Condition | Region | Equation |
|---|---|---|
| `h <= h1(p, Tsat)` | 1, compressed liquid | [`t1_ph`] |
| `h1(p, Tsat) < h <= h2(p, Tsat)` | 4, two-phase | `Tsat(p)` |
| `h > h2(p, Tsat)`, `p <= 4 MPa` | 2a | [`t2a_ph`] |
| `h > h2(p, Tsat)`, `4 < p <= p2bc,sat`, or `h > h2bc(p)` above it | 2b | [`t2b_ph`] |
| `h > h2(p, Tsat)`, `p > p2bc,sat`, `h <= h2bc(p)` | 2c | [`t2c_ph`] |

**In the two-phase region the answer is `Tsat(p)`**, which is exact but
carries no information about quality — that is what `h` is for, and
`singleflow1devap.m` recovers it separately.

# Range — capped at the region 1/3 boundary

This returns `NaN` for `p > 16.5292 MPa`, the same region-3 gap
[`super::basic::hl_p`] documents. Above that pressure the liquid branch, the
saturation line and part of the vapour branch all need region 3, which is
not translated. Both BEDOK operating points — a PWR at 15.5 MPa and a BWR at
7 MPa — sit below it.

Below the triple-point pressure it also returns `NaN`, which is the
reference's own behaviour.

```rust
pub fn t_ph(p: f64, h: f64) -> f64 { /* ... */ }
```

## Module `basic`

Basic property functions — enthalpy from the region equations.

# Provenance — third-party, BSD-2-Clause

As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
Source functions `h1_pT`, `h2_pT`, `hL_p`, `hV_p`.

# What belongs here

The reference's "basic and fundamental functions" block — the thin layer
that turns a region's dimensionless Gibbs derivative into a dimensioned
property. `h1_pT` is three lines around [`crate::iapws_if97::region1`], and
that is the whole pattern.

Only the functions BEDOK actually calls are translated. The full block also
carries `v`, `u`, `s`, `cp`, `cv`, `w` for each region; those come in when a
caller needs them.

# Units

Pressure MPa, temperature K, specific enthalpy kJ/kg.

```rust
pub mod basic { /* ... */ }
```

### Functions

#### Function `h1_pt`

`h = h1_pT(p, T)` — specific enthalpy of **compressed liquid** (region 1).

# Arguments

- `p` — pressure, **MPa**.
- `t` — temperature, **K**.

Region 1 is valid for 273.15 K ≤ T ≤ 623.15 K up to 100 MPa, on the liquid
side of the saturation line. **The reference does not check this**, and
neither does this function: it evaluates the region-1 equation wherever it
is asked, so a caller outside the region gets an extrapolation rather than a
`NaN`. The region tests live in the callers.

# Returns

Specific enthalpy, **kJ/kg**.

```rust
pub fn h1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `h2_pt`

`h = h2_pT(p, T)` — specific enthalpy of **superheated vapour** (region 2).

# Arguments

- `p` — pressure, **MPa**.
- `t` — temperature, **K**.

Same absence of range checking as [`h1_pt`].

# Returns

Specific enthalpy, **kJ/kg**.

```rust
pub fn h2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `hl_p`

`h = hL_p(p)` — specific enthalpy of **saturated liquid**, from pressure.

# Arguments

- `p` — pressure, **MPa**, on `[611.657e-6, 22.064]`.

# Returns

Specific enthalpy, **kJ/kg**, or `NaN` outside the range this translation
covers — see below.

# Partial: region 4b needs region 3, which is not translated

The reference splits the saturation line at the region 1/3 boundary,
`p_B13sat = 16.5291643 MPa`:

- **below it** (region 4a) the saturated liquid is region 1, so
  `hL = h1_pT(p, Tsat(p))`;
- **above it** (region 4b) it is region 3, so `hL = h3_rhoT(1/vL_p(p),
  Tsat(p))`.

Region 3 is not translated, so 4b returns `NaN` here where the reference
returns a number. This is a **real gap**, and the reason it is acceptable
for now is that both BEDOK operating points sit below the boundary — a PWR
at 15.5 MPa and a BWR at 7 MPa — so the benchmark cases never reach 4b. A
caller above 16.53 MPa gets `NaN`, which is loud rather than silent.

`NaN` outside the saturation line altogether is the reference's own
behaviour, not a gap.

```rust
pub fn hl_p(p: f64) -> f64 { /* ... */ }
```

#### Function `hv_p`

`h = hV_p(p)` — specific enthalpy of **saturated vapour**, from pressure.

# Arguments

- `p` — pressure, **MPa**, on `[611.657e-6, 22.064]`.

# Returns

Specific enthalpy, **kJ/kg**, or `NaN` above the region 1/3 boundary
pressure — the same region-3 gap [`hl_p`] documents, with region 2 standing
in for region 1 on the vapour side.

```rust
pub fn hv_p(p: f64) -> f64 { /* ... */ }
```

#### Function `v1_pt`

`v = v1_pT(p, T)` — specific volume of compressed liquid (region 1), m³/kg.

# Arguments

- `p` — pressure, **MPa**. - `t` — temperature, **K**.

# The reducing pressure is 16.53 MPa, not 1

Region 1 reduces pressure by `pstar = 16.53 MPa`, where region 2 uses 1 MPa.
The `1e-3` converts `kJ/m³` to `MPa`. Getting either wrong scales the answer
by a large constant factor, so both are spelled out here.

# Returns

Specific volume, **m³/kg**. Note this is SI, unlike the cm-g-s units the
rest of BEDOK works in — a caller wanting g/cm³ takes `1/(1000*v)`.

```rust
pub fn v1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `v2_pt`

`v = v2_pT(p, T)` — specific volume of superheated vapour (region 2), m³/kg.

As [`v1_pt`], but reducing by `pstar = 1 MPa`.

```rust
pub fn v2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `cp1_pt`

`cp = cp1_pT(p, T)` — isobaric specific heat of compressed liquid,
kJ/(kg·K).

```rust
pub fn cp1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `cp2_pt`

`cp = cp2_pT(p, T)` — isobaric specific heat of superheated vapour,
kJ/(kg·K).

```rust
pub fn cp2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `vl_p`

`v = vL_p(p)` — specific volume of **saturated liquid**, m³/kg.

Same region-4a/4b split, and the same region-3 gap above 16.5292 MPa, as
[`hl_p`].

```rust
pub fn vl_p(p: f64) -> f64 { /* ... */ }
```

#### Function `vv_p`

`v = vV_p(p)` — specific volume of **saturated vapour**, m³/kg.

As [`vl_p`], through region 2.

```rust
pub fn vv_p(p: f64) -> f64 { /* ... */ }
```

#### Function `hfg_p`

`hfg = hV_p(p) - hL_p(p)` — the latent heat of vaporisation, kJ/kg.

Not a function of the reference — the MATLAB writes the difference out at
each call site. Named here because the thermal hydraulics uses it often
enough that the subtraction is worth a name, and because it makes the shared
`NaN` range of its two operands explicit.

```rust
pub fn hfg_p(p: f64) -> f64 { /* ... */ }
```

## Module `region1`

Region 1 — the Gibbs free energy `gamma` and its derivatives.

Region 1 is the subcooled/compressed liquid: `273.15 K <= T <= 623.15 K`
with `p` between the saturation line and 100 MPa.

# Provenance

Translated from `IAPWS_IF97.m` by Mark Mikofski — see the crate `NOTICE`
for the full BSD-2-Clause terms this translation is made under, and
[`super`] for the module-level provenance block.

```rust
pub mod region1 { /* ... */ }
```

### Functions

#### Function `dgammadtau1_pt`

`dgammadtau1_pT(p, T)` — first derivative of `gamma` with respect to `tau`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `h1_pT` as `h = R * Tstar * dgammadtau`.

```rust
pub fn dgammadtau1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpi1_pt`

`dgammadpi1_pT(p, T)` — first derivative of `gamma` with respect to `pi`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `v1_pT` as `v = 1e-3 * R * T / pstar * dgammadpi`.

```rust
pub fn dgammadpi1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadtautau1_pt`

`dgammadtautau1_pT(p, T)` — second derivative with respect to `tau`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `cp1_pT` as `cp = -R * tau^2 * dgammadtautau`.

```rust
pub fn dgammadtautau1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpipi1_pt`

`dgammadpipi1_pT(p, T)` — second derivative with respect to `pi`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `kappaT1_pT`, the isothermal compressibility.

```rust
pub fn dgammadpipi1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpitau1_pt`

`dgammadpitau1_pT(p, T)` — mixed second derivative.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `alphav1_pT`, the isobaric cubic expansion
coefficient.

```rust
pub fn dgammadpitau1_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

## Module `region2`

Region 2 — the Gibbs free energy `gamma` and its derivatives.

Region 2 is the superheated vapour: from the saturation line up to 1073.15 K
below 100 MPa, and up to the 623.15 K / B23 boundary above it.

# Structure — ideal plus residual

Unlike region 1, `gamma` here splits into an **ideal-gas part** (9 terms in
`tau` alone) and a **residual part** (43 terms in both `pi` and `tau`), and
each derivative is the sum of the two. The ideal part of the `pi`
derivatives is analytic rather than a sum: `1/pi`, `-1/pi^2`, and exactly
zero for the mixed derivative.

# Provenance

Translated from `IAPWS_IF97.m` by Mark Mikofski — see the crate `NOTICE`
for the full BSD-2-Clause terms this translation is made under, and
[`super`] for the module-level provenance block.

```rust
pub mod region2 { /* ... */ }
```

### Functions

#### Function `dgammadtau2_pt`

`dgammadtau2_pT(p, T)` — first derivative of `gamma` with respect to `tau`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `h2_pT` as `h = R * Tstar * dgammadtau`.

```rust
pub fn dgammadtau2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpi2_pt`

`dgammadpi2_pT(p, T)` — first derivative of `gamma` with respect to `pi`.

The ideal contribution is `1/pi` in closed form, not a summation.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `v2_pT` as `v = 1e-3 * R * T / pstar * dgammadpi`.

```rust
pub fn dgammadpi2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadtautau2_pt`

`dgammadtautau2_pT(p, T)` — second derivative with respect to `tau`.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `cp2_pT` as `cp = -R * tau^2 * dgammadtautau`.

```rust
pub fn dgammadtautau2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpipi2_pt`

`dgammadpipi2_pT(p, T)` — second derivative with respect to `pi`.

The ideal contribution is `-1/pi^2` in closed form.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `kappaT2_pT`, the isothermal compressibility.

```rust
pub fn dgammadpipi2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `dgammadpitau2_pt`

`dgammadpitau2_pT(p, T)` — mixed second derivative.

The ideal part depends on `tau` alone, so its `pi` derivative is exactly
zero and the reference writes `dgammadpitau0 = 0` rather than summing.

# Arguments
- `p` — pressure, MPa.
- `t` — temperature, K.

# Returns
Dimensionless. Feeds `alphav2_pT`, the isobaric cubic expansion
coefficient.

# Sign, relative to region 1

Note this sum is **not** negated, where region 1's `dgammadpitau1_pT` is.
That is not an inconsistency: region 1's `pi` dependence enters as
`(7.1 - pi)^I`, whose `pi` derivative carries a minus sign, while region 2's
enters as `pi^I` and does not.

```rust
pub fn dgammadpitau2_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

## Module `region4`

Region 4 — the saturation line, `psat(T)` and `Tsat(p)`.

# Provenance — third-party, BSD-2-Clause

As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
Source functions `psat_T` and `Tsat_p`.

# What region 4 is

Regions 1, 2 and 3 each cover an area of the `(p, T)` plane. Region 4 is not
an area — it is the **curve** separating liquid from vapour, and the
formulation gives it as a single quartic in a reduced variable that can be
solved either way round. So one set of ten coefficients serves both
directions, which is why `psat_T` and `Tsat_p` below share the array `N`.

# Units

Pressure MPa, temperature K, as everywhere in this module.

# Range

The line runs from the triple point (273.16 K, 611.657 Pa) to the critical
point (647.096 K, 22.064 MPa). Outside that, both functions return `NaN` —
the reference initialises its output to `NaN` and only fills the valid mask,
which this reproduces. `NaN` is therefore the answer for a subcritical
query, not an error.

```rust
pub mod region4 { /* ... */ }
```

### Functions

#### Function `psat_t`

`p = psat_T(T)` — saturation pressure, MPa, from temperature, K.

# Arguments

- `t` — temperature in **K**, valid on `[273.16, 647.096]`, the triple point
  to the critical point.

# Returns

Saturation pressure in **MPa**, or `NaN` outside the valid range.

# Numerics

The reference evaluates the three quadratics `A`, `B`, `C` by Horner's
method and takes the root `beta = 2C / (-B + sqrt(B^2 - 4AC))`. That is the
*numerically stable* branch of the quadratic formula for this sign pattern —
the algebraically equivalent `(-B + sqrt(...)) / 2A` suffers cancellation.
Written the same way here.

```rust
pub fn psat_t(t: f64) -> f64 { /* ... */ }
```

#### Function `tsat_p`

`T = Tsat_p(p)` — saturation temperature, K, from pressure, MPa.

# Arguments

- `p` — pressure in **MPa**, valid on `[611.657e-6, 22.064]`, the triple
  point to the critical point. The lower bound is `psat_T(273.16)` and is
  computed rather than hard-coded, as the reference does.

# Returns

Saturation temperature in **K**, or `NaN` outside the valid range.

# Numerics

The mirror of [`psat_t`]: Horner-evaluated `E`, `F`, `G`, then
`D = 2G / (-F - sqrt(F^2 - 4EG))` and a second stable-branch root for
`theta`. Both minus signs are load-bearing for the same cancellation reason.

```rust
pub fn tsat_p(p: f64) -> f64 { /* ... */ }
```

#### Function `p_b13_sat`

Saturation pressure at the region 1 / region 3 boundary, MPa.

`psat_T(623.15) = 16.5291643 MPa`, per the reference's own comment. Computed
rather than hard-coded, so it cannot drift from [`psat_t`].

```rust
pub fn p_b13_sat() -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `T_MIN`

Triple-point temperature, K — the low end of the saturation line.

```rust
pub const T_MIN: f64 = 273.16;
```

#### Constant `T_CRIT`

Critical temperature, K — the high end of the saturation line.

```rust
pub const T_CRIT: f64 = 647.096;
```

#### Constant `P_CRIT`

Critical pressure, MPa.

```rust
pub const P_CRIT: f64 = 22.064;
```

#### Constant `T_B13`

Temperature at the region 1 / region 3 boundary, K.

Above this, saturated **liquid** properties need region 3 rather than
region 1. See [`crate::iapws_if97::basic::hl_p`], which is why this is
public.

```rust
pub const T_B13: f64 = 623.15;
```

## Module `transport`

Transport properties — viscosity and thermal conductivity.

# Provenance — third-party, BSD-2-Clause

As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
Source functions `mu_pT`, `k_pT`, `pB23_T`.

# These are separate IAPWS releases, not part of IF97 itself

IF97 is a formulation for the *thermodynamic* properties. Viscosity and
thermal conductivity come from their own releases, which IF97 supplies the
density for:

- **Viscosity** — IAPWS Formulation 2008 for the Viscosity of Ordinary
  Water Substance.
- **Thermal conductivity** — Revised Release on the IAPWS Formulation 1985
  for the Thermal Conductivity of Ordinary Water Substance (2008 revision).

Both are written in terms of **reduced density and temperature**, so the
kernels here take `(rho, T)` and the `(p, T)` wrappers get `rho` from the
IF97 region equations. That split is not in the reference — it inlines the
kernel three times, once per region — but it matters for verification: the
published check tables are stated at given `(rho, T)`, so the kernel can be
checked against them *exactly*, which a `(p, T)` entry point cannot be.

# The critical enhancement is absent, and that is the reference's choice

The viscosity release defines `mu = mu0 * mu1 * mu2`, where `mu2` is a
critical-region enhancement that rises steeply near 647 K and 322 kg/m³.
The reference computes only `mu0 * mu1`, the "industrial" form the release
permits outside that region. Away from the critical point `mu2` is 1, so
this is exact for reactor conditions; within roughly 10 K and 100 kg/m³ of
critical it under-predicts. Preserved as written.

# Units

Pressure MPa, temperature K, density kg/m³, viscosity **Pa·s**, thermal
conductivity **W/(m·K)**. Note both are SI here, where the BEDOK callers
work in cm-g-s and convert at the call site.

```rust
pub mod transport { /* ... */ }
```

### Functions

#### Function `pb23_t`

`p = pB23_T(T)` — pressure on the region 2 / region 3 boundary, MPa, from
temperature in K.

The inverse of [`super::backward::tb23_p`], and a plain quadratic where that
one is a square root. Both are needed: the transport functions select their
region by pressure at a given temperature.

```rust
pub fn pb23_t(t: f64) -> f64 { /* ... */ }
```

#### Function `mu_rho_t`

`mu = mu0(T) * mu1(rho, T)` — dynamic viscosity, **Pa·s**, from density and
temperature.

# Arguments

- `rho` — density, **kg/m³**.
- `t` — temperature, **K**.

# Returns

Dynamic viscosity in **Pa·s**. Multiply by 1e6 for the µPa·s the published
tables use.

# What this omits

The critical enhancement `mu2`; see the module docs.

```rust
pub fn mu_rho_t(rho: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `k_rho_t`

`k = k0 + k1 + k2` — thermal conductivity, **W/(m·K)**, from density and
temperature.

# Arguments

- `rho` — density, **kg/m³**.
- `t` — temperature, **K**.

# The reducing constants are not the critical constants

This release reduces by `Tstar = 647.26 K` and `rhostar = 317.7 kg/m³`,
which are the 1985 formulation's own values and differ slightly from the
critical point (647.096 K, 322.0 kg/m³) that the viscosity release uses.
Mixing the two would be a small but systematic error, so they are declared
separately here rather than shared.

# The `S` term branches on temperature

`S` is `1/deltaTbar` at or above the reducing temperature and
`C6/deltaTbar^0.6` below it. The reference writes this as a sum of two
logical-mask products, which is the MATLAB idiom for a branch; it is an
`if` here.

```rust
pub fn k_rho_t(rho: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `mu_pt`

`mu = mu_pT(p, T)` — dynamic viscosity, **Pa·s**, from pressure and
temperature.

# Arguments

- `p` — pressure, **MPa**, up to 100 MPa.
- `t` — temperature, **K**, from the triple point to 1073.15 K.

# Returns

Viscosity in **Pa·s**, or `NaN` outside the validity envelope, or in region
3 — see below.

# Region 3 returns `NaN`

The reference gets region 3's density from `v_pT`, which dispatches into the
region-3 backward equations. Those are not translated, so this returns `NaN`
there rather than a wrong number — the same principled gap
[`super::basic::hl_p`] and [`super::backward::t_ph`] carry. Region 3 is
above 623.15 K and above the 2/3 boundary pressure; BEDOK's coolant does not
go there.

```rust
pub fn mu_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

#### Function `k_pt`

`k = k_pT(p, T)` — thermal conductivity, **W/(m·K)**, from pressure and
temperature.

Same arguments, envelope and region-3 gap as [`mu_pt`].

```rust
pub fn k_pt(p: f64, t: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `T_MIN`

Lower temperature bound for both correlations, K — the triple point.

```rust
pub const T_MIN: f64 = 273.16;
```

#### Constant `T_B13`

Region 1/3 boundary temperature, K.

```rust
pub const T_B13: f64 = 623.15;
```

#### Constant `T_B23`

Region 2/3 boundary temperature, K, as the transport releases use it.

Note this is **863.15 K**, not the 623.15 K that bounds region 1 — the
transport releases carry their own region map.

```rust
pub const T_B23: f64 = 863.15;
```

#### Constant `T_MAX`

Upper temperature bound, K.

```rust
pub const T_MAX: f64 = 1073.15;
```

#### Constant `P_MAX`

Upper pressure bound, MPa.

```rust
pub const P_MAX: f64 = 100.0;
```

## Module `iaea3ds`

The IAEA 3-D PWR benchmark case.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `iaea3ds.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.
- **Composition maps:** `src/data/IAEA3DS_*.csv`; see
  `src/data/PROVENANCE.md`.

# Why this case matters more than the others

It is **pure neutronics**. There is no thermal-hydraulic feedback, no fuel
rod, no coolant — just a fixed two-group cross-section set on a fixed
material map. So it exercises the whole nodal-diffusion stack against a
*published eigenvalue* without any of the coupling layer in the way, and it
is the first thing in this crate that can be compared to a number someone
else computed.

# The problem

A 17 x 17 x 19 quarter-core PWR on a 10 cm mesh — 170 x 170 x 380 cm —
reflective on the low `x` and `y` faces (the quarter-core symmetry planes)
and vacuum on the other four. Two energy groups, five materials, fission
only in the fast group's daughter: `chi = [1, 0]`.

# The cross sections are `nu * Sigma_f`, not `Sigma_f`

`constants.nu` is **all ones**, so `sigmavalues.f` already carries the
`nu * Sigma_f` product. That is the benchmark's own convention and it is why
a `nu` of 1 is not a mistake here. Everything downstream multiplies by `nu`
anyway, so the arithmetic works out.

The rest reconstructs to the published specification exactly: `D1 = 1.5`,
`D2 = 0.4` in fuel via `Sigma_tot = 1/(3D)`, absorption 0.01 and 0.08 in the
two groups of outer fuel, and a down-scatter of 0.02 with no up-scatter.
Those identities are checked by a test rather than asserted here.

```rust
pub mod iaea3ds { /* ... */ }
```

### Functions

#### Function `iaea3ds`

`[params, geometry, constants, whichsigma, sigmavalues] = iaea3ds(params)`.

Builds the complete IAEA-3D case: extents, mesh, boundary conditions, the
two-group five-material cross-section set, and the material map.

# Returns

`(params, geometry, whichsigma, sigmavalues)`. The reference's `constants`
output carries only `chi`, `nu` and `frac_p`, all of which are already on
`sigmavalues` or `params`, so it is not returned separately.

# The mesh is fixed at 17 x 17 x 19

The reference computes `xscale = maxix/17` and friends as `int64` and uses
them to index the maps, which would in principle allow a refined mesh. But
the axial layer boundaries are then written as `14*zscale`, `18*zscale`, and
the radial lookup is `ceil(ix/maxix*17)` — an identity only at 17. This
translation fixes the mesh at the benchmark's own 17 x 17 x 19 and asserts
it, rather than reproducing a refinement path the reference never exercises
and that its own header (`FOR NODE SIZE = 10 cm`) does not claim.

# Panics

If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
17, 17, 19.

```rust
pub fn iaea3ds(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::matlab::Array3<usize>, crate::types::SigmaValues) { /* ... */ }
```

### Constants and Statics

#### Constant `REFERENCE_K_EFF_PARCS`

The benchmark's reference eigenvalue, as `iaea3ds.m`'s header records it.

Two independent codes are quoted there, agreeing to 1.4 pcm:

| Code | `k_eff` |
|---|---|
| PARCS | 1.029096 |
| ADPRES | 1.029082 |

**These come from that header, not from a primary publication** — see
`src/data/PROVENANCE.md` before citing them.

```rust
pub const REFERENCE_K_EFF_PARCS: f64 = 1.029_096;
```

#### Constant `REFERENCE_K_EFF_ADPRES`

The second reference eigenvalue; see [`REFERENCE_K_EFF_PARCS`].

```rust
pub const REFERENCE_K_EFF_ADPRES: f64 = 1.029_082;
```

## Module `handle2dcoords`

Resolve the two spatial extents for the 2-D routines.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `handle2dcoords.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod handle2dcoords { /* ... */ }
```

### Functions

#### Function `handle2dcoords`

`[maxi1, maxi2] = handle2dcoords(params)`.

Picks the first populated coordinate pair, in the reference's order:
cylindrical (`maxir`, `maxiz` — note this is the **r-z** plane, not the
`r`-`theta` pair its 3-D sibling uses), then Cartesian (`maxix`, `maxiy`),
then generic (`maxi1`, `maxi2`).

# Returns

Node counts along each of the two dimensions — dimensionless.

# Difference from [`crate::handle3dcoords::handle3dcoords`]

The 3-D version pre-initialises its outputs to `1` and so returns
`(1, 1, 1)` when nothing matches. This one does **not** initialise, so an
unmatched `params` leaves the outputs unassigned and MATLAB raises
`Output argument "maxi1" (and maybe others) not assigned`. That is
translated as [`BedokError::NoCoordinateBranch`] rather than a silent
default, because silently returning `1` here would be a repair the
reference does not make.

# Errors

[`BedokError::NoCoordinateBranch`] when no coordinate pair is fully
populated.

```rust
pub fn handle2dcoords(params: &crate::types::Params) -> crate::error::Result<(usize, usize)> { /* ... */ }
```

## Module `handle3dcoords`

Resolve the three spatial extents from whichever coordinate fields the case
file populated.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `handle3dcoords.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod handle3dcoords { /* ... */ }
```

### Functions

#### Function `handle3dcoords`

`[maxi1, maxi2, maxi3] = handle3dcoords(params)`.

Picks the first populated coordinate triple, in the reference's order:
cylindrical (`maxir`, `maxitheta`, `maxiz`), then Cartesian (`maxix`,
`maxiy`, `maxiz`), then generic (`maxi1`, `maxi2`, `maxi3`). All three
outputs default to `1` when nothing matches, exactly as the reference
initialises them.

# Returns

Node counts along each dimension — dimensionless, and at least `1`.

# Reference defect — carried over deliberately

In the generic branch the reference assigns

```text
maxi3=params.maxix;
```

where every indication is that `params.maxi3` was intended. It is
translated as written, per the no-silent-repairs rule in
the crate README, "Translation policy".

The consequence is sharper in Rust than in MATLAB, so it is worth stating.
The generic branch is only reached when the Cartesian branch did *not*
match, meaning at least one of `maxix`/`maxiy`/`maxiz` is absent. If the
absent one is `maxix`, MATLAB raises `Reference to non-existent field
'maxix'` and this function panics with the equivalent message. If `maxix`
happens to be present, both silently produce a wrong `maxi3`.

# Panics

If the generic branch is taken and `maxix` is not populated — mirroring the
reference's `Reference to non-existent field` error.

```rust
pub fn handle3dcoords(params: &crate::types::Params) -> (usize, usize, usize) { /* ... */ }
```

## Module `makegrad_dxyz`

The `gradD` diffusion operator and the `gradterms` face coefficients.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `makegradDxyz.m`, `main_exec_diff3d_standalone` snapshot.
  The Rust module is `makegrad_dxyz` because Rust warns on non-snake-case
  module names.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod makegrad_dxyz { /* ... */ }
```

### Types

#### Struct `GradD`

`gradD` and `gradterms`.

```rust
pub struct GradD {
    pub operator: crate::matlab::SparseMatrix,
    pub terms: crate::matlab::Array2<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `operator` | `crate::matlab::SparseMatrix` | The diffusion operator, `philenf` square. |
| `terms` | `crate::matlab::Array2<f64>` | Face diffusion coefficients, `philen` by 6, `(minus, plus)` per axis:<br>columns `0, 1` for `x`, `2, 3` for `y`, `4, 5` for `z`.<br><br>**Already doubled** — see the note on [`makegrad_dxyz`]. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GradD { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> GradD { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `makegrad_dxyz`

`[gradD, gradterms] = makegradDxyz(geometry, params, DiffD, whichsigma, tomode)`.

Builds the finite-difference diffusion operator from harmonic-mean face
diffusion coefficients, and records those face coefficients for the
transverse-leakage routines.

# Arguments

- `geometry` — node widths, per-line bounds, face boundary conditions.
- `params` — `G`, `Nc` and the extents.
- `diffd` — the **4-D** `(ix, iy, iz, g)` diffusion array from
  [`crate::calcdiffvalues3d::calcdiffvalues3d`]. This is one of only two
  functions taking that shape; most of the chain takes the flat `philen`
  vector instead.
- `whichsigma` — material per node, `0` for void.
- `tomode` — `None` selects the reference's default of [`IndexMode::Plain`].

# Errors

Propagates from [`convertsparseformat2d`] when `tomode` is
[`IndexMode::DiamondDifference`]. **That path is never exercised**: every
call site in the snapshot passes four arguments, so `tomode` is always 1.

# The stencil

With half-widths `h = L/2` and the harmonic-mean face coefficient

$$ \tilde{D}_{+} = \frac{(h + h_{+})}{2} \frac{D \, D_{+}}{h D + h_{+} D_{+}} \frac{1}{L} $$

the diagonal gets `Dt_plus/h_plus + Dt_minus/h_minus` and the two
neighbours get `-Dt_plus/h_plus` and `-Dt_minus/h_minus`.

At a boundary face the outward coefficient comes from the boundary
condition instead — `0` for reflective, `D/L` for zero-flux, and
`0.5 D / (2D + 0.5L)` for vacuum — while the inward one keeps the harmonic
mean. The neighbour term is pushed identically in all three branches.

# The diagonal is assigned by `z` and accumulated by `y` and `x`

This asymmetry is deliberate and load-bearing; do not "harmonise" it.

The reference pre-fills its triplet arrays with an identity, so slot `k`
*is* row `k`'s diagonal. The `z` blocks then use plain assignment
(`gradDele(idx) = ...`), wiping that `1`, while `y` and `x` accumulate onto
it. A fuelled node therefore ends with `z + y + x` and **no** identity term,
while a void node — skipped by every direction — keeps its `1`. That `1` is
exactly the unit-diagonal placeholder
[`crate::convertsparsekey3d::convertsparsekey3d`] later strips.

Making `z` accumulate too would leave a spurious `+1` on every fuelled
diagonal, and nothing would visibly break.

# Reference defect — a fuelled node outside `[low, high]` keeps its identity

The scheme above depends on `z` covering every fuelled node. A node that is
fuelled (`whichsigma != 0`) but falls **outside** `[zlow, zhi]` is skipped by
all three `z` branches, keeps its identity `1`, and then has `y` and `x`
accumulated on top — leaving a spurious `+1` on its diagonal.

That case is reachable rather than hypothetical: `geometry_ends3d` finds
only the **first contiguous run** per grid line (a limitation documented and
pinned in [`crate::geometry_ends3d`]), so material after an internal axial
gap is fuelled yet outside `[zlow, zhi]`. The two documented behaviours
interact. Pinned by a test below.

# `gradterms` is doubled at the end

The final line of the reference is

```text
gradterms=2*gradterms; %check this (seems correct)
```

The comment is the author's own, and is preserved here because it records a
genuine uncertainty rather than a settled derivation. The factor is applied
to every column, after all three directions have written.

# `geometry.Vi` is read and never used

The reference assigns `Vi=geometry.Vi;` at the top and never refers to it
again — dead code. It is therefore **not** a parameter here, and `Geometry`
needs no `vi` field on account of this function.

# Panics

If the off-diagonal count exceeds `philen*10`, reproducing
`error('Error in makegradD')`.

```rust
pub fn makegrad_dxyz(geometry: &crate::types::Geometry, params: &crate::types::Params, diffd: &crate::matlab::Array4<f64>, whichsigma: &crate::matlab::Array3<usize>, tomode: Option<crate::convertindexc2d::IndexMode>) -> crate::error::Result<GradD> { /* ... */ }
```

## Module `makeheatlaplacian_1dcylnd`

A 1-D cylindrical conduction operator — **dead code in the reference**.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `makeheatlaplacian_1dcylnd.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# Read this before using it: the reference never calls this file

Its **only** call site is `th_solverxyz.m:174`, and that line is commented
out. The live path is [`crate::fuelrodheat_1dcylnd`], which assembles the
same operator inline — and **not the same way**:

| | this file | `fuelrodheat_1dcylnd` |
|---|---|---|
| Interface conductivity | `2*cond(ir+1)*sumLr(ir+1)/Lr(ir+1)` — the outward node's value | `2*k_i*k_{i+1}/(k_i + k_{i+1})` — a harmonic mean |
| Radial weight | `sumLr`, the node's **outer** radius | `Ctr`, the node **centre** radius |
| Interface nodes | none; `maxir` unknowns | doubled at each material interface; `maxir + surfcount` unknowns |
| Gap treatment | bridges `ir` to `ir+2` | bridges, plus a dummy row |

So the snapshot carries two divergent discretisations of one operator and
the unreachable one is the more readable. Which the author intended is not
recorded. Translated because it is one of the 48 files in scope, and
recorded as defect T4 — **not** because it is a usable alternative.

A caller wanting fuel temperatures wants [`crate::fuelrodheat_1dcylnd`].

```rust
pub mod makeheatlaplacian_1dcylnd { /* ... */ }
```

### Functions

#### Function `makeheatlaplacian_1dcylnd`

`laplc = makeheatlaplacian_1dcylnd(params, geometry, temps, bc)` — the
radial conduction operator for one fuel rod.

# What it computes

The finite-volume conduction matrix for the integrated 1-D cylindrical heat
equation, **divided through by `2*pi`** as the sibling module's header notes
for the same convention. Entry `(i, i)` carries the sum of the inward and
outward conductances at node `i`, W/(cm·K); the off-diagonals carry their
negatives.

# Arguments

- `fuel` — needs `whichk`, `tcon`, `gap_conductance` and `lr`. (`Ctr` is
  read by the reference and never used — dead, and not a parameter here.)
- `maxir` — radial node count, the reference's `params.maxir`.
- `temps` — nodal temperatures, **K**, at least `maxir` long. Only used to
  evaluate the temperature-dependent conductivities.
- `bc` — the outer boundary conductance, W/(cm·K); in the live path this is
  `hcoeff * Rtot`.

# Returns

A `maxir`-square sparse operator. Rows for nodes with `whichk == 0` are left
as the identity, which the preallocation supplies.

# Reference defects carried here

- **Writes outside the declared shape (T5).** When `whichk(ir+1) == 0` the
  forward link is written to column `ir + 2`. At `ir = maxir - 1`, the last
  value the loop takes, that is column `maxir + 1` — outside the
  `sparse(..., maxir, maxir)` shape the function declares. MATLAB raises an
  index error. Here it **panics** with the same meaning, via
  [`SparseMatrix::add`]'s bounds assertion.
- **The first node's conductivity lookup is unguarded.** `cond(1) =
  tcon{whichk(1)}(temps(1))` has no `whichk(1) ~= 0` test, unlike every
  other lookup in the file and unlike `calc_tcond.m`, which exists to do
  exactly this and is called from nowhere (T6). A rod whose innermost node
  is void indexes `tcon{0}` and MATLAB raises. Panics here.
- **`sumLr` in the loop, `Lr` in the tail.** The gap conductance is scaled
  by `sumLr(irminus)` inside the loop but by `Lr(irminus)` in the final
  block — a cumulative radius against a single node thickness. One of the
  two is wrong; the snapshot does not say which.
- **The commented-out harmonic mean.** Two lines carry a struck-through
  `(Lr(i)+Lr(i+1))*(k_i k_{i+1})/(Lr(i) k_i + Lr(i+1) k_{i+1})` with the
  author's note "there should be a better formula for this". The live
  sibling module uses a harmonic mean, so this looks like an abandoned
  revision.

# Panics

If `temps` is shorter than `maxir`; if the innermost node is void (see
above); if a `whichk` value has no matching entry in `tcon`; or on the
out-of-shape column write at `ir = maxir - 1` (T5).

```rust
pub fn makeheatlaplacian_1dcylnd(fuel: &crate::types::FuelGeometry, maxir: usize, temps: &[f64], bc: f64) -> crate::matlab::SparseMatrix { /* ... */ }
```

## Module `makesigmadfxyz`

Expand per-material cross-section data onto the spatial mesh.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `makesigmadfxyz.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod makesigmadfxyz { /* ... */ }
```

### Types

#### Enum `SigmaIndexMode`

Which index grid the operators are built on.

The reference passes these as the bare integers `1` and `2` in `varargin`,
defaulting to `1`.

```rust
pub enum SigmaIndexMode {
    Full,
    HalfIndex,
}
```

##### Variants

###### `Full`

Mode 1 — full indices only, one entry per node. Every call site in the
snapshot uses this.

###### `HalfIndex`

Mode 2 — the `(2n+1)` half-index grid.

**Carries a reference defect that truncates the axial extent** — see
[`makesigmadfxyz`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaIndexMode { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SigmaIndexMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `makesigmadfxyz`

`sigma = makesigmadfxyz(params, sigmavalues, whichsigma, mode)`.

Builds the six sparse cross-section operators, plus the per-node `nu` and
`chi` arrays, by looking up each node's material and scattering its data
into the flattened `(group, node)` index space.

# Arguments

- `params` — supplies `G`, `Nc` and the extents.
- `sigmavalues` — per-material data; see [`SigmaValues`].
- `whichsigma` — material identifier per node, 1-based, `0` for void.
- `mode` — `None` selects the reference's default of [`SigmaIndexMode::Full`].

# Returns

[`Sigma`], with every matrix `philenf = philen + Nc*es` square. The
precursor tail beyond `philen` is left empty; it is the solvers that fill
it.

# The operators, and how they differ

| Field | Content | Shape |
|---|---|---|
| `tot` | total cross section | diagonal |
| `sd` | within-group scattering `Sigma_s(g -> g)` | diagonal |
| `fb` | bare `Sigma_f`, no `chi`, no `nu` | diagonal |
| `s` | full scattering, `g` into `gt` | off-diagonal |
| `f` | `chi * nu * Sigma_f` | off-diagonal |
| `fp` | `chi * Sigma_fp` — **no `nu` factor** | off-diagonal |

`f` and `fp` share the same sparsity pattern, since the reference builds
both from the same row/column arrays. That means `fp` inherits `f`'s
structural filter: an entry appears only where `Sigma_f` **and** `chi` are
both non-zero. A material with zero `Sigma_f` but non-zero `Sigma_fp` would
contribute nothing to `fp`. Whether that combination is physical is a
question for the case data, not for this translation.

# Reference defect — mode 2 truncates the axial extent

The three loops read

```text
for ix=m:m:m*maxix
    for iy=m:m:m*maxiy
        for iz=m:m:maxiz
```

The `iz` bound is `maxiz`, not `m*maxiz` as the other two are. At
`mode == 1` (`m == 1`) the two are the same and nothing is wrong. At
`mode == 2` (`m == 2`) the loop runs `iz = 2, 4, … maxiz`, covering only
`iz/m = 1 … maxiz/2` — **the upper half of the core silently gets no cross
sections at all**, leaving those rows of every operator empty.

This is latent: every call site in the snapshot passes `1` explicitly, so
mode 2 is never exercised. Translated as written per the no-silent-repairs
rule in the crate README, "Translation policy", and pinned by a test below.

# Reference wart — `nu` is indexed two different ways

Within the same loop body the reference reads `nu` as
`nu(material)` when filling `snu`, but as `nu(material, g)` when building
the fission operator. The first is a linear index into a 2-D array, which in
MATLAB's column-major order lands on `nu(material, 1)` — the group-1 value —
regardless of `g`.

So `sigma.nu` carries the **group-1** `nu` at every entry, while `sigma.f`
uses the true per-group `nu`. Reproduced here via
[`Array2::get_linear_column_major`].

The reference also accepts a scalar `nu` and expands it with
`nu = sigmavalues.nu * ones(G)`, giving a `G`-by-`G` matrix of that value.
Both index forms then read the same number, so the inconsistency is
invisible in that case.

# Panics

If more entries are assembled than the reference's preallocation allows,
reproducing `error('Error in makesigma.tot')` and its siblings. The limits
are `philen` for the diagonals, `philen*10` for fission and `philen*15` for
scattering — i.e. up to 10 and 15 groups respectively before the guard
trips.

```rust
pub fn makesigmadfxyz(params: &crate::types::Params, sigmavalues: &crate::types::SigmaValues, whichsigma: &crate::matlab::Array3<usize>, mode: Option<SigmaIndexMode>) -> crate::types::Sigma { /* ... */ }
```

## Module `thdiffusion_solvertimexyz`

Transient coupled neutronics / thermal-hydraulics — the time-dependent
counterpart of [`crate::thdiffusion_solverxyz`].

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `thdiffusion_solvertimexyz.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What it does, in three phases

Written for the NEACRP-L-335 rod-ejection and cold-water-injection
transients.

1. **Initial steady state.** [`crate::thdiffusion_solverxyz`] is run to
   convergence, and the transient fission operator is then divided by the
   resulting `k_eff` so the transient starts exactly critical. That stands
   in for the critical-boron search the benchmark performs to the same end.
2. **Rebuild and re-equilibrate.** The diffusion operator is reassembled at
   the steady state and the flux and eigenvalue are re-equilibrated on it
   with a power iteration, so time stepping starts from an exact
   equilibrium of the operator it will actually use — not of a slightly
   different one.
3. **Time integration** of the two-group diffusion equation with six
   delayed-neutron precursor families, the prescribed control-assembly
   motion, and one transient T-H step per time step.

# The two kinetics schemes

[`TimeScheme::ExponentialTransform`] is the default and the interesting
one: an exponential-transform implicit Euler for the flux with **analytic**
precursor integration, assuming the transformed fission source varies
linearly over the step. It is the scheme of the nodal program Ants
(A. Rintala, U. Lauranto, *Ann. Nucl. Energy* **190** (2023) 109868,
Eqs. (3)-(13)).

The frequencies are iterated **within** the step — a predictor pass at
`omega = 0`, then `freqiter - 1` correctors recomputed from the newest flux
of the current step. The reference records that extrapolating them from the
previous step instead proved unstable against the lagged T-H feedback,
producing a growing two-step power oscillation, so it is not done.

[`TimeScheme::ImplicitEuler`] is the reference's own "legacy" first-order
scheme: plain implicit Euler for both flux and precursors, with the
precursors eliminated analytically into the flux equation.

# The `omega*dt` clamp is physics, not overflow protection

The per-step exponent is clamped to `[-0.9, 2]`. The reference is explicit
that this is a **physical** bound: the upper limit keeps the transform
effective for the global mode (7.4x growth per step) while bounding
pathological extrapolation, and the lower limit keeps the transformed
time-derivative coefficient `omega + 1/dt` positive. Reproduced exactly.

# Three deliberate departures from the reference

Each follows a precedent already set elsewhere in this crate, and none
changes a number.

1. **The `.mat` steady-state cache is not translated.** The reference's
   `params.steadyfile` loads or saves a MATLAB `.mat` file around phase 1.
   That format is MATLAB's, and a library that silently reads a cache keyed
   only on a filename — which the reference's own comment warns must be
   deleted after any change to the case or params — is a correctness trap.
   The Rust signature takes `initial_steady: Option<&CoupledOutput>`
   instead, so a caller that wants the caching does it explicitly and owns
   the invalidation.
2. **The CSV and JPG writes are returned, not written.** Six `writetable` /
   `writematrix` calls and a `saveas` become fields on
   [`TransientOutput`], as the flux solvers' diagnostics already do.
3. **`th.inlettemp_t` is an enum, not a function handle.** See
   [`crate::types::InletForcing`].

# A reference quirk in the C5/C6 radial maps

The output maps are taken at "active-core axial layers 6 and 13", with
layer `L` spanning mesh layers `L*zscale + 1 ..= (L+1)*zscale`. That
indexing assumes the **PWR** model's 18 axial blocks (1 lower reflector,
2-17 active, 18 upper reflector). Case D1 has only 14 layers, so its
"layer 13" lands on the top reflector rather than inside the core.
Reproduced as written, per the no-silent-repairs policy, and pinned by a
test.

# Verification status

**The driver marches.** On [`crate::neacrpd1t`] — NEACRP case D1 cold-water
injection — it completes without tripping the divergence guard, starts at
exactly `P/P0 = 1`, and moves the power the right way: colder inlet water
means a denser moderator, more reactivity, and a rising power (+1.34% over
0.5 s) while the coolant outlet falls. Precursor concentrations stay
non-negative throughout. Measured 2026-08-18.

**The two kinetics schemes agree to 3.2e-6** on the same window. That is the
strongest evidence here: the exponential-transform and implicit-Euler paths
share the operator assembly but implement the kinetics algebra completely
separately, so their agreement tests the part of this module that is unique
to it.

**Nothing here has been compared to a published transient result.** The
NEACRP specification is not in `crates/kovan-literature`, so there is no C1
power curve to judge against, and the tests assert structure and
cross-scheme consistency only. Do not describe the transient path as
validated.

```rust
pub mod thdiffusion_solvertimexyz { /* ... */ }
```

### Modules

## Module `defaults`

The reference's transient defaults.

```rust
pub mod defaults { /* ... */ }
```

### Constants and Statics

#### Constant `PICARD`

`timepicard` — T-H feedback Picard passes per step.

```rust
pub const PICARD: usize = 1;
```

#### Constant `NODAL_UPDATE`

`nodalupdtime` — SA-nodal update interval, in steps.

```rust
pub const NODAL_UPDATE: usize = 1;
```

#### Constant `FREQ_ITER`

`freqiter` — flux solves per step: 1 predictor + `freqiter - 1`
correctors.

```rust
pub const FREQ_ITER: usize = 2;
```

#### Constant `UNIFORM_STEP`

The uniform time step used when the case supplies no `tgrid`, seconds.

```rust
pub const UNIFORM_STEP: f64 = 0.01;
```

#### Constant `REEQUILIBRATE_ITER`

Power iterations allowed in phase 2.

```rust
pub const REEQUILIBRATE_ITER: usize = 5000;
```

#### Constant `REEQUILIBRATE_TOL`

Phase-2 convergence tolerance, on both the flux and `k_eff` residuals.

```rust
pub const REEQUILIBRATE_TOL: f64 = 1e-9;
```

#### Constant `NODAL_REFINE`

Nodal-correction refinement passes at the fixed converged flux.

```rust
pub const NODAL_REFINE: usize = 4;
```

#### Constant `DIVERGENCE_CAP`

The divergence guard on `P/P0`.

Deliberately far above any physical excursion: an HZP case starting at
`P0 ~ kW` can reach `P/P0 ~ 1e6` legitimately, so the guard only trips
at `1e12`.

```rust
pub const DIVERGENCE_CAP: f64 = 1e12;
```

#### Constant `OMEGA_DT_MIN`

Lower clamp on the per-step exponent `omega*dt`.

```rust
pub const OMEGA_DT_MIN: f64 = -0.9;
```

#### Constant `OMEGA_DT_MAX`

Upper clamp on the per-step exponent `omega*dt`.

```rust
pub const OMEGA_DT_MAX: f64 = 2.0;
```

### Types

#### Enum `Termination`

Why the time integration stopped.

```rust
pub enum Termination {
    Completed,
    Diverged,
}
```

##### Variants

###### `Completed`

The whole grid was marched.

###### `Diverged`

The divergence guard tripped; the histories are truncated at that step.

The reference raises a warning and `break`s, then trims every history
vector to length `n`. Same here — the truncation is real, not cosmetic,
so a caller reading `time.len()` sees where it actually stopped.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Termination { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `TransientOutput`

The NEACRP-L-335 section 4 C transient results, plus what the reference
writes to disk.

```rust
pub struct TransientOutput {
    pub k_eff: f64,
    pub steady: crate::thdiffusion_solverxyz::CoupledOutput,
    pub th: crate::types::Th,
    pub time: Vec<f64>,
    pub relpower: Vec<f64>,
    pub avgfueltemp: Vec<f64>,
    pub maxfueltemp: Vec<f64>,
    pub coolouttemp: Vec<f64>,
    pub rodpos: Vec<f64>,
    pub rad_c5_z6: crate::matlab::Array2<f64>,
    pub rad_c5_z13: crate::matlab::Array2<f64>,
    pub rad_c6_z6: crate::matlab::Array2<f64>,
    pub rad_c6_z13: crate::matlab::Array2<f64>,
    pub tpmax: f64,
    pub prelmax: f64,
    pub scalar_flux_final: Vec<f64>,
    pub pwrdens_final: Vec<f64>,
    pub precursors_final: crate::matlab::Array2<f64>,
    pub timescheme: crate::types::TimeScheme,
    pub termination: Termination,
    pub reequilibrate_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | `output.k_eff` — the re-equilibrated initial eigenvalue from phase 2. |
| `steady` | `crate::thdiffusion_solverxyz::CoupledOutput` | The converged steady state phase 1 produced. |
| `th` | `crate::types::Th` | `output.th` — the final transient T-H state. |
| `time` | `Vec<f64>` | `output.time` — the time grid actually marched, seconds. |
| `relpower` | `Vec<f64>` | **C1** — core power relative to its steady value. |
| `avgfueltemp` | `Vec<f64>` | **C2** — core-averaged fuel temperature, K. |
| `maxfueltemp` | `Vec<f64>` | **C3** — maximum fuel temperature, K. |
| `coolouttemp` | `Vec<f64>` | **C4** — core-averaged coolant outlet temperature, K. |
| `rodpos` | `Vec<f64>` | The ejected bank's position at each step, in steps. |
| `rad_c5_z6` | `crate::matlab::Array2<f64>` | **C5-1** — radial power map at active layer 6, at the power maximum,<br>normalised to a peak of 1. |
| `rad_c5_z13` | `crate::matlab::Array2<f64>` | **C5-2** — the same at active layer 13. |
| `rad_c6_z6` | `crate::matlab::Array2<f64>` | **C6-1** — radial power map at active layer 6, at `t = tend`. |
| `rad_c6_z13` | `crate::matlab::Array2<f64>` | **C6-2** — the same at active layer 13. |
| `tpmax` | `f64` | When the power maximum occurred, seconds. |
| `prelmax` | `f64` | The peak `P/P0`. |
| `scalar_flux_final` | `Vec<f64>` | The flux at the final time. |
| `pwrdens_final` | `Vec<f64>` | Group-collapsed node power at the final time. |
| `precursors_final` | `crate::matlab::Array2<f64>` | Precursor concentrations at the final time, `philenf` by families. |
| `timescheme` | `crate::types::TimeScheme` | Which scheme ran. |
| `termination` | `Termination` | Why it stopped. |
| `reequilibrate_iterations` | `usize` | How many power iterations phase 2 needed. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TransientOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `build_time_grid_for_test`

Build the time grid the reference marches.

`[0, tgrid..., tend]`, then **rounded to 1 microsecond and deduplicated**,
which is how the reference stops overlapping range endpoints (its cases
write grids like `[0:0.025:2, 2:0.05:6, ...]`, repeating every join) from
producing a near-zero time step. Finally anything past `tend` is dropped.
`build_time_grid` exposed for the case modules' tests.

The grid construction is the only part of this driver a case can get wrong
on its own — an overlapping range that survives deduplication would be a
zero-length time step — so it is worth testing from the case side.

```rust
pub fn build_time_grid_for_test(params: &crate::types::Params, tend: f64) -> Vec<f64> { /* ... */ }
```

#### Function `thdiffusion_solvertimexyz`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`output = thdiffusion_solvertimexyz(geometry, params, th, sigmavalues, whichsigma, varargin)`.

# Arguments

- `feedback` — the cross-section slope tables, as the steady driver takes.
- `initial_steady` — a precomputed phase-1 result to reuse. `None` runs
  phase 1. This replaces the reference's `params.steadyfile` `.mat` cache;
  see the module docs.
- `initial_k_eff` — passed through to phase 1.

# Errors

Propagates whatever the steady solver and the operator chain raise.

# Panics

If the case supplies neither `params.tend` nor `params.tgrid` — the
reference raises `thdiffusion_solvertimexyz:notimedata` here — or if the
kinetics data (`velocities`, `beta_dnp`, `lambda_dnp`) is missing or
inconsistent.

```rust
pub fn thdiffusion_solvertimexyz(geometry: &crate::types::Geometry, params: &crate::types::Params, th: &crate::types::Th, sigmavaluesref: &crate::types::SigmaValues, feedback: &crate::sigmavalupd3d_handler::FeedbackTables, whichsigmaref: &crate::matlab::Array3<usize>, initial_steady: Option<&crate::thdiffusion_solverxyz::CoupledOutput>, initial_k_eff: Option<f64>) -> crate::error::Result<TransientOutput> { /* ... */ }
```

## Module `neacrpa1t`

NEACRP case A1 — central control-assembly ejection at **hot zero power**.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `neacrpa1t.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# The transient

NEACRP-L-335 (Revision 1), Figure 3.1. Same core as [`crate::neacrpa2`], but
at **hot zero power**: a 2775 W core (693.75 W in the modelled quarter) with
the coolant at 286 C. The central control assembly is ejected from **fully
inserted** to fully withdrawn in 0.1 s, and the transient is followed for
5 s.

# Why this is the harder of the two ejections

At full power (case A2) the ejected rod is worth roughly a dollar and the
Doppler feedback of an already-hot core damps the excursion. At HZP the fuel
starts in equilibrium with the coolant — `fueltempavg` is the coolant
temperature, not 891 K — so there is **no stored Doppler margin**, and the
rod is being pulled from full insertion rather than half. The reference's
own note records the consequence:

> the time grid uses 1 ms steps over the super-prompt-critical power spike
> (~0.1-0.5 s); the spike spans **several decades of power**.

That is why the grid is 3.5x denser than A2's, and it
is the regime [`crate::thdiffusion_solvertimexyz`]'s `freqmode` note is
about: per-node exponential-transform frequencies are unstable in
super-prompt ejections, so [`crate::types::FreqMode::Global`] is the default.

# The rod pattern is nearly all-in

Figure 3.1: banks 1, 2, 3, 5, 6 and 7 **fully inserted** (0 steps), bank 4
fully withdrawn (228). Case A2's pattern is a partial insertion by
comparison. This is the configuration the reference describes as
"heavily-rodded", and the one
[`crate::criticalboron_xyz`]'s cold-start warnings were written against.

# A second published number, and a second disagreement

| | ppm |
|---|---|
| this code (frozen-T-H secant + coupled verification) | 551.31 |
| benchmark (PANTHER, NEA/NSC/DOC(93)25 Tab 3.1) | **567.7** |

**-16.39 ppm, about -2.9%** — the same *direction* as case A2's -21.6 ppm,
and a similar relative size. Two independent cases disagreeing the same way
is more informative than either alone, and it is recorded here for that
reason.

As with A2 the search that produced 551.31 is not reproducible from this
snapshot: the comment cites `test_critboron2.m`, which was not shipped.

The reference also notes what happens if the boron is left at A2's value:

> At e.g. 1000 ppm the core is ~4200 pcm subcritical and the ejected rod is
> no longer ~1$ (sub-prompt transient).

So the boron here is not a tuning knob — get it wrong and the case stops
being the transient the benchmark specifies.

```rust
pub mod neacrpa1t { /* ... */ }
```

### Functions

#### Function `neacrpa1t`

**Attributes:**

- `Other("#[allow(clippy::type_complexity)]")`

`[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa1t(params)`.

Builds [`crate::neacrpa2t`] — which shares the kinetics data, heat
capacities and ejection duration — and overrides the five things case A1
changes: the time grid, the power ratio, the boron, the initial fuel
temperature and the rod pattern.

The reference's own header says it is "based on `neacrpa2t.m`", and diffing
the two confirms those five are the only substantive differences.

# Returns

`(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
[`crate::neacrpa2::neacrpa2`].

```rust
pub fn neacrpa1t(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::types::Th, crate::matlab::Array3<usize>, crate::types::SigmaValues, crate::sigmavalupd3d_handler::FeedbackTables) { /* ... */ }
```

### Constants and Statics

#### Constant `CRITICAL_BORON`

The critical boron concentration this code computes for case A1, ppm.

From `neacrpa1t.m`: a frozen-T-H secant plus coupled verification giving
`k_eff = 0.999990`. Compare [`BENCHMARK_CRITICAL_BORON`].

```rust
pub const CRITICAL_BORON: f64 = 551.31;
```

#### Constant `BENCHMARK_CRITICAL_BORON`

The **published** critical boron concentration for case A1, ppm.

PANTHER, NEA/NSC/DOC(93)25 Table 3.1, as quoted by `neacrpa1t.m`'s comment.
**Quoted from that comment, not from a primary publication checked here.**

```rust
pub const BENCHMARK_CRITICAL_BORON: f64 = 567.7;
```

#### Constant `HZP_POWER_RATIO`

The HZP power ratio: 2775 W core, 693.75 W in the modelled quarter.

Applied to [`crate::neacrpa2`]'s 693.75 MW, so `1e-6` gives 693.75 W.

```rust
pub const HZP_POWER_RATIO: f64 = 1e-6;
```

#### Constant `HZP_FUEL_TEMP`

The HZP fuel temperature, K — in equilibrium with the coolant.

```rust
pub const HZP_FUEL_TEMP: f64 = 559.15;
```

## Module `neacrpa2`

The NEACRP 3-D LWR core transient benchmark, PWR case A2 — steady state.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `neacrpa2.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.
- **Composition maps:** `src/data/NEACRPA2_*.csv`; see
  `src/data/PROVENANCE.md`.

# Why this case matters

It is the most complete case in the snapshot, and the one the transient
driver was written for. Two things appear here for the first time:

- **All five feedback channels at once** — boron, fuel temperature, coolant
  temperature, coolant density and control rods. [`crate::neacrpd1`] runs
  only two, so the boron, coolant-temperature and rod channels of
  [`crate::sigmavalupd3d_handler`] have never been exercised by a real case
  before this one.
- **A real control-rod bank pattern** — seven banks on a 17x17 map, with
  partial insertions (`crod = [100, 200, 100, 200, 200, 200, 200]` steps).
  This is what the rod-ejection transient `neacrpa2t` moves.

# The problem

A 17 x 17 x 18 core octant with rotational symmetry, 10.803 cm radial pitch,
reflective on the low `x` and `y` faces and zero flux elsewhere. Two energy
groups, **11 materials** (axial and radial reflectors, a re-entrant corner,
and eight fuel compositions from 2.1 to 3.1 w/o with burnable absorbers).

# WARNING — the axial mesh is non-uniform, and that hits defect G1

The axial layer heights are

```text
30, 7.7, 11, 15, 30 x10, 12.8, 12.8, 8, 30   cm
```

and [`crate::makegrad_dxyz`]'s face coupling is **only a consistent
discretisation on a uniform mesh** — defect **G1** in
`docs/bedok-reference-defects.md`, confirmed by measurement: a 2:1 cell-size
jump understates the face coupling by 25%, and the worst joint here is close
to 4:1 (30 cm against 7.7 cm).

This is **not** repaired, per the no-silent-repairs policy — repairing it
would move every NEACRP number and is a *correction*, which cannot be gated
on parity with the reference. What it means in practice:

- Results from this case carry a discretisation error at the axial layer
  joints that does not vanish under refinement unless the mesh is also made
  uniform.
- The reference always solves it with [`crate::sanodaldiffusion_solverxyz`],
  whose nodal correction is refitted against the same operator and appears
  to absorb much of it. **Do not solve this case with the bare
  finite-difference solver** and expect a sensible axial power shape.

# The cross sections are given as total and absorption

As in [`crate::neacrpd1`]: the case supplies total, absorption and the
down-scatter, and closes the within-group scattering by difference. `nu` is
all ones, so `sigmavalues.f` already carries `nu*Sigma_f`. Unlike case D1,
this case **does** populate `fp` directly.

# Transcription

The five feedback channels are 24 tables of 11 materials by 2 groups, plus
six down-scatter columns — around 450 numbers. They were **extracted
mechanically** from `neacrpa2.m` rather than retyped, and every distinct
numeric literal below was checked to appear verbatim in the source.

```rust
pub mod neacrpa2 { /* ... */ }
```

### Functions

#### Function `neacrpa2`

**Attributes:**

- `Other("#[allow(clippy::type_complexity)]")`

`[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa2(params)`.

Builds the complete NEACRP case-A2 steady state: the graded axial mesh, the
11-material two-group cross-section set, the three-layer material map, the
seven control-rod banks, the thermal-hydraulic inlet state and rod geometry,
and **all five** feedback tables.

# Returns

`(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
[`crate::neacrpd1::neacrpd1`].

# The mesh is fixed at 17 x 17 x 18

The reference computes `xscale`/`yscale`/`zscale` and indexes the maps with
`ceil(ix/maxix*17)`, an identity only at 17; the axial layer assignment is
likewise written in multiples of `zscale`. This translation fixes the mesh
at the benchmark's own 17 x 17 x 18 and asserts it.

# Panics

If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
17, 17, 18.

```rust
pub fn neacrpa2(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::types::Th, crate::matlab::Array3<usize>, crate::types::SigmaValues, crate::sigmavalupd3d_handler::FeedbackTables) { /* ... */ }
```

### Constants and Statics

#### Constant `MATERIALS`

The number of materials in the case's cross-section set.

```rust
pub const MATERIALS: usize = 11;
```

#### Constant `Z_LENGTHS`

The axial layer heights, cm — **non-uniform**; see the module warning.

```rust
pub const Z_LENGTHS: [f64; 18] = _;
```

## Module `neacrpa2t`

NEACRP case A2 — the central control-assembly ejection transient.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `neacrpa2t.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# The transient

NEACRP-L-335 (Revision 1), Figure 3.2 / section 3.2. The **central control
assembly (bank 1)** is withdrawn from 100 steps to 228 (fully out) in
**0.1 s** at full power, and the transient is followed for 5 s.

This is the case [`crate::thdiffusion_solvertimexyz`] was written for, and
the first in this crate to exercise its rod-ejection path at all —
[`crate::neacrpd1t`] has no rod motion. A super-prompt ejection is also the
regime the driver's `freqmode` note warns about: per-node frequencies are
unstable there, which is why `Global` is the default.

# This case duplicates the steady case rather than calling it

`neacrpa2t.m` is a **verbatim copy** of `neacrpa2.m` with the transient data
appended, where `neacrpd1t.m` calls `neacrpd1.m` directly. Diffing the two
shows the steady halves are byte-identical apart from **one line** — the
boron concentration.

This translation therefore calls [`crate::neacrpa2::neacrpa2`] and overrides
that one value, rather than duplicating ~450 numbers a second time. The
equivalence is **tested**, not assumed: if a future snapshot lets the two
copies drift, that test fails and this module must be split out again. Same
reasoning, and the same safeguard, as the shared enthalpy inversion between
the two flow solvers.

# The boron concentration is this code's own critical value

The steady case runs at 1000 ppm; this one raises it to
[`CRITICAL_BORON`] = 1139.01 ppm, which the reference's comment identifies
as **the critical boron concentration calculated for this code** — a
warm-started coupled search giving `k_eff = 1.000005`.

It also records the **official benchmark value**, and the two do not agree:

| | ppm |
|---|---|
| this code (coupled search) | 1139.01 |
| benchmark reference (PANTHER, NEA/NSC/DOC(93)25 Tab 3.1) | **1160.6** |

a difference of **-21.6 ppm, about -1.9%**. The reference's own note adds
that "the solver's `1/keff` scaling absorbs any small residual" — meaning the
transient starts exactly critical either way, so the discrepancy does not
propagate into the transient as a reactivity step. It is nonetheless a real
disagreement with the published benchmark, at the steady state, and it is
recorded here because it is the **only published NEACRP number anywhere in
the snapshot**.

**The search that produced 1139.01 cannot be reproduced from this snapshot:**
the comment cites `test_critboron3.m`, which was not shipped. See
`docs/bedok-reference-defects.md`, "Missing files".

```rust
pub mod neacrpa2t { /* ... */ }
```

### Functions

#### Function `neacrpa2t`

**Attributes:**

- `Other("#[allow(clippy::type_complexity)]")`

`[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa2t(params)`.

Builds [`crate::neacrpa2`], raises the boron to its critical value, and adds
the transient data: the time window and grid, two-group prompt velocities,
six-group delayed-neutron constants, fuel and cladding volumetric heat
capacities, and the bank-1 ejection scenario.

# Returns

`(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
[`crate::neacrpa2::neacrpa2`].

```rust
pub fn neacrpa2t(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::types::Th, crate::matlab::Array3<usize>, crate::types::SigmaValues, crate::sigmavalupd3d_handler::FeedbackTables) { /* ... */ }
```

### Constants and Statics

#### Constant `CRITICAL_BORON`

The critical boron concentration this code computes for case A2, ppm.

From `neacrpa2t.m`: a warm-started coupled search giving `k_eff = 1.000005`.
Compare [`BENCHMARK_CRITICAL_BORON`].

```rust
pub const CRITICAL_BORON: f64 = 1139.01;
```

#### Constant `BENCHMARK_CRITICAL_BORON`

The **published** critical boron concentration for case A2, ppm.

PANTHER, NEA/NSC/DOC(93)25 Table 3.1, as quoted by `neacrpa2t.m`'s comment.
**Quoted from that comment, not from a primary publication checked here** —
the specification is not in `crates/kovan-literature`. See
`src/data/PROVENANCE.md` before citing it.

```rust
pub const BENCHMARK_CRITICAL_BORON: f64 = 1160.6;
```

#### Constant `EJECT_TO`

The ejected bank's final position, in steps (228 = fully withdrawn).

```rust
pub const EJECT_TO: f64 = 228.0;
```

#### Constant `EJECT_DURATION`

The ejection time, seconds — independent of insertion depth.

```rust
pub const EJECT_DURATION: f64 = 0.1;
```

## Module `neacrpd1`

The NEACRP 3-D LWR core transient benchmark, BWR case D — steady state.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `neacrpd1.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.
- **Composition maps:** `src/data/NEACRPD1_*.csv`; see
  `src/data/PROVENANCE.md`.

# Why this case matters

[`crate::iaea3ds`] is pure neutronics; this is the first **coupled** case in
the crate. It carries everything the thermal-hydraulic side needs — core
power, coolant inlet state, mass flux, a 22-node fuel rod with UO2/gap/clad
materials, and per-material cross-section slopes against both fuel
temperature and coolant density — so it is what
[`crate::thdiffusion_solverxyz`] was written to consume.

# The problem

A 17 x 17 x 14 quarter core on a 30.48/2 cm radial by 30.48 cm axial mesh —
259.08 x 259.08 x 426.72 cm. Reflective on the low `x` and `y` faces (the
quarter-core symmetry planes), zero flux on the other four. Two energy
groups, **19 materials**, fission into the fast group only.

The material map is built from two files rather than one per level: a 17x17
*column* map naming which of 10 radial column types each lattice position
is, and a 14x10 *axial* table giving the material of each column type at
each level. A column entry of `0` is outside the core outline.

# The cross sections are given as total and absorption

The case supplies `sigmavalues.tot`, `sigmavalues.a` and the off-diagonal
scattering, then closes the within-group scattering by difference:

```text
s(m, 1, 1) = tot(m, 1) - a(m, 1) - s(m, 2, 1)
s(m, 2, 2) = tot(m, 2) - a(m, 2) - s(m, 1, 2)
```

That identity is what makes the set consistent, and it is checked by a test
rather than assumed. Absorption is not carried on [`SigmaValues`] because
nothing downstream reads it — it exists in the case file only to close the
scattering. As with `iaea3ds`, `nu` is all ones and `sigmavalues.f` already
carries the `nu * Sigma_f` product.

# `sigmavalues.*.upd` is dead data in the reference

The case file builds a per-node mask marking which nodes have feedback
applied — non-zero wherever the material fissions. **Nothing reads it.** A
search of the whole snapshot finds `.upd` written by the case files and
consumed nowhere, so it is not carried here. The feedback handler applies
its slopes to every material row, which is what the reference actually does.

# Two values are written twice; the second wins

`th.flowrate` is assigned three times in the reference, the first two
commented derivations and the third live. Only the last takes effect and
only the last is translated; the other two are recorded in the constant's
docs so the intent is not lost.

```rust
pub mod neacrpd1 { /* ... */ }
```

### Functions

#### Function `neacrpd1`

**Attributes:**

- `Other("#[allow(clippy::type_complexity)]")`

`[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpd1(params)`.

Builds the complete NEACRP case-D steady state: mesh, boundary conditions,
the 19-material two-group cross-section set, the material map, the
thermal-hydraulic inlet state and rod geometry, and the fuel-temperature and
coolant-density feedback tables.

# Returns

`(params, geometry, th, whichsigma, sigmavalues, feedback)`. The reference's
`constants` output carries only `chi` and `nu`, both already on
`sigmavalues`, and its feedback slopes ride on `sigmavalues.fueltemp` /
`sigmavalues.coolden` where this crate keeps them in a separate
[`FeedbackTables`].

# The mesh is fixed at 17 x 17 x 14

The reference computes `xscale = maxix/17` and friends and divides
`th.nfuelpin` by them, so a refined mesh is nominally allowed. But the
material lookup is `whichdata(ceil(ix/maxix*17), ceil(iy/maxiy*17))`, an
identity only at 17, and `geometry.Lz` is written with a stride of `maxiz`
that assumes the benchmark's own layer count. This translation fixes the
mesh at 17 x 17 x 14 and asserts it rather than reproducing a refinement
path the reference never exercises.

# Panics

If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
17, 17, 14.

```rust
pub fn neacrpd1(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::types::Th, crate::matlab::Array3<usize>, crate::types::SigmaValues, crate::sigmavalupd3d_handler::FeedbackTables) { /* ... */ }
```

### Constants and Statics

#### Constant `MATERIALS`

The number of materials in the case's cross-section set.

```rust
pub const MATERIALS: usize = 19;
```

## Module `neacrpd1t`

NEACRP case D1 — the inlet cold-water injection transient.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `neacrpd1t.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# The transient

NEACRP-L-335 (Revision 1) section 6.2 / Fig. 6.1, over 0 to 20 s. The steady
state is [`crate::neacrpd1`] unchanged; this file adds only the
time-dependent data.

The inlet subcooling **doubles** with a 2.5 s time constant:

```text
dh(t) = 46.52 * (2 - exp(-0.4 t))   kJ/kg below saturated liquid
```

`dh(0) = 46.52 kJ/kg` is exactly the steady inlet of `neacrpd1.m`, so the
forcing is continuous at `t = 0`. The inlet mass flow is **constant** and
there is **no control-rod motion** — `crodeject` stays `None`.

# Why the case forces the HEM thermal-hydraulic model

The transient chain ([`crate::th_solvertimexyz`] into
[`crate::singleflow1devaptime`]) is the homogeneous-equilibrium enthalpy
march, so the **initial steady state must run the same model**. A two-fluid
steady state has less void than HEM at the same conditions, and handing that
to the transient would be a density mismatch — a spurious reactivity step at
`t = 0`. The case therefore sets `th_model = 'hem'` explicitly.

That is also the only model this crate can run: the two-fluid path needs
`driftflux6_solverstatic1d.m`, which is absent from the snapshot.

# The case has to rebuild `fp`, because the steady case zeroes it

`neacrpd1.m` leaves `sigmavalues.fp` at zero — the steady solver derives
power from the fission source and never reads it. The transient does read
it: `P0 = sum(fp * phi)` would be `0/0 = NaN`. So the case builds it from
the `nu*Sigma_f` tables using the specification's prompt energy release
`E0 = 3.20e-11 J/fission` (Table 5.1):

```text
fp = E0 * (nu Sigma_f) / nu,   with nu = 1 as encoded
```

Under composition-uniform `nu` the `P/P0` **ratio** is exact, because the
`E0` scale cancels. The feedback slopes follow `f`'s, so `fp` stays
proportional to `f` under both feedback channels.

```rust
pub mod neacrpd1t { /* ... */ }
```

### Functions

#### Function `neacrpd1t`

**Attributes:**

- `Other("#[allow(clippy::type_complexity)]")`

`[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpd1t(params)`.

Builds [`crate::neacrpd1`] and layers the transient data on top: the time
window and grid, the two-group prompt velocities, six-group delayed-neutron
data, fuel and cladding volumetric heat capacities, the inlet forcing, and
the reconstructed prompt-fission operator.

# Returns

`(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
[`crate::neacrpd1::neacrpd1`].

```rust
pub fn neacrpd1t(params: &crate::types::Params) -> (crate::types::Params, crate::types::Geometry, crate::types::Th, crate::matlab::Array3<usize>, crate::types::SigmaValues, crate::sigmavalupd3d_handler::FeedbackTables) { /* ... */ }
```

### Constants and Statics

#### Constant `ENERGY_PER_FISSION`

The specification's prompt energy release per fission, J.

NEACRP-L-335 Table 5.1. Only the `P/P0` ratio is reported, and this scale
cancels out of it.

```rust
pub const ENERGY_PER_FISSION: f64 = 3.20e-11;
```

#### Constant `SUBCOOLING_0`

The steady-state inlet subcooling, kJ/kg — Fig. 6.1's `dh(0)`.

```rust
pub const SUBCOOLING_0: f64 = 46.52;
```

#### Constant `FORCING_RATE`

The approach rate of the cold-water forcing, 1/s (a 2.5 s time constant).

```rust
pub const FORCING_RATE: f64 = 0.4;
```

## Module `pauseonnan`

Abort the run if a vector has gone non-finite or complex.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `pauseonnan.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod pauseonnan { /* ... */ }
```

### Functions

#### Function `pauseonnan`

`pauseonnan(input)`.

A debugging guard the solvers call at points where a diverging iterate would
otherwise be carried silently into the next sweep. Raises on the first `NaN`
it finds; the reference also rejects complex input.

# Arguments

- `input` — the values to check, in any units.

# Errors

- [`BedokError::NanEncountered`] if any entry is `NaN`.

# What is checked, and what is not

The reference tests `any(isnan(input))`, which catches `NaN` but **not**
`Inf` — an infinite iterate passes this guard. That is preserved; use
`fixinfnan` for the non-finite case, as the reference does.

The `~isreal(input)` test cannot be reached here: the translation carries
`f64` throughout, so there is no complex value to detect.
[`BedokError::UnexpectedComplex`] exists to record that the reference makes
the check, and would become live if a complex path is ever introduced.

# Printing

Before erroring, the reference echoes the offending vector to the console
(the bare `input` on its own line). That is reproduced on stderr, since its
purpose — showing the user *what* went non-finite — is part of the
behaviour rather than incidental.

```rust
pub fn pauseonnan(input: &[f64]) -> crate::error::Result<()> { /* ... */ }
```

## Module `plotreactor3dcolour`

The scaled power map behind the 3-D power-density plot.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `plotreactor3dcolour.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What is and is not translated

The reference does two things: it **computes a scaled power map**, then it
**renders that map as a MATLAB figure** — building `fill3` patch vertices for
each node, mirroring them into four quadrants, attaching a colour bar and
writing `pwrdens3d.jpg`.

Only the first half is translated. The rendering half emits a MATLAB figure
and has no library equivalent; reproducing it would mean choosing a plotting
stack and writing files as a side effect, which this crate deliberately does
not do (see the flux solvers' `Diagnostics`, and the CSV policy in the crate
README). [`scaled_power`] returns the same quantity the figure colours by, so
a caller can render it however they like.

**What that omits, concretely:** the quadrant mirroring, the patch geometry,
and the `PWRlin` 256-step colour scale. None of it changes a number; all of
it is presentation.

# Two defects in the half that IS translated

Both are pinned by tests and neither is repaired.

**P1 — a one-group case gets an all-zero map.** The group collapse sits
entirely inside `if params.G > 1`, and `pwrdensG` is preallocated to zeros.
At `G == 1` nothing ever writes it, so every node plots as zero power. The
reference's only call site passes `G = 2`, so it has never been seen.

**P2 — for more than two groups the collapse overwrites instead of
accumulating.** The loop body is

```text
pwrdensG(1:es) = pwrdens(1:es) + pwrdens((g-1)*es+1 : g*es)
```

which **assigns** rather than adding into `pwrdensG`. After the loop only
group 1 plus the *last* group survive; groups 2 to `G-1` are silently
dropped. Correct at `G == 2` — the value every case in the snapshot uses —
and wrong for anything larger.

# The normalisation divides by the ungrouped total

`scaledpwr = pwrdensG / sum(pwrdens) ./ Vi`. Note the denominator is the sum
over the **whole** `pwrdens` vector, all groups, while the numerator is the
collapsed map. That is not a defect — it makes the map a fraction of total
core power per unit volume — but it does mean the map does not sum to 1.

```rust
pub mod plotreactor3dcolour { /* ... */ }
```

### Types

#### Struct `ScaledPower`

The scaled power map, and what the collapse did to get there.

```rust
pub struct ScaledPower {
    pub scaled: Vec<f64>,
    pub collapsed: Vec<f64>,
    pub peak: f64,
    pub all_zero_from_single_group: bool,
    pub groups_dropped: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scaled` | `Vec<f64>` | `scaledpwr` — power per unit volume as a fraction of the core total,<br>one entry per node. |
| `collapsed` | `Vec<f64>` | `pwrdensG` — the group-collapsed node power, before dividing by volume. |
| `peak` | `f64` | `PWRHIGH` — the map's maximum, which sets the colour scale. |
| `all_zero_from_single_group` | `bool` | Whether defect P1 fired: a one-group case, so `collapsed` is all zeros. |
| `groups_dropped` | `usize` | How many groups defect P2 silently dropped.<br><br>`0` for `G <= 2`; `G - 2` for anything larger, because only group 1 and<br>the last group survive the overwriting loop. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScaledPower { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `scaled_power`

The scaled power map `plotreactor3dcolour.m` colours its figure by.

# Arguments

- `pwrdens` — `results.pwrdens`, length `G * es`.
- `geometry` — needs `Vi`.

# Panics

If `pwrdens` is shorter than `G * es`.

```rust
pub fn scaled_power(params: &crate::types::Params, geometry: &crate::types::Geometry, pwrdens: &[f64]) -> ScaledPower { /* ... */ }
```

## Module `sanodaldiffusion_solverxyz`

Semi-analytic nodal diffusion — the solver the benchmark drivers call.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `sanodaldiffusion_solverxyz.m`,
  `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this adds over the finite-difference solver

[`crate::diffusion_solverxyz`] solves `gradD + sigma.tot - sigma.sd` by
source iteration. This one adds the SANM correction operator from
[`crate::calc_sanodalxyz`] and folds the whole scattering operator into the
left-hand side, so a pass is a single solve against
`gradD + nodal + sigma.tot - sigma.s` with a pure fission right-hand side.
On top of that it carries three things its plainer sibling does not:

- a **periodic nodal update**, re-running the expansion against the current
  flux every `nodalupd` iterations and refactorising;
- **fission-source extrapolation** every `fsexp` iterations
  ([`crate::fiss_src_extrapolatexyz`]), which is why the flux is kept as a
  five-generation history rather than a single vector;
- a **warm start**, so the coupled neutronics/T-H outer loop can reseed the
  iteration with the previous outer pass's flux.

This module is the last of the fourteen SANM files.

# Reference defects carried here

Three entries of `docs/bedok-reference-defects.md` are about this file, and
all three are reproduced rather than fixed:

- **N1** — `nodalupd == 1` destabilises the solver, and the built-in default
  `ceil((maxix+maxiy+maxiz)/10)` **is** 1 for any mesh whose extents sum to
  10 or fewer. See [`sanodaldiffusion_solverxyz`].
- **N10** — the normalisation comment is wrong, and the norms are
  inconsistent with [`crate::diffusion_solverxyz`]'s.
- **N2** — `Nc > 0` cannot conform.

Reading this file fresh added three more, recorded as D4-D6 in that
register: the dead Wielandt scaffolding, the mismatched normalisation pair
on an early break, and the lagged output state shared with
[`crate::diffusion_solverxyz`].

```rust
pub mod sanodaldiffusion_solverxyz { /* ... */ }
```

### Types

#### Enum `Termination`

Why the source iteration stopped. As [`crate::diffusion_solverxyz`]'s.

```rust
pub enum Termination {
    Converged,
    NonPositiveKeff,
    NanKeff,
    IterationCap,
}
```

##### Variants

###### `Converged`

Both residuals fell below the tolerance.

###### `NonPositiveKeff`

`k_eff <= 0`.

###### `NanKeff`

`k_eff` became `NaN`.

###### `IterationCap`

The iteration count passed [`MAX_ITER`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Termination { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Diagnostics`

The `params.debugdump` diagnostic maps.

# Why these are returned rather than written

The reference writes ten `writematrix` CSVs when `params.debugdump == 1` —
the antisymmetric part of six operator diagonals and their off-diagonal
column masses, plus `rel_power_inner.csv`, `scalar_flux.csv`,
`fission_source.csv` and `pwrdenss.csv`. Writing files as a side effect of
a library call is not reproduced here for the reasons given on
[`crate::diffusion_solverxyz::Diagnostics`]; the quantities are computed
exactly as the reference computes them and handed back.

Unlike its sibling, this solver **does** gate them on `params.debugdump`, so
they are `None` unless it is set — the computation is skipped entirely, as
in the reference.

Each map is `maxix` by `maxiy` and dimensionless. `diag` is the
antisymmetric part of the collapsed diagonal; `offdiag` the same for
`sum(m) - diag(m)`, the off-diagonal column mass.

```rust
pub struct Diagnostics {
    pub sigmaf: (crate::matlab::Array2<f64>, crate::matlab::Array2<f64>),
    pub sigmas: (crate::matlab::Array2<f64>, crate::matlab::Array2<f64>),
    pub sigmatot: (crate::matlab::Array2<f64>, crate::matlab::Array2<f64>),
    pub nodal: (crate::matlab::Array2<f64>, crate::matlab::Array2<f64>),
    pub gradd: (crate::matlab::Array2<f64>, crate::matlab::Array2<f64>),
    pub rel_power: crate::matlab::Array2<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sigmaf` | `(crate::matlab::Array2<f64>, crate::matlab::Array2<f64>)` | `sigmafxy.csv` and `sigmafxyoff.csv`. |
| `sigmas` | `(crate::matlab::Array2<f64>, crate::matlab::Array2<f64>)` | `sigmasxy.csv` and `sigmasxyoff.csv`. |
| `sigmatot` | `(crate::matlab::Array2<f64>, crate::matlab::Array2<f64>)` | `sigmatxy.csv` and `sigmatxyoff.csv`. |
| `nodal` | `(crate::matlab::Array2<f64>, crate::matlab::Array2<f64>)` | `nodalxy.csv` and `nodalxyoff.csv` — from the **initial** nodal<br>operator, built before the iteration starts. |
| `gradd` | `(crate::matlab::Array2<f64>, crate::matlab::Array2<f64>)` | `gradDxy.csv` and `gradDxyoff.csv`. |
| `rel_power` | `crate::matlab::Array2<f64>` | `rel_power_inner.csv` — the normalised assembly power map. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Diagnostics { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Diagnostics { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SaNodalOutput`

`output` — what the reference returns, plus the provenance it does not.

Deliberately **not** `Default`, as [`crate::diffusion_solverxyz::DiffusionOutput`].

```rust
pub struct SaNodalOutput {
    pub k_eff: f64,
    pub residual: f64,
    pub k_eff_residual: f64,
    pub scalar_flux: crate::matlab::Array2<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub phi_plot: crate::matlab::Array2<f64>,
    pub iterations: usize,
    pub nodal_updates: usize,
    pub termination: Termination,
    pub diagnostics: Option<Diagnostics>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | `output.k_eff` — the multiplication factor, dimensionless. |
| `residual` | `f64` | `output.residual` — the relative fission-source change, dimensionless. |
| `k_eff_residual` | `f64` | `output.k_eff_residual` — the relative `k_eff` change, dimensionless. |
| `scalar_flux` | `crate::matlab::Array2<f64>` | `output.scalar_flux` — the **whole five-generation history**, `philenf`<br>rows by [`HISTORY`] columns, column 0 newest.<br><br>The reference returns the matrix, not a vector, and that matters: this<br>is exactly what the warm-start argument expects back, so a coupled outer<br>loop can feed one call's output straight into the next call's<br>`initflux`. |
| `fission_source` | `Vec<f64>` | `output.fission_source` — `philenf` long. |
| `pwrdens` | `Vec<f64>` | `output.pwrdens` — `fission_source .* Vi`. |
| `phi_plot` | `crate::matlab::Array2<f64>` | `phi_plot` — the group-summed flux on the `zplot = 1` plane, `maxix` by<br>`maxiy`. Computed unconditionally by the reference and only used to draw<br>`figure(6)`; returned rather than plotted, so `params.plotfig` is not<br>read here.<br><br># It reads the newest generation, by accident rather than by choice<br><br>The reference indexes `output.scalar_flux(...)` with a **single** linear<br>index, on a value that is a `philenf`-by-5 matrix rather than a vector.<br>MATLAB's column-major linear indexing therefore lands the whole<br>calculation in **column 1** — the newest generation — because every<br>index it forms is at most `philenf`. That is the intended plane, so the<br>result is right; it is right for a reason the code does not state.<br><br>This is the same trap [`crate::matlab::Array2::get_linear_column_major`]<br>documents for `makesigmadfxyz`. Translated by reading column 0<br>explicitly. |
| `iterations` | `usize` | The count the reference prints as `Diffusion iteration`. |
| `nodal_updates` | `usize` | How many times the nodal correction was rebuilt and the operator<br>refactorised. Not in the reference's `output`; useful because the<br>interval, and hence this count, is what defect N1 is about. |
| `termination` | `Termination` | Why the iteration stopped. Not in the reference's `output`. |
| `diagnostics` | `Option<Diagnostics>` | The `params.debugdump` maps, `None` unless it was set. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SaNodalOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `sanodaldiffusion_solverxyz`

`output = sanodaldiffusion_solverxyz(geometry, params, sigmavalues, whichsigma, initial_k_eff, initflux)`.

Assembles the nodal-corrected diffusion operator and runs an accelerated
source iteration on it, returning the fundamental-mode flux and eigenvalue.

# Arguments

- `geometry` — needs `Vi`, plus everything [`makegrad_dxyz`] and the
  expansion chain read. `geometry.adf` supplies the assembly discontinuity
  factors and defaults to unity when absent.
- `params` — `G`, `Nc`, the three extents, and the four optional switches
  `nodalupd`, `fsexp`, `innertol` and `debugdump`.
- `sigmavalues` — per-material cross sections.
- `whichsigma` — the 1-based material map, `0` for void.
- `initial_k_eff` — `varargin{1}`; `None` is the reference's default of `1`.
- `initflux` — `varargin{2}`, the warm start. `None` is the flat guess. A
  matrix with at least [`HISTORY`] columns seeds the whole history; a
  narrower one has its **first column replicated** across all five, which is
  what `repmat(initflux(:,1), 1, nh)` does. A matrix whose row count is not
  `philenf` is **silently ignored** — the reference tests
  `size(initflux,1)==philenf` and falls through to the flat guess otherwise.

# The operator, and how it differs from the finite-difference one

```text
LHS = gradD + nodal + sigma.tot - sigma.s
RHS = fission_source / k_eff
```

The whole scattering operator is implicit, so there is no lagged scattering
source and a pass is one solve. Compare [`crate::diffusion_solverxyz`],
which keeps only the within-group diagonal `sigma.sd` on the left and lags
the rest — the converged operator is the same, the iteration is not.

# `nodalupd` — and why the default can be dangerous

The default is `ceil((maxix + maxiy + maxiz) / 10)`, which the reference's
own comment describes as "~5 for a 17x17x14 mesh" and claims smaller values
improve stability. **Defect N1 records the opposite**: an interval of 1 was
observed to run a small leaking cube to the 5000-iteration ceiling, where
any interval of 2 or more converged. And the default *is* 1 whenever the
three extents sum to 10 or less — so small test meshes get the pathological
setting automatically while real benchmarks (IAEA-3D gives 6) do not. Set
`params.nodalupd` explicitly on a small mesh.

The update fires on `iteration % nodalupd == 0`, counting the reference's
1-based iteration number.

# Normalisation — three integrals, two conventions

- `init_norm` is a **plain `sum`** of the initial fission source.
- `norm_factor`, applied once after the loop, is a **plain `sum`** of the
  final source.
- the `k_eff` update inside the loop uses **`norm(·, 1)`**.

A plain sum and a 1-norm agree only while the source stays non-negative.
[`crate::diffusion_solverxyz`] uses the 1-norm throughout and rescales every
pass rather than once at the end. Defect N10 covers both the inconsistency
and the fact that the "fission source integration = 1" comment describes
neither solver — what is actually preserved is the *initial* integral.

# Dead code in the reference

`weilandtfactor = 1.05` and `weilandt = 0` set up a Wielandt shift whose
every use site is commented out; `weilandt` can never become 1. `philen` is
computed and used only to size the initial `zeros(philen, 6)` nodal terms.
Both are preserved as written — the shift is not implemented here either.
Recorded as defect D4.

# On an early break, the normalisation pairs mismatched vectors

The final rescale divides by `sum(fission_source_new)` but applies to
`fission_source`. On a normal exit those are the same vector. On a `break`
they are **one iteration apart**, so the source is rescaled by a factor
derived from a different, and by hypothesis diverging, source. Preserved;
[`SaNodalOutput::termination`] is how a caller detects it. Defect D5.

# On a break, the reported state lags by one iteration

As [`crate::diffusion_solverxyz`]: the `break` precedes the increment, so
the returned `k_eff`, `residual` and `k_eff_residual` are the previous
pass's. The reference does *print* the offending new `k_eff` in its bail-out
message but does not return it. Defect D6.

# `Nc > 0` does not work

Two independent conformance failures, both defect N2: `calc_sanodalxyz`
returns a `philen`-square operator that is added to `philenf`-square ones,
and `Vi` is replicated to `G*es` while the fission source is `philenf` long.
All four benchmark cases set `Nc = 0`. Reproduced as panics.

# Errors

- [`BedokError::IterativeSolveNotTranslated`] if `philenf >= 50_000_000`.
- Whatever [`makegrad_dxyz`] raises.

# Panics

If `geometry.vi` is shorter than `maxix*maxiy*maxiz`, if `Nc > 0` (see
above), or wherever [`calc_sanodalxyz`] panics.

```rust
pub fn sanodaldiffusion_solverxyz(geometry: &crate::types::Geometry, params: &crate::types::Params, sigmavalues: &crate::types::SigmaValues, whichsigma: &crate::matlab::Array3<usize>, initial_k_eff: Option<f64>, initflux: Option<&crate::matlab::Array2<f64>>) -> crate::Result<SaNodalOutput> { /* ... */ }
```

### Constants and Statics

#### Constant `SIZE_THRESH`

`sizethresh` — above this many unknowns the reference switches to
preconditioned GMRES. See [`BedokError::IterativeSolveNotTranslated`].

```rust
pub const SIZE_THRESH: usize = 50_000_000;
```

#### Constant `MAX_ITER`

`maxiter` — the source-iteration cap. **5000**, where
[`crate::diffusion_solverxyz`] uses 10000.

```rust
pub const MAX_ITER: usize = 5_000;
```

#### Constant `HISTORY`

The flux history depth, `size(scalar_flux, 2)`.

The reference allocates `ones(philenf, 5)` and comments that it can be
increased if an acceleration scheme needs more.
[`crate::fiss_src_extrapolatexyz`] reads only the first **four** columns, so
the fifth generation is carried and never used — it is shifted along each
pass and falls off the end.

```rust
pub const HISTORY: usize = 5;
```

#### Constant `EXTRAP_HISTORY`

The number of generations [`crate::fiss_src_extrapolatexyz`] consumes.

```rust
pub const EXTRAP_HISTORY: usize = 4;
```

## Module `sigmavalupd3d`

Cross-section feedback — rebuild the material table from a per-node state.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `sigmavalupd3d.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

```rust
pub mod sigmavalupd3d { /* ... */ }
```

### Types

#### Struct `DeltaSigmaValues`

The per-material feedback slopes, plus the state they are referenced to.

`deltasigmavalues` in the reference. Each row is a material; the columns
match [`SigmaValues`].

```rust
pub struct DeltaSigmaValues {
    pub tot: crate::matlab::Array2<f64>,
    pub f: crate::matlab::Array2<f64>,
    pub fp: crate::matlab::Array2<f64>,
    pub s: crate::matlab::Array3<f64>,
    pub reference: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tot` | `crate::matlab::Array2<f64>` | Slope of the total cross section against the feedback variable. |
| `f` | `crate::matlab::Array2<f64>` | Slope of the fission cross section. |
| `fp` | `crate::matlab::Array2<f64>` | Slope of the prompt fission cross section. |
| `s` | `crate::matlab::Array3<f64>` | Slope of the scattering matrix, indexed `(material, gt, g)`. |
| `reference` | `f64` | `deltasigmavalues.ref` — the reference state the slopes are taken about.<br><br>Named `reference` because `ref` is a Rust keyword. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DeltaSigmaValues { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> DeltaSigmaValues { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `sigmavalupd3d`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

`[sigmavalues, whichsigma] = sigmavalupd3d(params, sigmavaluesold, whichsigmaold, whichsigmaref, deltasigmavalues, currval, m)`.

Applies the thermal-hydraulic feedback to the cross sections and, in doing
so, **re-numbers the material table one row per fuelled node**.

# What it actually does to the material numbering

This is the important structural point. On the way in, several nodes may
share a material row. On the way out, every fuelled node has been given its
**own** row, numbered in the scan order `ix`, `iy`, `iz`, and the returned
`whichsigma` points each node at its private row. That is what lets each
node carry a different temperature or density.

The returned table therefore has exactly as many rows as there are fuelled
nodes, and `whichsigma` is a fresh 1-based numbering with `0` for void —
the same convention [`crate::calcdiffvalues3d`] and
[`crate::makesigmadfxyz`] consume.

# Arguments

- `sigmavaluesold` — the current table, indexed by `whichsigmaold`.
- `whichsigmaold` — material per node for `sigmavaluesold`.
- `whichsigmaref` — material per node for `deltasigmavalues`, and the mask
  deciding which nodes are fuelled at all.
- `deltasigmavalues` — feedback slopes and their reference state.
- `currval` — the feedback variable per node, `es` long. The reference
  accepts a scalar and broadcasts it; pass a filled vector for that case.
- `m` — exponent applied to the feedback variable. `None` selects the
  reference's default of `1`.

# The feedback law

For each perturbed quantity,

$$ \Sigma = \Sigma_{old} + \frac{d\Sigma}{dv}\left(\mathrm{Re}(v^m) - \mathrm{Re}(v_{ref}^m)\right) $$

**`nu` and `chi` are not perturbed** — they are copied straight from
`sigmavaluesold`. Only `tot`, `f`, `fp` and `s` carry feedback.

# Two index spaces, and they are not the same

`sigmavaluesold` is indexed by `whichsigmaold`, while `deltasigmavalues` is
indexed by `whichsigmaref`. A node reads its base value from one table and
its slope from the other, at different row numbers. Conflating them would
pair each node with the wrong slope, and — because both are valid rows —
would produce plausible numbers rather than an error.

# Absent `fp`

[`SigmaValues::fp`] is optional, matching the reference's `isfield` guard in
`makesigmadfxyz`. This function reads it unguarded, so an absent `fp` is
treated as zeros here and the output carries a zero `fp` column. The
reference would raise `Reference to non-existent field`.

# Errors

[`crate::error::BedokError::NanEncountered`] if any output quantity contains
`NaN` — the reference runs `pauseonnan` over all six on the way out.

```rust
pub fn sigmavalupd3d(params: &crate::types::Params, sigmavaluesold: &crate::types::SigmaValues, whichsigmaold: &crate::matlab::Array3<usize>, whichsigmaref: &crate::matlab::Array3<usize>, deltasigmavalues: &DeltaSigmaValues, currval: &[f64], m: Option<f64>) -> crate::error::Result<(crate::types::SigmaValues, crate::matlab::Array3<usize>)> { /* ... */ }
```

## Module `sigmavalupd3d_handler`

Cross-section feedback — apply every enabled feedback channel in turn.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `sigmavalupd3d_handler.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What this is

The bridge from thermal-hydraulics back to neutronics. It takes the
reference cross-section set and applies each enabled feedback channel in
sequence, every one a call to [`crate::sigmavalupd3d`]:

| Channel | Feedback variable | Exponent |
|---|---|---|
| boron | `params.boron`, ppm | 1 |
| fuel temperature | `th.fueltempdoppler`, K | **0.5** |
| moderator temperature | `th.modtemp`, K | 1 |
| coolant temperature | `th.coolant.temps`, K | 1 |
| coolant density | `th.coolant.dens`, g/cm³ | 1 |
| control rods | a computed rod fraction, 0 to 1 | 1 |

Each is applied only if the case file supplied its table, so a case selects
its feedback model by which tables it defines. The reference's own comment
says as much: "split into independent factors/categories, can add more
categories as they come along".

**The `0.5` on fuel temperature is the square-root Doppler law**, and it is
the reason [`crate::sigmavalupd3d`] carries its `real(a^m)` machinery — a
negative argument under a square root is complex, and MATLAB keeps the real
part.

# The channels compose by chaining, not by summing

Each call takes the *previous* call's output as its `sigmavaluesold`, so the
perturbations accumulate in the order written above. The order is therefore
observable whenever two channels' slopes interact, though for the linear
tables the snapshot ships they commute.

# Reference defects carried here

- **C1, the most serious in the register: `rodlvl` is never initialised.**
  See [`sigmavalupd3d_handler`].
- **The rod-fraction CSV is written unconditionally** —
  `writematrix(rodfrac, 'rodfrac.csv')` runs on every call, outside the
  `debugdump` guard that protects the four dumps below it. Returned here
  rather than written, as everywhere else in this crate.
- **`sigmavaluesref.crod.ref = 0` mutates the caller's table** in MATLAB's
  copy-on-write sense — the local copy only, but it means the crod table's
  own `ref` field is ignored and forced to zero. Reproduced.

```rust
pub mod sigmavalupd3d_handler { /* ... */ }
```

### Types

#### Struct `FeedbackTables`

The feedback slope tables a case file may supply.

In the reference these are extra fields hung on the `sigmavaluesref` struct
and tested with `isfield`; here they are `Option`s on their own struct, so
the reference cross sections stay a plain [`SigmaValues`].

A `None` channel is simply not applied.

```rust
pub struct FeedbackTables {
    pub boron: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
    pub fueltemp: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
    pub modtemp: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
    pub cooltemp: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
    pub coolden: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
    pub crod: Option<crate::sigmavalupd3d::DeltaSigmaValues>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `boron` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.boron` — slopes against soluble boron, per ppm. |
| `fueltemp` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.fueltemp` — slopes against **the square root of** the<br>Doppler fuel temperature, per sqrt(K). |
| `modtemp` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.modtemp` — slopes against moderator temperature, per K. |
| `cooltemp` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.cooltemp` — slopes against coolant temperature, per K. |
| `coolden` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.coolden` — slopes against coolant density, per g/cm³. |
| `crod` | `Option<crate::sigmavalupd3d::DeltaSigmaValues>` | `sigmavaluesref.crod` — slopes against the rodded fraction of a node,<br>dimensionless.<br><br>Its `reference` is **forced to zero** on use; see the module docs. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FeedbackTables { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FeedbackTables { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `RodFraction`

The rod-fraction map, returned rather than written to `rodfrac.csv`.

```rust
pub struct RodFraction {
    pub frac: Vec<f64>,
    pub stale_level_carryovers: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `frac` | `Vec<f64>` | The rodded fraction of each node, 0 (unrodded) to 1 (fully rodded), in<br>the usual flattened order. |
| `stale_level_carryovers` | `usize` | How many lattice positions fell through the level search with `rodlvl`<br>left at a previous column's value — defect C1.<br><br>**Non-zero means the rod pattern is wrong**, silently, using another<br>column's insertion level. Zero on any pattern where every bank's tip<br>sits inside its column. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodFraction { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RodFraction { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `sigmavalupd3d_handler`

`[sigmavalues, whichsigma] = sigmavalupd3d_handler(params, geometry, sigmavaluesref, whichsigmaref, th)`.

# Arguments

- `params` — needs `boron` and the extents.
- `geometry` — needs `Lz` and, when the rod channel is enabled, `crodbanks`,
  `crod`, `crodstep` and `crodbtm`.
- `sigmavaluesref` — the unperturbed per-material cross sections.
- `feedback` — the slope tables; see [`FeedbackTables`].
- `whichsigmaref` — the reference material map.
- `th` — supplies `fueltempdoppler`, `modtemp`, `coolant.temps` and
  `coolant.dens`. The reference notes a caller with no T-H feedback may pass
  a dummy.

# Returns

`(sigmavalues, whichsigma, rodfraction)` — the perturbed **per-node** cross
sections, the compacted node-to-row map, and the rod-fraction map.

Note the output `whichsigma` is not the input one: [`crate::sigmavalupd3d`]
renumbers every fuelled node to its own row, so the returned table has one
row per node rather than per material.

# Defect C1 — `rodlvl` is never initialised

The rod level search is

```text
for iz = 1:maxiz
    if sum(Lz(idx+1 : idx+iz)) > rodpos(rod)
        rodlvl = iz;
        break
    end
end
```

with no assignment before the loop and no `else`. **If the bank's tip sits
at or above the top of its column the condition never fires**, `rodlvl` is
never written, and the code below reads whatever the *previous* `(ix, iy)`
left there.

That is not an exotic case: it is a **fully withdrawn bank**, which is the
end state of every rod-ejection transient — the primary thing this code
exists to model. The consequence is a silently wrong rod pattern, taking one
lattice position's insertion level for another's.

Translated with the same carry-over, counted in
[`RodFraction::stale_level_carryovers`] so a caller can see it happened. A
**first**-column occurrence has no previous value to inherit; MATLAB would
raise `Undefined function or variable 'rodlvl'`, and this returns
[`BedokError::NanEncountered`]'s sibling
[`BedokError::UninitialisedRodLevel`] rather than inventing one.

# Errors

- [`BedokError::UninitialisedRodLevel`] on a first-column C1 occurrence.
- Whatever [`crate::sigmavalupd3d`] raises — it runs `pauseonnan` over its
  six outputs.

# Panics

If the rod channel is enabled without `crodbanks`, or a bank number indexes
past `geometry.crod`.

```rust
pub fn sigmavalupd3d_handler(params: &crate::types::Params, geometry: &crate::types::Geometry, sigmavaluesref: &crate::types::SigmaValues, feedback: &FeedbackTables, whichsigmaref: &crate::matlab::Array3<usize>, th: &crate::types::Th) -> crate::Result<(crate::types::SigmaValues, crate::matlab::Array3<usize>, RodFraction)> { /* ... */ }
```

## Module `singleflow1devap`

Steady 1-D channel flow with boiling — the homogeneous-equilibrium model.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `singleflow1devap.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What it does, in two stages

The reference's own header calls this a "VERY SIMPLE 1-D boiling model", and
the structure is worth stating up front because the two stages are
independent:

1. **March the mixture enthalpy** up (or down) each channel from a plain
   energy balance, `dh/dz = q'/(G A)`. Nothing thermodynamic happens here —
   it is bookkeeping on where the heat went.
2. **Invert that enthalpy** into temperature, quality and void fraction at
   the channel pressure, using equilibrium thermodynamics plus a drift-flux
   void-quality relation.

Stage 2 is the point. A single temperature cannot represent a saturated
two-phase state — the whole boiling region sits at `Tsat`, and what varies
is the quality. Marching enthalpy and inverting afterwards is the
well-posed way to get both.

# Assumptions, all of them load-bearing

- **Constant pressure.** No pressure drop anywhere; the channel pressure is
  `th.coolant.inletpress` throughout. So there is no flow/pressure coupling
  and no density-wave behaviour.
- **Thermodynamic equilibrium.** Quality is `(h - hL)/(hV - hL)`, clamped to
  `[0, 1]`. Subcooled boiling and superheated liquid are unrepresentable by
  construction.
- **Channels do not communicate.** Each `(ix, iy)` column is marched on its
  own; there is no cross-flow.

# What it is *for*

The reference states the intent plainly: a cheap, robust **initial
condition** for the drift-flux solver, returning the same `th.coolant`
fields that solver reads as its initial guess — already carrying a physical
boiling profile instead of a uniform inlet state. `th_solverxyz.m` also uses
it as the channel model outright when `params.th_model == 'hem'`, which
`neacrpd1t` sets, so that a transient starts from a steady state computed by
the *same* model it will be marched with.

```rust
pub mod singleflow1devap { /* ... */ }
```

### Types

#### Struct `MixtureState`

The state that stage 2 recovers from a mixture enthalpy.

Every vector is one entry per node, in the usual flattened order, and the
units are BEDOK's cm-g-s throughout — see
[`singleflow1devap`]'s unit table.

```rust
pub struct MixtureState {
    pub enth: Vec<f64>,
    pub temps: Vec<f64>,
    pub alphag: Vec<f64>,
    pub quality: Vec<f64>,
    pub dens: Vec<f64>,
    pub ldens: Vec<f64>,
    pub gdens: Vec<f64>,
    pub vm: Vec<f64>,
    pub tcon: Vec<f64>,
    pub pran: Vec<f64>,
    pub kvis: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `enth` | `Vec<f64>` | Mixture enthalpy, kJ/kg, **after** the physical-window clamp. |
| `temps` | `Vec<f64>` | Mixture temperature, K — `Tsat` throughout the two-phase region. |
| `alphag` | `Vec<f64>` | Void fraction, dimensionless on `[0, 1]`. |
| `quality` | `Vec<f64>` | Equilibrium quality, dimensionless, clamped to `[0, 1]`. |
| `dens` | `Vec<f64>` | Mixture density, g/cm³. |
| `ldens` | `Vec<f64>` | Saturated liquid density, g/cm³. |
| `gdens` | `Vec<f64>` | Vapour density, g/cm³. |
| `vm` | `Vec<f64>` | Mixture velocity, cm/s. |
| `tcon` | `Vec<f64>` | Liquid thermal conductivity, W/(cm·K). |
| `pran` | `Vec<f64>` | Liquid Prandtl number, dimensionless. |
| `kvis` | `Vec<f64>` | Liquid kinematic viscosity, cm²/s. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MixtureState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> MixtureState { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `singleflow1devap`

`th = singleflow1devap(params, geometry, th, pwrdens)`.

# Arguments

- `params` — the three extents, plus the optional `evap_C0` and
  `evap_homog` closure switches.
- `geometry` — needs `Lz`, the per-node axial heights in cm, the `zlows` /
  `zhis` channel bounds, and `fuel.Rtot` / `fuel.subarea`.
- `th` — needs `maxpow`, `powratio`, `nfuelpin`, `coolheatfrac`,
  `flowrate`, `flowdir`, `heatflux` and the coolant inlet conditions. Its
  `coolant` fields are **overwritten** by this function.
- `pwrdens` — normalised power density per node, whatever the flux solver
  produced. Scaled here by `maxpow * powratio`.

# Returns

The updated [`Th`], with every `coolant` field populated: `enth`, `temps`,
`alphag`, `quality`, `press`, `dens`, `ldens`, `gdens`, `vm`, and the three
liquid transport properties `tcon`, `pran`, `kvis`.

# Units — the conversions at the end are easy to get wrong

The IAPWS layer works in SI; BEDOK works in cm-g-s. The three transport
properties are converted on assignment and each factor is load-bearing:

| Property | IAPWS | BEDOK | Factor |
|---|---|---|---|
| `tcon` | W/(m·K) | W/(cm·K) | `/100` |
| `pran` | — | — | `cp[kJ] * 1000` to make it dimensionless |
| `kvis` | m²/s | cm²/s | `*10000` |

Densities are `1/(1000 v)` — IAPWS gives m³/kg, BEDOK wants g/cm³.

# The enthalpy march is node-centred

The inlet node takes `enthin + 0.5*delta`, i.e. half a node's rise, and each
subsequent node adds half of its own plus half of its neighbour's. So the
stored value is the enthalpy at the node **centre**, not at a face. That is
consistent with everything else in the code being cell-centred, and it means
the outlet node's value is half a node short of the true channel exit
enthalpy.

# Reference defects carried here

- **The enthalpy clamp's comment contradicts its code (T10).**
  `enthmax = IAPWS_IF97('h2_pT', P, 1050)` is commented "steam at 900 K
  (safely below the 1073 K region-2 limit)". The value is **1050 K**, not
  900. The margin to the region-2 limit is therefore 23 K, not 173 K.
  Preserved as written; the clamp still works, but a reader trusting the
  comment would misjudge how much headroom it leaves.
- **`sat` is dead.** The three-way mask `sub`/`sup`/`sat` is computed, and
  `sat` is never read — the two-phase branch is the `temps` initialisation
  rather than an explicit case.
- **A channel with no power is skipped entirely**, leaving its enthalpy at
  the inlet value. The test is `any(pwrdens)` over the **whole** `z` column,
  `1:maxiz`, while the march itself runs only `zlow:zhi`. The two ranges
  disagree, which matters for a column whose powered nodes lie outside its
  own `[zlow, zhi]` bounds — reachable via
  [`crate::geometry_ends3d`]'s first-contiguous-run limitation.
- **Two different critical temperatures.** The surface-tension correlation
  uses `647.15 K` where the IF97 layer uses `647.096 K`. That is the
  correlation's own constant, not an error, but the two sit four lines apart
  and look like a typo.

# Panics

If `pwrdens`, `heatflux` or `geometry.lz` is shorter than the node count, or
if a per-node `flowrate` is.

```rust
pub fn singleflow1devap(params: &crate::types::Params, geometry: &crate::types::Geometry, th: &crate::types::Th, pwrdens: &[f64]) -> crate::types::Th { /* ... */ }
```

#### Function `invert_mixture_enthalpy`

Stage 2 — invert a mixture enthalpy into temperature, quality, void and the
liquid transport properties, at a fixed pressure.

# Why this is one function and not two copies

`singleflow1devap.m` and `singleflow1devaptime.m` each carry this block, and
the transient one's own comment says "(identical to singleflow1devap.m)" —
which it is, line for line. The two `.m` files differ **only** in stage 1,
the enthalpy march.

Duplicating ~90 lines to mirror that would create two copies that can drift
apart under any later fix, which is exactly what the workspace's reuse rule
exists to prevent. So the shared half lives here, in the module it
originated in, and [`crate::singleflow1devaptime`] calls it.

**If a future snapshot makes the two blocks differ, they must be split
again** — the sharing is justified by their being verbatim identical, not by
their being similar.

# Arguments

- `params` — read for `evap_c0` and `evap_homog` only.
- `p` — the channel pressure, **MPa**, constant across the whole core.
- `enth` — the marched mixture enthalpy, **kJ/kg**. Consumed, clamped, and
  returned in [`MixtureState::enth`].
- `flowrate` — the mass flux, g/(s·cm²), for the drift-flux closure and the
  mixture velocity.

# The physical window

The enthalpy is clamped to `[0, h2_pT(p, 1050 K)]` before anything else, so
the IAPWS inversions stay inside region validity. A runaway feedback can
otherwise push the enthalpy past the steam region and make
[`crate::iapws_if97::backward::t_ph`] return `NaN`. See defect T10 on
[`singleflow1devap`] for the comment/code mismatch in that clamp.

```rust
pub fn invert_mixture_enthalpy(params: &crate::types::Params, p: f64, enth: Vec<f64>, flowrate: &crate::types::MassFlux) -> MixtureState { /* ... */ }
```

## Module `singleflow1devaptime`

Transient 1-D channel flow with boiling — one implicit-Euler step.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `singleflow1devaptime.m`, `main_exec_diff3d_standalone`
  snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What changes from the steady version, and what does not

**Stage 2 — the inversion of mixture enthalpy into temperature, quality and
void — is identical**, and the reference says so in its own comment. It is
not duplicated here: both modules call
[`crate::singleflow1devap::invert_mixture_enthalpy`], which documents why
sharing is safe and when it would stop being so.

**Stage 1 is the whole difference.** The steady march integrates
`W dh/dz = q'`; this one adds the time derivative,

```text
rho A dh/dt + W dh/dz = q'_wall
```

and takes one implicit-Euler step of it.

# The face/centre scheme, and why it is written that way

The discretisation solves for the enthalpy at each cell **face** and defines
the cell-centred value as the average of its two faces:

```text
W (hf_i - hf_{i-1}) + cap_i (hc_i - hc_i_old) = q_i
hc_i = (hf_{i-1} + hf_i) / 2
cap_i = rho_old A Lz / dt
```

Substituting the second into the first and solving for `hf_i` gives the one
line the loop actually evaluates. The payoff is stated in the reference's
header and is worth checking: **as `dt -> inf`, `cap -> 0`** and the update
collapses to `hf_i = hf_{i-1} + q/W`, so `hc_i = hf_{i-1} + q/(2W)` — exactly
the steady half-node march of [`crate::singleflow1devap`]. The transient
scheme degenerates to the steady one rather than merely resembling it, which
is what makes a steady state computed by one a valid starting point for the
other.

# What is held constant

Mass flow rate and channel pressure do not change during the transient. The
reference notes the justification: the NEACRP PWR cases specify constant
inlet flow and a constant 155 bar core pressure. A transient that moved
either — a pump coastdown, a depressurisation — is outside this model.

```rust
pub mod singleflow1devaptime { /* ... */ }
```

### Functions

#### Function `singleflow1devaptime`

`th = singleflow1devaptime(params, geometry, th, pwrdens, thold, dt)`.

# Arguments

As [`crate::singleflow1devap::singleflow1devap`], plus:

- `thold` — the T-H state at the **previous** time step. Only
  `coolant.enth` and `coolant.dens` are read, supplying the old cell
  enthalpy and the density in the capacitance term.
- `dt` — the time step, **seconds**. Must be positive; `dt -> inf` recovers
  the steady solution, and `dt -> 0` freezes the enthalpy at `thold`'s.

# Returns

The updated [`Th`]. In addition to everything the steady version fills, this
sets `coolant.enthface` — the cell-**face** enthalpies the scheme actually
solves for.

# Differences from the steady march, beyond the time term

Two index details differ and both are the reference's:

- **The inlet face is re-seeded every channel**, `hfprev = enthin`, and the
  loop then covers `zlow..=zhi` inclusive in both directions. The steady
  version instead treats the first node specially *outside* the loop. The
  two arrive at the same place; the transient form is the tidier one.
- **The downward branch starts at `zhi` and includes it**, where the steady
  downward branch seeds `zhi` before looping over `zhi-1 ..= zlow`.

# Shared with the steady version

The unpowered-channel skip (`any(pwrdens)` over the whole `z` column rather
than `zlow:zhi`) and the whole of stage 2 are the same code and carry the
same notes — see [`crate::singleflow1devap`].

# Panics

If `pwrdens`, `heatflux`, `geometry.lz`, `thold.coolant.enth` or
`thold.coolant.dens` is shorter than the node count.

```rust
pub fn singleflow1devaptime(params: &crate::types::Params, geometry: &crate::types::Geometry, th: &crate::types::Th, pwrdens: &[f64], thold: &crate::types::Th, dt: f64) -> crate::types::Th { /* ... */ }
```

## Module `w3chf`

The W-3 critical-heat-flux correlation and the departure-from-nucleate-
boiling ratio.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `w3chf.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# Method reference

The W-3 (Tong) correlation for departure from nucleate boiling in a PWR rod
bundle. It is published in the open literature in **British units** —
pressure in psia, mass flux in lb/(hr·ft²), enthalpy in Btu/lb, equivalent
diameter in inches, heat flux in Btu/(hr·ft²) — and the reference has folded
the unit conversions into the coefficients so it can work in its own cm-g-s
and MPa units throughout.

**That folding was checked term by term during translation and it is
correct.** The check is worth recording, because a reader comparing this
code against a textbook statement of W-3 will otherwise see eight
unexplained constants:

| Reference constant | Published W-3 | Conversion |
|---|---|---|
| `0.06238` | `0.0004302` per psia | `x 145.038` psia/MPa |
| `0.01427` | `0.0000984` per psia | `x 145.038` |
| `0.5987` | `0.004129` per psia | `x 145.038` |
| `2.326` | `G/1e6` then `x 1e6` Btu/hr/ft2 | see below |
| `3271` | `1.037e6` Btu/(hr·ft²) | `x 3.15459e-4` W/cm² per Btu/hr/ft², then `x 10` |
| `124.1` per m | `3.151` per inch | `/ 0.0254` m/inch |
| `0.0003413` | `0.000794` per Btu/lb | `/ 2.326` kJ/kg per Btu/lb |

The `2.326` deserves its own line because it looks like the Btu/lb to kJ/kg
factor and **is not** — that is a coincidence. Mass flux enters the
published correlation as `G/1e6` with `G` in lb/(hr·ft²), and the whole
bracket is later multiplied by `1e6` Btu/(hr·ft²). Carrying
`Gm` in g/(cm²·s) through that chain gives
`1 g/(cm²·s) = 7373.4 lb/(hr·ft²)`, so the mass-flux term becomes
`Gm x 7373.4 x 3.15459e-4 = Gm x 2.3258` W/cm². The agreement with 2.326 to
four figures is what confirms the whole conversion chain, and it also fixes
the units of every input: **pressure MPa, enthalpy kJ/kg, hydraulic
diameter cm, density g/cm³, velocity cm/s, and the result W/cm².**

# Scope — this is a correlation, not a model

W-3 is an empirical fit valid over roughly 5.5-16 MPa, mass fluxes of
1.4-6.8 Mg/(m²·s), qualities from -0.15 to 0.15 and equivalent diameters of
0.5-1.8 cm. **Nothing here checks any of that**, matching the reference,
which evaluates the fit wherever it is asked. A DNBR computed outside the
correlation's range is an extrapolation and should not be reported as a
safety margin.

```rust
pub mod w3chf { /* ... */ }
```

### Types

#### Struct `Chf`

`chf` — the critical heat flux and the margin to it.

```rust
pub struct Chf {
    pub chf: Vec<f64>,
    pub dnbr: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `chf` | `Vec<f64>` | `chf.chf` — critical heat flux per node, **W/cm²**. |
| `dnbr` | `Vec<f64>` | `chf.dnbr` — departure-from-nucleate-boiling ratio, `chf / heatflux`,<br>dimensionless.<br><br>Above 1 the node is in nucleate boiling with margin; at or below 1 the<br>correlation predicts departure. A node with zero heat flux would give<br>infinity, which [`crate::fixinfnan`] turns into `0` — see the note on<br>[`w3chf`]. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Chf { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Chf { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `w3chf`

`chf = w3chf(geometry, th)` — critical heat flux at each node by the W-3
correlation, and the DNBR against the actual wall heat flux.

# Arguments

- `fuel` — needs `hydia`, the subchannel hydraulic diameter in **cm**.
  (`subarea` is read by the reference and never used; see below.)
- `th` — needs the coolant pressure, void fraction, mixture velocity, phase
  densities, enthalpy and quality per node, plus the inlet temperature and
  pressure and the wall heat flux.

# Returns

[`Chf`], both fields as long as `th.heatflux`.

# Reference defect — the upwind enthalpy is halved

The reference builds the enthalpy that enters the subcooling term as

```text
enthshift(1) = enthin;
enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2;
```

The second line is `(h_i + h_{i-1})/4` — an average, **halved again**. The
stray `/2` is almost certainly a typo for the two-point average
`(h_i + h_{i-1})/2`: nothing in the correlation motivates a factor of a
half, and the first element is set to the full inlet enthalpy rather than
half of it, so the two branches are inconsistent with each other.

Halving the enthalpy *raises* `hLsat - enthshift`, which raises `Kfour` and
so **overpredicts** the critical heat flux — a non-conservative error in the
direction that matters for a safety margin. Translated as written and
recorded as defect T1; see `docs/bedok-reference-defects.md`.

# A second deviation from published W-3, this one possibly deliberate

Published W-3 uses the **inlet** enthalpy in the subcooling term, constant
along the channel. The reference instead uses a per-node upwind-shifted
local enthalpy, which reduces to the inlet value only at the first node.
That may be an intentional local-conditions variant, or it may be the same
unfinished edit as the `/2`. The snapshot says nothing either way, so it is
preserved and flagged rather than repaired.

# `fixinfnan` masks a division by zero

`dnbr = chf / heatflux` is infinite wherever the heat flux is zero — every
unfuelled node, and every node before power is applied. The reference passes
the result through [`crate::fixinfnan::fixinfnan`], which substitutes `0`.
So **a zero DNBR means "no heat flux here", not "no margin"**, and the two
are indistinguishable in the output. This is the same masking defect C5
records against `fixinfnan`'s use after the flux solves.

# Dead reads in the reference

`gearth`, the gravitational acceleration, is assigned and never used;
`subarea` is read from the geometry and never used; and `ldens`/`gdens`
enter only through the mixture density. The three `writematrix` calls that
end the function are diagnostic dumps and are not reproduced — see the
module docs of [`crate::diffusion_solverxyz`] for why file writes are
returned rather than performed.

# Panics

If the per-node vectors in `th` are not all the same length.

```rust
pub fn w3chf(fuel: &crate::types::FuelGeometry, th: &crate::types::Th) -> Chf { /* ... */ }
```

## Module `w3chfhottest`

Find the hottest channel and evaluate W-3 critical heat flux on it.

# Provenance

- **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
  Institute (SNRSI).
- **Source file:** `w3chfhottest.m`, `main_exec_diff3d_standalone` snapshot.
- **Permission:** given by the author for open-source release under OUTRAM
  PARK; see the crate README, "Permission and attribution".
- **Licence:** GPL-3.0-only.

# What it does

Sums the wall heat flux down each `(ix, iy)` channel, picks the largest, and
runs [`crate::w3chf`] on that channel alone. The rationale is the usual one
for a hot-channel analysis: the departure-from-nucleate-boiling margin is
set by the worst channel, so there is no need to evaluate the correlation
everywhere.

# Reference defect C2 — the search can only return a diagonal channel

```text
if sum(heatflux(idx)) > qhi
    qhi  = sum(heatflux(idx));
    highx = ix;
    highy = ix;      % <- iy is meant
end
```

`highy` is assigned `ix`, not `iy`. So whichever channel is hottest, the one
actually analysed is `(ix, ix)` — always on the diagonal of the lattice.

**This silently analyses the wrong channel** for any core whose hot spot is
off-diagonal, which is most of them: a rod-ejection transient's hot spot
sits where the rod was, and a quarter-core model's peak is rarely diagonal.
The DNBR it reports is then a real number for a real channel — just not the
limiting one, so the margin is overstated whenever the diagonal channel is
cooler.

Translated as written, with the true peak reported alongside the analysed
one so a caller can see the two diverge.

```rust
pub mod w3chfhottest { /* ... */ }
```

### Types

#### Struct `HottestChannel`

Which channel the search picked, and which it should have.

```rust
pub struct HottestChannel {
    pub analysed: (usize, usize),
    pub true_peak: (usize, usize),
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `analysed` | `(usize, usize)` | The `(ix, iy)` the reference actually analyses — always `(ix, ix)` by<br>defect C2. |
| `true_peak` | `(usize, usize)` | The `(ix, iy)` that genuinely carries the most integrated wall heat<br>flux.<br><br>Not computed by the reference. When this differs from<br>[`HottestChannel::analysed`] the reported DNBR is for the wrong channel. |

##### Implementations

###### Methods

- ```rust
  pub fn misidentified(self: &Self) -> bool { /* ... */ }
  ```
  Whether defect C2 changed the outcome for this particular core.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HottestChannel { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HottestChannel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `w3chfhottest`

`chf = w3chfhottest(params, geometry, th)`.

# Arguments

- `params` — the three extents.
- `fuel` — passed through to [`crate::w3chf`]; needs `hydia`.
- `th` — needs `heatflux` and the coolant state over the whole core.

# Returns

`(chf, channel)` — the W-3 result for the analysed channel, `maxiz` entries
long, and which channel that was. See [`HottestChannel`] for why the second
is worth checking.

# Panics

If any per-node coolant vector is shorter than the node count.

```rust
pub fn w3chfhottest(params: &crate::types::Params, fuel: &crate::types::FuelGeometry, th: &crate::types::Th) -> (crate::w3chf::Chf, HottestChannel) { /* ... */ }
```

## Re-exports

### Re-export `BedokError`

```rust
pub use error::BedokError;
```

### Re-export `Result`

```rust
pub use error::Result;
```

