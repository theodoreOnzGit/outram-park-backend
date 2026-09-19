# HTR-10 — control rod geometry, materials, and published worth benchmarks

The control-rod specification and the B3/B4 reactivity-worth benchmark results
for the HTR-10, extracted from **IAEA-TECDOC-1382**. This is the input needed to
model control rod position at all — the existing core model
(`crates/nee_soon/src/htr10_rmc/core_model.rs`) carries the *borings* as a
homogenised reduced-density graphite band (TECDOC zones 31-40) and contains **no
absorber whatsoever**, i.e. it represents rods fully withdrawn and cannot
represent any other position.

**All dimensions are recorded in the units the source uses** (mm for the rod
internals, cm for axial coordinates and channel radius). No conversion has been
applied here, so the numbers stay byte-comparable against the document.

---

## Provenance

| Field | Value |
|---|---|
| Source | IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled reactor performance* |
| Sections | § 4.1.2 (specification), § 4.1.2.4 (problem B4), Tables 4-7, 4-8, 4-9, 4-13, 4-14, 4-15, and the consolidated comparison table |
| Catalogued copy | `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md` |
| Access tier | **Open** — already catalogued under `open/` |
| Processing step | **Read from the catalogued Markdown conversion**, 2026-09-20. No digitisation, no figure reading: every number below is transcribed from body text or a table in that file. |
| Figure NOT used | Fig. 4.9 (*Simplified structure of the HTR-10 control rod*) was **not** digitised; all geometry here comes from the text beside it. |

---

## 1. Rod count and channel

| Quantity | Value |
|---|---|
| Number of control rods in the side reflector | **10** |
| Absorber | **B4C** (boron carbide) |
| Control rod channel diameter | **13 cm** |
| Radial coordinate of channel centre | **102.1 cm** |
| Number of absorber-ball borings (separate system) | 7 |
| Irradiation borings | 3, diameter 130 mm |

The band the existing model already carries, `HTR10_CONTROL_ROD_INNER_CM =
95.6` to `HTR10_CONTROL_ROD_OUTER_CM = 108.6`, is exactly `102.1 ± 13/2` and so
**agrees with this source**.

## 2. Rod internal structure

Radial zone sequence, from the rod axis outward (thicknesses in mm):

| Sequence | Thickness (mm) | Material | Implied radii (mm) |
|---|---|---|---|
| 1 | 27.5 | void | 0 – 27.5 |
| 2 | 2 | stainless steel | 27.5 – 29.5 |
| 3 | 0.5 | void | 29.5 – 30.0 |
| 4 | **22.5** | **B4C** | **30.0 – 52.5** |
| 5 | 0.5 | void | 52.5 – 53.0 |
| 6 | 2 | stainless steel | 53.0 – 55.0 |

This reproduces the separately stated ring dimensions — B4C ring inner/outer
diameter **60 mm / 105 mm**, outer sleeve outer diameter **110 mm** — which is
the internal consistency check that the sequence was read in the right order.

| Quantity | Value |
|---|---|
| **Density of boron carbide in the rod** | **1.7 g/cm³** |
| Steel sleeve density | 7.9 g/cm³ |
| Steel composition (wt%) | Cr 18, Fe 68.1, Ni 10, Si 1, Mn 2, C 0.1, Ti 0.8 |
| Metallic joints / ends | Fe only for 27.5 mm < R < 55 mm, atom density 0.04 barn⁻¹cm⁻¹ |

Axial sequence from lower to upper end (mm), five B4C segments:

```
45(ss) / 487(B4C) / 36(ss) / 487(B4C) / 36(ss) / 487(B4C) / 36(ss) / 487(B4C) / 36(ss) / 487(B4C) / 23(ss)
```

So the absorber is **not** a continuous column: 5 × 487 mm of B4C separated by
36 mm steel joints, total absorber length **2435 mm** over a **2608 mm** span.
A model that smears a single 2608 mm absorber column will over-predict worth.

## 3. Axial travel

| Position | Axial coordinate of rod LOWER END |
|---|---|
| Fully **withdrawn** | **119.2 cm** |
| Fully **inserted** | **394.2 cm** |

Total travel **275 cm**. This is the natural parameterisation for a control-rod
axis in a parameter map: one scalar, the lower-end coordinate, spanning
119.2 → 394.2 cm.

## 4. Published worth benchmarks — the validation targets

Two benchmark variants are reported throughout, and **they are not
interchangeable**: the *original* benchmark and the *deviated* benchmark (the
latter using real dummy balls). Both are recorded here because quoting one
against a model built for the other is a silent 1-2 % Δk error.

### Full core (problem B3)

| Problem | Original VSOP | Original MCNP | Deviated VSOP | Deviated MCNP |
|---|---|---|---|---|
| B31 — ten rods fully inserted | 15.24 % | 16.56 % | 14.46 % | 15.31 % |
| B32 — one rod fully inserted | — | 1.413 % | 1.277 % | 1.343 % |

### Initial core, 126 cm loading height (problem B4)

| Problem | Original VSOP | Original MCNP | Deviated VSOP | Deviated MCNP |
|---|---|---|---|---|
| B41 — ten rods fully inserted | 18.27 % | 19.36 % | 17.23 % | 18.28 % |
| B42 — integral worth of one rod | 1.619 % | 1.793 % | 1.540 % | 1.572 % |

Conditions for B4: **helium atmosphere, core temperature 20 °C** as specified,
though the results section states the calculations were actually performed at
**27 °C** — a discrepancy present in the source, recorded here rather than
silently resolved.

### B42 differential worth curve (VSOP)

Rod lower-end axial positions, as listed in the specification text:

```
394.2, 383.618, 334.918, 331.318, 282.618, 279.018, 230.318   [cm]
```

Worth values, as listed in Table 4-9 (original benchmark) and Table 4-15
(deviated benchmark):

| | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| Original (Table 4-9) | 0.2564 | 0.6103 | 0.6489 | 1.266 | 1.302 | 1.609 | 1.619 |
| Deviated (Table 4-15) | 0.2395 | 0.5765 | 0.6167 | 1.201 | 1.236 | 1.528 | 1.540 |

> **AMBIGUITY — READ BEFORE PAIRING THESE.** The Markdown conversion lost the
> position row of Tables 4-9 and 4-15, so the position↔worth pairing is **not
> directly readable** from the catalogued text. The positions are printed in the
> specification in **descending** order (394.2 first) while the worths are
> printed in **ascending** order (0.2564 first). Taken literally that pairs
> *fully inserted* (394.2 cm) with the *smallest* worth, which is unphysical.
>
> Resolved by two constraints, not by preference:
> 1. Worth must increase monotonically with insertion depth.
> 2. The last value, **1.619 %**, equals the separately reported *integral*
>    worth of one fully inserted rod (Table 4-8), and 1.540 % likewise equals
>    the deviated integral worth (Table 4-14).
>
> Therefore the worth columns run **230.318 → 394.2 cm** (increasing
> insertion), i.e. the reverse of the order the positions are printed in the
> specification text.
>
> **This inference has not been checked against the original PDF's table
> layout.** Before using the differential curve as a quantitative gate, confirm
> the column order against Table 4-9 in the source document. The integral
> values (§ 4 above) carry no such ambiguity and should be preferred as the
> first gate.

---

## 5. What this record does NOT establish

- **No model has been run against any of these numbers.** They are targets, not
  results.
- The homogenised band currently in `core_model.rs` is a *withdrawn-rod* model.
  Worth cannot be computed from it at all until absorber material exists.
- TECDOC's own guidance is that Table 4-3's densities are **spatially
  homogenised**, and "if one is to consider three-dimensional effects, the
  homogenized densities are to be corrected by taking into consideration of the
  boring geometries". A 10-rod azimuthal array is intrinsically 3-D; an
  axisymmetric r-z model smears all ten into an annulus and cannot represent
  the *one-rod* problems (B32, B42) without an explicit correction.
