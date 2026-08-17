# BEDOK — defect register for the reference snapshot

**Status:** complete for the stage-1 translation (all four modules reported
2026-08-05; the flux-solver entries D1-D7 added 2026-08-13 when
`diffusion_solverxyz.m` and `sanodaldiffusion_solverxyz.m` were translated).
**Nothing here is fixed in stage 1, by design.**
**Parent:** `docs/bedok-port-scoping.md` §1.0.

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

### From `coupling/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| C1 | `sigmavalupd3d_handler.m:56-68` | **`rodlvl` is never initialised** when a bank tip sits at or above the top of a column — i.e. a fully withdrawn bank, *the end state of every rod-ejection transient*. MATLAB then silently reuses the previous `(ix,iy)` column's value. Translated with the same carry-over; a first-column occurrence returns an error. | **High** — hits the primary NEACRP use case |
| C2 | `w3chfhottest.m:22` | **`highy = ix`** where `iy` is meant. The hottest-channel search can only ever return a diagonal column. | **High** — silently wrong CHF location |
| C3 | `thdiffusion_solverxyz.m:191` | **CHF result is computed and never returned** — not placed in `output`. | Medium — dead work, no wrong answer |
| C4 | `criticalboron_xyz.m:150` vs `thdiffusion_solverxyz.m:159` | **Inconsistent `whichsigma` into the T-H solver**: one passes compositions, the other the compacted per-node map. One of the two must be wrong. | **High** — needs resolving before either path is trusted |
| C5 | `fixinfnan.m`, applied after every solve | **Turns divergence into a zero flux**, hiding a failed solve from the residual norms. | **High** — masks failure |
| C6 | `pauseonnan.m` | `if any(isnan(M))` on a matrix is true only when *every column* contains a NaN. A single NaN, or a whole NaN row, passes silently. Reproduced exactly, with a test. | Medium |
| C7 | Phase-2 re-equilibration (5000 its), `eigsolvecold` (8000 its) | **Silent non-convergence** — exit on the iteration cap with no error and no flag. | **High** — a non-converged answer is indistinguishable from a converged one |
| C8 | `sigmavalupd3d_handler.m` `fuelmask` | Hard-codes `whichsigmaref >= 4`, the NEACRP composition numbering. Any other case silently gets wrong C2/C3/C4 outputs. | Medium — case-specific |
| C9 | C5/C6 outputs | Hard-code the NEACRP 18-block axial model (layers 6 and 13, offset `L*zscale`). Meaningless for a differently blocked case; `radialmaplayer` will index past the mesh. | Medium — case-specific |
| C10 | Transient driver, `params.steadyfile` | **No validity check** on the cached steady state, though its own header says the cache must match the current case and params. (`criticalboron_xyz` at least sanity-checks the cached `k_eff`.) | Medium |
| C11 | Transient operators | **`Nc > 0` does not conform** — the steady flux is `philen` long, the transient operators `philenf`. All four cases set `Nc = 0`, so it is untested. | Low — untested path |
| C12 | `fixnegativematrix.m` | Operates on `find(mat)`, i.e. only stored non-zeros. Harmless for dense arrays; a trap if these become sparse. | Low |
| C13 | `sigmavalupd3d_handler.m:71` | Writes `rodfrac.csv` **unconditionally** inside the feedback update — once per Picard pass per time step. | Low — I/O noise |
| C14 | `thdiffusion_solverxyz.m:209-211` | Fuel-temperature history trim reports `Inf` as the residual when the loop breaks before any T-H update. | Low |

### From `cases/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| K1 | `neacrpa1t.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpd1.m` — `geometry.fuel.Vi` | **Fuel node volumes are identically zero for nodes 2:20 in all four NEACRP cases.** `rminus=sum(Lr(i-1))` and `rplus=sum(Lr(i))` are single-element sums, so they give node *thicknesses*, not cumulative radii; with uniform pellet nodes the annulus area comes out zero. Reproduced as-is; `node_area_corrected()` computes the intended annuli **for comparison only**. | **High** — fuel volumes feed the rod thermal solve |
| K2 | `convertindexc2d.m`, half-index → nodal branch | **Never round-trips.** `ceil(…)/2` and `(…+1)/2` produce half-integers for *every* index the forward map generates (1 → 2 → 2.5). MATLAB would fail inside `sparse`. Confirmed numerically; the port returns an error. | Medium — dead in the four cases, a trap if used |
| K3 | `convert_grid3d.m`, `Nc` branch | `(G+Nc-1)*energyindexstep` inside `for nn=1:Nc` gives every extra unknown the *same* index — a typo for `G+nn-1`. Dead, since `Nc=0` everywhere (cf. C11). | Low — untested path |
| K4 | `handle3dcoords.m` generic branch | Sets `maxi3 = params.maxix` — typo for `maxi3`. `handle2dcoords.m` has no default branch at all. | Medium |
| K5 | `convertindexc2d.m` | Reads `params.maxi1`/`maxi2` directly, while its only caller goes through `handle2dcoords`. A `params` carrying `maxix`/`maxiy` satisfies the caller and then errors in the callee. | Medium |
| K6 | `neacrpd1.m` | **Header says "Case D2"** while the file builds D1. | Low — but misleading |
| K7 | `neacrpd1.m` | `sigmavalues.fp` **entirely commented out**; `neacrpd1t.m` rebuilds it. The reference's own repair, ported as written. | Medium |
| K8 | `neacrpd1.m` | `params.boron = 1000` with **no boron table** to interpret it. | Medium |
| K9 | `neacrpd1.m` | **No coolant-temperature feedback**, and the control-rod section is a header with an empty body. | **High** for D1 — a feedback path the case needs is simply absent |
| K10 | `neacrpd1.m` | `th.flowrate` assigned twice; the first is dead. | Low |
| K11 | `geometry_ends3d.m` | Reports the full axis for an all-void line, and finds only the **first** contiguous run. | Medium |
| K12 | `th.gtube` | Not divided by the refinement factors, while `th.nfuelpin` is. | Medium |
| K13 | `hydia` | Wetted perimeter `2*pi*R + 4p - 8R` is non-standard. | Medium |
| K14 | `convertsparsekey3d.m` | Diagnostic hard-codes a 19x17x17 grid. | Low |
| K15 | `fixinfnan.m` | Its "min over finite values" is only true because MATLAB's `min` skips NaN — accidental, not intended (cf. C5). | Low |
| K16 | `geom2dxycase1.m` | `constants.chi=[1;1]` is per-material, not materials x G; the two coincide only at `G=1`. | Low |
| K17 | `iaea3ds.m` | Computes its scale factors **after** forcing the grid, so they are permanently 1. | Low — inert |

### From `th/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| T1 | `fuelrodheat_1dcylnd.m` / `…time` | **The gap ring is an orphan row fixed at `T = 1 K`.** Its diagonal is never overwritten from the identity seed and no other row references that column — the pellet surface couples straight across to the clad inner surface (`laplccol = id+2`). Harmless only because `th_solverxyz.m:185` clamps the profile afterwards. Pinned by a test. | **High** — a physically meaningless gap temperature, masked downstream |
| T2 | same | **A rod with no material→gap transition indexes past its own matrix.** With no gap, `surfcount = 0` so `maxid = maxir`, yet the `ir == maxir` branch writes column `maxid+1`. Hard error in MATLAB; raised as `UnsupportedRodLayout` here. | Medium — unreachable in the benchmark geometries |
| T3 | same | **A material→material interface with no gap assembles no conduction coefficient.** A direct fuel→clad interface sets `surf = 1`, but the `surf` branch only acts when `whichk(ir+1) == 0`, so no off-diagonal is added and `kplus` keeps a stale value: the diagonal becomes `2*kplus_previous` and the two sides are uncoupled. | Medium — unreachable in the benchmark geometries |
| T4 | `w3chfhottest.m:21` | `highy = ix` rather than `iy` — same defect as C2, found independently. | **High** |
| T5 | `w3chf.m` | **`enthshift` is not the inlet enthalpy the W-3 `K4` factor requires.** It builds a local two-node average `(0.5*enth(i)+0.5*enth(i-1))/2` with a stray extra `/2`, roughly doubling the apparent subcooling. Only `enthshift(1)` is the true inlet value. | **High** — wrong CHF magnitude |
| T6 | `w3chf.m` | That `i-1` walk **runs over the flat index, not along a channel**. Since `iz` varies fastest, the first node of every channel mixes in the top node of the *previous* channel. | **High** |
| T7 | `singleflow1devap.m:124,159` and elsewhere | **`Tsat - 2*eps` is a no-op in f64.** At `Tsat ~ 618 K` one ULP is ~1e-13, three orders above `2*eps`, so the intended "nudge below saturation" never happens — in the original either. A live parity risk for the IF97 substitution (§3). | Medium |
| T8 | `singleflow1devap.m:105` | Comment says "steam at 900 K"; the code passes 1050 K. | Low |
| T9 | `th_solverxyz.m:192` | Sets `fueltempavg := fueltempdoppler`; the actual volume average is commented out at line 189. | Medium |
| T10 | `driftflux6_solverstatic3d.m:129` | Computes "quality" by dividing by the latent heat while subtracting the *local* liquid enthalpy, so it is not the equilibrium quality outside saturation. | Medium |
| T11 | `th_solverxyz.m:149` | **`real(x^p)` on a negative base.** MATLAB returns `abs(x)^p * cos(p*pi)`, not `NaN`. Reproduced exactly via `matlab_real_powf` — this would otherwise have been a silent behavioural divergence. | Medium — a translation trap, not a reference bug |
| T12 | `th_solverxyz.m`, `w3chf.m` | Dead code: `Vi = repmat(...)`, `subflow`, `Lx`, `Ly`, `Lr`, `maxir`, `whichg`; `subarea`, `gearth`. | Low |

### From `nodal/` (reported 2026-08-05)

| # | Where | Defect | Severity |
|---|---|---|---|
| N1 | `sanodaldiffusion_solverxyz.m` | **`nodal_update_interval == 1` destabilises the solver.** The MATLAB comment claims small values improve stability; observed behaviour is the opposite — a 3³ leaking cube (`k_inf = 1`) runs to the 5000-iteration ceiling and reports `k_eff = 1.114`. Interval >= 2 converges to -103 pcm of finite difference. **The MATLAB default `ceil((nx+ny+nz)/10)` IS 1 for any mesh with `nx+ny+nz <= 10`.** Real benchmarks are safe (IAEA-3D gives 6). Not confirmed against MATLAB, since the reference is not run. **Confirmed within the translation 2026-08-13**, now that the solver itself is ported: on a uniform leaking cube, interval 1 runs to the 5000-iteration cap at meshes 3, 4 and 5, reporting `k_eff` 3.271 / 8.397 / 2.082 against converged interval-3 values of 2.128 / 2.260 / 2.335. The 4-cube figure exceeds `k_inf = 2.5`, so it is diverging, not merely slow. Pinned by two tests, one of which reaches the failure through the *default* interval rather than by setting it. | **High** for small meshes |
| N2 | `calc_sanodalxyz.m`, `calc_transleakagexyz.m` | **`Nc > 0` cannot work** — a `philen`-square operator is added to `philenf`-square ones, and a `philen`-wide operator multiplies a `philenf`-long flux. MATLAB dimension errors; the port panics identically. (cf. C11, K3.) | Low — untested path |
| N3 | boundary blocks | **A single-node direction is not a supported mesh** — `Lxb(xlow+1,…)` indexes out of bounds. Two nodes per direction minimum. | Low |
| N4 | `makesigmadfxyz.m` mode 2 | Loop bound `for iz=m:m:maxiz` is missing the `m*` factor the `ix`/`iy` bounds carry, so with `m=2` it covers only half the axial nodes. Mode 2 is unreachable from the SANM path; not ported. | Medium if ever reached |
| N5 | `makesigmadfxyz.m` | **`sigma.fp` omits the `nu` factor that `sigma.f` includes.** Asymmetric, and nothing in the snapshot says whether `sigmavalues.fp` is meant to carry it already. | Medium — ambiguous intent |
| N6 | `makesigmadfxyz.m` | **`nu` is read two different ways in one function** — `nu(m)` for `sigma.nu`, `nu(m,g)` for `sigma.f`. The scalar-broadcast path `nu*ones(G)` builds a G x G matrix, which works only by accident when `n_materials <= G`. | Medium |
| N7 | `calc_a1_expansionxyz.m` | A reflective outer face over a node with `D == 0` makes the G x G system **exactly singular**; MATLAB warns and emits `Inf`/`NaN`. Reproduced (the dense solver deliberately does not error on a zero pivot). | Medium |
| N8 | `calc_a1234_expansionxyz.m` | The **`1e6` magic substitution** for a zero diffusion coefficient leaks into `Ssource` at out-of-core nodes. | Medium |
| N9 | `calc_ABEFGHxyz.m` | **No small-`alpha` expansion** — every coefficient is a ratio of vanishing differences, `NaN` at `alpha = 0` and losing significance well before that. | Medium |
| N10 | `sanodaldiffusion_solverxyz.m` | **Normalisation comment is wrong**: it says "fission source integration = 1", but the code preserves whatever the *initial* integral was. It also normalises by a plain `sum` while `diffusion_solverxyz` uses a 1-norm. | Medium — inconsistent between the two solvers |
| N11 | `calc_sanodalxyz.m` | The near-zero-flux guard **silently falls back to finite difference**, on a threshold relative to the *global* flux maximum; and the shared `nodalterms(idxplus,5)=nodalterms(idx,6)` assignment sits *outside* the guard, so a neighbour can receive a stale value. | **High** — silent method change mid-solve |
| N12 | boundary-condition `switch` statements | **No `otherwise` branch** — a typo'd string silently leaves coefficients at zero. The `BoundaryCondition` enum makes that unrepresentable without changing behaviour for the three real values. | Medium |
| N13 | `makegradDxyz.m` | `gradterms=2*gradterms; %check this (seems correct)` — **the author's own uncertainty**, preserved verbatim. | Medium — flagged by the author |
| N14 | `calc_a1_expansionxyz.m` | Cosmetic: the x high-boundary block is commented `%zhi node`. | Low |

### From the two flux solvers (reported 2026-08-13)

Found while translating `diffusion_solverxyz.m` and
`sanodaldiffusion_solverxyz.m`, the last two SANM files. These are additions to
the register, not revisions of it — N1, N2 and N10 already covered the nodal
solver and still stand.

| # | Where | Defect | Severity |
|---|---|---|---|
| D1 | `diffusion_solverxyz.m:60-76, 160-174` | **The empty-grid compaction is dead code.** Both the compaction and its inverse are guarded by `keychange == 1`, where `keychange` is the literal `0` assigned four lines above. So a case with void nodes never gets the compacted operator the author wrote it for, and `convert_grid3d` / `convertsparsekey3d` are called from nowhere on this path. Not translated — there is nothing to reproduce, and writing an untested compaction would be inventing behaviour. | Medium — a written-but-disabled optimisation; someone will eventually flip it and find it untested |
| D2 | both solvers, the non-convergence `break` | **The reported state lags one iteration behind the failure.** The `break` fires before `iteration` is incremented, so the returned `k_eff`, `residual` and `k_eff_residual` are the *previous* pass's — the offending value that triggered the break is computed, tested, then discarded. `sanodaldiffusion_solverxyz` prints the new `k_eff` in its bail-out message but still returns the old one, so the printed and returned values disagree. | Medium — a bailed-out run returns plausible-looking numbers |
| D3 | `diffusion_solverxyz.m:48, 51, 54, 194` | **Four `writematrix` CSV dumps run unconditionally**, on every call, inside the inner solver. Its nodal counterpart puts the equivalent ten behind `params.debugdump`; this one has no switch at all. Every call scribbles `sigmafxy.csv`, `sigmasxy.csv`, `sigmatxy.csv` and `rel_power_inner.csv` into the working directory. Not reproduced as file writes — the quantities are returned instead; see the translation note below. | Medium — makes the solver unsafe to call concurrently and surprising to call at all |
| D4 | `sanodaldiffusion_solverxyz.m:73, 165, 175-177, 189-193` | **Dead Wielandt-shift scaffolding.** `weilandtfactor = 1.05` and `weilandt = 0` are set; every use is commented out, including the `weilandt = 1` that would activate it, so the flag can never change. The author's own comment says "does not seem to work". Preserved as written; the shift is not implemented in the translation either. | Low — inert, but it reads as a live feature |
| D5 | `sanodaldiffusion_solverxyz.m:251-253` | **The final normalisation pairs mismatched vectors on an early break.** `norm_factor = sum(fission_source_new)` is applied to `fission_source`. On a normal exit those are the same vector; on a `break` they are one iteration apart, so a diverging run's source is rescaled by a factor taken from a *different* and worse source. | Medium — only reachable on a failed solve, but it corrupts the diagnostic output of exactly that case |
| D6 | both solvers | **Silent non-convergence**, the same shape as C7: the iteration exits on `k_eff <= 0`, `isnan(k_eff)` or the iteration cap with no flag in `output`. `sanodaldiffusion_solverxyz` at least prints a message; `diffusion_solverxyz` prints nothing distinguishable. The translation adds a `Termination` enum to the output rather than reproducing the silence — see below. | **High** — a non-converged answer is indistinguishable from a converged one |
| D7 | `sanodaldiffusion_solverxyz.m:36-43` | **The warm-start speed claim does not generalise.** The comment states a seeded flux "greatly reduces the source-iteration count". Measured across twelve cold/warm pairs (meshes 3/4/5, `nodalupd` 2/3/5/10) on a uniform leaking cube, nine improved or were neutral — best case 115 iterations down to 65 — but three were **worse warm than cold**: 618→937 (3-cube, interval 2), 84→93 (3-cube, interval 3) and 107→132 (5-cube, interval 2). All twelve agreed on `k_eff` to 3.7e-6 relative, so the warm start is *correct*; only the speed claim is overstated, and every regression sits at a small nodal-update interval — the marginally-stable regime N1 is about. Pinned by a test. | Low — a documentation overclaim, not a wrong answer |

### From `th/` (reported 2026-08-13)

Found while translating the thermal-hydraulics layer. This section grows as
that layer lands; entries below cover `w3chf.m` and two files read while
mapping the dependency order.

| # | Where | Defect | Severity |
|---|---|---|---|
| T1 | `w3chf.m:34` | **The upwind enthalpy is halved.** `enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2` is a two-point average with a stray extra `/2`. Nothing in W-3 motivates a factor of a half, and `enthshift(1) = enthin` takes the *full* inlet enthalpy, so the two branches contradict each other. Halving the enthalpy raises `hLsat - enthshift`, raising `Kfour` and so **overpredicting** the critical heat flux. Measured at PWR conditions (15.5 MPa, 560 K inlet, quality -0.05): **+22.8%** on the predicted CHF, growing with subcooling. Pinned by a test. | **High** — a large, systematic, *non-conservative* error in a quantity whose only purpose is to bound a safety margin |
| T2 | `w3chf.m:31-35` | **Local enthalpy where published W-3 uses inlet enthalpy.** The correlation's subcooling term is defined against the channel inlet enthalpy, constant along the channel; the reference substitutes a per-node upwind-shifted local value that reduces to the inlet only at node 1. This may be a deliberate local-conditions variant or the same unfinished edit as T1 — the snapshot says nothing. Preserved and flagged. | Medium — ambiguous intent, changes the correlation's meaning |
| T3 | `w3chf.m:14, 16, 56-58` | `gearth` is assigned and never used; `subarea` is read from the geometry and never used; and three `writematrix` calls dump CSVs unconditionally on every call, as in D3. | Low |
| T4 | `makeheatlaplacian_1dcylnd.m` | **Dead code — the file is reachable only from a commented-out line.** Its sole call site, `th_solverxyz.m:174`, is commented out; the live path is `fuelrodheat_1dcylnd.m`, which assembles its own conduction matrix inline with a **different formula** (harmonic-mean interface conductivity against this file's `2*cond(ir+1)*sumLr(ir+1)/Lr(ir+1)`) and an interface-node-doubling scheme this file does not have. So the snapshot ships two divergent discretisations of the same operator, one of them unreachable. | Medium — a reader cannot tell which is intended, and the dead one is the more readable |
| T5 | `makeheatlaplacian_1dcylnd.m:62-67` | Within that dead file, the `whichk(ir+1)==0` branch writes to column `ir+2`, which at `ir = maxir-1` is `maxir+1` — **outside the declared `sparse(..., maxir, maxir)` shape**. MATLAB would raise an index error. Unreachable in practice only because the file is dead (T4). | Medium if T4 is ever undone |
| T6 | `calc_tcond.m` | Called from nowhere in the snapshot. It is the guarded per-node conductivity lookup that `makeheatlaplacian_1dcylnd.m` open-codes *without* its `whichk(i) ~= 0` guard — so the utility exists, is correct, and is not used by the code that needed it. | Low |
| T7 | `fuelrodheat_1dcylnd.m:70` | **The gap node is a dummy pinned at `T = 1`.** A node with `whichk == 0` gets `bvec(id) = 1` and keeps the preallocated diagonal `1`, and nothing else writes its row — conduction bridges from `id` to `id+2` around it. So it solves to exactly `1`, in kelvin. Confirmed by test at three power/coolant combinations. The live path survives it: `th_solverxyz.m` clamps the profile up to the coolant temperature on the next line, and takes its Doppler average from the centre and pellet-surface nodes rather than a volume average. But the value **is** in the returned vector, and the volume-average line that `th_solverxyz.m` has commented out would have consumed it. | Medium — contained today, and a trap for the next caller |
| T8 | `fuelrodheat_1dcylnd.m:79-90` | **A missing `else` leaves a stale conductance.** The `surf == 1` pass wraps its whole body in `if whichk(ir+1) == 0` with no `else`. Two *different, both solid* adjacent materials — fuel directly against cladding, no gap — reach that pass, emit no forward triplet at all, and then compute `laplcele(id) = kminus + kplus` from the **previous** iteration's `kplus`. The result is a row with an inflated diagonal and a missing off-diagonal, i.e. an operator that silently stops conserving energy. Unreachable for the benchmark meshes, which always model a gap. | Medium — latent; it would be silent if ever reached |
| T9 | `fuelrodheat_1dcylnd.m` | Three smaller things in the same file: `temps` is indexed to `id + 1` at the last unknown, so it must be `maxid` long where the mesh has only `maxir` nodes (a caller sizing it from `maxir` reads past the end); the `ir == maxir` branch reads `whichk(ir)` for *both* sides of its harmonic mean, making it a self-mean; and the `whichk(ir+1) == 0` branch multiplies its conductance by an extra `2`, with the un-doubled line commented out directly above and no derivation given. | Low-Medium |

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

---

## Missing files

Not defects, but the same origin: the handover is incomplete. These are
referenced by code in the snapshot and were not shipped.

| File | Referenced from | Consequence |
|---|---|---|
| `driftflux6_solverstatic1d.m` | `driftflux6_solverstatic3d.m:157` | The 3-D drift-flux path cannot run as shipped |
| `driftflux_solverstatic1d.m` | comments in `main_exec_diff3d.m`, `th_solverxyz.m` | Owner of every `params.jfnk*` and `params.ptc*` switch — see below |
| `driftflux_eqnstatic1d5.m` | comments in `main_exec_diff3d.m` | Metastable-liquid prototype |
| `enthmix_forward.m`, `enthmix_invert.m` | comments in `main_exec_diff3d.m` | Enthalpy energy-variable option |
| `bwrchfhottest.m` | `coupling` | BWR hottest-channel CHF |

**Consequence worth stating plainly:** because `driftflux_solverstatic1d.m` is
absent, **no JFNK solver exists in the snapshot**. The `params.jfnkprecon` /
`jfnkrel` / `jfnkverb` switches are set by the drivers and read by nothing. The
transient solver as shipped is linear implicit-Euler with exponential
transform. See `docs/bedok-port-scoping.md` §2.

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
