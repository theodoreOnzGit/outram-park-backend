# TRISO-ATOPS: the `atops2026paper` variant against the paper's own release numbers

> **Research, education and V&V only.** This is library verification of the
> `boon-lay` TRISO-ATOPS fork. It is not a source term for any plant, and it is
> AI-drafted: untrusted until reviewed.

GitHub issue #1015. Written 2026-10-22, on `develop` at `39e6252ea` plus the
change that adds the variant.

## The paper

B. Stoyer, A. Raichart and D. Petti, "TRISO-ATOPS: A Mechanistic Source Term
Model for Gas-Cooled Reactors", *Nuclear Technology* (2026), published online
17 August 2026, doi:[10.1080/00295450.2026.2671624](https://doi.org/10.1080/00295450.2026.2671624).
The online-first PDF prints no volume or issue. Page numbers here are the
printed ones (PDF page = printed + 1). This is restricted literature: only the
values a test compares are recorded, each with its table and page. No text,
figure or table is reproduced.

## What was asked, and the rule followed

The maintainer chose (2026-10-22) to add the paper's model as an explicit,
named option, `atops2026paper`, with the upstream code (de374c8) as the
default. The paper option should be "as close to paper as possible, so we can
reproduce the release". Where the paper is ambiguous, the maintainer asked for
an automated search: list the candidate readings first, ranked by the text,
then try them in order against the paper's printed releases and keep every
attempt. The default reading is the one the text supports best. If that
reading disagrees with the paper's printed numbers, this is said plainly and
flagged here.

**Candidates are readings of the paper or of upstream's code, never free
parameters.** Each list below was written before any run. Nothing was tuned.

## 1. Where the paper's equations differ from the upstream code

Read from s.II (pp. 4-13) against the port. Items a-d are the four on #1015.
Items e-j were found by reading the model sections in full.

| # | Item | Paper (page) | Upstream code |
|---|---|---|---|
| a | Booth radius `a`, normal operation | RMS of kernel radius and grain size (pp. 7-8, 26) | `√(2·a_grain·r)` = 65.3 µm |
| b | Booth radius, accident Eq. (19) | the same `a` (pp. 8, 11-12) | kernel radius `r` = 213 µm |
| c | Yttrium | Sr group (Table 1 p. 6; p. 9) | "other" |
| d | Below a law's range | nearest in-range temperature, every law (pp. 8-9) | Rb/Cs, Sr/Ba/Eu, Ag graphite only |
| e | Infinite series, Eqs. (6c) (6d) (8) (18) (19) (21) | to `∞` (pp. 7-8, 10-12) | fixed term counts; 1e-6 transient floor |
| f | "Other" fission metals | Booth on `D = 1e-19 m²/s` (pp. 6-7, 9) | `<R/B>_fail = 1e-5` |
| g | Palladium, accident | silver laws (Table 1; Eq. 18, p. 11) | Booth on an Ag-kernel law |
| h | Long-lived volatiles | Eq. (6c) on Kr's `D` (p. 5, p. 9) | Eq. (6a) at any half-life |
| i | Long-lived birth rate | `t_run time`, Eqs. (4b) (4d) (pp. 4-5) | `t_irrad` |
| j | Accident fuel inventory | `P`, `HPS` × `t_irrad/t_run time`, no branch, Eqs. (22) (23) (28) | unscaled, branch on `F·f − P` |

Not different, so not changed: the breathing / venting trapezoid (Eqs.
(26)-(27)), the `1e8` attenuation cap, Table 4's Eq. (6a) constants, the 20 %
and 4 % half-life screens, the coolant balances (Eqs. (11)-(17)).

## 2. Ambiguities and unstated inputs, with ranked candidates (written before any run)

| Id | Question | Candidates, best-supported first | Text basis |
|---|---|---|---|
| A1 | Booth radius `a` | (1) RMS of radii, 150.8 µm; (2) RMS of diameters, 301.3 µm; (3) upstream's geometric mean, 65.3 µm | (1) is the nomenclature's definition (p. 26) and p. 8's sentence. It is also the only reading inside p. 8's own bound ("between the radius and grain size of the kernel"), and it is "smaller" than the kernel radius, as p. 24 says of it. (2) is p. 7's and p. 24's wording ("kernel diameter"), but 301 µm exceeds the kernel radius. (3) is not a reading of the text |
| A2 | Accident Booth radius | (1) same `a`; (2) kernel radius | Eq. (19) is Eq. (6c) with `D't → ∫D' dt`, `D' = D/a²` (pp. 8, 12) |
| A3 | "Other" metals | (1) Booth on `D = 1e-19`; (2) fixed `1e-5` | p. 9 gives the `D`; pp. 6-7 use Booth for fission metals; `1e-5` appears nowhere in the paper |
| A4 | Long-lived volatiles | (1) Eq. (6c) on Kr's `D`; (2) Eq. (6a) | p. 6 limits Eq. (6a) to the short-lived; p. 5 sends long-lived nuclides to (6c). (2): p. 8 says I and Te behave "the same as xenon measured under irradiation" |
| A5 | Birth-rate time | (1) `t_run time`; (2) `t_irrad` | Eqs. (4b) (4d) print `t_run time` |
| A6 | Accident inventory | (1) normalised, no branch; (2) upstream's | Eqs. (22) (23) (28) and the p. 12 text |
| A7 | Series | (1) converged; (2) truncated | the `∞` limits |
| A8 | Low-temperature laws | (1) every law clamped; (2) upstream's partial set | pp. 8-9 |
| A9 | Yttrium | (1) Sr group; (2) other | Table 1, p. 9 |
| A10 | Silver's accident inventory | (1) all particles; (2) failed particles only (Eq. 23 literally) | p. 5: the coatings are no barrier to silver, so Eq. (5b) applies to every particle |
| A11 | Parent coupling | (1) only short-lived parents; (2) upstream's table flag | p. 10 and p. 11: Eqs. (13), (15) are "for nuclides with a short-lived parent" |
| A12 | Plate-out constant (an input) | (1) as printed, 7.5e-5 /s; (2) 7.5e-4 /s | Table 3 prints 7.5e-5. Upstream's GUI default is 7.5e-4, and #413 found that it reproduces every printed pool. Both are run for both variants |
| A13 | Table 19 run (unstated) | `t_irrad = t_run time` = 3 yr; no HPS; Table 3 geometry | p. 21 gives only "run for 3 years", 1250 °C, Table 17's fractions. No HPS follows from Table 18's Xe-133 having no HPS column. Graphite thickness is immaterial at 1250 °C (no attenuation, Fig. 3) |

## 3. Predictions (written before any run)

Hand estimates from the paper's equations:

- **Consistency.** `Atops2026Paper(PaperReadings::UPSTREAM_CODE)` reproduces
  `Upstream` bit for bit.
- **Table 19** (p. 25, complete failure, 1250 °C, 3 yr). `Upstream` gives
  8/8 within 2 %. Sr-90 Eq. (6c) at 65.3 µm gives 0.098 (paper 9.89e-2), and
  Kr-85 Eq. (6a) gives 0.585. `atops2026paper` gives 6/8. Kr-85 rises to
  about 0.94 (Eq. (6c) at 150.8 µm, +60 %). Sr-90 falls to about 0.044
  (−55 %). The short-lived volatiles and Cs are unchanged.
- **#1015 (a):** a larger radius, smaller release, by `a/65.3 µm = 2.31` on the
  short-time and R/B forms. **(b):** accident kernel release × `r/a = 1.41`.
  **(c):** Y goes from `<R/B> = 1e-5` to Sr-like. **(d):** `D` is higher below
  range, so release is higher.
- **Cases A and B, normal operation.** Short-lived volatile pools are unchanged.
  Long-lived metals fall about 9× from item (i) (Cs-137:
  `(1−e^{−λ·40 yr})/(1−e^{−λ·3 yr}) = 9.0`). Sr-90 graphite also falls
  further, because upstream's 4999-term Booth floor (about 1.2e-4) dominates
  in the cold nodes. "Other" metals' graphite rises about 10³-10⁴× (item f),
  and Y becomes Sr-like. The paper's printed graphite column (40/40 within 2 %
  under upstream) should drop to a handful of hits.
- **Overall.** The paper's printed results were generated by the code, so the
  as-written variant **will not** reproduce them. Every A1-A9 candidate that
  restores agreement will be the upstream-code reading.

## 4. Method of the run

Test: `atops2026paper_against_the_papers_printed_releases` in
`crates/boon-lay/tests/mhtgr_stoyer_workflow.rs`, run on 2026-10-22 in 52 s.
It reuses the #413 harness unchanged. Inputs are Tables 2, 3, 6, 7, 11 and 12
as printed (cited CSVs in `verification_and_validation/mhtgr_stoyer/`). Each
radial section's inventory is spread evenly over the 14 axial nodes (p. 13).
The accident uses the maintainer's Fig. 5 digitisation, resampled by PCHIP to
0.1 h, combined by Eq. (29), with the paper's /10 building reduction. The full
output is `verification_and_validation/mhtgr_stoyer/atops2026paper_reproduction_2026-10-22.txt`.

- **Pools and initial release** count as reproduced within 2 % (the paper's
  three significant figures).
- **Finals** count within 10 %. This allows for the Fig. 5 digitisation;
  upstream's own median is 0.90, as #413 recorded.
- **Table 19**: see the `table19` helper (A13).

Each configuration changes **one** reading from the as-written variant, in the
order of s.2. One combination was also written into the list before the run:
the four code-readings the predictions single out. All of them are run at
both plate-out constants (A12).

## 5. Results: every attempt

Counts are "reproduced / compared". G, C, P and HPS are the graphite,
circulating, plate-out and HPS columns of Tables 9 (A) and 13 (B). "initial"
is Tables 10 and 14's initial column. "final" is the final column, within
10 %, with the median of ours/paper in brackets. T19 is Table 19.

| configuration | k_plate | A: G | A: C | A: P | A: HPS | A: initial | A: final (median) | B: G | B: C | B: P | B: HPS | B: initial | B: final (median) | T19 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **atops2026paper (as written)** | 7.5e-5 | 0/40 | 10/64 | 1/52 | 9/24 | 3/46 | 4/46 (1.22) | 0/40 | 11/64 | 1/52 | 9/24 | 4/46 | 3/46 (1.24) | 6/8 |
| **atops2026paper (as written)** | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.22) | 6/8 |
| A1(2) radius: RMS of diameters | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 2/46 (0.64) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 2/46 (0.64) | 6/8 |
| A1(3) radius: upstream geometric mean | 7.5e-4 | 6/40 | 32/64 | 22/52 | 23/24 | 18/46 | 2/46 (2.62) | 6/40 | 33/64 | 22/52 | 23/24 | 18/46 | 3/46 (2.60) | 7/8 |
| A2(2) accident radius: kernel r | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 17/46 (0.89) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 14/46 (0.89) | 6/8 |
| A3(2) other metals: R/B 1e-5 | 7.5e-4 | 18/40 | 43/64 | 31/52 | 23/24 | 27/46 | 3/46 (1.21) | 18/40 | 43/64 | 31/52 | 23/24 | 27/46 | 5/46 (1.22) | 6/8 |
| A4(2) long-lived volatiles: Eq. (6a) | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.22) | 7/8 |
| A5(2) birth time: t_irrad | 7.5e-4 | 0/40 | 27/64 | 16/52 | 23/24 | 14/46 | 4/46 (1.21) | 0/40 | 27/64 | 16/52 | 23/24 | 14/46 | 7/46 (1.22) | 6/8 |
| A6(2) accident inventory: upstream | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 1/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 6/8 |
| A7(2) series: truncated | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.22) | 6/8 |
| A8(2) clamps: upstream's set | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.22) | 6/8 |
| A9(2) yttrium: other | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.25) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.24) | 6/8 |
| A10(2) silver accident: failed only | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 1/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.22) | 6/8 |
| A11(2) parents: upstream table flag | 7.5e-4 | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 3/46 (1.21) | 0/40 | 26/64 | 15/52 | 23/24 | 13/46 | 5/46 (1.22) | 6/8 |
| A1(3)+A3(2)+A4(2)+A5(2) | 7.5e-4 | 31/40 | 61/64 | 48/52 | 24/24 | 43/46 | 2/46 (2.62) | 31/40 | 61/64 | 48/52 | 24/24 | 43/46 | 2/46 (2.60) | 8/8 |
| **upstream (de374c8)** | 7.5e-5 | 40/40 | 11/64 | 21/52 | 10/24 | 16/46 | 24/46 (0.90) | 40/40 | 11/64 | 21/52 | 10/24 | 9/46 | 28/46 (0.90) | 8/8 |
| **upstream (de374c8)** | 7.5e-4 | 40/40 | 64/64 | 52/52 | 24/24 | 46/46 | 24/46 (0.90) | 40/40 | 64/64 | 52/52 | 24/24 | 46/46 | 23/46 (0.90) | 8/8 |

Rows at the printed `k_plate = 7.5e-5` for every one-at-a-time configuration
are in the run record. They move the same way, with fewer C/P/HPS hits in
every configuration.

### Table 19 (p. 25), complete failure at 1250 °C

| nuclide | paper | upstream | rel. diff | atops2026paper | rel. diff |
|---|---|---|---|---|---|
| Xe-133 | 2.23e-2 | 2.232e-2 | +0.1 % | 2.232e-2 | +0.1 % |
| Kr-85 | 5.85e-1 | 5.855e-1 | +0.1 % | 9.525e-1 | **+62.8 %** |
| Kr-88 | 1.98e-2 | 1.984e-2 | +0.2 % | 1.984e-2 | +0.2 % |
| I-131 | 2.54e-2 | 2.538e-2 | −0.1 % | 2.538e-2 | −0.1 % |
| I-133 | 1.30e-2 | 1.296e-2 | −0.3 % | 1.296e-2 | −0.3 % |
| Cs-134 | 1.00 | 1.000 | 0.0 % | 1.000 | 0.0 % |
| Cs-137 | 1.00 | 1.000 | 0.0 % | 1.000 | 0.0 % |
| Sr-90 | 9.89e-2 | 9.882e-2 | −0.1 % | 4.344e-2 | **−56.1 %** |

### How much each ambiguity moves each release number

Case A, `k_plate = 7.5e-4`. Each cell is ours/paper as initial / final
release. 1 is a perfect match.

| configuration | Kr-85 | Xe-133 | I-131 | Te-132 | Sr-90 | Y-91 | Cs-137 | Ag-110m | Zr-95 | Ce-144 |
|---|---|---|---|---|---|---|---|---|---|---|
| atops2026paper (as written) | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A1(2) RMS of diameters | 0.525 / 0.640 | 1.000 / 0.639 | 1.000 / 0.642 | 1.000 / 0.641 | 0.024 / 0.570 | 391.2 / 0.412 | 0.041 / 0.230 | 0.951 / 0.903 | 884 / 0.636 | 3182 / 0.632 |
| A1(3) geometric mean | 2.208 / 2.431 | 1.000 / 2.435 | 1.000 / 2.427 | 1.000 / 2.429 | 0.112 / 2.601 | 1806 / 1.879 | 0.111 / 0.650 | 0.951 / 0.903 | 4037 / 2.890 | 14203 / 2.774 |
| A2(2) accident radius r | 1.023 / 0.886 | 1.000 / 0.885 | 1.000 / 0.886 | 1.000 / 0.885 | 0.048 / 0.805 | 781.8 / 0.581 | 0.072 / 0.317 | 0.951 / 0.903 | 1761 / 0.896 | 6300 / 0.882 |
| A3(2) other R/B 1e-5 | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / 0.903 | 0.997 / 1.275 | 0.929 / 1.281 |
| A4(2) volatiles Eq. (6a) | 0.190 / 1.219 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A5(2) birth t_irrad | 5.373 / 1.197 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.431 / 1.136 | 781.8 / 0.820 | 0.651 / 0.786 | 0.998 / 0.904 | 1761 / 1.265 | 6772 / 1.244 |
| A6(2) accident inventory | 1.023 / 1.156 | 1.000 / 1.213 | 1.000 / 1.212 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.381 | 0.951 / 0.861 | 1761 / 1.265 | 6300 / 1.246 |
| A7(2) series truncated | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.058 / 1.166 | 781.8 / 0.842 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A8(2) upstream clamps | 0.714 / 1.216 | 1.000 / 1.213 | 1.000 / 1.212 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A9(2) Y other | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 2.598 / 1.273 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A10(2) Ag failed only | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / **0.018** | 1761 / 1.265 | 6300 / 1.246 |
| A11(2) parent flag | 1.023 / 1.216 | 1.000 / 1.213 | 1.000 / 1.213 | 1.000 / 1.212 | 0.048 / 1.135 | 781.8 / 0.820 | 0.072 / 0.394 | 0.951 / 0.903 | 1761 / 1.265 | 6300 / 1.246 |
| A1(3)+A3(2)+A4(2)+A5(2) | 0.996 / 2.442 | 1.000 / 2.435 | 1.000 / 2.427 | 1.000 / 2.429 | 0.995 / 2.603 | 1806 / 1.879 | 1.003 / 1.223 | 0.998 / 0.904 | 0.997 / 2.941 | 0.999 / 2.955 |
| upstream | 0.996 / 0.843 | 1.000 / 0.884 | 1.000 / 0.885 | 1.000 / 0.885 | 1.002 / 0.844 | 1.003 / 0.908 | 1.003 / 0.943 | 0.998 / 0.862 | 0.997 / 0.903 | 0.999 / 0.907 |

## 6. Predictions against results

| Prediction (s.3) | Result | |
|---|---|---|
| `Atops2026Paper(UPSTREAM_CODE)` = `Upstream`, bit for bit | exact, both cases, pools and sparse-Fig. 5 accident | hit |
| Table 19 upstream 8/8 | 8/8, worst −0.3 % (I-133) | hit |
| Table 19 paper 6/8; Kr-85 ≈ 0.94 (+60 %), Sr-90 ≈ 0.044 (−55 %) | 6/8; Kr-85 0.9525 (+62.8 %), Sr-90 0.04344 (−56.1 %) | hit |
| (a) release ÷ 2.31 | ÷ 2.29 (Eq. 6c), ÷ 2.25 (Eq. 6b; μ finite) | hit, within 3 % |
| (b) accident × 1.41 | × 1.410 | hit |
| (c) Y from 1e-5 to Sr-like | Y-91 initial release 782× the paper's (printed from 1e-5) | hit |
| (d) `D` higher below range | Kr at 400 °C: × 1.03e3 (predicted ≈ 1.0e3) | hit |
| long-lived metals ≈ ÷ 9 from (i) | Cs-137: A5 alone moves 0.072 → 0.651, × 9.04 | hit |
| "other" metals' graphite × 10³-10⁴ | Zr-95 × 1761, Ce-144 × 6300 | hit |
| graphite column drops to a handful | 0/40 in both cases | hit (stronger than predicted) |
| as-written variant does not reproduce the paper | it does not; upstream does | hit |

## 7. Decisions: which reading `atops2026paper` uses, and why

The rule set before the run: default to the reading the text supports best.
If that reading disagrees with the paper's printed numbers, keep it, and flag
the disagreement.

| Id | Text-best (default) | Best against the printed numbers | Agree? |
|---|---|---|---|
| A1 radius | RMS of radii, 150.8 µm | geometric mean (G 6/40, T19 7/8). Only in combination with A3-A5 does normal operation recover (G 31/40, C 61/64) | **no: flagged** |
| A2 accident radius | same `a` | kernel `r` (finals 17/46, median 0.89, against 4/46, 1.22) | **no: flagged** |
| A3 other metals | Booth on 1e-19 | fixed 1e-5 (G 18/40, C 43/64) | **no: flagged** |
| A4 long-lived volatiles | Eq. (6c) | Eq. (6a) (T19 7/8, Kr-85 = 0.585) | **no: flagged** |
| A5 birth time | `t_run time` | `t_irrad` (Cs-137 × 9.04 toward the paper) | **no: flagged** |
| A6 accident inventory | Eqs. (22)/(23) as printed | as printed (finals 3/46, against 1/46 for upstream's form) | yes |
| A7 series | converged | no count moves (Sr-90 initial 0.048 → 0.058) | yes (no evidence either way) |
| A8 clamps | every law | no count moves | yes (no evidence either way) |
| A9 yttrium | Sr group | "other" (Y-91 782 → 2.6, still not reproduced) | **no: flagged** |
| A10 silver | all particles | all particles (failed-only collapses Ag finals to 0.018) | yes |
| A11 parents | short-lived only | no number moves | yes |
| A12 `k_plate` | (input) | 7.5e-4 in every configuration, confirming #413's misprint finding | input, not variant |

**What is still unexplained.** No candidate on the list reproduces the paper.
Only the full set of code readings does, and that is `Upstream`. The
combination of A1(3), A3(2), A4(2) and A5(2) brings normal operation within
reach (43/46 initial releases). Its remaining 9/40 graphite misses are the Y
and series items. But its finals are 2.6× high, because the paper's accident
Booth uses that same `a`, which is now 65 µm. The finals agree only with
upstream's split: 65.3 µm in normal operation, the kernel radius in an
accident. The paper's text gives no reading that produces that split. Per the
maintainer's instruction, the search stopped at the end of the pre-written
list.

## 8. Conclusion

**The paper's printed release numbers were computed by the upstream code, not
by the equations as the paper writes them.** Every printed table that can be
checked shows this:

- Table 19 (Kr-85 = Eq. (6a); Sr-90 = the 65.3 µm radius);
- Tables 9 and 13 (the `1e-5` "other" metals, yttrium as "other", `t_irrad`
  births);
- Tables 10 and 14 (the kernel radius in the accident).

`atops2026paper` therefore implements the paper's *equations*, and cannot also
reproduce the paper's *numbers*. The two goals in the brief are in conflict.
`Upstream` is what reproduces the printed release, and it is the default.

**For the maintainer and the authors:** A1 (p. 7 says diameter, pp. 8 and 26
say radius, the code uses neither), A2, A3, A4, A5 and A9 are places where
the paper and its own code disagree. Each one moves a printed number, by the
amounts in s.5.
