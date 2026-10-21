# Benchmark composition maps

Material-number maps for the benchmark cases, one integer per node, comma
separated, one row per `ix`.

## `IAEA3DS_1.csv` … `IAEA3DS_4.csv`

**What they are.** The four axial composition layers of the **IAEA 3-D PWR
benchmark**, on the 17 x 17 quarter-core lattice. Material numbers are:

| Number | Material |
|---|---|
| 1 | outer fuel |
| 2 | inner fuel |
| 3 | inner fuel + control rod |
| 4 | reflector |
| 5 | reflector + control rod |

Which file applies to which axial level is decided by `iaea3ds.m` and
reproduced in `crate::iaea3ds`:

| File | Axial levels (1-based, of 19) |
|---|---|
| `IAEA3DS_1` | 1 — bottom reflector |
| `IAEA3DS_2` | 2–14 — the lower core |
| `IAEA3DS_3` | 15–18 — the upper core, where the rods sit |
| `IAEA3DS_4` | 19 — top reflector |

**Where they came from.** Transcribed byte-for-byte from the
`main_exec_diff3d_standalone` MATLAB snapshot handed over by **Than Yan Ren
(Singapore Nuclear Research and Safety Institute)**, where they live in
`BEDOKfiles/` and are read by `iaea3ds.m` with `readmatrix`. The only change
made was **stripping the UTF-8 byte-order mark** from the first line of each,
so `include_str!` parsing does not have to special-case it. No values were
altered; see the round-trip check in `crate::iaea3ds`'s tests.

**Underlying source.** These encode the IAEA 3-D PWR benchmark, a long-published
and widely reproduced reactor-physics test problem. `iaea3ds.m`'s own header
records two independent reference eigenvalues for the 10 cm mesh:

```
PARCS  K-EFF : 1.029096
ADPRES K-EFF : 1.029082  (error = 1.4 pcm)
```

Those two numbers are quoted **from that header**, which is the provenance this
repository actually has for them. They have not been checked against a primary
publication here, and the benchmark's originating document is not in
`crates/kovan-literature`. Anyone citing this comparison in a paper should
obtain the primary reference and confirm the values first — see
`DATA_POLICY.md` on data provenance, and the workspace `CLAUDE.md` rule that
literature informing the code belongs in the KOVAN archive.

**Licence / access tier.** The IAEA-3D benchmark specification is open,
published reactor-physics literature, and these maps are a numerical
restatement of its geometry. They carry no restriction and are committable.
They contain no facility-operational, proprietary or partner-confidential data.

## `NEACRPD1_1.csv`, `NEACRPD1_COL.csv`

**What they are.** The material map of the **NEACRP 3-D LWR core transient
benchmark (1991), BWR case D**, on the 17 x 17 quarter-core lattice, split into
two files rather than one per axial level:

| File | Shape | Meaning |
|---|---|---|
| `NEACRPD1_1` | 17 x 17 | which of 10 radial **column types** each lattice position is; `0` is outside the core outline |
| `NEACRPD1_COL` | 14 x 10 | the material number at each of the 14 axial levels, for each column type |

A node's material is `NEACRPD1_COL(iz, NEACRPD1_1(ix, iy))`, which is why the
core outline is necessarily a right prism — see the test in `crate::neacrpd1`.
Material numbers run 1 to 19 and index the cross-section tables built in that
module; 1, 4 and 19 are reflectors (bottom, top and radial) and do not fission.

**Where they came from.** Transcribed byte-for-byte from the
`main_exec_diff3d_standalone` MATLAB snapshot handed over by **Than Yan Ren
(Singapore Nuclear Research and Safety Institute)**, where `neacrpd1.m` reads
them with `readmatrix`. The only change was **stripping the UTF-8 byte-order
mark** from the first line of each. No values were altered.

**Underlying source.** These encode a case of the NEACRP 3-D LWR core transient
benchmark, published reactor-physics literature. **Unlike the IAEA-3D files
above, `neacrpd1.m` quotes no reference eigenvalue**, and the benchmark
specification is **not** in `crates/kovan-literature`. There is therefore no
published number to compare against, and `crate::neacrpd1`'s tests assert
structural properties only. Obtain the primary reference and catalogue it
through `kovan lit import` before making any parity claim on this case.

**Licence / access tier.** The NEACRP benchmark specification is open,
published reactor-physics literature and these maps are a numerical restatement
of its geometry. No restriction; committable. They contain no
facility-operational, proprietary or partner-confidential data.

## `NEACRPA2_1.csv`, `NEACRPA2_2.csv`, `NEACRPA2_3.csv`, `NEACRPA2_CRODBANKS.csv`

**What they are.** The radial maps of the **NEACRP 3-D LWR core transient
benchmark (1991), PWR case A2**, on the 17 x 17 core octant.

| File | Applies to | Contents |
|---|---|---|
| `NEACRPA2_1` | axial layers 1 and 18 | the axial reflector; materials 1-3 |
| `NEACRPA2_2` | axial layer 2 | the transition layer; materials 2-6 |
| `NEACRPA2_3` | axial layers 3-17 | the active core; materials 2-4, 6-11 |
| `NEACRPA2_CRODBANKS` | all layers | which of 7 control-rod banks covers each position, `0` for none |

Material numbers index the 11-material cross-section table built in
`crate::neacrpa2`: 1 axial reflector, 2 radial reflector, 3 radial reflector
re-entrant corner, and 4-11 eight fuel compositions (2.1 to 3.1 w/o, some with
12, 16 or 20 burnable absorber rods).

**Where they came from.** Transcribed byte-for-byte from the
`main_exec_diff3d_standalone` MATLAB snapshot handed over by **Than Yan Ren
(Singapore Nuclear Research and Safety Institute)**, where `neacrpa2.m` reads
them with `readmatrix`. The only change was **stripping the UTF-8 byte-order
mark**. No values were altered.

**Underlying source.** The NEACRP-L-335 (Revision 1) specification, which is
**not** in `crates/kovan-literature`.

**No reference eigenvalue is quoted in the snapshot** for case A2, so
`crate::neacrpa2`'s tests assert structural properties only. But unlike case D1,
**one published number does appear**: `neacrpa2t.m`'s comment records the
official critical boron concentration as **1160.6 ppm** (PANTHER,
NEA/NSC/DOC(93)25 Table 3.1), against the 1139.01 ppm that code computes for
itself. That is the only published NEACRP figure anywhere in the snapshot, and
it is a target for `crate::criticalboron_xyz` when that is translated.

Both values are quoted **from a MATLAB comment**, not from a publication checked
here. Obtain NEA/NSC/DOC(93)25 and catalogue it through `kovan lit import`
before citing either in a paper.

**Licence / access tier.** Open, published reactor-physics literature; these
maps are a numerical restatement of its geometry. No restriction; committable.
No facility-operational, proprietary or partner-confidential data.
