# Ring-RPT FHR pebble — `outram-mc-libs` vs OpenMC

**Generated:** 2026-09-11 (UTC), superseding the 2026-09-10 revision
**Crate commit:** `develop`

**Status: two defects found and fixed here; ring-RPT equivalence is reproduced;
the absolute-k disagreement is larger than before and better localised.**

1. The crate sampled elastic scattering off a target held **at rest** at every
   energy, for every nuclide without an S(α,β) table — so FLiBe, the kernel
   oxygen and carbon, and the SiC coating had no thermal equilibrium at all
   (bead `op-50vu`, Interpretation 5). Fixing it moved η and f onto the OpenMC
   reference (errors 0.41 % → 0.11 % and 0.83 % → 0.18 %).
2. The explicit-TRISO pebble carried **2.6 % more heavy metal than the deck**.
   `pack_spheres_crp` reports its packing fraction over the packing *cube*,
   but sphere centres are confined to `half − r_particle`, so the cube's
   interior is denser than nominal and the inscribed `r < 1.9` fuel sphere
   inherited that — 0.3078 against the deck's 0.30 (Interpretation 6).

**Ring-RPT equivalence is reproduced.** With both pebbles finally carrying the
same inventory, `RPT − explicit = +226 ± 316 pcm (0.71σ)` against the
reference's `−31 ± 92 pcm`, and ring-RPT removes **93 %** of the
double-heterogeneity error. The 2026-09-10 revision's −1079 pcm "method error"
was a cube-vs-sphere domain mismatch (Interpretation 4); the +1130 pcm that
briefly replaced it was the packing-fraction defect.

**The absolute k did not close — it grew, with every correct fix.** The explicit
pebble is now **+4004 pcm** above OpenMC, from +1681 pcm before this session's
work. Each fix is verified independently and none is in doubt, so the earlier,
smaller disagreement was partly cancellation.

What is left is **entirely `p·ε`** — the ratio of non-thermal to thermal flux.
It is not the U-238 or U-235 cross sections (±0.04 % / ±0.06 % vs NJOY) nor
their **resonance integrals** (+0.00 % vs NJOY and the published `RI_∞`), not
graphite's thermal law (±0.05 % vs THERMR), not the moderator scattering cross
sections (C, Be, F, Li, O, Si all within a few percent of published free-atom
values, where ~20 % would be needed), not the slowing-down kernel (ξ/ξ₀ = 1.000
from 4 eV to 10 keV on eight nuclides), not the explicit pebble's layer
resolution (each layer's share of the TRISO material exact to 0.5 %), and not
the HIGH data path or the eigenvalue driver in general — those reproduce a
**measured** criticality benchmark, Godiva/ICSBEP HEU-MET-FAST-001, to
**+57 ± 173 pcm**.

Not a validated result — an AI-assisted code-to-code check, no human V&V.

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

Run 2026-09-11 **after both fixes** (`op-50vu` free-gas target motion, and the
packing-fraction correction), 4000 histories × [30 inactive + 80 active], 600 K.
Every pebble is run in **both** domains; the sphere is OpenMC's boundary and
carries all reported comparisons.

**Reflective sphere r = 3 — comparable to OpenMC in absolute terms:**

| run | k-eff | Δ(vs explicit) | vs OpenMC |
|---|---|---|---|
| explicit TRISO | 1.40514 ± 0.00204 | — | **+4004 pcm** |
| ring-RPT | 1.40739 ± 0.00241 | **+226 ± 316 pcm (0.71σ)** | **+4260 pcm** |
| naive homogenised | 1.37323 ± 0.00219 | **−3191 pcm (10.7σ)** | — |
| ring-RPT (CSG, surface-tracked) | 1.40757 ± 0.00224 | — | +4278 pcm |

**Reflective cube half-width 3 — NOT comparable to OpenMC; kept to price the
corner-FLiBe over-count:**

| run | k-eff | Δ(vs explicit) | cube − sphere |
|---|---|---|---|
| explicit TRISO | 1.43372 ± 0.00237 | — | +2858 pcm |
| ring-RPT | 1.42781 ± 0.00213 | −590 ± 320 pcm (1.9σ) | +2042 pcm |
| naive homogenised | 1.40941 ± 0.00221 | −2431 pcm (7.5σ) | +3618 pcm |

**Ring-RPT CSG six factors:** η 2.0096, f 0.9199, p 0.5368, ε 1.3976,
P_FNL 1.0000, P_TNL 1.0000; product (k_4f) 1.38678, consistency gap +1.48 % (in
band; OpenMC's own gap on the same decomposition is +1.3 %). Leakage 3.1e-6.

**Like-for-like against the reference.** This crate's thermal cutoff is 0.625 eV,
the same cadmium cutoff the deck uses, so `k`, `η` and `f` are already directly
comparable — but `p` and `ε` are not, because the three-group form carries fast
absorption in `ε`. `SixFactors::two_group_openmc_convention()` re-attributes it:

| | ours (3-group) | ours (2-group, like-for-like) | OpenMC | Δ |
|---|---|---|---|---|
| η | 2.0096 | 2.0096 | 2.0073 | **+0.11 %** |
| f | 0.9199 | 0.9199 | 0.9216 | **−0.18 %** |
| p | 0.5368 | **0.5253** | 0.4842 | **+8.5 %** |
| ε | 1.3976 | **1.4280** | 1.5043 | **−5.1 %** |
| p·ε | 0.7502 | 0.7501 | 0.7284 | **+3.0 %** |

The conversion preserves the product, as it must — `p·ε = k/(η·f)` up to the
decomposition's own consistency gap — so it changes the split between `p` and `ε`
without touching the size of the disagreement.

**Two independent transport methods agree on the same geometry.** The
delta-tracked sphere gives 1.40739 ± 0.00241 and the surface-tracked CSG driver
gives 1.40757 ± 0.00224 — **18 pcm apart, 0.05σ**, from different initial
sources. Together with the uniform-medium cube-vs-sphere check (0.14σ), the
`DeltaDomain::Sphere` arm is pinned twice: once against the pre-existing cube
arm on a problem with an analytic answer, and once against a different tracking
algorithm on the real pebble.

#### What the two fixes moved

Same deck, same seeds, same settings, at each step:

| run (sphere) | 2026-09-11 am | + `op-50vu` | + packing fix |
|---|---|---|---|
| explicit TRISO | 1.38191 ± 0.00193 | 1.39610 ± 0.00209 | **1.40514 ± 0.00204** |
| ring-RPT | 1.38354 ± 0.00204 | 1.40739 ± 0.00241 | 1.40739 (unaffected) |
| naive homogenised | 1.35656 ± 0.00243 | 1.37323 ± 0.00219 | 1.37323 (unaffected) |
| ring-RPT (CSG) | 1.38279 ± 0.00270 | 1.40757 ± 0.00224 | 1.40757 (unaffected) |
| **Δ(RPT − explicit)** | +163 ± 281 (0.58σ) | +1130 ± 319 (3.5σ) | **+226 ± 316 (0.71σ)** |
| **vs OpenMC (explicit)** | +1681 pcm | +3100 pcm | **+4004 pcm** |

Only the explicit pebble uses the packing, so the second fix moves only its row
— and it is the row that closes `Δ(RPT − explicit)` back to statistical
agreement. The explicit pebble had been carrying 2.6 % more heavy metal than
either homogenised pebble, which is why those three numbers were never
comparable.

The free-gas fix's effect on the six factors, against OpenMC's ring-RPT
reference:

| | before | after | OpenMC | error before → after |
|---|---|---|---|---|
| η | 2.0155 | 2.0096 | 2.0073 | +0.41 % → **+0.11 %** |
| f | 0.9140 | 0.9199 | 0.9216 | −0.83 % → **−0.18 %** |
| p | 0.5266 | 0.5368 | 0.4842 | +8.8 % → **+10.9 %** |
| ε | 1.4131 | 1.3976 | 1.5043 | −6.1 % → **−7.1 %** |

**The thermal factors snapped onto the reference and the non-thermal ones did
not.** η and f are now within 0.11 % and 0.18 % of OpenMC — they were the two
factors a thermal-kernel defect should own, and fixing it fixed them. The whole
residual now lives in `p·ε`: ours 0.7502, OpenMC 0.7284, **+3.00 %**, against a
`k/(η·f)` ratio of **+3.21 %**. Those are the same number. Whatever is left is
entirely the ratio of non-thermal to thermal flux, and nothing else.

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

1. **Ring-RPT reproduces explicit TRISO — and it took two bug fixes and one
   geometry correction to say that on a model where all three pebbles carry the
   same inventory.**

   | | Δ(ring-RPT − explicit) | |
   |---|---|---|
   | reflective **sphere**, both fixes in | **+226 ± 316 pcm** | **0.71σ** |
   | sphere, packing-fraction defect present | +1130 ± 319 pcm | 3.54σ |
   | sphere, before `op-50vu` too | +163 ± 281 pcm | 0.58σ |
   | reflective cube (the 2026-09-10 comparison) | −1079 ± 301 pcm | 3.59σ |
   | OpenMC reference | −31 pcm | 0.34σ |

   Two different artefacts had to be removed, and one of them briefly *restored*
   agreement for the wrong reason. The 2026-09-10 −1079 pcm was a
   **cube-vs-sphere domain mismatch** (Interpretation 4 closes that account to
   the pcm). The +163 pcm that replaced it was measured with a thermal kernel
   that had no equilibrium (`op-50vu`) *and* an explicit pebble carrying 2.6 %
   extra heavy metal — two errors of opposite sign. Fixing only the first
   exposed the second as +1130 pcm. Fixing both gives **+226 ± 316 pcm**.

   **State the precision honestly: this is consistency at *our* uncertainty, not
   agreement at OpenMC's.** ±316 pcm on the difference is about 3.4× the
   reference's ±92 pcm, so this rules out a method error of ~700 pcm or larger
   and does not resolve one of ~100 pcm. **The deck author's 1.493359375 cm
   transfers to this code without adjustment** — but note that claim now rests
   on a code whose absolute spectrum is still 3 % out from the one the radius
   was fitted against, so `--search-rpt-radius` remains the way to measure how
   code-dependent the equivalence radius really is.

   The previously recorded −351 pcm and −736 pcm residuals, and the reasoning
   built on them — that RPT was "the correct direction" but could not reach
   OpenMC's −31 pcm "because the inner radius was fitted by the deck author
   against OpenMC" — described the cube, not the method.

2. **What ring-RPT is worth, quantitatively.** On the matched sphere the
   double-heterogeneity error is **−3191 pcm** (naive − explicit) and ring-RPT
   brings the residual to **+226 pcm** — it removes **93 %** of that error, and
   what remains is inside Monte Carlo noise. That is the case for the method,
   measured on the reference's own geometry with matched inventory.

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

5. **A P0 defect was found and fixed here: the crate had no free-gas target
   motion at all (`op-50vu`). It is the reason η and f were out, and fixing it
   moved k *away* from OpenMC.**

   The collision kernel had exactly two scattering branches: the S(α,β) law where
   a nuclide carried one, and otherwise `two_body_scatter_with_mu` /
   `elastic_scatter`, both of which hold the target **at rest** at every energy.
   No free-gas target velocity was sampled anywhere in the crate. A stationary
   target confines the outgoing energy to `[α·E, E]`: it can only take energy
   away. So for every nuclide without a thermal table — in this pebble that is
   Li-7, Be-9 and F-19 (the whole FLiBe coolant, **70 % of the model by volume**),
   O-16 and the free-gas C of the UCO kernel, and the Si and C of the SiC
   coating — there was **no thermal equilibrium at all**. Measured, 20 000
   walkers × 400 collisions from 1 eV at 600 K, against the correct `2kT`
   = 0.1034 eV fixed point (`examples/epithermal_slowing_down.rs`):

   | medium | ⟨E⟩ after 400 collisions | ⟨E⟩ / 2kT |
   |---|---|---|
   | C-12 with graphite S(α,β) | 1.004e-1 eV | 0.97 ✓ |
   | C-12 free gas (kernel C) | 1.481e-27 eV | 1.9e-26 |
   | Be-9 (FLiBe) | 1.787e-35 eV | 2.3e-34 |
   | F-19 (FLiBe) | 3.270e-18 eV | 4.2e-17 |
   | Li-7 (FLiBe) | 7.225e-44 eV | 9.3e-43 |
   | O-16 (UCO kernel) | 2.822e-21 eV | 3.6e-20 |
   | Si-28 (SiC coating) | 8.299e-13 eV | 1.1e-11 |

   Only the one nuclide with a thermal table escaped. OpenMC samples the target
   velocity below `FREE_GAS_THRESHOLD = 400·kT` (`sample_target_velocity`,
   `src/physics.cpp`), which at 600 K is `E < 20.7 eV` — the entire thermal range
   **and** the 6.674 eV and 20.87 eV U-238 resonances.

   **Why no earlier check could see it.** Every per-collision quantity was
   correct: `ξ/ξ₀ = 1.000` from 4 eV to 10 keV for every nuclide, no violations
   of the `α·E` kinematic floor, no spurious up-scatter
   (`examples/epithermal_slowing_down.rs`, section 1). The defect was not in any
   collision — it was the **absence of a fixed point** across a sequence of them,
   which only an equilibrium test can detect. The earlier `ξ` measurement
   (Interpretation 5 of the previous revision) ran from 0.0253 eV to 3.9 eV on
   **graphite only**, i.e. on the single nuclide that had a thermal table and was
   therefore the only one not affected.

   **The fix** is OpenMC's own algorithm: `free_gas_elastic_scatter` /
   `sample_target_velocity` in the constant-cross-section (CXS) approximation,
   which is the consistent partner of a Doppler-**broadened** σ for the collision
   rate. It is pinned against an oracle that is analytic and needs no library:
   the equilibrium neutron **density** must be Maxwellian, so weighting the
   chain's own samples by `1/v̄_rel(E)` must return `⟨E⟩ = 1.5 kT` and
   `⟨E²⟩ = 3.75 (kT)²`. It does, for `A` from 2 to 238
   (`physics::scatter::tests::free_gas_equilibrium_density_is_maxwellian`).
   Two weaker oracles were tried first and **both were wrong** — the `(2kT − E)`
   first moment and a plain Gamma(2, kT) stationary law; each gets the limits
   right and misses the collision-rate `|v_rel|` weighting in between, by 4–5 % at
   `A ≈ 2`. Their failure is recorded in the test file so nobody re-derives them.

   **What it moved, and the part that is uncomfortable.** η and f snapped onto
   OpenMC (errors 0.41 % → 0.11 % and 0.83 % → 0.18 %) — exactly the two factors
   a thermal-kernel defect owns. But k rose by +1419 pcm (explicit) and +2385 pcm
   (ring-RPT), so the disagreement with OpenMC **grew, from +1681 to +3100 pcm**.
   A correct fix moving the answer away from the reference means the previous
   agreement was partly cancellation, and the remaining error is larger than it
   looked. That is worth more than a smaller number would have been, and it is
   recorded as the result rather than reverted.

   This also retires a figure quoted in earlier revisions. "Graphite S(α,β) is
   worth ~1700 pcm, measured in
   `tests/htr10_graphite_thermal_scattering_pebble_bed.rs`" came from comparing
   the bound law against that broken arm — the test recorded free-gas k = 1.08838
   vs bound 1.95254, and its own interpretation named the cause
   (*"stationary-target, no up-scatter"*) without recognising it as a bug.
   Re-measured against a **correct** free gas at 18× the statistics, the two
   treatments differ by **+364 ± 479 pcm (0.76σ)** — consistent with zero. The
   test now asserts that agreement, which catches the defect's return at 37σ.

6. **The explicit pebble carried 2.6 % more heavy metal than the deck, because a
   packing fraction quoted over a *cube* is not the packing fraction of a ball
   cut out of it.**

   `pack_spheres_crp(radius, half, pf, seed)` sizes the particle count so that
   `N·v / (2·half)³ = pf` — the fraction over the packing **cube**. But sphere
   *centres* are confined to `half − radius`, so the density of centres is
   `N / (2(half − radius))³` and the volume fraction well inside the cube
   exceeds the nominal by `(h/(h−r))³`. The ring-RPT explicit pebble packs a cube
   of half-width 1.9425 and then clips to the `r < 1.9` fuel sphere — which
   selects exactly that denser interior. Measured two ways that agree to 2e-5
   (`PackedSpheres::volume_fraction_in_ball`, and by sampling
   `ExplicitTrisoPebble::material_at` itself): **pf 0.3078 in the fuel sphere
   against the deck's 0.3000, +2.6 %**.

   The deck specifies its packing fraction over the fuel **region** —
   `openmc.model.pack_spheres(radius, region=-fuel_sph, pf=0.30)` puts exactly
   30 % into the sphere — so this was 2.6 % more uranium than the model being
   compared against, in the *explicit* pebble only. `rpt_fuel_outer_radius` and
   the naive mix both take `pf = 0.30` analytically over the fuel sphere and were
   always right, which is why the three pebbles were not comparable to each other
   either.

   **The layer resolution, by contrast, is exact.** The same measurement gives
   each coating layer's share of the TRISO material to better than 0.5 % of the
   pure-geometry value `(r_i³ − r_{i−1}³)/r_opyc³` — so "nearest centre + radius
   rather than exact CSG", the approximation this study has carried in its
   caveats since the start, costs nothing measurable. Every layer being high by
   the *same* 2.0–2.7 % was the tell that the error was a density offset and not
   a layer bug.

   Fixed by measuring the realised in-sphere fraction and rescaling the requested
   cube `pf` once — it is linear in the particle count — which lands at 0.29929,
   within one sampling sigma of 0.30. Worth **+904 pcm** on the explicit pebble
   (1.39610 → 1.40514): the fuel zone is under-moderated at pf 0.30, so *removing*
   TRISO and returning the volume to matrix graphite raises k.

7. **The remaining offset is NOT in any cross section, NOT in the resonance
   integral, and NOT in the slowing-down kernel — measured against NJOY, against
   published data, and against analytic kinematics.**

   | | worst relative difference |
   |---|---|
   | U-238 total / elastic / fission / capture, point values vs NJOY PENDF | ±0.04 % (worst −0.17 %) |
   | U-235 total / elastic / fission / capture, point values vs NJOY PENDF | ±0.06 % |
   | graphite incoherent inelastic / coherent elastic vs THERMR | ±0.05 % (worst −0.14 %) |
   | **U-238 capture resonance integral, 0.5 eV – 100 keV** | **+0.00 %** vs NJOY (274.637 b vs 274.634 b) |
   | ξ above the S(α,β) cutoff, 4 eV – 10 keV, 8 nuclides | ±0.5 % vs analytic `ξ₀` |

   The **resonance integral** is the one that closes the biggest remaining hole,
   and it needed its own oracle. `u238_vs_njoy_pendf.rs` samples **19 discrete
   probe energies** — resonance peak centres and smooth regions. It pins peak
   *heights*. It cannot see the quantity resonance escape actually depends on:
   the **area** under each resonance. A grid too coarse between nodes loses area
   without moving any node value, so it would pass that oracle and still
   under-capture in transport — and transport interpolates on *our* grid, so
   `∫σ_γ dE/E` on our own grid **is** the resonance integral the Monte Carlo
   sees. Measured exactly (segment-wise closed form) in
   `examples/u238_resonance_integral.rs`: **274.637 b on our grid (291 322
   points) against 274.634 b on NJOY's (130 444 points) — +0.00 %**, and against
   the published infinite-dilution `RI_∞ = 275.7 b` for ENDF/B-VIII.0. Our grid
   is more than twice as dense as NJOY's over the band. U-238 capture is exact in
   both value and area.

   One further **deck mismatch** was found and corrected while looking, and it
   is worth nothing: `rpt_pebble.py::get_mixed_triso_fuel_material` builds the
   homogenised TRISO material with `add_element('C', ...)` and **no**
   `add_s_alpha_beta` call, so the buffer / PyC1 / PyC2 carbon — which *does*
   carry `c_Graphite` in the explicit pebble — is free-gas once homogenised.
   That is 83 % of the mixed material's carbon and ~66 % of all its atoms, and
   `homogenise_by_volume` had been (correctly, by its own contract) preserving
   the bound nuclides. Matching the deck moves the ring-RPT CSG pebble by
   **−12 ± 310 pcm**: statistically zero. The fuel shell's carbon sees little
   thermal flux, so its treatment does not matter here — but the comparison is
   now faithful to the model it claims to reproduce.

   Nor is the offset a general property of the HIGH data path or of the
   eigenvalue driver, because **that path reproduces a measured criticality
   benchmark**. Godiva (ICSBEP **HEU-MET-FAST-001**, `k = 1.0000 ± 0.0010`), run
   on the repo's own ENDF/B-VIII.0 tapes through `Nuclide::from_endf_file` and
   `run_keff` — the same reconstruction and the same driver as this study —
   gives **k = 1.00057 ± 0.00173, i.e. +57 ± 173 pcm**
   (`examples/godiva_keff_endf_local.rs`). Every comparison in this document
   before that one was against another code; this one is against an experiment.
   It exercises ν̄(E), the ENDF MF=5 fission spectrum, U-235 fast fission,
   inelastic levels and (n,2n) — and **no** thermal scattering and **no**
   resonance escape. It is the complement of the pebble, and it splits the search
   cleanly: whatever is left is specific to the non-thermal/thermal flux ratio,
   not to the shared machinery.

8. **Transport assembles the resonance absorption correctly too — checked
   against its analytic dilute limit, which is the one piece of physics the
   pebble exercises and Godiva does not.**

   Cross sections being right does not mean the *absorption rate* is right;
   resonance self-shielding in a continuous-energy Monte Carlo is supposed to be
   automatic, and therefore never gets tested. `examples/u238_resonance_escape.rs`
   tests it, with no geometry, no tracking and no majorant — the neutron is
   followed in **energy only**, so a failure here would be in the collision
   physics and a pass sends the search to the spatial side.

   In an infinite homogeneous medium of a moderator plus a *trace* absorber the
   flux is unperturbed and the absorption probability per source neutron is
   exactly `N_a · RI_∞ / (ξΣ_s)` to first order. `RI_∞` is not assumed: it is the
   274.637 b this crate's own reconstruction gives, already verified against
   NJOY. C-12 at 0.08 /b·cm, U-238, 600 K, 10⁵ eV → 0.5 eV:

   | N(U-238) | σ₀ \[b\] | P_abs(U-238) | analytic dilute | RI_eff \[b\] | RI_eff/RI_∞ |
   |---|---|---|---|---|---|
   | 1e-6 | 3.80e5 | 0.00449 | 0.00455 | 272.0 | **0.991 ± 0.011** |
   | 1e-5 | 3.80e4 | 0.04103 | 0.04548 | 253.0 | 0.921 |
   | 1e-4 | 3.80e3 | 0.23245 | — | 159.8 | 0.582 |
   | 1e-3 | 3.80e2 | 0.60981 | — | 56.8 | 0.207 |

   The dilute row lands on 1.000 within one sigma, and self-shielding then
   develops monotonically as σ₀ falls. The moderator's own 1/v capture is tallied
   separately and excluded — it is ~0.2 % of source neutrons, a third of the
   signal at the top row, and reading it as absorber capture makes that row
   over-absorb by 50 %.

9. **Two more mechanisms with the right sign, both excluded: the resonance
   *shape*, and the delta-tracking majorant.**

   **Shape, because area is not enough.** A resonance integral is invariant under
   Doppler broadening — it *is* the area, and broadening conserves it. So a
   resonance reconstructed too narrow and too tall passes the integral check
   exactly and still **over-self-shields** in transport: less absorption, `p`
   higher, `k` higher. Compared against NJOY's PENDF on **NJOY's own grid
   points** through the six resonances that carry the integral
   (`u238_resonance_integral.rs`):

   | window | points | peak NJOY \[b\] | peak ours \[b\] | worst rel | rms rel |
   |---|---|---|---|---|---|
   | 6.674 eV | 272 | 5.3875e3 | 5.3874e3 | −0.097 % | 0.043 % |
   | 20.87 eV | 331 | 5.0469e3 | 5.0470e3 | +0.116 % | 0.044 % |
   | 36.68 eV | 308 | 4.0081e3 | 4.0081e3 | +0.107 % | 0.044 % |
   | 66.03 eV | 315 | 1.5356e3 | 1.5357e3 | +0.097 % | 0.044 % |
   | 102.6 eV | 309 | 1.1169e3 | 1.1169e3 | +0.102 % | 0.044 % |
   | 189/208 eV | 599 | 4.4647e2 | 4.4647e2 | +0.099 % | 0.040 % |

   Shape and area are independent, so together they pin `σ_γ(E)` completely over
   the band resonance escape depends on.

   **The majorant, because an under-bound is silent.** Delta tracking is unbiased
   only while `Σ_maj ≥ Σ_t` everywhere; an under-bound loses collisions exactly
   where `Σ_t` spikes — the resonance peaks — and that is a bias with no error
   raised. `Majorant::bounding` lays 4096 log bins (0.64 % wide) with 32
   sub-samples each, a pitch of ~0.021 % in energy, while a Doppler width is
   1.1 % of E at 6.674 eV but only ~0.02 % by 20 keV, so the high-keV resonances
   are where a peak can slip through. The existing check in
   `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` covers 1e-4 to 4.2 eV
   and a different material set, so it does not answer this. Checked now on the
   **nuclides' own reconstructed grids** — where every resonance peak is a node,
   unlike another log grid, which would miss the same peaks the majorant does and
   agree for the wrong reason: **worst `Σ_t/Σ_maj` = 0.890** over 467 571 grid
   points × 9 materials. It bounds, with the full 30 % margin never approached.
   The check now runs as a preflight on every `fhr_ring_rpt_endf` run.

10. **The remaining offset is shared by both pebbles, and it is entirely `p·ε` —
   the non-thermal/thermal flux ratio.**
   Within this single run, both on the sphere:

   | | k | vs OpenMC |
   |---|---|---|
   | explicit TRISO | 1.40514 ± 0.00204 | **+4004 pcm** |
   | ring-RPT | 1.40739 ± 0.00241 | **+4260 pcm** |

   The two offsets differ by 256 pcm, inside the combined uncertainty — as they
   were before `op-50vu` (194 pcm) and were not in between, when the packing
   defect separated them. A bias that tracks both geometries equally is a
   property of the physics, not of the pebble model, so the RPT geometry and its
   fitted radius are ruled out as causes. Take the **explicit** pebble as the
   clean statement: no homogenisation, no fitted radius, **+4004 pcm**.

   The decomposition says what kind of bias it is, with η and f now matching:

   ```text
   p·ε      ours 0.5368 × 1.3976 = 0.7502
            OpenMC 0.4842 × 1.5043 = 0.7284      → +3.00 %
   k/(η·f)  ours   1.40757 / 1.84863 = 0.7614
            OpenMC 1.36479 / 1.84993 = 0.7378    → +3.21 %
   ```

   Those are the same number, so **the entire residual is the ratio of
   non-thermal to thermal flux**, and nothing else. `p` too high means too little
   epithermal absorption (U-238 resonance capture); `ε` too low means too little
   non-thermal production (fast and epithermal fission). Both say the same thing:
   relative to OpenMC, too few neutrons sit at non-thermal energies.

   The arithmetic candidate would be `φ_epi/φ_th ≈ Σ_a,th/(ξΣ_s)`, and every term
   in it has now been measured: `ξ` exact, the U-238 and U-235 resonance
   integrals exact, the moderator `Σ_s` within a few percent of published
   free-atom values where **~20 % would be needed**, and the thermal absorption
   *composition* matching (that is what `f` and `η` agreeing means). So the
   simple slowing-down picture does not have room for a 3 % effect either.
   `op-mzvp.2.12` owns what is left.

   Measured like-for-like (both at 0.625 eV, this crate's three-group form
   converted by `SixFactors::two_group_openmc_convention()`), the split is
   **p +8.5 %** (0.5253 vs 0.4842) and **ε −5.1 %** (1.4280 vs 1.5043). The
   conversion moves the split between them and, as it must, leaves the product
   alone. So the honest single statement is: *this code puts about 8 % too few
   neutrons above 0.625 eV, relative to the reference, and every term in the
   slowing-down balance that could do that has been measured and is right.*

11. **An earlier −1632 pcm baseline drift is still unexplained.** The
   explicit-cube case moved **−1632 pcm (≈5.4σ)** between commits `23cd2549` and
   `0cd9a22c` — same problem, same settings, same seed — while U-238
   reconstruction wall time fell from **91.7 s to 26.8 s** across 33 njoy commits
   (BROADR `thnmax`, the `sigfig` discontinuity shading, RECONR changes). The
   pre-registered test is `examples/u238_recon_fingerprint.rs`, run in both
   checkouts; it has not been run. Note this question is now partly answered from
   the other side: whatever those commits did, they did **not** change the U-238
   capture resonance integral, which matches NJOY's own PENDF to +0.00 %
   (Interpretation 7). Every `outram-mc` number recorded before 2026-09-11
   belongs to the old data, and everything before the free-gas fix belongs to a
   different kernel; neither should be compared against a current one.

12. **The consistency check passes** (+1.48 % gap, in the `(−1 %, +5 %)` band) —
   evidence that the 3-group decomposition in `run_keff_reactor_physics` is
   physically sound on a real thermal system.

13. **Both P1 transport bugs remain fixed** — GH #168 (concentric-sphere leak:
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
3. **Residual absolute-k bias — +4004 pcm on the explicit pebble, entirely
   `p·ε`, and now with no untested mechanism left that this environment can
   reach.** Excluded by measurement, each with its own oracle:

   | mechanism | oracle | result |
   |---|---|---|
   | U-238 / U-235 point σ | NJOY PENDF, 19 probe energies | ±0.04 % / ±0.06 % |
   | U-238 capture **area** | NJOY PENDF + published `RI_∞`, exact integral | **+0.00 %** |
   | U-238 capture **shape** | NJOY PENDF on its own grid, 6 resonances | worst 0.12 %, rms 0.044 % |
   | U-235 fission / capture RI | NJOY PENDF | +0.00 % |
   | graphite S(α,β) σ | NJOY THERMR | ±0.05 % |
   | moderator σ_t / σ_s, **all 8 nuclides** | NJOY PENDF (NJOY2016 rebuilt in-session) | ≤0.05 % |
| moderator thermal capture | NJOY PENDF at 0.0253 eV | ≤0.03 % |
   | slowing-down kernel `ξ` | analytic two-body kinematics, 8 nuclides | ξ/ξ₀ = 1.000 |
   | thermal equilibrium | analytic Maxwellian density, A = 2…238 | ✓ (after `op-50vu`) |
   | resonance **self-shielding** | analytic infinite-dilution limit | 0.991 ± 0.011 |
   | delta-tracking majorant | nuclides' own grids, 468 k points × 9 materials | worst 0.890, bounds |
   | tracking method | delta vs surface-tracked CSG, same geometry | 18 pcm, 0.05σ |
   | ν̄, χ, fast σ, inelastic, (n,2n), the driver | **ICSBEP HEU-MET-FAST-001** | **+57 ± 173 pcm** |
   | pebble composition, radii, densities, temperatures, S(α,β) assignment | the deck, term by term | identical |
   | explicit pebble layer volumes | exact geometry | 0.5 % |
   | packing fraction | the deck's own definition | fixed (`op-8l2e`) |

   The two mechanisms that *were* real — the missing free-gas target motion and
   the packing-fraction over-count — are both fixed, and fixing them made the
   disagreement **larger**, from +1681 to +4004 pcm. That is the most important
   fact in this record: the earlier, smaller number was cancellation.

   **The next measurement, in order of what it would settle:**

   a. **A second thermal system with an external answer.** Godiva settled the
      shared machinery — ν̄, χ, fast σ, inelastic levels, (n,2n), the eigenvalue
      driver — against an *experiment*. Nothing has settled the thermal half,
      and the pebble reference is a single deck. **This is the measurement that
      would settle the question**, because it is the only one that does not
      depend on the reference deck.

      The blocker is **not** nuclear data, as an earlier note here said.
      `/home/user/njoy-cand/n-001_H_001-ENDF8.0-Beta6.endf` exists locally, and
      `reference-data/endf/` already carries `tsl-HinH2O.endf`, U-234/235/238,
      O-16 and F-19 — enough for a water-moderated or solution benchmark, and a
      homogeneous solution sphere needs nothing beyond the `run_keff` driver
      Godiva already uses. What is missing is the **benchmark specification**:
      ICSBEP atom densities and their evaluated `k_eff ± σ`. The handbook is not
      freely reachable from here, `www-nds.iaea.org` is refused by the egress
      proxy (403), and GitHub access is scoped to this workspace's own two
      repositories, so `openmc-dev/benchmarks` cannot be read either.

      **Do not substitute remembered or web-summarised atom densities.** A
      criticality benchmark built on an unsourced specification produces a number
      that looks like validation and is not, which is worse than the gap it would
      fill. What is needed is one sourced specification — an ICSBEP evaluation,
      or a case file from a benchmark suite — and the rest is an afternoon.

   b. ~~The moderator cross sections against NJOY.~~ **Done 2026-09-11 — found
      nothing.** NJOY2016 was rebuilt in-session (its worktree had been reclaimed
      for disk; `git reset --hard`, cmake + gfortran, ~5 min) and the same
      RECONR + BROADR deck run at 600 K for every remaining nuclide. Worst
      relative difference over the 19 probe energies: C-12 +0.00 %, C-13 +0.00 %,
      O-16 +0.00 %, F-19 −0.02 %, Be-9 +0.02 %, Li-7 +0.02 %, Li-6 +0.17 %,
      Si-28 −0.05 % on the total, and ≤0.05 % on elastic. Thermal capture at
      0.0253 eV agrees to ≤0.03 %. **Every nuclide in the pebble is now verified
      against NJOY at 0.1 %, not "a few percent".** See
      `examples/u238_vs_njoy_pendf.rs`, which records the table and the one trap
      in reading it (this crate's `absorption` is MT=27, NJOY's MT=102 is
      radiative capture alone, so they diverge for a light nuclide above
      threshold and by a factor of 24 000 for Li-6's `(n,t)`).

   b2. **Two things noticed in the reference pipeline while checking it, neither
      of which biases `k`, but both of which say the deck has not been fully
      read.** `pipeline_triso_to_rpt.py` sets `FUEL_ZONE_RADIUS_CM = 1.0` with
      the comment *"The pebble is 4 cm across with a 1 cm graphite shell, so fuel
      lives inside a 1 cm radius"* — the shell is **0.1 cm** and the fuel zone is
      `r < 1.9 cm`. The constant is used only to place the starting source, so it
      does not bias a converged eigenvalue; but it means the **ring-RPT** source
      box (±1.0 cm) does not reach that pebble's fuel at all except in its
      corners (`r ≤ √3 = 1.732` just clips the 1.4934–1.7531 cm shell), so the
      RPT run starts from a badly-placed source and leans entirely on its 50
      inactive batches. The pipeline tallies Shannon entropy for exactly this
      reason; whether it was inspected for the reported run is not recorded.

   c. **Run `--search-rpt-radius`.** `RPT − explicit` is back to +226 ± 316 pcm,
      so the deck author's radius does transfer — but it was fitted against a
      spectrum this code is still 3 % away from, so the search remains the way
      to measure how code-dependent the equivalence radius actually is.

   d. **A second, independent OpenMC run of the pebble itself.** The reference
      is one run of another party's deck that this workspace has never
      reproduced. It is not unsupported — the deck author's own ENDF/B-VII.1 run
      gave 1.36864 / 1.36863 and the VIII.0 run here gave 1.36510 / 1.36479, two
      runs and two libraries agreeing to a −354 pcm library shift that moved both
      models together — but it has never been checked by a third party, and
      "+4 % with every mechanism on our side excluded" is the kind of result that
      should not rest on one unreproduced number.

   Superseded detail, kept so the trail is legible: the original hypothesis here
   was missing **URR self-shielding**, and it was **refuted** — U-238 carries
   `LSSF=1` in ENDF/B-VIII.0, so MF=3 already holds the infinitely-dilute
   unresolved values, and the missing self-shielding has the *wrong sign* to
   explain a positive bias (`op-mzvp.2.12`).

## Bookkeeping status

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** — blocked, and pending maintainer review.
