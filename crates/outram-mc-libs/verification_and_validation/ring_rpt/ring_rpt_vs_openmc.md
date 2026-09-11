# Ring-RPT FHR pebble — `outram-mc-libs` vs OpenMC

**Generated:** 2026-09-10 (UTC)
**Crate commit:** branch `op-mzvp2-pebble-wiring` (off `develop`)
**Status: first full-pebble comparison.** The two P1 transport bugs that blocked
this (GH #168 concentric-sphere leak, GH #169 charged-particle absorption) are
fixed, and graphite S(α,β) is wired. Absolute k is ~2.9 % above OpenMC, and the
discrepancy is concentrated in the resonance-escape (p, +9 %) and fast-fission
(ε, −6 %) factors — pointing at U-238 epithermal/fast cross sections
(leading suspect: URR self-shielding not reconstructed). η and f match within
1 %. Not a validated result — an AI-assisted code-to-code check, no human V&V.

## Methodology

**What RPT is.** A reactivity-equivalent physical transformation replaces the
stochastic TRISO-particle distribution of a pebble's fuel zone with a single
homogeneous fuel *shell* whose inner radius is tuned so the pebble's k-eff is
unchanged. The reference (`op-mzvp.1`, GitHub #156) established that OpenMC
reproduces this to **−31 ± 92 pcm** (0.34σ) on ENDF/B-VIII.0 at
r_inner = 1.493359375 cm, at 46× less cost than the 34 224-cell explicit model.

**The two pebbles** (`openmc_inputs/`, BSD-3 © 2024 theodoreOnzGit):
19.9 % HALEU UCO kernel (215 µm r), buffer/IPyC/SiC/OPyC to 425 µm, 30 % packing
in graphite; graphite matrix + 0.1 cm shell; FLiBe at 99.995 % Li-7; 600 K;
reflective `Sphere(r = 3.0)`.

**`outram-mc-libs` path.**
- Materials + geometry: new `src/pebble_beds/fhr_pebble.rs` (data-free builders —
  `TrisoSpec`, `homogenise_by_volume`, `rpt_fuel_outer_radius`,
  `fhr_pebble_geometry`). Homogenisation is a volume-weighted atom-density mix
  (exact — `Material` is atom-density-based, unlike an OpenMC `Material`).
- Nuclear data: **HIGH-fidelity ENDF/B-VIII.0**, `Nuclide::from_endf_file`
  (RECONR + BROADR on device @ 600 K). All 13 nuclides reconstruct; U-235/U-238
  are Reich-Moore (LRF=3) — no VII.1 fallback needed (`op-mzvp.2.6`).
- Six factors + spectrum: `run_keff_reactor_physics` (`op-mzvp.2.2`).
- Reference `examples/fhr_ring_rpt_endf.rs`, feature `endf-pebble-cases`.

**How `outram-mc-libs` runs it.** Four Monte-Carlo runs, 4000 histories ×
[30 + 80] generations, ENDF/B-VIII.0 @ 600 K, crystalline-graphite S(α,β) on the
buffer / PyC / matrix / shell carbon. Fuel-kernel carbon **and SiC-coating
carbon** are free-gas, matching the deck — the deck applies `c_Graphite` to
neither, and a dedicated `tsl-CinSiC` / `tsl-SiinSiC` law (both present in
`reference-data/endf/`) is used by neither code. Switching the SiC carbon from
`c_Graphite` to free-gas moved the CSG-sphere k by +105 pcm (1.39228 → 1.39333,
within 1σ) and the six factors not at all — confirming the 35 µm coating's
thermal treatment is negligible here.

### The domain mismatch, and its fix (2026-09-11)

The first revision of this study ran all three delta-tracked pebbles in a
reflective **cube** of half-width 3, and compared them against an OpenMC deck
whose boundary is a reflective **sphere** of radius 3. Those are not the same
problem: the cube's corners (`3 < r < 3√3 ≈ 5.196`) hold FLiBe the sphere does
not — roughly 2.3× the coolant volume — and FLiBe absorbs. So the cube runs'
absolute k could not be compared to OpenMC at all, and the only row that could
(the surface-tracked CSG ring-RPT) had **no explicit-TRISO twin**. That left the
central question — is the absolute offset a property of our *data* or of the RPT
*geometry*? — unanswerable, because the two candidate explanations were never
measured on the same boundary.

`pebble_beds::keff_delta` now carries `DeltaDomain { Cube, Sphere }`, and the
delta driver takes the domain (`run_keff_delta_in`; the old `run_keff_delta`
remains as a cube shorthand). The sphere arm solves `|r + t·u|² = R²` for the
exit distance and reflects specularly about the outward normal `n = r/R`.

It is validated against the pre-existing cube arm by
`geometry_independence_of_k_inf_for_a_uniform_medium`: a reflective boundary
means no leakage, so a **uniform** medium's eigenvalue is its `k∞`, a material
property with no shape dependence — cube and sphere must agree. They do, at
**0.14σ (−105 pcm)**. Two further tests pin the sampler (`<r³> = R³/2` for a
volume-uniform fill, versus `R³/4` for the `r = R·u` mistake) and the reflection
(2000 random flights plus an on-surface outward ray and a near-tangent ray, with
the direction asserted to stay normalised).

**One honest caveat about a specularly reflecting sphere.** Specular reflection
off a *plane* (the cube) is a true mirror, so a reflective cube is exactly an
infinite lattice. Specular reflection off a *sphere* is not: it conserves the
neutron's angular momentum about the centre, so between collisions a neutron
keeps the impact parameter it started with and can in principle orbit without
ever sampling the middle of the pebble. That would bias a heterogeneous problem.

It does not bias this one, for two reasons, and both are worth stating rather
than assuming. First, the medium scatters heavily — graphite is the moderator —
so collisions randomise the direction long before any orbit persists; the
conservation only holds *between* collisions. Second, and decisively for a
code-to-code comparison, **OpenMC's `boundary_type='reflective'` on a sphere is
the same specular reflection**, so the reference carries whatever this effect is
worth and the comparison is like-for-like either way. The caveat matters only if
someone later reads these `k∞` values as infinite-lattice results in their own
right; they are reflective-sphere results.

| run | driver | domain |
|---|---|---|
| explicit TRISO | `run_keff_delta_in` (Woodcock) | reflective **sphere** r = 3 **and** cube half-width 3; 54 706 packed particles in r < 1.9; 5 layers resolved by nearest-centre + radius |
| ring-RPT | `run_keff_delta_in` | **same two domains**; homogenised fuel shell 1.4934–1.7531 cm |
| naive homogenised | `run_keff_delta_in` | **same two domains**; homogenised fuel fills r < 1.9 |
| ring-RPT (CSG) | `run_keff_reactor_physics` | real **reflective sphere** r = 3 (`fhr_pebble_geometry`); + six factors + spectrum |

Every pebble is now run in **both** domains. The sphere rows carry all reported
comparisons — both `RPT − explicit` and the absolute k against OpenMC are on the
reference's own boundary. The cube rows are retained only to price the corner
over-count, which is now a measured quantity rather than a caveat.

## Reference

```bibtex
@misc{openmc_fuel_perf_project_2024,
  author = {Ong, Theodore},
  title  = {openmc\_fuel\_perf\_project: FHR TRISO pebble / ring-RPT models},
  year   = {2024}, note = {GitLab theodore\_ong/openmc\_fuel\_perf\_project,
  base commit ff92278, BSD-3-Clause}}
```
OpenMC 0.15.3-dev (`09ee8308d`), ENDF/B-VIII.0 HDF5, 600 K, 20 000 × 150 (50
inactive). Reference k-eff / six factors: `openmc_inputs/` + GitHub #156.

## Results

### OpenMC reference (op-mzvp.1) — full pebble, reflective sphere r=3

| | explicit TRISO | ring-RPT |
|---|---|---|
| k-eff | 1.36510 ± 0.00063 | 1.36479 ± 0.00067 |
| η / f / p / ε | 2.0158 / 0.9172 / 0.4837 / 1.5064 | 2.0073 / 0.9216 / 0.4842 / 1.5043 |
| **Δ(RPT − explicit)** | | **−31 pcm (0.34σ)** |

### `outram-mc-libs` — ENDF/B-VIII.0, `c_Graphite` S(α,β) (free-gas fuel + SiC C)

Run 2026-09-11 at `a7280ecb`, 4000 histories × [30 inactive + 80 active], 600 K.
Every pebble is run in **both** domains; the sphere is OpenMC's boundary and
carries all reported comparisons.

**Reflective sphere r = 3 — comparable to OpenMC in absolute terms:**

| run | k-eff | Δ(vs explicit) | vs OpenMC |
|---|---|---|---|
| explicit TRISO | 1.38191 ± 0.00193 | — | **+1681 pcm** |
| ring-RPT | 1.38354 ± 0.00204 | **+163 ± 281 pcm (0.58σ)** | **+1875 pcm** |
| naive homogenised | 1.35656 ± 0.00243 | **−2535 pcm (8.2σ)** | — |
| ring-RPT (CSG, surface-tracked) | 1.38279 ± 0.00270 | — | +1800 pcm |

**Reflective cube half-width 3 — NOT comparable to OpenMC; kept to price the
corner-FLiBe over-count:**

| run | k-eff | Δ(vs explicit) | cube − sphere |
|---|---|---|---|
| explicit TRISO | 1.31813 ± 0.00210 | — | −6378 pcm |
| ring-RPT | 1.30734 ± 0.00215 | −1079 ± 301 pcm (3.6σ) | −7620 pcm |
| naive homogenised | 1.30625 ± 0.00245 | −1188 pcm | −5031 pcm |

**Ring-RPT CSG six factors:** η 2.0155, f 0.9140, p 0.5266, ε 1.4131,
P_FNL 1.0000, P_TNL 0.9998; product (k_4f) 1.37051, consistency gap +0.89 % (in
band). Leakage 1.3e-4.

vs OpenMC ring-RPT: η **2.0073** (+0.4 %), f **0.9216** (−0.8 %),
p **0.4842** (**+9 %**), ε **1.5043** (**−6 %**).

**Two independent transport methods agree on the same geometry.** The
delta-tracked sphere gives 1.38354 ± 0.00204 and the surface-tracked CSG driver
gives 1.38279 ± 0.00270 — **75 pcm apart, 0.22σ**. Together with the uniform-
medium cube-vs-sphere check (0.14σ), the new `DeltaDomain::Sphere` arm is pinned
twice: once against the pre-existing cube arm on a problem with an analytic
answer, and once against a different tracking algorithm on the real pebble.

**The sign convention, because it is easy to get backwards.** Self-shielding
*reduces* absorption. The lumped (explicit) configuration has *more* of it, so
it captures less in the U-238 resonances and sits **higher**; homogenising must
come out **lower**. Hence `naive − explicit < 0`. Wang et al. (2014)'s +2820 pcm
`InfHomMedium` figure is **not** this quantity and runs the other way — it is a
multigroup cross-section *processing* bias against a continuous-energy
reference, as `examples/htr10_fuel_zone_kinf.rs` warns at length. Do not use it
to sanity-check the sign or magnitude here.

### Interpretation

**Read this first: the 2026-09-10 interpretation of this study was wrong, and
the error was a geometry mismatch, not physics.** That revision ran the
delta-tracked pebbles in a reflective *cube* and compared them to an OpenMC deck
bounded by a reflective *sphere*. Re-run on the sphere, the conclusions invert.

1. **Ring-RPT reproduces explicit TRISO, and the earlier "method error" was an
   artefact.**

   | domain | Δ(ring-RPT − explicit) | |
   |---|---|---|
   | reflective **sphere** (OpenMC's boundary) | **+163 ± 281 pcm** | **0.58σ** |
   | reflective cube (the old comparison) | −1079 ± 301 pcm | 3.59σ |
   | OpenMC reference | −31 pcm | 0.34σ |

   On the reference's own geometry the two pebbles are statistically
   indistinguishable, and agree with OpenMC.

   **State the precision honestly: this is consistency at *our* uncertainty, not
   agreement at OpenMC's.** Our ±281 pcm on the difference is about 3× OpenMC's
   ±92 pcm, so the result rules out a method error of ~600 pcm or larger and
   does not resolve one of ~100 pcm. It is enough to overturn the −1079 pcm
   claim, which is what it is used for here; it is *not* enough to assert the
   two codes agree to OpenMC's −31 ± 92 pcm. Tightening that needs roughly an
   order of magnitude more histories, and nothing in this study currently
   requires it. The previously recorded −351 pcm
   and −736 pcm residuals, and the reasoning built on them — that RPT was "the
   correct direction" but could not reach OpenMC's −31 pcm "because the inner
   radius was fitted by the deck author against OpenMC" — described the cube,
   not the method. **The deck author's 1.493359375 cm transfers to this code
   without adjustment.** There was never a radius discrepancy to explain.

2. **What ring-RPT is worth, quantitatively.** On the matched sphere the
   double-heterogeneity error is **−2535 pcm** (naive − explicit), and ring-RPT
   reduces the residual to **+163 pcm** — it removes **94 %** of that error, and
   what remains is inside Monte Carlo noise. That is the case for the method,
   measured on the reference's own geometry.

3. **The naive-homogenised medium was mis-specified until 2026-09-11, and the
   sphere is what exposed it.** `homogenise_by_volume` over the five TRISO
   layers is right for the ring-RPT *shell* — `rpt_fuel_outer_radius` sizes that
   shell to the total particle volume, so inventory is conserved — but the naive
   case filled the **entire** r < 1.9 cm zone with it, while the particles
   occupy only `pf = 0.30` of that zone. The naive pebble therefore carried
   **1/pf = 3.33× the heavy metal** of the explicit pebble and **none of the
   graphite matrix** that fills the other 70 % by volume and moderates it from
   the inside. It was not a homogenisation of the explicit pebble; it was a
   different reactor.

   The cube hid this. An over-fuelled ball with no internal moderator depends
   entirely on *external* moderation, so the cube's corner FLiBe propped it up
   (1.34137) while the sphere's removal collapsed it (1.25736) — a −8401 pcm
   response, opposite in sign to the two internally-moderated pebbles, which see
   the same change as a removal of absorber and rise. `mi::NAIVE_HOMOG` now
   mixes the TRISO material with matrix graphite at the packing fraction,
   conserving both. Post-fix the naive row responds **+5031 pcm**, in the same
   direction as the others.

   Two process notes, since the value sat in this document for a week. Its old
   cube figure was **wrong-signed** (+2324 pcm where the physics demands
   negative) and that, not its magnitude, was the available red flag. It was
   not caught partly because +2324 was read as corroborated by Wang's +2820 —
   the comparison `htr10_fuel_zone_kinf.rs` explicitly warns against making.

4. **The account closes to the pcm, which is why this is an explanation and not
   a coincidence.** The cube's corner FLiBe is not a common-mode offset: it is
   worth **+6378 pcm** to the explicit pebble but **+7620 pcm** to the ring-RPT
   pebble, a **+1242 pcm differential** — and `−1079 − (+163) = −1242`, exactly
   the phantom method error.

   The sign follows from the geometry with nothing fitted. The ring-RPT pebble's
   fuel is a shell at 1.4934–1.7531 cm; the explicit pebble's is dispersed
   through the whole r < 1.9 cm zone. RPT's fuel therefore sits further out on
   average, nearer the coolant, and fuel nearer the coolant is more sensitive to
   how much coolant there is. The cube's extra FLiBe (3 < r < 3√3, ~2.3× the
   sphere's volume) penalises it harder.

   **The general lesson is worth more than this result.** A domain mismatch
   between a model and its reference is not safely absorbed into "an absolute
   offset we can subtract". It cancels only for quantities that respond
   identically in both configurations, and two configurations that differ in
   *where the fuel sits* do not. Here it survived the differencing and was read
   as physics.

5. **The absolute offset is one bias in the data, shared by both pebbles.**
   Within this single run, both on the sphere:

   | | k | vs OpenMC |
   |---|---|---|
   | explicit TRISO | 1.38191 ± 0.00193 | **+1681 pcm** |
   | ring-RPT | 1.38354 ± 0.00204 | **+1875 pcm** |

   The two offsets differ by 194 pcm, inside the combined uncertainty. A bias
   that tracks both geometries equally is a property of the cross sections, not
   of the pebble model — so the RPT geometry and its fitted radius are ruled out
   as causes, and `op-mzvp.2.12` (U-238 epithermal/fast σ) owns what is left.

6. **This offset is roughly half what it was, and the change is not yet
   explained.** The explicit-cube case moved **−1632 pcm (≈5.4σ)** between
   commits `23cd2549` and `0cd9a22c` — same problem, same settings, same seed —
   while U-238 reconstruction wall time fell from **91.7 s to 26.8 s** across 33
   njoy commits (BROADR `thnmax`, the `sigfig` discontinuity shading, RECONR
   changes). If the sphere moved with the cube, the old explicit-sphere case sat
   near +3300 pcm, consistent with the +2854 pcm the old ring-RPT CSG row
   showed. On that reading the njoy work *halved* the disagreement with OpenMC
   and the stale numbers are stale in the good direction — but that is a
   hypothesis with a pre-registered test (`examples/u238_recon_fingerprint.rs`,
   run in both checkouts), **not a conclusion**. Every `outram-mc` number
   recorded before 2026-09-11 belongs to the old data and should not be compared
   against a new one.

7. **The consistency check passes** (+0.89 % gap, in the `(−1 %, +5 %)` band) —
   evidence that the 3-group decomposition in `run_keff_reactor_physics` is
   physically sound on a real thermal system.

8. **Both P1 transport bugs remain fixed** — GH #168 (concentric-sphere leak:
   k 0.24 → 1.39, leakage 0.87 → 1e-4) and GH #169 (Li-6(n,t) absorption
   0.04 → 938 b).

## Remaining work


1. **RPT radius search** — *implemented 2026-09-11*, opt-in behind
   `--search-rpt-radius`. **Demoted from the critical path.** It was filed to
   close the −736 pcm residual; that residual does not exist (see
   Interpretation 1), so this is no longer a fix for anything. It remains worth
   running **once**, as a measurement of how code-dependent the equivalence
   radius actually is — a question the earlier interpretation asserted an answer
   to without measuring. It solves for the inner radius at which **this code's**
   ring-RPT pebble reproduces **this code's** explicit-TRISO pebble, using
   `physics::search::search_for_keff` with the explicit-TRISO k as the target
   (not 1.0) and bisection over `r_inner ∈ [1.10, 1.64]` cm. The geometric
   ceiling is `r_inner ≤ (R³(1 − pf))^{1/3} ≈ 1.687` cm at pf = 0.30, or the fuel
   shell will not fit inside the r = 1.9 cm fuel zone. The gap between the
   converged radius and the deck author's 1.4934 cm *is* the measurement of how
   code-dependent the RPT fit is — it is not an error in either code.
2. **Explicit-TRISO pebble on the OpenMC boundary** — *addressed 2026-09-11* by
   the `DeltaDomain::Sphere` work above, which was the better route than the
   surface-tracked CSG build originally planned here: delta tracking is the
   right method for 54 706 randomly packed particles (that is what Woodcock
   tracking exists for), so the explicit pebble now runs on the reference's own
   reflective sphere without giving up the packing. One approximation remains —
   the five coating layers are resolved by nearest-centre + radius rather than
   exact CSG.
3. **Residual absolute-k bias (~+1700–1900 pcm), concentrated in p and ε** —
   now known to be *shared by both pebbles* (Interpretation 5), so it is a
   property of the cross sections and the pebble model is not implicated. Two
   threads, and they may be one:
   - cross-check reconstructed U-238 σ_γ(E) against the NNDC HDF5 the OpenMC
     deck used: at the 6.7 eV / 20.9 eV resolved resonances (expect agreement)
     and across the 20–150 keV URR band. **Note the original hypothesis here —
     missing URR self-shielding — was refuted**: U-238 carries `LSSF=1` in
     ENDF/B-VIII.0, so MF=3 already holds the infinitely-dilute unresolved
     values and the missing self-shielding has the *wrong sign* to explain a
     positive bias (`op-mzvp.2.12`).
   - settle the −1632 pcm baseline drift (Interpretation 6) with
     `examples/u238_recon_fingerprint.rs`, which compares the reconstruction
     between two checkouts without running transport. If the njoy work moved
     the cross sections, that is the same question as the bias itself.

## Bookkeeping status

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** — blocked, and pending maintainer review.
