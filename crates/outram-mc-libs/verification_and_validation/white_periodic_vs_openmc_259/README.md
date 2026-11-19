# White and periodic boundaries vs OpenMC, seed ensembles (GitHub #259)

Research and V&V only (`RESPONSIBLE_USE.md`). Status of this file: **PREDICTION
STAGE, written and committed before any run of the cases below** (2026-11-18).
Results are appended in a later commit; this section is not edited afterwards.

## What is already known (disclosed so the prediction is not hindsight)

* `../white_boundary/white_boundary_vs_openmc.md` (2026-09-22) compared white
  vs reflective on a 1-D slab against OpenMC `afa7a14` with **single runs**.
  Its largest miss was T = 1.0 white at 2.3 sigma of a combined budget built
  from single-run sigma, which ignores inter-generation correlation.
* `../rotational_periodic/README.md` (2026-11-14) showed rotational periodic is
  self-consistent against a full model, but never against OpenMC.
* Translational periodic (`BoundaryType::PeriodicWith`) has only
  single-particle track tests. No eigenvalue has been compared with OpenMC.

So two #259 acceptance items are open: **(A)** white vs reflective on a unit
cell matched to OpenMC with proper statistics, **(B)** a non-mirror-symmetric
lattice whose k differs between periodic and reflective, matched to OpenMC.

## Methodology (fixed now)

Both codes run **multi-group, one group**, so no nuclear data enters and the
comparison isolates transport + boundary handling. OpenMC 0.15.3 commit
`27e38e894697bb32a1dac7848d2618818b6b8daf`, built from source, no MPI.
Source of truth for the algorithm: OpenMC `src/boundary_condition.cpp`
(WhiteBC, TranslationalPeriodicBC) and `Surface::diffuse_reflect`.

**Statistic.** Per code, per configuration, 16 independent seeds (1..16). Each
seed gives one run mean `k_s` (all active batches). Report the ensemble mean
and the standard error over seeds (`sd/sqrt(N)`), never the single-run sigma.
Code-to-code: `z = (kA - kB) / sqrt(semA^2 + semB^2)`.
**Gate: |z| <= 2** on every comparison below. Not tunable.
Settings, both codes: 20 000 particles per generation, 20 inactive + 100
active generations. Seeds are the integers 1..16 in both codes.

**Replication plan, fixed before any run.** A comparison with |z| > 2 is
reported as a MISS in the table, then re-run once with a disjoint block of
seeds 17..32 and the pooled N = 32 reported alongside. The first-block miss
stays in the record. No other change (settings, XS, geometry) is allowed.

### Case A, white vs reflective slab (re-run of the 2026-09-22 geometry)

Slab x in [0, T], y,z in [-1, 1] reflective, left face carries the BC under
test, right face vacuum. One-group set `fuel`: total 1.0, absorption 0.30
(includes fission), fission 0.20, nu-fission 0.50, chi 1, scatter 0.70.
T in {0.5, 1.0, 3.0}. (T = 12 dropped: the effect is 3.5 sigma in OpenMC and
discriminates nothing.) Quantities compared: k(reflective), k(white) and
d = k(white) - k(reflective), for each T.

### Case B, periodic vs reflective on a rod lattice

Box x,y in [0, 6] cm, z in [-1, 1] cm (z faces reflective in both arms, the
model is z-invariant so reflective = periodic there). A z-axis cylinder rod,
r = 0.8 cm, material `fuel2`, in material `mod` filling the rest of the box.
One group:

| material | total | absorption | fission | nu-fission | chi | scatter |
|---|---|---|---|---|---|---|
| `fuel2` | 2.0 | 0.60 | 0.40 | 1.00 | 1 | 1.40 |
| `mod` | 0.5 | 0.01 | 0 | 0 | 0 | 0.49 |

(Equal total cross sections would make the flux flat and every arrangement
give the same k; the contrast is what makes the lattice arrangement matter.)

* **B1 asymmetric:** rod centre (1.2, 1.7). Not at the box centre, so the
  reflective box (mirror images, alternating gaps) and the periodic box
  (uniform pitch 6 cm) are different infinite lattices.
* **B0 control:** rod centre (3.0, 3.0). Symmetric under every mirror of the
  box, so periodic and reflective MUST agree. This is the arm that shows the
  comparison can come out null.

For each of B1, B0: x and y faces (a) reflective, (b) periodic pairs
(x=0 <-> x=6, y=0 <-> y=6; OpenMC `periodic_surface_id`; here
`BoundaryType::PeriodicWith`). Quantities: k(reflective), k(periodic),
d = k(periodic) - k(reflective).

## Predictions (written before the first run)

**Case A.** White lowers k relative to reflective at every T (cosine law peaks
at normal incidence, more leakage out of the vacuum face; see the sign
reversal recorded in the test header). Expected ensemble `d`:
T = 0.5 about -0.0219, T = 1.0 about -0.0117 (OpenMC's single-run values; this
port's single run gave -0.0134 at T = 1.0), T = 3.0 about -0.001 (small,
possibly not resolved). Expected agreement with OpenMC: all nine comparisons
within |z| <= 2 on 16 seeds. Honest risk: the earlier single-run T = 1.0
white miss (0.0015 absolute) is several ensemble sem if it were a real bias,
so if it is real the ensemble will expose it; I put roughly 25 % on at least
one Case A comparison missing the gate.

**Case B1.** `k(periodic) > k(reflective)`. Reasoning: reflection puts the
mirror-image rods at a 2.4 cm centre spacing in x (rod at x = 1.2) so they
shield each other, a fuel-clustering effect that lowers the fuel share of
absorption; the periodic lattice has uniform 6 cm pitch. Magnitude: relative
difference between 0.5 % and 5 % (low confidence in magnitude, moderate in
sign). Both codes show the same sign; |z| <= 2 on k(reflective), k(periodic)
and d for B1. If this port's periodic crossing were secretly a reflection, d
would be 0 in this port and non-zero in OpenMC, and the gate would fail.

**Case B0 (control).** |d| < 2 sem in both codes (periodic equals reflective
for a centred rod), and |z| <= 2 against OpenMC on both k's.

## Audit of existing results (step 4 of #259), plan

Search every V&V record, example, test and crate source tree in the workspace
for `BoundaryType::White`, `BoundaryType::Periodic*`, `boundary_type='white'`
/ `'periodic'`, and the word `white` as a boundary. Classify each hit by
whether a recorded number could have depended on the old silent specular
alias. Prediction: **no recorded number depends on it** (Periodic panicked
since 2026-09-22 and the 2026-09-22 change made White diffuse; the one earlier
white user is the lump scan's own Wigner-Seitz white which never used the
kernel). Results of the audit are appended below once done.

---

# RESULTS (appended 2026-11-18; the sections above are the unedited prediction stage)

## Headline

* **White boundary: reproduces OpenMC.** All nine white-related comparisons
  (k_refl, k_white, d) at T = 0.5, 1.0, 3.0 are within |z| <= 2 on 16 seeds.
  The effect itself is resolved in both codes at T = 0.5 and 1.0 (d/sem -63
  and -43 here, -192 and -75 in OpenMC).
* **Periodic boundary: reproduces OpenMC's periodic k and the sign and size
  of the periodic-vs-reflective effect, but NOT the reflective k of the
  asymmetric lattice.** B1 periodic agrees (z +0.34 pooled over 32 seeds). The
  control B0 agrees on both arms and is null in both codes. **B1 reflective is
  a MISS: z = -2.18 on seeds 1-16 and -3.11 pooled over seeds 1-32 (this port
  1.38024 +/- 0.00013, OpenMC 1.38071 +/- 0.00007, about -47 pcm). The
  replication made it worse, not better, so it is not a fluctuation.** Hence
  d(B1) is z +2.42 (16 seeds) and +2.23 (32 seeds): the periodic effect has
  the right sign and size (-0.0212 here, -0.0218 OpenMC) but is 2.5 % smaller
  in this port, entirely because of the reflective arm.
* **Not explained.** I did not investigate the cause. The offset is 0.034 % in
  k on a reflective box whose rod sits 0.4 cm from the x = 0 wall, while the
  centred-rod reflective control (B0) is +1.23 sigma the other way.
  Hypotheses only, none tested: reflective wall or corner handling near the
  off-centre rod (`compose_corner_reflection`); the inactive count (30 here
  against 20 in OpenMC); inter-generation correlation not captured by the seed
  sem. First steps would be a B1 reflective run of this port at 20 inactive,
  and moving the rod in steps toward the centre in both codes.
  **The gate in `tests/bc_vs_openmc_ensembles.rs` therefore fails on seeds
  1-16 (it asserts the pre-declared |z| <= 2); it is left failing, not ignored
  and not loosened.** Maintainer decision needed.

## Seeds 1-16 (the gate), mean +/- sem over seeds, z = (ours - OpenMC)/sqrt(sa^2+sb^2)

| case | bc | OpenMC | this port | z |
|---|---|---|---|---|
| A0.5 | reflective | 0.54607 +/- 0.00008 | 0.54618 +/- 0.00025 | +0.40 |
| A0.5 | white | 0.52471 +/- 0.00007 | 0.52491 +/- 0.00023 | +0.84 |
| A1.0 | reflective | 0.86638 +/- 0.00010 | 0.86673 +/- 0.00015 | +1.95 |
| A1.0 | white | 0.85391 +/- 0.00013 | 0.85412 +/- 0.00025 | +0.74 |
| A3.0 | reflective | 1.39565 +/- 0.00016 | 1.39537 +/- 0.00019 | -1.16 |
| A3.0 | white | 1.39452 +/- 0.00010 | 1.39432 +/- 0.00028 | -0.70 |
| B0 | reflective | 1.35890 +/- 0.00010 | 1.35915 +/- 0.00018 | +1.23 |
| B0 | periodic | 1.35897 +/- 0.00011 | 1.35911 +/- 0.00027 | +0.46 |
| B1 | reflective | 1.38065 +/- 0.00011 | 1.38009 +/- 0.00023 | **-2.18 MISS** |
| B1 | periodic | 1.35898 +/- 0.00010 | 1.35923 +/- 0.00018 | +1.16 |

Boundary-condition difference d (other minus reflective):

| case | OpenMC d | this port d | z | d/sem (OpenMC, ours) |
|---|---|---|---|---|
| A0.5 white | -0.02136 +/- 0.00011 | -0.02126 +/- 0.00034 | +0.28 | -192, -63 |
| A1.0 white | -0.01247 +/- 0.00017 | -0.01261 +/- 0.00030 | -0.41 | -75, -43 |
| A3.0 white | -0.00113 +/- 0.00019 | -0.00105 +/- 0.00033 | +0.21 | -5.9, -3.2 |
| B1 periodic | -0.02166 +/- 0.00015 | -0.02086 +/- 0.00029 | **+2.42 MISS** | -144, -71 |
| B0 periodic | +0.00007 +/- 0.00015 | -0.00004 +/- 0.00033 | -0.31 | +0.4, -0.1 |

A1.0 reflective at +1.95 is inside the gate but borderline and is flagged.

## Replication, seeds 17-32 (planned before the first run), B1 only, pooled N = 32

Only the comparisons that missed in the first block were replicated, as planned.

| case | bc | OpenMC N=32 | this port N=32 | z |
|---|---|---|---|---|
| B1 | reflective | 1.38071 +/- 0.00007 | 1.38024 +/- 0.00013 | **-3.11 MISS** |
| B1 | periodic | 1.35895 +/- 0.00007 | 1.35901 +/- 0.00017 | +0.34 |
| B1 | d(periodic - refl) | -0.02177 +/- 0.00010 | -0.02123 +/- 0.00022 | **+2.23 MISS** |

Raw per-seed values: `openmc_results/*.json`, `ours_results/*.json`;
`analysis.txt` is the output of `openmc_inputs/analyze.py`.

## Predictions vs outcome

* **Case A sign and size:** held (white below reflective at every T; OpenMC d
  -0.0214, -0.0125, -0.0011). The prediction "all nine within |z| <= 2, about
  25 % chance of a miss" held for Case A. The T = 1.0 single-run miss of
  2026-09-22 does not reproduce on 16 seeds (white z +0.74, d z -0.41): it was
  a single-run fluctuation.
* **B1 sign: PREDICTION WRONG.** I predicted k(periodic) > k(reflective) on a
  shielding argument (mirror-image rods 2.4 cm apart shield each other).
  Both codes show the opposite: periodic is about 1.6 % LOWER. The argument
  was backwards; why is not established here. The magnitude (0.5 to 5 %
  relative) held.
* **B0 control:** held in both codes (d consistent with 0).
* **Gate |z| <= 2 on every comparison: NOT met** (B1 reflective, and with it
  d(B1)). The gate was not moved.

## Audit of existing results (#259 step 4)

Methodology: `git grep` at `e14d4d90a^` (the parent of the 2026-09-22 change
that made White its own condition) for `BoundaryType::White|Periodic` and bare
`White|Periodic` in every `.rs` outside `src/geometry`; `git log -S` for when
each current user appeared; a working-tree search of `.rs`, `.py`, `.md`,
`.xml`, `.toml`, `.json` for `boundary_type='white'|'periodic'`.

* Before `e14d4d90a`, no code outside `outram-mc-libs/src/geometry` used
  `BoundaryType::White` or `Periodic`. The only other `White` is
  `CellBoundary::White` (the Wigner-Seitz boundary of
  `examples/lump_self_shielding_scan.rs`, `physics/slowing_down.rs`, the
  `lump_collision_probability` and `ring_rpt_hunt_lessons` tests), a separate
  implementation that never went through the kernel's boundary arm.
* The two current kernel users of `BoundaryType::White`,
  `examples/lumped_ugraphite_kinf.rs` and
  `dhoby-ghaut/examples/monte_carlo_web/lumped/model.rs`, appeared on
  2026-10-04 and later (`git log -S`: `43832d173`, `148be19d3`), after the
  diffuse fix, so their recorded numbers already used the real white boundary.
* `tutorial_rung3/openmc_inputs/lumped_openmc.py` uses
  `boundary_type="white"` on the OpenMC side only.
  `reference-data/virtual_test_bed/htgr/assembly/assembly.py` is an upstream
  OpenMC deck with a periodic hexagonal prism, not run by this workspace.
* **Result: no recorded V&V number rests on the old specular alias, so
  nothing needed re-measuring.** Not checked: V&V prose that describes a
  white boundary without any of the searched spellings.

## Provenance

* OpenMC 0.15.3, commit `27e38e894697bb32a1dac7848d2618818b6b8daf`, built from
  source, no MPI; multi-group mode, library `mgxs.h5` written by
  `openmc_inputs/mk.py` (one group, numbers in the tables above); no nuclear
  data.
* 20 000 particles per batch, 20 inactive + 100 active batches, seeds 1-16
  (and 17-32 for the B1 replication). This port: 20 000 particles, 30
  inactive + 100 active generations, the same seed integers. Shannon entropy
  not recorded.
* Uncertainty: every k is the mean over seeds of per-run active means, +/- the
  standard error over seeds.
* This port: `run_keff_mg`, release, two worker threads.
* **Hardware (all timings):** Intel Xeon Processor @ 2.10 GHz, 4 logical
  cores, 15 GB RAM, Linux 6.18, CPU only, shared with two other agents' jobs.
  OpenMC: one thread per process, two processes at a time; 16 seeds took 85 to
  190 s per Case A configuration and 366 to 488 s per Case B configuration.
  This port: blocks of 4 seeds x 10 configurations (40 runs) took 300 to 410 s
  with two worker threads. Not comparable across codes (different thread
  counts, shared machine).
* Geometry pictures (ported plotter, `render_material_slice`, from the
  assembled `Geometry`): `slab_T1.0_xy.png` (slab x in [0,1], y in [-1,1]),
  `B1_periodic_xy.png` (6 x 6 box, rod at (1.2, 1.7), r 0.8, checked against
  the axes), `B0_periodic_xy.png` (rod at (3, 3), not inspected by eye). The
  plotter does not draw boundary conditions; the periodic pairing is verified
  by the track tests.
* Scripts, verbatim: `openmc_inputs/mk.py`, `ensemble.py`, `analyze.py`.
  Rust: `tests/bc_vs_openmc_ensembles.rs`
  (`OUTRAM_BC259_FIRST/SEEDS/OUT/ONLY`).

## The OpenMC driver (`openmc_inputs/ensemble.py`, inline)

```python
"""OpenMC driver for white_periodic_vs_openmc_259.

usage: ensemble.py <case> <bc> <nseeds> <first_seed> <workdir>
  case: A0.5 | A1.0 | A3.0   slab of thickness T; bc reflective | white
        B1 | B0              rod lattice (B1 off-centre, B0 centred); bc reflective | periodic
<workdir> must contain mgxs.h5 (made by mk.py). Writes
<workdir>/<case>_<bc>_s<first_seed>.json with the per-seed keff (mean over
the active batches of one run)."""
import sys, json, os, time
import numpy as np
import openmc

case, bc, nseeds, first, workdir = (sys.argv[1], sys.argv[2], int(sys.argv[3]),
                                    int(sys.argv[4]), os.path.abspath(sys.argv[5]))
PARTICLES, INACTIVE, ACTIVE = 20000, 20, 100
MGXS = os.path.join(workdir, 'mgxs.h5')


def build(case, bc):
    if case.startswith('A'):
        T = float(case[1:])
        m = openmc.Material(name='fuel')
        m.set_density('macro', 1.0)
        m.add_macroscopic('fuel')
        left = openmc.XPlane(0.0, boundary_type=bc)
        right = openmc.XPlane(T, boundary_type='vacuum')
        ym = openmc.YPlane(-1.0, boundary_type='reflective')
        yp = openmc.YPlane(1.0, boundary_type='reflective')
        zm = openmc.ZPlane(-1.0, boundary_type='reflective')
        zp = openmc.ZPlane(1.0, boundary_type='reflective')
        cell = openmc.Cell(fill=m, region=+left & -right & +ym & -yp & +zm & -zp)
        src = openmc.stats.Box((0.0, -1.0, -1.0), (T, 1.0, 1.0))
        return [m], openmc.Geometry([cell]), src
    cx, cy = {'B1': (1.2, 1.7), 'B0': (3.0, 3.0)}[case]
    fuel = openmc.Material(name='fuel2')
    fuel.set_density('macro', 1.0)
    fuel.add_macroscopic('fuel2')
    mod = openmc.Material(name='mod')
    mod.set_density('macro', 1.0)
    mod.add_macroscopic('mod')
    t = 'periodic' if bc == 'periodic' else 'reflective'
    xm = openmc.XPlane(0.0, boundary_type=t)
    xp = openmc.XPlane(6.0, boundary_type=t)
    ym = openmc.YPlane(0.0, boundary_type=t)
    yp = openmc.YPlane(6.0, boundary_type=t)
    if bc == 'periodic':
        xm.periodic_surface = xp
        xp.periodic_surface = xm
        ym.periodic_surface = yp
        yp.periodic_surface = ym
    zm = openmc.ZPlane(-1.0, boundary_type='reflective')
    zp = openmc.ZPlane(1.0, boundary_type='reflective')
    rod = openmc.ZCylinder(x0=cx, y0=cy, r=0.8)
    box = +xm & -xp & +ym & -yp & +zm & -zp
    c_f = openmc.Cell(fill=fuel, region=box & -rod)
    c_m = openmc.Cell(fill=mod, region=box & +rod)
    src = openmc.stats.Box((cx - 0.8, cy - 0.8, -1.0), (cx + 0.8, cy + 0.8, 1.0))
    return [fuel, mod], openmc.Geometry([c_f, c_m]), src


ks, t0 = [], time.time()
for seed in range(first, first + nseeds):
    wd = os.path.join(workdir, f'run_{case}_{bc}_{seed}')
    os.makedirs(wd, exist_ok=True)
    os.chdir(wd)
    mats, geom, src = build(case, bc)
    ms = openmc.Materials(mats)
    ms.cross_sections = MGXS
    ms.export_to_xml()
    geom.export_to_xml()
    s = openmc.Settings()
    s.energy_mode = 'multi-group'
    s.batches = INACTIVE + ACTIVE
    s.inactive = INACTIVE
    s.particles = PARTICLES
    s.seed = seed
    s.source = openmc.IndependentSource(space=src, constraints={'fissionable': True})
    s.output = {'tallies': False, 'summary': False}
    s.export_to_xml()
    openmc.run(output=False)
    sp = openmc.StatePoint(f'statepoint.{s.batches}.h5')
    ks.append(float(sp.keff.nominal_value))
    sp.close()
json.dump({'case': case, 'bc': bc, 'seeds': list(range(first, first + nseeds)), 'k': ks,
           'particles': PARTICLES, 'inactive': INACTIVE, 'active': ACTIVE,
           'wall_s': time.time() - t0},
          open(os.path.join(workdir, f'{case}_{bc}_s{first}.json'), 'w'))
print(case, bc, 'mean %.6f sem %.6f wall %.1fs' % (np.mean(ks),
      np.std(ks, ddof=1) / np.sqrt(len(ks)), time.time() - t0))
```

---

# B1 reflective discrepancy: diagnosis (GitHub #259)

Status of this section: **HYPOTHESIS STAGE, written and committed before any
diagnostic run** (2026-11-18). Results are appended below in a later commit;
the text down to "Experiments" is not edited afterwards.

## Upstream read first (no run yet)

* OpenMC `ReflectiveBC::handle_particle` (`src/boundary_condition.cpp`):
  `surf.reflect(r, u)` about the ONE struck surface, renormalise, then
  `Particle::cross_reflective_bc` (`src/particle.cpp:657`): set `u`, put the
  particle back in `cell_last`, flip the surface token, and re-find the cell
  from the root (`neighbor_list_find_cell`). No nudge, no corner composition:
  a particle exactly on two walls reflects off the first and meets the second
  at distance 0 on its next move.
* This port (`geometry/crossing/mod.rs`, `Geometry::cross_surface`,
  `Reflective` arm): `compose_corner_reflection` reflects off the struck wall
  and every other reflective wall within 1e-9 cm of the crossing point,
  `nudge_across` moves the particle 1e-9 cm inside, and the driver
  (`physics_mg.rs::transport_history`) re-locates with the outgoing surface
  token. For a lone wall this is the same mirror as upstream; the two differ
  only at a corner/edge (composed vs sequential, which is the same retro-
  reflection for orthogonal planes) and by the 1e-9 cm nudge.
* What upstream does that the port does not: nothing that bears on this box.
  Corner events need a crossing within 1e-9 cm of two walls, probability of
  order 1e-9 per boundary event; the rod (r 0.8 at (1.2, 1.7)) is 0.4 cm from
  x = 0 and 0.9 cm from y = 0 and never coincident with a wall. The reflective
  arm is also the same code in B0 (centred rod), which agrees.
* Reviewed-function check: `src/geometry/crossing/review.md` holds no
  `kind = "review"` block (it is the empty kovan note), so nothing in
  `crossing/mod.rs` is stamped.

## Hypotheses, with predicted sign and size (before measuring)

The discrepancy to explain: port - OpenMC = 1.38024 - 1.38071 = **-47 pcm**
(-4.7e-4), z -3.11 on 32 seeds; periodic B1 and both B0 arms agree.

* **H1, the reflective crossing is defective (a missing limit or nudge
  effect).** Predicted effect: k of the port is LOW by about 47 pcm relative
  to the true reflective lattice. Discriminator E1 below. My prior is low
  (~20 %): the code reading above finds no mechanism, and corner events are
  of order 1e-9 per crossing.
* **H2, the 30 vs 20 inactive batches.** A source confined to one rod in a
  6 cm box relaxes in a few generations; predicted effect of 20 vs 30 inactive
  in the port: |dk| < 5e-5 (inside the seed sem), sign indeterminate. Prior
  ~5 %. Discriminator E2.
* **H3, finite-population (batch-size) bias that differs between the codes.**
  This port's MG driver redraws the next source WITH replacement
  (`physics_mg.rs::resample`, its own doc says "crude"); OpenMC carries each
  site once (sampling without replacement, no duplicates). Duplicated sites
  add source variance, and the known k bias from a fluctuating source scales
  as 1/N. Predicted: the port is biased LOW, the gap scales as 1/N, so at
  N = 80 000 the gap shrinks from -47 pcm to about -12 pcm (a factor 4).
  Why B1 reflective and not the others: OpenMC's reflective sem is smallest
  there; the other cases may carry the same bias at smaller, unresolved size
  (B0 reflective is +1.23 sigma the other way, which this hypothesis does not
  explain). Prior ~35 %. Discriminator E3.
* **H4, a fluctuation that survived 32 seeds (3.1 sigma, p about 0.002
  two-sided, but 10 comparisons were looked at, so p about 0.02).** Prior
  ~40 %. Discriminator: a disjoint block of seeds 33-64 for B1 reflective in
  both codes; predicted gap there 0 +/- 25 pcm.

## Experiments (cheapest discriminators first)

* **E1, unfold the reflective box by the method of images.** The reflective
  6x6 box with a rod at (1.2, 1.7) is EXACTLY the periodic 12x12 box with four
  rods at (1.2, 1.7), (10.8, 1.7), (1.2, 10.3), (10.8, 10.3) (mirror images
  through x = 0 and y = 0, wrapped by the pitch 12). Run that "B1U" in this
  port using only the periodic path (which agrees with OpenMC) and in OpenMC.
  If H1: port B1U = OpenMC value (1.3807), i.e. +47 pcm above port reflective.
  If not H1: port B1U = port reflective (1.3802): the reflective crossing is
  consistent with the periodic one and the gap is not in the boundary code.
  Source: the four rods, so the same particles per rod ratio as before is not
  kept; 20 000 particles per batch as everywhere.
* **E2** port B1 reflective with 20 inactive (new env knob).
* **E3** port and OpenMC B1 reflective at N = 80 000 particles per batch.
* **E4** seeds 33-64 for B1 reflective in both codes.

Seed ensembles only; z = (ours - OpenMC) / sqrt(sem_a^2 + sem_b^2) over the
per-seed run means.

## Experiments E1, E3, E4 as run, and prediction for E5 (written before E5)

Interim measurements (hardware: Intel Xeon @ 2.10 GHz, 4 logical cores shared
with other jobs; port 2 worker threads, OpenMC 1 thread per process):

* **E1 (unfolded 12 x 12 periodic, 4 rods; 50 inactive both codes, seeds 1-16):**
  port 1.38015 +/- 0.00022, OpenMC 1.38078 +/- 0.00007, z -2.67. OpenMC's
  unfolded value equals its own reflective B1 (1.38065 / 1.38071), as the
  images argument requires. The port's unfolded value equals ITS reflective
  B1 (1.38024). So the port is low on this lattice **through its periodic
  path too**: H1 (reflective crossing defect) is not supported.
* **E4 (OpenMC B1 reflective, seeds 33-64):** 1.38087 +/- 0.00008. The OpenMC
  value is stable at 1.3807-1.3809 over 64 seeds, H4 (fluctuation) is not
  supported.
* **E3 (port N = 80 000, 12 seeds, 30 + 100):** B1 reflective
  1.38049 +/- 0.00014 (was 1.38024 +/- 0.00013 at N = 20 000), B1 periodic
  1.35901 +/- 0.00010 (unchanged). OpenMC at N = 80 000 pending.

**E5 prediction (before running):** replace the MG driver's with-replacement
resampling by `comb_resample` (the CE drivers' routine since #460). If H3 is
the whole cause, port B1 reflective at N = 20 000, 16 seeds, 30 + 100 rises
by +35 to +47 pcm to 1.3806-1.3807 (z against OpenMC 1.38071 within 2), and
B1U and B0 move by no more than their sem. If it moves by less than 15 pcm,
H3 is rejected as the main cause.

## Diagnosis: results (appended 2026-11-18; hypotheses above unedited)

Hardware for all runs: Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB RAM,
Linux 6.18, CPU only, shared with other agents' jobs. Port: 2 worker threads
per test process. OpenMC 0.15.3 (`27e38e8`): 1 thread per process. Wall times:
OpenMC B1 reflective 32 seeds 708 s; OpenMC N = 80 000, 12 seeds 1054 s; port
full gate (160 runs) 802 s; port N = 80 000 (24 runs) 2124 s.

### Verdict

**The cause is not the reflective boundary. The best-supported cause is the
multigroup driver's population control**: `physics_mg.rs::resample` redrew the
next source WITH replacement, the leftover of the pre-#460 sampler that the CE
drivers had already replaced by `fission::comb_resample`. Switching the MG
driver to the comb (`physics_mg.rs`, this change) brings B1 reflective, the
unfolded B1U and every other #259 case inside the pre-declared gate. The
attribution is not a large direct effect: the old-vs-new change in B1
reflective is +41 +/- 21 pcm (1.9 sigma, pooled 32 seeds). What carries the
weight is that the comb value agrees with OpenMC (z -0.34, was -3.11) and so
does B1U (z +0.11, was -2.67), with a prediction made in advance (below).

### What was ruled out

* **H1 (reflective crossing defect): NOT supported (E1).** The method-of-images
  unfolding of B1 (periodic 12 x 12, four rods; periodic path only, no
  reflective x/y wall) is low in the old port too: 1.38015 +/- 0.00022 against
  OpenMC 1.38078 +/- 0.00007 (z -2.67, 16 seeds, 50 inactive in both), the same
  gap as the reflective box. OpenMC's reflective and unfolded values agree with
  each other (1.38071 / 1.38078), so the mirror argument holds. The upstream
  read (above) found no missing limit. Not measured: a direct count of corner
  hits.
* **H4 (fluctuation): NOT supported (E4).** OpenMC B1 reflective on seeds
  33-64: 1.38087 +/- 0.00008; OpenMC is stable at 1.3807-1.3809 over 64 seeds.
* **H2 (30 vs 20 inactive): not run separately.** B1U used 50 inactive in both
  codes and the gap was the same as at 30 vs 20; treated as unsupported, not
  as measured.

### H3 as measured, with the prediction scorecard

* E5 prediction (committed before the run): the comb raises port B1
  reflective by +35 to +47 pcm; B1U and B0 move by no more than their sem.
  Measured, pooled 32 seeds: **+41 +/- 21 pcm** (held, at 1.9 sigma). B1U:
  1.38080 +/- 0.00017 with the comb against 1.38015 old (+65 pcm). B0 moved
  +0.0002 (reflective) and -0.0005 (periodic), inside 2 sem.
* **H3's 1/N prediction (E3) was NOT supported at the available precision.**
  I predicted the gap to shrink from -47 to about -12 pcm at N = 80 000. Old
  code at 80 000 (12 seeds): 1.38049 +/- 0.00014; OpenMC at 80 000 (12 seeds):
  1.38085 +/- 0.00008; gap -36 pcm, z -2.2. The port moved only +25 +/- 19 pcm
  for 4x the particles. Consistent with a bias falling with N, but not showing
  1/N; not measured further. Why the effect is large for B1/B1U and unresolved
  for the other cases is NOT explained here.

### New values (comb resampling), seeds 1-16, mean +/- sem over seeds

| case | bc | OpenMC | port old | port new | z old | z new |
|---|---|---|---|---|---|---|
| A0.5 | reflective | 0.54607 +/- 0.00008 | 0.54618 | 0.54611 +/- 0.00020 | +0.40 | +0.15 |
| A0.5 | white | 0.52471 +/- 0.00007 | 0.52491 | 0.52493 +/- 0.00021 | +0.84 | +1.00 |
| A1.0 | reflective | 0.86638 +/- 0.00010 | 0.86673 | 0.86624 +/- 0.00026 | +1.95 | -0.51 |
| A1.0 | white | 0.85391 +/- 0.00013 | 0.85412 | 0.85408 +/- 0.00025 | +0.74 | +0.63 |
| A3.0 | reflective | 1.39565 +/- 0.00016 | 1.39537 | 1.39575 +/- 0.00029 | -1.16 | +0.31 |
| A3.0 | white | 1.39452 +/- 0.00010 | 1.39432 | 1.39413 +/- 0.00027 | -0.70 | -1.36 |
| B1 | reflective | 1.38065 +/- 0.00011 | 1.38009 | 1.38076 +/- 0.00024 | -2.18 | +0.42 |
| B1 | periodic | 1.35898 +/- 0.00010 | 1.35923 | 1.35911 +/- 0.00021 | +1.16 | +0.51 |
| B0 | reflective | 1.35890 +/- 0.00010 | 1.35915 | 1.35913 +/- 0.00027 | +1.23 | +0.78 |
| B0 | periodic | 1.35897 +/- 0.00011 | 1.35911 | 1.35858 +/- 0.00025 | +0.46 | -1.40 |

Differences d (other - reflective), new: A0.5 white z +0.61, A1.0 white
+0.80, A3.0 white -1.13, B1 periodic **+0.03** (was +2.42), B0 periodic -1.53
(was -0.31). **Worst |z| 1.53: `white_and_periodic_ensembles_match_openmc`
passes** (it failed before on B1 reflective and d(B1)). The port's sems did not
shrink (0.0002-0.0003 here); the new B0 periodic and d(B0) are now the largest
deviations, inside the gate, not explained.

Replication, comb, B1 only, seeds 1-32: reflective 1.38065 +/- 0.00017
(OpenMC 1.38071 +/- 0.00007, z -0.34; old 1.38024 +/- 0.00013, z -3.11),
periodic 1.35920 +/- 0.00016 (OpenMC 1.35895 +/- 0.00007, z +1.4).

### Consequences and what was not re-measured

* The change moves the random stream of **every** `run_keff_mg` user. In this
  crate `tests/stats_move_fingerprints.rs::two_group_mg_sphere_is_bit_identical`
  was re-pinned (k 0.66115 -> 0.65646, -0.7 sigma of that run's own k_std).
  Other crates calling `run_keff_mg` (`nee_soon`, `dhoby-ghaut`) were NOT
  re-run; any recorded MG number of theirs is a draw from the old stream.
* `tests/white_boundary_vs_openmc.rs::white_and_reflective_differ_as_openmc_
  says_they_do` now FAILS on its single-seed draw at T = 3.0: d = +0.00056
  (OpenMC -0.00116; single-run sigma of d about 0.00085, so a 2.0 sigma draw).
  That gate asserts the SIGN of a 1.4 sigma effect on one seed; it passed on
  the old stream by the draw, not by resolving the effect (the 16-seed
  ensemble above gives d(T=3) = -0.00162 +/- 0.00040). Seed and assertion
  left untouched: maintainer's call (suggestion: assert the sign for T <= 1.0
  only and leave T = 3 to the ensemble gate). Its other two tests pass.
