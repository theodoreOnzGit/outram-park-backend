# HTR-10 — R-Z zone geometry of the simplified benchmark model

The axisymmetric (r-z) zone map of the HTR-10 initial-criticality benchmark, as
drawn in **Figure 2** of Terry et al. (2005). This is the geometry `op-tvmf` was
opened to obtain, and the input `op-lhu6` needs to mesh the core.

**All dimensions are in centimetres**, recorded exactly as read. No conversion
has been applied; conversion to `uom` types happens at the code boundary, so
these numbers stay byte-comparable against the figure for anyone re-checking.

The model is **radially symmetric** — an r-z partition, not a 3-D one. The
borings (control rods, coolant channels, KLAK) are *homogenised* into the rings
where they sit; they are not resolved as discrete holes.

---

## Provenance

| Field | Value |
|---|---|
| Source | Terry, W. K.; Kim, S. S.; Montierth, L. M.; Cogliati, J. J.; Ougouag, A. M. (2005) |
| Title | *Evaluation of the HTR-10 Reactor as a Benchmark for Physics Code QA* |
| Report | INL/CON-05-00852 **(PREPRINT)** |
| Figure | **Fig. 2**, "Simplified HTR-10 benchmark model" |
| Obtained from | <https://www.osti.gov/servlets/purl/911178> (accessed 2026-08-13) |
| Access tier | **Proprietary** — see `crates/kovan-literature/CATALOGUE.md` |
| **Processing step** | **Hand-read from the figure by the maintainer**, 2026-08-13 |
| Units | centimetres, as authored |

**On the processing step — read this before trusting the numbers.** These were
**transcribed by eye by the maintainer**, not produced by `kovan-digitise`.
That distinction matters and must not be blurred:

- There is **no calibration record and no stated uncertainty**, which a
  digitiser run would have produced automatically.
- It is nonetheless the *human* path, not the agent path. The workspace rule
  forbidding eye-reading binds **AI assistants**; a maintainer reading their own
  source is a different act, and only a human may attest to a dataset anyway.
- The figure is **raster** (5 `/Subtype /Image`, all `FlateDecode`, no vector
  operators), so no exact extraction was possible — the coordinates are simply
  not in the PDF content stream.

**Why the numbers may be recorded although the document may not be
redistributed:** facts are not copyrightable. Terry §1 further states that all
descriptive data "were obtained from published documents, mainly two IAEA
TECDOC reports", both of which this workspace holds in the **open** tier — see
the corroboration below, which establishes an open provenance path for most of
this partition.

**Citation caution.** The preprint's first page says it "should not be cited or
reproduced without permission of the author". For publication, cite the IAEA
TECDOCs or the IRPhEP handbook (NEA/NSC/DOC(2006)1), not this preprint.

---

## Radial partition

**Ten boundaries**, giving ten radial bands (the innermost runs from the axis).
As read, ascending:

```
25.0  41.75  70.75  90.0  95.6  108.6  140.6  148.6  167.793  190.0
```

| # | r_inner | r_outer | Material |
|--:|--:|--:|---|
| 10 | 167.793 | **190.0** | Boronated carbon bricks (outermost) |
| 9 | 148.6 | 167.793 | Graphite reflector |
| 8 | **140.6** | **148.6** | **Cold coolant flow region** |
| 7 | 108.6 | 140.6 | Reflector |
| 6 | **95.6** | **108.6** | **Graphite reflector with control-rod borings** |
| 5 | 90.0 | 95.6 | Reflector |
| 4 | 70.75 | **90.0** | Core — outer radial subdivision |
| 3 | 41.75 | 70.75 | Core — middle radial subdivision |
| 2 | **25.0** | 41.75 | Core — inner radial subdivision |
| 1 | 0 | **25.0** | Innermost — discharge-tube radius |

**Material assignment above r = 90 was stated by the maintainer.** The four
bands **below** r = 90 are subdivisions of the core region whose individual
materials/zones are **not yet recorded** — the labels in rows 1–4 are
placeholders describing position, not attested material identity. Do not treat
them as zone assignments until confirmed.

### Corroboration — six of ten boundaries are independently reproduced

Checked 2026-08-13 against **Table 2** of the same paper, which is textual and
therefore independent of the figure:

| Boundary | Independent derivation | Agrees |
|--:|---|:--:|
| 190.0 | reflector outer diameter 380 / 2 | ✓ |
| 148.6 | coolant channel r = 144.6 **+** diameter 8.0 / 2 | ✓ |
| 140.6 | coolant channel r = 144.6 **−** diameter 8.0 / 2 | ✓ |
| 108.6 | control-rod channel r = 102.1 **+** diameter 13 / 2 | ✓ |
| 95.6 | control-rod channel r = 102.1 **−** diameter 13 / 2 | ✓ |
| 90.0 | core radius | ✓ |
| **25.0** | **fuel discharge tube radius** | ✓ |
| 167.793 | *not derivable* — boronated-brick / graphite **material** interface | n/a |
| 41.75 | *not derivable from Table 2* — see below | — |
| 70.75 | *not derivable from Table 2* — see below | — |

The channel annuli are homogenised into rings whose edges are exactly
`r_centre ± d/2`, and the stated material sequence lands on exactly those rings.

### Working hypothesis for 41.75 and 70.75 — NOT confirmed

These two sit strictly between the discharge-tube radius (25.0) and the core
radius (90.0), which is exactly the radial span the **conus** covers. Terry §3
records that for the TWODANT and PEBBED models "the additional simplification
was made of representing the sloping surface of the conus by **stair-steps**".
Four radial values across that span (25.0, 41.75, 70.75, 90.0) is consistent
with a three-step staircase.

The steps are **not uniform** — 90.0→70.75 is 19.25, 70.75→41.75 is 29.0,
41.75→25.0 is 16.75 — so if this is the staircase, the steps were chosen to
follow the cone rather than to divide the radius evenly. The axial data will
confirm or refute this: a stair-stepped conus requires matching z-levels at each
radial step. **Treat as a hypothesis until the axial partition is in.**

### Corroboration — five of seven boundaries are independently reproduced

Checked 2026-08-13 against **Table 2** of the same paper, which is textual and
therefore independent of the figure:

| Boundary | Independent derivation | Agrees |
|--:|---|:--:|
| 190.0 | reflector outer diameter 380 / 2 | ✓ |
| 148.6 | coolant channel r = 144.6 **+** diameter 8.0 / 2 | ✓ |
| 140.6 | coolant channel r = 144.6 **−** diameter 8.0 / 2 | ✓ |
| 108.6 | control-rod channel r = 102.1 **+** diameter 13 / 2 | ✓ |
| 95.6 | control-rod channel r = 102.1 **−** diameter 13 / 2 | ✓ |
| 90.0 | core radius | ✓ |
| 167.793 | *not derivable* — it is the boronated-brick / graphite **material** interface, so no channel geometry generates it | n/a |

The channel annuli are homogenised into rings whose edges are exactly
`r_centre ± d/2`, and the material sequence lands on exactly those rings. Every
boundary is now accounted for by either arithmetic or a material change.

### This partition has an open provenance path

`op-tvmf` recorded that IAEA-TECDOC-1382 part 2's FIG 4.20 survives OCR only as
a bare radial list: **90, 95.6, 108.6, 167.793, 190**. All five are confirmed
above. The two the OCR list lacked — **140.6** and **148.6** — are recoverable
from Table 2 arithmetic. So the full radial partition can be established from
open sources, and does not depend on the preprint.

---

## Axial partition

**Seventeen boundaries**, giving sixteen axial bands. As read, descending:

```
610.0  540.0  510.0  495.0  465.0  450.0  430.0  402.0  388.764
351.818  228.758  130.0  114.7  105.0  95.0  40.0  0.0
```

### Orientation: z increases DOWNWARD

This is not an assumption — it is forced by the arithmetic below. `z = 0` is the
top of the model and `z = 610` the bottom, consistent with the 610 cm reflector
height spanning the full extent. IAEA-TECDOC-1382 part 2 states that
`z = 351.818` is **zero core height**, i.e. the top of the conus; the pebble bed
then extends *upward* (decreasing z) and the conus *downward* (increasing z).

### Corroboration — three Table 2 heights reproduced exactly

Checked 2026-08-13. Each is a **difference of two independently-read
boundaries** matching a textual value from the same paper:

| Span | Arithmetic | Table 2 quantity | Agrees |
|---|---|---|:--:|
| Core cavity | 351.818 − 130.0 = **221.818** | Height of core cavity 221.818 | ✓ |
| Conus | 388.764 − 351.818 = **36.946** | Height of conus 36.946 | ✓ |
| Pebble bed at criticality | 351.818 − 228.758 = **123.06** | Core height 123.06 | ✓ |
| Full model extent | 610.0 − 0.0 = **610** | Reflector / discharge-tube height 610 | ✓ |

So the axial partition is anchored at four places by text that is independent of
the figure, and the three interior spans agree to the last decimal. Combined
with the six radial boundaries reproduced from channel arithmetic, the
hand-reading is corroborated at ten independent points.

Derived positions that follow:

- **Core cavity:** z ∈ [130.0, 351.818]
- **Conus:** z ∈ [351.818, 388.764]
- **Pebble bed, critical loading:** z ∈ [228.758, 351.818]
- **Cavity above the bed (void/gas space):** z ∈ [130.0, 228.758], 98.758 cm

### One unreconciled value — for the maintainer to check

IAEA-TECDOC-1382 part 2's FIG 4.20 axial OCR list is
**105, 114.7, 130, 171.698, 351.818, 388.764, 402, 430**. Seven of its eight
values appear in the reading above — **105, 114.7, 130, 351.818, 388.764, 402,
430** all match. The eighth, **171.698**, does **not** appear; the reading has
**228.758** in that region instead.

This is **not necessarily an error**. `op-tvmf` records explicitly that the
pairing of those OCR'd numbers to specific zone boundaries "is NOT recoverable
and must not be treated as authoritative", and FIG 4.20 belongs to a different
contributor's model in the TECDOC, not to Terry's Fig. 2. But 228.758 is the
value that reproduces the 123.06 cm critical core height exactly, so if only one
of the two is right, the evidence favours 228.758. **Worth a second look at the
figure in that region.**

### Remaining axial anchors not yet placed

From Table 2, still to be matched against band boundaries once zone identities
are recorded:

| Quantity | Value |
|---|--:|
| Height of cold coolant flow channels | 405 cm |
| Height of control-rod / irradiation channels | 450 cm |

Note **450** appears in the reading; **405** does not (the reading has 402).
Whether the coolant-channel *extent* is meant to coincide with a zone boundary
is unconfirmed — do not force it.

The VTB Griffin deck gives an outer bound only: bottom `z < 41`, top `z > 490`.
The reading's 40.0 and 495.0 sit either side of those, consistent.

### Grid shape

Ten radial bands × sixteen axial bands = 160 potential cells, against **81**
subvolumes read from the figure. So roughly half the tensor-product cells are
merged — the zone map is emphatically **not** a full tensor grid, and the
subvolume list cannot be reconstructed from the two boundary lists alone. The
zone-to-band assignment must be recorded explicitly.

---

## Zone count

The maintainer reads **81 subvolumes** from Fig. 2. IAEA-TECDOC-1382 part 2's
Table 4-3 lists **83 zones** with carbon and boron atom densities. The
difference of two is **not yet reconciled** — it may be that the figure
homogenises where the table does not, or that two table zones fall outside the
figure's extent. Do not assume a 1:1 mapping between the two until this is
resolved.

Seven radial bands do not divide 81, so the map is **not a plain tensor grid** —
some axial levels must subdivide radially differently from others. The axial
data will show where.

## Status

**Not validated.** Per the workspace V&V rule, nothing here may be described as
validated until the maintainer has personally reviewed it, and no transport or
mesh calculation has yet consumed it. The radial half is corroborated against an
independent table in the same paper (above); the axial half is not yet recorded.
