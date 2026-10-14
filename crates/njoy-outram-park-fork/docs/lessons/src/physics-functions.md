# Where the physics is

> **Proposal for maintainer review (2026-10-14), not yet accepted.**
> GitHub issue #868, Part A. Nothing on this page is settled until the
> maintainer has reviewed it; the lessons' existing Code Review map links
> (which start at each rung's entry point) stay as they are until then.

Each lesson's "In the code" step links into the
[Code Review map](../../code-review/) at an **entry point**: the function a
caller would use. From there the path down to the physics runs through
drivers, tape readers, grid bookkeeping and error handling. This page skips
that path. For each rung it names only the functions that **compute the
physics**: a formula, a kernel, or the algorithm NJOY's routine implements.
Every link opens that function's card on the map, with its source and its
review stamp.

What is left out on purpose: entry points, drivers, ENDF readers and writers,
ACE layout and serialisers, error plumbing, interpolation-table lookups and
formatting. That software glue belongs in the deep dive (#868 Part B), not in
the lessons.

How the lists were made: candidate functions were taken from each lesson's
"reading the path" table and from each module's source, and each one was kept
only if its body computes something a physicist would recognise. Each link
target was checked against the source on 2026-10-14: the file exists, it
defines a function of that name, and for a `Type::fn` id the function sits
inside that type's `impl`. The upstream column comes from each function's own
doc comment, checked against the subroutine names in NJOY2016's `src/`.
Borderline calls are marked **(borderline)** in the last column, with the
reason.

<a id="where-sigma-comes-from"></a>

## Rung 0 — where σ(E) comes from

No physics function of its own. This page is the map of the pipeline: it
names the rungs and the route a Monte Carlo nuclide takes through them. Its
physics is the resonance reconstruction of [rung 2](#reconr) and the Doppler
broadening of [rung 3](#broadr).

<a id="endf"></a>

## Rung 1 — ENDF

No physics function, as an honest answer. This rung is about the **format**:
how an evaluation is laid out on a tape and read back. The one function
close to physics is the interpolation law, because the evaluator chooses it as
a statement about how σ varies between tabulated points.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`terp1`](../../code-review/#crates/njoy-outram-park-fork/src/endf/interp.rs::terp1) | σ between two tabulated points under the evaluator's ENDF law (histogram, lin-lin, y linear in ln x, ln y linear in x, log-log) | `endf.f90` `terp1` | **(borderline)** the law is part of the evaluated data, but the function is a generic interpolator; it may belong in Part B |

<a id="reconr"></a>

## Rung 2 — RECONR

The physics: σ(E) at 0 K from resonance parameters, through the resonance
formalism the evaluation names (SLBW, MLBW, Reich-Moore, Adler-Adler, R-matrix
limited), built from the barrier penetrability, shift factor and hard-sphere
phase shift of each partial wave, then made into a lin-lin table to a stated
tolerance.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`shift_and_penetrability`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/slbw.rs::shift_and_penetrability) | the penetrability P_l(ρ) and shift factor S_l(ρ) of the centrifugal barrier for partial wave l | `reconr.f90` `facts` | the closed-form barrier functions every formalism uses |
| [`phase_shift`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/slbw.rs::phase_shift) | the hard-sphere phase shift φ_l(ρ), which sets potential scattering and its interference with resonances | `reconr.f90` `facphi` | a closed-form formula |
| [`eval_slbw_lstate`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/slbw.rs::eval_slbw_lstate) | single-level Breit-Wigner capture, fission and elastic for one l-state (LRF=1) | `reconr.f90` `csslbw` | the SLBW formula itself |
| [`eval_mlbw_lstate`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/slbw.rs::eval_mlbw_lstate) | multi-level Breit-Wigner: elastic with level-level interference summed per spin J (LRF=2) | `reconr.f90` `csmlbw` | the MLBW formula itself |
| [`eval_rm_range`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/rm.rs::eval_rm_range) | Reich-Moore cross sections from the reduced R-matrix of each (l, J), in upstream's summation order (LRF=3) | `reconr.f90` `csrmat` | the Reich-Moore formalism, including its 3×3 matrix inversion |
| [`eval_aa_range`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/aa.rs::eval_aa_range) | Adler-Adler cross sections with their background polynomials (LRF=4) | `reconr.f90` `csaa` | the Adler-Adler formula itself |
| [`cssammy`](../../code-review/#crates/njoy-outram-park-fork/src/samm/xsformula/cssammy.rs::cssammy) | the full multichannel R-matrix cross sections for an R-matrix-limited evaluation (LRF=7) | `samm.f90` `cssammy` | the R-matrix formalism; it calls `setr` (R and level matrix) and `crosss` (per-pair cross sections) |
| [`refine_resonance_grid`](../../code-review/#crates/njoy-outram-park-fork/src/reconr/mod.rs::refine_resonance_grid) | the adaptive energy grid: panels are bisected until lin-lin interpolation of the resonance σ is within `err`, with the resonance-integral test | `reconr.f90` `resxs` | **(borderline)** a numerical method, not a formula; kept because the tolerance it enforces is what makes the tabulated σ(E) a faithful stand-in for the formula |

In the unresolved range RECONR computes the infinitely-dilute average cross
sections with UNRESR's kernel,
[`unresolved_cross_sections`](../../code-review/#crates/njoy-outram-park-fork/src/unresr/mod.rs::unresolved_cross_sections),
at a dilution of 10¹⁰ b; that kernel is listed under [rung 4](#purr).

<a id="broadr"></a>

## Rung 3 — BROADR

The physics: the 0 K σ(E) convolved with the target nuclei's Maxwellian
thermal motion (the SIGMA1 kernel, exact for a free gas and a piecewise-linear
σ), up to the energy where the evaluation stops being pointwise.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`Walk::bsigma`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::Walk::bsigma) | the Doppler-broadened σ at one energy: the free-gas convolution integral over every 0 K panel, summed exactly | `broadr.f90` `bsigma` | the SIGMA1 kernel |
| [`Kernel::hunky`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::Kernel::hunky) | the H-functions, differences of the F-functions across one panel, that the exact panel integral is built from | `broadr.f90` `hunky` | part of the closed-form convolution |
| [`funky`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::funky) | F_n(a) = ∫ zⁿ exp(−z²) dz / √π from a to ∞, n = 0..4 | `broadr.f90` `funky` | the integrals of the Maxwellian kernel |
| [`hnabb`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::hnabb) | H_n(a, b) by direct Taylor expansion when the panel is so narrow that the F-difference cancels | `broadr.f90` `hnabb` | the same integral, in the regime where the closed form loses precision |
| [`Walk::broadn`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::Walk::broadn) | the output grid: broadened points are added at midpoints until the broadened σ is linear to `errthn` | `broadr.f90` `broadn` | **(borderline)** the grid-refinement loop, like RECONR's; kept because the broadened curve is only as good as this tolerance |
| [`broadening_limit`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/mod.rs::broadening_limit) | `thnmax`: the top of the resolved range (capped at 6.5 MeV and the evaluation's EMAX), above which σ is an energy average that must not be broadened | `broadr.f90` `broadr` (`thnmax` logic) | **(borderline)** a limit, not a formula; kept because it encodes a physical fact about the data, and its absence was this port's worst BROADR defect (the U-238 20 keV seam) |

<a id="purr"></a>

## Rung 4 — UNRESR and PURR

The physics: in the unresolved range only average resonance parameters are
known. UNRESR computes self-shielded averages analytically (the ETOX /
Bondarenko method, via width-fluctuation integrals); PURR samples resonance
ladders from the Wigner and Porter-Thomas distributions and bins the sampled σ
into probability tables.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`unresolved_cross_sections`](../../code-review/#crates/njoy-outram-park-fork/src/unresr/mod.rs::unresolved_cross_sections) | effective self-shielded total, elastic, fission and capture at one energy, for each background dilution σ₀ | `unresr.f90` `unresl` | the ETOX average over the resonance statistics |
| [`ajk`](../../code-review/#crates/njoy-outram-park-fork/src/unresr/wfun.rs::ajk) | the J and K width-fluctuation integrals that carry the self-shielding | `unresr.f90` `ajku` | the Bondarenko integrals |
| [`uw`](../../code-review/#crates/njoy-outram-park-fork/src/unresr/wfun.rs::uw) | the complex probability integral w(z) = e^(−z²) erfc(−iz), the Doppler-broadened line shape | `unresr.f90` `uw` | the line-shape function (PURR's twin is `purr/wfun.rs` `uw2`, `purr.f90` `uw2`) |
| [`penetrability_factor`](../../code-review/#crates/njoy-outram-park-fork/src/unresr/mod.rs::penetrability_factor) | the unresolved-range penetrability V_l(ρ) and phase shift, which make Γn energy dependent | `unresr.f90` `uunfac` | the barrier formula in the unresolved convention |
| [`generate_ladder`](../../code-review/#crates/njoy-outram-park-fork/src/purr/ladder.rs::generate_ladder) | one sampled resonance ladder: Wigner-distributed spacings, chi-squared (Porter-Thomas) widths | `purr.f90` `ladr2` | the statistical model of resonances |
| [`probability_table`](../../code-review/#crates/njoy-outram-park-fork/src/purr/unrest.rs::probability_table) | the Monte Carlo core: σ evaluated over many ladders, binned into total-σ bands, with the conditional partials and Bondarenko moments per band | `purr.f90` `unrest` | the probability-table method itself |
| [`line_shape`](../../code-review/#crates/njoy-outram-park-fork/src/purr/unrest.rs::line_shape) | the Doppler line shape at one sampled point, in upstream's four precision tiers | `purr.f90` `unrest` (lines 1950-2204) | the ψ/χ line shapes that broaden each sampled resonance |
| [`infinite_dilution_reference`](../../code-review/#crates/njoy-outram-park-fork/src/purr/ladder.rs::infinite_dilution_reference) | the analytic infinite-dilution average σ, which the sampled tables must reproduce | `purr.f90` `unresx` | **(borderline)** it is physics, used as a convergence check on the sampling |

<a id="thermr"></a>

## Rung 5 — THERMR and LEAPR

The physics: below a few eV the target's binding and motion matter. Inelastic
scattering follows the scattering law S(α, β) (with the short-collision-time
approximation outside its table); coherent elastic scattering is Bragg
diffraction from crystal planes; incoherent elastic scattering is set by the
Debye-Waller integral. `calcem` then turns the inelastic kernel into
outgoing-energy and equally-probable-cosine tables.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`IncoherentInelastic::double_differential`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/inelastic.rs::IncoherentInelastic::double_differential) | the double-differential inelastic cross section d²σ/dE′dμ from the interpolated S(α, β) | `thermr.f90` `sig` (tabulated branch) | the thermal scattering kernel |
| [`IncoherentInelastic::sct_double_differential`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/inelastic.rs::IncoherentInelastic::sct_double_differential) | the same kernel beyond the (α, β) table: the short-collision-time Gaussian with the effective temperature T_eff | `thermr.f90` `sig` (label 170) | the asymptotic physics outside the tabulated law |
| [`sigl`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/calcem/sigl.rs::sigl) | σ(E→E′) and its equally probable scattering cosines at one (E, E′) | `thermr.f90` `sigl` | integrates the kernel over μ and divides it into equal-probability bins |
| [`compute_iform0`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/calcem/iform0.rs::compute_iform0) | `calcem`'s walk: for each incident E, the adaptive E′ grid, σ_inel(E) and the emission rows | `thermr.f90` `calcem` (`iform = 0`) | the algorithm that builds the inelastic cross section and its secondary distribution from the kernel |
| [`CoherentElastic::cross_section`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/coherent.rs::CoherentElastic::cross_section) | coherent elastic σ = S(E, T)/E, zero below the first Bragg edge | `thermr.f90` `sigcoh` / `coh` | Bragg scattering |
| [`CoherentElastic::bragg_reflections`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/coherent.rs::CoherentElastic::bragg_reflections) | the discrete Bragg reflections at E: cosines μᵢ = 1 − 2Eᵢ/E and their weights | `thermr.f90` `coh` | the angular side of Bragg scattering |
| [`IncoherentElastic::iel_record`](../../code-review/#crates/njoy-outram-park-fork/src/thermr/incoherent_elastic.rs::IncoherentElastic::iel_record) | incoherent elastic σ(E) and its equally probable cosines from the Debye-Waller integral (H in ZrH) | `thermr.f90` `iel` | the incoherent-elastic formula, in upstream's arithmetic |

Considered and left out: `thermr/calcem/mod.rs` `sig`, which is
`double_differential` plus NJOY's `sigmin = 1e-10` floor (a numerical cutoff,
though it changes the row lengths, #865); and `sigu` / `compute_iform1`, the
`iform = 1` route that the ACE path does not use.

<a id="heatr"></a>

## Rung 6 — HEATR and GASPR

The physics: the energy a reaction leaves behind locally (KERMA), from
kinematics or by energy balance (incident energy plus Q minus the energy
carried away by neutrons and photons); the share of a recoil's energy that
displaces atoms (Lindhard's partition); and how many hydrogen and helium
atoms each reaction makes.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`single_neutron_factor`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/spectra.rs::single_neutron_factor) | the 2A/(A+1)² factor of two-body elastic recoil heating | derived in the doc comment (HEATR's kinematic-limit check, `kchk`) | a kinematics formula |
| [`kinematic_per_event`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/kerma.rs::kinematic_per_event) | the kinematic-limit heating per event for each reaction model (H1–H5: elastic, capture, single neutron, fission, multi-neutron) | `heatr.f90` (kinematic-limit KERMA, `kchk`) | the per-reaction heating formulas |
| [`Heatr::disbar`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/driver/disbar.rs::Heatr::disbar) | mean outgoing neutron energy and damage for a two-body reaction, from the MF=4 angular distribution | `heatr.f90` `disbar` | two-body kinematics averaged over the angular law |
| [`Heatr::capdam`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/driver/disbar.rs::Heatr::capdam) | the recoil damage of radiative capture (the recoil from the emitted photon) | `heatr.f90` `capdam` | recoil kinematics |
| [`lindhard_damage`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/damage.rs::lindhard_damage) | the Lindhard-Robinson partition: the part of a recoil's energy that goes into displacements rather than electronic excitation | `heatr.f90` `df` | the damage-energy model |
| [`Heatr::gambar`](../../code-review/#crates/njoy-outram-park-fork/src/heatr/driver/gheat.rs::Heatr::gambar) | the mean photon energy of an MF=15 continuum and, for capture, the mean recoil and its damage | `heatr.f90` `gambar` | the photon side of the energy balance |
| [`gas_channel`](../../code-review/#crates/njoy-outram-park-fork/src/gaspr/mod.rs::gas_channel) | which light particles (p, d, t, ³He, α) each reaction emits, including the breakup channels | `gaspr.f90` (lines 501-820) | **(borderline)** a table rather than a formula, but it is the reactions' nuclear content (nucleon bookkeeping), not software |

Two implementations exist: the typed `Kerma` (the first two rows), which is
readable, and the routine-by-routine port in `heatr/driver/` (the others),
which is byte-identical to NJOY2016's tape. Which of them the lesson should
point at is a question for review.

<a id="groupr"></a>

## Rung 7 — GROUPR and GAMINR

The physics: a group constant is a flux-weighted average of σ over the group.
The weighting flux is a chosen spectrum, shielded by the resonances
(Bondarenko's narrow-resonance flux, or a solved slowing-down equation);
transfer matrices add the kinematics of where the scattered particle lands.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`group_integral`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/panel.rs::group_integral) | ∫φ dE and ∫σφ dE over one group, panel by panel; their ratio is the group constant | `groupr.f90` `panel` (the 1-D reduction) | the definition of a group constant |
| [`AnalyticWeight::evaluate`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/weights.rs::AnalyticWeight::evaluate) | the weighting spectrum φ(E): Maxwellian, 1/E, fission and fusion pieces by `iwt` | `groupr.f90` `getwtf` | the physical spectrum shapes |
| [`bondarenko_flux_value`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/unresolved.rs::bondarenko_flux_value) | the narrow-resonance flux φ = C(E)(σ₀ + σ_p)/(σ_t + σ₀) | `groupr.f90` `genflx` | the Bondarenko self-shielding formula |
| [`genflx_slowing_down`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/slowing_down.rs::genflx_slowing_down) | the self-shielded flux from the integral slowing-down equation, heterogeneity included | `groupr.f90` `genflx` (`iwt < 0` branch) | solves the neutron balance |
| [`UnresolvedTable::shield`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/unresolved.rs::UnresolvedTable::shield) | the self-shielded unresolved σ at the run's dilutions, from the UNRESR/PURR tables | `groupr.f90` `getunr` | carries rung 4's shielding into the groups |
| [`TwoBodyFeed::feed`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/two_body.rs::TwoBodyFeed::feed) | the Legendre moments of the transfer from E into each sink group for elastic and discrete-level scattering | `groupr.f90` `getdis` | two-body kinematics with the MF=4 angular law |
| [`Cm6Emission::to_lab`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/kinematics/cm.rs::Cm6Emission::to_lab) | an MF=6 centre-of-mass emission transformed into lab-frame energy-angle moments | `groupr.f90` `cm2lab` | frame-change kinematics |
| [`bach`](../../code-review/#crates/njoy-outram-park-fork/src/groupr/kinematics/bach.rs::bach) | the Kalbach-Mann slope a(E, E′) from Kalbach-86 systematics | `groupr.f90` `bach` | a physics systematics formula |
| [`gtff_incoherent`](../../code-review/#crates/njoy-outram-park-fork/src/gaminr/gtff.rs::gtff_incoherent) | the photon incoherent (Compton) transfer into each group: Klein-Nishina with the incoherent scattering function | `gaminr.f90` `gtff` (`mtd = 504`) | the photon scattering kernel (GAMINR) |

<a id="errorr"></a>

## Rung 8 — ERRORR and COVR

The physics, here statistics: the evaluation's covariances, given in its own
energy bins (MF=33) or on resonance parameters (MF=32), are carried onto a
user's group structure. MF=33 uses each `LB` law; MF=32 uses the sandwich
rule with sensitivities of the group σ to every resonance parameter.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`contribution`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/covcal.rs::contribution) | one MF=33 record's covariance contribution to a pair of union groups, for each `LB` law (0–6, 8) | `errorr.f90` `covcal` (lines 2150-2417) | the covariance laws themselves |
| [`covcal`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/covcal.rs::covcal) | absolute covariances on the union grid, relative records scaled by the group σ | `errorr.f90` `covcal` | **(borderline)** mostly a loop over sections; kept because it does the relative-to-absolute step |
| [`grpav`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/grpav.rs::grpav) | flux-weighted union-group σ for every reaction (infinite dilution) | `errorr.f90` `grpav` | the group average the covariances are weighted with |
| [`covout`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/covout.rs::covout) | the union-group covariances collapsed, flux-weighted, to the user's groups, with the MF=32 part added | `errorr.f90` `covout` | the collapse of a covariance matrix |
| [`rpxlc12`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/resprx/resolved.rs::rpxlc12) | sensitivities ∂σ_g/∂p by central differences of the resonance σ, folded with the MF=32 parameter covariance (`LCOMP = 1, 2`) | `errorr.f90` `rpxlc12` | **(borderline)** the sandwich rule is here, but the routine also reads its MF=32 records |
| [`ggmlbw`](../../code-review/#crates/njoy-outram-park-fork/src/errorr/resprx/mlbw.rs::ggmlbw) | the MLBW σ (total, elastic, fission, capture) that the sensitivities are differences of | `errorr.f90` `ggmlbw` | a resonance formula (ERRORR's own copy, distinct from RECONR's) |
| [`CovarianceMatrix::to_correlation`](../../code-review/#crates/njoy-outram-park-fork/src/covr/correlation.rs::CovarianceMatrix::to_correlation) | corr(i, j) = cov(i, j) / √(cov(i, i) cov(j, j)) | `covr.f90` `corr` (lines 672-688) | the statistics COVR reports |

<a id="acer"></a>

## Rung 9 — ACER

Most of ACER is **layout**: it arranges what earlier rungs computed into the
ACE blocks a transport code reads, and that is Part B's subject. The physics
it does compute itself is in turning distributions into the forms ACE can
sample: tabulated angular laws, equally probable bins, and a few spectra and
systematics that ACER evaluates.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`acesix_equiprobable`](../../code-review/#crates/njoy-outram-park-fork/src/acer/acesix.rs::acesix_equiprobable) | THERMR's σ(E→E′) emission divided into equally probable outgoing-energy bins, each bin's area-weighted mean E′ | `aceth.f90` `acesix` | **(borderline)** statistics of the emission law; it decides what the transport code samples, so it is kept as physics |
| [`ptleg2`](../../code-review/#crates/njoy-outram-park-fork/src/acer/acensd.rs::ptleg2) | a Legendre angular distribution turned into a tabulated, normalised f(μ) | `acecm.f90` `ptleg2` | **(borderline)** a change of representation, but it builds a probability distribution that must stay positive and normalised |
| [`fmn`](../../code-review/#crates/njoy-outram-park-fork/src/acer/energy/madland_nix.rs::fmn) | the Madland-Nix prompt fission neutron spectrum | `acefc.f90` `fmn` | a physics formula |
| [`law3_discrete_level`](../../code-review/#crates/njoy-outram-park-fork/src/acer/energy/core.rs::law3_discrete_level) | ACE law 3: outgoing energy of a discrete-level reaction from its Q and A | `acefc.f90` `acelod` (inline law-3 branch) | two-body kinematics |
| [`bachaa`](../../code-review/#crates/njoy-outram-park-fork/src/acer/acelf6.rs::bachaa) | the Kalbach-86 slope a, in MeV (ACER's own copy) | `acecm.f90` `bachaa` | a physics systematics formula |
| [`iheat`](../../code-review/#crates/njoy-outram-park-fork/src/acer/photoatomic/incoherent_heating.rs::iheat) | the photon incoherent heating: mean energy left behind by Compton scattering with the scattering function | `acepa.f90` `iheat` | the photon heating integral |

The UNR block (`unr_words`) and the serialisers are layout; their physics was
computed in [rung 4](#purr).

<a id="output-formats"></a>

## Other codes' formats

No physics function. DTFR, RESXSR, MATXSR, WIMSR, CCCCR and POWR reorganise
group constants and pointwise data for other codes. The arithmetic MIXR
does (`mix_reaction`, a number-density-weighted sum σ = Σ wᵢσᵢ) is a
one-line mixture rule rather than physics worth a lesson.

<a id="openmc-interchange"></a>

## OpenMC interchange

No physics function. This page is about writing and reading OpenMC's HDF5 and
XML layouts: format conversion throughout.

<a id="wmp"></a>

## Windowed multipole

The physics: σ(E, T) evaluated on the fly as a sum over poles, each
Doppler-broadened analytically through the Faddeeva function, plus a
broadened polynomial background. This is a port of OpenMC's evaluator, not of
NJOY, so the upstream column names OpenMC.

| function | the physics it computes | upstream | why physics, not glue |
|---|---|---|---|
| [`WindowedMultipole::evaluate`](../../code-review/#crates/njoy-outram-park-fork/src/wmp/evaluate.rs::WindowedMultipole::evaluate) | σ_s, σ_a, σ_f at (E, T): the pole sum in the energy's window plus the curve fit | OpenMC `WindowedMultipole::evaluate` | the multipole representation itself |
| [`faddeeva`](../../code-review/#crates/njoy-outram-park-fork/src/wmp/evaluate.rs::faddeeva) | the broadened line shape of one pole, in the integral form the multipole sum uses | OpenMC `faddeeva` | Doppler broadening of a pole |
| [`w_standard`](../../code-review/#crates/njoy-outram-park-fork/src/wmp/evaluate.rs::w_standard) | the Faddeeva function w(z) by Weideman's rational approximation | Weideman (1994), as OpenMC uses it | **(borderline)** a special-function algorithm; kept because its accuracy is the broadening's accuracy |
| [`broaden_wmp_polynomials`](../../code-review/#crates/njoy-outram-park-fork/src/wmp/evaluate.rs::broaden_wmp_polynomials) | the Doppler-broadened polynomial background | OpenMC `broaden_wmp_polynomials` (Josey et al. 2016, eq. 16) | Doppler broadening of the background |

The GPU path (`gpu_wmp`) evaluates the same formulas in `f32` for speed; it is
acceleration, not new physics.

<a id="consumer-surface"></a>

## The consumer surface

No physics function. This page is about the API a caller uses
(`NuclearDataLibrary` and its builders, the CLI tools); every physics function
it reaches is listed under the rung that owns it.
