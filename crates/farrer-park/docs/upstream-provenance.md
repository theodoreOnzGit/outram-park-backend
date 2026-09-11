# Farrer Park — upstream provenance

The record of what was vendored, at which commit, and what its licence actually
says. `vendor/` at the workspace root is **gitignored**, so the trees themselves
are not in this repository — this file is the durable record of what was read.

Vendored 2026-09-11 (bead `op-1zd8`), shallow clones (`--depth 1`):

| Upstream | Path | Commit | Commit date | Size |
|---|---|---|---|---|
| [idaholab/moose](https://github.com/idaholab/moose) | `vendor/moose` | `fe80d24fa4c58f677d6672e5063ec08715953c32` | 2026-09-10 | 1.1 GB |
| [prisms-center/plasticity](https://github.com/prisms-center/plasticity) | `vendor/prisms-plasticity` | `ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4` | 2026-08-27 | 205 MB |
| [prisms-center/Fatigue](https://github.com/prisms-center/Fatigue) | `vendor/prisms-fatigue` | `2c8fc9a2cc85c0b2b3dcfac987f3a9e603b7fb9f` | 2023-10-13 | 57 MB |

Reproduce with:

```bash
mkdir -p vendor
git clone --depth 1 https://github.com/idaholab/moose.git            vendor/moose
git clone --depth 1 https://github.com/prisms-center/plasticity.git  vendor/prisms-plasticity
git clone --depth 1 https://github.com/prisms-center/Fatigue.git     vendor/prisms-fatigue
```

## Licences, read from each tree's own files

Checked on 2026-09-11 against the vendored sources, not inferred from a badge or
a package index.

**MOOSE — LGPL-2.1 (not "or later").** `vendor/moose/LICENSE` is the verbatim
LGPL-2.1 text, and every source file carries the header
`//* Licensed under LGPL 2.1, please see LICENSE for details`. No "or any later
version" wording appears. Note `COPYRIGHT` at the repo root is a **U.S.
Government rights notice** arising from DOE and NSF contracts (Battelle Energy
Alliance / INL, Triad National Security / LANL) — it is *not* the licence, and
reading it alone gives the wrong answer.

**PRISMS-Plasticity — LGPL-2.1-or-later.** `vendor/prisms-plasticity/LICENSE`
opens with a custom preamble before the LGPL text:

> (c) 2016 The Regents of the University of Michigan, PRISMS Center
>
> This code is a free software; you can use it, redistribute it, and/or modify
> it under the terms of the GNU Lesser General Public License as published by
> the Free Software Foundation; **either version 2.1 of the License, or (at your
> option) any later version.**

This is *more* permissive than the other two, not less.

**PRISMS-Fatigue — LGPL-2.1.** `vendor/prisms-fatigue/LICENSE` is the verbatim
LGPL-2.1 text; the README refers to it without an "or later" grant.

## Why GPL-3.0-only is clean, and one-way

LGPL-2.1 section 3 permits a licensee to opt into the ordinary GPL instead, and
explicitly allows choosing a later GPL version. Quoting the clause the
relicensing rests on, verbatim from `vendor/prisms-plasticity/LICENSE`:

> You may opt to apply the terms of the ordinary GNU General Public License
> instead of this License to a given copy of the Library. To do this, you must
> alter all the notices that refer to this License, so that they refer to the
> ordinary GNU General Public License, version 2, instead of to this License.
> **(If a newer version than version 2 of the ordinary GNU General Public
> License has appeared, then you can specify that version instead if you wish.)**

So GPL-3.0 is explicitly permitted for all three, and Farrer Park exercises that
option — consistent with the rest of this workspace.

**The flow is ONE-WAY.** Code may come from these upstreams into this
GPL-3.0-only crate; code from this crate **cannot** go back to MOOSE or the
PRISMS codes under their LGPL terms. Section 3 also notes the change "is
irreversible for that copy". Same shape as `outram-park-fork-pflotran`
(LGPL-2.1 PFLOTRAN into GPL-3.0) and `raffles` (Apache-2.0 RAVEN into GPL-3.0).

## What is actually relevant to the port

- `vendor/moose/modules/solid_mechanics`, `tensor_mechanics`, `contact` — the
  structural-mechanics modules.
- `vendor/prisms-plasticity/src/{ellipticBVP, materialModels, enrichmentModels,
  userInputParameters, utilityObjects}` — the crystal-plasticity FEM layer
  (bead `op-q75c`).
- `vendor/prisms-fatigue` — FIPs and microstructure-sensitive fatigue
  (bead `op-q1zn`), plus the published case studies that are the parity targets
  (bead `op-9smz`).

## Why this matters operationally

The workspace "Debugging a port: read upstream first" hard rule requires reading
the upstream routine that owns a behaviour **before** hypothesising about a
discrepancy. That rule is unusable without the sources on disk. Any session
debugging a Farrer Park discrepancy should re-clone per the commands above if
`vendor/` is absent — an ephemeral container will not have it.

Per-file attribution headers remain mandatory on anything ported from these
trees; see `NOTICE`.

## What was actually read, and where each piece landed

Added 2026-09-11 with the crystal-plasticity and fatigue work (beads `op-q75c`,
`op-q1zn`). The workspace "Debugging a port: read upstream first" hard rule
makes upstream the specification, so the specific routines consulted are
recorded here rather than left implicit in the code.

### PRISMS-Plasticity (`ffdf4eb6`)

| Upstream file and location | What it settled | Where it landed |
|---|---|---|
| `applications/crystalPlasticity/fcc/*/slipNormals.txt`, `slipDirections.txt` | the twelve FCC `{111}<110>` systems **and their ordering** | `src/crystal/slip.rs`, `SlipFamily::systems` |
| `applications/crystalPlasticity/bcc/simpleTension/slip*.txt` | the twelve BCC `{110}<111>` systems and their ordering | same |
| `applications/.../LatentHardeningRatio.txt` | `q = 1.0` coplanar, `1.4` non-coplanar, in 3x3 (FCC) and 2x2 (BCC) diagonal blocks | `SlipFamily::latent_hardening_matrix`; verification case 12 checks the computed matrix against this file entry for entry |
| `src/materialModels/crystalPlasticity/MaterialModels/RateDependentModel/calculatePlasticity.cc:213` | the Schmid tensor is built as `m (x) n` in the crystal frame | `SlipSystem::schmid_tensor`, symmetrised (see below) |
| ...`:216-220` | it is carried into sample axes as `R S R^T`, so `rotmat` is **crystal-to-sample** | `Orientation`'s stored convention, `rotate_symmetric` |
| ...`:693` | the power-law flow rule, and that `delgam_ref = gamma_dot_0 * delT` and `strexp = m` | `PowerLawFlow::slip_increment` |
| ...`:645-657, 698` | `h_b = h0 (1 - s_b/s_sat)^A` indexed by the **slipping** system, accumulated as `s_a += q[a][b] h_b \|d gamma_b\|` | `SaturatingHardening::advance` |
| ...`:700-708` | **the clamp `s <- min(s, s_sat)`** | `SaturatingHardening::advance`; see below |
| ...`:594-602` | a cubic line search (`lnsrch`) on `0.5 \|R\|^2` stabilises the local Newton | replaced by backtracking in `CrystalPlasticity::update` |
| ...`:258-301, 335` | deformation-increment sub-stepping (`numberOfCuts`) | **not ported** — bead `op-ypfy` |
| ...header comment | the local Newton starts from the **previously converged stress** | `CrystalState::stress`, used as the initial guess |
| `src/materialModels/crystalPlasticity/rotationOperations.cc`, `odfpoint` | Rodrigues to rotation matrix, `R = ((1-r.r) I + 2 r (x) r - 2 eps_ijk r_k)/(1+r.r)` | `Orientation::from_rodrigues` |
| `src/materialModels/crystalPlasticity/calculatePlasticity.cc:52` | the crystal stiffness is rotated into sample axes per grain | `CrystalElasticity::stiffness_in_sample_frame`, done by full fourth-order contraction instead of a Voigt Bond matrix |
| `applications/crystalPlasticity/fcc/FCC_Random_RateDependent/prm.prm` | the constants this crate uses as its worked example: cubic `C11/C12/C44 = 170/124/75` GPa, `s_0 = 16` MPa, `h_0 = 180` MPa, `s_sat = 148` MPa, `A = 2.25`, `gamma_dot_0 = 1e-3`/s, `m = 0.02`-`0.1`, `dt = 0.1` s | `tests/crystal.rs`, `copper()` |

**The single most important thing read.** The clamp at `saturationStress`
(`calculatePlasticity.cc:700-708`) is applied unconditionally after every
hardening update. It is not cosmetic: without it `1 - s/s_sat` goes negative on
the next step and `pow(negative, A)` with upstream's own non-integer
`A = 2.25` is **NaN**, which then propagates into the stress silently. A
first-principles reading of the hardening law would not have produced it. This
is exactly the failure mode the "read upstream first" rule exists to catch — a
missing *guard*, not a wrong *formula*. `crystal::flow`'s unit test asserts both
halves: that the clamped path stays finite, and that the unclamped expression
really is NaN.

**Where this port deliberately differs.** Upstream is finite-deformation
(`Fe`/`Fp`, exponential update of `Fp`, second Piola-Kirchhoff stress on the
intermediate configuration, lattice reorientation from the plastic spin);
Farrer Park is small strain, so the additive split `eps = eps_e + eps_p` is
used with the **symmetric** Schmid tensor `sym(m (x) n)` rather than upstream's
unsymmetrised `m (x) n`. Contracting either with a symmetric Cauchy stress
gives the same resolved shear, so nothing is lost in `tau`; what is lost is the
spin, and therefore texture evolution. Upstream's Ohno-Wang backstress is also
not ported. Both are recorded as beads (`op-tau7` and the crystal-plasticity
limitations in `crates/farrer-park/CLAUDE.md`), not left to be discovered.

### PRISMS-Fatigue (`2c8fc9a2`)

| Upstream file and location | What it settled | Where it landed |
|---|---|---|
| `src/calculate_FIPs.py:75-76` | the plastic shear strain measure is the **half**-range, `(gamma^tension - gamma^compression) / 2`, taken from the tension and compression peaks of the final cycle | `CycleExtremes::shear_strain_amplitudes` |
| ...`:82-83` | the plane-normal stress is taken **at the point of maximum tension** and **clipped at zero** | `CycleExtremes` and `FatemiSocie::fip` |
| ...`:94-97` | `FS_FIP = (d gamma/2) (1 + k sigma_n / sigma_y)` | `FatemiSocie::fip` |
| ...`:210-213` | the defaults `k = 10.0` and `sigma_y` = the macroscopic yield stress | `FatemiSocie::with_default_weight` |
| `src/volume_average_FIPs.py`, `Al7075_band_averaging` | average **within** a region, then take the maximum over slip systems and regions as the grain's value — the order is not interchangeable | `fatigue::region_fip`, `rank_regions` |
| `src/generate_microstructures.py` (band construction) | how a grain is partitioned into slip bands | **not ported** — bead `op-9jo0` |
