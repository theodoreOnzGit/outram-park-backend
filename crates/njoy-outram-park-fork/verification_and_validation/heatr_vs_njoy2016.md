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

So the difference is in how `disbar` bounds the recoil, which has **not** been
read. Magnitude: 0.86 eV·b against a scale reaching 1e5, confined to a ~45 eV
window at threshold. Recorded as an open question rather than guessed at.

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
  (Si-28) for reasons this comparison cannot separate — the kinematic limit and
  the energy-balance method are different quantities there, and untangling them
  needs the photon-production side (MT=442), which is not compared here.
- **The damage port remains deliberately partial**: no MF=4 anisotropy, no
  MT=447 disappearance recoil, no continuum or `(n,xn)` recoil. Sections 2 and 3
  above now quantify that rather than merely naming it.
