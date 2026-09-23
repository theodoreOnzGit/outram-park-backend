# CLAUDE.md — outram-mc-libs

Pure-Rust port of the OpenMC Monte Carlo neutron transport kernels.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openmc/`

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: k-eff within **500 pcm** of the ICSBEP Godiva
  bare-HEU-sphere benchmark (HEU-MET-FAST-001), reconstructed from an ENDF
  evaluation rather than a pre-built ACE library. Evidence class: **cross-code
  comparison** (this crate is a port of OpenMC's kernels), supported by unit
  tests and internal consistency.

  Measured at declaration: **k_eff = 0.99659 ± 0.00300, i.e. −341 pcm**, via
  `examples/endf_to_keff.rs` reading `n-092_U_235`/`n-092_U_238` from disk. **286 tests pass** (13 ignored).

  The bar is set at 500 pcm because that is what the crate demonstrably
  achieves today, not because 500 pcm is a good criticality tolerance — it is
  not. Expect this to tighten once the scatter matrix and unstructured-mesh
  tallies land.

- **2026-09-12 — evidence moved to `examples/godiva_keff_endf_local.rs`.
  The bar itself is unchanged at 500 pcm.** Maintainer decision.

  **Why.** Writing a V&V gate around the declaration exposed that
  `endf_to_keff.rs` *cannot test the bar it was cited for*. It runs
  3000 histories × 70 active generations, giving **σ ≈ 330 pcm**, so a 500 pcm
  bar is 1.5 σ wide. Gating that at 4 σ needs σ ≤ 125 pcm — roughly seven times
  the histories. On a re-run (2026-09-12) it gave **k_eff = 0.99327 ± 0.00329,
  i.e. −673 pcm**, *outside* the 500 pcm bar. Against the −341 pcm recorded at
  declaration that is 0.75 σ of combined statistics, so it is **not** a
  regression — but neither run can establish compliance either way. The
  evidence was too noisy for the claim resting on it.

  It is also a **two-nuclide** model (U-235 + U-238). Godiva's ICSBEP
  specification carries three; U-234 at 4.9184e-4 /b·cm is absent.

  **New evidence: `k_eff = 1.00057 ± 0.00173, i.e. +57 ± 173 pcm** — 0.33 σ from
  a benchmark that is an *experiment*, not another code. Via
  `examples/godiva_keff_endf_local.rs` on ENDF/B-VIII.0 from
  `reference-data/endf/`, all **three** ICSBEP nuclides, 5000 histories ×
  [40 inactive + 120 active]. At σ = 173 pcm the 500 pcm bar is 2.9 σ wide, so
  this run can actually resolve it. Suites: outram-mc-libs **350 passed / 0
  failed** (12 binaries), njoy-outram-park-fork **773 passed / 0 failed**
  (49 binaries).

  `endf_to_keff.rs` keeps a gate, but one sized to what its statistics can
  resolve, and its doc comment now says plainly that it is a tutorial and no
  longer the maturity evidence.

  **The bar was deliberately NOT tightened in this change**, though +57 ±
  173 pcm would support something nearer 200–300 pcm. Moving the evidence and
  moving the bar are separate maintainer decisions, and only the first was made.

- **2026-09-13 — the +57 ± 173 pcm evidence above is superseded. The bar itself
  is still unchanged at 500 pcm, and still holds.**

  **Why.** `+57 ± 173 pcm` was a **single seed's draw**, not the code's answer.
  A paired 96-seed study at that example's own settings (5000 histories ×
  [40 inactive + 120 active], all three ICSBEP nuclides, ENDF/B-VIII.0 from
  `reference-data/endf/`, single-threaded CPU; 57.6 M active histories per arm)
  measured the then-current code's true mean at **+228 ± 18 pcm**, seed-to-seed
  **sd 178 pcm**. That puts the recorded +57 at **−0.97 sigma**. Two single runs
  of that program differ by ~√2 × 180 ≈ 250 pcm from re-randomisation alone.

  **Current evidence: `+314 pcm, sem ±21, sd 205 pcm` over 96 seeds** — the same
  study run on HEAD, i.e. after the MT=91 continuum Q-value cap (gh:#192). The
  cap itself is worth **+85 ± 26 pcm (3.2 sigma)**: a physically correct fix
  that moves this case *further* from a measured criticality experiment. It is
  the honest statement of the tension, and it is stated rather than smoothed.

  **Against the 500 pcm bar:** +314 ± 21 pcm is **8.9 sigma inside** it on the
  pooled number, so the declaration stands with more margin than a single run
  suggests. It is outside the ICSBEP ±100 pcm band, and was before the cap too.
  The likeliest home for the remaining +314 pcm is the *shape* of the MT=91
  continuum law — a Weisskopf evaporation stand-in covering 10–25 % of Godiva's
  collisions — not its bound.

  `examples/godiva_keff_endf_local.rs` now records `RECORDED_PCM = 314.0` with
  the full method in its doc comment. **The bar was again deliberately NOT
  moved**; the remaining single-seed baselines elsewhere in the repo (Jemima,
  `godiva_keff_endf`, `endf_to_keff`) have not been re-measured, and the
  citation sweep plus the `assert_reproduces_keff` gate-sizing defect (it uses
  4 sigma of *one* run where independent arms need √2 sigma) are tracked in
  gh:#196 / `bn:op-awwi`.

- **2026-09-13 (same day, later) — evidence updated again to `+214 ± 20 pcm`.
  The bar is still 500 pcm and still not moved.**

  **What changed.** MT=91 (continuum inelastic) and MT=16 ((n,2n)) had been
  modelled with a **Weisskopf evaporation stand-in**, because RECONR gives MF=3
  magnitudes but no secondary-energy law. The evaluation's own
  `f₀(E→E')` — ENDF **MF=6 LAW=1** — is now read and sampled instead
  (`ContinuumEmission` in `njoy-outram-park-fork`), and the second (n,2n)
  neutron is an independent draw from it rather than a copy of the primary.

  **Worth, measured rather than asserted:** a paired 64-seed ensemble with the
  law switched off in one arm (`examples/godiva_mf6_continuum_ensemble.rs`,
  same settings as above) gives

  | arm | n | mean | sd | sem |
  |---|---|---|---|---|
  | MF=6 evaluated law | 64 | **+214 pcm** | 160 | ±20 |
  | Weisskopf stand-in | 64 | **+319 pcm** | 204 | ±25 |
  | **difference** | | **−105 pcm** | | **±32 (3.3 sigma)** |

  The stand-in arm's `+319 ± 25` reproduces the independently measured
  `+314 ± 21` above to 0.15 sigma, which is a check on the harness rather than a
  restatement of it.

  **Two corrections worth keeping on the record.** First, the predicted *sign*
  was wrong: the evaluated law is softer where MT=91 opens (`⟨E'/E⟩` 0.2095 vs
  0.2787 at 2 MeV on U-238), and the stated expectation was that this would cut
  leakage and push *k* up. It went down. In a bare fast metal sphere the
  spectrum-hardness terms — `ν̄(E)` and U-238 threshold fission — evidently
  outweigh the leakage term; that decomposition is a hypothesis, not a measured
  result. Second, the two physics fixes of this day pull opposite ways and both
  are correct: gh:#192's two-body cap moved Godiva **+85 pcm away** from the
  experiment, reading MF=6 moved it **−105 pcm back toward** it. Neither was
  chosen for its direction.

  **Against the 500 pcm bar:** +214 ± 20 pcm is **14 sigma inside** it. Still
  outside the ICSBEP ±100 pcm band, as it has been throughout.
  `examples/godiva_keff_endf_local.rs` records `RECORDED_PCM = 214.0`.


- **2026-09-15 — `+214` superseded by `+16 ± 11 pcm`. The bar stays at 500 pcm;
  moving it is a maintainer decision and has not been made.**

  **What changed.** Every inelastic collision drew `mu_cm = 2*prn − 1` —
  isotropic in the centre of mass — while elastic correctly used the
  evaluation's ENDF MF=4 tabulated cosine. The discrete levels are not
  isotropic (U-238 MT=51 has CM `⟨μ⟩` from `+0.033` at 1 MeV to `+0.510` at
  14 MeV; 39 of 40 levels carry anisotropic data on each of U-235 and U-238).
  Sampling them isotropically understates `⟨μ⟩`, inflates
  `Σ_tr = Σ_t(1 − ⟨μ⟩)`, suppresses leakage and raises `k`. Godiva is 55.8 %
  leakage. Wiring in MF=4/MT=51…90 is bead `op-tm9f`.

  **Current evidence: `+16 pcm, sem ±11, sd 173` over 256 seeds**, via the new
  `examples/godiva_keff_ensemble.rs` at the same settings as before (5000
  histories × [40 inactive + 120 active], all three ICSBEP nuclides,
  ENDF/B-VIII.0 from `reference-data/endf/`). That is a **−198 pcm** move from
  `+214`, against **−168 / −181 / −219 pcm predicted three independent ways
  before the work** — from the elastic ablation scaled by the `⟨μ⟩` budget,
  from the `k_eff`/`k_inf` leakage split, and from one-group diffusion on the
  measured `P_NL`. A prediction of sign and magnitude made in advance and then
  met is the reason to believe this, more than the size of what is left.

  **Against the 500 pcm bar:** `+16 ± 11 pcm` is 44 sigma inside it. **For the
  first time this case is also inside the ICSBEP ±100 pcm band**, which it had
  been outside of throughout its history.

  **Re-measured 2026-09-16 at 16 seeds: `+46 ± 44 pcm`**, 0.7 sigma from the
  256-seed `+16 ± 11` and 1.0 sigma from the benchmark. Both gates in
  `examples/godiva_keff_ensemble.rs` pass. Quote the 256-seed number; this one
  exists to show the result reproduces at a seed count someone will actually
  run.

  **The diagnosis was checked, not only the answer.** Four confirmations, the
  first two specified on `op-tm9f` before the work and the last two added when
  "is the physics right" was asked directly:

  - **`k_inf` must not move.** It uses a reflective boundary, so it has no
    leakage; a leakage fix has to leave it alone. Measured `2.26558 → 2.26587`,
    i.e. **`+13 ± 71 pcm`, 0.2 sigma** — consistent with zero while `k_eff`
    moved `−198 pcm`. Had `k_inf` moved materially, the `k_eff` agreement would
    be two errors cancelling rather than one removed.
  - **Direct ablation.** `examples/godiva_inelastic_anisotropy_ablation.rs`,
    32 seeds per arm: ANISO `+45 ± 32`, ISO `+269 ± 30`, difference
    **`−224 ± 44 pcm` (5.1 sigma)**. The ISO arm reproduces the old `+214 ± 20`
    to 1.5 sigma, which checks the harness.
  - **Per-level `⟨μ_cm⟩` against OpenMC's ACE**
    (`tests/inelastic_mubar_vs_openmc.rs`): worst **3.1e-3** over 32 points,
    differences random in sign. Both codes independently agree that U-235's
    MT=51/52/54/55 are near-isotropic while its MT=53/56 and all of U-238's low
    levels are not — the structural fact a naive parse would flatten.
  - **Aggregate `⟨μ⟩` and `Σ_tr`** (`tests/ablation_suite.rs`): `⟨μ_elastic⟩`
    0.2631 vs 0.2645, `⟨μ_inelastic⟩` 0.0236 vs 0.0245, and the **MT=91
    continuum share 0.3135 vs 0.3140** — the last being the one that makes the
    `⟨μ⟩` comparison meaningful at all, since the continuum scores `μ = 0` on
    both sides and dilutes the mean directly.

  **A correction to this study's own reference values, found by that check.**
  `transport_decomposition.py` read OpenMC's angular tables at the bracketing
  index rather than interpolating, which biased `⟨μ_el⟩` to 0.2740 (true 0.2645)
  and `⟨μ_inel⟩` to 0.0254 (true 0.0245), and `Σ_tr` to 0.37141 (true 0.37457).
  This is the **fourth** appearance of the nearest-point trap in this study and
  the first in our own script. The one-group diffusion price moves +219 → +209
  pcm against −198 measured, so the conclusion is robust — but a reference is
  not right merely for being external.

  **Four qualifications, none of which the number above should be read past.**

  1. **±11 pcm is our sampling uncertainty, not the comparison's.** ICSBEP
     quotes `1.0000 ± 0.0010`. Nothing on our side sees past a ±100 pcm band on
     the reference, so `+16` is agreement but is **not meaningfully better than
     `+80`**. Do not quote this as "16 pcm accuracy".
  2. **The physics underneath is not cleared.** The `~69 pcm` spectral residual
     found by the OpenMC cross-code study lives in `k_inf`, has no leakage
     component, and is untouched by this — our flux spectrum is still 0.45 %
     harder than OpenMC's in mean `E` (3.5 sigma), with 1.31 % less flux below
     300 keV (5.2 sigma) — **re-measured after the fix and unchanged**: mean `E`
     still +0.45 %, mean `ln E` still +0.05 %, exactly as a leakage-only fix
     predicts. `k_inf` against OpenMC is `+82 ± 55 pcm` after this
     change against `+69 ± 23 pcm` before — the same number within statistics,
     i.e. untouched. Tracked as `op-os8x`. **Two offsetting errors land on the


     **Re-measured again 2026-09-16 on HEAD, and LOCALISED.** Both codes were
     rebuilt in-session (NJOY2016 `ac5adf5f` → ACE → HDF5 → OpenMC 0.15.3
     `27e38e89`) and run 8 seeds a side: mean `E` **+0.42 %** (4.6 sigma), mean
     `ln E` **+0.05 %** (8.9 sigma), flux below 300 keV **−1.22 %** (6.0
     sigma). So `op-os8x` survives `op-og56` intact, as that change's own
     spectral ablation predicted it would.

     The per-bin comparison now says **where**: we carry **+0.88 % too much
     flux at 1.9–3.0 MeV** (4.7 sigma, 13.8 % of the flux) and **1.4–1.9 % too
     little at 67–174 keV**. That is a deficit of down-scatter out of the MeV
     window, and the only channel that moves a 2 MeV neutron to ~100 keV in one
     collision is **inelastic** — elastic off U-238 loses at most 1.7 % per
     collision. It is *not* the angular law (both `op-tm9f` and `op-og56` are
     in) and *not* the cross sections (≤ 0.06 % flux-weighted). The leading
     suspect was the MT=91 continuum `f₀(E→E')` shape.

     **That suspect is now measured and REJECTED (2026-09-17).** The transfer
     comparison had run on **U-238 only**, while Godiva is **93.7 % U-235**;
     `tests/mt91_transfer_vs_openmc.rs` now runs both, and over 1.5–10 MeV both
     agree with OpenMC to **0.0000 %** in `⟨E'⟩`, median and `P(E' < 300 keV)`
     — 37 rows on U-238, 25 on U-235. Exactly equal, not merely consistent. It
     is the fifth time in this study that an exclusion proved only as wide as
     the window it was measured in, and the first where the missing axis was
     the **nuclide**; the two laws are genuinely unalike (at 2 MeV U-235 puts
     18.4 % below 300 keV against U-238's ~30 %), so the substitution was never
     safe.

     Also checked and worth having: **no U-235 or U-238 discrete level has
     `|Q|` in 1.75–2.95 MeV** (39 levels each, MT=51–89; MT=90 is absent from
     both evaluations), so no discrete level can move a 2.5 MeV neutron to
     ~100 keV in one collision. That is what made MT=91 the leading suspect,
     and is why rejecting it leaves the **mechanism genuinely open** rather
     than merely narrowed. The suspect list is now competing-channel branching
     on U-235, **U-234** (which has never appeared in any oracle), the discrete
     levels' `Q` and selection probabilities, and the MT=16/17/5 spectra above
     ~6 MeV. Full record:
     `verification_and_validation/openmc_godiva_cross_code/README.md`.
     right `k` too**; what rules that out *here* is the `k_inf` check above, not
     the size of the residual.
  3. **The continuum angular correlation is still dropped** (MF=6 LANG=1/2,
     `op-og56`), as is the `EnergyAngular` interpolation flag. This fixed the
     *discrete* levels only.

     **Partly resolved 2026-09-16 — `LANG = 1` (Legendre) is now read and
     sampled; `LANG = 2` (Kalbach-Mann) is not.** The MF=6 parser was
     discarding `f₁ … f_NA` at parse time and never reading `LANG`, so every
     MT=91 and MT=16 neutron left isotropically. It now carries the
     coefficients, linearises them through the same routine MF=4 uses, and
     samples the cosine correlated with the outgoing-energy row actually drawn.
     `ContinuumAngular` distinguishes *evaluated-isotropic* from *unported*
     from *ablated*, which the old `0.0` could not.

     Measured on ENDF/B-VIII.0: 8652 of U-238's 8654 MT=91 rows carry
     `NA > 0`. But weighted by each row's own `f₀` the law is **exactly
     isotropic below 1.2 MeV** and only reaches `⟨μ_cm⟩ = +0.073` at 8.5 MeV
     and `+0.272` at 14 MeV — so unlike `op-tm9f`'s discrete levels this is a
     *high-energy* correction and the predicted worth on Godiva is small
     (recorded before measuring: down, well under 50 pcm).

     **Priced 2026-09-16 at `−38 ± 23 pcm`** (128 seeds per arm,
     `examples/godiva_continuum_anisotropy_ablation.rs`): ANISO `−26 ± 17`
     against ISO `+11 ± 15`. The prediction held on direction and magnitude,
     **but at 1.6 σ this is a bound, not a measurement** — consistent with zero,
     and excluding an `op-tm9f`-sized effect at 7 σ. Do not quote `−38` as the
     worth; quote it as *bounded below 70 pcm at 3 σ*. Resolving it to 3 σ needs
     ~400 seeds per arm. (An earlier 32-seed run gave `−41 ± 43`; its seeds are
     a **subset** of these, so it is superseded, not confirmatory.)

     The ISO arm independently reproduces the pre-`op-og56` `+16 ± 11 pcm` to
     **0.3 σ**, which checks the instrument. `RECORDED_PCM = 16.0` predates
     `op-og56` and this says the current mean is near `−22`; it is **not**
     changed on a 1.6 σ shift, and the drift gate built on it (~693 pcm for a
     single run) is nowhere near tripping.

     **A predicted sign that did not survive measurement.** This was recorded
     as hardening the spectrum (for a CM law `⟨E'_lab⟩` rises with `⟨μ_cm⟩`) and
     therefore moving `op-os8x` the wrong way. Measured 2026-09-16
     (`examples/godiva_continuum_spectrum_ablation.rs`, 8 seeds per arm): mean
     `E` **−0.076 % ± 0.109**, mean `ln E` **−0.008 % ± 0.008**, flux below
     300 keV **+0.157 % ± 0.226** — nothing resolved at 2 σ and every central
     value pointing the *other* way. The prediction is **not supported**, and
     the honest statement is a bound: below ~0.22 % in mean `E`, against
     `op-os8x`'s +0.45 %. **This law is not the explanation for `op-os8x` in
     either direction.** The discriminating follow-up is the same comparison
     under a reflective boundary, where only the per-collision term survives.

     **`LANG = 2` (Kalbach-Mann) landed the same day**, reusing
     `groupr::kinematics::bach` (already in the crate as an NJOY port) for the
     slope and inverting the Kalbach cumulative in closed form, one variate, so
     the ablation's RNG-stream invariant still holds. Verified against the
     density's own closed-form mean `⟨μ⟩ = r(coth a − 1/a)`. O-16's MT=91
     threshold is ~10 MeV, far above a fission spectrum, so this changes
     nothing for the reactor cases here — it closes the representation gap.

     Still dropped: the `EnergyAngular` interpolation flag, and `LANG = 11…15`
     (tabulated cosines), which no evaluation in `reference-data/endf/` uses on
     a neutron subsection.
  4. **One benchmark.** A bare fast HEU metal sphere, one geometry, one
     temperature. Says nothing about thermal systems or the crate's other cases.

  **`RECORDED_PCM` now lives in `examples/godiva_keff_ensemble.rs`** and is
  `16.0`. That example is new and closes a real gap: the previous `+214` and
  `+314` rested on pooled studies run ad hoc that **nothing in the repository
  could reproduce**. Its gates are sized off what a run can resolve rather than
  off the accuracy achieved — a `|mean| ≤ 30 pcm` gate would fail a correct
  build about a third of the time at the default 32 seeds.

  ~~`examples/godiva_keff_endf_local.rs` still records `RECORDED_PCM = 214.0`
  and is **stale**; re-pointing it is follow-up work.~~ **CORRECTED 2026-09-18
  — this claim was itself stale.** Verified on that date: the file records
  `RECORDED_PCM = 16.0` and `RECORDED_SEM_PCM = 11.0` (`:267`, `:269`) and
  gates through `RecordedKeff::pooled` (`:192`), i.e. it was re-pointed at the
  256-seed pooled result at some point after this paragraph was written. The
  follow-up it describes is **done**.

  It remains a **single-seed tutorial** whose own docs say it cannot resolve
  the 500 pcm bar — that part was and is true, and is why it gates against the
  pooled number rather than its own draw.

**Upstream license:** OpenMC is MIT-licensed. This Rust port is GPL-3.0-only
per the workspace default; the port constitutes new copyrightable expression.

## Standing goal: OpenMC-like API + notebooks-as-verification-tests (MANDATORY)

`outram-mc-libs` should **eventually function API-wise like OpenMC** (mirror its
Python / `capi` surface in idiomatic Rust), and **every notebook in
https://github.com/openmc-dev/openmc-notebooks becomes a verification test** for
the OUTRAM PARK Monte Carlo path. This is a durable direction, not a one-off.

- **This crate owns** the transport / geometry / tally / depletion /
  variance-reduction notebooks: `pincell`, `hexagonal-lattice`, `triso`,
  `candu`, `cad-based-geometry`, `unstructured-mesh-part-i/ii`,
  `tally-arithmetic`, `tally-power-normalization`, `expansion-filters`,
  `flux-spectrum`, `gamma-detector`, `post-processing`, `pandas-dataframes`,
  `mg-mode-part-i/ii/iii`, `depletion`, `shielded_room_weight_window`, `capi`.
- **njoy-outram-park-fork owns** the data notebooks (`nuclear-data`,
  `nuclear-data-resonance-covariance`, `search`, mgxs/mdgxs generation).
- Approach: a notebook→test→required-API **mapping doc**, then a
  `tests/openmc_notebooks/` harness — tractable notebooks as live tests, the
  rest `#[ignore]` with a documented "requires API X" reason + a per-notebook
  bead. `pincell`/Godiva k_eff (op-u6s.1) is the natural first live case.
- Notebooks are OpenMC-project open-source (MIT) — cite provenance
  (source notebook + commit) per RESPONSIBLE_USE.md; V&V docs state methodology
  **and** measured results. Tracked under beads epic **op-6tz**.

### Reference values + comparison outputs (MANDATORY)

- **For ALL outram-mc comparisons, reference values come from the openmc
  `.ipynb` files themselves** — the k-effective / tally results stored in the
  notebook cell outputs (fetch the raw notebook; cite the notebook + commit).
  Do not invent or approximate a reference; use the number the notebook printed,
  and match the notebook's geometry / material / data as closely as the
  available data allows so the comparison is apples-to-apples.
- **Write each comparison as a CSV** to
  `verification_and_validation/openmc_notebook_comparisons/` (one CSV per
  notebook; columns: date, notebook, case, our k ± σ, reference k ± σ, Δk (pcm),
  combined σ, σ-distance, data used on each side, stats note). That folder is
  **gitignored** — the CSVs are reproducible generated outputs, kept local, not
  committed. The interpretation/write-up still goes in the committed V&V docs +
  the relevant bead.

### Code-to-code verification: commit the OpenMC input scripts (MANDATORY)

**Whenever a V&V case runs OpenMC itself** to produce the reference — as
distinct from reading a notebook's stored cell output — **the exact Python
scripts that generated the OpenMC output must be captured in the V&V record**,
not merely cited. A cited-but-absent deck is a reference a reader cannot
reproduce or check.

- **Snapshot the scripts** used for that run into
  `verification_and_validation/<topic>/openmc_inputs/` (verbatim copies, as
  they were when the run was made — do not point at a mutable working tree).
  Carry the upstream `LICENSE`/notice with them; the maintainer's decks
  (`~/Documents/research/openmc_fuel_perf_project`, GitLab
  `theodore_ong/openmc_fuel_perf_project`) are BSD-3-Clause © 2024
  theodoreOnzGit — own work, GPL-3-compatible, so the copy is fine with the
  notice preserved.
- **Embed the driver script inline** in the `.md` (the top-level script that
  builds the model and calls `openmc.run()`) as a fenced ` ```python ` block,
  so the reader sees the geometry/materials/settings without opening another
  file. Larger factory/helper modules may stay as the committed snapshot and
  be linked by relative path.
- **State the provenance block** in the `.md`: the deck's source URL + commit
  hash, the OpenMC version and commit, the cross-section library + version, the
  chain file (if depletion), particle/batch/inactive counts, the Shannon-entropy
  convergence check, and the statistical uncertainty on every reported number.
- **`openmc_inputs/` is committed** (unlike the gitignored CSV outputs) — the
  scripts are small, and they are the reproducibility artefact for the
  reference side of the comparison.

The `.venv-openmc` + `openmcbin` setup for actually running these lives in the
workspace-root `CLAUDE.md` reference notes / session memory.

---

## Porting rule (mandatory) — mirror the canonical source, do not reinvent

**Every transport / physics / geometry behaviour in this crate must be ported
from the canonical OpenMC C++ source at `/home/teddy0/Documents/research/openmc/`
(`src/*.cpp`, `include/openmc/*.h`).** Before implementing anything, grep the
OpenMC source for the corresponding function and mirror its logic — do not
re-derive or reinvent physics that already exists upstream. Cite the reference
`file:line` in the Rust doc comment so a reader can diff against the original.

**Only when a behaviour is genuinely absent upstream** (e.g. the pebble-bed
`delta_tracking` / `stochastic_media` specialization) do you scaffold new parts
and build them out — and mark them clearly as new work, not a port.

Rationale: the crate's entire value is *fidelity* to OpenMC. Reinvented logic
silently drifts from the reference. The C++ is the source of truth; this crate is
a translation of it.

---

## Scope

### In scope
| Module | C++ source | What it does |
|---|---|---|
| RNG | `src/random_lcg.cpp` | LCG with O(log n) jump-ahead for particle splitting |
| Distributions | `src/random_dist.cpp` | Maxwell, Watt, tabulated samplers |
| Geometry / position | `include/openmc/position.h` | 3-D position and direction vectors (cm) |
| Geometry / surfaces | `src/surface.cpp` | Quadric CSG surfaces + distance/sense |
| Geometry / cells | `src/cell.cpp` | Boolean RPN region evaluation |
| Geometry / universes | `src/universe.cpp` | Universe nesting hierarchy |
| Geometry / lattices | `src/lattice.cpp` | Rect + hex lattice indexing |
| Geometry / geometry | `src/geometry.cpp` | `locate_particle`, `distance_to_boundary` |
| Particle state | `src/particle.cpp` | Phase-space state (r, u, E, wgt, seed, …) |
| Particle bank | `src/bank.cpp` | Fission site banking for k-eigenvalue |
| Material | `src/material.cpp` | Nuclide mixture, macroscopic XS |
| Nuclide XS | `src/nuclide.cpp` | Point-energy grid + log-log interpolation |
| Reactions | `src/reaction.cpp` | MT table, Q-value, secondary sampling |
| S(α,β) thermal | `src/thermal.cpp` | Thermal scattering law tables |
| Source sampling | `src/source.cpp` | External source: spatial/energy/angle |
| Tallies | `src/tallies/tally.cpp` | Filter composition + accumulator |
| Tally filters | `src/tallies/filter_*.cpp` | Cell, energy, material, universe, mesh |
| Scoring | `src/tallies/tally_scoring.cpp` | Flux, reaction rate, current accumulation |
| Transport loop | `src/physics.cpp` | `collision()`, `transport_history_based()` |
| Scattering | `src/physics_common.cpp` | Elastic, inelastic, CM-frame kinematics |
| Fission | `src/physics.cpp` | ν sampling, fission bank creation |
| Multigroup | `src/physics_mg.cpp`, `src/mgxs.cpp` | Group-averaged cross-section transport — ~~stub — pending~~ **CORRECTED 2026-09-22**: implemented in `src/physics/physics_mg.rs` (771 lines) as `Mgxs` / `MgxsLibrary` + `run_keff_mg`, exercised live by `tests/openmc_notebooks/mg_mode_part_i.rs`. ~~Still absent: anisotropic MG scattering (`src/scattdata.cpp`, `max_order`)~~ **CORRECTED 2026-09-22 (gh:#265)** — anisotropic MG scattering is ported: `src/physics/scattdata.rs` carries `LegendreKernel` (rejection sampling, upstream's 10 % bounding-box margin and its `f > 0` negativity truncation) and `TabularKernel` (`convert_legendre_to_tabular`, the clamp-and-renormalise path), and `Mgxs::with_legendre_scattering` wires them into `run_keff_mg`. Paired P0-vs-P3 worth on a bare 35 cm cube: **−4371 ± 44 pcm (99.6 σ)** at 128 seeds against **−4200 predicted before the run**, with the zero-leakage control at −1 ± 38 pcm (0.0 σ), and agreement with OpenMC's own MG mode within 1.3 σ on every arm (~~a 32-seed first pass gave −4308 ± 93~~ superseded by the 128-seed run, which it agrees with) — `verification_and_validation/mg_anisotropic_scattering/`. Still absent: MGXS *generation* from CE data (the `njoy-outram-park-fork` track) |
| Depletion | `src/chain.cpp`, `openmc/deplete/` | **Implemented** — CRAM `exp(A·dt)` burnup, `DepletionChain`, transmutation matrix, one-group operator (`src/depletion/`: `chain.rs`, `cram.rs`, `matrix.rs`, `operator.rs`); live one-group burnup test vs the `depletion` notebook |

### Out of scope (will NOT be ported)
- **ENDF nuclear data parsing** — `src/endf.cpp`, `include/openmc/endf.h`
- ~~**HDF5 I/O** — cross-section library loading; data arrives pre-loaded~~
  **CORRECTED 2026-09-22 (maintainer direction), and the claim moved again on
  2026-09-23.** HDF5 read *and* write is in scope **for the workspace**, and
  lives in `njoy-outram-park-fork`, "cos it deals with nuclear data"
  (gh:#270) — that crate depends on the pure-Rust `hdf5-pure`, so no C
  toolchain, Android or wasm problem.

  ~~writing does not exist anywhere in the workspace yet~~ — **that was true
  when written and is no longer.** `njoy-outram-park-fork::hdf5` now has
  `mgxs.h5` and nuclide `.h5` writers (both verified by **OpenMC reading them
  back and transporting**), a `cross_sections.xml` reader, and
  statepoint/summary/source writers.

  **The part that still holds for this crate:** it takes its data pre-loaded,
  captures run state (`physics::state_point`) and hands it to that codec at
  the **run boundary**. Its inner transport loop opens no file.
- **XML configuration parsing** — `src/xml_interface.cpp`
- **CMFD accelerator** — `src/cmfd_solver.cpp`
- **Random ray extension** — `src/random_ray/`
- **Photon/electron transport** — `src/photon.cpp`
- **Python/ctypes C API** — `openmc/lib/` Python package
- **Geometry overlap checker** — `src/geometry_aux.cpp` (overlap detection only; the core intersection logic is in scope)

**Brought INTO scope 2026-09-22 (maintainer direction), tracked under the
capability-parity epic gh:#257:**

- **Track files, state points and particle restart** — `src/track_output.cpp`,
  `src/state_point.cpp`, `src/particle_restart.cpp` (gh:#271). This crate
  captures the run state; the file codec is `njoy-outram-park-fork`'s
  (gh:#270), so the transport loop still does no file I/O itself.
- **Geometry plotting** — but **only** as a generated standalone matplotlib
  Python script (gh:#268). OpenMC's native rasteriser `src/plot.cpp` stays out
  of scope.

The other gaps that audit found are enhancements to things already in scope, not
scope changes: variance reduction (#258), the White/Periodic boundary-condition
defect (#259), curvilinear tally meshes (#260), the 16 missing filters (#261),
the 11 missing scores including the kinetics group (#262), tally triggers and
derivatives (#263), source types (#264), anisotropic multigroup scattering
(#265), depletion integrators and transfer rates (#266), stochastic volume
calculation (#267), and multi-temperature treatment (#269, P3 — NJOY is the
standing workaround).

---

## Design decisions

### Units: raw `f64`, not `uom`
Unlike `outram-foam-basic-lib` (which uses `uom` for thermophysics), this crate uses
plain `f64` throughout the inner transport loop.  Monte Carlo simulates billions
of particle histories; a single neutron transport simulation may call
`distance_to_boundary` and `xs_at_energy` O(10⁸) times.  `uom` quantity wrappers
add zero-cost abstraction in principle, but in practice the compile-time overhead
and ergonomic friction in deeply nested loops is not worth it.

Documented unit conventions (enforced by naming, not types):
| Quantity | Unit |
|---|---|
| Length | cm (OpenMC default) |
| Energy | eV |
| Cross-section | barn = 1 × 10⁻²⁴ cm² |
| Macroscopic XS | cm⁻¹ |
| Atom density | atoms / barn·cm |
| Temperature | eV (1 eV ≈ 11604 K) |
| Particle weight | dimensionless (1.0 = fully weighted) |

### No HDF5 dependency in this crate
Cross-section data is loaded externally and passed in by value or reference.
This crate is pure algorithmic: no file I/O, no XML, no HDF5.

**REFINED 2026-09-22 (maintainer direction).** This still holds *for the
transport loop*, and it is why the HDF5 codec was placed in
`njoy-outram-park-fork` rather than here (gh:#270). What changed is that the
workspace as a whole now does HDF5 **write** as well as read, and that this
crate will grow run-state output — track files, state points, particle restart
(gh:#271). The rule to keep is the seam: this crate produces and consumes
**structured data at the run boundary**, and something else turns that into
bytes on disk. `no file I/O in the inner loop` is the invariant; `no HDF5
anywhere in the workspace` never was one.

### Neutron-only initially
Photon and electron physics (`src/photon.cpp`) are deferred.  The `ParticleType`
enum reserves slots for them, but only `Neutron` transport is implemented.

### Parallelism: per-particle RNG streams
OpenMC's reproducibility guarantee relies on each particle having a completely
independent LCG stream obtained by jump-ahead.  This Rust port preserves that
design: `init_seed(id, offset, master)` derives a unique starting seed for each
particle.  The jump-ahead in `future_seed(n, seed)` is O(log n), implemented in
`src/rng/lcg.rs`.

### RNG goal: statistical correctness, NOT particle-for-particle parity

**Maintainer's decision, 2026-08-06.** This crate does **not** need to reproduce
OpenMC's random number sequence draw-for-draw. What it needs is for the
**statistics to be right**.

That distinction decides how RNG work is justified and tested here:

- **Do not** treat "our uniforms differ from OpenMC's" as a defect in itself, and
  do not add tests that pin our output to OpenMC golden values as an end in
  itself. A converged result agreeing with OpenMC *within statistics* is the
  standard, not bitwise agreement.
- **Do** treat statistical quality as a hard requirement. The generator must
  behave like a good uniform source in the ways Monte Carlo transport actually
  depends on — equidistribution, and no exploitable structure in the *tuples* a
  history consumes (position, then direction, then energy come from consecutive
  draws, so k-tuple structure matters, not just single-draw uniformity).
- **Do** keep the *stream separation* guarantees. Independence between particle
  streams is a statistical property, not a parity one, and it is the thing that
  makes a reported uncertainty mean anything. See `op-rbo`: a defect there left
  neighbouring histories reading near-identical streams, which barely moved the
  central value but made the quoted sigma meaningless.

This does **not** relax the porting rule above. Mirroring OpenMC remains the
default for *physics* — geometry, kinematics, cross-section treatment — because
fidelity is this crate's whole value. The exemption is narrow and applies to the
**bit-level output of the RNG only**, where matching upstream is a means to
statistical quality rather than the goal itself.

Practical consequence: where OpenMC's RNG design exists *for* statistical
quality — as the PCG output permutation does, see `op-jis` — port it, and gate it
with statistical tests rather than golden-value comparisons.

---

## Port reference (read on demand)

The full Rust-module → OpenMC C++ source map, the bottom-up porting order (with
per-module implementation status), and the prioritised test backlog all live in
**`docs/port-reference.md`**.

## GPU policy: CI is GPU-INDEPENDENT; detection is at run time (HARD RULE)

**Maintainer direction, 2026-09-19.** These are requirements, not preferences:

- **CI must never fail merely because no GPU is exposed** — Windows, Linux or
  macOS.
- **GPU availability is detected at RUN TIME**, by `gpu::probe()`, never by a
  build-time assumption.
- **CPU / reference tests are mandatory on every platform.** They are the
  trusted path and must run everywhere.
- **GPU tests are conditional** on an available, compatible adapter, and must
  `SKIP` cleanly without one.
- **`SplitPolicy::Auto` must fall back to CPU gracefully** when no GPU backend
  or device exists. `hybrid_without_a_gpu_runs_everything_on_the_cpu` is the
  test that pins this; do not weaken it.

Verified 2026-09-19 by running the suites with the Vulkan loader hidden
(`VK_ICD_FILENAMES=/nonexistent VK_DRIVER_FILES=/nonexistent`): every GPU test
prints `SKIP` and passes, and nothing else changes.

### A software ICD is a LOCAL convenience, never a CI requirement

Because every `gpu::*` test skips without an adapter, a host with no Vulkan
driver runs the whole GPU suite green while executing none of it. That is
correct for CI and useless for development — so when you want the shaders to
actually run *locally*, install Mesa's software rasteriser:

```bash
apt-get update && apt-get install -y mesa-vulkan-drivers   # provides lvp_icd.json
```

`gpu::probe()` then returns `llvmpipe (LLVM …)` and the WGSL genuinely
executes. **Do not make this a CI dependency** and do not write a test that
requires it.

**This is not hypothetical value.** Doing it on 2026-09-19 immediately exposed
`hybrid_splits_work_across_both_devices` as broken: it asserted
`SplitReason::Split` whenever *any* adapter existed, while `SplitPolicy::Auto`
correctly returns a zero GPU share for `DeviceClass::Cpu` — handing work to a
software rasteriser costs buffer copies to run the same arithmetic on the same
cores. The library was right and the test was wrong, and the test had **never
executed on any host**. See `bn` for the record.

**Two things follow for anyone adding a GPU test here.**

- **Do not assert on `SplitPolicy::Auto`'s choice** unless the test also
  handles a CPU-class adapter. Force the share with
  `SplitPolicy::GpuFraction(..)` when the point is to exercise the split
  machinery rather than the heuristic.
- **A software adapter is `DeviceClass::Cpu`, not `Virtual`.** Code that
  branches on device class will take the CPU path on it, which is correct and
  is also why such a test can pass vacuously.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check --release -p outram-mc-libs --lib
cargo test  -p outram-mc-libs --lib --release
```

## Porting workflow (mandatory)

After implementing any module, update `src/prelude.rs` with new public items,
then `cargo check -p outram-mc-libs` to verify.
