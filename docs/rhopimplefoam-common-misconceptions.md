# Common Misconceptions: rhoPimpleFoam and Finite-Volume Discretisation

A living catalogue of the conceptual traps people (including this repository's
maintainer) actually fall into when learning the pressure-based finite-volume
schemes behind `rhoPimpleFoam` — and, in this project, when trying to verify an
AI-synthesised solver change from first principles.

**Why this exists.** These are not hypothetical. Every entry below was hit
during a real tutoring session while deriving the `TampinesSteamArray` solver
fixes. They are recorded because the *disproofs* are the valuable part: each one
has a short argument that kills the misconception outright, and those arguments
teach the underlying structure better than a correct statement ever does.

**How to use this with an AI tutor.** Ask to be taught **one step at a time**,
Socratically — you attempt each step, the tutor checks you, and only then do you
advance. Do not ask for a finished derivation; you will not learn it, and if it
is going into a manuscript it will not be your own work. When you get something
wrong, add it here.

**Format.** Each entry is: the misconception → why it's tempting → the disproof
→ the correct picture. The disproof is the point.

---

## A. Discretisation structure

### A1. "Only the time derivative is at the new time"

**Why it's tempting.** The time derivative is the only term that *visibly*
mentions old and new levels, so it feels like the only one carrying `u^new`.

**The disproof.** Suppose it were true — convection and diffusion evaluated
entirely at the old time. Then every cell's new velocity could be written down
directly, with no coupling to any other cell. There would be **no matrix, no
`a_P`, no linear solve at all**. But PIMPLE demonstrably solves a linear system.
Contradiction.

**The correct picture.** In an implicit scheme, time, convection *and* diffusion
all involve `u^new`; that is precisely why they contribute coefficients to the
matrix. A scheme where only the time term is implicit is the **fully explicit**
scheme — and it is shackled to a Courant stability limit, which is why we avoid
it.

### A2. "`a_P` holds new-time terms, `H` holds old-time knowns"

**Why it's tempting.** The time term really does split that way: `Vρ_P/Δt` into
`a_P`, and `Vρ_Pᵒu_Pᵒ/Δt` into `H`. Generalising feels natural.

**The disproof.** The **viscous term contains no old-time values whatsoever**,
yet it contributes to *both* `a_P` and `H`. So the split cannot be temporal.

**The correct picture.** There are two *independent* axes, and conflating them is
the root error:

| axis | meaning |
|---|---|
| implicit vs explicit | is the value at the new time (unknown) or known? |
| diagonal vs off-diagonal | is it *this cell's* value, or a *neighbour's*? |

- `a_P` = coefficient on **this cell's own** unknown `u_P` — the matrix diagonal.
- `H(u)` = the **neighbours'** contributions `Σ a_N u_N` (still unknowns!) plus
  genuinely known sources.

`H` is written as a *function of `u`* for exactly this reason: it depends on the
neighbouring velocity field and must be re-evaluated as the solution updates. In
practice you evaluate it with the latest available iterate — that refresh *is*
the corrector loop.

#### A2a. Variant: "`H` is `a_P`'s coefficients applied to `u^old`"

**Why it's tempting.** A2 half-corrected: you accept that `H` is a separate
bucket, but assume it *mirrors* `a_P` — same coefficients, evaluated at the old
time. It has a pleasing symmetry.

**The disproof.** If `H` were only `a_P`'s coefficients times `u^old`, the
neighbour values `u_N` would **never appear anywhere in the system**. But the
off-diagonal `u_N` terms are the coupling (A3) — without them nothing diffuses.
So `H` must contain `u_N`.

**The correct picture.** Exactly **one** of the three contributions is genuinely
old-time; the other two are neighbour values at the **new** time:

```
H(u)  =   V ρ_Pᵒ u_Pᵒ/Δt          ← time: the only genuinely OLD term
        + Σ_f (μ_f A_f/δ_f) u_N    ← viscous: NEIGHBOUR values, new time
        − Σ_(inflow) φ_f u_N       ← convection: NEIGHBOUR values, INFLOW faces
```

Two details that are easy to miss:

- **Coefficients are not reused wholesale.** `a_P` takes the viscous coefficient
  *bare* (`Σ μA/δ`, no velocity attached); `H` takes the same coefficient
  *multiplied by `u_N`*. Same number, different partner.
- **Convection splits by face, not by time.** `a_P` gets the **outflow** faces;
  `H` gets the **inflow** faces — a consequence of the upwind rule (C1), not of
  any time-level distinction.

**Why the time term is the exception.** The time derivative is the only term that
spans *two time levels*; it is a discretisation in time and necessarily
references `tⁿ`. Viscous and convection are purely spatial operators evaluated at
the new level, so they have no old-time content to contribute. Once the previous
step is complete, `u_Pᵒ` is simply known data — hence a source.

### A3. "Both viscous entries end up on the diagonal"

**Why it's tempting.** Both are implicit (new-time), so they feel like they
belong to the same place.

**The disproof.** If both were diagonal, the matrix would be **purely diagonal** —
no `a_N` anywhere, no cell coupled to any other. It would invert cell-by-cell
with no linear solve, and **nothing would ever diffuse**.

**The correct picture.** The off-diagonal entries *are* diffusion — they are the
mechanism by which a cell feels its neighbours:

```
Σ_f μ_f A_f (u_N − u_P)/δ_f
  → a_P gets  Σ_f  μ_f A_f/δ_f          (coefficient of u_P — diagonal)
  → H   gets  Σ_f (μ_f A_f/δ_f) u_N     (neighbours — off-diagonal)
```

Both are implicit; only one is diagonal.

### A4. "The viscous term is semi-implicit"

**Why it's tempting.** "Semi-implicit" gets attached to the whole scheme, so it
seems to apply to every term.

**The correct picture.** The viscous term is **fully implicit** — both `u_P` and
`u_N` are new-time unknowns, nothing is lagged. **Convection** is the
semi-implicit one: it is nonlinear (velocity transports itself), so the mass flux
`φ_f` is lagged at the previous iterate while the transported `u_f` stays
implicit. That is a **Picard** (successive-substitution) linearisation, and
refreshing the lagged flux each outer iteration drives the timestep toward a
fully implicit solution.

### A5. Neighbour terms belong to "the neighbour's equation"

**The correct picture.** No — every term discussed lives in **cell P's own row**
of the matrix:

```
row for cell P:   [ … a_W … a_P … a_E … ] · u = source
                        ↑     ↑     ↑
                    off-diag diag off-diag
```

The `u_N` entries are P's *coupling to* its neighbours, not the neighbours'
equations. They are bundled into `H` when rearranging to `a_P u_P = H(u) − ∇p`.

---

## B. Signs and conventions

### B1. Face gradient written as `(u_P − u_N)/δ_f`

**Why it's tempting.** "Divergence measures outward flow," so it feels right to
orient the difference outward-first.

**The disproof (physical, convention-free).** Let the neighbour move faster,
`u_N > u_P`. Viscosity must drag the slow cell along, so `P` **gains** momentum.
Only `(u_N − u_P)` gives a positive flux into `P`. The reversed ordering says a
cell *loses* momentum to a faster neighbour, which is nonsense.

**The correct picture.** The outward convention is already carried by the
**face-area vector `A_f`** in `Σ_f (·)_f · A_f`. The gradient itself is just a
directional derivative — `(ahead − behind)/distance` — and going outward from
`P`, "ahead" is `N`:

```
(∇u)_f  ≈  (u_N − u_P)/δ_f
```

Flipping it *as well* applies the outward convention twice.

### B2. Dropping the cell volume or the face areas in discretised continuity

**The disproof.** Dimensional audit. Writing

```
(ρ_new − ρ_old)/Δt + Σ_f (ρu)_f = 0
```

gives `kg/(m³·s)` for the first term and `kg/(m²·s)` for the second. They cannot
balance.

**The correct picture.** Integrating over the cell produces a volume factor, and
Gauss's theorem produces a **dot product with the outward face-area vector**:

```
V_cell (ρ_new − ρ_old)/Δt  +  Σ_f (ρu)_f · A_f  =  0
```

Now both terms are `kg/s`. Dividing by `V_cell` gives the continuity-consistent
density directly.

---

## C. Interpolation schemes

### C1. "Upwind takes the neighbour's value"

**The correct picture.** Upwind takes the value from the **upstream** cell —
whichever cell the flow is coming *from*. That is frequently `P` itself:

```
φ_f > 0  (flow leaving P)        → u_f = u_P   → diagonal
φ_f < 0  (flow entering from N)  → u_f = u_N   → off-diagonal
```

Taking information from where the flow came from is exactly *why* upwind is
stable.

### C2. "Central differencing is a finite difference"

**The correct picture.** For convection you need the face **value**, so central
differencing is an **interpolation** (a weighted average):

```
u_f ≈ ½(u_P + u_N)        interpolation  — sums
(∇u)_f ≈ (u_N − u_P)/δ_f  difference     — subtracts
```

Same two ingredients, opposite operation. "Difference" language belongs to
gradients (diffusion), not to face-value reconstruction.

### C3. "rhoCentralFoam uses central differencing"

**The correct picture.** `rhoCentralFoam` is a **density-based** solver built on
the **central-upwind** schemes of Kurganov–Tadmor and Kurganov–Noelle–Petrova
(KNP). These use local **wave speeds** to bias the flux — upwinding is built in,
and the schemes are non-oscillatory by construction.

**Why the distinction matters.** *Pure* central differencing on convection is
oscillatory at high cell Péclet number; *pure* upwind is stable but heavily
diffusive and smears shocks. KNP exists to obtain non-oscillatory behaviour
*without* upwind's excessive diffusion — which is exactly why it is the right
tool for suppressing ringing at a near-sonic front.

---

## D. Pressure–velocity coupling

### D1. "Pressure is solved from the momentum equation"

**Why it's tempting.** Pressure *appears* in the momentum equation, so that looks
like its home.

**The disproof.** Momentum is already the equation used to obtain `u`. One vector
equation cannot deliver two unknown fields. And pressure appears there only as a
**gradient** `∇p` — there is no `p` term to isolate and solve for.

**The correct picture.** Pressure is the **orphan**: no equation solves for it
directly. Assign momentum → `u`, energy → `T`, equation of state → `ρ`, and
**continuity is left over**. Pressure is therefore determined *implicitly*, by
the requirement that the velocity field satisfy mass conservation. Combining the
momentum relation with continuity manufactures a **pressure equation** — the
whole basis of pressure-based (SIMPLE/PISO/PIMPLE) methods.

### D2. `HbyA` is mysterious

**The correct picture.** It is only `H(u)/a_P`. Divide `a_P u_P = H(u) − ∇p` by
`a_P` and define `rAU ≡ 1/a_P`:

```
u_P = HbyA − rAU ∇p,        HbyA ≡ H(u)/a_P
```

`HbyA` is the velocity a cell would have from its **neighbours and its own
time-history alone** — that is, *everything except the pressure force*. The full
velocity is that, minus the pressure-gradient correction.

### D3. Incompressible intuition carried into compressible solvers

**The correct picture.** In incompressible `pimpleFoam`, `ρ` is constant and drops
out of continuity entirely, leaving `∇·u = 0`; density is not in the picture. A
compressible solver must instead link `ρ` to `p` in order to build a pressure
equation:

```
ρ ≈ ρ* + ψ(p − p*),      ψ ≡ ∂ρ/∂p
```

`ψ` is the stiffness knob. `ψ → 0` recovers the incompressible limit — so a
**too-small `ψ` makes the solver behave as if the fluid were incompressible**,
and it will not resist a pressure drop the way the real fluid does.

### D4. "The pressure (Laplacian) equation is the predictor" / "momentum predicts the pressure"

**Why it's tempting.** The pressure equation is the elaborate, clever-looking
step, so it feels like the main event. And pressure *does* appear in the momentum
equation, so momentum looks like its source.

**The correct picture.** They are the other way round. **Momentum predicts
velocity; continuity corrects pressure.**

```
1. MOMENTUM PREDICTOR   solve a_P u_P = H(u) − ∇p*  with the guessed p*
                        → u*, which does NOT satisfy continuity
2. form rAU = 1/a_P and HbyA = H(u)/a_P
3. PRESSURE CORRECTOR   substitute u = HbyA − rAU∇p into CONTINUITY
                        → the Laplacian; solve for the new p
4. VELOCITY CORRECTION  recompute u = HbyA − rAU∇p with the new p
                        → continuity now satisfied
5. repeat 3–4 (PISO correctors); refresh and repeat all (PIMPLE outer)
```

**The kernel of truth.** Momentum genuinely *feeds* the pressure equation — both
`HbyA` and `rAU` come out of the momentum matrix. But the equation being
*enforced* is continuity. Momentum supplies the `u`–`p` relationship; continuity
is what pins `p`.

In one line: **momentum turns a guessed pressure into a velocity; continuity
turns that velocity's mass error into a pressure correction.**

---

## E. Conservation form

### E1. Conservative and non-conservative energy forms are interchangeable

**Why it's tempting.** In continuous calculus they *are* equal:

```
∂(ρh)/∂t + ∇·(ρuh) = ρ Dh/Dt
```

**The disproof.** That equality is obtained by expanding both terms and cancelling
`h[∂ρ/∂t + ∇·(ρu)]` — the continuity residual. It vanishes only because
continuity holds **exactly**. If continuity is satisfied only to within a residual
`r`, the two forms differ by exactly `h·r`, a spurious energy source or sink.

**The correct picture.** Discretely, `r` is generally nonzero, because the
thermodynamic density `ρ(p,h)` from the equation of state and the density implied
by the discrete mass balance are computed by *different parts of the solver* and
need not agree. Their gap **is** `r`. During flashing, density swings violently,
so the leak `h·r` becomes large.

The fix is to use the **continuity-consistent** density in the storage term:

```
ρ_cont = ρᵒ − (Δt/V) Σ_f φ_f
```

which forces `r = 0` by construction, regardless of what the equation of state
returns.

---

## F. Solution algorithms and iteration

### F1. "Segregated solvers are fully implicit"

**Why it's tempting.** Tutorials contrast "implicit" segregated solvers with
explicit density-based ones, and the shorthand sticks.

**The correct picture.** Each segregated solve is implicit **in its own primary
unknown** — `u_P` and all its neighbours `u_N` are solved *simultaneously* in one
matrix. What is lagged is never the unknown itself; it is the **coefficients**.
There are two distinct lags:

| lag | what is stale | example |
|---|---|---|
| **nonlinearity lag** (*within* an equation) | a coefficient depending on the unknown itself | Picard-lagged mass flux `φ_f` in convection |
| **coupling lag** (*between* equations) | a value owned by another equation | `∇p` in momentum; `u` in continuity |

In `Σ_f φ_f u_f`, the `u_f` is implicit (it is in the matrix); the `φ_f` is a
lagged coefficient. The outer corrector loop repairs **both** lags at once —
which is why such loops are said to drive the timestep *towards* a fully implicit
solution. *Towards*, not *to*.

### F2. "Gaussian elimination" for the `a_P`/`H` iteration

**The correct picture.** Gaussian elimination is a **direct** method (LU) — one
pass, exact, no iteration. The splitting used here is **iterative**:

```
A = D + N   →   u = D⁻¹(b − N u)
```

with all neighbour values from the previous sweep this is **Jacobi**; using the
freshest available values as you sweep is **Gauss–Seidel**. The `a_P u_P = H(u)`
form is exactly this splitting, which is *why* the unknown appears on both sides
and *why* the scheme must iterate.

### F3. "Where did the predictor–corrector algorithm even come from?"

Not a misconception so much as a gap — but a very common one, and worth stating.

**The algorithm is not extra physics. It is a workaround for a choice.**

Conservation laws are *coupled*: momentum needs `p`, continuity needs `u`. There
are exactly two ways to handle that:

| approach | how | cost |
|---|---|---|
| coupled / monolithic | all unknowns in **one** matrix, solved at once | huge, ill-conditioned, memory-hungry |
| segregated | one equation at a time, one variable each | small matrices, but each solve uses **stale** values from the others |

**If you chose the monolithic route, there would be no predictor–corrector at
all.** The loop exists solely to repair the staleness that segregation
introduces.

And the *order* is forced, not chosen: pressure has no equation of its own, so
one must be manufactured; manufacturing it requires a `u`–`p` relation; only
momentum can supply that. Hence momentum first, then pressure, then velocity
correction.

> Predictor–corrector is what "solve the coupled system simultaneously"
> degenerates into when you insist on solving one variable at a time.

SIMPLE, PISO and PIMPLE are then just different answers to *how many times do we
go around, and what do we refresh each time?*

### F4. Why SIMPLE needs one pressure correction and PISO needs two or more

**SIMPLE (steady).** Intermediate iterations are physically meaningless — only
the converged state matters. One correction per outer pass suffices, because
leftover coupling error is cleaned up on the next of many thousands of passes
(under-relaxation keeps this stable).

**PISO (transient).** Every timestep is a real physical state carried forward, so
the coupling must be resolved *before* advancing time. And one correction is not
enough for a specific reason: correcting `u` immediately makes `H(u)` stale,
since `H` depends on neighbouring velocities — so the pressure just solved for is
no longer consistent with the velocity field it produced. The second corrector
repairs that; a third repairs the residue of the second, with diminishing
returns.

**PIMPLE** is the hybrid: PISO inner correctors plus SIMPLE-style outer
iterations with under-relaxation, permitting Courant numbers well above one.

---

## Contributing

Add an entry whenever a genuine misconception surfaces — especially your own.
Keep the four-part format, and make the **disproof** carry the weight: an
argument that makes the wrong idea collapse ("there would be no matrix,"
"nothing would diffuse," "the units cannot balance") is worth more than a
restatement of the right answer.

## References

Standard, non-novel background:

- H. Jasak, *Error Analysis and Estimation for the Finite Volume Method with
  Applications to Fluid Flows*, PhD thesis, Imperial College, London, 1996.
- H. Jasak, A. Jemcov, Ž. Tuković, "OpenFOAM: A C++ Library for Complex Physics
  Simulations," *International Workshop on Coupled Methods in Numerical
  Dynamics*, Dubrovnik, 2007.
- J. H. Ferziger, M. Perić, *Computational Methods for Fluid Dynamics*, Springer.
- L. Orgogozo et al., "An open source massively parallel solver for Richards
  equation," *Computer Physics Communications* **185**(12), 3358–3371, 2014.
  (Picard vs. Newton linearisation trade-offs in an OpenFOAM-based solver.)
