# The ACE pipeline: NJOY2016 for OpenMC, and whether this workspace's NJOY reproduces it

**Generated:** 2026-09-20, UTC.
**Class:** verification — code-to-code against NJOY2016 and a working OpenMC
run. **Not validation**: no experiment is compared against, and no human V&V.
AI-assisted draft.

## Why this exists

The ICSBEP study compares three arms:

1. **OpenMC + official HDF5 data** — OpenMC's own pipeline end to end
2. **OpenMC + ACE generated here by NJOY2016** — same transport, our data processing
3. **`outram-mc-libs` + the same evaluations** — our transport

Arm 1 → arm 2 isolates data processing; arm 2 → arm 3 isolates transport. This
record covers building arm 2, and then asks the separate question of whether
**this workspace's own Rust NJOY port** reproduces the NJOY2016 tables arm 2
rests on.

**Arm 1 cannot be run in this container.** Every source of the official HDF5
library is refused by egress policy — `openmc.org`, `anl.box.com`,
`nucleardata.lanl.gov` and `www-nds.iaea.org` all return `connect_rejected`.
Recorded here so a future reader does not assume it was merely skipped.

## Part 1 — the reference ACE set, and OpenMC running on it

### Toolchain, built in-session

| tool | provenance |
|---|---|
| NJOY2016 | `github.com/njoy/NJOY2016`, cmake + gfortran, ~1 min |
| OpenMC | `openmc-dev/openmc` commit `afa7a14ac5cb8630f642a77229ca64dc3eaeef81`, Release, MPI off |
| pugixml | v1.15, commit `ee86beb30e4973f5feffe3ce63bfa4fbadf72f38` |
| fmt | 11.0.2, commit `0c9fce2ffefecfdce794e1859584e25877b7b592` |

OpenMC's CMake pins pugixml and fmt as **tarball URLs**, which this
environment's proxy refuses with 403 (`codeload.github.com` likewise). They
were cloned at the **exact pinned tags** over git and handed to FetchContent as
local source dirs — *not* replaced with apt's versions, which are pugixml 1.14
and **fmt 9.1.0** against a pinned 11.0.2. No OpenMC source was patched.

`openmc --version` reports `0.0.0` because the clone is `--depth 1` and the tag
history CMake reads the version from is absent. **The commit hash is the honest
identifier.**

### Getting the set without regenerating it

The generated tables are published as **`reference-data/ace`**, a git submodule
pointing at [`theodoreOnzGit/ace_and_other_data`](https://github.com/theodoreOnzGit/ace_and_other_data).

```bash
git submodule update --init reference-data/ace
gunzip -c reference-data/ace/endf-b-viii.0/293.6K/U235.ace.gz > U235.ace
```

**They are gzipped because they have to be.** ACE is ASCII and compresses about
6.4x; raw, U-235 is 129.6 MB and the 0 K U-235 is 301.6 MB, both over GitHub's
**100 MiB per-file hard limit**. gzip was chosen over Git LFS so a plain clone
suffices with no LFS quota. `MANIFEST.tsv` carries the SHA-256 of each
*uncompressed* table; all four were verified to round-trip to the exact
checksum and byte count before publishing.

**The 0 K table is not for transport.** It is the matched oracle for Part 2
below and nothing else.

This is the **first submodule in this workspace**, so a fresh clone needs
`git clone --recurse-submodules`, or `git submodule update --init` after the
fact. Without it `reference-data/ace/` is an empty directory rather than an
error, which is the failure mode to recognise.

### Generating the set

`make_ace.sh` (already committed) drives RECONR → BROADR(293.6 K) → PURR →
ACER. MAT numbers were read **out of the tapes themselves**, not recalled:

| nuclide | MAT | tape | ACE bytes |
|---|---|---|---|
| U-234 | 9225 | `n-092_U_234-ENDF8.0.endf` | 9 107 181 |
| U-235 | 9228 | `n-092_U_235-ENDF8.0.endf` | 135 931 655 |
| U-238 | 9237 | `n-092_U_238.endf` | 126 511 619 |

All three `exit=0`. The written header confirms the temperature:
`92235.00c 233.024800 2.5300E-08` — `kT = 2.53e-8` MeV = 0.0253 eV = 293.6 K.

### OpenMC reads HDF5, not ACE

The transport solver does not consume ACE. `ace2hdf5.py` converts with
**OpenMC's own reader** (`openmc.data.IncidentNeutron.from_ace`) rather than a
reimplementation, and emits the `cross_sections.xml`. All three converted at
294 K with the expected atomic weight ratios.

The Python package needs **Python ≥ 3.12**; this container's `python3` is
3.11.15, with 3.12.3 and 3.13.12 present but shadowed. A 3.12 venv is the fix.

### The result: OpenMC runs on it

`godiva_ace_model.py` builds ICSBEP HEU-MET-FAST-001 with the atom densities
from this workspace's own `examples/godiva_keff_endf_local.rs`, so both codes
run the same model. 10 000 particles × [50 inactive + 200 batches]:

```
 k-effective (Collision)     = 0.99916 +/- 0.00091
 k-effective (Track-length)  = 0.99976 +/- 0.00077
 k-effective (Absorption)    = 0.99832 +/- 0.00104
 Combined k-effective        = 0.99939 +/- 0.00061
 Leakage Fraction            = 0.57385 +/- 0.00043
```

**−61 ± 61 pcm** from the benchmark's 1.0000 on the combined estimator. The
57.4 % leakage matches the bare sphere's documented value. **Arm 2 is live.**

This is a single run, not a pooled ensemble — quote it as such.

## Part 2 — does this workspace's NJOY reproduce NJOY2016?

Driver: `njoy-outram-park-fork/examples/ace_vs_njoy2016.rs`.

### The oracle had to be matched first

`examples/write_ace.rs` runs **RECONR only, at 0 K**. The production deck runs
**RECONR → BROADR(293.6 K) → PURR → ACER**. Comparing them would confound
Doppler broadening, probability tables and any genuine ACER discrepancy, and a
difference could be attributed to none of them. So `make_ace_0k.sh` produces a
**matched 0 K RECONR→ACER-only** oracle. Its header reads
`kT = 0.0000E+00`, and it is 316 MB against the broadened set's 136 MB —
unbroadened resonances need a far finer grid.

### A methodological error, found and fixed before it was reported

The first comparator diffed `xss[i]` against `xss[i]`. The two union grids are
**not the same length** (ours 237 049 points, NJOY's 233 515), so index `i` is
a *different energy* in each file. That version reported absorption differing
by a factor of **1753**, which was entirely an artefact of the misalignment.
Its "comparing the prefix bounds the agreement" caveat was also wrong: a prefix
diff across mismatched grids bounds nothing.

**Both figures below are kept deliberately**, because the difference between
them is the point:

| comparison | total | absorption | elastic |
|---|---|---|---|
| ours interpolated onto NJOY's grid | 3.355e-3 | 9.839e-3 | 6.297e-3 |
| **at the 2463 shared grid points, no interpolation** | **4.697e-7** | **1.527e-6** | **8.058e-7** |

The interpolated row is dominated by the *comparison's* own lin-lin error
across resonance peaks — every one of its worst cases sits at 1.4–2.2 keV, in
the resolved resonance region, which is exactly where under-sampling a peak
bites. Removing it improves the agreement by **three orders of magnitude**.

**The port agrees with NJOY2016 to ~1e-6 relative, i.e. 6–7 significant
figures**, consistent with the crate's recorded maturity bar. Both grids span
exactly `[1e-11, 30]` MeV; ours carries 1.51 % more points, which is ordinary
adaptive-subdivision difference at the same 0.001 tolerance.

Only **1.1 %** of NJOY's energies are also in ours. That is a small shared
sample and is stated rather than glossed: the two adaptive grids genuinely
subdivide differently, and the 1e-6 figure is measured on their intersection.

### What the port does NOT yet produce

| gap | evidence | consequence |
|---|---|---|
| **fission ν̄ (NU) block** | `JXS(2)`: ours `0`, NJOY `1167576` | **An ACE with no NU block cannot drive a fission eigenvalue** — ν̄ = 0, no fission source. This alone blocks feeding our port's ACE to OpenMC. |
| **37 reactions** | ours 47 MTs, NJOY 84 | missing MT=649 ((n,p) continuum) and MT=800–835 ((n,α) discrete levels) |
| **photon production** | `NXS(6)` NTRP: ours `0`, NJOY `583` | no photon transport from our tables |

No MT is present in ours and absent from NJOY — the port is a strict subset,
not a divergent set.

**Heating is not compared.** The 0 K oracle deck has no HEATR stage, so NJOY's
heating column is identically zero while this port always computes a KERMA.
That is the *deck* differing, not the port, and the comparator says so rather
than printing a silent `n=0`.

## What this does NOT establish

- Not validation. No experiment.
- The 1e-6 agreement is **RECONR + ACER at 0 K only**. BROADR and PURR are
  compared separately by `tests/broadr_light_nuclide_pendf_golden.rs` and
  `tests/purr_u238_ptables_vs_njoy.rs`; this record says nothing about them.
- One nuclide (U-235) and one temperature (0 K).
- The Godiva number is a single OpenMC run, not a pooled ensemble.
