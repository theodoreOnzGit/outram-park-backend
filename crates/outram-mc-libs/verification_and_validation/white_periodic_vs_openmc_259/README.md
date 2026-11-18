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
