# rhoPimpleFoam stability — a student's guide

*A step-by-step walkthrough of why a compressible pressure–velocity solver
goes unstable, and how each fix works. Written after debugging a real set of
failures in the OUTRAM PARK rhoPimpleFoam port (2026-07-14). Companion to the
bead `op-21g.12`; the mechanism applies identically to `TampinesSteamArray`
(`tampines-steam-tables`) and `OPCPFluidArray` (`outram-park-fork-coolprop`),
which share this solver's design.*

> **How to read this.** Each section is a lesson: first the physics/algebra,
> then the failure it produces, then the fix. If you only remember one thing:
> in a pressure-based compressible solver, **almost every "blow up" is a
> boundary-condition or pressure-equation bookkeeping error, not a bad mesh.**

---

## Lesson 0 — What rhoPimpleFoam is actually solving

We march a compressible fluid in time. Per timestep we must satisfy three
coupled things at once:

1. **Mass** (continuity): `∂ρ/∂t + ∇·(ρU) = 0`.
2. **Momentum**: `∂(ρU)/∂t + ∇·(ρUU) = −∇p + ∇·(μ∇U)`.
3. **An equation of state (EOS)**: `ρ = ρ(p, h)` — here the real IAPWS-IF97
   steam tables, or CoolProp.

The trouble is that pressure does not have its own transport equation. It is
whatever value makes the velocity field satisfy continuity. PISO/PIMPLE is the
trick for finding that pressure. The whole method is a dance between a
**momentum predictor** (guess U from the current p) and a **pressure
correction** (fix p so the corrected U conserves mass).

Keep one mental picture: **pressure is a Lagrange multiplier that enforces
mass conservation.** When the solver blows up, it is almost always because we
told that multiplier the wrong thing at a boundary.

---

## Lesson 1 — How the pressure equation is assembled

In the momentum predictor we write the discrete momentum equation as

```
A·U = H(U) − ∇p
```

- `A` is the diagonal of the momentum matrix (units kg/s). It contains the
  `ρV/Δt` time term plus convection/diffusion.
- `H(U)` is everything off-diagonal (neighbour contributions + explicit
  sources).
- `rAU = V/A` is the "inverse diagonal" (m³·s/kg). `HbyA = H(U)/A` is the
  velocity you'd get ignoring the pressure gradient.

Substituting the momentum relation into continuity gives the **pressure
equation**. In our compressible, low-speed form:

```
∇·(ρ_f · rAU_f · ∇p)  −  ψ·V/Δt · p   =   ∇·(ρ_f · HbyA_f)  −  ψ·V/Δt · p_old
        (Laplacian)         (compressibility)     (mass imbalance)     (old-time)
```

where `ψ = (∂ρ/∂p)|_T` is the **compressibility**. That `ψ·V/Δt` term on the
diagonal is doing something quietly essential: it makes the matrix
non-singular (invertible) *without* needing to pin a reference pressure cell.
Remember it — it is the hero and the villain of Lesson 4.

In code the matrix is built in two steps:

```rust
let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p); // Laplacian term + BC contributions
for c in 0..n { p_eqn.ldu.diag[c] += psi[c]*V[c]/dt; } // add compressibility diagonal
// ... then add the mass-imbalance source ...
```

**This two-step assembly is where the first bug hides.**

---

## Lesson 2 — The pressure-source clobbering bug

### The physics of a Dirichlet pressure boundary

Suppose the outlet has a **fixed pressure** BC, `p = p_bc` (e.g. 1 bar). A
finite-volume Laplacian discretises the boundary face flux as
`coeff·(p_bc − p_owner)`. Rearranged, that puts **two** things into the linear
system for the owner cell:

- a term `+coeff` **on the diagonal** (the matrix "knows" this cell is tied to
  a Dirichlet boundary), and
- a term `+coeff·p_bc` **in the source vector** (the boundary's actual value).

Both are needed. Together they say "cell pressure is pulled toward `p_bc` with
strength `coeff`." Our `fvm::laplacian` does exactly this:

```rust
BoundaryCondition::FixedValue(v) => {
    mat.ldu.diag[owner] += coeff;
    mat.source[owner]  += coeff * v;   // <-- the Dirichlet source
}
```

### The bug

Now look at the buggy assembly:

```rust
let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p); // source now holds coeff*p_bc ✓
for c in 0..n { p_eqn.ldu.diag[c] += psi[c]*V[c]/dt; }
p_eqn.source = Field::new(source_p);  // <-- OVERWRITES the source! ✗
```

That last line replaces the entire source vector with our freshly computed
mass-imbalance + old-time source — **throwing away the `coeff·p_bc` term** that
`fvm::laplacian` had just put there. But the matching `+coeff` is still on the
diagonal. So the boundary cell's equation now reads, effectively,

```
(… + coeff)·p_owner − (neighbours) = (mass terms)   // but the +coeff·p_bc is GONE
```

which is the discretisation of a Dirichlet boundary with **`p_bc = 0`**. The
solver dutifully drives the outlet cell toward zero pressure. From a uniform
1 bar equilibrium — a state that should not move at all — it manufactures a
collapsing pressure at the outlet, a backflow, and within ~10 steps the field
explodes.

### The tell

We caught it by starting from **exact equilibrium** (uniform p, zero velocity,
outlet fixed at exactly the field value) and printing the pressure source of
the outlet cell. It read `ψ·V/Δt·p_old` *only* — the `coeff·p_bc ≈ 4×10⁻²`
Dirichlet contribution was missing, while the diagonal still carried its
`coeff`. A correct solver holds equilibrium as a fixed point; ours didn't, and
the source told us exactly which term had vanished.

> **Lesson:** a matrix built by a helper (`fvm::laplacian`) may already carry
> boundary contributions in *both* the diagonal and the source. If you then
> need to add your own source, **add to it — never overwrite it**, or you
> silently keep half of every Dirichlet boundary.

### The fix

```rust
for (s, &sp) in p_eqn.source.iter_mut().zip(source_p.iter()) {
    *s += sp;   // ADD, don't overwrite: preserves coeff*p_bc from the Laplacian
}
```

With defaults this only *adds* to a source that is zero for interior cells, so
it changes nothing there — but it preserves the Dirichlet term on every
fixed-pressure boundary cell. Equilibrium becomes a fixed point again, and a
fixed-pressure outlet stops blowing up. (This same bug and fix live in all six
of the workspace's pressure solvers — the two array backends plus
`rho_pimple_foam`, `pimple_foam`, `sonic_foam`, `hrm_foam`.)

---

## Lesson 3 — The boundary mass-flux write-back

A subtler cousin. When we assemble the mass-imbalance source we must account
for the flux through boundary faces. For a **fixed-velocity inlet** the true
face flux is `ρ_f · U_bc·S_f` (we know the velocity, so we know the flux
exactly). We compute that `corrected_flux` and use it in the source — good.

But the *face flux field* `phi_hbya` was interpolated from `HbyA`, whose
boundary values are a plain zero-gradient extrapolation (they do **not** know
about the prescribed inlet velocity). If we use `corrected_flux` in the source
but forget to also write it back into `phi_hbya.boundary`, then the line
`self.phi = phi_hbya` a few statements later stores the **wrong** boundary
flux. Next timestep, the continuity (`rhoEqn`) step reads that stale boundary
flux and the inlet mass balance is corrupted.

Fix: write the corrected flux back where it will be stored.

```rust
BoundaryCondition::FixedValue(ubc) => {
    let corrected_flux = rho_f.boundary[pi].values[fi] * ubc.dot(mesh.face_area_vectors[gf]);
    phi_hbya.boundary[pi].values[fi] = corrected_flux; // <-- write-back
    corrected_flux
}
```

> **Lesson:** the flux you *use in the pressure source* and the flux you
> *store for the next timestep* must be the same object. Boundary bookkeeping
> has to be consistent across the whole step.

---

## Lesson 4 — Stiff liquids and the water-hammer

Now the physics-not-bookkeeping failure. Everything above is fixed; the solver
holds equilibrium and drives a gas pipe happily. Then we drive **liquid
water** and it still crashes. Why?

### Compressibility sets the conditioning

Recall the pressure diagonal term `ψ·V/Δt`, with `ψ = (∂ρ/∂p)|_T`.

- **Gas** (nitrogen at 1 bar, 300 K): `ψ ≈ ρ/p ≈ 1.1×10⁻⁵ s²/m²`. Large. The
  `ψ·V/Δt` anchor is significant, the pressure equation is well-conditioned,
  and a modest inlet flow needs only a modest pressure to accelerate it.
- **Liquid water**: `ψ ≈ 4.5×10⁻⁷ s²/m²`, ~25× smaller. The anchor is tiny.
  Water barely compresses, so to accelerate a nearly-incompressible column you
  need a **large** pressure. This is *water hammer*.

### The Joukowsky surge

Start a liquid column of sound speed `c ≈ 1450 m/s` impulsively to velocity
`Δu`. The classical water-hammer (Joukowsky) pressure rise is

```
Δp = ρ · c · Δu
```

For `Δu = 0.5 m/s`: `Δp ≈ 1000 · 1450 · 0.5 ≈ 7.3×10⁵ Pa` — about **7 bar** on
top of 1 bar. We watched exactly this: the inlet cell surged to ~9.5 bar in a
few steps (physically correct!). Then the pressure wave **reflects** off the
fixed-pressure outlet as a rarefaction, and the inlet-side pressure
**undershoots** — in our run, all the way to **−12,735 Pa, i.e. negative
absolute pressure.**

Negative absolute pressure is unphysical (real water cavitates — it flashes to
vapour — long before that). The IAPWS-IF97 `(p, h)` flash has no answer there,
so it panics: `p,h point is outside pressure range`.

> **Lesson:** a "crash" here is not a code bug — it is the solver faithfully
> computing a transient so violent it leaves the region where the equation of
> state is defined. The question becomes *how do we keep the numerics inside
> the EOS's valid domain?*

---

## Lesson 5 — The standard fixes

Real compressible CFD codes survive stiff transients with a small toolbox.
Here is the same toolbox, and when to reach for each.

### 5a. Pressure bounding (OpenFOAM `pressureControl`)

The direct fix for "pressure left the EOS range": clamp it back. OpenFOAM's
compressible solvers do exactly this, and the rhoSimpleFoam change log spells
out the reasoning — *"In order to support complex equations of state, the
pressure can no longer be unlimited and rhoSimpleFoam now limits the pressure
rather than the density to handle start-up more robustly"* (OpenFOAM-plus
commit `655fc787`). Their `pressureControl::limit` is literally:

```cpp
if (limitMaxP_) { ... p = min(p, pMax_); }
if (limitMinP_) { ... p = max(p, pMin_); }
```

Our port mirrors it with a per-cell `p = p.clamp(p_min, p_max)` after each
pressure solve, defaulting `[p_min, p_max]` to the EOS validity range. Two
things to appreciate:

- With **default** bounds it only reshapes states the flash could not evaluate
  anyway, so it is nearly a no-op in normal running; set **tight** bounds to
  impose e.g. a cavitation floor.
- `f64::clamp` passes a `NaN` through unchanged, so a genuinely diverged field
  is **not** silently masked — it still reaches the flash. Bounding hides an
  *out-of-range but finite* transient; it does not hide a true divergence.

Bounding is *necessary but not sufficient*: it fixed our pressure crash, but
the same water-hammer rarefaction also drove the **enthalpy** below the
273.15 K validity isotherm. A single-phase `(p, h)` EOS simply cannot represent
that cavitation/flashing state.

### 5b. PISO vs SIMPLE vs PIMPLE, and under-relaxation

- **PISO** (`n_outer = 1`, a couple of inner correctors, no relaxation) is a
  *transient* scheme. It assumes `Δt` is a small, real, CFL-limited timestep.
  For an acoustic problem that means `Δt < Δx/c` — for water, `Δx/c ≈
  0.1/1450 ≈ 7×10⁻⁵ s`. Violate that and even the correct physics goes
  unstable.
- **SIMPLE** (`n_outer` large, one inner corrector, heavy under-relaxation
  `α_p ≈ 0.3`, `α_u ≈ 0.7`) is a *steady-state* scheme. Here `Δt` is a
  pseudo-timestep and you iterate to convergence. Under-relaxation
  `p ← p_prev + α·(p_solved − p_prev)` damps the step-to-step change, taming
  the acoustic ringing.
- **PIMPLE** is the blend: several outer correctors at chosen relaxation, so
  you can take a larger `Δt` than pure PISO while still resolving some
  transient.

In our tests, PISO + bounding survived a ≈ 0.02 m/s impulsive water start;
adding velocity under-relaxation raised that to ≈ 0.05 m/s. Neither is 0.5 m/s.

### 5c. Don't start impulsively — ramp

The most physical fix of all: real pumps do not step the flow from 0 to
0.5 m/s in one instant. **Ramp the inlet velocity** over many timesteps and
the Joukowsky surge `ρ·c·Δu` is spread over many small `Δu`, never building a
7-bar spike. If you find yourself fighting a startup transient, ask first
whether the *forcing* is physical, not just whether the solver is robust.

### 5d. Acoustic CFL

Tie it together: choose `Δt` from the **sound speed**, not the flow speed, for
a compressible transient. `Co_acoustic = c·Δt/Δx` should be ≲ 1 for PISO. Fast
liquids (high `c`) force small `Δt`; that is not the solver being fragile, it
is the acoustics being fast.

---

## Lesson 6 — Boundary-condition well-posedness

Finally, the classic combinations. A pressure-based solver wants **one
pressure reference somewhere** and consistent velocity/pressure pairs:

| Patch | Velocity | Pressure | Note |
|---|---|---|---|
| Inlet | `fixedValue` (U) | `zeroGradient` | you set the flow, p floats |
| Outlet | `zeroGradient` | `fixedValue` (p) | you set the back-pressure, U floats |
| Wall | `fixedValue` (0) | `zeroGradient` | no-slip |

Pitfalls worth memorising:

- **Over-constraining pressure.** Fixing pressure at the inlet *and* the outlet
  (or fixing p everywhere and also pinning a reference cell) over-determines
  the Lagrange multiplier and the correction equation "becomes unhappy."
- **Backflow at a `zeroGradient` outlet.** If recirculation develops, fluid
  re-enters through the outlet and a plain zero-gradient velocity BC is no
  longer appropriate. OpenFOAM's answer is `inletOutlet` /
  `pressureInletOutletVelocity` (zero-gradient on outflow, fixed on inflow) —
  a capability this port does not yet have, and a good next addition.
- **Numerically legal but physically absurd forcing.** The equations may still
  be solvable at `U_inlet = 10⁴ m/s` for water; the Courant number just
  becomes ridiculous and convergence vanishes. The solver does not know
  physics — you do.

> **Closing rule of thumb (from CFD practice):** *if the discretisation is
> correct but the BCs are wrong, the simulation usually either diverges
> immediately or converges to nonsense.* When it blows up, suspect the
> boundaries and the pressure-equation bookkeeping first — exactly the order
> of the lessons above.

---

## References

- OpenFOAM `pressureControl::limit`,
  `src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C`,
  Copyright (C) 2017 OpenFOAM Foundation (GPL-3.0). Source:
  <https://github.com/OpenFOAM/OpenFOAM-5.x/blob/master/src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C>
- "rhoSimpleFoam: added support for compressible liquid flows" (limit
  pressure, not density, for robust start-up with complex EOS), OpenFOAM-plus
  commit `655fc787`:
  <https://develop.openfoam.com/Development/OpenFOAM-plus/-/commit/655fc7874808927d14916307a2230a8965bdb860>
- rhoPimpleFoam solver guide:
  <https://doc.openfoam.com/2312/tools/processing/solvers/rtm/compressible/rhoPimpleFoam/>
- Joukowsky / water-hammer: any standard fluid-transients text, e.g. Wylie &
  Streeter, *Fluid Transients in Systems* (1993).
- The in-repo debugging trail and V&V record: bead `op-21g.12`, and
  `tampines-steam-tables/verification_and_validation/pressure_bounding_vs_openfoam_pressurecontrol.md`.
