# CLS and SCLS — verification on the FHR pebble

Chord Length Sampling and semi-implicit CLS, chased down against explicit
geometry. Everything here is **verification against another code path**, never
validation: the references are this crate's own explicit-geometry calculations,
not experiments.

| | |
|---|---|
| **Branch** | `claude/outram-mc-setup-cb1bg9`, merged from `develop` at `74231843f` |
| **Problem** | FHR reference **unit cell** — 1.9 cm fuel zone / 2.0 cm pebble / FLiBe to r = 3.0 cm, reflective, 600 K |
| **Fuel** | `TrisoSpec::FHR_HALEU_UCO` — 215 µm kernel, OPyC at 425 µm, packed at 30 % |
| **Data** | HIGH tier, ENDF/B-VIII.0 + crystalline-graphite S(α,β) |
| **Reference** | exact delta (Woodcock) tracking on the same geometry, same seed |

---

## Summary — two defects, and they point the same way

CLS sat about **3300 pcm below exact** on this problem, only ~1000 pcm better
than *full homogenisation*. That is far worse than a working stochastic-media
method should be, and it turned out to be two separate things.

| # | defect | affects | effect | status |
|---|---|---|---|---|
| 1 | inclusion chords sampled **exponentially**, not from a sphere's chord law | **CLS only** — SCLS ray-traces real spheres | under-absorbs in the kernel; **masks** part of the gap | fixed |
| 2 | CLS applied to the **whole TRISO particle**, with the kernel smeared through it | **CLS and SCLS** — both build the same smeared medium | destroys grain-level self-shielding; **causes** the gap — worth **+3818 pcm** | variant added for each, measured |

The `affects` column is not a detail. Defect 1 was the *smaller* error and hit
only one of the two methods; defect 2 was the larger one and hit both. So the
whole-particle CLS-versus-SCLS comparison — the obvious thing to run, and what
was run first — differed by a defect the two did not share while agreeing on a
much bigger one they did.

---

## Defect 1 — a sphere's chord is not exponential

`sample_chord` draws `ℓ = −⟨ℓ⟩·ln ξ`, and it was used for **both** phases.
That is correct for a **Markovian binary mixture** — which is exactly what this
module's references describe (Lux & Koblinger; Zimmerman & Adams) — but the
inclusions here are **spheres of fixed radius**, the non-Markovian case. For a
sphere under uniform isotropic incidence the impact parameter is uniform in
area, so

```text
ℓ = 2R·√ξ ,   f(ℓ) = ℓ / (2R²)  on  0 ≤ ℓ ≤ 2R
```

whose mean is `4R/3` — Cauchy's result, the same as the exponential was given.
**That is why it went unnoticed: the mean was right, and the tests only checked
the mean.**

Measured, 400 000 samples at `R = 0.02135 cm`:

| | mean | σ/⟨ℓ⟩ | `P(ℓ > 2R)` | max / 2R |
|---|---|---|---|---|
| ray-traced sphere (truth) | 0.028474 | **0.353** | **0** | **1.00** |
| exponential (previous code) | 0.028516 | 1.000 | **0.223** | **8.01** |

**22 % of sampled inclusion chords were longer than the longest chord a sphere
has**, out to eight diameters.

### The sign of this defect — predicted before measuring, and it is the awkward one

`1 − e^{−Σℓ}` is concave in `ℓ`, so by Jensen a **higher**-variance chord
distribution at the same mean absorbs **less**. Mean absorption probability per
kernel traversal:

| `Σ⟨ℓ⟩` | true sphere | exponential | difference |
|---|---|---|---|
| 0.57 | 0.42229 | 0.36293 | −0.059 |
| 1.42 | 0.72408 | 0.58758 | −0.137 |
| **2.85** | **0.89850** | **0.74038** | **−0.158** |
| 8.54 | 0.98785 | 0.89563 | −0.092 |

So the old code **under-absorbed** in the kernels, which *raises* k. Correcting
it moves k **down, away from exact** — this defect was **masking** part of the
true error, not causing it. Recorded because it is the opposite of what one
would hope for, and it is what the physics says.

**Fix:** `cls::sample_chord_sphere`. The matrix phase stays exponential, which
is standard and correct. Three new tests — the diameter bound, both analytic
moments, and a quantile match against a ray trace of a real sphere — each
verified to **fail** on the previous behaviour.

> **The ray-trace test was itself defective when first written, and was
> rewritten 2026-09-18.** It drew an impact parameter `b = R√u` and evaluated
> the secant `2√(R² − b²)`. Because `1 − u` is uniform when `u` is, that is
> `2R√(1−u)` — *literally `sample_chord_sphere`'s own law, rearranged*. It
> compared two spellings of one formula and would have passed even if the
> formula were the wrong one for a sphere, so it could not support the claim in
> its own name.
>
> It now builds the geometry and intersects it: an entry point uniform on the
> sphere's surface, a direction drawn **cosine-weighted about the inward
> normal** (which is what "uniform isotropic incidence" means for a convex body,
> and the condition under which Cauchy's `4V/S` holds), and the exit found by
> solving the ray–sphere quadratic with `c = |P|² − R²` computed rather than
> assumed zero. Nowhere on that path is `2R√ξ` evaluated. The trace also
> self-checks that no chord it produces exceeds `2R`, so a broken reference
> cannot quietly validate a broken sampler.
>
> Discriminating power, since a tolerance is only meaningful against the
> alternative: an exponential with the same mean has median `0.924R` against the
> sphere's `1.414R`, a gap of **24.5 % of 2R** against a **2 %** bar.

---

## Defect 2 — CLS was applied to the wrong inclusion

`DhTreatment::ChordLength` takes the **whole TRISO particle** as the inclusion
and fills it with a volume-homogenised kernel-plus-coatings material. On the FHR
spec that smears the fuel kernel over

```text
(r_opyc / r_kernel)³ = (0.0425 / 0.0215)³ = 7.7×
```

its own volume. Resonance self-shielding is precisely what that dilution
destroys — and grain-level shielding is the **larger** of the two effects a
doubly-heterogeneous treatment exists to keep. Hence CLS landing barely better
than full homogenisation: both throw the kernel-level shielding away, and CLS
merely keeps the particle-level part.

**`DhTreatment::ChordLengthKernel`** applies CLS where the physics is: the
inclusion is the **kernel at full density**, and the matrix is the four coatings
plus the graphite, homogenised. The packing fraction follows from geometry
rather than being chosen:

```text
pf_kernel = pf_particle · (r_kernel / r_opyc)³ = 0.30 · 0.1295 = 0.03884
```

### Why this is also the regime where CLS works

`examples/cls_scls_regime_sweep.rs` measures CLS's error against explicit RSA
geometry as a function of packing fraction — **unresolved at `pf = 0.05`,
+0.0357 (≈11 combined s.e.) by `pf = 0.20`** — because the Markovian matrix chord
cannot represent the exclusion correlation of non-overlapping spheres, and that
correlation grows with `pf`.

Whole-particle CLS runs at **`pf = 0.30`**: the worst end of that range, and past
where plain RSA can even build a packing. Kernel-level CLS runs at
**`pf = 0.039`**, where the sweep says the method is accurate. The two defects
therefore push the same way, and the variant addresses both.

### The comparison is controlled

`kernel_level_cls_conserves_the_fuel_zone_inventory` pins, to 1e-12 per
nuclide, that the two CLS arms hold the **same heavy metal per unit fuel-zone
volume** — they differ in *where* the fuel sits, not how much there is. So any
eigenvalue difference between them is a **self-shielding** effect and nothing
else.

---

## Eigenvalue result — the diagnosis, tested

FHR reference unit cell, 800 × [15 inactive + 40 active], same seed and same
geometry on every arm. Measured 2026-09-18.

**All seven arms**, one run, one seed, one geometry. Verdicts are the example's
own, at its **3 σ** bar.

| treatment | k | Δ vs exact | σ | verdict | time | speed | stored |
|---|---|---|---|---|---|---|---|
| delta tracking (**exact**) | 1.38050 ± 0.00791 | — | — | reference | 148.4 s | 1.00× | 26 801 |
| CLS, **whole-particle** | 1.34901 ± 0.00727 | **−3149 pcm** | 2.9 | not resolved | 68.9 s | 2.16× | 0 |
| CLS, **kernel-level** | **1.38719 ± 0.00662** | **+669 pcm** | **0.6** | **not resolved** | 157.6 s | 0.94× | 0 |
| SCLS, whole-particle | 1.33799 ± 0.00638 | **−4251 pcm** | 4.2 | **RESOLVED bias** | 2158.7 s | **0.07×** | 0 |
| **SCLS, kernel-level** | **1.37587 ± 0.00625** | **−463 pcm** | **0.46** | **not resolved** | 3298.3 s | **0.045×** | 0 |
| naive homogenisation | 1.34647 ± 0.00731 | **−3403 pcm** | 3.2 | **RESOLVED bias** | 83.1 s | 1.78× | 0 |
| ring-RPT (fitted annulus) | 1.39754 ± 0.00734 | +1704 pcm | 1.6 | not resolved | 68.8 s | 2.16× | 0 |

**Applying CLS to the kernel instead of the smeared particle recovers
+3818 pcm.** That is the confirmation of defect 2: CLS's error on this problem
was not mainly its algorithm, it was what the algorithm was applied to.

### The strongest single result here: the two recoveries agree

Defect 2 was diagnosed as a **shared** defect — the same wrong inclusion, sitting
underneath two different algorithms. If that reading is right, fixing it must
buy **the same amount** in each, because what is being removed is the
homogenisation and not anything either algorithm does. If instead each method's
gap were mostly its own, the two recoveries would differ.

| algorithm | whole-particle | kernel-level | recovery |
|---|---|---|---|
| CLS | −3149 pcm | +669 pcm | **+3818 pcm** |
| SCLS | −4251 pcm | −463 pcm | **+3788 pcm** |

**The two agree to 30 pcm**, against a combined standard error of order
1000 pcm on any single one of these differences. Two algorithms that share no
sampling machinery — one re-samples chord statistics, the other ray-traces
materialised spheres — recover the same 3.8 % in `k` when the same
homogenisation is removed from underneath them.

That is a **prediction that could have failed and did not**, and it is stronger
evidence for the diagnosis than either recovery alone. The agreement is not
built in: nothing in `build_kernel_level_cls` ties the two arms' eigenvalues
together, and their *costs* differ by 21× on the same run.

Both kernel arms end up **not resolved** from exact — SCLS at **0.46 σ** and
CLS at **0.65 σ**, the two closest of all five approximations, against
ring-RPT's 1.6 σ and whole-particle CLS's 2.9 σ.

> **A correction to an earlier revision of this table**, which marked
> whole-particle CLS "resolved" at 2.93 σ. The example's own criterion is
> `z >= 3.0`, so its verdict is **not resolved** — it is a marginal case, not a
> resolved bias, and the table now reports what the program reports. The
> −3149 pcm central value is unchanged and the conclusion is unaffected (the
> +3818 pcm *move* is what carries the argument, and that is several times the
> uncertainty), but a write-up must not upgrade its own evidence past the bar
> the code applies.

**Two results here are worth stating plainly because they are unflattering.**
Whole-particle SCLS is **worse than simply smearing the entire fuel zone**
(−4251 against −3403 pcm) while costing **26× naive homogenisation's runtime**.
And ring-RPT — the arm that reproduced exact tracking to +37 pcm in the
7200-history study — reads +1704 pcm here. That is 1.6 σ at these statistics and
consistent with the earlier result at 1.5 σ, so it is noise rather than a
regression; it is recorded because a reader comparing the two tables will
otherwise think something broke.

The prediction was recorded before the run, together with the reasoning that
the chord-law fix would push the other way — and it does: correcting the chord
law alone left CLS at 1.34901, statistically unchanged from the 1.34933 it gave
before, because that defect was *masking* a fraction of a much larger error.

### Two qualifications, both of which matter

**The statistics are thin.** At 800 histories the combined standard error is
about 1000 pcm, so this run resolves ~2000–3000 pcm and no better. "Not
resolved at 0.65σ" is therefore a much weaker statement than *agreement*: it
says the residual is under the noise floor, not that it is zero. Claiming
kernel-level CLS reproduces the exact answer needs the 7200-history
statistics. What the run does establish firmly is the **−3149 → +669 pcm
move**, which is several times the uncertainty.

**It costs the entire speed advantage.** 157.6 s against exact delta tracking's
148.4 s — **1.06×, slightly slower than the method it approximates**. The
reason is mechanical: whole-particle CLS is fast *because* the smeared particle
lowers the delta-tracking majorant, and that is the very dilution that destroys
the self-shielding. Restoring the undiluted kernel restores the true majorant,
so the virtual-collision count returns to delta tracking's, while CLS still
pays a mutex-guarded chord-bookkeeping call per query where delta tracking does
an O(1) grid lookup.

**So on this problem accuracy and speed come from the same approximation, and
one cannot be kept without losing the other.** What kernel-level CLS still buys
is **memory**: 0 particles stored against 26 801. That is the honest case for
it — a method for geometries too large to pack, not a faster way to solve one
that fits.

## Reproducibility — CLS is stateful, and that is not the same as non-deterministic

CLS's point query **samples**, so `DhUniverse::material_at` is not pure: it
consumes from a stream behind a mutex. That is a fair thing to be suspicious of,
and `DhUniverse::keff` documents the consequence — ordered on
`ComputeType::CpuSingleThread`, order-dependent on the multi-threaded backend.

The claim was tested rather than trusted. `KeffSettings::default()` selects
`CpuSingleThread`, so the **prediction, recorded before the run**, was exact
reproduction — not agreement within statistics.

| arm | run | k | time |
|---|---|---|---|
| CLS, kernel-level | main V&V run | 1.38719 ± 0.00662 | 157.6 s |
| CLS, kernel-level | replicate, separate process | **1.38719 ± 0.00662** | 159.6 s |
| SCLS, kernel-level | main V&V run | 1.37587 ± 0.00625 | 3298.3 s |
| SCLS, kernel-level | replicate, separate process | **1.37587 ± 0.00625** | 3265.7 s |

**Identical in every printed digit**, eigenvalue and standard error alike, for
**both** kernel arms, from separate processes each doing its own cross-section
reconstruction. Only wall clock differs — 1.3 % and 1.0 % respectively — which
is the machine and not the method.

So the mutex-guarded sampler is a **performance** concern and an ordering
concern on the parallel backend (bead `op-ifl3`), **not** a determinism defect
on the backend this V&V was run on. A result that merely agreed within 1 sigma
would not have distinguished those two readings; an exact match does.

The caveat stands unchanged for multi-threaded CLS: treat any such number as one
draw rather than *the* answer.

**This check is load-bearing for the eigenvalue table, not a curiosity.** The
two SCLS arms cost tens of minutes each, so they were measured in separate
processes using `OUTRAM_DH_VV_ONLY` rather than by re-running all seven arms
end to end. Merging rows from different processes into one comparison table is
only legitimate if a filtered run reproduces the corresponding row of a full
run. It does, for both kernel arms, to every printed digit. Without that, the
kernel-level rows would have to be quoted as separate experiments rather than
compared directly against the delta-tracking reference in the same table.

> **The SCLS replicate was an accident, and it is worth saying so.** The filter
> string `cls, kernel` was intended to select `CLS, kernel-level` alone —
> but **`cls` is a substring of `scls`**, so it also selected
> `SCLS, kernel-level` and spent another 54 minutes on it. The footgun is now
> documented in `examples/dh_keff_vv.rs`.
>
> It is reported rather than quietly folded in because the *provenance of
> evidence matters*: this replicate was not designed, and an accident that
> happens to confirm a result is weaker than a test built to break it. What
> makes it usable here is that the quantity was fixed before the run existed —
> the original 1.37587 was already recorded — so there was no opportunity to
> choose the comparison after seeing it. Had it disagreed, the table would have
> been wrong and this section would say so.

---

## Regime sweep — CLS and SCLS against explicit geometry

`examples/cls_scls_regime_sweep.rs`, **40 000 histories/arm** (s.e. ~0.0023), seed
20260918, cube of half-width 0.5 cm. Inclusions are pure absorbers, so this measures the
**matrix-side correlation error only** — cleanly separated from the inclusion
chord law of defect 1, and unaffected by its correction. `*` = beyond two
combined standard errors.

### Packing fraction (r = 0.05 cm, scatter mfp = 0.30 cm)

| `pf` | `P_RSA` | `CLS − RSA` | `SCLS − RSA` |
|---|---|---|---|
| **0.0388** (kernel-level regime) | 0.2928 | −0.0034 | −0.0016 |
| 0.05 | 0.3469 | +0.0021 | +0.0015 |
| 0.10 | 0.5208 | **+0.0169*** | **+0.0158*** |
| 0.20 | 0.6907 | **+0.0357*** | **+0.0397*** |
| 0.30 | — | — | RSA saturates; packing failed |

At `pf = 0.05` **neither model is resolved from the explicit reference** — CLS
is accurate in the dilute limit, which is the point that makes kernel-level CLS
worth trying.

**`pf = 0.0388` is the kernel-level variant's own packing fraction**
(`0.30 × (0.0215/0.0425)³`), swept explicitly on 2026-09-18 so that claim rests
on a measured point rather than on extrapolation from `pf = 0.05`. Both models
are unresolved there too — CLS at **1.5** combined s.e., SCLS at **0.7**. The
central values are *negative* where the higher-`pf` rows are positive, but at
1.5 s.e. that is noise and is not evidence of a sign change.

~~The regime the FHR pebble occupies was not swept.~~ **CORRECTED 2026-09-18.**
That is still true of the *whole-particle* arm at `pf = 0.30`, which RSA cannot
build. It is **no longer true of the kernel-level arm**, whose regime is now the
first row of this table. Recorded rather than silently replaced, because the
earlier statement was used to qualify the kernel-level result.

CLS **over-absorbs**, growing with `pf`. That is the sign that costs reactivity,
and it matches the eigenvalue result.

`pf = 0.30` could not be built at all — plain RSA saturates below it. Real TRISO
packings sit at or above 0.30, which is what the RSA–DEM work of Tan et al.
(2026) exists to reach, **so this sweep cannot probe the regime the FHR pebble
actually occupies**, and the trend says the error there is worse, not better.

### Scattering (pf = 0.20, r = 0.05 cm)

| scatter mfp [cm] | `P_RSA` | `CLS − RSA` | `SCLS − RSA` |
|---|---|---|---|
| 0.10 | 0.7212 | **+0.0451*** | **+0.0442*** |
| 0.30 | 0.6907 | **+0.0357*** | **+0.0397*** |
| 1.00 | 0.6758 | **+0.0351*** | **+0.0571*** |
| 3.00 | 0.6699 | **+0.0340*** | **+0.2243*** |

CLS's error grows as scattering increases, from +0.0340 at mfp 3.0 to +0.0451 at
mfp 0.10 — the direction the memoryless assumption predicts.

> ~~CLS's error **doubles** as scattering increases (+0.0220 → +0.0441)~~
> **CORRECTED.** That was read off an 8000-history run whose standard error was
> 0.005, and the trend was partly noise. At 40 000 histories (s.e. 0.0023) the
> variation is about **33 %, not a factor of two**. The direction survives; the
> magnitude did not. Recorded rather than silently replaced, because the first
> number was already written down.

**SCLS fails badly in the opposite corner: +0.2243 at mfp 3.0**, six times CLS's
error on the same case and the one place where the two models diverge sharply.
Its retention window is `λ_transport + R`, so a long mean free path makes the
window exceed the domain; nothing is ever culled and the "local view" stops
being local.

### Inclusion radius (pf = 0.20, scatter mfp = 0.30 cm)

| r [cm] | `CLS − RSA` | `SCLS − RSA` |
|---|---|---|
| 0.025 | **+0.0334*** | **+0.0315*** |
| 0.050 | **+0.0357*** | **+0.0397*** |
| 0.080 | **+0.0441*** | **+0.0384*** |

A weaker dependence, same direction.

---

## Defect 2 is inherited by SCLS, and was hiding inside its result

SCLS was measured at **−3365 pcm** and read as a statement about SCLS. It is
not. `DhTreatment::Scls` builds its inner CLS medium like this
(`dh_universe.rs`):

```rust
let particle = homogenise_particle(&params.materials, params.spec)?;   // kernel SMEARED
let cls = ClsMedium::new(r_particle, pf, /* inclusion */ …, /* matrix */ …);
let medium = SclsMedium::new(cls, …, window);
```

`r_particle`, `pf`, and a volume-homogenised kernel-plus-coatings material —
**character for character the construction whole-particle CLS uses**. SCLS's
retention window sits on top of a medium carrying defect 2 in full.

The numbers to compare are the **matched pair from one run**, the 7200-history
study recorded in `examples/dh_keff_vv.rs` (combined s.e. ~320 pcm):

| treatment | k | Δ vs exact |
|---|---|---|
| delta tracking (exact) | 1.38647 ± 0.00211 | — |
| CLS, whole-particle | 1.35380 ± 0.00267 | **−3267 pcm** |
| SCLS, whole-particle | 1.35282 ± 0.00228 | **−3365 pcm** |

(Do **not** pair −3365 against the −3149 in the eigenvalue table above — that
one is from the 800-history run, a different set of statistics. An earlier draft
of this section did exactly that and made the gap look twice its size.)

So the two differ by **98 pcm**, well inside a ~340 pcm combined error on the
difference, while both sit ~3300 pcm below exact. The wrong-inclusion error
dominates both, and 98 pcm is all that was ever attributable to the retention
machinery under study.

**This matters beyond bookkeeping: it invalidates the obvious comparison.**
Judging SCLS against CLS in their whole-particle forms compares two methods
that share an error several times larger than their difference. Any conclusion
about SCLS's retention drawn that way is measuring the homogenisation both
inherit.

`DhTreatment::SclsKernel` was therefore added, sharing one
`build_kernel_level_cls()` helper with `ChordLengthKernel` so the volume
arithmetic exists once and cannot drift between the two kernel arms.

**Measured, and the diagnosis holds.** Kernel-level SCLS gives
**1.37587 ± 0.00625, i.e. −463 pcm at 0.46 σ — not resolved from exact**,
against whole-particle SCLS's resolved −4251 pcm. The **+3788 pcm** recovered
matches CLS's **+3818 pcm** to 30 pcm; see *The strongest single result here*
above for why that agreement is the real confirmation rather than either number
alone.

### SCLS inherits defect 2 but is structurally immune to defect 1

The two defects do **not** land on the two methods the same way, and reading
the code settles it without needing a run.

`SclsMedium` never samples an inclusion chord. When a flight is inside an
inclusion it calls `ray_sphere_exit(pos, u, center, r)` — an exact ray-trace of
a sphere it has actually materialised and remembered — and the only place it
calls `sample_chord` is the **matrix** gap, where the exponential is the right
law. Checked exhaustively: `self.cls.` appears five times in `scls.rs` and every
one reads a scalar parameter (`mean_chord_matrix`, `inclusion_radius`,
`inclusion_material`, `matrix_material`, `packing_fraction`). It never delegates
a sampling call to `ClsMedium`.

That is the whole premise of the semi-implicit method — retain real geometry
instead of re-sampling statistics — so this is by design rather than by luck.

| | defect 1 (chord law) | defect 2 (wrong inclusion) |
|---|---|---|
| CLS | **affected** — sampled inclusion chords exponentially | **affected** |
| SCLS | **immune** — ray-traces materialised spheres | **affected** |

So the two methods were never mis-modelled in the same way, and the pre-fix
comparison between them was doubly confounded: they differed by one defect they
did not share while agreeing on a much larger one they did.

### The grain count is preserved exactly, and that is the point

Moving the inclusion from the particle to the kernel must not change *how many*
grains there are — a TRISO particle has exactly one kernel. Because

```text
pf_kernel = pf * (r_kernel / r_particle)^3
```

the implied number density is unchanged identically:

```text
n_kernel = pf_kernel / r_kernel^3 = pf / r_particle^3 = n_particle
```

`kernel_level_variants_preserve_the_grain_number_density` pins this for **both**
kernel arms and it holds to **0.0 relative** — the same three multiplications
undone in the opposite order, so it is exact in binary rather than merely close.
Together with `kernel_level_cls_conserves_the_fuel_zone_inventory` (1e-12 per
nuclide) that fixes both the amount of fuel and its division into grains, which
is what licenses reading any remaining eigenvalue difference as
**self-shielding** rather than as a changed problem.

### What the kernel arms still get wrong

The four coating shells are homogenised **into the matrix**. Their inventory and
the grain count survive exactly, as above; their **address** does not. Every
coating physically wraps one specific kernel, so a neutron leaving a kernel
always crosses ~210 µm of carbon and SiC before reaching graphite. In the model
it enters a matrix holding those materials at their average concentration
everywhere, including far from any grain.

Expected second order, and the reason is worth stating rather than asserting:
the coatings are scatterers with little absorption and no resonances of
consequence, so they perturb the moderation *between* grains, not the shielding
of the resonance absorber itself — which is the large effect the kernel arms
restore. **But this has not been ablated**, so it is a live candidate for part
of the residual and must not be quoted as small. Separating it needs a
three-phase CLS (kernel / coating shell / graphite) that this module does not
implement.

---

## SCLS is slower than the exact method it approximates

~~The SCLS arm was killed after 12.5 minutes without finishing — more than five
times the cost of resolving the geometry exactly.~~ **SUPERSEDED 2026-09-18 —
it was allowed to finish, and "more than five times" understated it by a factor
of three.**

On the FHR unit cell at 800 × [15 inactive + 40 active], same seed and geometry
on both arms:

| | k | Δ vs exact | σ-distance | time | vs exact |
|---|---|---|---|---|---|
| delta tracking (**exact**) | 1.38050 ± 0.00791 | — | — | **148.4 s** | 1.00× |
| SCLS, whole-particle | 1.33799 ± 0.00638 | **−4251 pcm** | 4.2, **resolved** | **2158.7 s** | **14.5× slower** |
| SCLS, kernel-level | 1.37587 ± 0.00625 | −463 pcm | 0.46, not resolved | **3298.3 s** | **22.2× slower** |

**SCLS costs 14.5× the exact method it approximates, and is resolved 4251 pcm
away from it.**

> **This −4251 pcm and the −3365 pcm quoted earlier are the same result at
> different statistics, not a contradiction.** −3365 is the 7200-history run
> (combined s.e. ~320 pcm); −4251 is this 800-history run (combined s.e.
> ~1016 pcm). They differ by 886 pcm against a combined 1065 pcm, i.e. **0.83
> sigma** — consistent. Quote −3365 when a number is wanted; quote −4251 only
> alongside the timing it came with, since the two must come from one run to be
> comparable.
 Both halves matter: a method that were merely slow would still
be useful where memory is the binding constraint, and a method that were merely
biased would still be useful if it were fast. This is neither.

The earlier figure was a *lower bound from a kill*, recorded as though it were a
measurement. Replacing it changes no conclusion — the recommendation was already
"do not use SCLS here" — but a bound and a measurement are different objects and
the write-up should not have blurred them.

This is the pathology `DhTreatment::Scls` already documents: the window
`λ_transport + R ≈ 2.68 cm` exceeds the 1.9 cm fuel zone, so nothing is culled,
the retained set grows without bound, and `material_at` scans it linearly on
every query. The regime sweep reproduces the same failure on a controlled
problem — `+0.2243` at scatter mfp 3.0 cm, six times CLS's error on the same
case. **SCLS should not be used on a graphite pebble**, and this is now measured
from two directions rather than argued from one.

**The kernel-level variant does not rescue the cost, and the reason is exact
rather than empirical.** Moving the inclusion from the particle to the kernel
leaves the grain *number density* unchanged identically (see above), so the
retained set inside a window of given radius holds the **same number of
inclusions**. The window itself shrinks only by the difference in inclusion
radius — `λ_tr + 0.0215` against `λ_tr + 0.0425`, i.e. ~2 parts in 270 — and is
capped at the domain in both cases anyway. So the linear scan that dominates
SCLS's cost is essentially unchanged. Kernel-level SCLS was expected to be
**comparably slow, not slower and not appreciably faster**; that prediction was
recorded before its run.

> **That prediction is revised, and the revision is recorded BEFORE the number
> landed — at 43 min of transport against the whole-particle arm's 36 min, with
> the run still going.** The reasoning above is right about the *scan* and wrong
> about the *number of scans*. Cost is
>
> ```text
> total = (queries per history) x (cost per query)
> ```
>
> and the argument only addressed the second factor. The first one rises,
> for the same reason kernel-level **CLS** cost 157.6 s against whole-particle
> CLS's 68.9 s — a factor of **2.29**: restoring the undiluted kernel restores
> the true delta-tracking majorant, which multiplies the virtual-collision rate
> and therefore the number of `material_at` calls. SCLS pays that same multiplier
> on top of a per-query cost that is already unbounded.
>
> Scaling the whole-particle arm's 2158.7 s by that 2.29 predicts **~4900 s
> (~82 min)**. The honest statement is that the original prediction was made by
> checking one factor of a product and is expected to fail on the other; the
> measured value is in the table below, and it is reported whichever way it
> falls.

**Measured: 3298.3 s. Both predictions were wrong.**

| | predicted | measured |
|---|---|---|
| original — "comparably slow", `~1.0x` the whole-particle arm | 2160 s | — |
| revised — majorant scaling, `2.29x` | ~4900 s | — |
| **actual** | | **3298.3 s, `1.53x`** |

The revision got the **direction** right — kernel-level SCLS *is* appreciably
slower, so the original claim of "comparably slow, not slower" is refuted — and
**overestimated the magnitude by 49 %**. The true multiplier is `1.53x`, not
`2.29x`.

Why the CLS multiplier does not transfer intact: the majorant argument says the
*query count* rises by `2.29x`, and for CLS that is the whole story because its
per-query cost is `O(1)`. SCLS's per-query cost is a linear scan over the
retained set, and the retained set is built from the **inclusions a flight
actually meets**, which is geometry — it does not grow just because the
virtual-collision rate did. Extra virtual collisions therefore land
disproportionately on inclusions already retained, so the added work is
sub-proportional to the added queries. `1.53x` against `2.29x` is that
sub-proportionality, measured.

**This explanation is post-hoc and is offered as a hypothesis, not a result.**
It was constructed to fit a number already in hand, which is exactly the
circumstance in which an explanation is cheapest and least trustworthy. Testing
it would mean instrumenting the retained-set size and the query count
separately across both arms; that has not been done.

**Kernel-level SCLS is therefore the most expensive arm on this problem —
22.2x exact delta tracking** — while being the most accurate approximation of
the five. Accuracy and cost are anti-correlated across this whole table, and
this arm is the extreme point of that trade.

---

## What is NOT established

- **No validation.** Every reference here is another code path in this crate.
- **Neither kernel-level arm has been SHOWN TO AGREE with exact tracking.**
  Both are *not resolved* from it — SCLS at 0.46 sigma, CLS at 0.65 sigma — on
  an 800-history run whose combined standard error is ~1000 pcm — which says the residual is under the noise
  floor, not that it is zero. A 7200-history run would be needed to claim
  agreement, and has not been made. What **is** established firmly is the
  **−3149 → +669 pcm move**, several times the uncertainty.
- **One problem, one spec, one temperature.** The FHR reference unit cell at
  600 K with `TrisoSpec::FHR_HALEU_UCO`. Nothing here speaks to a different
  kernel radius, packing fraction, coating stack, or spectrum, and the
  kernel-level variant's whole advantage is a *ratio* of radii — a spec with a
  thinner coating stack sits closer to the whole-particle case and would
  recover less.
- **The WHOLE-PARTICLE regime was not swept** — RSA cannot build `pf = 0.30`.
  The trend extrapolates unfavourably, but it is an extrapolation. The
  **kernel-level** regime (`pf = 0.0388`) *was* swept directly, and both models
  are unresolved from explicit geometry there.
- **The sweep's inclusions are pure absorbers in a non-multiplying cube.** It
  isolates the matrix-side correlation error cleanly, which is what it is for,
  but it is not an eigenvalue problem and carries no resonance structure — so
  "unresolved at `pf = 0.0388`" bounds the *geometric* error of the kernel-level
  variant, not its total error on a real pebble.
- **The coatings' spatial correlation with the kernel is discarded** by both
  kernel-level variants and has **not** been ablated. Inventory and grain number
  density are conserved exactly (two tests pin them); the coatings' position is
  not. Argued second order because they are low-absorption scatterers, but
  unmeasured — isolating it needs a three-phase CLS this module does not have.
- **The SCLS source paper is not catalogued.** The method follows Tan, Feng,
  Chan & Wang (2025), `10.1016/j.anucene.2025.111436`, which is **not** in
  `crates/kovan-literature`. The workspace requires any literature that informs
  the code to be catalogued, and the implementation has therefore not been
  checked line-by-line against its source.
- **No human has reviewed these numbers.** AI-assisted draft material under
  `RESPONSIBLE_USE.md`.
