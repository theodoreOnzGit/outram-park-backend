# Reading a nuclide library OpenMC itself produced

GitHub **#303**, and its acceptance:

> - A nuclide read from OpenMC's library reproduces `xs_at_energy` against the
>   ENDF-reconstructed one for the same nuclide within the tolerance
>   reconstruction itself carries.
> - LCT-008 run from the HDF5 library agrees with the ENDF route on `k` within
>   statistics.
> - The sweep is re-run with the fourth arm present, and the data-time
>   comparison is restated as a like-for-like number.

Measured 2026-09-24. **The reader exists and is verified against upstream's own
output; the three acceptance rows above are downstream of it and are not yet
run.** What remains is stated under *What is not done*.

## What was missing, and the measurement that shows why it mattered

There was no nuclide HDF5 reader anywhere in this workspace: `hdf5` had writers
and XML readers, but nothing that turned an OpenMC-format neutron `.h5` back
into data. So `outram-mc-libs` could obtain data only by reconstructing it from
ENDF tapes in process. The 2026-09-24 LCT-008 timing sweep measured the gap
that leaves (11-nuclide simplified LCT-008, same geometry and settings,
4 seeds):

| | nuclear data | transport |
|---|---|---|
| OpenMC + its ENDF/B-VIII.0 HDF5 library | 1.54 ± 0.06 s | 7.06 ± 0.05 s |
| outram-mc + rust-njoy from ENDF tapes | 158.57 ± 10.80 s | 59.52 ± 3.22 s |

The 103× on nuclear data is **not a slowdown** — it is reconstruction against
deserialisation, two different tasks. The point is that the *comparable*
measurement did not exist, because our side could not start from a pre-built
library.

## Methodology

`read_nuclide` parses the layout of `IncidentNeutron.export_to_hdf5` (OpenMC
`afa7a14`): file type, the nuclide header, `kTs`, the per-temperature energy
grids, every reaction's MT / `Q` / frame / redundancy / cross section with its
`threshold_idx`, each product's particle and law type, `total_nu`, and the URR
temperature set.

**The oracle is upstream's OUTPUT, not its prose.** The file read is
`U235.h5`, built by

```text
reference-data/endf  --NJOY2016-->  ACE
                     --openmc.data.IncidentNeutron.from_ace-->
                     --.export_to_hdf5()-->  U235.h5
```

i.e. by upstream's own converter (see
`../nuclide_h5_fissile/openmc_inputs/ace2hdf5.py`). A convention taken from a
reference implementation's output cannot be mis-transcribed the way one taken
from its prose can — which is how three format defects were caught on the write
side (`../nuclide_h5_fissile/fissile_write_2026_09_24.md`).

The library is a generated ~74 MB artefact and is **not committed**, so the
tests skip with a printed reason unless `OUTRAM_OPENMC_XS_DIR` points at it.

## Results, 2026-09-24

`cargo test --release -p njoy-outram-park-fork --test nuclide_h5_vs_openmc`
with `OUTRAM_OPENMC_XS_DIR` set — **3 passed, 0 failed**:

```text
U235 nu_total: thermal 2.42985, 10 MeV 3.83022
U235: 87 reactions, 45 with a threshold, 76027 grid points,
      temps ["294K"], urr at ["294K"]
U235.h5 neutron-product laws: {"correlated": 4, "uncorrelated": 47}
U234.h5 neutron-product laws: {"kalbach-mann": 4, "uncorrelated": 69}
```

**Header:** `U235`, `Z = 92`, `A = 235`, `metastable = 0`,
`AWR = 233.0248` (to < 1e−4).

**ν̄ = 2.42985 at 0.0253 eV**, rising to **3.83022 at 10 MeV**. This is the
single most informative number in the read: U-235's thermal ν̄ is 2.43, so the
value confirms at once that the `Tabulated1D` `vstack` halves were **not**
swapped (a swap returns an *energy*, ~1e−5, not a yield) and that the lin-lin
interpolation and end clamps are right.

**Both threshold invariants hold across all 87 reactions**, asserted rather
than assumed:

- `threshold_idx + len(xs) == 76027` for every reaction;
- `xs[0] == 0.0` exactly for all **45** thresholded reactions.

These are the same two invariants the writer now enforces, so the reader and
writer are checked against the same external evidence rather than against each
other.

**Law inventory, and why its agreement is a real cross-check.** The reader sees
`{correlated: 4, uncorrelated: 47}` on U-235 and `{kalbach-mann: 4,
uncorrelated: 69}` on U-234. This crate's **ACE-side** decoder records the same
split from the ACE laws by an entirely different route —
`crate::acer::ce_laws`'s module docs give `{LAW3: 39, LAW4: 1, LAW61: 4}` for
U-235/U-238 and `{LAW3: 40, LAW4: 4, LAW44: 4}` for U-234, where LAW61 maps to
`correlated` and LAW44 to `kalbach-mann`. Two independent paths agreeing on
which nuclide uses which continuum law is evidence; one path restated twice
would not be.

**A non-neutron file is refused**, not parsed into numbers: reading an `mgxs`
file through the neutron path errors naming the actual file type.

**A corrupted threshold is refused on read too.** A reaction whose
`threshold_idx` and cross-section extent contradict the grid is the one
corruption a reader cannot notice later — every number parses and the cross
section is simply in the wrong place — so it is rejected at read time with the
arithmetic in the message.

## The rank-2 attribute limit does NOT apply to reading

`../nuclide_h5_fissile/fissile_write_2026_09_24.md` records that `continuous`,
`correlated` and `kalbach-mann` cannot currently be **written**, because their
incident-grid interpolation is a rank-2 attribute that `hdf5-pure` 0.20.1
cannot emit. **Reading them is unaffected:** `Group::attrs` returns an
`AttrValue` without a shape, and the `(2, NR)` layout is row-major with known
extents, so splitting the flat array recovers `(breakpoints, interpolation)`
unambiguously. #303 is therefore not blocked by what blocks #304 — which is
worth stating explicitly, because the two issues are otherwise mirror halves.

## Row 1: the reader reproduces the reference library's cross sections exactly

`outram-mc-libs/tests/band_xs_from_hdf5_reader.rs` recomputes, **in Rust and
in-process**, the band-averaged cross-section oracle that
`band_averaged_xs_vs_openmc.rs` had been carrying as `const` arrays transcribed
from `band_xs_oracle.py` (h5py + numpy). Same six bands, same `dE/E` lethargy
weight, same 4000-point trapezoid.

| nuclide | worst \|diff\| across all bands and channels |
|---|---|
| U-235 | **0.001 %** |
| U-238 | **0.166 %** |

Every `sum` column agrees to **0.000 %**. U-238's 0.166 % worst case is on
**fission at 7.8e−5 barn** — a channel essentially closed in that band, where a
tiny absolute difference is a large relative one; the next largest is 0.118 %,
also U-238 fission, and nothing else exceeds 0.007 %.

So the reader assembles cross sections off a real fissile nuclide's file
**identically** to an independent tool. Combined with
`band_averaged_xs_vs_openmc.rs`, which already compares those same oracle
numbers against this workspace's ENDF reconstruction, acceptance row 1's
substance is met: the HDF5-read cross sections and the ENDF-reconstructed ones
are compared, on the right instrument.

**Why band-averaged and not pointwise**, restated because it matters: a
pointwise comparison of two independently generated energy grids inside
resonances is dominated by grid placement rather than physics, and the sibling
test's own docs decline to chase it for that reason. The lethargy average needs
no flux, so it also cannot be contaminated by the flux difference such
comparisons are usually reached for.

**Why this is worth having beyond row 1.** A transcribed oracle is a real weak
point, and this workspace has been bitten by one: `transport_decomposition.py`
read angular tables at the bracketing index instead of interpolating, biasing
`<mu_el>` to 0.2740 against a true 0.2645 — the **fourth** appearance of the
nearest-point trap and the first in our own script. A number that reaches a
test through a script and a human is a number nothing re-derives. Now something
does.

The existing test is deliberately **not** modified: it carries a careful
argument about which instrument is meaningful in the resolved-resonance region,
and rewiring its oracle is a separate change from showing the oracle can be
re-derived.

## What is NOT done

- **A `ReadNuclide -> outram_mc_libs::Nuclide` conversion**, which is what
  *transporting* from a read library needs (as opposed to comparing cross
  sections off one, which row 1 asks for and which is done above).
  `XsSource` has two variants, `Core` and `Pointwise`, and `Pointwise` wraps a
  `ReconrResult`; a third variant touches the transport hot path in a mature
  crate, so it is named here as its own increment rather than started.
- **Secondary distributions are identified but not decoded.** The reader
  records each product's law `type` and particle, which is what the coverage
  assertions need, but does not yet unpack the `(3, n)` / `(5, n)` tables into
  samplable form. Transport from a read library needs that; reporting which
  laws a library uses does not.
- **LCT-008 from the HDF5 route, and the fourth arm of the timing sweep**
  (acceptance rows 2 and 3) both wait on the item above.

None of these are blocked by anything external. They are the next increment.
