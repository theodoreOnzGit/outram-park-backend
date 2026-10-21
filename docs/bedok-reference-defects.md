# BEDOK — defect register for the reference snapshot

**Status:** stage-1 translation complete (all four modules reported 2026-08-05;
the flux-solver entries D1-D7 added 2026-08-13; G1/G2 2026-08-18; Z1 2026-08-18;
G3 2026-08-21). Every runnable case has been verified against the running
MATLAB, so **stage 2 — corrections — is open as of 2026-08-21.**
**Parent:** the crate README, "Translation policy".

**Corrections landed so far.** Each is behind a switch on `Params`, and
`Params::reference_faithful()` turns all of them off.

| Entries | What | Switch | Landed |
|---|---|---|---|
| **G1, G2, G3** | The diffusion face coupling and `gradterms` | `Params::gradd_form`, default `GradDForm::Conservative` | 2026-08-21 |
| **T5, T6** | The W-3 `K4` subcooling enthalpy | `Params::w3_form`, default `W3Form::Published` | 2026-08-21 |
| **C2, T4** | `highy = ix` in the hottest-channel search | `Params::hot_channel_search`, default `HotChannelSearch::Correct` | 2026-08-21 |
| **T9, T13** | `fueltempavg` aliased to the Doppler temperature | `Params::fueltemp_average`, default `FuelTempAverage::VolumeWeighted` | 2026-08-22 |

Everything else in the register below is **still translated as-is and not
fixed.**

Defects found in Than Yan Ren's MATLAB while translating it. The snapshot was
handed over unfinished, so some of these are work in progress rather than
mistakes — the port cannot tell which, and does not try to. Each is
**translated as-is** and recorded here.

> **Corrections are stage-2 work.** Decided 2026-08-05.

---

## How a correction differs from a substitution

Stage 2 contains two kinds of change, and conflating them would break the one
gate the whole plan rests on:

| | Substitution | Correction |
|---|---|---|
| What it is | Swap a component for an OUTRAM PARK library | Fix a defect in this register |
| Effect on results | **Must not change them** | **Deliberately changes them** |
| How it is validated | Parity against stage 1, within tolerance | Cannot use parity — stage 1 is the thing being corrected |
| What justifies it | Reproducing the reference | Benchmark agreement, or a physical/derivational argument |

So a correction **cannot be signed off by the parity gate**. It requires, per
entry: the before and after numbers, what changed and why, and a justification
that does not appeal to the reference. After a correction lands, that
component's parity baseline is **re-recorded** against the corrected behaviour,
and the entry below is marked with the date and the commit.

Do corrections **one at a time**, and never in the same change as a
substitution. A component that is simultaneously substituted and corrected has
no interpretable gate at all.

---

## Register

Severity is about consequence if left standing, not about how wrong the code
looks.

> **Two sections share the `T` prefix and both number from `T1`.** The
> `th/` sections reported 2026-08-05 (the coupled thermal-hydraulic path) and
> 2026-08-13 (the rod-conduction and CHF files) each run `T1` upward, so **nine
> ids — `T1` to `T9` — name two different defects each**. `T1` is both the gap
> ring pinned at 1 K and the halved W-3 upwind enthalpy; `T7` is both the
> `Tsat - 2*eps` no-op and the 1 K gap dummy.
>
> **Always cite a `T` id together with its file or its section**, in code
> comments as well as here. This has already caused one miscitation. Renumbering
> the second section is the real fix and is a maintainer decision, since it
> means rewriting citations across the crate.


### From `coupling/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| C1 | `sigmavalupd3d_handler.m:56-68` | **`rodlvl` is never initialised** when a bank tip sits at or above the top of a column — i.e. a fully withdrawn bank, *the end state of every rod-ejection transient*. MATLAB then silently reuses the previous `(ix,iy)` column's value. Translated with the same carry-over; a first-column occurrence returns an error. | ~~**High** — hits the primary NEACRP use case~~ **LATENT, measured 2026-08-21.** Full withdrawal (228 steps) puts the tip at 401.2 cm in a column meshed to 428 cm — the NEACRP model has an axial reflector above the core. It would take ~245 steps to trip, 17 beyond the mechanism's full-out position, so the branch is **dead in every case in the snapshot**, steady and transient. Still a real fault for a shorter column or longer travel; not correctable today because no number would move. |
| C2 | `w3chfhottest.m:22` | **`highy = ix`** where `iy` is meant. The hottest-channel search can only ever return a diagonal column. | **High** — silently wrong CHF location  **CORRECTED 2026-08-21** (with T4, `th/` 2026-08-05) |
| C3 | `thdiffusion_solverxyz.m:191` | **CHF result is computed and never returned** — not placed in `output`. | Medium — dead work, no wrong answer  **ALREADY ADDRESSED.** The port **returns** the CHF the reference discards — `CoupledOutput::chf` and `chf_channel`. Recorded in the crate README as a deliberate departure, on the same reasoning as the CSV policy: the computation is unchanged and the caller gains the answer. It is also what made defects T5/T6 and C2/T4 measurable at all, since a discarded number cannot be compared to anything. |
| C4 | `criticalboron_xyz.m:150` vs `thdiffusion_solverxyz.m:159` | **Inconsistent `whichsigma` into the T-H solver**: one passes compositions, the other the compacted per-node map. One of the two must be wrong. | ~~**High** — needs resolving before either path is trusted~~ **RESOLVED 2026-08-21: the premise is false, neither is wrong.** `th_solverxyz` reads `whichsigma` only through `== 0` (to skip an unfuelled column), and `sigmavalupd3d`'s compaction writes `0` exactly where `whichsigmaref` is `0` — the one property the consumer reads is the one the transformation preserves. Measured: the two maps differ at **3976 of 5202** nodes on A2 and **3483 of 4046** on D1, and `th_solverxyz`'s fuel temperature, coolant temperature and heat flux are **bit-identical**. Both paths can be trusted. Not unified in code — that would change no number and would erase a faithful trace of the reference. |
| C5 | `fixinfnan.m`, applied after every solve | **Turns divergence into a zero flux**, hiding a failed solve from the residual norms. | **High** — masks failure  **CORRECTED 2026-08-22** — the substitution is now counted and surfaced (`fixinfnan_counted`, `SaNodalOutput::non_finite_substitutions`). The patch itself is byte-for-byte the reference's, so no result moves; what changes is that a caller can tell a solve blew up. Measured **0 substitutions** on IAEA-3D, NEACRP A2 and NEACRP D1, so none of the crate's benchmark eigenvalues rests on a patched flux — previously an untestable assumption. |
| C6 | `pauseonnan.m` | `if any(isnan(M))` on a matrix is true only when *every column* contains a NaN. A single NaN, or a whole NaN row, passes silently. Reproduced exactly, with a test. | Medium  **CLOSED 2026-08-23 — reproduced deliberately, and superseded in practice.** MATLAB's `any(isnan(M))` on a matrix is column-wise, so a single NaN passes silently. Reproduced exactly with a test that asserts the *wrong* behaviour, because a caller comparing against the MATLAB needs the same blind spot. **The gap it leaves is now covered from the other side**: defect C5's correction counts every non-finite the flux solve produces (`SaNodalOutput::non_finite_substitutions`), so a NaN that `pauseonnan` misses is caught anyway. |
| C7 | Phase-2 re-equilibration (5000 its), `eigsolvecold` (8000 its) | **Silent non-convergence** — exit on the iteration cap with no error and no flag. | **High** — a non-converged answer is indistinguishable from a converged one  **CORRECTED 2026-08-22** — both capped loops now report a verdict. `eigsolve_cold` returns a `ColdSolveVerdict` (converged, final residuals) and the count of abandoned solves reaches the caller as `BoronOutput::cold_solves_not_converged`; the transient's Phase-2 re-equilibration reports `reequilibrate_converged` and `reequilibrate_residual`. The iterations are unchanged. |
| C8 | `sigmavalupd3d_handler.m` `fuelmask` | Hard-codes `whichsigmaref >= 4`, the NEACRP composition numbering. Any other case silently gets wrong C2/C3/C4 outputs. | Medium — case-specific  **CLOSED 2026-08-23 as case-specific, not correctable.** It is wrong only for a case the snapshot does not contain, and there is no second case to define what right would be. Translated as written, documented, and pinned. `fuelmask`'s `whichsigmaref >= 4` is the NEACRP composition numbering; the snapshot contains only NEACRP cases plus IAEA-3D, which has no T-H. |
| C9 | C5/C6 outputs | Hard-code the NEACRP 18-block axial model (layers 6 and 13, offset `L*zscale`). Meaningless for a differently blocked case; `radialmaplayer` will index past the mesh. | Medium — case-specific  **CLOSED 2026-08-23 as case-specific, not correctable.** It is wrong only for a case the snapshot does not contain, and there is no second case to define what right would be. Translated as written, documented, and pinned. The C5/C6 radial maps hard-code the NEACRP 18-block axial model. Both NEACRP cases use it, and `radialmaplayer` is not reached by any other. |
| C10 | Transient driver, `params.steadyfile` | **No validity check** on the cached steady state, though its own header says the cache must match the current case and params. (`criticalboron_xyz` at least sanity-checks the cached `k_eff`.) | Medium  **CANNOT ARISE HERE, verified 2026-08-23.** The `.mat` steady-state cache is **not translated**: `params.steadyfile` becomes an explicit argument, so the caller passes the state in rather than the driver silently loading a file that may not match the case. A deliberate departure recorded in the crate README. There is no cache to validate, so the missing validity check has nothing to guard. |
| C11 | Transient operators | **`Nc > 0` does not conform** — the steady flux is `philen` long, the transient operators `philenf`. All four cases set `Nc = 0`, so it is untested. | Low — untested path  **LATENT, verified 2026-08-23.** Every case in the snapshot sets `Nc = 0` (`neacrpa2.m`, `neacrpd1.m`, `iaea3ds.m` all checked), so this branch is unreachable. It stays recorded as a trap for whoever first sets `Nc > 0`. |
| C12 | `fixnegativematrix.m` | Operates on `find(mat)`, i.e. only stored non-zeros. Harmless for dense arrays; a trap if these become sparse. | Low  **LATENT, verified 2026-08-23.** The `find(mat)`-only behaviour belongs to the **sparse** `fixnegativematrix`, which has no call site in the port. The live path (`sigmavalupd3d_handler`) calls `fixnegativematrix_dense`, where every entry is visited and the defect cannot arise. The sparse form is kept, translated as-is, for the trap the entry warns about. |
| C13 | `sigmavalupd3d_handler.m:71` | Writes `rodfrac.csv` **unconditionally** inside the feedback update — once per Picard pass per time step. | Low — I/O noise  **ALREADY ADDRESSED**, by the same policy as D3: `rodfrac.csv` is not written. The rod-fraction map is returned as `RodFraction` from `sigmavalupd3d_handler`, so the once-per-Picard-pass-per-time-step I/O does not happen. |
| C14 | `thdiffusion_solverxyz.m:209-211` | Fuel-temperature history trim reports `Inf` as the residual when the loop breaks before any T-H update. | Low  **CLOSED 2026-08-23 — the port already reports it honestly.** The reference's `Inf` residual on a break before any T-H update is not hidden here: `CoupledOutput` carries `fueltemp_converged` separately from `termination`, so a caller can tell an `Inf` residual from a converged one. The reference prints `[NOT converged]` and nothing else. |

### From `cases/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| K1 | `neacrpa1t.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpd1.m` — `geometry.fuel.Vi` | **Fuel node volumes are identically zero for nodes 2:20 in all four NEACRP cases.** `rminus=sum(Lr(i-1))` and `rplus=sum(Lr(i))` are single-element sums, so they give node *thicknesses*, not cumulative radii; with uniform pellet nodes the annulus area comes out zero. Reproduced as-is; `node_area_corrected()` computes the intended annuli **for comparison only**. | ~~**High** — fuel volumes feed the rod thermal solve~~ **LATENT, measured 2026-08-21.** They do not: `geometry.fuel.Vi` has **no consumer** in the snapshot. Neither `fuelrodheat_1dcylnd`, `fuelrodheattime_1dcylnd`, `th_solverxyz` nor `th_solvertimexyz` reads it; the steady and transient rod solves build their own coefficients. Correcting it would move no number. |
| K2 | `convertindexc2d.m`, half-index → nodal branch | **Never round-trips.** `ceil(…)/2` and `(…+1)/2` produce half-integers for *every* index the forward map generates (1 → 2 → 2.5). MATLAB would fail inside `sparse`. Confirmed numerically; the port returns an error. | Medium — dead in the four cases, a trap if used  **LATENT, verified 2026-08-23.** The non-round-tripping map is `IndexMode::DiamondDifference`, which appears **only inside `convertindexc2d` and its own tests** — no solver reaches it, and every reference call site passes four arguments, fixing the mode to 1. Pinned by a test that asserts the round-trip failure rather than hiding it. |
| K3 | `convert_grid3d.m`, `Nc` branch | `(G+Nc-1)*energyindexstep` inside `for nn=1:Nc` gives every extra unknown the *same* index — a typo for `G+nn-1`. Dead, since `Nc=0` everywhere (cf. C11). | Low — untested path  **LATENT, verified 2026-08-23.** Every case in the snapshot sets `Nc = 0` (`neacrpa2.m`, `neacrpd1.m`, `iaea3ds.m` all checked), so this branch is unreachable. It stays recorded as a trap for whoever first sets `Nc > 0`. |
| K4 | `handle3dcoords.m` generic branch | Sets `maxi3 = params.maxix` — typo for `maxi3`. `handle2dcoords.m` has no default branch at all. | Medium  **LATENT, verified 2026-08-23.** The `maxi3 = params.maxix` typo is in `handle3dcoords`'s **generic** branch, reached only when a case populates `maxi1/maxi2/maxi3` rather than `maxix/maxiy/maxiz`. **No case in the snapshot does** — all four set the Cartesian fields. Pinned by a test that asserts the typo. |
| K5 | `convertindexc2d.m` | Reads `params.maxi1`/`maxi2` directly, while its only caller goes through `handle2dcoords`. A `params` carrying `maxix`/`maxiy` satisfies the caller and then errors in the callee. | Medium  **LATENT, verified 2026-08-23.** Same generic branch as K4: `convertindexc2d` reads `maxi1`/`maxi2` directly while its caller resolves through `handle2dcoords`. Unreachable for every case in the snapshot, and doubly so because `convertindexc2d`'s only non-trivial mode is itself dead (see K2). |
| K6 | `neacrpd1.m` | **Header says "Case D2"** while the file builds D1. | Low — but misleading  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The port's `neacrpd1` module docs state plainly that the file builds **case D1** despite the header, and the case data was checked against NEACRP-L-335 section 6.2, which defines D1 as the inlet cold-water transient. |
| K7 | `neacrpd1.m` | `sigmavalues.fp` **entirely commented out**; `neacrpd1t.m` rebuilds it. The reference's own repair, ported as written. | Medium  **CLOSED 2026-08-23 — this is the reference's own repair, not a defect to fix.** `neacrpd1.m` comments out `sigmavalues.fp` and `neacrpd1t.m` rebuilds it, so the transient case supplies what the steady case omits. Ported as written; D1t runs correctly and matches the MATLAB over its **full 20 s window** to 2.9e-11, which is the evidence that the rebuild is sufficient. |
| K8 | `neacrpd1.m` | `params.boron = 1000` with **no boron table** to interpret it. | ~~Medium~~ **DOWNGRADED 2026-08-22 to Low.** The BWR specification's cross-section formula (NEACRP-L-335 section 5.3) has **no boron term**, so no table is missing — see K9. What remains is a vestigial assignment that no BWR path reads: tidiness, not a missing feedback path. |
| ~~K9~~ | `neacrpd1.m` | ~~**No coolant-temperature feedback**, and the control-rod section is a header with an empty body.~~ | ~~**High** for D1~~ **WITHDRAWN 2026-08-22 — not a defect.** Checked against the specification itself, **NEACRP-L-335 (Rev. 1)**, Finnemann and Galati, OECD/NEA, Oct 1991 (Jan 1992). Section 5.3 defines every BWR cross section as `Sigma = Sigma_0 + Sigma_rho*(rho - rho_0) + Sigma_T*(sqrt(T) - sqrt(T_0))` — **two derivative terms, water density and sqrt(Doppler temperature), and coolant temperature is not among them**; section 5.8 repeats it in prose. The rod half resolves the same way: section 5.3 says the compositions "take into account all the materials present in the core (fuel, coolant, structures, **control** and burnable absorbers)", so a BWR absorber sits *inside* the composition, unlike the PWR half of the same benchmark which carries a separate `DeltaSigma_CA` overlay (Tables 2.5.1/2.5.2). `neacrpd1.m` implements the specification correctly. Pinned by `neacrpd1::tests::k9_the_feedback_channels_match_the_benchmark_specification`. |
| K10 | `neacrpd1.m` | `th.flowrate` assigned twice; the first is dead. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The dead first assignment of `th.flowrate` is simply not translated twice; the live value is the second. |
| K11 | `geometry_ends3d.m` | Reports the full axis for an all-void line, and finds only the **first** contiguous run. | Medium  **CLOSED 2026-08-23 — documented, pinned, and load-bearing.** `geometry_ends3d` finding only the first contiguous run per line is a real limitation, and it **interacts** with `makegradDxyz`'s identity-diagonal defect (a fuelled node past an internal gap keeps a spurious `+1`). Both are pinned by tests. Correcting it would change the operator on any case with an axial material gap; **no case in the snapshot has one**, so there is no before/after to justify the change. |
| K12 | `th.gtube` | Not divided by the refinement factors, while `th.nfuelpin` is. | Medium  **NOT TRANSLATED, verified 2026-08-23 — moot for this port.** `th.gtube` has **zero references anywhere in the crate**; it is not carried on `Th` and no solver reads it. The refinement division the entry contrasts it against *is* carried — `th.nfuelpin` is `264/4` for A2 and `196/4` for D1 — so the asymmetry exists in the reference but has no expression here. If `gtube` is ever added, it must be divided by the same refinement factors. |
| K13 | `hydia` | Wetted perimeter `2*pi*R + 4p - 8R` is non-standard. | Medium  **LIVE, quantified 2026-08-23 — and a maintainer decision, with new evidence.** The perimeter `2*pi*R + 4p - 8R` adds `4(p - 2R)`, four times the rod-to-rod gap: an imaginary boundary between neighbouring subchannels that carries no shear and is not wetted surface. It makes the hydraulic diameter **~30% too small** (A2 0.8403 vs a standard `4A/(2*pi*R)` of 1.1943 cm; D1 1.2177 vs 1.7002). **It is live in two independent places:** `th_solverxyz`/`th_solvertimexyz` **recompute** it for `Re`, `Nu` and `hcoeff` on every coupled pass (defect T12 — they recompute rather than read, so the stored and used values could diverge), and `w3chf` **reads** `fuel.hydia` for its `K3` factor. Effect: **+7%** on the heat-transfer coefficient (weak, `hcoeff ~ D^-0.2`) but **+23% on `K3`, hence +23% on the predicted CHF** — compounding with T5/T6's +22.7%, in the same non-conservative direction. **The specification does give a hydraulic diameter** — NEACRP-L-335 Table 5.23: **1.4730 cm** for the BWR, with flow cross-section 400.78 cm2 and heated perimeter 880.5256 cm (`= 196*pi*1.430`, confirmed). **But it is a different control volume**: the spec's is the *macroelement*, the code's is a *per-rod subchannel*, and the two flow areas do not match either (196 x 1.9096 = 374.27 cm2 against the spec's 400.78). So the spec does not settle the code's value directly — it settles that a defensible target exists, and reconciling the control volumes is the work required. **Not corrected**: changing it moves CHF and the heat-transfer coefficient on both PWR and BWR cases, and the right target depends on which control volume the model is meant to represent — a maintainer call. |
| K14 | `convertsparsekey3d.m` | Diagnostic hard-codes a 19x17x17 grid. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The hard-coded 19x17x17 decode is a **diagnostic print**, not a computation. |
| K15 | `fixinfnan.m` | Its "min over finite values" is only true because MATLAB's `min` skips NaN — accidental, not intended (cf. C5). | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. MATLAB's `min` skipping NaN makes the 'min over finite values' true by accident; the port implements it explicitly as min-over-finite, so the behaviour is identical and no longer accidental. |
| K16 | `geom2dxycase1.m` | `constants.chi=[1;1]` is per-material, not materials x G; the two coincide only at `G=1`. | Low  **LATENT, verified 2026-08-23.** `constants.chi = [1;1]` being per-material rather than materials-by-G only matters at `G > 1`, and it belongs to `geom2dxycase1` — a **2-D** case that no solver in the snapshot can run (its own call site is commented out in `main_exec_diff3d.m`). The port returns it as a purpose-built `Case2d` precisely so it cannot be fed to a 3-D solver by accident. |
| K17 | `iaea3ds.m` | Computes its scale factors **after** forcing the grid, so they are permanently 1. | Low — inert  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. `iaea3ds`'s scale factors are computed after the grid is forced and are therefore permanently 1; the port carries them as such, and IAEA-3D reproduces the published `k_eff` to -1.1 pcm regardless. |

### From `th/` — the coupled thermal-hydraulic path (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| T1 | `fuelrodheat_1dcylnd.m` / `…time` | **The gap ring is an orphan row fixed at `T = 1 K`.** Its diagonal is never overwritten from the identity seed and no other row references that column — the pellet surface couples straight across to the clad inner surface (`laplccol = id+2`). Harmless only because `th_solverxyz.m:185` clamps the profile afterwards. Pinned by a test. | **High** — a physically meaningless gap temperature, masked downstream  **CORRECTED (made avoidable) 2026-08-23.** The 1 K value is **left in place deliberately** — the gap is an unresolved void with no temperature to compute, so interpolating would invent a plausible-looking number the solve never produced (worse, because unnoticeable), and dropping the row would change the vector length and every caller's `maxid` arithmetic. Instead the trap is now avoidable: `fuelrodheat_1dcylnd::gap_dummy_unknowns` locates the dummy rows by replaying the solver's own `ir`/`id` walk, `without_gap_dummies` strips them, and `Th::fueltemp`'s doc warns at the point of use. Measured on the NEACRP rod: the dummy sits at unknown **6**, and including it pulls the radial mean down **84 K (-10.0%)**. |
| T2 | same | **A rod with no material→gap transition indexes past its own matrix.** With no gap, `surfcount = 0` so `maxid = maxir`, yet the `ir == maxir` branch writes column `maxid+1`. Hard error in MATLAB; raised as `UnsupportedRodLayout` here. | Medium — unreachable in the benchmark geometries  **LATENT, verified 2026-08-23.** Needs a rod with **no** material-to-gap transition. Both NEACRP rods set `gapn = 1` (`neacrpa2.m` and `neacrpd1.m` checked), so `surfcount >= 1` always and the out-of-bounds `ir == maxir` branch is unreachable. Recorded as a trap for a gapless rod. |
| T3 | same | **A material→material interface with no gap assembles no conduction coefficient.** A direct fuel→clad interface sets `surf = 1`, but the `surf` branch only acts when `whichk(ir+1) == 0`, so no off-diagonal is added and `kplus` keeps a stale value: the diagonal becomes `2*kplus_previous` and the two sides are uncoupled. | Medium — unreachable in the benchmark geometries  **LATENT, verified 2026-08-23.** Needs a direct material-to-material interface with no gap between. Both NEACRP rods interpose a gap (`gapn = 1`), so the `surf` branch always finds `whichk(ir+1) == 0`. Same reachability argument as T2. |
| T4 | `w3chfhottest.m:21` | `highy = ix` rather than `iy` — same defect as C2, found independently. | **High**  **CORRECTED 2026-08-21** (with C2) |
| T5 | `w3chf.m` | **`enthshift` is not the inlet enthalpy the W-3 `K4` factor requires.** It builds a local two-node average `(0.5*enth(i)+0.5*enth(i-1))/2` with a stray extra `/2`, roughly doubling the apparent subcooling. Only `enthshift(1)` is the true inlet value. | **High** — wrong CHF magnitude  **CORRECTED 2026-08-21** (with T6) |
| T6 | `w3chf.m` | That `i-1` walk **runs over the flat index, not along a channel**. Since `iz` varies fastest, the first node of every channel mixes in the top node of the *previous* channel. | **High**  **CORRECTED 2026-08-21** (with T5) |
| T7 | `singleflow1devap.m:124,159` and elsewhere | **`Tsat - 2*eps` is a no-op in f64.** At `Tsat ~ 618 K` one ULP is ~1e-13, three orders above `2*eps`, so the intended "nudge below saturation" never happens — in the original either. A live parity risk for the IF97 substitution (§3). | Medium  **CLOSED 2026-08-23 — a no-op in BOTH codes, so it is faithful by construction.** `Tsat - 2*eps` cannot move a double at 618 K (one ULP is ~1e-13, three orders above `2*eps`), so the intended nudge below saturation never happens in the reference either. The port reproduces the same no-op, which is why D1 and D1t match the MATLAB exactly through boiling. **Correcting it would introduce a nudge the reference never applies** and move every BWR result away from parity, with nothing external to justify the change. |
| T8 | `singleflow1devap.m:105` | Comment says "steam at 900 K"; the code passes 1050 K. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The comment says 'steam at 900 K' where the code passes 1050 K; the code is translated, the comment is corrected in the port's doc. |
| T9 | `th_solverxyz.m:192` | Sets `fueltempavg := fueltempdoppler`; the actual volume average is commented out at line 189. | Medium | **CORRECTED 2026-08-22** (with T13, its transient twin) |
| T10 | `driftflux6_solverstatic3d.m:129` | Computes "quality" by dividing by the latent heat while subtracting the *local* liquid enthalpy, so it is not the equilibrium quality outside saturation. | Medium  **CORRECTED 2026-08-23** — `Params::quality_form`, default `QualityForm::Equilibrium`. The two-fluid path now computes `clamp((h - h_l,sat)/h_fg, 0, 1)`, **the same function `singleflow1devap` computes on the HEM path** — verified identical to `0.000e0` over a sweep spanning subcooled liquid to superheated vapour at 6.7 MPa, where the old form was off by up to **0.133** and produced values outside `[0, 1]`. Justified without a benchmark: W-3's K1/K2/K3 are defined against the equilibrium quality, and the sibling solver already computed it, so this is a derivational plus internal-consistency argument. **NEACRP D1's `k_eff` is unmoved** (1.0429504111 both), the quality going from 4.70e-11 of floating-point residue to the exact zero a subcooled channel should report. The correction matters **prospectively**: the two-fluid path's 1-D kernel is missing, and supplying it would send the old form's unclamped quality straight into the CHF correlation. |
| T11 | `th_solverxyz.m:149` | **`real(x^p)` on a negative base.** MATLAB returns `abs(x)^p * cos(p*pi)`, not `NaN`. Reproduced exactly via `matlab_real_powf` — this would otherwise have been a silent behavioural divergence. | Medium — a translation trap, not a reference bug  **CLOSED 2026-08-23 — reproduced deliberately; this is a language-semantics requirement, not a defect to repair.** MATLAB's `real(x^p)` on a negative base returns `abs(x)^p * cos(p*pi)` rather than `NaN`, and the port reproduces it exactly through `matlab::real_powf`. Departing would silently change results wherever a negative base arises, and the reproduction is what makes the feedback handler match the MATLAB to 3.3e-13. |
| T12 | `th_solverxyz.m`, `w3chf.m` | Dead code: `Vi = repmat(...)`, `subflow`, `Lx`, `Ly`, `Lr`, `maxir`, `whichg`; `subarea`, `gearth`. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. Dead reads (`Vi`, `subflow`, `Lx`, `Ly`, `Lr`, `maxir`, `whichg`, `subarea`, `gearth`) — residue of a commented-out inline conduction assembly. They are **not parameters** of the port's function, which is why the signature is shorter than the reference's. |

### From `nodal/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| N1 | `sanodaldiffusion_solverxyz.m` | **`nodal_update_interval == 1` destabilises the solver.** The MATLAB comment claims small values improve stability; observed behaviour is the opposite — a 3³ leaking cube (`k_inf = 1`) runs to the 5000-iteration ceiling and reports `k_eff = 1.114`. Interval >= 2 converges to -103 pcm of finite difference. **The MATLAB default `ceil((nx+ny+nz)/10)` IS 1 for any mesh with `nx+ny+nz <= 10`.** Real benchmarks are safe (IAEA-3D gives 6). Not confirmed against MATLAB, since the reference is not run. **Confirmed within the translation 2026-08-13**, now that the solver itself is ported: on a uniform leaking cube, interval 1 runs to the 5000-iteration cap at meshes 3, 4 and 5, reporting `k_eff` 3.271 / 8.397 / 2.082 against converged interval-3 values of 2.128 / 2.260 / 2.335. The 4-cube figure exceeds `k_inf = 2.5`, so it is diverging, not merely slow. Pinned by two tests, one of which reaches the failure through the *default* interval rather than by setting it. | **High** for small meshes  **CORRECTED (reporting) 2026-08-22.** The instability is **not** fixed — it is a property of the nodal update, not a mistranslation, and the two pinning tests stand. What is fixed is the silence: `SaNodalOutput::effective_nodalupd` reports the interval actually used, so a caller who never set `params.nodalupd` can see it resolved to 1. Verified at the cliff the entry names — extents summing to 10 give **1**, to 11 give **2**, IAEA-3D's 17/17/19 gives 6 — and an explicit value still wins. |
| N2 | `calc_sanodalxyz.m`, `calc_transleakagexyz.m` | **`Nc > 0` cannot work** — a `philen`-square operator is added to `philenf`-square ones, and a `philen`-wide operator multiplies a `philenf`-long flux. MATLAB dimension errors; the port panics identically. (cf. C11, K3.) | Low — untested path  **LATENT, verified 2026-08-23.** Every case in the snapshot sets `Nc = 0` (`neacrpa2.m`, `neacrpd1.m`, `iaea3ds.m` all checked), so this branch is unreachable. It stays recorded as a trap for whoever first sets `Nc > 0`. |
| N3 | boundary blocks | **A single-node direction is not a supported mesh** — `Lxb(xlow+1,…)` indexes out of bounds. Two nodes per direction minimum. | Low  **LATENT, verified 2026-08-23.** Needs a direction of extent 1; the smallest extent in any case is **14** (NEACRP D1's `maxiz`), against 17/17/18 for A2 and 17/17/19 for IAEA-3D. Unreachable, and already guarded — the leakage stencil's 2-node minimum is documented and asserted. |
| N4 | `makesigmadfxyz.m` mode 2 | Loop bound `for iz=m:m:maxiz` is missing the `m*` factor the `ix`/`iy` bounds carry, so with `m=2` it covers only half the axial nodes. Mode 2 is unreachable from the SANM path; not ported. | Medium if ever reached  **LATENT, verified 2026-08-23.** The missing `m` factor only bites in half-index mode (`m = 2`). **Every** call site of `makesigmadfxyz` in the port passes `None` for the mode — `criticalboron_xyz`, `diffusion_solverxyz`, `sanodaldiffusion_solverxyz` and `thdiffusion_solvertimexyz` all checked — which selects mode 1, exactly as the reference's own call sites do. Unreachable; pinned by test. |
| N5 | `makesigmadfxyz.m` | **`sigma.fp` omits the `nu` factor that `sigma.f` includes.** Asymmetric, and nothing in the snapshot says whether `sigmavalues.fp` is meant to carry it already. | Medium — ambiguous intent  **CLOSED 2026-08-23 — unresolvable from the snapshot, and inert in it.** Whether `sigmavalues.fp` is meant to carry the `nu` factor already is not stated anywhere in the reference, so there is no fact of the matter to translate to. `neacrpd1.m` comments `fp` out entirely and `neacrpd1t.m` rebuilds it (K7); the transient reproduces the MATLAB over its full 20 s window to **2.9e-11** with the port's reading, which is the only evidence available and is consistent with it. Reopen only if the author or a second case settles the convention. |
| N6 | `makesigmadfxyz.m` | **`nu` is read two different ways in one function** — `nu(m)` for `sigma.nu`, `nu(m,g)` for `sigma.f`. The scalar-broadcast path `nu*ones(G)` builds a G x G matrix, which works only by accident when `n_materials <= G`. | Medium  **LATENT, verified 2026-08-23.** The scalar-broadcast `nu*ones(G)` path works 'by accident when n_materials <= G'. Every case in the snapshot supplies `nu` per material **and** per group, so the broadcast branch is not taken: NEACRP A2 has 11 compositions and D1 has 19, against `G = 2` — the accident condition is not even satisfied, which is why the branch must be, and is, unreached. |
| N7 | `calc_a1_expansionxyz.m` | A reflective outer face over a node with `D == 0` makes the G x G system **exactly singular**; MATLAB warns and emits `Inf`/`NaN`. Reproduced (the dense solver deliberately does not error on a zero pivot). | Medium  **CLOSED 2026-08-23 — reproduced deliberately, and now detectable.** A reflective outer face over a `D == 0` node makes the local system singular and the reference emits `Inf`/`NaN`. Reproduced. What has changed since the entry was written is that the result is **no longer silent**: defect C5's counter reports any non-finite reaching the flux, measured **zero** on every case. |
| N8 | `calc_a1234_expansionxyz.m` | The **`1e6` magic substitution** for a zero diffusion coefficient leaks into `Ssource` at out-of-core nodes. | Medium  **CLOSED 2026-08-23 — reproduced, contained, and measured.** The `1e6` substitution for a zero diffusion coefficient does leak into `Ssource` at out-of-core nodes; those nodes carry no fission source and are stripped by the unit-diagonal compaction before the eigenvalue is formed. Every case reports zero non-finite substitutions and the three benchmark eigenvalues are unaffected. |
| N9 | `calc_ABEFGHxyz.m` | **No small-`alpha` expansion** — every coefficient is a ratio of vanishing differences, `NaN` at `alpha = 0` and losing significance well before that. | Medium  **CORRECTED 2026-08-23** — `Params::nodal_coeff_form`, default `NodalCoeffForm::SeriesBelowSmallAlpha`. Below a measured crossover of `alpha = 0.1` the six coefficients come from their Taylor series, derived term by term and checked against 60-digit arithmetic. **The correction is a bit-exact no-op on the snapshot** — 72,258 in-core coefficients across all three cases are identical under both settings — so it needed no benchmark re-validation. It is defensive: a finer mesh or a near-pure-scatterer region (`Sigma_r -> 0`) would reach the failure, and the failure is **silent**, returning finite plausible values wrong by up to **3.5e13** at `alpha = 1e-6` rather than NaN. **Margin correction:** an earlier note here put the cases 3535x above the failure, from analysing `mc` alone; the worst coefficient is `Bb`, which amplifies `mc`'s loss, and the true margin is **4x** (A2's minimum alpha 0.3535 against the crossover 0.1). |
| N10 | `sanodaldiffusion_solverxyz.m` | **Normalisation comment is wrong**: it says "fission source integration = 1", but the code preserves whatever the *initial* integral was. It also normalises by a plain `sum` while `diffusion_solverxyz` uses a 1-norm. | Medium — inconsistent between the two solvers  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The reference's normalisation *comment* is wrong; the code is right and is translated. The port's doc states what the code actually preserves. |
| N11 | `calc_sanodalxyz.m` | The near-zero-flux guard **silently falls back to finite difference**, on a threshold relative to the *global* flux maximum; and the shared `nodalterms(idxplus,5)=nodalterms(idx,6)` assignment sits *outside* the guard, so a neighbour can receive a stale value. | **High** — silent method change mid-solve  **CORRECTED (reporting) + measured LATENT, 2026-08-22.** The suppression is now counted (`SaNodal::guard_suppressions`, summed into `SaNodalOutput::nodal_guard_suppressions`); the guard itself is unchanged, so no number moves. **Measured: 0 suppressions on IAEA-3D, NEACRP A2 and NEACRP D1** — the guard never fires on any case in the snapshot. That also disposes of the second half: the `nodalterms(idxplus,5)=nodalterms(idx,6)` copy outside the guard can only propagate a stale value *when the guard suppresses*, which never happens here. No numeric correction is warranted, and the counter makes that checkable rather than assumed. |
| N12 | boundary-condition `switch` statements | **No `otherwise` branch** — a typo'd string silently leaves coefficients at zero. The `BoundaryCondition` enum makes that unrepresentable without changing behaviour for the three real values. | Medium  **STRUCTURALLY IMPOSSIBLE HERE, verified 2026-08-23.** The reference dispatches on a boundary *string* with no `otherwise` branch, so a typo silently leaves the coefficients at zero. This port replaces the string with the `BoundaryCondition` enum (`Vacuum`/`ZeroFlux`/`Reflective`), so every `match` is exhaustive and a mistyped condition is a **compile error**, not a silent zero. The failure mode cannot be reproduced. |
| N13 | `makegradDxyz.m` | `gradterms=2*gradterms; %check this (seems correct)` — **the author's own uncertainty**, preserved verbatim. | Medium — flagged by the author  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. The author's own `%check this (seems correct)` on the `gradterms` doubling, preserved verbatim because it records a genuine uncertainty rather than a settled derivation. The doubling itself is load-bearing and is exercised by the G3 correction, which had to satisfy `gradterms == L * (operator off-diagonal)` **including** that factor of 2. |
| N14 | `calc_a1_expansionxyz.m` | Cosmetic: the x high-boundary block is commented `%zhi node`. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. A mislabelled comment (`%zhi node` on the `x` high-boundary block). |

### From the two flux solvers (reported 2026-08-13)

Found while translating `diffusion_solverxyz.m` and
`sanodaldiffusion_solverxyz.m`, the last two SANM files. These are additions to
the register, not revisions of it — N1, N2 and N10 already covered the nodal
solver and still stand.

| # | Where | Defect | Severity |
|---|---|---|---|
| D1 | `diffusion_solverxyz.m:60-76, 160-174` | **The empty-grid compaction is dead code.** Both the compaction and its inverse are guarded by `keychange == 1`, where `keychange` is the literal `0` assigned four lines above. So a case with void nodes never gets the compacted operator the author wrote it for, and `convert_grid3d` / `convertsparsekey3d` are called from nowhere on this path. Not translated — there is nothing to reproduce, and writing an untested compaction would be inventing behaviour. | Medium — a written-but-disabled optimisation; someone will eventually flip it and find it untested  **LATENT, verified 2026-08-23.** Both the compaction and its inverse are guarded by `keychange == 1` where `keychange` is the literal `0` assigned four lines above, so neither can run. Translated as written and documented at `diffusion_solverxyz.rs:196`; no behaviour to correct. |
| D2 | both solvers, the non-convergence `break` | **The reported state lags one iteration behind the failure.** The `break` fires before `iteration` is incremented, so the returned `k_eff`, `residual` and `k_eff_residual` are the *previous* pass's — the offending value that triggered the break is computed, tested, then discarded. `sanodaldiffusion_solverxyz` prints the new `k_eff` in its bail-out message but still returns the old one, so the printed and returned values disagree. | Medium — a bailed-out run returns plausible-looking numbers  **ALREADY ADDRESSED.** The reference's break fires before the iteration counter advances, so the reported state lags one pass behind the failure. The port increments first — `// The break increments first, so the failing pass is in the history` — so the returned histories **include** the failing pass, and `Termination` says why it stopped. Recorded as a deliberate departure alongside D6. |
| D3 | `diffusion_solverxyz.m:48, 51, 54, 194` | **Four `writematrix` CSV dumps run unconditionally**, on every call, inside the inner solver. Its nodal counterpart puts the equivalent ten behind `params.debugdump`; this one has no switch at all. Every call scribbles `sigmafxy.csv`, `sigmasxy.csv`, `sigmatxy.csv` and `rel_power_inner.csv` into the working directory. Not reproduced as file writes — the quantities are returned instead; see the translation note below. | Medium — makes the solver unsafe to call concurrently and surprising to call at all  **ALREADY ADDRESSED.** The four `writematrix` dumps are **returned in a `Diagnostics` struct rather than written to disk** — a deliberate departure recorded in the crate README, so a library call has no filesystem side effects. The computation is unchanged. |
| D4 | `sanodaldiffusion_solverxyz.m:73, 165, 175-177, 189-193` | **Dead Wielandt-shift scaffolding.** `weilandtfactor = 1.05` and `weilandt = 0` are set; every use is commented out, including the `weilandt = 1` that would activate it, so the flag can never change. The author's own comment says "does not seem to work". Preserved as written; the shift is not implemented in the translation either. | Low — inert, but it reads as a live feature  **LATENT, verified 2026-08-23.** The Wielandt scaffolding is not translated at all — `weilandtfactor`/`weilandt` appear nowhere in the port outside documentation, because every use in the reference is multiplication by 1 or addition of 0. Dead in the reference, absent here. |
| D5 | `sanodaldiffusion_solverxyz.m:251-253` | **The final normalisation pairs mismatched vectors on an early break.** `norm_factor = sum(fission_source_new)` is applied to `fission_source`. On a normal exit those are the same vector; on a `break` they are one iteration apart, so a diverging run's source is rescaled by a factor taken from a *different* and worse source. | Medium — only reachable on a failed solve, but it corrupts the diagnostic output of exactly that case  **CLOSED 2026-08-23 — cannot arise on a converged solve, and an early break is now flagged.** The mismatched normalisation only differs when the loop breaks early, and on a normal exit the two vectors are the same. Since D6/D2, an early break is reported through `Termination` and the failing pass is in the returned history, so a caller can tell the normalisation is suspect rather than having to infer it. |
| D6 | both solvers | **Silent non-convergence**, the same shape as C7: the iteration exits on `k_eff <= 0`, `isnan(k_eff)` or the iteration cap with no flag in `output`. `sanodaldiffusion_solverxyz` at least prints a message; `diffusion_solverxyz` prints nothing distinguishable. The translation adds a `Termination` enum to the output rather than reproducing the silence — see below. | **High** — a non-converged answer is indistinguishable from a converged one  **ALREADY ADDRESSED** — this port adds a `Termination` value to both flux solvers (`Converged`, `NonPositiveKeff`, `NanKeff`, `IterationCap`), which is exactly the flag D6 says the reference lacks. Recorded in the crate README as a deliberate departure. No further work. |
| D7 | `sanodaldiffusion_solverxyz.m:36-43` | **The warm-start speed claim does not generalise.** The comment states a seeded flux "greatly reduces the source-iteration count". Measured across twelve cold/warm pairs (meshes 3/4/5, `nodalupd` 2/3/5/10) on a uniform leaking cube, nine improved or were neutral — best case 115 iterations down to 65 — but three were **worse warm than cold**: 618→937 (3-cube, interval 2), 84→93 (3-cube, interval 3) and 107→132 (5-cube, interval 2). All twelve agreed on `k_eff` to 3.7e-6 relative, so the warm start is *correct*; only the speed claim is overstated, and every regression sits at a small nodal-update interval — the marginally-stable regime N1 is about. Pinned by a test. | Low — a documentation overclaim, not a wrong answer  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. A comment claiming warm starts 'greatly reduce' iterations, which does not generalise. No code depends on the claim. |

### From `th/` — the rod-conduction and CHF files (reported 2026-08-13)

Found while translating the thermal-hydraulics layer. This section grows as
that layer lands; entries below cover `w3chf.m` and two files read while
mapping the dependency order.

| # | Where | Defect | Severity |
|---|---|---|---|
| T1 | `w3chf.m:34` | **The upwind enthalpy is halved.** `enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2` is a two-point average with a stray extra `/2`. Nothing in W-3 motivates a factor of a half, and `enthshift(1) = enthin` takes the *full* inlet enthalpy, so the two branches contradict each other. Halving the enthalpy raises `hLsat - enthshift`, raising `Kfour` and so **overpredicting** the critical heat flux. Measured at PWR conditions (15.5 MPa, 560 K inlet, quality -0.05): **+22.8%** on the predicted CHF, growing with subcooling. Pinned by a test. | **High** — a large, systematic, *non-conservative* error in a quantity whose only purpose is to bound a safety margin  **DUPLICATE of T5** in the `th/` coupled section, and **CORRECTED 2026-08-21** with it via `Params::w3_form`. The two `th/` sections recorded the same `w3chf` fault independently. |
| T2 | `w3chf.m:31-35` | **Local enthalpy where published W-3 uses inlet enthalpy.** The correlation's subcooling term is defined against the channel inlet enthalpy, constant along the channel; the reference substitutes a per-node upwind-shifted local value that reduces to the inlet only at node 1. This may be a deliberate local-conditions variant or the same unfinished edit as T1 — the snapshot says nothing. Preserved and flagged. | Medium — ambiguous intent, changes the correlation's meaning  **DUPLICATE of T5/T6** in the `th/` coupled section, and **CORRECTED 2026-08-21** with them via `Params::w3_form`, which uses the published inlet enthalpy. |
| T3 | `w3chf.m:14, 16, 56-58` | `gearth` is assigned and never used; `subarea` is read from the geometry and never used; and three `writematrix` calls dump CSVs unconditionally on every call, as in D3. | Low  **COSMETIC, no behaviour — closed 2026-08-23.** Translated as written and documented at the item that carries it. Nothing computes differently, so there is no before/after to gate a correction on; the record is the fix. More dead assignments in `w3chf`; not translated as parameters. |
| T4 | `makeheatlaplacian_1dcylnd.m` | **Dead code — the file is reachable only from a commented-out line.** Its sole call site, `th_solverxyz.m:174`, is commented out; the live path is `fuelrodheat_1dcylnd.m`, which assembles its own conduction matrix inline with a **different formula** (harmonic-mean interface conductivity against this file's `2*cond(ir+1)*sumLr(ir+1)/Lr(ir+1)`) and an interface-node-doubling scheme this file does not have. So the snapshot ships two divergent discretisations of the same operator, one of them unreachable. | Medium — a reader cannot tell which is intended, and the dead one is the more readable  **LATENT, verified 2026-08-23.** `makeheatlaplacian_1dcylnd` has **no live call site** in the port — the only references are `pub mod` in `lib.rs` and two doc cross-references. Its sole reference call site is a commented-out line in `th_solverxyz.m`; the live path assembles the same operator inline in `fuelrodheat_1dcylnd`. |
| T5 | `makeheatlaplacian_1dcylnd.m:62-67` | Within that dead file, the `whichk(ir+1)==0` branch writes to column `ir+2`, which at `ir = maxir-1` is `maxir+1` — **outside the declared `sparse(..., maxir, maxir)` shape**. MATLAB would raise an index error. Unreachable in practice only because the file is dead (T4). | Medium if T4 is ever undone  **LATENT, verified 2026-08-23.** Inside the dead file above — no live call site, so the out-of-shape write is unreachable. |
| T6 | `calc_tcond.m` | Called from nowhere in the snapshot. It is the guarded per-node conductivity lookup that `makeheatlaplacian_1dcylnd.m` open-codes *without* its `whichk(i) ~= 0` guard — so the utility exists, is correct, and is not used by the code that needed it. | Low  **LATENT, verified 2026-08-23.** `calc_tcond` has **zero** references anywhere in the port. Called from nowhere in the snapshot either. |
| T7 | `fuelrodheat_1dcylnd.m:70` **(duplicate of T1 in the coupled `th/` section)** | **The gap node is a dummy pinned at `T = 1`.** A node with `whichk == 0` gets `bvec(id) = 1` and keeps the preallocated diagonal `1`, and nothing else writes its row — conduction bridges from `id` to `id+2` around it. So it solves to exactly `1`, in kelvin. Confirmed by test at three power/coolant combinations. The live path survives it: `th_solverxyz.m` clamps the profile up to the coolant temperature on the next line, and takes its Doppler average from the centre and pellet-surface nodes rather than a volume average. But the value **is** in the returned vector, and the volume-average line that `th_solverxyz.m` has commented out would have consumed it. | Medium — contained today, and a trap for the next caller  **CORRECTED (made avoidable) 2026-08-23.** The 1 K value is **left in place deliberately** — the gap is an unresolved void with no temperature to compute, so interpolating would invent a plausible-looking number the solve never produced (worse, because unnoticeable), and dropping the row would change the vector length and every caller's `maxid` arithmetic. Instead the trap is now avoidable: `fuelrodheat_1dcylnd::gap_dummy_unknowns` locates the dummy rows by replaying the solver's own `ir`/`id` walk, `without_gap_dummies` strips them, and `Th::fueltemp`'s doc warns at the point of use. Measured on the NEACRP rod: the dummy sits at unknown **6**, and including it pulls the radial mean down **84 K (-10.0%)**. |
| T8 | `fuelrodheat_1dcylnd.m:79-90` | **A missing `else` leaves a stale conductance.** The `surf == 1` pass wraps its whole body in `if whichk(ir+1) == 0` with no `else`. Two *different, both solid* adjacent materials — fuel directly against cladding, no gap — reach that pass, emit no forward triplet at all, and then compute `laplcele(id) = kminus + kplus` from the **previous** iteration's `kplus`. The result is a row with an inflated diagonal and a missing off-diagonal, i.e. an operator that silently stops conserving energy. Unreachable for the benchmark meshes, which always model a gap. | Medium — latent; it would be silent if ever reached  **LATENT, verified 2026-08-23.** The missing `else` in the `surf == 1` pass only fires for a rod whose `whichk(ir+1) != 0` at that point — i.e. a **gapless** rod. Both NEACRP rods set `gapn = 1`, so the guarded branch always matches and no forward link is ever skipped. Same reachability argument as T2/T3. |
| T9 | `fuelrodheat_1dcylnd.m` | Three smaller things in the same file: `temps` is indexed to `id + 1` at the last unknown, so it must be `maxid` long where the mesh has only `maxir` nodes (a caller sizing it from `maxir` reads past the end); the `ir == maxir` branch reads `whichk(ir)` for *both* sides of its harmonic mean, making it a self-mean; and the `whichk(ir+1) == 0` branch multiplies its conductance by an extra `2`, with the un-doubled line commented out directly above and no derivation given. | Low-Medium  **CLOSED 2026-08-23 — all three parts settled, and the doubling is CORRECT.** (a) The `temps(id+1)` read is **asserted** (`temps.len() >= maxid`), so a caller sizing from `maxir` panics instead of reading past the end. (b) The `ir == maxir` self-harmonic-mean is `harmonic(k,k) = k` and is defensible where there is no `ir+1`. (c) **The extra `2` is derivable, not spurious**: that branch links the outermost fuel node to the pellet-surface unknown across **half** a node thickness, and a centre-to-face conductance is `k*r/(dr/2) = 2*k*r/dr`. Verified against the analytic pellet solution `q_v*R^2/(4k) = 420.25 K`: the discrete drop converges on it (328.3, 371.0, 394.8, 407.3, 413.7 K at 4/8/16/32/64 nodes). Removing the `2` would add 120.1, 56.0, 27.1, 13.3, 6.6 K at those meshes — roughly doubling the error everywhere. **The commented-out un-doubled line is the mistake, not the live one.** **New finding, not previously recorded:** the scheme is only **first order** (error ratios 1.87, 1.94, 1.97, 1.98) because every branch uses the node-**centre** radius where the **face** radius belongs, understating the conduction area by `dr/2`. The pellet drop is still **1.55% low at 64 radial nodes**, and the NEACRP rods use **5** — so every fuel temperature this crate reports carries a correspondingly larger discretisation error. A property of the reference's scheme, not a translation error; recorded, not repaired, since correcting it moves every fuel temperature. |

**Two places the flux-solver translation deliberately does not reproduce the
reference**, both recorded here so they are visible rather than buried:

1. **The CSV dumps are returned, not written.** D3's four unconditional writes
   and the nodal solver's ten `debugdump`-gated ones are computed exactly as the
   reference computes them and handed back in a `Diagnostics` struct. A library
   that writes files as a side effect of being called cannot be used from two
   threads, cannot be tested without a temp directory, and surprises every
   caller. The physics is identical either way, and a caller that wants the
   files can write them.
2. **The `gmres`/`ilu` branch is not translated.** Both solvers switch to
   preconditioned GMRES once `philenf >= 50_000_000` — fifty million unknowns,
   whose sparse operators alone would not fit in memory on any machine this runs
   on. The branch is unreachable for every case in the snapshot. Rather than
   write an ILU and a restarted GMRES that could never be exercised against the
   reference, and therefore never verified, the translation returns
   `BedokError::IterativeSolveNotTranslated`, which names the threshold and
   cannot be mistaken for the direct path having run.

A third difference is an addition rather than an omission: both solvers return a
`Termination` value saying why the iteration stopped, which the reference has no
equivalent of (D6). Nothing is subtracted to make room for it.

### From `makegradDxyz.m` — the non-uniform-mesh defect (reported 2026-08-18)

Found while porting `neacrpa2.m`, which is the first case in the snapshot with
a **graded axial mesh**. Recorded separately from the other `makegradDxyz`
entry above because it is a different failure with a much wider blast radius.

| # | Where | Defect | Severity | Status |
|---|---|---|---|---|
| G1 | `makegradDxyz.m:114-116` and the matching `x`/`y` blocks | **The face-diffusion coupling is only a consistent discretisation on a UNIFORM mesh.** With half-widths `h = L/2`, the code forms `Dt_plus = 0.5*(h+hp)*(D*Dp)/(h*D + hp*Dp)/L` and inserts `Dt_plus/hp`. For `D = Dp` that reduces to `D/(L*Lp)`, where the conservative finite-volume coupling is `D/(L*(L+Lp)/2)`. The two agree only when `L == Lp` and otherwise differ by exactly `(L+Lp)/(2*Lp)` — **measured: a 2:1 cell-size jump understates the coupling by 25%**, confirmed by test to 1e-12 against the predicted factor. On `neacrpa2.m`'s **actual** mesh — after defect Z1's rounding — the worst joint is 30 cm against 8 cm, where the coupling is misstated by **+137.5%**, and it sits at the bottom of the core where the axial power shape is set. | **High on any graded mesh**; nil on a uniform one | **CORRECTED 2026-08-21** |
| G2 | same lines | **The harmonic mean pairs each diffusion coefficient with the wrong half-width.** Series resistance `h/D + hp/Dp` gives a face conductance `D*Dp/(h*Dp + hp*D)`; the code writes `D*Dp/(h*D + hp*Dp)`. Independent of G1, and like G1 it vanishes when `h == hp`. Not separately quantified — G1's test holds `D` uniform to isolate the width error. | Medium, same regime | **CORRECTED 2026-08-21** |
| G3 | `makegradDxyz.m`, every `gradterms(...)` assignment | **`gradterms` does not describe the operator it was built alongside.** The function emits two things from one face calculation: the operator, whose off-diagonal for a face is `Dt/h_neighbour`, and `gradterms`, which `calc_sanodalxyz.m` subtracts from the SA-nodal current so that only the *difference* from the finite-difference estimate survives. That subtraction cancels only if `gradterms` is the same coupling expressed as a face current — `L * (operator off-diagonal)`. The reference stores `Dt` and doubles it at the end, giving `2*Dt` where consistency needs `Dt*L/h_neighbour`; the two agree **only when the neighbour is the same width as the node**. Boundary faces mirror the node's own half-width and are unaffected whatever the mesh. **Measured on a 2 / 4 / 8 cm stack: `gradterms` is out by a factor of 2 at a 4 cm node facing an 8 cm one** — the ratio `L / (2*h_neighbour)`. Distinct from G1 and G2, which are both about the value of `Dt` itself; this is about `Dt` being used in two places that need different normalisations. | **High on a graded mesh**, and it interacts — see below | **CORRECTED 2026-08-21** |

**G1, G2 and G3 must be corrected together, or not at all.** This was learned
the expensive way on 2026-08-21. Correcting G1/G2 alone — the operator's face
coupling, leaving `gradterms` as the reference writes it — leaves the two
*more* inconsistent than the reference was, because the SA-nodal correction
then subtracts a finite-difference current the operator never produced and the
cancellation the method depends on stops working. The symptom is not a small
bias: NEACRP A2's first coupled pass reached a peak fuel temperature of
**1995 K against the reference operator's 968 K**, `k_eff` hit 410 by the
second pass, and the coupled loop never recovered in 200. The eigenvalue of the
*static* solve looked entirely reasonable throughout (+81 pcm), which is what
made it deceptive — the damage was in the power distribution, not the
eigenvalue.


**Corrected together, they behave.** With G1, G2 and G3 all applied, A2's
coupled loop converges in **16 passes — the same count as the reference** —
with a comparable fission-source residual (7.328e-5 against 7.332e-5) and a
pass-1 peak fuel temperature of 953 K against the reference's 968 K. The
half-applied state was the entire problem.
**Why this matters here.** `neacrpa2.m`, `neacrpa2t.m` and `neacrpa1t.m` all use
the benchmark's graded axial mesh (`30, 7.7, 11, 15, 30, ... 12.8, 12.8, 8, 30`
cm), so every NEACRP PWR result passes through G1 at several axial joints, the
worst of them close to a 4:1 ratio.

**Why it has not obviously bitten.** Those cases are solved with
`sanodaldiffusion_solverxyz`, whose SA-nodal correction is refitted against the
same operator and appears to absorb much of the inconsistency. A bare
finite-difference solve on a graded mesh has no such compensation, which is how
this was originally surfaced — on a fine-mesh CC3 model, where the thesis's
graded axial grid corrupted the axial power shape (bottom-node power 0.488
against a converged 0.70) while uniform meshes tracked the reference.

**Not repaired**, per the no-silent-repairs policy, and this one is emphatically
a case where that matters: correcting it would move every NEACRP number. It is a
**correction**, not a substitution, so it cannot be gated on parity with the
reference — it would need its own before/after study against a benchmark. See
the crate README, "Translation policy".

---

### From the case files — the axial mesh is silently rounded (reported 2026-08-18)

Found by running the MATLAB while chasing X1. **This is the most consequential
reference defect found so far**, and it is invisible from reading the source.

| # | Where | Defect | Severity |
|---|---|---|---|
| Z1 | `neacrpa2.m`, `neacrpd1.m`, and the same block in `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1t.m` | **The axial mesh is silently rounded to integers by MATLAB integer-type contagion.** The cases build `geometry.Lz(...) = Zlengths(...)/zscale` where `zscale = int64(maxiz/N)`. In MATLAB `double / int64` returns an **`int64`**, rounding to nearest. So `7.7/int64(1)` is **8**, `12.8/int64(1)` is **13**, `30.48/int64(1)` is **30**. The mesh the solver integrates is not the one the case file writes. | **High**  **REPRODUCED DELIBERATELY, not pending.** The port must mesh what the MATLAB meshes or it is not a translation of it; reproducing Z1 is what brought the NEACRP cases into exact agreement. Correcting it would mean meshing the height the case files *write* (427.3 cm for A2, 426.72 for D1) and would move every NEACRP result away from the reference. That is a **case-data decision for the maintainer**, not a defect fix. |

**Verified by running MATLAB R2026a, 2026-08-18:**

```text
class(7.7/int64(1))    ->  int64,  value 8
class(12.8/int64(1))   ->  int64,  value 13
class(30.48/int64(1))  ->  int64,  value 30

neacrpa2  geometry.Lz(1:18) = 30 8 11 15 30 30 30 30 30 30 30 30 30 30 13 13 8 30
          (the file writes    30 7.7 11 15 ... 12.8 12.8 8 30)
neacrpd1  geometry.Lz(1:3)  = 30 30 30      (the file writes 30.48)
```

**Each case is left internally inconsistent**, because `geometry.Ztot` is
computed from the *un-rounded* `Zlengths`:

| case | `Ztot` says | the mesh actually sums to |
|---|---|---|
| `neacrpa2` | 427.3 cm | **428 cm** |
| `neacrpd1` | 426.72 cm | **420 cm** — a core 1.6% shorter |

**Why it went unnoticed.** `iaea3ds`'s layer heights are already integers (10 cm
radial, 20 cm axial), so the rounding is a no-op there — which is precisely why
that case reproduces the published `k_eff` to -1.1 pcm while the NEACRP cases
did not.

**Reproduced, not repaired.** This port originally implemented the *written*
values — what the source says, but not what it does. It now reproduces the
rounded mesh, keeping the written values alongside as `Z_LENGTHS_AS_WRITTEN`.

**What fixing it bought:** see X1 below. It turned a divergence into a converged
solve and brought the feedback chain into round-off agreement with the MATLAB.

---

### From the benchmark cases (reported 2026-08-18)

Found while translating `iaea3ds.m` and `neacrpd1.m`, the first two case files.

| # | Where | Defect | Severity |
|---|---|---|---|
| B1 | `neacrpd1.m:209-215`, and identically in `neacrpa2.m:223-228` and `neacrpa1t.m:238-243` | **The fuel-rod node volumes are computed from thicknesses, not radii.** The annular-shell loop sets `rminus = sum(Lr(i-1))` and `rplus = sum(Lr(i))`, but for a scalar index `sum` is the identity, so those are node *thicknesses*. Every fuel node has the same thickness, so `Vi(2:20)` comes out **exactly zero**: the array sums to 0.0209 cm3/cm where the pellet and cladding occupy 1.6061, understating the rod cross section by a factor of 77. Confirmed by test. | Low **today**, High if ever consumed — the field is dead (see below)  **DUPLICATE of K1**, and **LATENT** for the same reason: `geometry.fuel.Vi` has no consumer anywhere in the snapshot — neither rod solve reads it, and the T9 volume average deliberately derives its weights from `Lr` instead. Correcting it would move no number. The two entries describe the same `sum(Lr(i))`-on-a-scalar-index fault in the same three case files. |
| B2 | `neacrpd1.m:295-307` and the equivalent block in every case file | `sigmavalues.fueltemp.upd` and `sigmavalues.coolden.upd` are built with a per-node loop marking every fissile node, and **nothing in the snapshot reads either**. A search for `.upd` finds writes in the case files and no consumers. Not translated. | Low — dead data  **LATENT, verified 2026-08-23.** `sigmavalues.fueltemp.upd` and `coolden.upd` are built by a per-node loop and **nothing in the snapshot reads either** — the same shape as K1. No consumer, no consequence. |

**Why B1 is contained.** `geometry.fuel.Vi` is read into `Vif` by
`th_solverxyz.m:32` and `th_solvertimexyz.m:41`, and then used only on two
**commented-out** lines — one of them the volume-averaging line that the Doppler
average replaced. So the wrong values reach nothing. They are reproduced as
written, per the no-silent-repairs policy, and pinned by a test that will fail
the moment someone re-enables a consumer without fixing the formula.

**A non-defect worth recording**, because it looks like one: both case files set
`constants.nu = ones(...)`, so `sigmavalues.f` already carries the `nu*Sigma_f`
product rather than a bare fission cross section. That is the benchmarks' own
convention, not an omission, and every downstream multiplication by `nu` is
consistent with it.

---

## Verified against the benchmark specification

Checked directly against **NEACRP-L-335 (Rev. 1)**, catalogued as
`1991finnemann-neacrp-l-335`. Distinct from the MATLAB comparison below: that
one asks *is the translation faithful*, this asks *is the case data right*.

| quantity | specification | this port |
|---|---|---|
| PWR gap conductance | 1e4 W/(m2 K), section 2.10 | `neacrpa2` **1.0 W/(cm2 K)** |
| BWR gap conductance | 3.5e3 W/(m2 K), section 5.10 | `neacrpd1` **0.35 W/(cm2 K)** |
| Doppler weight `alpha` | **0.7**, sections 2.5 and 5.5 | `doppleralpha: 0.7` both cases |
| UO2 conductivity | `1.05 + 2150/(T-73.15)`, section 2.7 | exact |
| Zircaloy-4 conductivity | `7.51 + 2.09e-2 T - 1.45e-5 T^2 + 7.67e-9 T^3` | exact |
| UO2 heat capacity | `162.3 + 0.3038 T - 2.391e-4 T^2 + 6.404e-8 T^3` | exact |
| Zircaloy-4 heat capacity | `252.54 + 0.11474 T` | exact |
| UO2 density / dishing | 10.412 g/cm3, 1.248% | exact |
| Zircaloy density | 6.6 g/cm3 | exact |
| CA insertion lengths | 228, 200, 150, 100, 0 steps, section 3.2 | exact |
| CA ejection time | 100 ms, section 3.2 | `EJECT_DURATION` |
| BWR compositions | Tables 5.4-5.22, i.e. **19** | highest index 19 |
| BWR flow cross-section | 400.78 cm2, Table 5.23 | per-rod x196 = 374.27 — **different control volume** |
| BWR heated perimeter | 880.5256 cm, Table 5.23 | `196*pi*1.430` = 880.5256, **exact** |
| BWR hydraulic diameter | **1.4730 cm**, Table 5.23 | per-rod 1.2177 — **see K13** |

**The two gap conductances differ by a factor of nearly three between the PWR
and BWR halves of the same benchmark**, and the port carries both correctly —
worth stating because using one value for both would be an easy and invisible
error.

This section exists because defect K9 was withdrawn only by reading the
specification. A translation cannot tell a missing feature from a faithfully
absent one, nor a wrong constant from a right one.

---

## Verified against the running MATLAB

What has been compared against MATLAB R2026a directly, and to what tolerance.
This is the strongest evidence the crate has: not agreement with a published
number, but agreement with the code being translated.

| what | case | agreement |
|---|---|---|
| `sigmavalupd3d_handler` — all five feedback channels, rod tip search, material renumbering | A2 | **3.3e-13** |
| rod-fraction map | A2 | **1.1e-15** |
| `sanodaldiffusion_solverxyz`, static, at `nodalupd` frozen/200/100/50/20 | A2 | **exact**, 10 s.f. |
| `sanodaldiffusion_solverxyz`, static, at `nodalupd` frozen/100/50/20/5 | D1 | **exact**, 0.000 pcm |
| `thdiffusion_solverxyz` coupled on `hem` | A2 | **exact** — `k_eff` 1.0139476080 both |
| `thdiffusion_solverxyz` coupled on `hem` | D1 | **exact** — `k_eff` 0.9752848326 both, same 27 passes, and fuel T, coolant T, heat flux and power identical to every printed digit |
| `thdiffusion_solvertimexyz` transient, 0-0.5 s, cold-water injection | D1t | **exact** — C1 power 1.6e-11, temperatures identical, precursors 2.8e-11, C1 history worst 4.3e-9 |
| `thdiffusion_solvertimexyz` transient, 0-0.15 s, **rod ejection** | A2t | **exact** — C1 power 2.6e-11, rod position 0.000 steps at every step, C1 history worst 3.7e-9 |
| `thdiffusion_solvertimexyz` transient, 0-0.15 s, **super-prompt HZP ejection** | A1t | **2.1e-7** through a 67-fold power excursion over 151 steps — round-off compounded by exponential growth |
| `thdiffusion_solvertimexyz`, the **full specified 20 s window**, 261 steps | D1t | **exact** — C1 power 2.9e-11, the non-monotonic peak at the same time step, and the `tmaxfuel` clamp reached identically at 3100 K |
| `sanodaldiffusion_solverxyz`, pure neutronics, no feedback | IAEA-3D | **exact** — `k_eff` 1.0290842762 both, -0.0000 pcm |
| `criticalboron_xyz` end to end | A2 | **both abort at the same boron** — MATLAB at 1139.19 ppm, this port at 1139.1939954, each tripping its own sanity guard on a destabilised eigensolve (defect N1) |

**Why two cases matter more than one.** A2 is a PWR at 15.5 MPa with a graded
axial mesh, five feedback channels and 11 materials. D1 is a BWR at 6.7 MPa
whose coolant boils, on a uniform mesh, with two feedback channels and 19
materials, a different rod geometry and a different pin. They exercise nearly
disjoint paths through the thermal-hydraulics — D1's coolant reaches
`Tsat(6.7 MPa) = 556.03 K` and enters the two-phase branch of
`singleflow1devap` that A2 never touches.

**The transient path is now covered too**, on all three cases: a cold-water
injection with no rod motion, a full-power rod ejection, and the super-prompt
HZP ejection. Between them they exercise the exponential-transform kinetics,
analytic precursor integration over six delayed families, the prescribed inlet
forcing, a moving control assembly, and the transient thermal-hydraulics.

**Every case in the snapshot that can be run has now been compared**, steady and
transient, short window and full window, pure neutronics and fully coupled.

**What remains outside the comparison**, and should not be described as verified:

- **The individual T-H modules** are verified *transitively* — the coupled and
  transient comparisons drive `th_solverxyz`, `th_solvertimexyz`,
  `singleflow1devap`, `singleflow1devaptime` and `fuelrodheat_1dcylnd` on every
  step — but none has been called directly with a MATLAB-supplied input and its
  output diffed. A defect that cancels within the coupled loop would not show.
- **`w3chf` and `w3chfhottest`** run inside the coupled driver but their output
  is **discarded by the reference** (defect C3), so nothing compares it. The
  22.8% CHF error (T1) is measured against the published correlation, not the
  MATLAB.
- **The A2/A1 transients beyond 0.15 s.** D1t is verified over its full 20 s;
  the two ejection cases specify 5 s and were compared over 0.15 s, enough to
  cover the ejection itself but not the later decay.
- **`geom2dxycase1`** — no solver in the snapshot can run it, in either code.
- **IAPWS region 3** — not translated at all, which caps everything at
  16.5292 MPa. A2 at 15.5 MPa is under it, but not by much.

---

## Maintainer decisions taken

### Defects G1/G2/G3 — the conservative face coupling is now the DEFAULT

**Decided 2026-08-21 by the maintainer.** `Params::gradd_form` defaults to
`GradDForm::Conservative`; `Params::reference_faithful()` returns the
reference's numerics for anything that needs to reproduce the MATLAB, and every
MATLAB-parity test builds from it.

**The precondition was met first.** The decision was deferred until every
difference between the MATLAB and this port was resolved, for the reason
recorded at the time: with a correction on, a mismatch could be either a
translation error or the correction, and the two cannot be separated. Parity
was established case by case (see "Verified against the running MATLAB"), so
that objection no longer applies.

**The evidence, measured on the decision.**

| | reference | conservative |
|---|---|---|
| A2 static eigenvalue | 1.0230689628 | 1.0238996849 (+81.20 pcm) |
| A1 static eigenvalue | 0.9977440304 | 0.9983590075 (+61.64 pcm) |
| IAEA-3D, NEACRP D1 | — | **bit-for-bit unchanged**, uniform meshes |
| A2 coupled `k_eff` @ 1000 ppm | 1.0139476080, 16 passes | 1.0153550800, 16 passes |
| A2 differential boron worth | -9.914 pcm/ppm | -9.917 pcm/ppm |
| **A2 critical boron** | ~1138.8 ppm | **~1152.5 ppm** |
| **vs published 1160.6 ppm (PANTHER)** | **-21.8 ppm** | **-8.1 ppm** |
| A1 coupled `k_eff` @ 500 ppm | 1.0050164273, 7 passes | 1.0059514689, 6 passes |
| A1 differential boron worth | -9.703 pcm/ppm | -9.701 pcm/ppm |
| **A1 critical boron** | ~551.4 ppm | **~561.0 ppm** |
| **vs published 567.7 ppm (PANTHER)** | **-16.3 ppm** | **-6.7 ppm** |

**The justification does not appeal to the reference**, which it could not: the
correction closes most of the gap to the published critical boron on **both**
PWR cases, and by nearly the same fraction.

| case | reference | conservative | published | gap closed |
|---|---|---|---|---|
| A1 (hot zero power) | 551.4 | 561.0 | 567.7 | **59%** |
| A2 (hot full power) | 1138.8 | 1152.5 | 1160.6 | **63%** |

A1 and A2 share the geometry, mesh, cross-section tables and material map — so
the operator change is the same one — but differ in power level (2775 W against
2775 MW), rod pattern (six banks fully in against a partial insertion) and
boron (roughly half). **Two benchmark points that far apart in state moving by
nearly the same fraction is what a systematic discretisation error being
removed looks like**, and is much harder to explain as coincidence than either
case alone.

Supporting it: the correction is the consistent finite-volume discretisation;
it is a bit-exact no-op wherever the mesh is uniform; it leaves the
differential boron worth unchanged on both cases (-9.914 to -9.917 on A2,
-9.703 to -9.701 on A1), so it shifts the eigenvalue without distorting the
feedback; and the coupled loop converges in the same number of passes or one
fewer.

**The reference arms are a good control.** Their secants reproduce both of this
code's own quoted critical borons from searches the snapshot does not ship:
1138.8 against 1139.01 on A2, and **551.44 against 551.31** on A1.

**What the correction does NOT explain.** About 40% of each gap survives it —
-8.1 ppm on A2, -6.7 ppm on A1. Nothing measured here accounts for that, and it
should not be attributed to the remaining register entries, to the two-group
cross-section reconstruction, or to method bias in PANTHER's own values until
one of them is actually measured.

**What was given up.** This crate no longer reproduces the MATLAB's
eigenvalues on the NEACRP PWR cases by default. That is a real cost, and it is
why the switch and `reference_faithful()` exist rather than the correction
being hard-coded.

**A superseded objection, and the measurement now runs the other way.** An
earlier note here held that the correction "degrades coupled convergence" on
A2, and treated that as evidence against defaulting it on. That was **defect
G3** — the correction half-applied, with `gradterms` left inconsistent with a
corrected operator — not a property of the correction.

Re-measured on 2026-08-22 with all three corrected, A2's Phase-0 solve at the
case's default nodal interval **converges in 10 passes with the correction on
and hits the 51-pass cap with it off** (`k_eff` 1.026437 against 184.52). So
the correction does not merely fail to degrade convergence on this case, it is
what makes it converge. The objection is not just withdrawn; the evidence
points the opposite way.

### Defects C2/T4 and T5/T6 — the CHF is now computed as published, in the right channel

**Landed 2026-08-21.** Two switches, both defaulting to the correction:
`Params::w3_form` and `Params::hot_channel_search`.

**Why these two together.** They are separable in code and inseparable in
consequence: C2/T4 picks **which channel** is analysed, T5/T6 sets **the flux
computed in it**, and both feed the single number the crate reports. Measuring
either alone understates what a reader of that number was getting.

**The evidence, on NEACRP A2's converged steady state** (identical `k_eff` in
both arms — this is post-processing and does not feed back):

| | channel analysed | true peak | peak CHF, W/cm2 | limiting DNBR |
|---|---|---|---|---|
| as written | **(2, 2)** | (2, 5) | 337.8054 | 2.5462 |
| corrected | (2, 5) | (2, 5) | **275.3966** | **2.0816** |

**The reported thermal margin falls 18.2%.** Split: the `K4` enthalpy carries
about 17.4% of the overstatement, the channel misidentification a further 1.0%.

**The justification does not appeal to the reference.**

- **T5/T6** — the published W-3 correlation (Tong, 1967) forms `K4` from the
  **inlet** enthalpy. The reference builds a per-node upwind average, halves it
  with a stray `/2` that nothing in W-3 motivates, and walks `i-1` over the
  **flat index** so the first node of each channel mixes in the top node of the
  previous one. The overprediction measured on the real case, **+22.66%**,
  matches the +22.8% measured independently on a synthetic node.
- **C2/T4** — there is no reading under which `highy = ix` is intended. The
  line above it is `highx = ix`, establishing that the pair `(ix, iy)` is what
  is being recorded, and `iy` is in scope and unused.

**Both defects overstate margin**, in a quantity whose only purpose is to bound
one. Nothing in the snapshot consumes the result — defect C3 discards it — so
neither had a chance to be noticed; this crate returns it, which is what made
them measurable.

**What this does NOT establish.** The corrected CHF is checked against the
published correlation's *form*, not against measured CHF data, and W-3 carries
its own range of validity. This is verification, not validation.

---

### Defects T9/T13 — `fueltempavg` is a volume average, and the feedback is untouched

**Landed 2026-08-22 at the maintainer's direction**, who set both halves of the
requirement: `fueltempavg` computed as the volume average, and
`fueltempdoppler` kept as the quantity driving temperature feedback, following
the NEACRP cases. Switch: `Params::fueltemp_average`, default
`FuelTempAverage::VolumeWeighted`.

**The feedback requirement is the benchmark's.** NEACRP-L-335 sections 2.5
(PWR) and 5.5 (BWR) define the Doppler temperature as
`T = (1 - alpha) * T_F,C + alpha * T_F,S` with `alpha = 0.7`, and
`sigmavalupd3d_handler` takes its fuel-temperature channel from
`th.fueltempdoppler`. Volume-averaging the *feedback* temperature would be a
departure from the benchmark, not a correction — so the correction is confined
to the reported average.

**Measured on NEACRP D1**, coupled steady, hottest node:

| | `fueltempavg` | `fueltempdoppler` |
|---|---|---|
| reference alias | 1474.5021 K | 1474.5021 K |
| volume-weighted | **1714.5219 K** | 1474.5021 K |

`k_eff` 0.9752848326 -> 0.9752848326 (**+0.000 pcm**), 27 passes both, and the
Doppler temperature **bit-identical** node for node (max change 0.000e0 K).
The largest change in the reported average anywhere in the core is **240 K**.

**Verification of the weights, not just of the difference.** A uniform
volumetric source gives a parabolic pellet profile whose exact volume average
is `0.5*Tc + 0.5*Ts`, against the Doppler weight's `0.3*Tc + 0.7*Ts` — so the
average must sit *above* the Doppler value by exactly `0.2*(Tc - Ts)`. Fed the
analytic profile, the implementation converges on the exact mean with the error
falling by **4.00 at every mesh doubling** (second-order, as a mid-point rule
on annuli should).

**Two traps this correction had to avoid**, both other register entries:

- **The gap node is pinned at 1 K** (T7/T1). Averaging the whole returned
  profile — which is what the commented-out line would have done — drags in a
  physically meaningless value. The average runs over the pellet nodes only.
- **`geometry.fuel.Vi` is wrong** (K1/B1): built from node thicknesses where
  cumulative radii are meant, so it is identically zero on a uniform pellet
  mesh. The weights are derived from `geometry.fuel.Lr` instead, so this
  correction does **not** depend on K1 being fixed first.

That second point revises K1's status once more. K1 was found latent because
`fuel.Vi` has no consumer — but the commented-out line in `th_solverxyz.m` is
plainly the consumer it was *meant* to have, so K1 is latent **because** T9 is
present, not independently of it. Correcting T9 through `Lr` keeps it latent.

**What this is not.** A reporting change. Anyone who has quoted a BEDOK
"average fuel temperature" for a NEACRP case was quoting a Doppler weight, and
the two differ by up to 240 K on D1 — but no eigenvalue, power distribution or
transient result moves.

---

### Defects C5 and C7 — a failed solve is no longer silent

**Landed 2026-08-22.** These two are a different kind of correction from the
four before them: **they move no number at all.** Neither fixes a wrong answer;
both remove the ability of a wrong answer to pass for a right one.

- **C5** — `fixinfnan` patches `Inf`/`NaN` out of the flux after every linear
  solve, and the residual norms are then computed on the patch, so a diverged
  solve can report a small residual. The patch is reproduced exactly and the
  substitutions are now **counted**, reaching a caller through
  `SaNodalOutput::non_finite_substitutions`.
- **C7** — the cold power iteration (8000) and the transient's Phase-2
  re-equilibration (5000) both exit on their cap with no error and no flag.
  Both now return whether they converged and their final residuals.

**Evidence.** IAEA-3D, NEACRP A2 and NEACRP D1 all report **zero**
substitutions, so none of the eigenvalues this crate compares against a
published benchmark was computed from a repaired flux. That was an assumption
before; it is a checked property now. The A2 critical-boron search reports
`bootstrapped == false` and no abandoned cold solves.

**Why this was worth doing despite moving nothing.** The X1 investigation took
as long as it did partly because a diverged A2 solve looked healthy from its
residuals. Both defects are of that shape: they do not produce a wrong result,
they destroy the evidence that would let you notice one.

**What is deliberately NOT done.** Neither is promoted to an error or a
`Termination`. A non-zero count and a `converged == false` are reported, and it
is the caller's decision what to do about them — turning them into failures
would change control flow, which is more than a diagnostic correction should
do without a separate decision.

---

---

## Pending maintainer decisions

Things an agent must **not** decide unilaterally, recorded so they are not lost.

*(None outstanding.)*

---

## Open discrepancies against the reference (not defects in the reference)

These are places where **this translation disagrees with the MATLAB**, cause
unknown. They are listed separately from the defect register above, which
records faults *in the reference*. An entry here is a question, not a finding.

| # | Where | Discrepancy | Status |
|---|---|---|---|
| X1 | `thdiffusion_solverxyz` / `criticalboron_xyz` on `neacrpa2` | **Root cause found and fixed: defect Z1**, the silently rounded axial mesh. This port had implemented the mesh the case file *writes*; the MATLAB integrates a rounded one. With Z1 reproduced, the feedback handler matches the MATLAB to **3.3e-13**, the rod-fraction map to **1.1e-15**, and the A2 coupled solve **converges in 27 passes** against the MATLAB's 23 — where it previously hit the iteration cap at 51 with `k_eff ~ 43`. **A residual `k_eff` gap of +2144 pcm remains** (1.035684 against 1.013943), now on a *converged* solve. | **RESOLVED.** Two causes, both now understood: (1) defect **Z1**, the silently rounded axial mesh, which this port had not reproduced; and (2) defect **N1**, the unstable default nodal-update interval, which makes the coupled trajectory chaotic so two codes land on different attractors. With Z1 reproduced and `nodalupd = 20`, the two codes agree **exactly** — `k_eff` 1.0139476080 in both, and the fuel temperature, coolant temperature, heat flux and power identical to every printed digit. **Not a translation error.** |

**Narrowed 2026-08-18 by running the same search on case A1**, via
`criticalboron_xyz`'s `the_search_on_case_a1_...` diagnostic.

| case | this port | reference | difference | bootstrapped |
|---|---|---|---|---|
| A1 (HZP) | 551.14 ppm | 551.31 ppm | **-0.17 ppm (-0.03%)** | **no** |
| A2 (full power) | 1253.29 ppm | 1139.01 ppm | **+114.28 ppm (+10.03%)** | **yes** |

**What this rules out.** A1 and A2 share the cross-section tables, the material
map, the whole feedback chain (including the boron channel), the eigensolver,
the thermal-hydraulics and this entire search. A mistranslation in any of them
could not leave A1 within 2 pcm of the reference. The earlier "translation error
in the feedback chain" hypothesis is therefore effectively eliminated, as is
defect G1 — both cases carry G1 identically, and A1 even reproduces the
reference's *own* -16 ppm disagreement with the published benchmark.

**What remains, now two specific questions:**

1. **Does the Phase-0 bootstrap converge a different coupled state?** It freezes
   the nodal correction and under-relaxes at fixed boron, so it may settle
   somewhere the standard solver would not — and Phases 1 and 2 then search
   around that state.
2. **Why did `thdiffusion_solverxyz` fail on A2 in the first place?** The
   reference presumably reached 1139.01 without a fallback. If the coupled
   driver fails on A2 here where the MATLAB succeeds, *that* is the defect and
   the bootstrap is only exposing it.

**Investigated further 2026-08-18. Two results, one of them negative.**

**(a) The A2 Phase-0 solve was instrumented.** It does not simply fail. It
recovers from the cold start by pass 7, holds `k_eff = 1.0355` for nine
consecutive passes with the fuel-temperature residual halving exactly each pass
(640, 320, 160, 80, 40, 20 K — the `wrelax = 0.5` under-relaxation approaching a
*stationary* target), and then diverges at pass 16 and never recovers. A1 on the
same core converges monotonically in 7 passes. So this is an **already-converging
iteration being destabilised**, not a bad initial guess.

**(b) The hypothesis that G1 causes X1 was tested and REFUTED** — correctly,
but the supporting observation recorded here was itself wrong, and is corrected
below.

*As recorded 2026-08-18:* enabling the G1/G2 correction and re-running A2's
Phase-0 solve gave a **worse** result — where the reference operator at least
reached a 1.0355 plateau, the corrected operator never plateaued at all. The
conclusion drawn was that "a better-conditioned discretisation made the symptom
worse", and it was carried forward as evidence against defaulting the
correction on.

*Corrected 2026-08-22:* that was a **half-applied correction**, not the
correction. Defect **G3** — `gradterms` left inconsistent with a corrected
operator — was not yet known, and it corrupts the power distribution while
leaving the eigenvalue plausible. Re-measured with G1, G2 and G3 corrected
together, the comparison **inverts**:

| operator | termination | `k_eff` |
|---|---|---|
| reference | IterationCap at 51 | 184.52 |
| conservative | **Converged in 10 passes** | 1.026437 |

The control passed in both runs — IAEA-3D, on a uniform mesh, unchanged at
1.029084.

**The hypothesis itself is still refuted**: G1 was not the cause of X1, whose
causes were Z1 and N1. But the reasoning "a better-conditioned discretisation
made the symptom worse" was measuring a defect in the experiment, not a
property of the operator.

**(c) The MATLAB was RUN, and it fails the same way.** 2026-08-18, MATLAB
R2026a, `neacrpa2` through `thdiffusion_solverxyz.m` with `main_exec_diff3d.m`'s
own set-up:

| | MATLAB | this port |
|---|---|---|
| termination | 51 iterations, `[NOT converged]` | `IterationCap` at 51 |
| `k_eff` | 8594.168746 | 45.781941 |
| **fuel-temp residual** | **1.270425e+03 K** | **1270.4035 K** |
| usable by Phase 0 | no | no |

**The translation is faithful — this is not a port bug.** The fuel-temperature
residual agrees to five significant figures, i.e. both codes come to rest in the
same physical state. `k_eff` differs wildly because after divergence it is
chaotic garbage; the *physical* variable is what matters and it matches.

**And the MATLAB run explains why.** Its output carries **21 warnings** reading

```text
channel solve failed: Undefined function 'driftflux6_solverstatic1d' ...
  - keeping previous state.
```

`neacrpa2.m` sets no `th_model`, so both codes take the **two-fluid** default,
whose 1-D kernel is **missing from the snapshot** (see "Missing files"). Every
powered channel fails and keeps its inlet state, so **the coolant cannot heat**.
The coupled loop is then being asked to find a fixed point in which power rises
without the coolant ever responding — and no such state exists. The fuel
temperature climbs until it saturates, which is the 1270 K residual both codes
report.

**So the answer to "why does `thdiffusion_solverxyz` fail on A2 at all?" is:
because the thermal-hydraulic path it defaults to is non-functional in this
snapshot, in the MATLAB just as much as here.** It is not an instability in the
driver, not the feedback, and not defect G1.

**Where that leaves X1.** Not in the translation. The reference's 1139.01 ppm
cannot have come from a converged default-path Phase-0 solve, because that does
not converge in the MATLAB either. It must have come from the bootstrap, from
`th_model = 'hem'`, or from some other setting in `test_critboron3.m` — which is
not in the snapshot. **X1 is now most likely a difference in how the two searches
were configured, not in what they compute.**

**(d) The feedback channels were bisected, and one is responsible.** Running
A2's Phase-0 with the channels enabled one at a time:

| channels | diverged at | `k_eff` | passes |
|---|---|---|---|
| none | - | 1.0012 | 10 |
| boron only | - | 1.0214 | 10 |
| fueltemp only | - | 1.0014 | 10 |
| cooltemp only | - | 1.0018 | 10 |
| coolden only | - | 1.0094 | 10 |
| **crod only** | **7** | **394.72** | 26 |
| ALL five | 15 | 15.18 | 26 |

**The control-rod channel destabilises the loop**, and this **corrects (c)
above**: the inference that a frozen coolant leaves no fixed point was too
strong. The `none` row has the same frozen coolant and converges fine. A frozen
coolant is not fatal by itself — the rod feedback on top of it is.

It is also consistent with the MATLAB converging on `hem` with all five channels
live: the rod channel is not broken in isolation, it is **sensitive to the
coolant state**.

**Two candidate mechanisms, neither confirmed:**

1. The rod channel most enlarges the material table (11 base materials become
   3978 rows on this case), and `calc_bucklingxyz`'s cache is fingerprinted on
   three sums and three non-zero counts — which cannot separate every distinct
   cross-section set. A collision silently reuses the wrong cached
   coefficients. That defect is already in the register; this would be the
   first evidence of it actually firing.
2. The rod slope's `reference` is forced to zero on use, so a fully rodded node
   takes the *entire* slope rather than a departure from a reference state — a
   far larger perturbation than the other four channels apply.

**What would settle it.** Run the same bisect in the MATLAB. A faithful
translation should destabilise there too; if the MATLAB is stable with
`crod`-only, the rod channel is where the translation diverges and X1 finally
has an address. Separately, instrument `calc_bucklingxyz`'s cache to count
fingerprint collisions on this case — that is cheap and would confirm or kill
candidate 1 outright.

---

**(e) The MATLAB was run on the WORKING thermal-hydraulic path.** `neacrpa2.m`
sets no `th_model`, so both codes default to two-fluid — whose 1-D kernel is
missing (21 `Undefined function` warnings in the MATLAB run), leaving the
coolant frozen. Forcing `th_model = 'hem'`, the only functional path:

| A2 Phase-0, `hem` | MATLAB | this port (before Z1) |
|---|---|---|
| termination | **converged, 23 iterations** | `IterationCap` at 51 |
| `k_eff` | **1.013943** | ~43.2 |
| fuel-temp residual | **0.4942 K** | 1270.05 K |

This **withdrew the conclusion drawn in (c)**. That the two codes fail
identically on a path broken in *both* said nothing about faithfulness; on the
path that works they did not agree at all. X1 was a port defect.

**(f) The feedback handler was diffed against the MATLAB, and it found the root
cause.** Per-node cross sections after `sigmavalupd3d_handler` agreed to only
2e-6 - 2e-5 — far above round-off. The rod-fraction map localised it exactly:
both codes gave 270 rodded nodes, 221 full and 49 partial, but **every partial
fraction differed by precisely 0.01**. Working backwards, this port's cumulative
axial height at each rod tip was **0.3 cm short** — which is **defect Z1**.

**After reproducing Z1:**

| | before | after |
|---|---|---|
| rod-fraction map vs MATLAB | 2.1e-3 | **1.1e-15** |
| feedback handler, worst of six sums | 1.9e-5 | **3.3e-13** |
| A2 coupled solve | cap at 51, `k_eff ~ 43` | **converges in 27** (MATLAB: 23) |
| A2 `k_eff` | ~43.2 | **1.035684** (MATLAB 1.013943) |

**What remains: +2144 pcm on a converged solve** — a far better-posed problem
than the divergence was, and the feedback chain is now cleared as its source.

**(g) The neutronics was bisected against the MATLAB, and it is exact.**

With the SA-nodal correction **frozen** (`nodalupd` huge, so it is built once
from a flat flux), the two codes agree to ten significant figures:
`k_eff = 1.0230689628` in both, residual `9.507775e-7` in both, flux sum
agreeing to 2.9e-11. Sweeping the update interval:

| `nodalupd` | this port | MATLAB |
|---|---|---|
| frozen | 1.0230689628 | 1.0230689628 |
| 200 | 1.0244511444 | 1.0244511444 |
| 100 | 1.0245061195 | 1.0245061195 |
| 50 | 1.0245084306 | 1.0245084306 |
| 20 | 1.0245087144 | 1.0245087144 |
| **6** (default) | **3661.34** | **837.30** |

**Identical at every interval where the solve is stable.** The static
neutronics — operator, boundary conditions, cross sections, nodal build, nodal
refresh and eigenvalue iteration — is faithful in full. At the default interval
of 6 both codes diverge, to different garbage, which is defect N1 on a real case
and the reference's own documented behaviour here.

This also **clears `calc_bucklingxyz`'s cache** as a candidate: it is consulted
on every nodal refresh, and the refreshed results match exactly.

**(h) The remaining gap is entirely in the thermal-hydraulics.** Comparing the
converged A2 `hem` state:

| | this port | MATLAB |
|---|---|---|
| `k_eff` | 1.0356836577 | 1.0139434818 |
| fuel T min/max | 558.10 / **891.19** | 575.29 / **1180.16** |
| coolant T min/max | 558.10 / **559.15** | 559.15 / **605.65** |
| **heat flux sum** | **2.21** | **1.2246e5** |
| power density sum | 9.0194e5 | 8.7949e5 |

The power agrees to 2.6%, but **the heat flux derived from it is five orders of
magnitude too small**, so no heat reaches the coolant. This port's coolant
maximum is *exactly* the inlet temperature and its fuel maximum is *exactly*
`params.fueltempavg` — the initial value unfuelled nodes retain — meaning the
fuelled nodes collapsed to coolant temperature instead of heating.

**(i) The thermal-hydraulics is correct in isolation; the coupled loop breaks
it.** Feeding one `th_solverxyz` pass the power from a frozen-nodal eigensolve
(verified exact against the MATLAB):

| | A2, one pass | MATLAB converged |
|---|---|---|
| heat flux sum | **1.195e5** | 1.2246e5 |
| heat flux max | 158.18 | 142.33 |
| fuel T max | 1124.77 | 1180.16 |
| coolant T max | 560.31 (rising off inlet) | 605.65 |

Nothing is orders of magnitude wrong. **The earlier conclusion in (h) — that
the heat flux is five orders too small — described the converged state
correctly but attributed it wrongly.** The T-H does not compute a bad heat
flux; the coupled loop destroys a good one.

**(j) The per-pass trace shows the fission source going NEGATIVE.** From pass
10 the `pwrdens` handed to the T-H is negative (settling near -3.05e5). The
consequence chain is exact: negative `pinpowdens` makes the rod solve return a
profile below the coolant temperature, `th_solverxyz` clamps it up to the
coolant, and the wall heat flux `hcoeff*(T_surface - T_coolant)` becomes
**exactly zero**. From pass 14 the recorded heat flux halves precisely each pass
— `(1-0.5)*old + 0.5*0` — the fuel pins at `params.fueltempavg` and the coolant
decays to its inlet. The loop then "converges" because nothing is moving.

This **supersedes the under-relaxation hypothesis in (i)**: the relaxation is
faithfully reproduced and is merely the decay envelope, not the cause.

**Per-pass against the MATLAB** (A2, `hem`, full history captured):

```text
MATLAB : 452.9, 3278.6, 127.3, 855.6, 74.4, 35742.1, 329.8,
         1.033735, 1.028019, 1.022682, ... 1.013943   (23 passes)
port   : 87.2, 243.9, 659.9, 1572.9, 30.7, 134.3, 63.9, 453.6,
         110.7, 60.9, 404.9, 618.9, 94.5,
         1.030652, 1.032362, 1.033708, ... 1.035684   (27 passes)
```

**Both codes go through a wild cold-start phase** — the MATLAB reaches 35742 —
so that is not the defect. What differs is the recovery: the MATLAB leaves it at
pass 8 and then **decreases monotonically** to 1.0139 as feedback takes hold,
while this port leaves it at pass 14 and **creeps upward** to 1.0357 and stalls,
its feedback already dead. The MATLAB's first settled value (1.033735) is almost
exactly this port's pass-16 value (1.033708) — both reach the same
neighbourhood, only one keeps going.

Neither code guards negative flux: `fixnegative` is **commented out** at
`sanodaldiffusion_solverxyz.m:204`, and this port matches that.

**(k) RESOLVED — it was chaos, not a mistranslation.** Both the inner tolerance
schedule and the warm start were checked and are faithful (this port stores and
forwards the solver's full five-generation history, as the reference does). The
trajectories nevertheless differ from pass 1 — because at the default
`nodalupd = 6` the inner eigensolve is **unstable on this case**, and a cold
static solve diverges in *both* codes (3661 here, 837 in the MATLAB). Two
implementations integrating an unstable iteration separate at round-off.

Re-running the coupled solve at a **stable** interval settles it:

| A2, `hem`, `nodalupd = 20` | MATLAB | this port |
|---|---|---|
| `k_eff` | 1.0139476080 | **1.0139476080** |
| fuel T max | 1180.1501 | **1180.1501** |
| coolant T max | 605.6450 | **605.6450** |
| heat flux sum | 1.224531e5 | **1.224531e5** |
| pwrdens sum | 8.794949e5 | **8.794949e5** |

Identical to every digit printed, converging monotonically in 16 passes with the
power positive throughout.

**Practical consequence, and it binds the reference equally: NEACRP case A2 must
not be run at the default nodal-update interval.** The default for this mesh is
`ceil((17+17+18)/10) = 6`, and at 6 the coupled answer is meaningless in either
code. The MATLAB's 1.013943 at the default is close to the right answer by luck,
not by construction — its own trajectory passes through `k_eff = 35742` on the
way. See defect **N1**.

---

---

## Missing files

Not defects, but the same origin: the handover is incomplete. These are
referenced by code in the snapshot and were not shipped.

| File | Referenced from | Consequence |
|---|---|---|
| `driftflux6_solverstatic1d.m` | `driftflux6_solverstatic3d.m:157` | The 3-D drift-flux path cannot run as shipped. **This is the default `th_model`**, so every case that does not explicitly select `'hem'` runs with a coolant that cannot heat. Confirmed by running the MATLAB on 2026-08-18: 21 `Undefined function` warnings and a non-converging coupled solve on NEACRP A2. The `try`/`catch` turns a missing solver into a silently frozen coolant  **Name clash to be aware of, 2026-08-23:** the crate now contains a module called `driftflux6_solverstatic1d`, and it is **not** this missing file. It is the renamed wrapper, translated from `driftflux6_solverstatic3d.m`, renamed at the maintainer's direction because it is intended to become the 1-D kernel. The MATLAB file named here is still absent, and the module still delegates to a solve that does not exist. |
| `driftflux_solverstatic1d.m` | comments in `main_exec_diff3d.m`, `th_solverxyz.m` | Owner of every `params.jfnk*` and `params.ptc*` switch — see below |
| `driftflux_eqnstatic1d5.m` | comments in `main_exec_diff3d.m` | Metastable-liquid prototype |
| `enthmix_forward.m`, `enthmix_invert.m` | comments in `main_exec_diff3d.m` | Enthalpy energy-variable option |
| `bwrchfhottest.m` | `coupling` | BWR hottest-channel CHF |
| `test_critboron3.m` | a comment in `neacrpa2t.m` | The critical-boron search that produced case A2's 1139.01 ppm. Without it that number cannot be reproduced or checked — only taken on trust from the comment that records it |

**Consequence worth stating plainly:** because `driftflux_solverstatic1d.m` is
absent, **no JFNK solver exists in the snapshot**. The `params.jfnkprecon` /
`jfnkrel` / `jfnkverb` switches are set by the drivers and read by nothing. The
transient solver as shipped is linear implicit-Euler with exponential
transform. See the "Missing files" section of `docs/bedok-reference-defects.md`.

The single-phase HEM path (`singleflow1devap`) is complete, and it is the path
the benchmark cases actually use.

---

## Known-unfinished by the author's own admission

`main_exec_diff3d.m:38-46` documents a metastable-liquid / RELAP5-3D-style
interfacial vapour-generation prototype and states, in Yan Ren's own words:

> `STATUS: PROTOTYPE - does NOT converge`

with a diagnosis (low void → small interfacial area → no evaporation → runaway
superheat) and what it would need (the full RELAP5-3D interfacial-closure
suite). This is not a defect to fix; it is a documented dead end, and it should
stay documented rather than be quietly attempted.
