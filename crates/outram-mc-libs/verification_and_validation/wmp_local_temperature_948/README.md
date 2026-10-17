# Windowed multipole at the local temperature, against data processed exactly at that temperature (GitHub #948)

**Issue:** [#948](https://github.com/theodoreOnzGit/outram-park-backend/issues/948),
step 2 (method B) of [#945](https://github.com/theodoreOnzGit/outram-park-backend/issues/945)
(epic #925). Step 1 is #947
(`../temperature_interpolation_947/README.md`), whose instrument this study
reuses so the two are comparable.

**Status:** upstream reading, the probe of the data (§2) and the
predictions (§4) were written 2026-10-18 **before any run at T > 0**.
Results are in §5 onwards.

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
- **Exact arm**: `process::reconstruct` once (RECONR 1e-3), then
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
