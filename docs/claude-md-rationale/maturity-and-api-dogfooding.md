# Rationale: crate maturity, headless mode and API dogfooding

> Split out of the root `CLAUDE.md` on 2026-09-21 to keep that file under the
> 150k-character context limit. **This is the full original text, verbatim**,
> including the maturity roster's per-crate evidence, the six honest notes,
> and the measured Opus API-guessing table.

## Every egui simulator ships a headless mode (HARD RULE)

**Any example or binary with an egui/eframe GUI MUST also provide a headless
execution path** that runs the underlying model with no window, no event loop
and no GUI thread, and emits a machine-readable trace on stdout.

```
cargo run --release --example <sim> -- --headless [steps] [sample_every]
```

**WHY: a GUI-only simulator cannot be tested, and cannot be trusted.** An agent
or a CI job cannot open a window, so without this the model can only be checked
by a human watching it — which means in practice it is not checked at all. Every
claim about what the simulator does becomes unfalsifiable.

It also makes a specific, recurring class of bug invisible. `htgr_sim_v1`'s
`PlantCommands::default()` is documented as starting *"near steady state rather
than on a prompt excursion"*. The first headless run ever taken of it (2026-09-06)
showed power **overshooting to 27.8 MW — roughly 2.8x nominal — before settling
near 8.1 MW**, with the bed temperature still drifting downward 1200 s in. The
docstring was wrong and had been wrong unnoticed, because nobody could run the
thing without watching it.

**Requirements:**

- **No GUI, no window, no event loop, no spawned physics thread.** The headless
  path drives the model directly.
- **Deterministic.** No wall clock, no RNG seeded from time, no I/O inside the
  loop. Same config in, byte-identical trace out. Assert this in a test — it is
  the property every committed fixture depends on.
- **Machine-readable output**, CSV or equivalent, with a stable header and fixed
  precision so a committed fixture diffs cleanly.
- **A regression test that calls the headless path directly**, not through the
  GUI.
- Where the model can leave physical range, assert bounds in that test. Loose
  bounds that catch divergence are worth far more than none; **this is a
  harness check, not physics V&V, and must not be described as validation.**

**This is a precondition for declaring any simulator's behaviour, not an
optional convenience.** A simulator without a headless mode has no reference
baseline, so it cannot be refactored safely and cannot be shown to still work
afterwards.

Tracked: `op-otiy`.

### When this applies: only to crates declared mature (HARD RULE)

**The dogfooding rule below is a hard rule for every crate the maintainer has
declared mature, and is not enforced on any crate before that.** Both halves
bind: do not skip it on a mature crate, and do not demand it of one still
finding its shape.

**API polish comes after the internals are shown to be reasonably accurate,
never before.** An interface is a commitment to a shape, and shaping an
interface around physics that is still moving means paying for the same
interface twice — and the wheel, the stubs, the examples and the codegen
registry all move with it. Worse, an API that is pleasant to call and quietly
wrong is more dangerous than one that is awkward, because the ergonomics
invite trust the numbers have not earned.

**What "reasonably accurate" means here.** A crate becomes eligible when its
internals are backed by at least one of:

- **Analytical or manufactured solution.** Agreement with a closed-form
  result, or an MMS convergence study showing the expected order of accuracy.
  This proves the numerics.
- **Cross-code comparison.** Agreement with an established code (OpenMC,
  Serpent, MOOSE, OpenFOAM, NJOY) on the same input, where no closed form
  exists.
- **Unit tests and internal consistency.** Conservation, symmetry, reciprocity
  and limiting-case invariants holding under test. Necessary always, and
  sufficient on its own only for crates with no physics to get wrong
  (utility, I/O, tooling).

**Published-benchmark agreement is deliberately not on that list.** Matching
HTR-10, MSRE or ICSBEP is the *goal* of the validation pipeline, and that
pipeline is driven through the very API in question — so requiring it first
would deadlock the work. Benchmark agreement is the reward for a good API, not
its entry fee. (Rationale recorded 2026-09-05; revisit if it proves too
lenient.)

**The tolerance is per-crate, and lives in that crate's own `CLAUDE.md`.**
There is no workspace-wide number: 500 pcm means something in `outram-mc-libs`
and nothing in `outram-park-fork-dwsim-libs`. Each crate states its own bar,
the reference it is measured against, and the evidence class above that it
claims. A crate with no such statement is by definition not yet mature.

**Who declares it.** The maintainer, and only the maintainer, marks a crate
mature. An agent that believes a crate has cleared its bar may *propose*
maturity — open the issue in both trackers, cite the specific runs and
numbers, and stop there. Do not flip the flag, and do not begin enforcing the
dogfooding rule on the strength of your own assessment.

**The bar moves, and that is expected.** Standards tighten as the physics
firms up and as reference data improves. Record every revision as a dated
entry in the crate's own `CLAUDE.md`, keeping the superseded ones:

```
- 2026-09-05 — mature. Bar: k-eff within 500 pcm of a cross-code OpenMC run
  on the same ENDF/B-VIII.0 evaluation. Evidence: cross-code comparison.
- 2026-11-xx — bar tightened to 200 pcm now that the scatter matrix is
  verified; the 500 pcm entry above stands as what was accepted before.
```

Keeping the history matters more than it looks: an agent reading the crate
later needs to know not just today's bar but that it moved, or it will
misread older results as failures against a standard that did not exist when
they were produced.

**Declared mature as of 2026-09-15** (12 of 40 crates). The bar and its
evidence live in each crate's own `CLAUDE.md`; this roster is a pointer, not
the authority:

| crate | bar | evidence class |
|---|---|---|
| `tampines-steam-tables` | IF97 region eqns to 1e-8 vs IAPWS values; flash tables looser (0.5% vol, 8% λ) | reference standard |
| `tuas_boussinesq_solver` | CIET outlet temp within 0.2 °C; Gnielinski 2% | experimental + cross-code |
| `outram-foam-appbuilder-lib` | Sod (1978) Table II vs exact Riemann; L2 within 5% of peak | analytical / MMS |
| `chem-eng-real-time-process-control-simulator` | discretisations exact at samples vs closed form | analytical (+ Scilab, dissertation) |
| `outram-foam-basic-lib` | conservation to 1e-12; convergence order matches theory | analytical / MMS |
| `njoy-outram-park-fork` | agrees with NJOY2016 to 7 significant figures | cross-code |
| `outram-mc-libs` | k-eff within 500 pcm of ICSBEP Godiva | cross-code |
| `teh-o-prke` | published β reproduced; PRKE limiting cases exact | unit + consistency |
| `outram-park-fork-liggghts` | integrator + contact laws vs closed form; **plus** agrees with upstream LIGGGHTS-PUBLIC `3d5c00f2` compiled and run — ~~3 of 4~~ **4 of 6 deterministic cases** bit-identical throughout (**CORRECTED 2026-09-17**: there are six, and *both* oblique cases agree to 1-3 ulp rather than one), bulk bed packing fraction within 0.20 %, and — omitted entirely before — the **HTR-10 full core** at `D/d = 30`: `φ` to four decimals, median pebble **61 µm** from LIGGGHTS' over 27 554 pebbles, plus the **conus/discharge mesh geometry** to `φ` within 0.01 % with all 27 554 pebbles within 1 mm at early time; **granular physics still NOT validated (no experimental comparison)** | analytical / MMS + cross-code |
| `farrer-park` | MMS L2 order within 0.15 of theory per element; patch test 1e-12; Lamé **displacement** within 1%, **stress** on observed order (1 ± 0.2 linear, 2 ± 0.2 quadratic); **shear locking uncured, no benchmark validation** | analytical / MMS |
| `outram-park-fork-dwsim-libs` | agrees with upstream DWSIM `1abf72d1` to 4 sig figs; **PR EOS matches to 6 s.f. (measured 2026-09-13)**; flash-layer comparison still open | cross-code |
| `petir` | agrees with GSL 2.8 compiled and run — 5 of 7 numerics surfaces bit-identical throughout, the rest 75-97 % with worst relative difference 1.1e-15; ARM `exp`/`log`/`pow` bit-identical | cross-code |

Every other crate is **not** declared, and the dogfooding rule does not apply
to it.

**`petir` was declared on 2026-09-15**, by the maintainer, on the stated
condition "if it agrees with GSL". It does, and by the strongest mechanism on
this roster: GSL 2.8 was **built from the vendored tree and executed**, and its
output committed under `reference-data/gsl/` so the comparison regenerates
rather than being trusted.

One qualification belonged with the declaration and was closed the same day.
At the moment of declaration the compiled comparison covered `cheb`,
`expint`, `gamma_inc` and the `linalg` QR only; `roots`, `min`, `deriv`,
`interp`, `integration` and `ode` had GSL-derived tests but no compiled
reference. Those six were given one, and five of the seven surfaces came back
**bit-identical throughout**. The consolidated table — every surface, its
upstream, its measured agreement, and what is NOT covered — is
`crates/petir/docs/verification-summary.md`.

Six honest notes on this roster: `teh-o-prke` is the thinnest of the
twelve and lacks analytical transient validation (its own file says so, and
says what would fix it); `outram-mc-libs`' 500 pcm is now far looser
than what it achieves — as of 2026-09-15 Godiva sits at **+16 ± 11 pcm over 256
seeds**, 44 sigma inside the bar and inside ICSBEP's own ±100 pcm band, after
the discrete inelastic angular distributions were wired in (`op-tm9f`). The bar
was deliberately **not** tightened with that result; doing so is a maintainer
decision. Note also that agreeing on `k` is not the same as agreeing on the
physics: a ~69 pcm *spectral* residual against OpenMC is still open
(`op-os8x`), and the crate's own `CLAUDE.md` records it beside the result; the
Scilab half of the process-control crate's evidence lives in the maintainer's
dissertation rather than in this repository, so its recorded bar is written
against the analytical tests that *are* reproducible here; `farrer-park`
carries a **known uncured defect** — shear locking, measured at 66.7 % too
stiff on Quad4 at element aspect ratio 4 (`op-uqqg`) — so its maturity says
the numerics it *does* implement converge at the theoretical rate, not that
every element is fit for bending, and it is the only entry on this roster
declared within days of the crate first existing, with an analytical/MMS bar
and no cross-code or benchmark leg at all; and **`outram-park-fork-dwsim-libs`
has met its bar at the EOS layer only** — upstream DWSIM has since been built
and run headless (procedure in that crate's `CLAUDE.md`), and its `Z_PR` agrees
with this port to 6 significant figures. The flash-layer comparison the bar
also names is still open, and 0 of the 107 rows in its port-coverage matrix are
`PORTED + VALIDATED`. Read that crate's file before citing it as validated.
Finally, `petir`'s bar is cross-code agreement **alone** — nothing in it has
been compared against a published benchmark, and both of its bookkeeping axes
remain unsigned. The same caveat now applies to
`outram-park-fork-liggghts`, which gained a cross-code leg on 2026-09-15
(upstream LIGGGHTS built and run, `reference-data/liggghts/`): it reproduces
LIGGGHTS essentially exactly, but that says nothing about whether LIGGGHTS'
granular physics is right for an HTR-10 bed — there is still **no
experimental comparison**, and the settled voidage both codes produce
(`ε ≈ 0.442`) sits 2.2 percentage points above the Dixon (1988) correlation
for `D/d = 6`. Read that crate's `docs/verification-and-validation.md`
before citing it as validated.
### Verifying it: dogfood the API on a small model (HARD RULE)

> **If it is too complex for Haiku, it is a bad API.**

That is the standing test for **every crate declared mature** (see the gate
directly above — it is not asked of a crate still finding its shape), and it
is a test, not a slogan: a small model has no budget to read the source, so it can
only use what the interface itself makes discoverable — exactly like the human
this section already requires the API to serve. When it cannot get there, the
interface is wrong, however correct the physics underneath.

**Before claiming a public API is usable from Python, run it on a Haiku agent
with a fresh context and the wheel only — no repository access, no `.pyi`
pasted in, no source.** Count the round trips to a working script and log
every exception. That number is the usability measure; the exception log is
the fix list.

**WHY THIS EXISTS: capability masks usability defects.** A strong model is the
*worst* instrument for this, for the same reason the author of a tool is the
worst person to usability-test it — when the API is unguessable it reads the
Rust source and compensates, and the compensation is invisible in the result.
The finished script looks clean and the defect ships.

Measured on 2026-09-04, writing short scripts against this workspace's own
wheel **with** full source access and grep, an Opus session guessed wrong ten
times across six scripts:

| guessed | actual |
|---|---|
| `mc.Sphere(center, radius)` | `Sphere(x0, y0, z0, r, bc)` |
| `RegionToken.HalfSpace(0, False)` | needs a `HalfSpaceSense` |
| `HalfSpaceSense.Negative()` | `.Inside()` |
| `KeffResult.k_eff` | `.k_mean` |
| `GeometryPath.cell` | only `.material` exists |
| `profiles.stage_temperatures` | `.stage_temperature` (singular) |
| `triso_particle(c, [radii], [mats])` | needs `TrisoRadii`, `TrisoMaterials` |
| `TrayHydraulics()` | `.HoldupTimeConstant(tau_seconds=..)` |
| `ControlDict()` | thirteen positional arguments |
| `FvSolution()` | not constructible at all |

Every one cost a round trip. A model that *cannot* read the source has no way
to recover from any of them, which is the point: it is forced to rely on the
affordances the API actually provides, exactly as a human at a terminal is.

**What the test requires:**

- **Fixed context.** Wheel installed, nothing else. Handing the agent the
  stubs or the crate source tests something other than the API.
- **A fixed task list** spanning the tiers — a one-call case, a case assembled
  from parts, and one exotic enough to need the low-level types.
- **Every exception logged, not just the count.** Ranked by frequency across
  tasks, that log *is* the backlog, in priority order. This is the same
  technique as `outram-park/codegen/blocked.md` applied one level up: make the
  gaps countable and they stop being a matter of opinion.

**What it does NOT measure: physical correctness.** A script can run cleanly
and be nonsense. This never substitutes for the V&V tests — it tests whether
the API can be *found and called*, nothing more.

**Reading the result.** A failure the Opus session also hit is an API defect.
A failure unique to the small model is more likely its own priors — the tell
is whether a different model trips on the same call. Fix the first kind.

**The same principle applies one layer down, to Rust callers.** A confusing
trait-bound error is the compiler's version of an undiscoverable API, and
`#[diagnostic::on_unimplemented]` has been stable since Rust 1.78 — it
replaces "the trait bound `X: Y` is not satisfied" with a message, a label
and a note of your choosing, naming what to construct instead.
`#[diagnostic::do_not_recommend]` (stable since 1.85) suppresses a blanket
impl the compiler would otherwise suggest unhelpfully. This workspace has 65
public traits and, as of 2026-09-04, not one of them carries either
attribute. Any trait whose bound a caller can plausibly fail should — the
audit is `op-wiep`, ranked by how often each trait is actually named as a
bound (`EquationOfState` 91, `ThermoModel` 89, `FluidComponentTrait` 53
lead it).

A good message does not restate the bound. It names the concrete types that
*do* implement the trait, or the constructor to call: "implemented by
`PengRobinson`, `Srk`, `PengRobinson1978`" turns a search through the source
into a choice from a list, which is the same standard the Python enums are
held to above.

The harness itself, and the first baseline, are tracked as `op-m2mj`. Take
the baseline *before* changing anything: the fixes this suggests — enum
errors that name their constructors, did-you-mean on a wrong attribute,
constructors that return the general type rather than swallowing the run —
should be measured against it, not asserted.

