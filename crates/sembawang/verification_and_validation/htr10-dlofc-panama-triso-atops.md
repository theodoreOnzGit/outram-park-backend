# HTR-10 DLOFC: PANAMA-I fuel failure wired into the TRISO-ATOPS release chain

**Date:** 2026-09-24. **GitHub:** #296 (this work), #295 (PANAMA equation-level V&V).
**Code:** `crates/sembawang/src/htr10.rs`,
`crates/sembawang/examples/htr10_dlofc_panama_source_term.rs`.

> **RESEARCH, EDUCATION AND V&V ONLY.** Not for facility operation, licensing,
> safety-critical decisions, emergency planning or emergency response. No dose
> quantity is computed. **No number in this document is a source term for
> HTR-10.** `RESPONSIBLE_USE.md` and `AI_USAGE.md` apply; this is AI-assisted
> draft material pending human review.

## 1. What was done, and what it is for

`boon-lay` carries two independent models that had never been run together on
one reactor:

- `fuel_failure` — PANAMA-I (Verfondern & Nabielek, HTA-IB-03/90), which
  computes the *fraction of TRISO particles that fail*;
- `triso_atops_fork` — INL TRISO-ATOPS, which computes *how much activity
  escapes* given a failure fraction.

This work joins them on an HTR-10 case and drives the result with the published
HTR-10 equilibrium-core inventory (`changi::activity::inventory`, Liu & Cao
2002, *Nucl. Eng. Des.* **218** 81–90, Table 1, ORIGEN2 at 80 000 MWd/t).

**Where the seam goes, and why not the obvious place.** `boon_lay::fuel_failure::htr10`
had already established that PANAMA's in-service failure fraction under *normal
operation* is 10⁻¹⁵–10⁻⁶ across HTR-10's plausible fuel-temperature band,
against the 3·10⁻⁵ as-manufactured placeholder that release calculations
actually use — so substituting it there would divide every activity by ~10⁷ on
the strength of a model answering a different question. The seam therefore
feeds TRISO-ATOPS's **`f_inc_acc`**, the accident-added incremental failure
fraction. The four normal-operation fractions stay the caller's, because PANAMA
models none of them (its own `φ_o` is an input to it too, page -480-).

## 2. Methodology

### 2.1 Inputs, each classified

| kind | input | value | source |
|---|---|---|---|
| HTR-10's own | kernel radius | 250 µm | IAEA-TECDOC-1382 pt 2 Table 4-17, read from `tampines::pebble_bed::triso::TrisoParticle::htr10` |
| HTR-10's own | SiC thickness | 35 µm (380 → 415 µm) | same |
| HTR-10's own | unfuelled graphite shell | 5.00 mm (60 mm pebble, 50 mm fuelled zone) | same, via `Pebble::htr10` |
| HTR-10's own | 22-nuclide core inventory | Bq per nuclide | Liu & Cao 2002 Table 1, via `changi` |
| derived | `F_b` | 0.0851 FIMA | from the published 80 000 MWd/t; arithmetic shown and tested in `boon_lay::fuel_failure::htr10` |
| derived | `t_B` | 1080 FPD | from 10 MW over 27 000 × 5 g HM |
| **stand-in** | `σ_oo` / `m_oo` | 834 MPa / 8.02 | EO 1607 — the report's **own** HTR-Module choice (footnote 1, p-503) |
| **stand-in** | `Γ` | 1.4·10²⁵ m⁻² EDN | HTR-Module (Table 2, p-504) |
| **stand-in** | `T_B` | 776 °C | HTR-Module average (Table 2). HTR-10 publishes a *maximum* fuel temperature, not an average |
| **stand-in** | transient peak / time to peak | 1500 °C at 30 h | HTR-Module DLOFC, JRC EUR 28712 EN §9.9.2 and §7.2.2 |
| **stand-in** | normal-operation `f_hm`, `f_sic`, `f_inc`, `f_inc_sic` | 1·10⁻⁴, 1·10⁻⁴, 2.3·10⁻⁵, 3.6·10⁻⁵ | TRISO-ATOPS's NP-MHTGR reference case, pinned against upstream `de374c8` |
| **ESTIMATE** | cooldown time constant | 60 h | **not cited** — see §6 |
| **ESTIMATE** | late-time temperature | 900 °C | **not cited** — see §6 |
| prescribed | `x_liftoff` | 0 | there is nothing to lift off; pools start empty |
| prescribed | `f_inc_sic_acc` | 0 | PANAMA's `φ₂` is already inside `f_inc_acc`; see §6 |

### 2.2 The transient

`DlofcShape`: linear rise from `T_B` to the peak over 30 h, then
`T = T_late + (T_peak − T_late)·exp(−(t − t_peak)/τ)`, carried to 200 h — the
window HTA-IB-03/90 itself reports HTR-Module depressurised failure at
(page -504-), so the result is directly comparable with that statement.

The rise is what the JRC volume states: *"After 30 hours the maximum fuel
temperature reaches 1 500 °C. After reaching the maximum the temperature
decreases"* (§9.9.2), and *"the maximum temperature is around 1 500 °C in the
hotspot region of the HTR Module after 30 hours"* (§7.2.2). Table 44 gives the
same accident's nominal peak as 1450 °C and its maximum as 1615 °C; both are run
as a sensitivity.

**Using HTR-Module's transient for HTR-10 is a stand-in, in the conservative
direction.** HTR-10 is 10 MW thermal against HTR-Module's 200 MW, at lower mean
power density and with a far shorter heat-transport path out of the core, so its
own DLOFC peak is expected *below* 1500 °C. The choice is also internally
consistent: the `σ_oo`/`m_oo` and `Γ` stand-ins already come from this same
HTR-Module column of this same report.

### 2.3 Node resolution, and the direction of its error

**One radial ring, one axial node.** The inventory is a whole-core figure and no
HTR-10 DLOFC temperature *distribution* is available, so a finer node count
would invent a distribution neither input carries. The consequence has a sign:
the whole core is put on the hot-node history, while the source itself says
under 5 % of elements reach the peak. **This over-states the release**, which is
why it is reportable as a bound rather than an estimate.

### 2.4 Grid convergence — measured, not assumed

PANAMA steps between samples at the interval midpoint temperature, so the
answer is grid-dependent.

| samples | step | `f_inc` at 200 h | Δ from previous | Δ ratio |
|---|---|---|---|---|
| 51 | 4 h | 9.005579·10⁻⁸ | — | — |
| 101 | 2 h | 1.0553258·10⁻⁷ | +1.5477·10⁻⁸ | — |
| **201** | **1 h** | **1.0676990·10⁻⁷** | +1.2373·10⁻⁹ | **12.51** |
| 401 | 30 min | 1.0708605·10⁻⁷ | +3.1615·10⁻¹⁰ | 3.914 |
| 801 | 15 min | 1.0716552·10⁻⁷ | +7.947·10⁻¹¹ | 3.979 |
| 1601 | 7.5 min | 1.0718541·10⁻⁷ | +1.990·10⁻¹¹ | 3.994 |

Second order in Δt from the 2 h step down (ratios → 4); the 4 h step is **not**
in the asymptotic range and the table says so rather than starting where the
sequence behaves. Richardson on the finest pair gives **1.071920·10⁻⁷**, so the
201-sample grid reported everywhere below is **0.39 % low**. Nothing here is
quoted finer than that.

Pass criterion and result: `the_time_grid_converges_at_second_order_and_the_error_is_stated`.

## 3. Results — PANAMA over the transient

`φ₁` at the end of irradiation (PANAMA's `t = 0` value, page -482-):
**1.214·10⁻¹²**.

| t \[h\] | T \[°C\] | `φ₁` | `φ₂` | `f_inc` |
|---|---|---|---|---|
| 0 | 776.0 | 1.214·10⁻¹² | 0 | 1.214·10⁻¹² |
| 20 | 1258.7 | 7.406·10⁻¹¹ | 0 | 7.406·10⁻¹¹ |
| 30 (peak) | 1500.0 | — | 0 | `f_inc_acc` = **2.684·10⁻⁸** |
| 40 | 1407.9 | 9.349·10⁻⁸ | 0 | 9.349·10⁻⁸ |
| 60 | 1263.9 | 1.061·10⁻⁷ | 0 | 1.061·10⁻⁷ |
| 200 | 935.3 | 1.068·10⁻⁷ | 0 | **1.068·10⁻⁷** |

`φ₂` is **identically zero** at every temperature this transient reaches. It is
3.4·10⁻¹⁵ at a flat 1600 °C and only overtakes `φ₁` between 2000 and 2200 °C,
matching HTA-IB-03/90 page -508-. So **thermal decomposition contributes nothing
to an HTR-10 DLOFC**, and the whole accident failure here is the pressure-vessel
mechanism.

### 3.1 The one check the literature supports

HTA-IB-03/90 page -504- states that HTR-Module **depressurised** stays below
10⁻⁶ at 200 h. That is a statement about a *transient*.

| arm | `f_inc` at 200 h | against the 10⁻⁶ bound |
|---|---|---|
| **DLOFC transient**, peak 1500 °C at 30 h | **1.068·10⁻⁷** | **consistent**, 9.4× below |
| flat 200 h at 1600 °C | **3.103·10⁻⁵** | 31× above |

The flat hold exceeding the bound is **not** a disagreement: it holds the fuel at
its accident limit for the whole window, which no transient does. The transient
arm is the one the report's sentence is about, and it agrees.

**This is not a validation.** It is HTR-Module's bound checked against
HTR-Module's transient, evaluated with HTR-10's geometry and burnup; and PANAMA
was validated on German TRISO over 1600–2500 °C, so both arms are
extrapolations at or below 1600 °C. Digitising the report's Fig. 10 (the actual
HTR-Module accident temperature history) would turn it into a real check — #296
asks for it.

### 3.2 Isothermal reference grid, 200 h holds

| T \[°C\] | `φ₁` | `φ₂` | `f_inc` |
|---|---|---|---|
| 1200 | 4.529·10⁻⁹ | 0 | 4.529·10⁻⁹ |
| 1400 | 6.848·10⁻⁷ | 0 | 6.848·10⁻⁷ |
| 1500 | 5.440·10⁻⁶ | 0 | 5.440·10⁻⁶ |
| 1600 | 3.103·10⁻⁵ | 3.442·10⁻¹⁵ | 3.103·10⁻⁵ |
| 1800 | 4.464·10⁻⁴ | 3.264·10⁻⁹ | 4.464·10⁻⁴ |
| 2000 | 4.794·10⁻³ | 2.779·10⁻⁴ | 5.071·10⁻³ |
| 2200 | 6.069·10⁻² | 9.770·10⁻¹ | 9.784·10⁻¹ |

Reproduces `boon_lay::fuel_failure::htr10`'s own accident table, which is the
cross-check that the seam has not changed the failure model.

## 4. Results — the release, and the ablation control

19 of the 22 published nuclides are modelled by TRISO-ATOPS. **H-3
(3.81·10¹² Bq), Xe-135m (2.64·10¹⁵ Bq) and Rb-88 (1.03·10¹⁶ Bq) are absent from
its 84-nuclide table** and are named rather than zeroed; the last two are among
the larger entries, so every total below is a total over the 19 and never over
the core.

Seven more are screened out by TRISO-ATOPS's own half-life test (`t½` under 4 %
of the accident duration): Kr-83m, Kr-85m, Kr-87, Kr-88, I-132, I-134, I-135.

Venting spans only the **heating** leg (upstream's `coolant_release` selects
`dT/dt ≥ 0`): 30 release windows over 0–30 h.

| nuclide | inventory \[Bq\] | released \[Bq\] | released/inv | PANAMA ablated \[Bq\] | on/off |
|---|---|---|---|---|---|
| Kr-85 | 8.750·10¹³ | 2.153·10⁸ | 2.461·10⁻⁶ | 2.151·10⁸ | 1.0009 |
| Xe-131m | 1.070·10¹⁴ | 2.633·10⁸ | 2.461·10⁻⁶ | 2.631·10⁸ | 1.0009 |
| Xe-133 | 2.050·10¹⁶ | 5.045·10¹⁰ | 2.461·10⁻⁶ | 5.040·10¹⁰ | 1.0009 |
| Xe-133m | 5.900·10¹⁴ | 1.452·10⁹ | 2.461·10⁻⁶ | 1.451·10⁹ | 1.0009 |
| Xe-135 | 7.940·10¹⁵ | 1.954·10¹⁰ | 2.461·10⁻⁶ | 1.952·10¹⁰ | 1.0009 |
| I-131 | 9.770·10¹⁵ | 2.404·10¹⁰ | 2.461·10⁻⁶ | 2.402·10¹⁰ | 1.0009 |
| I-133 | 2.110·10¹⁶ | 5.192·10¹⁰ | 2.461·10⁻⁶ | 5.188·10¹⁰ | 1.0009 |
| Sr-89 | 1.300·10¹⁶ | 4.825·10⁹ | 3.711·10⁻⁷ | 4.823·10⁹ | 1.0004 |
| Sr-90 | 5.340·10¹⁴ | 1.982·10⁸ | 3.711·10⁻⁷ | 1.981·10⁸ | 1.0004 |
| Cs-134 | 3.110·10¹⁴ | 1.076·10¹⁰ | 3.461·10⁻⁵ | 1.076·10¹⁰ | 1.0004 |
| Cs-137 | 6.920·10¹⁴ | 2.395·10¹⁰ | 3.461·10⁻⁵ | 2.394·10¹⁰ | 1.0004 |
| Ag-110m | 2.160·10¹² | 5.407·10⁴ | 2.503·10⁻⁸ | 5.407·10⁴ | 1.0000 |

**Total over the 19 modelled nuclides: 1.8762·10¹¹ Bq with PANAMA,
1.8747·10¹¹ Bq ablated — ratio 1.0008.**

### 4.1 The headline result: at HTR-10's DLOFC the seam is negligible, and that is the finding

`f_inc_acc = 1.068·10⁻⁷` against an as-manufactured `f_hm + f_inc =
1.230·10⁻⁴`. Volatile release is linear in that sum, so the accident-added
failure raises the source term by **0.08 %**. The ablation arm is the control:
the two columns are *not* equal, so the seam is wired and doing arithmetic — it
is simply small.

**This is not a defect in either model.** It is the quantitative statement that
at a 1500 °C peak an HTR-10 DLOFC source term is governed by *as-manufactured
fuel quality*, not by accident-induced particle failure. The crossover is above
the design limit: see §4.3.

### 4.2 Two structural properties of the chain, neither a bug

- **`released/inv` is identical across Kr, Xe and I** (2.461·10⁻⁶). The Booth
  transient release fraction depends only on `∫D dt / r²`, the atoms↔curies
  conversion cancels `λ`, and TRISO-ATOPS gives noble gases and halogens the
  same kernel correlation. So the released *fraction* is a property of the
  transport group, not of the nuclide; the nuclides differ only through their
  inventories. Same reason Sr-89/Sr-90 and Cs-134/Cs-137 pair up.
- **Ag-110m's on/off ratio is exactly 1.** TRISO-ATOPS's silver branch sets the
  scaling fraction to 1 — the breakthrough model already embeds the failed
  population — so **PANAMA's failure fraction has no effect on silver at all**.
  A reader expecting the fuel-failure model to move Ag-110m will not see it
  move, and that is upstream's structure rather than a wiring error here.

### 4.3 Sensitivity 1 — the peak, and where the growth actually comes from

| peak \[°C\] | `f_inc_acc` (200 h) | released \[Bq\] | vs 1500 °C | of which from `f_inc_acc` | from diffusion |
|---|---|---|---|---|---|
| 1450 (Table 44 nominal) | 3.753·10⁻⁸ | 1.158·10¹¹ | 0.617 | 0.999 | 0.618 |
| **1500 (§9.9.2)** | 1.068·10⁻⁷ | **1.876·10¹¹** | 1.000 | 1.000 | 1.000 |
| 1615 (Table 44 maximum) | 1.010·10⁻⁶ | 6.920·10¹¹ | 3.688 | 1.007 | 3.661 |
| 1800 (**beyond the limit**) | 2.233·10⁻⁵ | 2.872·10¹² | 15.305 | 1.180 | 12.965 |

The split is exact rather than estimated: the volatile path is linear in
`f_hm + f_inc + f_inc_acc`, so that ratio *is* the failure model's share and the
remainder is the release fraction's.

**The source term's temperature sensitivity is Arrhenius diffusion's, not the
fuel-failure model's** — going 1500 → 1800 °C multiplies the release by ~15,
of which the failure model contributes ~1.18. 1800 °C is included only to locate
the crossover and is **not** an HTR-10 condition.

The peak alone moves the answer by a factor 3.7 across the report's own three
values for the *same* accident (1450/1500/1615 °C). That spread is larger than
anything the seam contributes, which is the practical argument for #296's ask.

### 4.4 Sensitivity 2 — `f_hm`, over the German-lineage measured bracket

HTR-10's fuel is German-lineage. The measured free-uranium record
(Kugeler et al. 2017 Table 7, carried in
`boon_lay::fuel_failure::htr10::qualification::FREE_URANIUM_FRACTIONS`) is
7.8·10⁻⁶ to 50.7·10⁻⁶ — 2 to 13 times **better** than NP-MHTGR's 1·10⁻⁴.

| `f_hm` | released \[Bq\] | vs NP-MHTGR |
|---|---|---|
| 1.000·10⁻⁴ (NP-MHTGR) | 1.876·10¹¹ | 1.0000 |
| 5.070·10⁻⁵ (German worst) | 1.208·10¹¹ | 0.6441 |
| 7.800·10⁻⁶ (German best) | 6.272·10¹⁰ | 0.3343 |

Neither German value is HTR-10's own measurement; they bracket what a
German-lineage production line achieved, which is the nearest published thing
available. **A factor 3 on the source term sits in this one uncited input** —
substantially more than the seam this work added.

## 5. Two defects found in `sembawang` while doing this, and fixed in the same change

1. **`Caveats::negative_atom_count_seen` documented the wrong direction for half
   the cases it covers.** It said the total is "correspondingly under-stated".
   That holds for an unclamped negative atom count, but the other path that sets
   it — a negative per-window first difference — is *floored to zero* at the
   `SourceTerm` boundary, which **raises** the sum above the cumulative endpoint.
   The flag fires on this case through the second path: the **silver**
   breakthrough release fraction rises, is driven negative by its `−a/(2r)`
   time-lag term and clamped to zero until breakthrough, then grows — so the
   cumulative curie series is non-monotonic. Measured: Ag-110m had **13 of 30
   windows negative**, and the windows sum to 2.503345·10⁻² Ci against a
   cumulative endpoint of 2.500569·10⁻² Ci — **over-stated by a factor 1.0011**.
   Doc corrected in place with the measured numbers.

   **Which nuclides are affected was itself measured, after first being
   generalised — recorded because the sequence matters.** The original claim
   here ("every other nuclide had zero negative windows") rested on four checks
   — Kr-85, Cs-137, Sr-90, Ag-110m — plus the structural argument that silver
   is the only nuclide routed through the breakthrough model. The argument is
   right and it was still not evidence: four is not twelve, and an argument
   that has never been able to fail cannot carry a claim written into a caveat
   other people act on. `only_silver_carries_floored_negative_windows` now runs
   the chain **once per nuclide** on a one-nuclide inventory, because
   `accident_release` returns one `Caveats` for a whole run and "which nuclide"
   cannot be read off a combined result. Result: **exactly one of the twelve,
   Ag-110m**; Kr-85, Xe-131m, Xe-133, Xe-133m, Xe-135, I-131, I-133, Sr-89,
   Sr-90, Cs-134 and Cs-137 are all clean.
2. **`Caveats::booth_transient_floored` is never set by anything.** It is
   documented as reporting the ~1.216·10⁻⁴ Booth floor, but a search of the
   workspace finds writes to it only in that module's own tests, so it is always
   `false` on a computed result. Marked **NOT WIRED** at the field, with why
   detecting it needs release-fraction values `accident_release` does not keep.

## 6. What is needed to promote this, in priority order

1. **An HTR-10 DLOFC fuel temperature history.** Everything here rests on
   HTR-Module's. The report's own three values for its peak already span a
   factor 3.7 in the released activity (§4.3), so this is the largest single
   uncertainty and it is not reducible by better modelling.
2. **The cooldown leg** — JRC Figures 15/74/158, or HTA-IB-03/90 Fig. 10,
   digitised. This replaces the two ESTIMATEd parameters
   (`τ = 60 h`, `T_late = 900 °C`). Their influence is bounded and **measured as
   exactly zero** on the released activity, because the venting model discards
   every cooling sample (`the_estimated_cooldown_parameters_cannot_move_the_release`
   runs five arms and gets one answer); they move PANAMA's failure accumulation
   only, and that term is 0.08 % of the answer. So this is wanted for
   completeness, not because a number depends on it.
3. **HTR-10 fuel-qualification data** — a measured free-uranium fraction,
   as-manufactured defect fraction or release fraction. A factor 3 sits in
   `f_hm` alone (§4.4), and without any HTR-10 measurement no code-to-data
   comparison for HTR-10 is possible at all.
4. **`T_B`, an HTR-10 *average* fuel temperature.** HTR-10 publishes a maximum;
   PANAMA needs an average, and 776 °C is HTR-Module's.
5. **A radial/axial temperature distribution**, to stop the whole core being run
   on the hot-node history (§2.3).
6. **A time-dependent `f_inc_acc` through the release chain.**
   `accident_release` takes one scalar, so the end-of-transient value is applied
   from `t = 0`, which over-predicts the early release. Reported both ways
   (`f_inc_acc` at the peak is 2.684·10⁻⁸ against 1.068·10⁻⁷ at 200 h, a factor
   4), and at 0.08 % of the answer it is currently immaterial — it would stop
   being immaterial above ~1800 °C.

## 7. Not verified, not validated

- **No measured HTR-10 failure fraction, free-uranium fraction or release
  fraction exists in this workspace's literature**, so no code-to-data
  comparison for HTR-10 was possible and none is claimed. This was re-checked
  rather than taken from the earlier search: the finding in
  `boon_lay::fuel_failure::htr10::qualification` §1 still holds.
- `sembawang`'s orchestration has **no upstream and no code-to-code
  verification**. The release physics it calls does (`boon-lay`'s fork against
  TRISO-ATOPS `de374c8`); the node loops, venting pairing and window assembly do
  not.
- PANAMA-I was validated on **German** TRISO over **1600–2500 °C**. Every number
  in this document at or below 1600 °C is an **extrapolation**, defensible by
  fuel lineage and by nothing else.
- The two PANAMA equation-level disagreements (#295) are unresolved and bound
  what any of this is worth.
