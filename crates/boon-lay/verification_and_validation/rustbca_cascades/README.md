# RustBCA cascades in an HTR-10 TRISO particle: a cost probe

> **Research, education and V&V only.** Fission-fragment ranges below are
> **illustrative**. They have not been compared with measured data.

This is the record for `examples/rustbca_cascades.rs` (GitHub issue #876, item
7). The full method, with every input and its source, and the predictions
written before the run, are in that file's doc comment. This page lists the
results and the geometry picture.

## What was run

- **Code:** one full binary-collision cascade at a time with boon-lay's copy of
  RustBCA v3.0.0 (`a356280d`). The primary is followed, and so is every recoil
  above its element's cutoff.
- **Geometry:** HTR-10 TRISO layers unrolled into planar layers, using RustBCA's
  `Mesh1D`. The radii come from
  `TrisoSpec::HTR10_LI2014` and the compositions from
  `htr10::fuel_pebble_materials`.
- **Electronic stopping:** `INTERPOLATED`. Nothing was tuned.

### Particles

| particle | energy | where it was born |
|---|---|---|
| light fission fragment, Sr-95 | 100.47 MeV | kernel |
| heavy fission fragment, Xe-139 | 68.66 MeV | kernel |
| B-10(n,α) alpha | 1.472 MeV | kernel and buffer |
| O-16 knock-on | 223 keV | kernel |
| C-12 knock-on | 286 keV | buffer, IPyC and SiC |

All the birth energies are derived from ENDF/B-VIII.0 values.

## The geometry picture

`triso_stack_mesh1d.svg` is drawn from the assembled `Mesh1D`, with one
density lookup every 0.5 µm.

**What was checked:**

- The interfaces recovered from the lookups are at 0, 40, 75, 115, 205 and
  455 µm, which matches `TrisoSpec`.
- The layer order is OPyC, SiC, IPyC, buffer, kernel.
- Every birth point lies inside its intended layer.
- IPyC and OPyC have one composition, so the solver cannot tell them apart.
  They are drawn in one colour.

**What a planar unrolling cannot show:** curvature. Every range here is under
17 µm, against a kernel radius of 250 µm.

## Results, 2026-10-16

**Hardware:** Intel Core i9-13900K (32 logical CPUs, 16 visible to the
process), 62.5 GiB RAM, Arch Linux 7.2.9, rustc 1.98.0, CPU only. One thread
was used. The machine was shared: the load average was 6.5 at the start of the
run. The full run took 220.8 s.

| case | n | range µm (mean ± sem) | s per cascade (mean, sd) | tracked recoils (mean) |
|---|---|---|---|---|
| light fragment, kernel (illustrative) | 16 | 10.69 ± 0.17 | 1.47, 0.37 | 317 562 |
| heavy fragment, kernel (illustrative) | 16 | 7.41 ± 0.12 | 2.00, 0.24 | 454 318 |
| light fragment, 2 µm inside the kernel surface (illustrative) | 16 | 16.8 ± 2.2 | 2.36, 1.4 | 839 512 |
| light fragment, free-flight arm (illustrative) | 16 | 10.59 ± 0.13 | 2.27, 0.68 | 334 531 |
| heavy fragment, free-flight arm (illustrative) | 16 | 7.34 ± 0.09 | 3.09, 0.49 | 456 875 |
| alpha, kernel | 64 | 3.907 ± 0.033 | 0.0283, 0.0024 | 1 452 |
| alpha, buffer | 64 | 9.430 ± 0.012 | 0.0635, 0.0049 | 9 598 |
| O-16 knock-on, kernel | 64 | 0.320 ± 0.009 | 0.0287, 0.0048 | 6 218 |
| C-12 knock-on, buffer | 64 | 1.261 ± 0.017 | 0.0925, 0.017 | 40 968 |
| C-12 knock-on, IPyC | 64 | 0.728 ± 0.009 | 0.109, 0.022 | 54 422 |
| C-12 knock-on, SiC | 64 | 0.570 ± 0.009 | 0.0832, 0.020 | 36 810 |

Of the light fragments born 2 µm inside the kernel surface, 38 % came to rest
outside the kernel. Every case's first cascade was re-run with the same seed
and came back bit-identical.

## Against the predictions

### Ranges

- **Light fragment: missed.** The prediction was 4–9 µm and the result is
  10.7 µm. The predicted reason for a short range, that bare-`Za²` Bethe
  stops the fragment too hard, is **not supported**.
- **Heavy fragment, alpha in the kernel, and the knock-ons:** inside their
  predicted bands.
- **Alpha in the buffer:** 9.4 µm, just above the predicted 5–9 µm.

### Cost

**The cost prediction was too low.** The model tracks 10–100 times more
recoils than predicted. The reason is that upstream's cutoff energies (1 to
3 eV) sit far below the displacement energies.

| particle | cost per cascade |
|---|---|
| fragment | 1.5–2.0 s |
| alpha | 28–64 ms |
| knock-on | 29–109 ms |

### Free-flight option

The prediction was that the free-flight option would be at least 3 times
cheaper. **That is refuted.** It is 1.5 times more expensive, and it gives the
same ranges.

## Not done

- Ranges have not been compared with measured or tabulated data. That is the
  next V&V step, and it should be uncalibrated first.
- The cost of following the primary alone was not measured.
- Nothing has been run in a browser.
