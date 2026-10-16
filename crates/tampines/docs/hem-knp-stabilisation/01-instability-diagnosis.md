# HEM / KNP instability: diagnosis (epic #896, phase 1)

> **Note, 2026-10-16:** this report is kept as written. The `HybridAllMach`
> mode, `KnpFaceClosure` and `assemble_hybrid_dissipation` it refers to were
> removed that day (maintainer decision D7, "retire the KNP hybrid"; see
> [`README.md`](README.md)). Line numbers into `mod.rs` no longer match.

Read-only diagnosis, 2026-10-16. Nothing in the repo was edited.

**Upstream used.** OpenFOAM is not vendored anywhere in the workspace. Neither
`crates/outram-foam-basic-lib/upstream_source/` nor the tampines one holds it:
both contain only a README. So I fetched it with a sparse clone into the
scratchpad: `OpenFOAM/OpenFOAM-10`, commit `89f925dc` (2024-02-02). It lives at
`.../scratchpad/upstream/of10/applications/solvers/compressible/{rhoCentralFoam,rhoPimpleFoam}`,
with `tutorials/compressible/rhoCentralFoam/shockTube`. Every upstream claim
below is from those files, read in this session.

---

## 0. Headline: the instability is not in the KNP part

`TampinesSteamArray` is **not** a KNP solver. It is a pressure-based
`rhoPimpleFoam` port, and `SolverMode::Pimple` is its default. KNP enters only
as an opt-in deferred-correction dissipation, `SolverMode::HybridAllMach`, and
only on faces where both sides are near-sonic and dense.

The observed failure (§2) happens in the **default Pimple mode, with KNP
off**. Per #406 the hybrid fails the same way. So "stabilise the HEM/KNP
solver" is really about the pressure-based HEM path:

- the break-cell mass over-drain;
- the explicit energy convection;
- the fixed Δt with no Courant control.

The KNP hybrid is a secondary layer on top of these.

---

## 1. Observed facts (code, with paths)

| # | Fact | Where |
|---|---|---|
| O1 | ~~Fixed Δt. No Courant number is computed anywhere: `compressibleCourantNo` and `setDeltaT` appear only inside the commented-out C++ reference.~~ **RESOLVED 2026-10-16 (gh:#900, A1):** `courant_number` ports `compressibleCourantNo.H`; `TimeStepControl::AdaptiveCourant` ports `setDeltaT.H` (opt-in); the default fixed step returns `CourantViolation` when an outflow Courant number exceeds 1. | `tampines-steam-tables/src/openfoam_algorithms/rhoPimpleFoam/mod.rs` (`grep -i courant` matches only the reference comment lines 421, 455 and 494) |
| O2 | Continuity is explicit, `rho = rho_old - dt*div(phi)`, followed by `rho.max(1e-4)` floors at mod.rs:1767-1772 and in `correct_thermo`, plus the `rho_cont` floor at about mod.rs:2222. | mod.rs |
| O3 | ~~**Energy convection is explicit** (`fvc::div` / `fvc::div_limited` as a source term).~~ **RESOLVED 2026-10-16 (gh:#900, A2):** `EnergyBalanceMode::Implicit` (upwind `fvm::div` plus deferred correction) is the default; `Explicit` is the ablation. Upstream uses implicit `fvm::div(phi, he)`. | mod.rs:2172-2177; upstream `rhoPimpleFoam/EEqn.H` |
| O4 | **RESOLVED 2026-10-16 (gh:#900):** `KineticEnergyTerms::Included` is the default; register row 12. Original finding: ~~**The EEqn omits the kinetic-energy terms.** Upstream has `fvc::ddt(rho,K) + fvc::div(phi,K)`. The port's EEqn has neither, and `docs/rhopimplefoam-port-omissions.md` has no row for them, so this is an unrecorded (UNEXPLAINED) omission.~~ | upstream `EEqn.H`; port mod.rs:2164 |
| O5 | **Upstream's own near-sonic option is missing.** `pimple.transonic()` (`fvm::div(phid, p)` in the pEqn) is absent from the port and ~~from the omissions register~~ (**registered 2026-10-16** as row 15, UNEXPLAINED; still absent from the port). | upstream `pEqn.H` |
| O6 | **Default changed 2026-10-16 (gh:#900, A3):** `PsiRefresh::OncePerOuterCorrector` is now the default; the per-inner-corrector full re-flash of ρ remains (register row 14). Original finding: **Upstream updates thermo once per outer corrector.** `thermo.correct()` sits only in `EEqn.H`, which runs before the pEqn. Inside the pEqn, upstream linearises with `thermo.correctRho(psi*p - psip0)`. The port re-flashes the full EOS, including ψ, after every inner corrector. So upstream ≈ `PsiRefresh::OncePerOuterCorrector`, the variant the port measured as *more* accurate: plateau 360.7 psia vs 392.4, GS-1 RMSE 80.6 vs 109.0 (`PsiRefresh` doc, mod.rs:589ff). The doc says "not verified, sources not vendored"; it is now checkable. | upstream `EEqn.H:26`, `pEqn.H`; port mod.rs `correct_thermo`, call at about 2142 |
| O7 | **How the KNP hybrid differs from upstream `rhoCentralFoam`** (see the list below). | `central_upwind.rs`, mod.rs:2906-3096; upstream `rhoCentralFoam.C` |
| O8 | HEM sound speed: Kieffer eq. 28 in the dome, falling back to an FD speed, floored at `C_MIN_MPS = 1 m/s`. Its own doc calls the Kieffer closure "AI-generated/unvalidated". | `central_upwind.rs` `hem_sound_speed_ph` |
| O9 | The `(p,h)` flash **panics** (`ph_flash_region`, `ph_flash_eqm/mod.rs:901`), although a checked facade `try_t_ph_eqm` returning `Err(OutOfRange)` exists. I observed both in the `boundary_273_15_repro` output (below). ~~`correct_thermo` uses the panicking API.~~ **RESOLVED 2026-10-16 (gh:#900, A4):** `correct_thermo` uses the checked `try_*` facade and returns `ThermoOutOfRange`. | `boundary_273_15_repro` run |
| O10 | **The CoolProp twin diverges from tampines:** | `outram-park-fork-coolprop/src/openfoam_algorithms/rhoPimpleFoam/mod.rs` |
| O11 | **Doc defects**, which I did not fix because this slice is read-only: | see below |

**O7 detail: what upstream `rhoCentralFoam` does that the hybrid does not**

- **(a) The KNP flux is the flux, and Δt follows the wave-speed Courant number.**
  Upstream marches with the KNP flux itself and sets Δt from
  `centralCourantNo` (`amaxSf`) and `maxCo`; the shockTube tutorial uses
  `adjustTimeStep yes; maxCo 0.2`. The port adds `β·(knp−central)` to `phi`
  *after* the pEqn has converged. That breaks the pEqn's mass balance and has
  no wave-speed Courant check.
- **(b) Energy dissipation.** Upstream dissipates the conserved total energy
  ρE: `phiEp` reduces to `−aSf·(ρE_pos − ρE_neg)`. The port injects **no**
  energy dissipation.
- **(c) Reconstruction.** Upstream reconstructs ρ and ρU (with the U limiter),
  and `e`, `rPsi = p/ρ` and `c` (with the T limiter), then forms
  `p_pos = rho_pos*rPsi_pos`. The port reconstructs ρ, U, he, p and c each
  independently. The EOS-closure alternative was tried and refuted (mod.rs
  `KnpFaceClosure`). Upstream's `p = ρ·(p/ρ)` form was never tried.
- **(d) Boundary faces.** Upstream fluxes boundary faces too; the port loops
  over internal faces only.

**O10 detail: the CoolProp twin**

- **No `fvc::ddt_corr` call (0 hits).** That is the transient Rhie–Chow term
  whose absence caused the tampines Edwards failure (ablation log A6).
- `correct_thermo` is **single-phase only**: a two-phase cell keeps its
  previous ρ, T and ψ, so it is not HEM in the EOS update.
- It *does* have `EnergyBalanceMode::Implicit` (implicit upwind plus deferred
  correction), measured at Co = 2: explicit +47 539 % of span vs implicit
  +1.19 %. It also has `h_min`/`h_max` clamps and a configurable taper.
- The `central_upwind.rs` files differ only in the sound-speed closure.

**O11 detail: doc defects**

- mod.rs: the `SolverMode` doc text is glued onto `PsiRefresh` (about lines
  565-582).
- mod.rs: the `assemble_hybrid_dissipation` doc is stacked onto
  `close_knp_face` (about lines 2836-2870).
- `collaboration/edwards_tampines_regen/hybrid_debug_log.md` and
  `hybrid_stability_debug_log.md` are cited but do not exist.
- The CoolProp `openfoam_algorithms/CLAUDE.md` says ψ = (∂ρ/∂p)_T; the code
  uses ∂ρ/∂p|_h.
- The omissions register says the drained-cell hold "engages 0 times". That
  holds only under Linear: it engages under van Leer (below).

## 2. Observed failure (run 2026-10-16, release)

```
cargo test --release -p tampines-steam-tables --test edwards_blowdown --no-run   # 5m07s build
EDW_HE_SCHEME=vanleer EDW_DBG=1 timeout 600 target/release/deps/edwards_blowdown-ad1dd5430acc6b02 \
    edwards_obrien_pipe_blowdown_600ms --exact --nocapture
```

The run is Pimple mode (KNP off), 24 cells, 30 µs. It **FAILED after 60.1 s**.
Log: `runs/edwards_pimple_vanleer_30us_2026-10-16.log` (beside this file).

```
step  2100 t=0.0630 | break: p=2598kPa T=498.5K a=0.000 | mdot=57.68 u=16.54
step  2200 t=0.0660 | break: p=1674kPa T=476.7K a=0.849 | mdot=24.84 u=43.16
WARNING pressure bounding engaged -- cell 22, solved p = -1.878176e4 Pa ... (psi = 1.009358e-6, rho = 8.300392e2 kg/m3)
step  2400 t=0.0720 | break: p=1724kPa T=478.1K a=0.850 | mdot=25.28 u=44.25
WARNING drained-cell enthalpy hold engaged -- cell 22 ... (rho_old = 1.364551e0 kg/m3, unclamped rho_old - dt*div(phi) = -5.469606e0)
step  2500 t=0.0750 | break: p=8448kPa T=502.4K a=0.000 | mdot=357.59 u=102.51    <- above the 7 MPa initial pressure
step  2900 t=0.0870 | break: p=6549kPa ...
step  3200 t=0.0960 | break: p=1155kPa T=459.4K a=0.937 ...
step  4100 t=0.1230 | break: p=2464kPa T=496.3K a=0.568 | mdot=40.45 u=26.23
panicked at ph_flash_eqm/mod.rs:901:9: p,h point below 273.15K
  ph_flash_region <- t_ph_eqm <- TampinesSteamArray::correct_thermo <- step
```

**Reading.** The break-end cell 22 first fails in the pressure solve while
still liquid (ρ = 830, ψ = 1e-6). The pressure is clamped. Then cell 22 is
**asked to lose about 5× its own mass in one 30 µs step**
(6.8 kg/m³ removed from 1.36 kg/m³ held). That is followed by a non-physical
water hammer to 8.4 MPa, which exceeds the initial pressure. The run then
oscillates between flashing (α ≈ 0.85-0.94) and liquid until a cell's `he`
falls below the 273.15 K isotherm.

**The panic is the guard, not #342.** It comes from the `(p,h)` guard itself
(mod.rs:901), so the state is far outside the domain. #342's 17.7 mK band is
not involved.

**Other runs.**

- `boundary_273_15_repro`: 4 passed, round-trip error −1.768e-2 K, as #342
  states.
- I started three 600 ms runs. Two were killed for time and CPU: van Leer at
  10 µs, and the hybrid under Linear. I did not observe them to completion.

## 3. Hypotheses, each with the predicted effect stated before any fix

**H1 (strongest). The outflow mass-Courant number exceeds 1 at the break cell.**

- *Mechanism.* With fixed Δt and an uncapped break velocity
  `u = mdot/(rho_cell·A_pipe)` (`tests/edwards_blowdown.rs` `choked_break`),
  explicit continuity removes more mass than the cell holds. The same condition
  makes the explicit energy update non-convex. Writing ρ_c·h for the cell's
  updated content:

  `ρ_c·h = (ρ_old − dt·F_out/V)·h_c + dt·F_in/V·h_in`

  This is a convex combination **iff** `ρ_old − dt·F_out/V ≥ 0`.
- *Prediction.* Capping outflow Co ≤ 1 (or adaptive Δt) leaves the 0–0.06 s
  plateau essentially unchanged; there Co ≈ u·dt/dx = 16·3e-5/0.17 ≈ 0.003.
  It removes the drained-hold events and the 8.4 MPa spike.
- *Untested.* Whether the cap alone stops the 0.123 s panic.

**H2. The explicit energy convection divides by a collapsing diagonal.**

- *Mechanism.* With upstream's implicit upwind `fvm::div`, the EEqn diagonal is
  `ρ_cont·V/dt + F_out = ρ_old·V/dt + F_in > 0` **even when ρ_cont < 0**. The
  "diag pinned at 1e-4·V/dt → he = −1.7e10" mechanism cannot occur.
- *Prediction.* Under van Leer the `he` blow-up disappears and plateau changes
  are small. This is the coolprop measurement: implicit is more diffusive only
  as Co → 1.

**H3. ψ / ρ is updated inconsistently with upstream (O6).**

- *Mechanism.* The full EOS re-flash plus FD ψ after every inner corrector
  makes the stiff-liquid cell's pressure equation jump. Witness the −18.8 kPa
  solve at ρ = 830, ψ = 1e-6.
- *Prediction.* Switching to `OncePerOuterCorrector`, or to upstream's linear
  `correctRho(ψ·(p−p0))`, cuts the pressure-bound events. The existing
  measurement already moved the plateau into the experimental band.

**H4. Mach-gated KNP switches on and off across the saturation line.**

- *Mechanism.* Equilibrium c falls by about two orders of magnitude from
  liquid to the dome, so β switches abruptly. On top of that, the deferred
  `d_phi` is added after the pEqn and breaks its mass balance.
- *Prediction.* Logging β per face per step shows flicker at the front.
  Upstream-style coupling (implicit transonic pEqn, or a true KNP flux with
  maxCo) removes it.
- *Weak evidence.* Not measured.

**H5. The missing K terms (O4) are an energy error at choked u.**

- *Prediction.* Small (½u² ≈ 0.1–5 kJ/kg at 16–100 m/s). This is not a
  stability driver, but the omission must be registered.

## 4. Candidate fixes, ranked by evidence

| Rank | Fix | Upstream behaviour restored | Evidence |
|---|---|---|---|
| 1 | Compute a convective Courant number `Σφ_out·dt/(ρV)` (plus an acoustic `amaxSf` Courant number when the hybrid is on). Add adaptive Δt (maxCo, upstream's damped growth `min(dt+0.1·dt_new, 1.2·dt)`), or return `Err` when the outflow Co exceeds 1. | `compressibleCourantNo.H` / `centralCourantNo.H` + `setDeltaT.H` | Observed −5.47 drain; O1 |
| 2 | Implicit energy convection by default: port `EnergyBalanceMode::Implicit` from the CoolProp twin (reuse, not new code). Register the K-term omission. | `EEqn.H`: `fvm::div(phi, he)`, `fvc::ddt(rho,K)+fvc::div(phi,K)` | H2 algebra; coolprop Co = 2 measurement |
| 3 | Make `OncePerOuterCorrector` the default (thermo after the EEqn, ψ held in the pEqn) and re-measure Edwards. | `EEqn.H:26` `thermo.correct()`, `pEqn.H` `correctRho` | O6 plus the existing measurement. The maintainer must decide, since it changes the default; then re-measure all V&V numbers |
| 4 | Have `correct_thermo` use the checked `try_*` facade and propagate the error through `advance_timestep`'s `Result`. This replaces a panic with an error; it adds no stability. | OpenFOAM FatalError, not UB | O9 |
| 5 | Replace or rework the hybrid. Options: (a) port upstream `transonic` (implicit, consistent with the pEqn); or (b) a true KNP march for the near-sonic regime that matches rhoCentralFoam: ρE dissipation, boundary faces, `p=ρ·rPsi` reconstruction, maxCo 0.2. Reuse the appbuilder `RhoCentralFoam` (Sod-validated, but perfect-gas, fixed Δt, reconstructs e not T). | `pEqn.H` transonic branch; `rhoCentralFoam.C` | O5, O7; H4 untested |
| 6 | Back-port `fvc::ddt_corr` to the CoolProp twin, and fix its single-phase `correct_thermo` before calling it HEM. | `pEqn.H` `ddtCorr` | O10; tampines A6 result |

**Recommended first measurement.** Run H1 + H2 together under
`EDW_HE_SCHEME=vanleer`, Pimple, 30 µs, and count `pressure_bound_events` and
`drained_hold_events`. Then ablate each fix individually.
