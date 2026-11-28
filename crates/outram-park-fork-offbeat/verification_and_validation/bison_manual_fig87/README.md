# MATPRO UO2 swelling against BISON Theory Manual Fig. 8.7

> **Research, education and V&V only.** AI-written, 2026-11-28, not
> reviewed by a human. This is verification of a transcription, not
> validation.

**Methodology and results:** in the doc comment of
`tests/bison_manual_fig87_matpro_swelling.rs`.

**Source.** Hales et al. (2013), *BISON Theory Manual*, INL/EXT-13-29930,
Fig. 8.7, p. 22 (PDF page 24). In the maintainer's private corpus
(`propreitrary-literature-theodore`,
`reports/inl-ext-13-29930-bison-theory-manual-2013.pdf`). It has no reuse
licence, so the figure and its render are **not** committed here. Only the
digitised points (in the test) and the digitiser are committed.

**Digitising.** Run `digitise_fig87.py` on a 400 dpi render:

```bash
pdftoppm -r 400 -png -f 24 -l 24 <manual.pdf> vhi
python3 -I digitise_fig87.py vhi-24.png
```

- Each axis was calibrated from tick marks detected in the render. The x axis
  is 192.75 px per 500 K (300 K at the left axis). The y axis is 217.6 px per
  0.05, with 0 at row 2858.
- The red "10 − Gaseous" curve is the lower red trace in each column.
- Columns within 45 K of the curve markers (300, 900, 1500, 2100 and 2700 K)
  are excluded, as is the legend box.
- One pixel is 2.3e-4 in ΔV/V.

**Result in one line:** the curve's shape (gaseous swelling relative to its
peak) agrees with `SwellingModel::Uo2Matpro` to within 0.0027 at five
temperatures from 1597 to 2399 K. The magnitudes are not checked, because the
figure's density and burnup basis are not stated.
