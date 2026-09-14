# Edwards blowdown, post-PETIR — ablation log

**Status: OPEN — original failure FIXED (A6), residual defect remains.** This is a running log of a debugging campaign, not a
write-up of a finished result. Entries are appended in the order they were
run, and **failed attempts are recorded as prominently as successful ones** —
a hypothesis that was measured and refuted is the main product of an ablation,
and deleting it means the next reader pays for it again.

**Companion documents.** The ground-up explanation of this solver and of the
three faults fixed before this campaign is
[`edwards_blowdown_solver_debugging.md`](./edwards_blowdown_solver_debugging.md);
read that first if the terms here (flashing plateau, `rho_cont`, the all-Mach
hybrid) are unfamiliar. Beads: `op-bgg0` (this campaign), `op-s2dc` (the
defect), `op-ppmk` (the evidence-standard finding that came out of it).

---

## 0. What started this

The Edwards–O'Brien blowdown case stopped passing. The regression bisects to a
single commit:

| commit | | result |
|---|---|---|
| `09787761` — *petir: lift the RealMath trait into the crate* | pre-sweep | **passed**, 161.79 s |
| `baf9408b` — *route exp/ln/powf through PETIR's ARM port* | post-sweep | **FAILED**, panic at 66.94 s |

Test: `edwards_hybrid_damps_ringing_vs_pimple`, release mode, back-to-back on
one machine. The panic is

```
panicked at ph_flash_eqm/mod.rs:865: p,h point below 273.15K
  ph_flash_region <- t_ph_eqm <- correct_thermo <- step
```

with the instrumented state (recorded in `op-s2dc`)

```
OUT-OF-DOMAIN cell 22: p = 4.202124e5 Pa, h = -1.712179e10 J/kg,
                       h_floor(273.15K) = 3.857666e2, rho_prev = 1.000000e-4
```

`baf9408b` is a mechanical sweep of 173 transcendental call sites onto PETIR's
ARM optimized-routines ports — a change of about one ulp per call.

**The perturbation is not the defect, and reverting it is not the fix.** A V&V
case whose outcome flips on the last bit of `exp()` is not a case anyone should
be citing; the 1 ulp merely tipped over something that was already marginal.
What this campaign is looking for is the thing that was marginal.

(The transferable lesson — that "1009 lib tests unchanged and the V&V fixtures
regenerate character-identical" is the wrong acceptance check for a ~10⁹-step
chained solve — is `op-ppmk`, and is not re-argued here.)

---

## 1. Leading hypothesis, from reading the assembly

Not a guess from the physics: this is what the energy equation actually builds.
`step()` assembles

```rust
let rho_cont = {
    let mut rc = rho_old.clone() + (-dt) * div_phi_final;
    for c in 0..n {
        if rc.internal[c] < 1e-4 { rc.internal[c] = 1e-4; }   // <-- the clamp
    }
    rc
};
let mut e_eqn = fvm::ddt_coeff_old(&rho_cont, &rho_old, &self.he, &he_old, dt)
    + fvm::laplacian(&alpha_h_f, &self.he);
```

and `ddt_coeff_old` is

```rust
mat.ldu.diag[c] += coeff_new.internal[c] * v_dt;              // rho_cont * V/dt
mat.source[c]   += coeff_old.internal[c] * phi_old.internal[c] * v_dt;
```

The module's own comment states the reason `rho_cont` exists at all: discrete
continuity

$$\frac{\rho_{cont} - \rho_{old}}{\Delta t} = -\nabla\cdot\phi$$

holds **exactly**, so the term $h_{old}(\rho_{cont}-\rho_{old})/\Delta t =
-h_{old}\nabla\cdot\phi$ cancels the $h\,\nabla\cdot\phi$ part of
$\nabla\cdot(\phi h)$ term for term. That cancellation is what reduces the
equation to the material derivative $\rho\,Dh/Dt = dp/dt$ and produces the
flashing plateau.

**The clamp breaks that identity.** The moment it fires, $\rho_{cont} \neq
\rho_{old} - \Delta t\,\nabla\cdot\phi$, the cancellation is no longer exact,
and an uncancelled $h\,\nabla\cdot\phi$ of perfectly ordinary magnitude is left
in the source — divided by a diagonal that has been pinned at $10^{-4}V/\Delta
t$. Enthalpy of order $-10^{10}$ J/kg is exactly what that arithmetic produces,
and it explains why the blow-up is at the break cell and nowhere else.

**Predicted sign and magnitude, stated before measuring** (per the workspace
rule): the defect should appear only in cells that reach the floor; it should
appear first at the cell nearest the break; and the failure should be
*insensitive* to the KNP path, because nothing in the argument involves the
Mach blend.

**Candidate fix.** Not moving the floor. A cell holding no mass has no
meaningful specific enthalpy, so none should be solved for: give it an identity
row (`diag = 1`, `source = he_old`) and hold `he`, rather than dividing an
ordinary source by a floored diagonal.

---

## 2. Ablation ledger

Every run, in order. `t_end` is the simulated window, not wall clock.

| # | Configuration | Result | Notes |
|---|---|---|---|
| A0a | `09787761`, default build, `edwards_hybrid_damps_ringing_vs_pimple` (0.15 s) | **PASS** 161.79 s | pre-sweep reference |
| A0b | `baf9408b`, default build, same test | **FAIL** 66.94 s | post-sweep; panic as above |
| A0c | `09787761`, default build, `edwards_obrien_pipe_blowdown_600ms` | **PASS** 346.17 s | full transient; golden reference captured |
| A0d | `892c3898` + `--features platform-libm`, same 600 ms test | **PASS** 346.06 s | reproduces A0c byte-for-byte in all three CSVs |
| A1 | `892c3898`, default build, same 600 ms test | **FAIL** 62.59 s | pure PIMPLE; dies at t ~ 0.117-0.120 s |
| A2 | A1 + instrumentation at the energy solve | **FAIL** (by design) | `rc_unclamped = -1.643`: mass over-drain, not a small density |
| A3 | A1 + drained-cell enthalpy hold | **FAIL** 70.75 s | original failure cleared, flashing reached (alpha -> 0.519); NEW failure at cell 19 with no clamp involved |

Entries from A1 onward are this campaign's own and are appended below as they
are run.

---

## A1 — baseline: current develop, default build, pure PIMPLE, 600 ms

**Configuration.** `892c3898`, default features (PETIR ARM route),
`edwards_obrien_pipe_blowdown_600ms`, `EDW_DBG=1`. No code change.

**Result: FAIL at 62.59 s wall.** Same panic as A0b.

```
panicked at ph_flash_eqm/mod.rs:875: p,h point below 273.15K
  ph_flash_region <- t_ph_eqm <- correct_thermo <- step
```

**When.** Between step 3900 and step 4000 — simulated **t ≈ 0.117–0.120 s**.
The last two traced steps:

```
step  3800 t=0.1140 | break: p=2692kPa T=499.2K a=0.000 | GS-1: p=390psia ... mdot=58.25 u=16.72
step  3900 t=0.1170 | break: p=2656kPa T=499.5K a=0.000 | GS-1: p=385psia ... mdot=58.49 u=16.80
```

### What this confirms

**Pure PIMPLE fails.** The 600 ms test uses `SolverMode::Pimple` unless
`EDW_HYBRID=1` is set (`edwards_blowdown.rs:356`), and it is not set here. So
the failure is reachable with the KNP path entirely out of circuit.

**That refutes Mach gating as a candidate for this failure, before spending a
run on it.** Shifting `HYBRID_RHO_TAPER_LO`/`HI` or the blend window cannot fix
something that happens with the blend switched off. Recorded here so the branch
is not re-opened later: it is not that retuning the taper was tried and did not
help — it is that the taper is provably not in the code path.

### What this UNDERMINES — read before trusting section 1

The trace is not what the hypothesis predicts, and this is worth being blunt
about rather than quietly moving on.

`op-s2dc` records the failing cell as *drained to the density floor*
(`rho_prev = 1.000000e-4`). But right up to the last traced step the break cell
is reported at **`a = 0.000` — no void at all** — with temperature *rising*
(498.3 K at t = 0.081 s to 499.5 K at t = 0.117 s) and break pressure falling
through 2656 kPa. A cell at zero void fraction is liquid, and liquid is not a
cell that has drained to `1e-4` kg/m³.

Three readings, not yet distinguished:

1. The trace's "break" station and the cell that panics (`cell 22` in
   `op-s2dc`) are **different cells**, and the drained one is simply not the one
   being printed.
2. The blow-up is **fast** — the cell goes from liquid to floored inside the
   100 steps between traces, so no trace ever shows the intermediate state.
3. The `op-s2dc` diagnosis is **incomplete or wrong**, and the floor is a
   consequence rather than the cause.

Note also that 2656 kPa at 499.5 K is essentially `p_sat(T)`: the failure
happens exactly as the break cell reaches flashing, which is the most violent
moment in the transient and the one the whole `rho_cont` cancellation exists to
handle.

**Next (A2) is therefore instrumentation, not a fix.** Print, at the moment of
failure: the cell index, `rho`, `he`, `p`, `rho_old`, the unclamped
`rho_old - dt*div_phi`, and whether the clamp fired — and the same for its
neighbours. Guessing at a fix before that would be building on a diagnosis the
trace has just called into question.

---

## A2 — instrumentation: what is actually at the failing cell

**Configuration.** A1 plus a temporary dump at the energy solve (`EDW_INSTR=1`),
placed there deliberately rather than at the panic site: at the solve,
`rho_cont`, `rho_old`, `div_phi` and the assembled diagonal are all still in
scope, whereas one call later inside the `(p,h)` flash every one of them is gone.

**Result: FAIL, as designed — the instrumentation panics first.** Cell 22 of 24:

```
cell            he        he_old          rho      rho_old   rc_unclamped  clamp       div_phi            p         diag
  19     9.97035e5     9.97031e5    8.10028e2    8.09919e2      8.10126e2  false    -6.90251e3    2.86532e6    1.92893e4
  20     9.99013e5     9.99017e5    3.31528e2    3.33286e2      3.31524e2  false     5.87315e4    2.45184e6    7.89366e3
  21     9.96653e5     9.96630e5    8.26479e2    8.26470e2      8.26794e2  false    -1.08180e4    4.02166e6    1.96861e4
  22   -1.71218e10     9.31365e5   1.25411e-2    1.45700e0     -1.64339e0   true     1.03346e5    6.11824e2   2.38410e-3
  23     9.72760e5     9.72773e5    4.82649e2    5.08194e2      5.09782e2  false    -5.29530e4    2.41097e6    1.21380e4
clamped cells this step: [22]
dt = 3.0e-5
```

### The finding

**The continuity density does not go small. It goes NEGATIVE.**

$$\rho_{old} - \Delta t\,\nabla\cdot\phi = 1.457 - 3\times10^{-5}\times 1.03346\times10^{5} = -1.643$$

The flux is asking to remove **3.10 kg/m³** from a cell that holds **1.457**, in
one 30 µs step. That is not a rounding matter the floor is smoothing over — it
is a **mass over-drain**, and the floor is hiding it.

Everything else follows arithmetically. The clamp pins `rho_cont` at `1e-4`, so
the diagonal is `1e-4·V/Δt = 2.384e-3`; the exact continuity identity that makes
`h_old·(ρ_cont − ρ_old)/Δt` cancel the `h·∇·φ` in `∇·(φh)` is broken the moment
the clamp fires; and the uncancelled remainder, of entirely ordinary size, is
divided by that diagonal. `he = −1.71218e10` against `he_old = 9.31365e5` is
what that division produces.

### Resolving A1's three open readings

1. **Different cell — YES, partly.** The `EDW_DBG` "break" station is not
   cell 22. Cell 22's neighbours are at 810, 331, 826, 483 kg/m³, all healthy;
   only cell 22 is rarefied. So the trace showing `a = 0.000` was never looking
   at the cell that fails.
2. **Faster than the trace interval — YES.** `clamped cells this step: [22]` —
   one cell, one step. There is no gradual approach to watch.
3. **`op-s2dc` incomplete — YES.** It recorded `rho_prev = 1.000000e-4` and read
   that as "the cell drained to the floor". The floor is a *consequence*. The
   cause is that the requested drain exceeds the available mass. That distinction
   decides the fix: moving or lowering the floor cannot help, because the
   quantity being floored is negative.

Note `p = 611.824 Pa` — the cell has also bottomed out on the triple-point
pressure floor. And `rho = 1.254e-2` (the EOS density from the previous
`correct_thermo`) sits two orders below `rho_old = 1.457`, so the EOS and the
continuity density have already diverged badly before this step.

### Consequence for the fix

There are **two stacked defects**, and they need separating:

- **D1, the over-drain.** `Δt·∇·φ > ρ_old`. A CFL-type violation on the mass
  equation at the break. This is the root.
- **D2, the division.** The energy equation divides an uncancelled source by a
  floored diagonal. This is the proximate cause of the panic.

A fix for D2 alone will stop the panic and leave mass being created by the
clamp. That is worth knowing rather than assuming, so it is run next, on its
own, as A3 — which is also the "full rhoPimpleFoam baseline with only the energy
drain fix" this campaign was asked for.

---

## A3 — energy-drain fix alone (the requested rhoPimpleFoam baseline)

**Configuration.** A1 plus the drained-cell enthalpy hold: a cell whose
continuity density hits the floor gets an identity row (`diag = 1`,
`source = he_old`) instead of a solve. Nothing else changed — no flux limiter,
no floor change, pure PIMPLE.

**Result: FAIL at 70.75 s wall — but it is a DIFFERENT failure, later, and the
solver does real physics in between.**

### What A3 bought

The original failure at `t ≈ 0.117 s` is gone. The run now passes through it and
**flashing begins**, which the pre-A3 run never reached:

```
step  3900 t=0.1170 | break: p=2656kPa T=499.5K a=0.000 | mdot=58.49 u=16.80
step  4000 t=0.1200 | break: p=2513kPa T=497.4K a=0.428 | mdot=43.11 u=21.33
step  4100 t=0.1230 | break: p=2434kPa T=495.7K a=0.519 | mdot=40.90 u=23.89
```

Void fraction goes 0.000 → 0.428 → 0.519, temperature turns over (499.5 → 495.7 K)
and the break mass flow drops (58.5 → 40.9 kg/s) as vapour appears. That is the
onset of the flashing plateau, and it is the behaviour the case exists to
reproduce.

**So D2 was real and the fix is correct as far as it goes.** It is not
sufficient.

### The new failure — and it is NOT the clamp

Cell 19, at `t ≈ 0.123–0.126 s`:

```
cell            he        he_old          rho      rho_old   rc_unclamped  clamp       div_phi            p         diag
  18     9.94660e5     9.94670e5    3.50217e2    3.50937e2      3.50077e2  false     2.86799e4    2.44604e6    8.33540e3
  19     3.43649e7     9.96917e5    1.42655e-2   1.21352e-2     1.46167e-2  false    -8.27167e1    7.17020e2   3.48030e-1
  20     3.47772e5     3.48667e5    9.69984e2    9.72376e2      9.69161e2  false     1.07182e5    5.62939e4    2.30759e4
  21     9.33174e5     9.41299e5    1.79767e2    1.81145e2      1.84081e2  false    -9.78537e4    1.57472e6    4.38300e3
clamped cells this step: []
```

Read that carefully, because it rules out the obvious follow-up:

- **`clamped cells this step: []`.** No clamp fired anywhere. The A3 hold never
  engaged. Whatever this is, it is not the floor.
- **Cell 19 is not draining — it is GAINING mass.** `div_phi = −8.27e1`, so
  `rho_cont` *rises* from 1.21e-2 to 1.46e-2. The over-drain story (D1) does not
  apply to this cell either.
- **`he` blows up anyway**, to `+3.44e7` J/kg — about nine times the physical
  maximum for water, and positive this time rather than negative.

The mechanism is the same arithmetic with a different cause. `diag = ρ_cont·V/Δt
= 3.48e-1` because the cell is nearly empty (1.4e-2 kg/m³ — roughly 1/70 000 of
liquid water). Against a diagonal that small, *any* residual in the source
dominates, and the equation for **specific** enthalpy is simply ill-conditioned.
No clamp is needed to produce it.

### The actual anomaly, which is upstream of the energy equation

Look at the pressure across four adjacent cells:

| cell | p (Pa) | ρ (kg/m³) |
|---|---|---|
| 18 | 2.446e6 | 350.2 |
| 19 | **7.170e2** | **1.43e-2** |
| 20 | 5.629e4 | 970.0 |
| 21 | 1.575e6 | 179.8 |

That is **four orders of magnitude of pressure variation between neighbouring
cells**, with an isolated near-vacuum cell sandwiched between two dense ones.
That is not a thermodynamic problem and not a floor problem — it is a **local
breakdown of the pressure–velocity coupling**, and the energy blow-up is
downstream of it.

It is not a CFL violation either, which is worth stating because it is the
natural next guess. With `dx = 4.096/24 = 0.171 m`, `u ≈ 21 m/s` and
`dt = 30 µs`, the convective Courant number is `0.0037`; at `c ≈ 400 m/s` the
acoustic one is `0.07`. Both are tiny. The timestep is not the problem.

**Timing is the clue.** The breakdown lands immediately after flashing onset
(`a: 0.000 → 0.428` in the 100 steps before it), which is exactly where `ψ =
∂ρ/∂p|_h` changes by about two orders of magnitude as the flashing compliance
term switches on. The module's own `ψ` comment says so directly. That makes the
pressure equation's diagonal the first place to look, not the energy equation's.

### Ledger entry

| # | Configuration | Result | Notes |
|---|---|---|---|
| A2 | A1 + instrumentation at the energy solve | FAIL (by design) | `rho_cont = −1.643`; over-drain identified |
| A3 | A1 + drained-cell enthalpy hold | **FAIL** 70.75 s | original failure cleared; flashing reached (α → 0.519); new failure at cell 19, no clamp involved |

**A3 is kept.** It fixes a defect that is real, independently of what comes
next, and without it the run cannot even reach the regime where the remaining
problem lives.

**Next (A4): stop looking at the energy equation.** Instrument `ψ`, `p` and the
pressure-equation diagonal across cells 17–21 through flashing onset, and find
out how cell 19 arrives at 717 Pa while its neighbour sits at 2.45 MPa.

---

## A4 — the pressure equation, and what the bounding was hiding

**Configuration.** A3 plus a one-shot dump (`EDW_INSTR_P=1`) fired the **first**
time the pressure solve wants to put a cell below `p_min`, printed *before*
`pressureControl::limit` clamps it. Latched to report once, so it reports the
earliest occurrence rather than drowning the log after the field has already
broken down.

**Result.** First excursion at **t ≈ 0.114–0.117 s** — not a slow accumulation,
but the very first time it happens, coinciding with the original A1 failure.

```
==== pEqn wants p < p_min at cell 21 (p_min = 6.1182e2 Pa) ====
cell         p_raw         p_old          psi          diag        source          rho           he
  18     2.09919e6     2.09623e6   2.99816e-4    7.14093e-3     1.49838e4    2.06487e2    9.93277e5
  19     2.86414e6     2.86408e6   2.21828e-3    5.28198e-2     1.51279e5    8.08952e2    9.96973e5
  20     2.49454e6     2.49942e6   6.11914e-4    1.45715e-2     3.63468e4    3.57143e2    9.99064e5
  21    -2.72348e4     3.73547e6   1.02327e-6    4.26336e-5    -1.08840e1    8.26681e2    9.96292e5
  22     4.34045e5     3.23217e5   3.33061e-5    8.27930e-4     3.21136e2    8.28404e0    1.00333e6
  23     2.21069e6     2.66017e6   9.99921e-7    4.13129e-5     8.37324e1    8.31795e2    9.72922e5
cells below p_min: [21]
```

### `psi` is not the bug — it is correct

Cell 21 is dense subcooled liquid (826.7 kg/m³) at 3.735 MPa, and
`psi = 1.02e-6`. For subcooled liquid `psi = ∂ρ/∂p|_h ≈ ρ·κ_T = 826.7 ×
4.5\times10^{-10} ≈ 3.7\times10^{-7}` — same order. The value is right.

That is what makes it dangerous. With `V/Δt ≈ 23.81`, the anchor term
`psi·V/Δt·p_old = 2.436e-5 × 3.735e6 = +91.0`, while the total source is
**−10.88**, so the mass-flux part is ≈ **−101.9** and more than cancels the
anchor. The residual is negative, and dividing it by a diagonal of `4.26e-5`
gives a negative absolute pressure.

Compare **cell 23**: `psi = 9.99921e-7`, `diag = 4.13129e-5` — essentially
identical — but a *positive* source, and it lands healthily at 2.21 MPa. So a
tiny `psi` is **not sufficient** to cause this. Tiny `psi` *plus* a negative
mass-flux residual is.

This is the same stiff-liquid amplification $A \approx 1/(p\,\kappa_T)$ that the
`p(rho,h)` conditioning work measured from the other direction — about 600 at
3.7 MPa. A sub-percent mass imbalance becomes a multi-MPa pressure swing.

### The real signal: the pressure field is CHECKERBOARDED

Read `p_old` across the row rather than one cell at a time:

| cell | 18 | 19 | 20 | **21** | **22** | 23 |
|---|---|---|---|---|---|---|
| p_old (MPa) | 2.10 | 2.86 | 2.49 | **3.74** | **0.32** | 2.66 |

A 3.74 MPa spike immediately beside a 0.32 MPa dip, with everything else near
2.5. That is textbook **odd–even (checkerboard) decoupling** of a collocated
pressure–velocity arrangement. The stiff-liquid cell goes negative first
because it has the least diagonal to damp the oscillation — but the oscillation
is the disease, and the negative pressure is a symptom.

**Bounding is therefore not a safety net, it is a mask.** A cell clamped up from
−27 kPa to 611 Pa is indistinguishable downstream from one that legitimately
landed there, which is precisely why cell 19's 717 Pa in A3 looked like it came
from nowhere. **If the bound triggers, something is already wrong.**

### Root cause: a ported operator that was never wired in

The flux correction *is* the standard Rhie–Chow arrangement —
`phi = phi_HbyA − rho_rauf·snGrad(p)·|Sf|`, with the same `rho_rauf` in the
pressure Laplacian. What is missing is the **transient** half. Upstream
`rhoPimpleFoam` builds

```cpp
phiHbyA = fvc::interpolate(rho)*fvc::flux(HbyA)
        + rhorAUf*fvc::ddtCorr(rho, U, phi);   // <-- absent in this port
```

and this port has only the first term:

```rust
let mut phi_hbya = rho_f.clone() * fvc::flux(&hbya);
```

**`fvc::ddt_corr` is already in this workspace**, fully implemented, with
OpenFOAM's `fvcDdtPhiCoeff` limiter, a faithful port of
`EulerDdtScheme::fvcDdtPhiCorr`. Its own doc comment states its purpose:

> Re-injecting it into `phiHbyA` before the pressure solve (as
> `interpolate(rAU)·ddtCorr`) keeps the face flux coupled to its own history,
> which is what suppresses pressure–velocity (checkerboard) decoupling

It has **zero call sites**. It was ported and never connected.

This is the "search the workspace before building" and "read upstream first"
rules landing on the same line of code: the fix already exists in-tree, and
upstream says where it goes.

### Falsifiable prediction, recorded BEFORE running A5

The Rhie–Chow damping scales with `rAU ~ Δt`. Without `ddtCorr`, a **smaller**
timestep gives **weaker** damping and therefore **more** checkerboarding — the
classic small-Δt collocated-solver failure.

So `EDW_DT_US=10` must fail at an **earlier or equal simulated time** than the
30 µs baseline's `t ≈ 0.117 s`, despite costing 3× the wall clock to get there.
If it instead runs further and cleaner, this hypothesis is **wrong** and the
`ddtCorr` lead should be dropped rather than defended.

---

## A5 — timestep sweep: **the prediction was WRONG**

**Configuration.** A3 (enthalpy hold) with `EDW_DT_US=10` — a 3× smaller
timestep. Nothing else changed.

**Result: PASS, 1150.05 s.** The full 600 ms transient completes.

### The prediction, and its refutation

Recorded in A4 and committed (`e75ce9ea`) *before* this run:

> The Rhie–Chow damping scales with `rAU ~ Δt`. Without `ddtCorr`, a **smaller**
> timestep gives **weaker** damping and therefore **more** checkerboarding […]
> So `EDW_DT_US=10` must fail at an **earlier or equal simulated time** than the
> 30 µs baseline's `t ≈ 0.117 s` […] If it instead runs further and cleaner,
> this hypothesis is **wrong** and the `ddtCorr` lead should be dropped rather
> than defended.

It ran further and cleaner. **The prediction is refuted and the reasoning
behind it was wrong for this case.** Writing it down rather than reinterpreting
it, per the standing terms of this log.

What the refutation costs, precisely:

- The **small-Δt collocated-decoupling argument does not apply here.** Smaller
  `Δt` is *more* stable, not less. That is ordinary stability-limit behaviour.
- **`ddtCorr` is no longer implicated as THE root cause** on the strength of
  the `Δt` trend. It remains a genuine, unexplained omission from the port
  (register row 1) and is worth wiring in on its own merits — upstream has it
  and we do not — but that is now a *correctness* argument, not a *this-is-the-
  bug* argument. The distinction matters and the earlier commit message
  overstated it.
- The **checkerboard observation itself still stands** — 2.10, 2.86, 2.49,
  3.74, 0.32, 2.66 MPa is not a healthy pressure field however it arose. What
  is now open is whether the oscillation is a *cause* or another *symptom* of
  the stiffness.

### What the result positively supports

A genuine **stability limit exceeded at 30 µs**, and not an advective or
acoustic one — the Courant numbers there are 0.0037 and 0.07. The limit is
thermodynamic.

The leading candidate is the **`psi` linearisation window**. The pressure
equation linearises `ρ(p)` about `p_old` using `psi = ∂ρ/∂p|_h`, computed by a
central difference of step `dp = max(p·10⁻³, 50)` ≈ **3.7 kPa** at cell 21's
3.735 MPa. That secant is only trustworthy over a pressure change of about that
size, and the `Δp` a step actually takes scales with `Δt`. At 30 µs the step's
`Δp` evidently runs far outside the window, so the pressure equation
extrapolates a slope well beyond where it was measured — and in stiff liquid,
where `A ≈ 1/(p·κ_T) ≈ 600`, that overshoots hard enough to reach negative
absolute pressure. At 10 µs the step's `Δp` falls back inside the window.

This is consistent with everything measured so far, including why the failing
cell is *dense subcooled liquid adjacent to the flashing front* rather than one
of the rarefied cells: stiff at the boundary.

**It is a hypothesis, not a finding.** The discriminating test is to vary the
`psi` FD step at fixed `Δt = 30 µs`: if widening `dp` toward the actual
per-step `Δp` removes the failure, the window is the mechanism.

### Caveat — A5 does not isolate `Δt`

A5 carries the **A3 enthalpy hold as well as** the smaller timestep, so "passes
at 10 µs" is a property of (A3 + 10 µs), not of 10 µs alone. The clean
separation is a 10 µs run with the hold removed. Recorded so the result is not
over-claimed.

| # | Configuration | Result | Notes |
|---|---|---|---|
| A4 | A3 + pressure-equation instrumentation | FAIL (by design) | cell 21 solves to −27.2 kPa, clamped to 611.8 Pa |
| A5 | A3 + `dt = 10 µs` | **PASS** 1150.05 s | full 600 ms; **refutes** the A4 prediction |

---

## A6 — `fvc::ddt_corr` wired in: **PASS at 30 µs, and more accurate**

**Configuration.** A3 (enthalpy hold) plus the transient Rhie–Chow term
restored to `phiHbyA`, at the **default 30 µs** — the timestep at which every
previous run failed. Plus the new pressure-bound counters.

**Result: PASS, 390.75 s.** The full 600 ms transient completes.

### It does not merely stop the crash — it improves the physics

| quantity | golden (`09787761`, pre-PETIR) | **A6** | Edwards experiment |
|---|---|---|---|
| GS-1 RMSE vs data (16 pts, 0–0.30 s) | 58.6 psia | **42.8 psia** | — |
| GS-1 flashing plateau (0.02–0.06 s) | 392.7 psia | **359.0 psia** | ≈ 350–367 psia |
| peak break flow | 127.6 lbm/s | 125.3 lbm/s | — |
| GS-1 p at `t_end` | 26.4 psia | 13.5 psia | — |
| cold-tail artefact | absent | absent (min T 366.6 K) | — |

**RMSE against the experiment falls 27 %**, and the flashing plateau moves from
392.7 psia — *above* the measured band — to 359.0 psia, *inside* it. A term
restored for correctness, not tuned for agreement, improved agreement. That is
the outcome a physically-motivated fix is supposed to produce, and it is
evidence the term belongs there independently of the stability argument.

### But the run is NOT clean, and the new counter is what says so

```
pressure-bound events    : 150 (worst undershoot 1.7175e5 Pa, worst overshoot 0.0000e0 Pa)
```

150 cell-updates where the pressure solve left the EOS range, undershooting
`p_min` by as much as **171.75 kPa**. Without the counter (fix 5, added in the
same change) this would have been reported as an unqualified pass — which is
precisely the failure mode that let the original defect hide.

**So fix 1 is a large improvement and is not sufficient.** The pressure
equation still produces physically impossible states; the run now survives them
because the clamp reshapes them and nothing downstream happens to blow up.

### Where this leaves the diagnosis

- The **checkerboard was real and `ddtCorr` was its cure** — restoring the
  transient Rhie–Chow coupling fixed the 30 µs failure outright.
- The **A5 refutation still stands.** The `Δt` trend did not point at
  `ddtCorr`, and I was wrong to predict it would. The term turned out to be the
  fix anyway, reached by the upstream-omission audit rather than by the
  stability argument — which is a point in favour of the register, not of the
  prediction.
- **A residual stiffness defect remains**, size 150 events / 171.75 kPa. The
  `psi`-linearisation-window hypothesis from A5 is untouched by this result and
  is the next candidate, now testable against a *passing* baseline instead of a
  crashing one.

### Still open, not to be lost

- The A3 enthalpy hold is still in the build. With the checkerboard gone it may
  never fire; if so it should be reconsidered per the band-aid register.
- A6 has not been separated from A3. A clean `ddtCorr`-only run (hold removed,
  30 µs) is needed before crediting either alone.
- The golden reference is **not** reproduced by A6, and should not be: A6 is a
  deliberately different — and better — discretisation. The golden reference
  remains the record of the pre-PETIR platform-libm trajectory, nothing more.

| # | Configuration | Result | Notes |
|---|---|---|---|
| A6 | A3 + `fvc::ddt_corr` at 30 µs | **PASS** 390.75 s | RMSE 58.6 → 42.8 psia; plateau into the measured band; **150 bound events remain** |
