# Windowed multipole at the local temperature, against data processed exactly at that temperature (GitHub #948)

**Issue:** [#948](https://github.com/theodoreOnzGit/outram-park-backend/issues/948),
step 2 (method B) of [#945](https://github.com/theodoreOnzGit/outram-park-backend/issues/945)
(epic #925). Step 1 is #947
(`../temperature_interpolation_947/README.md`), whose instrument this study
reuses so the two are comparable.

**Status:** upstream reading, the probe of the data (§2) and the
predictions (§4) were written 2026-10-18 **before any run at T > 0**
(commit `7b6a33747e`). Two methodology amendments made after the first
look, and why, are in §5.0. Results are in §5 onwards; the decision
asked of the maintainer is §7.

**This is a measurement and a wiring, not a default change** unless §4.4's
pre-registered criterion is met (it is stated before the runs).

## 1. What upstream does (read first)

OpenMC `d7d3284a1` (`/home/teddy0/Documents/research/openmc`).

**Transport (`src/nuclide.cpp:620-680`).** With multipole data loaded
(`settings::temperature_multipole`, **off** by default upstream,
`src/settings.cpp:81`) and `E_min <= E <= E_max`, `calculate_xs` calls
`multipole_->evaluate(E, sqrtkT)` at the **particle's own** `sqrtkT` and sets
`total = s + a`, `elastic = s`, `absorption = a`, `fission = f`,
`nu_fission = f * nu(E)`. Nothing else: inelastic is implicitly zero inside
the range ("assumes that inelastic scattering does not occur in the resolved
resonance region", `docs/source/methods/cross_sections.rst:140-142`).
`index_temp = -1`, so:

- **URR tables are skipped** inside the range (`nuclide.cpp:826`,
  `&& !use_mp`);
- **S(α,β) still applies** on top (`calculate_sab_xs` runs after, line 823);
- **elastic kinematics use the local kT whenever the nuclide has multipole
  data, at any energy** (`src/physics.cpp:734`,
  `kT = nuc->multipole_ ? p.sqrtkT()^2 : nuc->kTs_[i_temp]`);
- **fission samples the first fission reaction** inside the range
  (`physics.cpp:598-603`);
- depletion (n,γ) is `σ_a − σ_f` (`nuclide.cpp:640-648`).

Outside `[E_min, E_max]` the tables are used. In upstream's *Multipole*
temperature method the tables are loaded at a single temperature
(`cross_sections.rst:176-179`); with interpolation they bracket it.

**The evaluator (`src/wmp.cpp:73-165`).** Window
`i = min(n_w - 1, (√E − √E_min)·inv_spacing)`; the curve fit
`Σ_k c_k E^{(k-2)/2}` (`fit_order + 1` terms), Doppler-broadened by
`broaden_wmp_polynomials` (Josey et al. 2016, Eq. 16) **only where the
window's `broaden_poly` flag is set**, else evaluated raw; plus the window's
poles, each `Re[r · √π · dopp/E · W(z)]`, `z = (√E − p)·dopp`,
`dopp = √(A/kT)`. At 0 K the asymptotic pole form.

**The approximations the form makes, from upstream's own documentation**
(`docs/source/methods/cross_sections.rst:90-138`): the exact free-gas
broadening of a pole has a second term, the `C` integral; it is dropped
"for `E ≫ k_B T/A`". "This polynomial fit is then broadened exactly. This
exact broadening can make up for the removal of the C integral, as typically
at low energies, only curve fits are used."

**The temperature range the curve fit is valid over** — the third limit
recorded on #945 — is set by the generator, not the format. The format
(`docs/source/io_formats/data_wmp.rst`) stores no temperature range.
Upstream's generator (`openmc/data/multipole.py`) fits each window's curve
fit over the window **extended by ±4 Doppler widths at
`TEMPERATURE_LIMIT = 3000 K`** (lines 34, 580, 621-626), adding poles until
the fit meets `rtol = 1e-3`, `atol = 1e-5` there (lines 663-675), and it sets
`broaden_poly = True` on every window (line 728). So a library made by that
generator is designed for 0–3000 K.

**The CORE library is not from that generator version**: its U-238 has
`broaden_poly` on 115 of 4309 windows (§2), not all. Its own design range is
not stated anywhere this workspace holds: the MIT `WMP_Library` checkout
the bake reads (`/home/teddy0/Documents/research/WMP_Library`) is not on
this machine, and the WMP papers (Hwang 1987; Josey, Ducru, Forget, Smith,
*J. Comput. Phys.* 307 (2016) 715) are **not in the corpus**
(`crates/kovan-literature/reactor-literature/`, 137 PDFs searched for
"multipole"; only one unrelated hit). Nothing here is cited from memory;
the range is **measured** (§5) at 293.6–2500 K.

### 1.1 Ours against upstream, before this change

| # | upstream | ours (before #948) | effect |
|---|---|---|---|
| 1 | `WindowedMultipole::evaluate` | `njoy_outram_park_fork::wmp::WindowedMultipole::evaluate`: same window rule, same curve fit and `broaden_poly` branch, same `fit_order + 1` terms, same 0 K branch | none in range. Out of range (`E < E_min`) ours clamps to window 0 where upstream's `size_t` cast wraps to the last window; unreachable, both only evaluate in range |
| 2 | `faddeeva` = Johnson's Faddeeva package | Weideman rational approximation, N = 48 | agrees with OpenMC reading our written `.h5` to 1.2e-12 (`njoy-outram-park-fork/tests/wmp_h5_vs_openmc.rs`) |
| 3 | `std::erf` | `erf` via the same Weideman `w` | as 2 |
| 4 | `evaluate(E, sqrtkT)` | `evaluate(E, T_kelvin)`, `sqrtkT = √(8.617333262e-5·T)` | same constant (`include/openmc/constants.h:116`) |
| 5 | **any** nuclide with WMP data uses it in `[E_min, E_max]` at the particle's T | **only the LOW tier** (`Nuclide::from_core`, WMP + fast MGXS). A pointwise (ENDF/ACE) nuclide **never** used WMP: its resolved range stayed at its build temperature whatever the material's | **the gap #948 closes** |
| 6 | above `E_max`: pointwise tables | LOW tier: fast multigroup (not upstream; by design of that tier) | unchanged here |
| 7 | URR skipped inside the range | n/a (the LOW tier has no URR) | needed for a pointwise nuclide with WMP |
| 8 | elastic kinematics at local kT for a multipole nuclide, all energies | LOW tier already local; pointwise: data kT | needed |
| 9 | fission: first fission reaction | pointwise: the χ mixture weights read the partial-fission tables | difference kept, documented (§6) |
| 10 | depletion (n,γ) = σ_a − σ_f | `reaction_xs(mt, e)` reads the table | difference kept, documented (§6) |
| 11 | `evaluate_deriv` (tally derivatives) | not ported | out of scope |

## 2. Probe of the data (run before the predictions)

`examples/wmp_local_temperature_948.rs probe`. CORE library
(`njoy-outram-park-fork/src/data/wmp_core.wmpl`, MIT `WMP_Library`,
documented as ENDF/B-VII.1 in `njoy-outram-park-fork/docs/wmp-nuclide-manifest.md`).

| nuclide | E range (eV) | windows (broaden_poly) | poles | fit_order | window √E spacing | Doppler width √(kT/A) at 300 / 2500 K |
|---|---|---|---|---|---|---|
| U-238 | 1e-5 – 2.0e4 | 4309 (115) | 4062 | 2 | 3.28e-2 | 1.05e-2 / 3.02e-2 |
| U-235 | 1e-5 – 2.25e3 | 2250 (40) | 5254 | 3 | 2.11e-2 | 1.05e-2 / 3.04e-2 |
| Pu-239 | 1e-5 – 2.5e3 | 539 (21) | 1403 | 4 | 9.28e-2 | 1.04e-2 / 3.02e-2 |
| Fe-56 | 1e-5 – 8.5e5 | 850 (7) | 347 | 2 | 1.08 | 2.16e-2 / 6.23e-2 |
| O-16 | 1e-5 – 2.355e6 | 508 (1) | 53 | 2 | 3.02 | 4.04e-2 / 1.17e-1 |

The `broaden_poly` windows are the lowest ones (U-238: 1e-5 to 14 eV).

**Which evaluation the CORE data is.** WMP at 0 K against RECONR at 0 K
(tolerance 1e-3) on RECONR's own grid, relative:

| WMP vs tape | band integral ∫σ_c dE/E, worst decade band in range | NR capture integral σ₀ = 100 b, worst band | pointwise σ_c, worst |
|---|---|---|---|
| U-238 vs ENDF/B-**VII.0** | 2.5e-5 | 1.1e-5 | 5.2e-3 |
| U-238 vs ENDF/B-**VIII.0** | 2.8e-2 (0.1–1 keV) | 2.7e-2 | ×680 in valleys |
| U-235 vs ENDF/B-**VII.0** | 1.8e-5 | 1.2e-4 | 8.8e-3 |
| U-235 vs ENDF/B-**VIII.0** | 2.7e-1 (1–2.25 keV) | 2.7e-1 | ×99 |
| O-16 vs VII.0 (elastic) | 2.7e-5 | — | — |
| O-16 vs VIII.0 (elastic) | 1.5e-2 | — | — |

**What this table does and does not show (added after §5.0).** Both
integrals are trapezoids **on RECONR's own grid**, so they say that WMP
passes through RECONR's nodes, not that the integrals of the two smooth
functions agree to 1e-5: RECONR's linearisation (tolerance 1e-3) biases
the table's own integral by up to ~1e-3, and that bias is shared here.
It is the right instrument for *which evaluation* (the question of this
section), not for *how accurate*.

**So the CORE WMP reproduces ENDF/B-VII.0's resolved range (identical to
VII.1's for these nuclides, to the precision measured) and is a different
evaluation from the ENDF/B-VIII.0 tapes this workspace uses by default.**
Attaching it to an VIII.0 nuclide would not be a Doppler treatment; it
would swap the resolved-range evaluation (U-235 capture by 27 % in
1–2.25 keV). This study therefore measures WMP against the **VII.0** tapes,
and the wiring refuses a WMP set whose evaluation does not match the
nuclide's own (§3).

## 3. Methodology

### 3.1 Data and processing (as #947, with the evaluation matched)

- **U-238**: ENDF/B-VII.0 `n-092_U_238-ENDF7.0.endf`; **U-235**: ENDF/B-VII.0
  `n-092_U_235-ENDF7.0.endf`. CORE WMP `U238`, `U235`.
- **Moderator** for the slowing-down instrument: C-12 ENDF/B-VIII.0 at 300 K
  in every arm, exactly as #947 (only U-238's representation differs).
- **Exact arm**: `process::reconstruct` once (~~RECONR 1e-3~~ **RECONR
  1e-4**, amended after the first look, §5.0), then
  `process::cross_sections_at(T)` (BROADR at **errthn 1e-4**, PURR and HEATR
  off), through `Nuclide::from_processed`, read by `Nuclide::xs_at_energy`.
- **WMP arm**: the *same* nuclide object with the CORE WMP attached
  (`Nuclide::with_multipole`), read by the same `xs_at_energy(E, T)`. Inside
  `[E_min, E_max]` that is `WindowedMultipole::evaluate(E, T)`.
- **Temperatures**: 293.6, 600, 900, 1200, 1500 K and 2500 K (an extreme,
  inside upstream's generator limit of 3000 K, beyond any solid-fuel
  operating range).

### 3.2 Quantities (the #947 instrument)

1. **Pointwise** σ_c and σ_t, WMP/exact − 1, on the 0 K RECONR grid, in
   1e-5–1 eV, 1 eV–1 keV and 1–20 keV (U-235: to 2.25 keV).
2. **Band integrals** ∫σ dE/E over the #947 bands.
3. **NR self-shielded integrals** at σ₀ = 10, 100, 1000 b over 1 eV–20 keV
   (U-235: capture and fission, 1 eV–2.25 keV): `σ₀·N` (the RI) and `N/D`.
4. **Resonance escape probability p** of infinite U-238 + C-12, source
   10 keV, scored to 1 eV, σ_b = 100 and 1000 b, deterministic slowing down
   (`slowing_down::solve_on_grid`), which #947 §3.3 showed reproduces a
   per-collision Monte Carlo (19σ-resolved difference matched to 0.8σ).
5. **Floor**: the exact table at 900 K at errthn 1e-4 against 1e-5 (p and
   the integrals), so a WMP error below it is not read as one.
6. **Transport** (#948 acceptance 2): `k` of an infinite-like homogeneous
   U(VII.0, 3 % enriched) + C-nat (VII.0) mixture at 900 K, tables at 900 K
   against the same nuclides with WMP, `run_keff` (bare sphere, radius
   200 cm), CPU multi-thread, independent seeds.
7. **Majorant**: `Majorant::bounding` built over that material with WMP on,
   at each temperature, then `Majorant::audit` at 4000 log energies plus
   every breakpoint, its ulp neighbours and interval midpoints.
8. **Cost**: ns per `xs_at_energy` and per `total_at_energy` for U-238 at
   10⁶ log-uniform energies in 1 eV–20 keV, table against WMP.

### 3.3 Which numbers to quote

For the comparison with #947: **p at σ_b = 100 b and the RI at
σ₀ = 100 b** (§3.2 items 3–4), as #947 §2.4 decided. Pointwise maxima are
reported, not quoted: #947 showed they sit in interference valleys where
BROADR's own thinning moves capture by ~1 %.

## 4. Predictions (written 2026-10-18, before any run at T > 0)

Basis: the 0 K floor (§2) is ≤ 3e-5 in integrals and ≤ 0.9 % pointwise.
Doppler broadening averages a pointwise fit error over ~one Doppler width,
so at T the WMP-specific errors are the dropped `C` integral (small for
E ≫ kT/A, i.e. above ~1e-3 eV) and the unbroadened curve fit of the
excluded poles (the windows without `broaden_poly`). Neither should move a
lethargy integral; both grow with T.

| # | quantity | prediction |
|---|---|---|
| P1 | pointwise σ_c, 1 eV–1 keV, max \|err\| | ≤ 1 % at every T ≤ 1500 K; ≤ 2 % at 2500 K |
| P2 | pointwise σ_c, 1–20 keV, max \|err\| | ≤ 3 % (valleys, BROADR-thinning dominated) at ≤ 1500 K; ≤ 5 % at 2500 K |
| P3 | pointwise σ_c, 1e-5–1 eV | ≤ 0.1 % at ≤ 1500 K; ≤ 0.5 % at 2500 K (C integral, if anywhere) |
| P4 | band integrals, every band | ≤ 1e-4 at every T |
| P5 | NR capture RI, σ₀ = 100 b (and 10, 1000 b) | ≤ 0.02 % at ≤ 1500 K; ≤ 0.05 % at 2500 K; no trend in T below 1500 K |
| P6 | **p, σ_b = 100 b** | ≤ 0.02 % at ≤ 1500 K (#947's upstream table scheme: +0.40 % at Δ = 300 K, +0.037 % at Δ = 75 K, so WMP ≥ 2× better than the finest grid #947 measured); ≤ 0.05 % at 2500 K |
| P7 | thinning floor at 900 K, p | ≤ 3e-5 relative |
| P8 | U-235 capture and fission RI σ₀ = 100 b | ≤ 0.05 % at every T |
| P9 | k, WMP − tables at 900 K | 0 within 2σ (true difference ≲ 10 pcm) |
| P10 | majorant audit | worst Σ_t/Σ_maj ≤ 1 at every T, once the WMP pole peaks are breakpoints of the pointwise nuclide too |
| P11 | cost per U-238 lookup in the resolved range | WMP 5–20× the table lookup (`Fast` tier) |

### 4.4 Default ON or opt-in: the criterion, fixed before running

The correct-physics rule asks for default ON once WMP is verified as
accurate as the tables at the measured temperatures. The criterion:

1. **Accuracy**: P5/P6 hold, i.e. the WMP error in p and the RI at
   σ₀ = 100 b is no larger than the exact table's own thinning floor
   (P7) plus 0.02 %, at every T ≤ 1500 K; **and**
2. **Same evaluation**: the WMP data and the nuclide's tape are the same
   evaluation (§2). The default route reads ENDF/B-VIII.0 tapes and the
   CORE WMP is VII.x, so on the default data this is **not** met for any
   nuclide the probe checked, whatever item 1 shows.

If 1 holds and 2 does not, the wiring is kept **opt-in** (a checked
builder that refuses a mismatched evaluation), and the choice goes to the
maintainer with the numbers.

## 5. Results (run 2026-10-18)

Hardware for every timing below: `CPU only for transport, 3 cores
(taskset 0-2 or 3 threads), linux, 13th Gen Intel(R) Core(TM) i9-13900K,
62.5 GiB RAM`, shared machine (other agents building). All numbers are
`WMP/exact − 1` in % unless marked.

### 5.0 Two amendments after the first look (recorded, not hidden)

1. **The exact arm's RECONR tolerance, 1e-3 → 1e-4.** The first `mismatch`
   run refused U-238 **VII.0** — CORE's own evaluation — because its
   absorption band integral over 10–20 keV at 900 K differed by
   **−1.17e-3**, where 0 K had shown 3e-6. The first hypothesis was that the
   gated thing (WMP) was wrong. The `seam` stage tested it: the difference
   is **the same at 293.6, 900 and 2500 K** (−1.04e-3 to −1.10e-3 at
   10–15 keV), so it is not a Doppler effect, and it **shrinks 7.7×** when
   only RECONR's tolerance is tightened to 1e-4 (−1.35e-4). It is RECONR's
   linearisation, which BROADR integrates exactly and carries into every
   broadened table; WMP, evaluated pointwise, does not share it. The 0 K
   probe (§2) could not see it because both arms were integrated on
   RECONR's nodes. The exact arm is therefore made at RECONR 1e-4 /
   BROADR 1e-4, and the ladder (§5.4) measures what is left.
2. **The evaluation check measures each channel against the band's table
   total** (not absorption against absorption). With the first form, O-16
   VII.0 — CORE's evaluation, total agreeing to 8e-5 — was refused on an
   absorption of microbarns (+4067× relative), and U-238 VII.0 on the
   linearisation above. The absorption *fraction* is what transport sees.
   The tolerance stayed at 1e-3. Measured at 900 K with production
   processing (RECONR/BROADR 1e-3, the `mismatch` stage):

   | tape | verdict | worst band (channel, relative to the table total) |
   |---|---|---|
   | U-238 ENDF/B-VII.0 | accepted | total −2.1e-4 (10–20 keV) |
   | U-238 ENDF/B-VIII.0 | refused | total −8.6e-3 (1–10 eV) |
   | U-235 ENDF/B-VII.0 | accepted | total −1.4e-4 (1–2.25 keV) |
   | U-235 ENDF/B-VIII.0 | refused | absorption +3.5e-2 (1–2.25 keV) |
   | O-16 ENDF/B-VII.0 | accepted | total −7.7e-5 (1–2.36 MeV) |
   | O-16 ENDF/B-VIII.0 | refused | total +1.6e-2 (10–100 keV) |

   The gap is ≥ 4.7× on the accepting side and ≥ 8.6× on the refusing
   side. The check costs 0.13–0.17 s for a uranium isotope.

### 5.1 Instrument checks

- Rebuilt slowing-down grid against the library's own: p = 0.1732812311
  both (293.6 K, σ_b = 100 b).
- 0 K floor on the RECONR 1e-4 grid (1.28 M points U-238, 0.61 M U-235):
  pointwise capture +0.51 / −0.52 % (U-238), +0.61 / −0.91 % (U-235, both at
  the 283.7 eV doublet); NR capture σ₀ = 100 b −0.0001 % / +0.0001 %.

### 5.2 U-238 pointwise (ENDF/B-VII.0, exact arm RECONR 1e-4 / BROADR 1e-4)

Largest over / under, %:

| T K | capture 1e-5–1 eV | capture 1 eV–1 keV | capture 1–20 keV | total 1 eV–20 keV |
|---|---|---|---|---|
| 293.6 | +0.009 / −0.017 | +0.082 / −0.200 | +0.304 / −87.5 @ 20 keV | +0.116 / −66.3 @ 20 keV |
| 600 | +0.008 / −0.021 | +0.077 / −0.205 | +0.300 / −87.3 @ 20 keV | +0.102 / −75.9 @ 20 keV |
| 900 | +0.008 / −0.025 | +0.078 / −0.221 | +0.295 / −86.0 @ 20 keV | +0.098 / −74.8 @ 20 keV |
| 1200 | +0.008 / −0.030 | +0.073 / −0.179 | +0.288 / −84.7 @ 20 keV | +0.094 / −71.4 @ 20 keV |
| 1500 | +0.008 / −0.034 | +0.057 / −0.194 | +0.278 / −83.3 @ 20 keV | +0.090 / −68.0 @ 20 keV |
| 2500 | +0.007 / −0.048 | +0.058 / −0.217 | +0.250 / −77.1 @ 20 keV | +0.091 / −59.7 @ 20 keV |

**The −60 to −87 % is one point, E = 20 000 eV exactly**, the step where
ENDF's resolved range meets the unresolved range: the table there holds
the URR-side value (0.279 b capture at 0 K), WMP the resolved-side value
(0.030 b), because upstream's range test `E <= E_max` is inclusive
(`nuclide.cpp:623`) and U-238's E_max is 19999.99 eV. Excluding that
point, every pointwise error is ≤ 0.30 % (capture) and ≤ 0.12 % (total),
**at every temperature including 2500 K**, and it does not grow with T.

### 5.3 U-238 integrals, and U-235

| T K | capture band integrals (worst of 5) | total band (worst) | NR RI σ₀ = 10 | 100 | 1000 b | **p, σ_b = 100 b** | p, σ_b = 1000 b |
|---|---|---|---|---|---|---|---|
| 293.6 | −0.0079 | −0.0123 | −0.0052 | −0.0037 | −0.0019 | **+0.0063** | +0.0010 |
| 600 | −0.0086 | −0.0044 | −0.0054 | −0.0037 | −0.0016 | **+0.0065** | +0.0009 |
| 900 | −0.0091 | −0.0035 | −0.0056 | −0.0037 | −0.0015 | **+0.0067** | +0.0009 |
| 1200 | −0.0096 | −0.0038 | −0.0058 | −0.0037 | −0.0015 | **+0.0070** | +0.0009 |
| 1500 | −0.0102 | −0.0041 | −0.0061 | −0.0038 | −0.0015 | **+0.0074** | +0.0010 |
| 2500 | −0.0125 | −0.0048 | −0.0070 | −0.0042 | −0.0015 | **+0.0087** | +0.0011 |

(p itself: 0.173281 at 293.6 K to 0.091237 at 2500 K.) §5.4 shows that
about half of each number is the exact arm's own processing error.

U-235 (ENDF/B-VII.0), 1 eV – 2.25 keV, %:

| T K | capture RI σ₀ = 10 / 100 / 1000 | fission RI σ₀ = 10 / 100 / 1000 | pointwise capture | pointwise fission | fission 1e-5–1 eV |
|---|---|---|---|---|---|
| 293.6 | −0.0047 / −0.0046 / −0.0039 | −0.0015 / −0.0028 / −0.0029 | +0.25 / −35.4 @ 2250 | +91.2 @ 2250 / −0.11 | +0.043 / −0.036 |
| 600 | −0.0068 / −0.0068 / −0.0061 | −0.0018 / −0.0038 / −0.0043 | +0.16 / −19.3 @ 2250 | +73.4 @ 2249.9 / −0.10 | +0.038 / −0.034 |
| 900 | −0.0088 / −0.0089 / −0.0083 | −0.0022 / −0.0048 / −0.0058 | +0.15 / −17.6 @ 2250 | +61.2 @ 2249.9 / −0.09 | +0.033 / −0.033 |
| 1200 | −0.0107 / −0.0111 / −0.0106 | −0.0026 / −0.0060 / −0.0075 | +2.1 / −17.4 @ 2250 | +52.6 @ 2249.8 / −0.09 | +0.028 / −0.031 |
| 1500 | −0.0126 / −0.0133 / −0.0129 | −0.0031 / −0.0071 / −0.0091 | +12.7 / −16.9 @ 2250 | +46.1 / −3.3 @ 2250 | +0.023 / −0.030 |
| 2500 | −0.0191 / −0.0209 / −0.0212 | −0.0046 / −0.0112 / −0.0151 | +19.1 / −17.7 @ 2250 | +32.7 / −6.1 @ 2250 | +0.022 / −0.040 |

Away from the top fraction of an eV of the range, U-235's pointwise errors
are ≤ 0.25 % (capture) and ≤ 0.11 % (fission). **The edge is the one WMP
limit this study resolves:** within a few Doppler widths of E_max
(ΔE ≈ 2√(E·kT/A) ≈ 1 eV at 2.25 keV, 900 K) WMP lacks the broadened tail
of whatever lies above its range, while BROADR broadened the table across
it. Its weight in the integrals is what the RI columns show, and it grows
with T, as the U-235 RIs do (−0.005 % at 293.6 K, −0.021 % at 2500 K).

### 5.4 The processing ladder at 900 K: who is closer to the truth

U-238, against RECONR 1e-5 / BROADR 1e-5 (3.85 M-point reconstruction;
1321 s for the ladder, one thread), %:

| arm | p σ_b = 100 b | RI σ₀ = 10 b | RI σ₀ = 100 b | worst capture band |
|---|---|---|---|---|
| RECONR 1e-3 / BROADR 1e-3 (**production**, NJOY defaults) | −0.0496 | +0.0353 | +0.0254 | +0.0527 |
| RECONR 1e-3 / BROADR 1e-4 (#947's exact arm) | −0.0301 | +0.0228 | +0.0159 | +0.0504 |
| RECONR 1e-4 / BROADR 1e-4 (this study's exact arm) | −0.0049 | +0.0036 | +0.0025 | +0.0062 |
| RECONR 1e-4 / BROADR 1e-5 | −0.0030 | +0.0023 | +0.0016 | +0.0058 |
| **WMP** | **+0.0018** | **−0.0021** | **−0.0011** | **+0.0032** |

**WMP at 900 K is closer to converged NJOY processing than any table
measured, and ~25× closer than the production table** on p. The §5.3
WMP-against-exact numbers (+0.0067 % on p at 900 K) are mostly the exact
arm's own error (−0.0049 %). For #947 this means its "exact" arm carried
−0.03 % in p and +0.016 % in the RI. #947's differences are between tables
made from one reconstruction, which share most of that, so its
interpolation errors stand; but its absolute floor is this one, not the
2e-5 its §3.0 recorded (it varied BROADR only).

### 5.5 The delta-tracking majorant

`tests/wmp_local_temperature_948.rs`, U-238 VII.0 (tables at 900 K, WMP at
the material temperature), `Majorant::bounding(1e-5, 2e7, 1024 bins × 8)`,
then `audit` (2000 log points plus every breakpoint, its ulp neighbours and
interval midpoints; 525 592 energies):

| material T K | margin 0: worst Σ_t/Σ_maj (at E) | margin 0.05 (the crate's other WMP pins) |
|---|---|---|
| 293.6 | **1.00269** (11.40 keV) | 0.95494 |
| 900 | **1.00074** (3.437 keV) | 0.95308 |
| 1500 | **1.00056** (1.245 keV) | 0.95291 |
| 2500 | **1.00032** (8.713 keV) | 0.95268 |
| control: built at 293.6 K, audited at 1500 K | 3.28 (1.661 keV) | — |

With no margin the WMP pole nodes leave the narrowest (coldest) peaks up to
0.27 % above the bound: the node `Re(p)²` is not the exact maximum of a
broadened, interfering line. That is the gap `Majorant::bounding`'s doc
already names for WMP ("between them the shape is only sampled") and
covers with its margin; the crate's existing WMP pins use 0.05, which
bounds at every temperature here with 4.5 % to spare. The control shows
the audit sees temperature: a cold-built majorant is 3.3× under at
1500 K. **A majorant must be built at the temperature it is used at**, as
the majorant module doc says; the continuous-in-T cover is #945's.

### 5.6 Cost

U-238 at 900 K, 10⁶ log-uniform energies in 1 eV–20 keV, one thread:

| lookup | tables | WMP | ratio |
|---|---|---|---|
| `xs_at_energy` (all channels) | 332 ns | 501 ns | 1.5 |
| `total_at_energy` (flight distance) | 33 ns | 500 ns | 15.3 |

In transport (§5.7) the WMP arm ran 3.6× slower (3 threads) and 2.8×
(2 threads) on a graphite-moderated U case. `total_at_energy` evaluates
the full WMP form to read one channel, which is where the cost is.

### 5.7 Transport: k with WMP against the tables at 900 K

U(3 at.%, ENDF/B-VII.0 U-235 and U-238) + C-nat (ENDF/B-VII.0, free gas,
no S(α,β)), C/U = 300, N_C = 0.08 /(b·cm), bare sphere of radius 200 cm,
all tables RECONR/BROADR 1e-3 at 900 K, `Material::temperature` = run
temperature = 900 K; the WMP arm attaches CORE `U235` and `U238`
(`with_core_multipole`, accepted). `run_keff`, CPU multi-thread
(3 threads), independent seeds (11, 23):

| arm | k | histories | wall |
|---|---|---|---|
| tables at 900 K | 0.91088 ± 0.00041 | 40 000 × [20 + 200] | 328 s |
| WMP at 900 K | 0.91078 ± 0.00047 | 40 000 × [20 + 200] | 1191 s |
| **WMP − tables** | **−10 ± 63 pcm (−0.16σ)** | | **3.6× slower** |

A first, shorter pair (10 000 × [20 + 100], 2 threads) gave −121 ± 174 pcm
(−0.69σ) at 2.8× the cost. #948's acceptance 2 holds: no difference is
resolved. A 63 pcm σ cannot see the ~2–5 pcm the integrals of §5.3–5.4
imply, so this checks the wiring (flight, selection, collision,
kinematics on one consistent set), not the accuracy.

The majorant on this material (`majorant` stage, margin 0, 1024 log bins ×
8 → 221 143 nodes, ~1.2 s each, audit at 773 880 energies):

| material T K | 293.6 | 600 | 900 | 1200 | 1500 | 2500 |
|---|---|---|---|---|---|---|
| worst Σ_t/Σ_maj, margin 0 | 1.000252 | 1.000146 | 1.000067 | 1.000046 | 1.000046 | 1.000027 |

Control (built at 293.6 K, audited at 1500 K): 1.1685 at 6.52 eV. Diluted
in carbon the WMP deficit at margin 0 is 0.025 % at most; any margin above
it bounds.

### 5.8 Predictions against results

| # | predicted | measured | verdict |
|---|---|---|---|
| P1 | pointwise capture 1 eV–1 keV ≤ 1 % (≤ 2 % at 2500 K) | ≤ 0.22 % at every T | held |
| P2 | 1–20 keV ≤ 3 % (≤ 5 % at 2500 K) | ≤ 0.30 % except the one point at E_max (−87 %, the RRR/URR step) | held away from the seam; the seam was not predicted |
| P3 | thermal ≤ 0.1 % (≤ 0.5 % at 2500 K) | ≤ 0.034 % (0.048 % at 2500 K) | held |
| P4 | band integrals ≤ 1e-4 at every T | capture ≤ 1.0e-4 to 1500 K, 1.25e-4 at 2500 K; total 1.23e-4 at 293.6 K | **marginally refuted** (half is the exact arm's own error, §5.4) |
| P5 | RI ≤ 0.02 %, no trend in T to 1500 K | ≤ 0.0061 % (σ₀ = 10 b), ≤ 0.0042 % (100 b); a slow trend (−0.0052 → −0.0061 %, σ₀ = 10 b) | magnitude held; "no trend" refuted |
| P6 | p ≤ 0.02 % to 1500 K, ≤ 0.05 % at 2500 K | +0.0063 … +0.0074 %; +0.0087 % at 2500 K | held |
| P7 | thinning floor at 900 K ≤ 3e-5 in p | exact arm −4.9e-5 against the finest processing; production −5.0e-4 | **refuted** (RECONR, not BROADR, dominates) |
| P8 | U-235 RI ≤ 0.05 % | ≤ 0.013 % to 1500 K, 0.021 % at 2500 K | held |
| P9 | k: 0 within 2σ | −10 ± 63 pcm (−0.16σ) | held |
| P10 | majorant ≤ 1 with pole nodes, margin 0 | 1.0027 at 293.6 K, margin 0 | **refuted**; ≤ 0.955 with the crate's margin 0.05 |
| P11 | WMP 5–20× the table per lookup | 15× for the total, 1.5× for the full lookup | held for the total only |

### 5.9 What #947 is compared with

On the quantity both studies quote (p, σ_b = 100 b, strongly shielded
U-238 + C-12), the worst error of each route over 293.6–1500 K:

| route | error in p |
|---|---|
| upstream stochastic interpolation, 17 tables (Δ = 75 K) | +0.037 % (#947) |
| upstream stochastic, 5 tables (Δ = 300 K) | +0.40 % (#947) |
| Chebyshev fit in √T, 5 tables | −0.010 % (#947) |
| **WMP at the local T** | **+0.0074 % against RECONR/BROADR 1e-4; +0.0018 % against 1e-5 (900 K)** |
| a production table processed exactly at T | −0.050 % (900 K, §5.4) |

The evaluations differ (#947 used ENDF/B-VIII.0 U-238, this study VII.0,
because the CORE WMP is VII.0's); relative errors on the same instrument
are comparable, absolute p is not.

## 6. What was wired, and what differs from upstream

- `Nuclide::with_multipole(wmp) -> Result<Self, MultipoleAttachError>`,
  `with_core_multipole()`, `without_multipole()` (the ablation),
  `multipole()`, `multipole_consistency(&wmp)`, and
  `MULTIPOLE_EVALUATION_TOLERANCE` (`src/material/nuclide/multipole.rs`).
- Inside `[e_min, e_max]` of the attached set (both ends inclusive, as
  upstream): `xs_at_energy` and `total_at_energy`, and so flight, nuclide
  selection, the collision and `Material::macro_xs*`, read
  `WindowedMultipole::evaluate(E, Material::temperature)`; inelastic,
  (n,xn), MT=5 and the other channels are zero there; no URR band is drawn
  (`needs_urr_draw`, `xs_at_energy_urr`, `total_upper_bound`); S(α,β)
  still replaces elastic below its cutoff.
- `free_gas_kt` uses the lookup temperature at **every** energy for a
  nuclide with WMP attached (upstream `physics.cpp:734`).
- `majorant_breakpoints` adds the set's range ends, window edges and pole
  peaks.
- **Kept different from upstream, documented:** the partial-fission
  spectrum weights and `reaction_xs(mt, e)` (per-MT tallies, depletion
  rates) still read the tables at their build temperature inside the WMP
  range, where upstream uses the first fission reaction and `σ_a − σ_f`.
  Upstream refuses nothing; this port refuses another evaluation.
- Tests: `tests/wmp_local_temperature_948.rs` (3) and
  `material::nuclide::multipole::tests` (1).

## 7. Default ON or opt-in: **opt-in**, by the pre-registered criterion

Criterion 1 (accuracy) **holds**: WMP's error in p and the RI is below the
exact table's own processing error at every T to 1500 K (§5.3–5.4), and
far below a production table's. Criterion 2 (same evaluation) **fails on
the default data**: every ENDF/B-VIII.0 tape checked is refused, because
the CORE WMP is ENDF/B-VII.0's resolved range. So no constructor attaches
it, and §4.4 sends the choice here.

**For the maintainer to decide:**
1. Whether constructors should attach CORE WMP *automatically where the
   check accepts it* (today: ENDF/B-VII.0 tapes only). That would be
   default-ON physics wherever it is the same evaluation, at about 3.6×
   transport cost, and it changes nothing for ENDF/B-VIII.0.
2. Whether to make WMP for ENDF/B-VIII.0. That is a fitting job
   (upstream's generator, `openmc/data/multipole.py`: vector fitting, then
   windowing designed to 3000 K); nothing in this workspace does it.
3. Whether the seam at E_max (§5.2–5.3) should stay inclusive, as
   upstream, or hand the last few Doppler widths back to the tables.

## 8. Limitations

- Two nuclides (U-238, U-235), one evaluation (ENDF/B-VII.0). The CORE
  library's own design temperature range is not documented in anything this
  workspace holds (§1); it was measured to 2500 K only.
- The p instrument is #947's: isotropic-CM scattering off a target at
  rest, infinite medium, 1 eV–10 keV.
- 2500 K is beyond any solid-fuel operating range, run as an extreme only.
- Per-MT tallies and depletion rates inside the WMP range read tables (§6).
