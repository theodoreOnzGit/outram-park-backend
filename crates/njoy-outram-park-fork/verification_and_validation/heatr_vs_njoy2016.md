# HEATR KERMA and damage energy vs NJOY2016 — Fe-58 and Si-28

**Date:** 2026-09-17
**Module:** `src/heatr/` (KERMA MT=301, damage energy MT=444)
**Test:** `tests/heatr_vs_njoy2016.rs` (3 tests, 2 nuclides)
**Status:** verification (cross-code). **No human V&V.**

## Why this exists

`tests/heatr_kerma_vs_kinematics.rs` (added the same week) checks HEATR's
heating models against the closed forms they are *defined* by. That is the right
first check, and it says nothing about whether the port agrees with NJOY. The
damage half — `src/heatr/damage.rs`, 229 lines carrying the Lindhard-Robinson
partition, the displacement-threshold table and the two-body recoil kinematics —
had **no test of any kind**.

## Methodology

### Reference

NJOY2016 at commit `ac5adf5f`, built from source and executed. Deck, per nuclide:

```
moder
20 -21 /
reconr
-21 -22 /  'pendf' /  <mat> 0 /  0.001 /  0 /
heatr
-21 -22 -23 0 /
<mat> 4 0 0 1 1 /          ! npk=4, nqa=0, ntemp=all, local=1, iprint=1
443 444 445 446 /
moder
-23 24 /
stop
```

on the committed ENDF/B-VIII.0 evaluations in `reference-data/endf/`, trimmed to
MF=1 plus MF=3/MT=301, 443, 444, 445, 446 and committed under
`reference-data/heatr/` with the deck beside each tape.

**`local = 1` is load-bearing.** It deposits photon energy locally rather than
transporting it, which is the regime this crate's `Kerma` models. MT=445/446 are
unaffected (verified byte-identical against a `local = 0` run); MT=444 is,
through its MT=447 term.

### MT=443 is deliberately not compared

Its name — "total kinematic kerma (high limit)" — invites the reading that it is
what `Kerma` computes. It is not. For capture, `heatr.f90:5282-5294` accumulates

```
hk = E_gamma*sigma*y - sigma*rtm*(Q + E*A/(A+1))^2/2
```

a photon-momentum recoil diagnostic. At 1e-5 eV on Fe-58 it reads 1.8e4 eV·b
against this crate's 4.4e8 — a factor of 24 000. Comparing the two would be a
units-grade error dressed up as a physics finding, and it was nearly made here:
the first version of this comparison did exactly that before `heatr.f90` was
read.

## Results

### 1. KERMA in the capture-dominated region — exact, and the residual is explained

Below the first inelastic threshold only elastic and capture are open, and
capture dominates. There this crate computes `H = sigma_cap*(E + Q)`, the
kinematic limit; NJOY with `local = 1` deposits the **evaluation's** photon
energy plus recoil. So

```
ours(E) - njoy(E) = sigma_cap(E) * deficit,   deficit = Q - sum(E_gamma)
```

with `deficit` a property of the evaluation and **independent of E**. That
constancy is the real check: a wrong Q, a wrong cross-section coupling or a
mis-indexed grid would each break it.

Measured over five probes spanning eight decades (1e-5 to 1e4 eV):

| nuclide | implied deficit per capture | ours / njoy |
|---|---|---|
| Si-28 | **0** (8.473899e6 implied vs 8.473900e6 `QI`) | 1.000000 – 1.002507 |
| Fe-58 | **204.96 – 209.11 keV** of a 6.58043 MeV `Q` (3.18 %) | 1.031794 – 1.032821 |

Si-28's evaluation balances its capture Q exactly, and the two codes agree to
**1e-6 relative at 1e-5 eV**. Fe-58's photon data falls ~208 keV short, and the
resulting +3.27 % is constant to 4 significant figures across eight decades.
That is not a defect in either code: it is the evaluation's own energy-balance
deficiency, and exposing it is what HEATR's energy-balance method exists for.

### 2. Damage energy where scattering is isotropic — agrees, with a small honest bias

`DamageEnergy` uses an **isotropic centre-of-mass** recoil distribution; NJOY
weights by the evaluation's MF=4 (`heatr.f90`'s 64-point Gauss-Legendre
`disbar`). Below ~10 keV elastic scattering off a medium nucleus is very nearly
isotropic in the CM, so the two must agree — and agreement tests the Lindhard
partition, the recoil bounds, the uniform-recoil average, the `E_d` table and
the cross-section coupling all at once.

Against **MT=445** (elastic damage alone), 121 log-spaced points, 1.2–10 keV:

| nuclide | `E_d` | mean | worst | sign split |
|---|---|---|---|---|
| Fe-58 | 40 eV | **+0.16 %** | +0.93 % at 1.43 keV | 98 above / 23 below |
| Si-28 | 25 eV | **+0.31 %** | +0.72 % at 10 keV | 117 above / 4 below |

The assertion is on the **mean** (< 0.5 %) and on **both signs being present**,
not only on the worst point. A chord difference between two independently
converged grids scatters about zero; a wrong partition or a wrong `E_d` biases
every point the same way. The small positive bias that remains is real — it is
the onset of the anisotropy effect below — and is left visible rather than
absorbed into a tolerance.

### 3. The cost of the missing MF=4 anisotropy, measured for the first time

Forward-peaked elastic scattering lowers the mean recoil energy, so an isotropic
treatment **over**-predicts damage. Sign and direction were predicted before
measuring; both held.

On elastic alone (`ours / MT=445 - 1`):

| E | Fe-58 | Si-28 |
|---|---|---|
| 100 keV | +3.0 % | +8.8 % |
| 464 keV | +11.8 % | +3.8 % |
| worst in 0.1–0.9 MeV | **+28.5 %** (900 keV) | **+68.3 %** (580 keV) |

On the MT=444 total (`ours / njoy`):

| E | Fe-58 | Si-28 |
|---|---|---|
| 1 keV | 1.0035 | 0.9900 |
| 10 keV | 0.9980 | 1.0068 |
| 100 keV | 1.0298 | 1.0874 |
| 1 MeV | 1.1985 | 1.6098 |
| 5 MeV | **1.8069** | 1.4674 |
| 19 MeV | 0.9701 | 0.9154 |

Until now "MF=4 anisotropy is not ported" was an unquantified caveat in the
module docs. This is what it costs. Note the total crosses back through 1.0 near
19 MeV: the missing anisotropy pushes ours **up** and the missing channels
(MT=447 disappearance recoil, continuum, `(n,xn)`) push it **down**, so that is
a cancellation, not agreement, and the test says so rather than asserting it.

## An open discrepancy at the damage threshold

NJOY's MT=445 is non-zero at **562.5 eV** on Fe-58, where the two-body elastic
kinematic maximum recoil `4A/(A+1)^2 * E` is 37.8 eV — **below** the `E_d = 40 eV`
cut. The kinematic threshold implied by `E_d` is 594.5 eV; this crate's first
non-zero point is its first grid node above that, 607.7 eV.

Two candidate causes were **read and eliminated**, not assumed:

- NJOY's `df` (`heatr.f90:2015-2052`) cuts at `break = E_d` exactly as this port
  does — `else if (e.lt.break) then df=0`.
- NJOY's default `E_d` table (`heatr.f90:328-360`) gives 40 eV for Z=26, exactly
  as this port's `default_displacement_energy` does. The whole table was
  compared entry by entry and matches.

~~So the difference is in how `disbar` bounds the recoil, which has **not** been
read. Magnitude: 0.86 eV·b against a scale reaching 1e5, confined to a ~45 eV
window at threshold. Recorded as an open question rather than guessed at.~~

### Diagnosed 2026-10-05 (GitHub #535): `disbar` interpolates between nodes

`disbar` (`heatr.f90:1829-2013`) was read. It does **not** bound the recoil
differently; it does not evaluate the damage at the requested energy at all.

- It keeps two nodes, `(el, daml)` and `(en, damn)`. When a request `ee` passes
  `en`, the next node is `e = step*el` with `step = 1.1`, lowered to the next
  MF=4 energy (`enext`) if that comes first and raised to `ee` if the request
  is further still (`heatr.f90:1953-1955`).
- The 64-point Gauss-Legendre recoil integral with `df` runs **only at the
  node** (`:1992-2001`).
- The value returned for `ee` is `terp1(el,daml,en,damn,ee,dame,2)`
  (`:2010`): a straight line between nodes.

So when one node sits below the kinematic threshold (damage 0) and the next
above it, every request between them gets a positive chord value.

**Prediction, then the check on NJOY's own tape.** If this is the cause, the
MT=445 points below 594.5 eV lie on one straight line that reaches zero at a
node between 594.5/1.1 = 540.5 eV and 594.5 eV. From
`reference-data/heatr/fe58-ENDF8.0-0K-local1.heatr.pendf`:

| E (eV) | NJOY MT=445 (eV·b) | on the chord? |
|---|---:|---|
| 562.5 | 0.85851 | defines it |
| 593.75 | 2.99995 | defines it |
| zero crossing | **549.97 eV** | the node `500 × 1.1 = 550` eV |
| 605 (next node, `550 × 1.1`) | 3.771 (chord) | |
| 625 | 10.571 | chord from (605, 3.771) through 656.25 gives **10.53** |

The zero crossing lands on a 10 % step from a 500 eV node to within 0.03 eV,
and the next segment is consistent to 0.4 %. **The sub-threshold values are
NJOY's interpolation, not physics.** This port runs the recoil integral at
every requested energy, so its first non-zero point (607.7 eV, its first grid
node above 594.5 eV) is the kinematic threshold. **Not a port defect**; the
two codes differ by NJOY's chord error, which is largest where the damage
curve bends most, i.e. at threshold.

Two side notes from the same reading, neither affecting these two nuclides:

- `disbar`'s elastic shortcut skips nodes below
  `enx = edis*arat/(4*afact)` with **`edis = 25` eV hard-coded**
  (`:1891, :1938-1943`), not the material's `E_d`. For Fe-58 `enx` = 371.6 eV
  and for Si-28 186.1 eV, both at or below the real threshold, so it changes
  nothing here; for a material with `E_d < 25` eV it would zero real damage
  below `enx`.
- If byte parity with NJOY's MT=444/445 were ever wanted, the port would have
  to adopt the same 10 % node stepping. That would make it less accurate at
  threshold, so it is a maintainer decision, not something to do by default.

### 4. Photon energy production (MT=442) and the energy-balance KERMA — H6a (2026-10-05, GitHub #535)

**Methodology.** NJOY2016 (`ac5adf5f33`) HEATR re-run on the same two
evaluations with `local = 0` and `442` added to `npk`; tapes and decks are
`reference-data/heatr/{fe58,si28}-ENDF8.0-0K-local0.*`. `local = 0` is needed
because HEATR runs `gheat`, its MF=12/13 pass, only then (`heatr.f90:378`).
MT=301 and MT=443–446 are byte-identical between this run and the committed
`local = 1` run with `npk` unchanged, so adding 442 changes nothing else.

The port's MT=442 (`photon::PhotonProduction`) now follows HEATR's four
routes: MF=6 photons through `nheat` (mean energy × yield × σ), MF=12 LO=2
cascades through `hconvr` (reusing ACER's `Lo2Cascade` with HEATR's level
energies), MF=12/13 through `gheat`, and capture with MF=12 photons by energy
balance minus the photon recoil (`disgam`, `tabsqr`).

HEATR prints energies to 7 figures, coarser than its grid inside narrow
resonances, so each NJOY value is compared with the envelope of ours over its
abscissa's ±5e-7 interval (both ends, the point, and every node of our RECONR
grid inside it). Pass criterion for MT=442: every point within 1e-6 of the
envelope. Gate: `tests/heatr_mt442_vs_njoy2016.rs`,
`mt442_matches_njoy_at_print_precision`.

**Results, MT=442** (worst distance outside the envelope; median plain
`|ours/njoy − 1|` in brackets):

| nuclide | points | E < 1 eV | 1 eV – first inelastic | above |
|---|---|---|---|---|
| Fe-58 | 34 277 | 2.2e-7 (7.5e-8) | 4.6e-7 (7.8e-8) | 3.2e-7 (5.5e-8) |
| Si-28 | 9 210 | 1.7e-7 (5.7e-8) | 3.4e-7 (6.7e-8) | 4.1e-7 (7.5e-8) |

Before H6a: Fe-58's MT=442 was 0 everywhere (all its photons are MF=6 or LO=2);
Si-28's was +6.8e-4 below 1 eV, 8 % median to the first inelastic threshold,
and 0.04–1.5 % of NJOY's value above it.

**Results, MT=301** (recorded, not asserted; median `|ours/njoy − 1|` above the
first inelastic threshold):

| comparison | `QI` only | `nheat` Q rule (`Kerma::from_endf`) |
|---|---|---|
| Fe-58, `local = 1` | 0.76 | **0.11** |
| Si-28, `local = 1` | 0.38 | **0.31** |
| Fe-58, `local = 0` | 1.00 (clamped to 0) | 1.35 |
| Si-28, `local = 0` | 1.00 (clamped to 0) | 0.99 |

**Interpretation.**

- **Most of the `−75 %` in the caveat below was the deposited Q.** `nheat`
  deposits `E + q0 − Ē_n` with `q0 = 0` for a discrete level (`:1180`), i.e.
  recoil plus excitation; the port deposited `QI`, the recoil alone.
  `Kerma::from_endf` applies `nheat`'s whole `q0` table, and is now what the
  ACE route uses.
- **The `local = 0` residual is the `local = 1` residual, amplified.** NJOY's
  values satisfy `local0 = local1 − MT442` exactly, and so do ours. Fe-58 at
  2 MeV is 10 % high at `local = 1` and 1.9× at `local = 0`, because MT=442
  removes 89 % of the total there. The cause is the neutron side's kinematic
  estimate (isotropic two-body, H5 spectra), which H6b replaces with
  `nheat`'s own `disbar`/`conbar`/`sixbar` means. (Section 5 shows this
  was the neutron side, and that most of it was two defects rather than
  the kinematic estimate as such.)
- **Fe-58 below 1 eV at `local = 0` is 756× NJOY.** Its capture photons are
  MF=6. For that case NJOY deposits only the photon recoil (`sixbar`'s
  `tabsq6`) and no energy balance; the port deposits the 208 keV by which the
  photon lines fall short of `Q` (section 1). H6c.
- Si-28 below 1 eV agrees to 6e-8 at `local = 0`: capture with MF=12 photons,
  energy balance and recoil, as NJOY.

### 5. The two-body neutron side — H6b part 1 (2026-10-05, GitHub #535)

**Methodology.** Same oracles as section 4 (NJOY2016 `ac5adf5f33`, Fe-58 and
Si-28, ENDF/B-VIII.0, 0 K, `local = 1` and `local = 0`), same envelope
measure. `Kerma::from_endf` was changed to follow `nheat`
(`heatr.f90:1050-1460`) for the channels with a two-body outgoing neutron,
read line by line from upstream before any number was compared:

- **`disbar`** (`:1829-2013`, ported as `heatr/twobody.rs`) for elastic and
  for the discrete levels MT=51-90 without MF=6: the deposited energy is
  `σ·(E + q0 − yld·Ē')`, with `Ē' = E·(1 + 2bw̄ + b²)/(A+1)²`, `b = A·√(1 −
  E_th/E)`, and `w̄` MF=4's first Legendre coefficient from `hgtfle`
  (`File4Angular` with `File4Options::HEATR`: `toler = 1e-6`, MT=51-90
  lab data taken as CM when `A ≥ 10`, the log-law sign fallback). `c = Ē'/E`
  is evaluated at nodes 10 % apart, pulled down to the next MF=4 energy, and
  interpolated lin-lin between them, walked from the reaction's threshold
  energy as `nheat` does.
- **MT=600-849:** `σ·(E + q0)` (`disbar` leaves `c = 0` for a charged
  outgoing particle).
- **`nheat`'s skip list** (`:1066-1110`): MT=3, 4, 10, 26, 27, 101, 121-151,
  201-599, MT=103-107 when their partials MT=600-849 exist, MT=16 when
  875-890 exist, MT=18 when MT=19 exists. RECONR rebuilds MT=4 as the sum of
  the levels, and the port had been heating it beside them.

Gates in `tests/heatr_mt442_vs_njoy2016.rs`:
`heating_matches_njoy_where_elastic_and_mf12_capture_are_the_only_channels`
(Si-28 MT=301 below the first inelastic threshold, both `local` settings,
every point within 1e-6 of the envelope, the MT=442 criterion, fixed before
the run) and `mt4_is_not_heated_beside_the_levels` (at 2 MeV on Fe-58, ours
within half of the double-count term `σ_4·E·2A/(A+1)²` of NJOY).

**Results, MT=301** (median `|ours/njoy − 1|`; worst distance outside the
envelope in brackets):

| comparison | E < 1 eV | 1 eV – first inelastic | above first inelastic |
|---|---|---|---|
| Fe-58, `local = 1` | 3.3e-2 (3.3e-2) | 1.7e-2 (5.5e-2) | **3.3e-4** (0.99 at 140 MeV) |
| Si-28, `local = 1` | 5.3e-8 (1.7e-7) | **8.0e-8 (4.0e-7)** | **2.3e-3** (1.00 at 150 MeV) |
| Fe-58, `local = 0` | 755 (755) | 6.3e-2 (850 at 37.8 keV) | **3.9e-3** (1.00 at 25.7 MeV) |
| Si-28, `local = 0` | 5.9e-8 (2.0e-7) | **7.3e-8 (4.7e-7)** | **7.4e-3** (1.00 at 20.2 MeV) |

Above the first inelastic threshold, the medians were 0.11, 0.31, 1.35 and
0.99 after section 4. MT=4 test: ours − NJOY = 4.75e2 eV·b at 2 MeV, against
8.00e4 for a double count.

**Interpretation.**

- **Si-28 below the first inelastic threshold is exact at print precision.**
  Elastic and MF=12 capture are the only open channels, so this verifies
  `disbar` (anisotropic mean cosine, node chain), capture's energy balance
  and recoil (H6a), and the MT=442 subtraction together.
- **Above the threshold the medians fell by 30-350×.** The two-body port and
  the MT=4 skip carry it; `nheat`'s `q0` rule (section 4) was already in.
- **The worst points are at 14-150 MeV** (and at a few near-zero values),
  where the continuum and MF=6 neutron means are still the kinematic
  estimate: `conbar` and `sixbar`, H6b part 2.
- **Fe-58's residual below the threshold is its capture**, whose photons are
  MF=6: NJOY deposits only the photon recoil (`tabsq6`), where the port
  deposits the 208 keV photon deficit. That is H6c, and it is 3.3 % at
  `local = 1` and 755× at `local = 0` below 1 eV, where the deficit is all
  that is left after the photons are removed.

## What this does NOT establish

- **No validation.** Agreement with another code on the same evaluation is not
  agreement with measurement. Nothing here says ENDF/B-VIII.0's photon or
  damage data is right — indeed Fe-58's capture photon data is shown to be
  208 keV short of its own Q.
- **No human review.** Per `RESPONSIBLE_USE.md`, untrusted AI-assisted draft.
- **Two nuclides, both mid-mass, at 0 K.** Says nothing about actinides (U-238's
  HEATR tape is 144 MB and is not committed; its deck is) or about temperature
  dependence.
- **The KERMA comparison is asserted only below the first inelastic threshold.**
  Above ~1 MeV the two codes' H5 treatments diverge by −75 % (Fe-58) to +73 %
  (Si-28) ~~for reasons this comparison cannot separate — the kinematic limit and
  the energy-balance method are different quantities there, and untangling them
  needs the photon-production side (MT=442), which is not compared here~~.
  **CORRECTED 2026-10-05:** MT=442 is now compared (section 4) and matches;
  most of Fe-58's gap was the deposited Q, and with `nheat`'s rule the median
  miss above threshold at `local = 1` is 11 % (Fe-58) and 31 % (Si-28). ~~What
  remains is the neutron side (H6b), recorded and not asserted.~~
  **CORRECTED 2026-10-05 (H6b part 1, section 5):** with `disbar` and
  `nheat`'s skip list the medians above threshold are 3.3e-4 (Fe-58) and
  2.3e-3 (Si-28) at `local = 1`, recorded and not asserted. Below the
  threshold Si-28 is now asserted at both `local` settings. `conbar`,
  `sixbar` and Fe-58's MF=6 capture (H6c) remain.
- **The damage port remains deliberately partial**: no MF=4 anisotropy, no
  MT=447 disappearance recoil, no continuum or `(n,xn)` recoil. Sections 2 and 3
  above now quantify that rather than merely naming it.
