# MHTGR end-to-end workflow check against Stoyer et al. (GitHub #413)

**Research, education and V&V only.** This is library verification of the
`boon-lay` TRISO-ATOPS fork. It is not a source term for any plant.

## Source

B. Stoyer, A. Raichart, et al., *TRISO-ATOPS: A Mechanistic Source Term Model
for Gas-Cooled Reactors*, Nuclear Technology (2026), section III.

- **Access tier:** proprietary. The values the check needs are committed here
  as a **cited table**, with the table and page of every value. **The PDF is
  not committed.**
- **Pages:** `pdf_page` = printed page + 1.

## Files

| File | Paper | Pages (printed / pdf) | Content |
|---|---|---|---|
| `constants.csv` | Tables 2, 3, 12; s.III.B.1 | 7 / 8; 21 / 22; 19 / 20 | failure fractions and constants, Case A and Case B |
| `table06_case_a_inventory_ci.csv` | Table 6 | 14-15 / 15-16 | 350 MWth inventories [Ci] per radial section |
| `table07_core_temperature_k.csv` | Table 7 | 15 / 16 | 14 x 3 core temperatures [K] |
| `table09_case_a_normal_operation_ci.csv` | Table 9 | 17-18 / 18-19 | Case A graphite / circulating / plate-out / HPS [Ci] |
| `table10_case_a_accident_release_ci.csv` | Table 10 | 18 / 19 | Case A initial and final releases [Ci], final **as printed** (the paper's x10 building reduction already applied) |
| `table11_case_b_inventory_ci.csv` | Table 11 | 20-21 / 21-22 | 600 MWth inventories |
| `table13_case_b_normal_operation_ci.csv` | Table 13 | 22-23 / 23-24 | Case B pools |
| `table14_case_b_accident_release_ci.csv` | Table 14 | 23 / 24 | Case B releases, final as printed |
| `upstream_case_{a,b}.csv` | computed | -- | upstream TRISO-ATOPS at de374c8 on these inputs, unrounded |
| `upstream_case_{a,b}_diagnostic_kplate_7p5e-4.csv` | computed | -- | the same with k_plate = 7.5e-4 1/s (diagnostic, see below) |

## Extraction

The tables were extracted with `pdftotext -raw` (poppler), one page at a time,
and parsed by a deterministic regex: a mass-number line followed by a line of
symbol and values. **No table value was typed by hand**, except
`constants.csv`, which was transcribed from the same text layer. Rows were
spot-checked against the rendered pages 16 and 24. Row counts are 64 per
inventory/pool table and 46 per release table.

## Reproduce

```bash
python3 crates/boon-lay/dev/mhtgr_stoyer_upstream.py   # upstream, needs numpy
cargo test --release -p boon-lay --test mhtgr_stoyer_workflow -- --nocapture
```

## Findings (2026-09-29)

**Port vs upstream.** The worst relative difference is **5.6e-12** across every
pool of every nuclide, in both cases.

**Upstream (and the port) vs the paper, with Table 3 as printed.** Here
`k_plate = 7.50E-05 /s`.
- Graphite agrees for 40/40 nuclides.
- Only 11/64 circulating values agree: long-lived metals run ~10x high.
- Only 10/24 HPS values agree: halogens run ~5x high.

**With `k_plate = 7.5e-4 /s`.** This is upstream's own GUI and manual default.
Every printed value is reproduced within 2 %, in both cases:
- graphite 40/40;
- circulating 64/64;
- plate-out 52/52;
- HPS 24/24;
- initial release 46/46.

On this evidence, Table 3's `7.50E-05` is a misprint for `7.50E-04`. The
committed inputs keep the value as printed, and the 7.5e-4 run is labelled a
diagnostic.

**Not yet done: the final (heat-up) releases.** They need the four accident
temperature curves of **Fig. 5** (printed p. 15, pdf p. 16). Those exist only
as a plot and must be digitised first; this was requested on #413.

The paper reduces its final releases "by an order of magnitude" for
10 %/day building leakage (s.III.A.5). That post-processing will be
replicated as stated, and flagged.
