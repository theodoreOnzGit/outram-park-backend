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
