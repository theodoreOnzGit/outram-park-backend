# Edwards blowdown on `TampinesSteamArray` — a development-history and debugging write-up

**Audience.** A maintainer who has to keep this solver working but is *not*
confident in the two-phase thermodynamics or the finite-volume numerics behind
it. This document explains, from the ground up, what was broken in the
`TampinesSteamArray` blowdown solver, how each fault was diagnosed, which
physical and numerical concepts the fixes rely on, and why the fixes work. It is
deliberately slow and explanatory. Nothing here is a spec; the code and the
in-line derivation comments are the source of truth (see
[Further reading](#further-reading)).

The work described spans three sessions of debugging, captured tersely in
`collaboration/edwards_tampines_regen/{debug_log,hybrid_debug_log,hybrid_stability_debug_log}.md`
and released as README changelog entries **v0.2.3** and **v0.2.4**. The
corresponding beads are `op-21g.14` (the plateau fix), `op-21g.15` (the all-Mach
hybrid), and `op-21g.15.7` (the stability fix).

---

## 1. The problem in one picture

### What is being simulated

The **Edwards–O'Brien pipe blowdown** (Edwards & O'Brien, 1970) is *the*
textbook two-phase depressurisation benchmark. A long horizontal pipe is filled
with hot, high-pressure **subcooled** water (about 505 K, 7 MPa — a liquid held
below its boiling point only because it is under pressure). One end is sealed by
a rupture disc. At $t = 0$ the disc bursts, the pipe is suddenly open to the
atmosphere, and the water blows down. Pressure transducers ("gauge stations"
GS-1 … GS-7) along the pipe record the pressure history. GS-1 is at the closed
end, farthest from the break.

`TampinesSteamArray` is a one-dimensional finite-volume solver — the pipe is cut
into 24 control volumes ("cells") in a row — that marches this transient forward
in time. It is a **HEM-closed rhoPimpleFoam** solver; both of those terms are
unpacked in the concept primer below.

### The shape we must reproduce: the flashing plateau

When the disc bursts, a rarefaction (decompression) wave races down the pipe and
the local pressure drops. The moment the pressure falls to the **saturation
pressure** of the hot water, the liquid starts to **flash** — to boil
explosively into steam. Here is the key physical fact:

> Boiling absorbs a large amount of **latent heat**. As long as liquid is still
> flashing into vapour, that latent-heat sink holds the fluid *pinned* on its
> saturation line, and the pressure cannot fall much below the saturation
> pressure. On a pressure-versus-time plot this shows up as a **plateau** — the
> pressure stalls near $p_{sat}(T_0) \approx 350$ psia and holds there for tens
> of milliseconds before the pipe finally empties and the pressure decays.

That ~350 psia plateau is the signature of the physics. The digitised benchmark
("Data", GS-1) is roughly:

| $t$ (s) | 0.02 | 0.04 | 0.06 | 0.10 | 0.20 | 0.30 |
|---|---|---|---|---|---|---|
| $p$ (psia) | 350 | 364 | 367 | 312 | 289 | 190 |

A solver that gets the physics right reproduces that stall. A solver that gets it
wrong does not — and *how* it fails tells you what it got wrong.

### The symptom, stage by stage

The story has three failure modes, each fixed in turn:

1. **Baseline (as-found): the pressure collapsed to almost zero.** The plateau
   was 17 psia instead of ~350 (RMSE 276 psia against the benchmark). The
   pressure fell straight *through* the saturation line without stalling, and the
   **void fraction** (the vapour volume fraction) jumped almost instantly to 1.0.
   In plain terms: the model let the water **decompress far past where it should
   have started boiling** — it went *subcooled in the wrong direction* — and
   flashed to nearly pure steam near atmospheric pressure instead of holding the
   plateau. This is the fault tracked by bead `op-21g.14`.

2. **After the plateau fix: the solver rang.** With the plateau recovered, the
   pressure traces developed a high-frequency **ringing** (oscillation) riding on
   top of the smooth blowdown, worst at the near-break and interior gauges. The
   plateau was right, but the trace was noisy. This motivated the *all-Mach
   hybrid* (bead `op-21g.15`).

3. **The first hybrid crashed.** The ringing-damping hybrid worked over the
   first 0.15 s but then **panicked** at $t \approx 0.18$ s: a nearly-empty cell
   in the middle of the pipe rarefied to near-vacuum and the solver drove its
   state off the edge of the steam-table's validity range. This is bug
   `op-21g.15.7`.

The rest of this document explains the concepts, then walks each stage.

---

## 2. Concept primer

Short, plain-language explanations of every idea the debugging trail leans on.
Each ends with *why it matters here*.

### 2.1 The flashing plateau = saturation-pressure pinning by latent heat

Water boils when its pressure drops to the saturation pressure for its
temperature. Boiling consumes **latent heat of vaporisation** (roughly 1.7–2.0
MJ per kg of water turned to steam at these conditions). During a blowdown there
is no external heater; that latent heat can only come from the fluid's own
internal energy. So the fluid cannot simply keep decompressing — every attempt to
drop the pressure below $p_{sat}$ instantly boils a little more liquid, and the
energy that boiling demands is drawn back out of the pressure/temperature state,
which **pushes the pressure back up to $p_{sat}$**. The result is a
self-regulating stall: the pressure sits at $p_{sat}(T)$ and the temperature
tracks it down the saturation curve. That stall *is* the plateau. *Why it
matters:* if a solver's energy bookkeeping lets enthalpy drain away too fast, the
latent-heat sink is starved, nothing pushes the pressure back up, and the plateau
never forms — exactly the baseline failure.

### 2.2 Conservative vs non-conservative energy discretisation, and the `h·∇·φ` cancellation

The energy equation this solver advances is, in continuous form,

$$\frac{\partial (\rho h)}{\partial t} + \nabla \cdot (\phi h) = \frac{dp}{dt}$$

where $\rho$ is density, $h$ is specific enthalpy (energy per kg), and $\phi$ is
the mass flux (the mass flowing across cell faces per unit time). The first term
is the rate of change of energy stored in a cell; the second is energy carried in
and out by flow; the right side is reversible pressure work.

There are two ways to discretise (turn into arithmetic on a grid) the storage
term $\partial(\rho h)/\partial t$:

- **Conservative:** $\frac{\rho\, h - \rho_{old}\, h_{old}}{\Delta t}$ — density
  *and* enthalpy are taken at the new and old time levels together.
- **Non-conservative (the bug):** $\rho \frac{h - h_{old}}{\Delta t}$ — the
  *same* (current) density multiplies both time levels, so it is really
  $\rho\,\partial h/\partial t$, and the piece
  $h_{old}(\rho - \rho_{old})/\Delta t$ is silently missing.

Why the missing piece is fatal: continuity (conservation of mass) says
$(\rho - \rho_{old})/\Delta t = -\nabla\cdot\phi$. The conservative storage term
therefore contains a hidden $-h\,\nabla\cdot\phi$ contribution that **cancels**
the $+h\,\nabla\cdot\phi$ piece buried inside the flow term $\nabla\cdot(\phi h)$.
When that cancellation happens, the whole equation collapses to the physically
correct material derivative

$$\rho \frac{Dh}{Dt} = \frac{dp}{dt} \quad\Longrightarrow\quad dh \approx \frac{dp}{\rho},$$

a *small, reversible* enthalpy change that keeps the fluid gliding down the
saturation dome — the plateau. If you use the non-conservative form, the
cancellation does not happen. During the violent flash the break region has
strong net outflow ($\nabla\cdot\phi \gg 0$), so the un-cancelled $+h\,\nabla\cdot\phi$
becomes a large spurious enthalpy *sink*: the model **over-drains enthalpy**, the
liquid goes subcooled, and it only flashes near atmospheric pressure. *Why it
matters:* this single discretisation choice is the entire baseline failure.

### 2.3 Compressibility `ψ = ∂ρ/∂p`, and why the fixed-enthalpy two-phase form is the right one

A pressure-based solver needs to know how much the density changes when the
pressure changes — the **compressibility** $\psi = \partial\rho/\partial p$. This
number sets the "stiffness" of the pressure equation: it is the diagonal
coefficient $\psi\, V/\Delta t$ that tells the solver how hard the pressure
resists being changed by a given mass imbalance.

But $\partial\rho/\partial p$ *at constant what?* The answer depends on the
algorithm. This is a **segregated** solver: within a pressure-correction inner
iteration the enthalpy $h$ is held **frozen** (it is only updated afterward by the
energy equation). So the density's response to a pressure change during the
pressure solve is the **fixed-enthalpy** derivative

$$\psi = \left.\frac{\partial \rho}{\partial p}\right|_h.$$

Inside the two-phase dome this is enormous compared to the naive isothermal value.
The reason is the **flashing term**. In two-phase equilibrium, density is
governed by the quality $x$ (mass fraction vapour): $v = (1-x) v_f + x\, v_g$
(specific volume of liquid and vapour). When pressure drops at fixed enthalpy,
the equilibrium quality $x$ *changes* — more liquid flashes — and that adds a
term $(v_g - v_f)\,dx/dp$ to the compliance. The frozen, quality-weighted
*isothermal* compressibility used originally, $\kappa_T = x\,\kappa_{vap} +
(1-x)\,\kappa_{liq}$, **omits the flashing term entirely**. Numerically it was
about **100× too small** in the dome. *Why it matters:* the flashing compliance
is precisely what pins the pressure on the saturation line. With a compressibility
100× too small, the pressure equation thinks the fluid is nearly incompressible
and lets the pressure overshoot straight through the plateau.

### 2.4 HEM equilibrium sound speed vs the frozen Wood–Wallis speed

"HEM" = **Homogeneous Equilibrium Model**: the liquid and vapour are assumed to
move at the same velocity (*homogeneous*) and to be in thermodynamic equilibrium
(*equilibrium* — the phases share one temperature and sit on the saturation
line). The alternative closures relax one of those.

Two-phase mixtures have a famously low and non-intuitive **sound speed** — it can
dip to a few tens of m/s, well below either pure phase. But there are two
different two-phase sound speeds:

- **Equilibrium (HEM) sound speed:** the phases re-equilibrate (re-flash) as the
  acoustic wave passes. This is the physically correct speed for a slow,
  equilibrium process, and it is what this solver uses everywhere — computed from
  the Kieffer (1977) equilibrium relation `w_ps_eqm_region4_kieffer` in the dome,
  with the region forward speeds in single phase.
- **Frozen / Wood–Wallis sound speed:** the phase fractions are held fixed
  ("frozen") as the wave passes — no re-flashing. This is a different, generally
  higher number.

*Why it matters:* a maintainer directive on `op-21g.15` requires this solver be
**HEM through and through** — the same equilibrium sound speed feeds the regime
Mach number, the shock-capturing wave speeds, and the choked-flow break boundary.
Mixing in a frozen speed would be physically inconsistent with the equilibrium
energy and continuity equations the rest of the solver uses.

### 2.5 Pressure-based (rhoPimpleFoam) vs density-based (rhoCentralFoam), and why a pressure solver *rings*

There are two families of compressible-flow solvers:

- **Pressure-based (rhoPimpleFoam / PIMPLE):** treats the pressure implicitly
  through a pressure-correction equation (the "P" in PIMPLE). It is excellent for
  low-Mach and stiff-acoustic flows because it does not have to take tiny time
  steps to resolve fast sound waves — the acoustics are handled implicitly. Its
  weakness: at a **sharp, near-sonic front** (like the flashing shock) an implicit
  pressure solver has little numerical dissipation there and tends to **ring** —
  produce spurious oscillations — because it is trying to represent a
  near-discontinuity on a coarse grid with a centred, low-dissipation scheme.
- **Density-based (rhoCentralFoam / KNP):** marches the conserved variables
  $[\rho, \rho U, \rho E]$ explicitly using a **central-upwind** flux (the
  Kurganov–Noelle–Petrova, "KNP", scheme). It has built-in **numerical
  viscosity** proportional to the local wave speeds, which is exactly what damps
  ringing at a shock — that is what shock-capturing schemes are for. Its weakness:
  it needs small time steps and is clumsy in stiff low-Mach regions.

*Why it matters:* the plateau-fixed PIMPLE solver was correct but rang at the
flashing front. The natural remedy is to borrow the KNP scheme's shock-capturing
dissipation *only* at the front, while keeping PIMPLE everywhere else — a hybrid.

### 2.6 Mach-based blending

You do not want the KNP dissipation switched on everywhere — that would smear the
smooth parts of the solution and defeat the point of using an implicit pressure
solver. You want it *only* near the sonic front. The **Mach number** $Ma = |u|/c$
(flow speed over sound speed) is the natural regime indicator: it is small in the
quiescent bulk and approaches 1 at a near-sonic front. So the hybrid **blends**
the two schemes with a weight

$$\beta(Ma) = \mathrm{clamp}\!\left(\frac{Ma - lo}{hi - lo},\, 0,\, 1\right),$$

defaults $lo = 0.3$, $hi = 1.0$. At $\beta = 0$ (subsonic) the added dissipation
is **identically zero**, so the default PIMPLE path is bit-for-bit unchanged; at
$\beta = 1$ (near-sonic) the full KNP shock-capturing is active. *Why it matters:*
this is what lets the hybrid be an *opt-in, additive* correction that cannot
disturb the validated PIMPLE result away from the front.

### 2.7 The rarefied empty-cell runaway

Late in the transient the pipe is nearly empty. A cell's density can fall toward
vacuum. Two things then conspire. First, the conservative energy equation's
time-derivative diagonal is $\rho_{cont}\, V/\Delta t$; as $\rho_{cont}$ collapses
to its numerical floor, that diagonal vanishes and the linear solve for enthalpy
becomes **ill-conditioned** — a tiny flux imbalance produces an unbounded change
in $h$. Second, an *explicit* shock-capturing correction (the KNP deferred term)
evaluated on a nearly-empty cell can over-drive it. Together they can tip a cell's
$(p, h)$ state below the steam table's validity edge (273.15 K), where the
`(p,h)` flash panics. *Why it matters:* this positive-feedback density collapse is
the crash at $t \approx 0.18$ s — and the fix is to recognise that there is *no
shock to capture* in the near-vacuum tail and simply stop applying the correction
there.

---

## 3. The debugging trail, as a story

The dead ends are kept in, because *why* they failed is the most instructive part.

### Part A — recovering the flashing plateau (bead `op-21g.14`, README v0.2.3)

**Iteration 0 — baseline, reproduce the failure.** With the as-found solver the
GS-1 plateau came out at **17.4 psia** (target ~350), RMSE 276 psia. The trace
collapsed 1015 → 14 psia by $t = 0.03$ s and flat-lined; void jumped to ~1.0
almost instantly. Reading the code before touching anything, the energy equation
was assembled with `fvm::ddt_coeff(&self.rho, &self.he, &he_old, dt)` — which
uses the *same* current density for both time levels, i.e. the **non-conservative**
$\rho\,\partial h/\partial t$ form from §2.2. The diagnosis was written down before
any edit: the missing $h_{old}(\rho - \rho_{old})/\Delta t$ term means the
$h\,\nabla\cdot\phi$ cancellation never happens, so enthalpy over-drains during the
flash and the liquid subcools. This is a **structural** (conservation) error, not
a tuning problem.

**Iteration 1 — H1: PISO relaxation (a deliberate dead end, to prove the point).**
Before doing surgery, the cheap thing was tried first: crank up the pressure–
velocity iteration (4 outer / 4 inner PISO correctors, no under-relaxation)
instead of the baseline (2, 3, 0.5, 0.7). Result: plateau 17.4 → **35.7** psia.
Essentially nothing. *This dead end was the point:* it confirmed the failure is
**structural energy conservation**, not the pressure–velocity iteration. No amount
of iterating a wrong equation fixes it. (The stronger PISO config was kept anyway
— it is the right choice for a fast transient.)

**Iteration 2 — H2a: conservative ddt with the *EOS* density (FAILED, instructively).**
A new operator `fvm::ddt_coeff_old(coeff_new, coeff_old, φ, φ_old, dt)` was added
to build the proper conservative form $(\rho\, h - \rho_{old}\, h_{old})/\Delta t$,
and the energy equation switched to use it with the **EOS density** `self.rho`
(the density `correct_thermo` writes from the $(p,h)$ flash). This **panicked
almost immediately** — a cell's $(p,h)$ flashed into Region 5 ($T > 1073$ K), i.e.
the fix now *over-heated* some cells. Why: the EOS density is not
continuity-consistent with the mass flux `self.phi`. Mid-flash the EOS density
drops faster than the $\psi\, dp/dt$ the pressure equation feeds into $\phi$, so
$(\rho_{eos} - \rho_{old})/\Delta t \ne -\nabla\cdot\phi$; the leftover shows up as
a spurious energy *source*. The lesson: for the cancellation to hold, the density
in the storage term must be the one that *exactly* satisfies discrete continuity —
not the thermodynamic one.

**Iteration 3 — H2b: conservative ddt with the *continuity* density (FIX #1).**
The energy equation now builds

$$\rho_{cont} = \rho_{old} - \Delta t\, \nabla\cdot\phi \quad(\text{floored at } 10^{-4})$$

from the *final* mass flux, and uses `ddt_coeff_old(&rho_cont, &rho_old, …)`. Now
$(\rho_{cont} - \rho_{old})/\Delta t = -\nabla\cdot\phi$ holds **exactly**, the
$h\,\nabla\cdot\phi$ cancellation is term-for-term, and the energy equation reduces
to the material derivative $\rho\, Dh/Dt = dp/dt$. Result: plateau 17.4 → **41.0**
psia; break-flow peak a more physical 55 → 125 lbm/s (experiment ~96–111); stable
600 ms; no over-heat. And a big **shape** change: GS-1 now correctly held subcooled
through the early decompression, tracked the benchmark down, and **flashed at the
right pressure (~380 psia)** — *then overshot the plateau*, collapsing to a ~96
psia two-phase plateau instead of holding at 350. The energy over-drain was cured
(flash now initiates correctly), but a **second, separate** fault remained: once a
cell flashes, the pressure overshoots *through* the saturation plateau. That is not
an energy problem — it is a **compressibility** problem (§2.3).

**Iteration 4 — H3: `ψ = ∂ρ/∂p|_h` by finite difference (FIX #2).** In
`correct_thermo`, the compressibility was changed from the frozen quality-weighted
isothermal $\rho\,\kappa_T$ to a central finite difference of the real $(p,h)$
flash at **fixed enthalpy**:

$$\psi = \frac{\rho(p + \delta p,\, h) - \rho(p - \delta p,\, h)}{2\,\delta p},
\qquad \delta p = \max(10^{-3} p,\ 50\ \text{Pa}),$$

with $p \pm \delta p$ clamped to the valid pressure range (falling back to the
isothermal value only at the degenerate clamped edge). This is $\partial\rho/\partial p|_h$
— the correct linearisation for a segregated solver that freezes $h$ during the
pressure solve. In single phase it agrees with the old value (so subcooled and
superheated behaviour is unchanged); inside the dome it is ~100× larger because
the $(p,h)$ flash re-solves the equilibrium quality at each perturbed pressure,
capturing the flashing compliance. Result: **plateau 392.7 psia** (target
350–367; was 41), **RMSE 59.8 psia** (was 273). GS-1 now holds 380–394 psia
through the flashing window, then declines; void rises **gradually** 0 → ~0.8 with
no instantaneous jump and no decompression past saturation.

**Which hypothesis was it?** Both structural fixes were required and are
complementary: H2b restores enthalpy conservation so the flash *initiates* at the
right pressure; H3 restores two-phase compliance so the pressure is *pinned* at the
plateau instead of overshooting. H1 (relaxation) alone did essentially nothing,
proving the problem was structural. Crucially, **no clamps, floors, or saturation
clips were added** — the plateau emerges purely from the corrected thermodynamics.

### Part B — the all-Mach hybrid to damp the ringing (bead `op-21g.15`, README v0.2.3)

With the plateau recovered, the traces rang at the near-sonic front. The design
(§2.5–2.6): keep the pressure-based PIMPLE skeleton untouched and add the KNP
central-upwind dissipation as a **Mach-weighted deferred-correction flux**, gated
by `SolverMode { Pimple (default), HybridAllMach }`. The dissipation on each
internal face is $\beta\,(\text{KNP} - \text{central})\cdot|S_f|$, where "central"
is the same KNP flux with its jump (dissipation) term zeroed — so the difference is
the *pure* numerical viscosity, and at a uniform field it is identically zero. The
sound speed for both the Mach number and the KNP wave speeds $a = u \pm c$ is the
HEM equilibrium speed throughout (§2.4). The KNP flux math was copied in-tree into
`central_upwind.rs` (OpenFOAM provenance header preserved, no `outram-foam`
dependency). A naive first port crashed the Edwards run; three findings fixed it.

**B-1 — dissipate static enthalpy `ρ·he`, not total energy `ρE`.** The upstream
density-based scheme transports total energy $\rho E = \rho\, h_{tot} - p$, which
carries the pressure work $-p$. But this array's segregated energy equation
advances **static** enthalpy $\partial(\rho h)/\partial t + \nabla\cdot(\phi h) =
dp/dt$ with the pressure work already handled by the separate $dp/dt$ source.
Injecting a $\rho E$ dissipation re-injects that $-\Delta p$ pressure work a second
time; across the strong rarefaction the large $\Delta p$ then **over-cooled** the
near-break cell straight through the 273.15 K isotherm (a `(p,h)` panic). Fix: the
conserved energy variable for *this* system is static $\rho\cdot he$.

**B-2 — gate on `min(Ma)` of the two adjacent cells, not `max(Ma)`.** A diagnostic
showed the added mass flux was 5–35× the physical flux even at tiny $\beta \approx
0.09$. Cause: at a **liquid / two-phase interface** the liquid side's sound speed
(~1400 m/s) dominates the KNP wave speeds, so the numerical viscosity
$a_L a_R / da \sim c_{liq}/2 \sim 500$ is huge — but that liquid acoustic wave is
genuinely **low-Mach** ($|u|/c_{liq} \ll 1$) and must *not* be dissipated. Gating
$\beta$ on `max(Ma)` let the two-phase side's high Mach trigger dissipation of the
low-Mach liquid wave. Switching to `min(Ma)` sees the subsonic liquid side and
returns $\beta = 0$ there, activating the KNP dissipation only where **both** sides
are near-sonic — the developed two-phase front, where $c$ is uniformly small and
the viscosity magnitude is physical. After the fix the added flux dropped to a
physical 1.5–5 kg/s.

**B-3 — do *not* add a separate energy dissipation source (it double-counts).**
With `min(Ma)` and $\rho\cdot he$, dissipating continuity + momentum was stable and
matched pure PIMPLE in the bulk while retaining the plateau. But re-adding *any*
explicit energy source — even a gentle enthalpy-gradient diffusion — either
over-cooled below 273.15 K or, when scaled down to stay stable, over-damped and
**suppressed the physical flashing front**. Root cause: the continuity dissipation
is folded into $\phi$ *before* the energy equation recomputes $\rho_{cont}$ and its
convective term, so $\nabla\cdot(\phi h)$ already transports the dissipative
enthalpy — the plateau-fix cancellation $(\rho_{cont} - \rho_{old})/\Delta t =
-\nabla\cdot\phi$ carries it for free. A standalone energy source double-counts that
and breaks the balance. **Decision:** energy shock-capturing rides on the
continuity dissipation; no separate energy source.

**B-4 — a validity-edge guard for the long tail.** As the pipe empties, interior
cells drift toward the $(p,h)$ validity edges where the HEM closure is undefined.
Faces touching a cell whose temperature is within a margin of the 273.15 K /
1073.15 K isotherms (guard band $[300\text{ K}, 1050\text{ K}]$) get **no**
dissipation. This hardens the long run without touching the ringing phase (whose
front cells are ~490–500 K), and the 0–0.15 s numbers are bit-identical with and
without it.

**Result over 0–0.15 s.** Ringing is measured as *excess total variation* — the
wiggle on top of the monotone blowdown. Summed over the gauges it dropped
**4670 → 2079 psia, −55.5 %**, with the largest cuts at the near-break/interior
gauges GS-2 (−67 %), GS-3 (−70 %), GS-1 (−60 %). The plateau was retained (GS-1
mean 392.7 → 387.7 psia). The default `Pimple` path stayed bit-identical by
construction (subsonic faces get exactly zero added flux). **But** the full 600 ms
run still crashed at $t \approx 0.18$ s — leading to Part C.

> **A cautionary note from this session.** Running `rustfmt src/lib.rs` during
> cleanup **recursed through `mod` declarations and reformatted the entire
> crate** (rustfmt follows `mod x;` unless `--skip-children`). Reverting those
> unintended changes with `git checkout` also discarded two pieces of
> *uncommitted* working-tree work, which had to be reconstructed and re-verified.
> The lesson recorded for maintainers: never run a bare `rustfmt` on a module
> root, and sanity-check `git status` against your own in-flight notes.

### Part C — stabilising the hybrid over the full transient (bug `op-21g.15.7`, README v0.2.4)

**The symptom.** `HybridAllMach` panicked at `ph_flash_eqm/mod.rs:837`
("p,h point below 273.15K") at $t \approx 0.186$ s (step ~6240, $\Delta t = 30$
µs). The 0–0.15 s comparison test was already green.

**Diagnosis (env-gated instrumentation, hybrid-only so PIMPLE stayed
bit-identical).** Four findings:

1. *The failing cell.* The panic was a single-step catastrophic swing, not a slow
   drift: cell 14's enthalpy went $950.8$ kJ/kg $\to -7.32\times10^6$ kJ/kg in one
   energy solve, at $p = 611$ Pa (the triple-point floor), $\rho_{cont} = 10^{-4}$
   (its clamp floor), $\rho \approx 0.012$ kg/m³ — i.e. the energy equation acting
   on a **nearly-empty** cell whose time-derivative diagonal $\rho_{cont} V/\Delta t$
   had collapsed to its floor, making the solve ill-conditioned (§2.7).

2. *How cell 14 got there.* A trajectory dump showed a **localised implosion**:
   cell 14 rarefied in a runaway from $\rho = 127$ kg/m³ (step 5900) through 108,
   78, 23, to vacuum over ~350 steps while its neighbours stayed dense. At collapse
   the total face flux was ~100 kg/s versus a physical break flow of ~35 kg/s — the
   KNP dissipation was *inflating* the flux.

3. *Which component.* Disabling continuity dissipation *or* momentum dissipation
   *each individually still panicked* (momentum-only failed faster). So it was **not
   a sign error** in one term — it was the explicit KNP deferred correction *as a
   whole* being over-driven in the rarefied tail (a stiffness problem, not
   anti-diffusion).

4. *Where the dissipation legitimately acts.* Instrumenting the minimum
   dissipated-face density over the 0–0.15 s ringing window gave **≈ 106.5 kg/m³**:
   every face that contributes to the ringing reduction sits at $\rho \gtrsim 106$
   kg/m³ (the dense flashing front). The runaway lives entirely at $\rho < 100$
   kg/m³. **A density threshold cleanly separates the two.**

**The fix — a rarefied-tail density taper.** In `assemble_hybrid_dissipation`,
after the `min(Ma)` blend, multiply $\beta$ by

$$g(\rho_{face}) = \mathrm{clamp}\!\left(\frac{\rho_{face} - 50}{100 - 50},\, 0,\, 1\right),
\qquad \rho_{face} = \min(\rho_{owner}, \rho_{nb}),$$

with `HYBRID_RHO_TAPER_LO = 50`, `HYBRID_RHO_TAPER_HI = 100` kg/m³. Below 50 the
KNP dissipation is **zero** (rarefied tail ⇒ pure PIMPLE, which is stable over the
full transient); above 100 it is at **full weight** (dense front, untouched); a
linear ramp between. The **physical justification** is not a fudge: the all-Mach
KNP scheme is designed for the *dense near-sonic flashing front*. In the emptying
tail the mixture rarefies toward vacuum, the HEM equilibrium closure degrades, and
**there is no flashing shock to capture** — an explicit deferred correction there
only over-drives a nearly-empty cell. Removing it in that regime is physically
appropriate. And because the minimum dissipated-face density in the
physics-of-interest window is 106.5 kg/m³ (above `ρ_hi = 100`), the taper is
**inert** there — the ringing reduction and plateau are bit-identical to the
untapered hybrid. A sweep confirmed the thresholds: a $[10, 40]$ taper still
panicked (the 40–100 band still dissipated the runaway), while $[50, 100]$ was
stable with identical ringing/plateau.

---

## 4. The result

Measured 2026-07-16, 24 cells, $\Delta t = 30$ µs.

| Quantity | Baseline | After plateau fix (`Pimple`) | After hybrid + stability fix (`HybridAllMach`) |
|---|---|---|---|
| GS-1 flashing plateau (psia) | 17.4 | 392.7 | 387.7 |
| GS-1 pressure RMSE vs Data (psia) | 275.8 | 59.8 | **30.6** |
| Near-sonic ringing (summed excess-TV, 0–0.15 s) | — | 4669.6 | 2079.3 (**−55.5 %**) |
| Full 600 ms | collapses to ~0 | stable | **stable** (no panic/NaN, void ∈ [0,1]) |
| Break-flow peak (lbm/s; exp. ~96–111) | 55 | 127.7 | 125.7 |
| Cold-tail artefact | — | absent | absent (min tail $T \approx 372$ K) |
| Default `Pimple` lib tests | — | 915 pass | **920 pass** / 0 fail |

Target benchmark plateau is ~350–367 psia. The `Pimple` path (392.7 psia) is the
validated, always-stable default; the stabilised `HybridAllMach` (387.7 psia, RMSE
30.6) is now *more accurate* than `Pimple` and is no longer flagged experimental.

**What is still imperfect (honest limitations).** These are secondary accuracy
items, *not* the qualitative failures that were the task:

- The recovered **plateau sits ~8 % high** (≈ 388–393 vs ~355 psia) — an
  HEM-closure / axial-profile accuracy item.
- The **late (0.1–0.3 s) pressure decline is too shallow** (e.g. 288 vs 190 psia at
  0.3 s in the `Pimple` run).
- The initial rarefaction drops to the plateau a touch faster than the
  acoustically-resolved benchmark (a mesh / CFL effect).
- **Residual void-fraction oscillations** persist in the depressurising tail — a
  minor secondary artifact.
- Metastable / non-equilibrium effects are outside HEM by construction and are not
  modelled.

As always in this workspace: these numbers are unverified against a wider set of
cases until validated; treat them as the measured result of *this* benchmark on
*this* date.

---

## 5. Further reading

- **In-code derivation.** The `rhoPimpleFoam` module `//!` doc comment and the
  detailed comment blocks in `src/openfoam_algorithms/rhoPimpleFoam/mod.rs`
  (`step`, `correct_thermo`, `assemble_hybrid_dissipation`) and
  `central_upwind.rs` are the authoritative derivation of every equation above —
  the conservative continuity-density ddt, the `∂ρ/∂p|_h` finite difference, the
  `min(Ma)` gate, and the density taper are each documented at their call site.
- **Module references.** `src/openfoam_algorithms/CLAUDE.md` lists the primary
  literature: Tomlinson & Aumiller (1999, B-T-3271), Edwards & O'Brien (1970),
  Hendrie (1973, axial IC), Schmidt, Gopalakrishnan & Jasak (2010, HRMFoam
  template), and De Lorenzo et al. (2017a, HEM + tabulated IAPWS-IF97) — the
  closest architectural precedent for TAMPINES-as-thermo. It also points to the
  companion stability primer,
  `crates/outram-foam-appbuilder-lib/src/solvers/rho_pimple_foam/docs/stability_a_students_guide.md`,
  which explains the pressure–velocity coupling failure modes this solver shares.
- **Raw debug logs.** The terse, blow-by-blow session records this narrative was
  built from live in `collaboration/edwards_tampines_regen/`:
  `debug_log.md` (plateau), `hybrid_debug_log.md` (all-Mach hybrid), and
  `hybrid_stability_debug_log.md` (full-transient stability).
- **Beads.** `op-21g.14` (plateau), `op-21g.15` (+ children `.1`–`.7`, the
  hybrid), `op-21g.15.7` (stability). README changelog **v0.2.3** / **v0.2.4**.
