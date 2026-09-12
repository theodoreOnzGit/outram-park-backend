//! NJOY2016 oracle values, committed as golden data.
//!
//! # Provenance
//!
//! Every number here was measured by running **NJOY2016 release 2016.79**
//! (`18Mar25`) on the same open ENDF/B-VIII.0 evaluations this crate reads from
//! `reference-data/endf/`. The dates are recorded per table. The NJOY output
//! tapes are 24–37 MB and are **not** checked in; the handful of numbers taken
//! off them are, which is what lets the comparisons run on a machine with no
//! NJOY installed and no multi-megabyte tape on disk.
//!
//! The decks that produce each tape are reproduced in the doc comments of the
//! examples that read them directly:
//! `examples/graphite_vs_njoy_thermr.rs`, `examples/h2o_vs_njoy_thermr.rs`,
//! `examples/h2o_kernel_vs_njoy_thermr.rs`,
//! `examples/graphite_kernel_vs_njoy_thermr.rs` and
//! `examples/u238_vs_njoy_pendf.rs`.
//!
//! # Why this lives in the library rather than in a test file
//!
//! It was in a test file, and the examples that generated it carried their own
//! copies of the same numbers in prose. Two copies of an oracle drift, and the
//! prose copy drifts first because nothing checks it. Both the golden tests and
//! the examples now read *these* tables, so there is exactly one recorded value
//! per measurement and a single place to update when the oracle is re-run.
//!
//! # These are oracle values, not targets
//!
//! Agreement here is a claim about this crate's fidelity to the code it ports.
//! Where the port is known to be wrong, the discrepancy is recorded in the table
//! comments rather than smoothed away — see [`H2O_KERNEL`], whose −2 % to −5.5 %
//! column is the open defect tracked as GitHub #188.

/// One point of a cross-section comparison: `(E [eV], sigma [b])`.
pub type XsPoint = (f64, f64);

/// One point of a first-moment kernel comparison: `(E [eV], <E'>/E)`.
pub type KernelPoint = (f64, f64);

// ─────────────────────────────────────────────────────────────────────────────
// H in H₂O
// ─────────────────────────────────────────────────────────────────────────────

/// **H-in-H₂O incoherent-inelastic cross section**, THERMR MT=222 for
/// `tsl-HinH2O` at 293.6 K. Measured 2026-09-11.
///
/// This crate sits a consistent **+0.65 % to +1.47 %** above NJOY across the
/// whole tabulated range — a small, one-signed magnitude offset. The comment on
/// each row is this crate's own value on the date above.
///
/// Note what this table does *not* see: the cross section is the *area* of the
/// scattering law, and [`H2O_KERNEL`] shows the *shape* is wrong by up to
/// 5.5 % in the opposite direction. A magnitude oracle alone would have cleared
/// a defective law.
pub const H2O_XS: &[XsPoint] = &[
    (1.000000e-03, 1.168588e2), // ours 1.184556e2, +1.37 %
    (5.000000e-03, 8.185424e1), // ours 8.253468e1, +0.83 %
    (1.000000e-02, 6.932242e1), // ours 6.992186e1, +0.86 %
    (2.530000e-02, 5.168752e1), // ours 5.214197e1, +0.88 %
    (5.000000e-02, 3.951121e1), // ours 3.990964e1, +1.01 %
    (1.000000e-01, 3.255061e1), // ours 3.289730e1, +1.07 %
    (2.000000e-01, 2.745337e1), // ours 2.776834e1, +1.15 %
    (4.000000e-01, 2.363236e1), // ours 2.391182e1, +1.18 %
    (6.250000e-01, 2.215801e1), // ours 2.230188e1, +0.65 %
    (1.000000e+00, 2.150120e1), // ours 2.181613e1, +1.46 %
    (2.000000e+00, 2.094862e1), // ours 2.125675e1, +1.47 %
];

/// The envelope [`H2O_XS`] is asserted inside: **2 %**, against a worst measured
/// deviation of +1.47 %.
pub const H2O_XS_TOL: f64 = 0.02;

/// NJOY's H-in-H₂O law runs to **10 eV**; this crate's ends between 2 and 4 eV.
///
/// That is why [`H2O_XS`] stops at 2 eV. The handover to free gas is a real
/// difference between the two codes and is asserted separately, rather than
/// being hidden by truncating the comparison silently.
pub const H2O_LAW_UPPER_BOUND_EV: (f64, f64) = (2.0, 4.0);

/// **H-in-H₂O scattering kernel**, THERMR MF=6/MT=222 for `tsl-HinH2O` at
/// 293.6 K, reduced to its first moment: `(E [eV], <E'>/E, xi = <ln(E/E')>)`.
/// Measured 2026-09-11.
///
/// # This table records an open defect, deliberately
///
/// This crate's kernel is **too narrow**: it moves neutrons −5.5 % at 1.5 meV
/// through to +1.5 % at 1.9 eV, crossing over at 0.1116 eV, with `xi` 3–5 % low.
/// Water therefore moderates about 4 % less per collision here than in NJOY.
/// That is [GitHub #188], and the numbers below are the measurement it rests on.
///
/// **Do not "fix" a failure of the kernel assertions by widening them.** The
/// envelope is pinned at 6 % precisely because the defect is 5.5 % — it is
/// sized to catch the defect getting *worse*, and it must be *tightened* when
/// the port is repaired, not loosened.
///
/// # How a defect this size survived a full analytic test file
///
/// `tests/thermal_h2o_sab.rs` checks the free-atom limit, detailed balance, the
/// cross section, and the effective temperature. A kernel that is too narrow
/// satisfies detailed balance *exactly* (it is a symmetry, not a width),
/// reproduces the free-atom limit (that tests the absence of binding), has
/// nearly the right area (see [`H2O_XS`]) and the right effective temperature
/// (a scalar). Only a direct comparison of the outgoing-energy distribution
/// sees it.
///
/// [GitHub #188]: https://github.com/theodoreOnzGit/outram-park-backend/issues/188
pub const H2O_KERNEL: &[(f64, f64, f64)] = &[
    (1.500000e-03, 7.304950, -0.893430), // ours 6.899970 (−5.54 %)
    (5.000000e-03, 2.510210, -0.426060), // ours 2.414360 (−3.82 %)
    (1.000000e-02, 1.666430, -0.235610), // ours 1.618340 (−2.89 %)
    (2.530000e-02, 1.193860, -0.055270), // ours 1.176320 (−1.47 %)
    (5.000000e-02, 1.010440, 0.086990),  // ours 1.002250 (−0.81 %)
    (1.115700e-01, 0.800170, 0.366900),  // ours 0.800170 (+0.00 %)
    (2.000000e-01, 0.713830, 0.489700),  // ours 0.718120 (+0.60 %)
    (4.170400e-01, 0.642280, 0.620940),  // ours 0.646770 (+0.70 %)
    (6.250000e-01, 0.605110, 0.710540),  // ours 0.612900 (+1.29 %)
    (1.050000e+00, 0.565930, 0.801740),  // ours 0.572550 (+1.17 %)
    (1.855000e+00, 0.539260, 0.869670),  // ours 0.547230 (+1.48 %)
];

/// The envelope [`H2O_KERNEL`] is asserted inside: **6 %**, sized to the −5.54 %
/// defect it documents. Tighten this when #188 is fixed; never widen it.
pub const H2O_KERNEL_TOL: f64 = 0.06;

/// Incident energy at which this crate's H-in-H₂O kernel crosses NJOY's, in eV.
/// Below it this crate's `<E'>/E` is low, above it high — the sign structure
/// that identifies a *width* error rather than a scale error.
pub const H2O_KERNEL_CROSSOVER_EV: f64 = 0.11157;

// ─────────────────────────────────────────────────────────────────────────────
// Graphite
// ─────────────────────────────────────────────────────────────────────────────

/// **Graphite incoherent-inelastic cross section**, THERMR MT=229 for
/// `tsl-crystalline-graphite` at 600 K. Measured 2026-09-11.
///
/// 600 K is a *tabulated* temperature on that tape, so no temperature
/// interpolation is involved and any difference is in the law itself.
///
/// Worst deviation **−0.14 %**, at the top of the range. This is what a ported
/// thermal law looks like when it is right, and it is the control against which
/// [`H2O_KERNEL`]'s −5.5 % is read.
pub const GRAPHITE_XS_INELASTIC: &[XsPoint] = &[
    (1.000000e-04, 3.341434e0),  // ours 3.342667e0, +0.04 %
    (1.000000e-03, 1.137236e0),  // ours 1.135755e0, −0.13 %
    (5.000000e-03, 7.150516e-1), // ours 7.149814e-1, −0.01 %
    (2.530000e-02, 1.110868e0),  // ours 1.110623e0, −0.02 %
    (5.000000e-02, 1.710240e0),  // ours 1.709684e0, −0.03 %
    (1.000000e-01, 2.588940e0),  // ours 2.588316e0, −0.02 %
    (2.000000e-01, 3.485030e0),  // ours 3.483340e0, −0.05 %
    (5.000000e-01, 4.231276e0),  // ours 4.230133e0, −0.03 %
    (1.000000e+00, 4.485358e0),  // ours 4.483433e0, −0.04 %
    (2.000000e+00, 4.612172e0),  // ours 4.608760e0, −0.07 %
    (3.000000e+00, 4.654246e0),  // ours 4.649273e0, −0.11 %
    (3.900000e+00, 4.673901e0),  // ours 4.667192e0, −0.14 %
];

/// **Graphite coherent-elastic cross section**, THERMR MT=230 for
/// `tsl-crystalline-graphite` at 600 K. Measured 2026-09-11.
///
/// Worst deviation **−0.09 %**. The two points below the first Bragg edge are
/// exactly zero on both sides, which is a *structural* agreement rather than a
/// numerical one: a code that has the Bragg cutoff in the wrong place, or that
/// smears it, cannot produce an exact zero there.
pub const GRAPHITE_XS_COHERENT: &[XsPoint] = &[
    (1.000000e-04, 0.0),         // below the first Bragg edge — exactly zero
    (1.000000e-03, 0.0),         // below the first Bragg edge — exactly zero
    (5.000000e-03, 4.616808e0),  // ours 4.616808e0, −0.00 %
    (2.530000e-02, 4.041842e0),  // ours 4.041842e0, −0.00 %
    (5.000000e-02, 3.252226e0),  // ours 3.252226e0, +0.00 %
    (1.000000e-01, 2.243431e0),  // ours 2.243431e0, −0.00 %
    (2.000000e-01, 1.275185e0),  // ours 1.275184e0, −0.00 %
    (5.000000e-01, 5.188752e-1), // ours 5.188752e-1, +0.00 %
    (1.000000e+00, 2.594442e-1), // ours 2.594442e-1, −0.00 %
    (2.000000e+00, 1.297221e-1), // ours 1.297221e-1, −0.00 %
    (3.000000e+00, 8.655500e-2), // ours 8.648140e-2, −0.09 %
    (3.900000e+00, 6.653488e-2), // ours 6.652415e-2, −0.02 %
];

/// The envelope both graphite cross-section tables are asserted inside:
/// **0.5 %**, against a worst measured deviation of −0.14 %.
pub const GRAPHITE_XS_TOL: f64 = 0.005;

/// Highest energy at which graphite's coherent-elastic cross section is still
/// exactly zero, in eV — i.e. below the first Bragg edge.
pub const GRAPHITE_FIRST_BRAGG_EDGE_ABOVE_EV: f64 = 1.0e-3;

/// **Graphite scattering kernel**, THERMR MF=6/MT=229 for
/// `tsl-crystalline-graphite` at 600 K: `(E [eV], <E'>/E)`, coherent elastic
/// excluded from the moment. Measured 2026-09-11.
///
/// # This table corrects a claim this workspace had been repeating
///
/// "Graphite's kernel matches THERMR to ≤0.5 %" was used to argue that water's
/// −5.5 % was uniquely bad. That figure was **scoped to 0.1–4 eV**. Over the
/// range where water is compared, graphite is **−1.53 %** at 0.0253 eV against
/// water's −1.47 % — the same, not better.
///
/// What actually distinguishes the two is **convergence**: graphite's deviation
/// falls to ≤0.1 % above 0.2 eV and stays there, while water's grows
/// monotonically to +1.5 %. The discriminator is the trend, not the worst
/// value, which is why [`GRAPHITE_KERNEL_CONVERGED_ABOVE_EV`] exists and is
/// asserted separately.
///
/// The third column of each comment is the coherent-elastic share of the total
/// cross section at that energy, which is why the low-energy points carry the
/// larger deviations: little inelastic signal is left to measure.
pub const GRAPHITE_KERNEL: &[KernelPoint] = &[
    (1.000000e-02, 4.907990), // ours 4.897740 (−0.21 %), 83 % coherent elastic
    (2.530000e-02, 1.903930), // ours 1.874890 (−1.53 %), 78 %
    (5.000000e-02, 1.264140), // ours 1.246550 (−1.39 %), 66 %
    (1.115700e-01, 1.011580), // ours 1.004850 (−0.67 %), 43 %
    (2.000000e-01, 0.936480), // ours 0.936760 (+0.03 %), 27 %
    (4.170400e-01, 0.890410), // ours 0.890280 (−0.01 %), 13 %
    (6.250000e-01, 0.878570), // ours 0.879470 (+0.10 %), 9 %
    (1.050000e+00, 0.869530), // ours 0.870310 (+0.09 %), 5 %
    (1.855000e+00, 0.863980), // ours 0.864200 (+0.03 %), 3 %
    (3.750000e+00, 0.860400), // ours 0.860980 (+0.07 %), 2 %
];

/// The envelope [`GRAPHITE_KERNEL`] is asserted inside: **2 %**, against a worst
/// measured deviation of −1.53 %.
pub const GRAPHITE_KERNEL_TOL: f64 = 0.02;

/// Above this energy graphite's kernel agrees with NJOY to
/// [`GRAPHITE_KERNEL_CONVERGED_TOL`] and stays there. This convergence — not
/// the worst-point figure — is what distinguishes a correct thermal law from
/// [`H2O_KERNEL`]'s, whose deviation instead grows with energy.
pub const GRAPHITE_KERNEL_CONVERGED_ABOVE_EV: f64 = 0.2;

/// The tight envelope graphite's kernel holds above
/// [`GRAPHITE_KERNEL_CONVERGED_ABOVE_EV`]: **0.4 %**, against a worst measured
/// deviation of +0.10 % there.
pub const GRAPHITE_KERNEL_CONVERGED_TOL: f64 = 0.004;

/// **Graphite scattering ANGLE**, 600 K:
/// `(E [eV], mubar_inelastic, mubar_coherent_elastic, mubar_total)`.
/// Measured 2026-09-12.
///
/// # Why an angular table exists at all
///
/// Every other thermal oracle in this workspace is an **energy-domain** oracle
/// — [`GRAPHITE_XS_INELASTIC`] (how often), [`GRAPHITE_KERNEL`] (how much energy
/// per collision), `graphite_energy_decrement` (ξ), `slowing_down_oracle` (the
/// whole energy treatment). None of them constrains μ. Before this table the
/// only assertion on a thermal cosine anywhere in the crate was
/// `(-1.0..=1.0).contains(&mu)`.
///
/// The angle sets σ_tr = σ_s(1 − μ̄), hence the diffusion coefficient, hence
/// the thermal flux shape in a heterogeneous cell — the axis the FHR ring-RPT
/// residual hunt (`op-mzvp.2.12`) had narrowed to.
///
/// # The two oracles, which are independent of each other
///
/// - **Inelastic** — THERMR MF=6/MT=229 carries, per incident energy, `NEP`
///   groups of `NA + 2 = 18` numbers `(E′, f(E′), μ₁…μ₁₆)` whose 16 cosines are
///   **equally probable** in the laboratory frame.
///   `examples/graphite_kernel_vs_njoy_thermr.rs` parses the same records and
///   discards those cosines; `examples/graphite_mubar_vs_njoy_thermr.rs` keeps
///   them and forms `μ̄ = Σ f(E′)·mean(μ|E′) / Σ f(E′)` by the same trapezoid
///   used for the energy moment. So the two differ only in which column they
///   reduce.
/// - **Coherent elastic** — needs no MF=6 at all. `E·σ_coh(E)` from
///   MF=3/MT=230 is a staircase whose 296 risers *are* the Bragg edges
///   `(E_i, f_i)`; each edge scatters at exactly `μ_i = 1 − 2E_i/E`, so
///   `μ̄_el(E) = Σ f_i(1 − 2E_i/E)/Σ f_i` is recovered from NJOY's own cross
///   section without ever reading this crate's edge table.
///
/// The `total` column is the two weighted by THERMR's own MT=229 and MT=230
/// cross sections at that energy, which is the quantity transport actually uses.
///
/// # Result: the thermal angle is NOT the ring-RPT residual
///
/// Worst deviation **+0.0085 absolute on μ̄_inelastic** (0.0253 eV, 5σ of the
/// 400 000-sample statistics, so real but tiny) and **+0.0050 on μ̄_total**
/// (3.75 eV). μ̄_total is ≈ 0.05 across the whole range, so a 0.005 error moves
/// σ_tr = σ_s(1 − μ̄) by **0.05 %**. Against a +4004 pcm k-residual that is an
/// exclusion, not a candidate.
///
/// The Bragg column is the tighter of the two — worst **+0.0030** at 0.2 eV,
/// and it reproduces the sign reversal (μ̄_el runs −0.40 at 2.6 meV, just above
/// the first edge where only backscattering is open, to +0.97 at 3.75 eV where
/// every edge is open and forward-scattering dominates) without being given the
/// edge table.
///
/// Comments carry this crate's own value on the date above.
pub const GRAPHITE_MUBAR: &[(f64, f64, f64, f64)] = &[
    // E [eV]      mubar_inel  mubar_el   mubar_tot
    (1.012000e-03, -0.10126, 0.00000, -0.10126), // ours −0.10071 / n/a     / −0.10071
    (2.600000e-03, -0.15658, -0.40179, -0.36809), // ours −0.15545 / −0.40179 / −0.36793
    (5.000000e-03, -0.20404, -0.25734, -0.25019), // ours −0.20552 / −0.25810 / −0.25104
    (1.000000e-02, -0.25087, 0.02533, -0.02079), // ours −0.25540 / +0.02530 / −0.02176
    (2.530000e-02, -0.27620, 0.00568, -0.05509), // ours −0.28467 / +0.00656 / −0.05628
    (5.000000e-02, -0.25874, 0.12636, -0.00636), // ours −0.26037 / +0.12786 / −0.00578
    (1.035000e-01, -0.20580, 0.28889, 0.02065),  // ours −0.20676 / +0.28831 / +0.01974
    (2.000000e-01, -0.13444, 0.53293, 0.04434),  // ours −0.13590 / +0.53588 / +0.04415
    (3.900000e-01, -0.06204, 0.74362, 0.05075),  // ours −0.06044 / +0.74410 / +0.05201
    (6.250000e-01, -0.02259, 0.83965, 0.05279),  // ours −0.02374 / +0.83971 / +0.05172
    (1.050000e+00, 0.00741, 0.90455, 0.05415),   // ours +0.00874 / +0.90454 / +0.05513
    (2.020000e+00, 0.03014, 0.95039, 0.05508),   // ours +0.03444 / +0.95048 / +0.05959
    (3.750000e+00, 0.04196, 0.97327, 0.05555),   // ours +0.04653 / +0.97357 / +0.06054
];

/// The envelope [`GRAPHITE_MUBAR`]'s inelastic column is asserted inside:
/// **0.02 absolute on μ̄**, against a worst measured deviation of +0.0085.
///
/// The bound is **absolute, not relative**, on purpose: μ̄_inelastic passes
/// through zero near 0.9 eV, so a relative bound there is arithmetic noise —
/// the same trap [`GRAPHITE_KERNEL`]'s doc records for ξ.
pub const GRAPHITE_MUBAR_TOL: f64 = 0.02;

/// The tighter envelope the **coherent-elastic** column holds: **0.01 absolute**,
/// against a worst measured deviation of +0.0030. It is tighter because the
/// Bragg law is discrete and deterministic — there is no kernel integration in
/// it, only the edge table and `μ = 1 − 2E_i/E`.
pub const GRAPHITE_MUBAR_ELASTIC_TOL: f64 = 0.01;

/// **Graphite scattering kernel WIDTH**, 600 K: `(E [eV], sqrt(var(E'))/<E'>)`
/// of the incoherent-inelastic outgoing-energy distribution, from THERMR
/// MF=6/MT=229 by quadrature. Measured 2026-09-12.
///
/// # What this sees that [`GRAPHITE_KERNEL`] cannot
///
/// [`GRAPHITE_KERNEL`] is the **first** moment `<E'>/E`. A kernel can have
/// exactly the right mean and the wrong spread, and the spread is what decides
/// how many neutrons cross the 0.625 eV group boundary per collision — i.e. the
/// joining region between the 1/E slowing-down spectrum and the Maxwellian.
/// Nothing in this crate had ever compared it.
///
/// # Results, and the defect this table records
///
/// Against NJOY, this crate's sampled kernel is
///
/// ```text
///    E [eV]     w NJOY    w ours   rel        w ours    rel
///                                             (4x grid)
///    0.00101    0.76168   0.73571  -3.41 %    0.73501   -3.50 %
///    0.0026     0.79883   0.77356  -3.16 %    0.77334   -3.19 %
///    0.005      0.84932   0.82747  -2.57 %    0.82683   -2.65 %
///    0.01       0.90255   0.87859  -2.65 %    0.87975   -2.53 %
///    0.0253     0.79241   0.71967  -9.18 %    0.72008   -9.13 %
///    0.05       0.54768   0.48432 -11.57 %    0.48322  -11.77 %
///    0.1035     0.35619   0.33874  -4.90 %    0.32113   -9.84 %
///    0.2        0.27905   0.26517  -4.98 %    0.25740   -7.76 %
///    0.39       0.22681   0.25083 +10.59 %    0.21578   -4.86 %
///    0.625      0.19328   0.21794 +12.76 %    0.18610   -3.72 %
///    1.05       0.16345   0.18220 +11.47 %    0.15795   -3.37 %
///    2.02       0.13644   0.18971 +39.04 %    0.13329   -2.31 %
///    3.75       0.11994   0.16209 +35.15 %    0.11740   -2.11 %
/// ```
///
/// **The sign flips at 0.39 eV and the excess reaches +39 % at 2 eV.** The
/// fourth column is the same measurement with `N_EMIT_GRID` raised from 48 to
/// 192, and it identifies the cause exactly: the emission tables sit on a
/// 48-point log grid over 1e-5 … 4 eV, i.e. **adjacent incident energies differ
/// by 31.6 %**, and `select_table` picks between the two bracketing tables by
/// ACE statistical interpolation. Mixing two tables whose means are 31.6 % apart
/// adds a variance `r(1-r)(m2-m1)^2` that the true kernel does not have. Where
/// the intrinsic spread is small — above ~0.4 eV, where `w` has fallen to 0.12 —
/// that added variance dominates. Where the intrinsic spread is large (below
/// 0.1 eV, `w` ~ 0.8) it is invisible, and what is left is the **−3 % to −12 %**
/// narrowing of the 16-bin equiprobable representation itself.
///
/// # The k-worth of this defect was MEASURED, not assumed: −63 pcm
///
/// Same deck, same seed, `examples/fhr_ring_rpt_endf.rs` with
/// `OUTRAM_RINGRPT_ONLY=csg`, 2026-09-12:
///
/// ```text
///   N_EMIT_GRID = 48   k = 1.40745 +/- 0.00214   p(2-group) = 0.5254
///   N_EMIT_GRID = 192  k = 1.40682 +/- 0.00221   p(2-group) = 0.5249
/// ```
///
/// **−63 pcm**, inside its own sampling error, against a +4004 pcm residual.
/// So this is a real defect of the thermal kernel and it is **not** the cause of
/// the FHR ring-RPT disagreement. The grid was left at 48 deliberately: raising
/// it shifts every recorded thermal k-eff by tens of pcm, which is a separate
/// change with its own re-baselining, not something to fold into a measurement.
///
/// Tracked as [GitHub #190] and bead `op-x77y`. The angular oracle from the same
/// session — [`GRAPHITE_MUBAR`], which excluded the angle — is bead `op-i7u9`.
///
/// Comments carry this crate's own value at `N_EMIT_GRID = 48`.
///
/// [GitHub #190]: https://github.com/theodoreOnzGit/outram-park-backend/issues/190
pub const GRAPHITE_KERNEL_WIDTH: &[(f64, f64)] = &[
    (1.012000e-03, 0.76168), // ours 0.73571 (−3.41 %)
    (2.600000e-03, 0.79883), // ours 0.77356 (−3.16 %)
    (5.000000e-03, 0.84932), // ours 0.82747 (−2.57 %)
    (1.000000e-02, 0.90255), // ours 0.87859 (−2.65 %)
    (2.530000e-02, 0.79241), // ours 0.71967 (−9.18 %)
    (5.000000e-02, 0.54768), // ours 0.48432 (−11.57 %)
    (1.035000e-01, 0.35619), // ours 0.33874 (−4.90 %)
    (2.000000e-01, 0.27905), // ours 0.26517 (−4.98 %)
    (3.900000e-01, 0.22681), // ours 0.25083 (+10.59 %)
    (6.250000e-01, 0.19328), // ours 0.21794 (+12.76 %)
    (1.050000e+00, 0.16345), // ours 0.18220 (+11.47 %)
    (2.020000e+00, 0.13644), // ours 0.18971 (+39.04 %)
    (3.750000e+00, 0.11994), // ours 0.16209 (+35.15 %)
];

/// The characterisation envelope [`GRAPHITE_KERNEL_WIDTH`] is asserted inside:
/// **50 %**, against a worst measured deviation of +39.0 %.
///
/// This is a **characterisation** bound on a known open defect — the same
/// contract [`H2O_KERNEL`] carries. It is sized to fail if the defect grows, not
/// to bless it. **Tighten it to ~15 % when the emission grid is refined; never
/// widen it.**
pub const GRAPHITE_KERNEL_WIDTH_TOL: f64 = 0.50;

/// Below this energy the table-interpolation excess is swamped by the kernel's
/// own spread, and what remains is the equiprobable representation's own
/// narrowing — which holds inside [`GRAPHITE_KERNEL_WIDTH_NARROW_TOL`].
/// Asserted separately because the two halves of the table fail for different
/// reasons and a single envelope would hide that.
pub const GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV: f64 = 0.2;

/// The envelope the width holds below
/// [`GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV`]: **15 %**, against a worst
/// measured deviation of −11.6 %, and it is **one-signed** (always narrow).
pub const GRAPHITE_KERNEL_WIDTH_NARROW_TOL: f64 = 0.15;

/// Linear-linear interpolation on an ascending `(E, sigma)` table, returning
/// zero outside it — the same contract as NJOY's `gety1`, so a comparison
/// against a PENDF or THERMR tape is made on the oracle's own terms.
///
/// ```
/// # use outram_mc_libs::vv::njoy_golden::interp_linlin;
/// let table = [(1.0, 10.0), (2.0, 20.0)];
/// assert_eq!(interp_linlin(&table, 1.5), 15.0);
/// assert_eq!(interp_linlin(&table, 9.0), 0.0); // outside: zero, not clamped
/// ```
#[must_use]
pub fn interp_linlin(pairs: &[(f64, f64)], e: f64) -> f64 {
    if pairs.is_empty() || e < pairs[0].0 || e > pairs[pairs.len() - 1].0 {
        return 0.0;
    }
    let i = match pairs.binary_search_by(|p| p.0.partial_cmp(&e).expect("finite grid")) {
        Ok(i) => return pairs[i].1,
        Err(i) => i,
    };
    let (e0, s0) = pairs[i - 1];
    let (e1, s1) = pairs[i];
    if e1 == e0 {
        return s1;
    }
    s0 + (s1 - s0) * (e - e0) / (e1 - e0)
}

// ─────────────────────────────────────────────────────────────────────────────
// U-238 point cross sections
// ─────────────────────────────────────────────────────────────────────────────

/// **NJOY2016 2016.79 PENDF for U-238**, MAT 9237 at 600 K: `(E [eV], sigma [b])`
/// at nineteen probe energies — thermal, the four big low-lying capture
/// resonances, the resolved tail, the resolved/unresolved boundary, the
/// unresolved band, and fast. Measured 2026-09-11.
///
/// # What these pin, and what they cannot
///
/// These are **point** values, so they pin the *peak heights*. They structurally
/// cannot see the **area** under a resonance, which is what actually drives
/// resonance escape: a grid too coarse between the nodes loses area without
/// moving any node value. That gap is closed by
/// `examples/u238_resonance_integral.rs`, which integrates `sigma_gamma dE/E`
/// on this crate's own grid — the grid transport actually interpolates on — and
/// gets +0.001 % against NJOY over 0.5 eV to 100 keV.
///
/// # Results
///
/// MT=1 and MT=2 agree to **0.04 %** worst, MT=102 to **0.17 %** worst (at
/// 19 keV, where capture is 0.119 b between resonances). Where a row carries no
/// comment, this crate and NJOY agree to better than the printed precision.
///
/// MT=18 is a special case — see [`u238_pendf::FISSION`].
pub mod u238_pendf {
    /// MT=1, total.
    pub const TOTAL: &[(f64, f64)] = &[
        (2.530000e-02, 1.194280e1), // ours 1.194284e1, +0.00 %
        (1.000000e+00, 9.571597e0), // ours 9.571636e0, +0.00 %
        (6.674000e+00, 5.749893e3), // ours 5.749417e3, −0.01 %
        (2.087000e+01, 7.283905e3), // ours 7.286665e3, +0.04 %
        (3.668000e+01, 1.005539e4), // ours 1.005545e4, +0.00 %
        (6.603000e+01, 3.207754e3), // ours 3.207856e3, +0.00 %
        (1.026000e+02, 4.503320e3), // ours 4.503401e3, +0.00 %
        (1.000000e+03, 2.148533e1), // ours 2.148538e1, +0.00 %
        (1.000000e+04, 1.320347e1), // ours 1.320402e1, +0.00 %
        (1.900000e+04, 1.035556e1), // ours 1.035799e1, +0.02 %
        (2.000000e+04, 1.229868e1), // ours 1.229866e1, −0.00 %
        (3.000000e+04, 1.383150e1), // ours 1.383150e1, +0.00 %
        (5.000000e+04, 1.303331e1), // ours 1.303331e1, +0.00 %
        (1.000000e+05, 1.180514e1), // ours 1.180514e1, +0.00 %
        (1.500000e+05, 1.131349e1), // ours 1.131349e1, +0.00 %
        (5.000000e+05, 8.435778e0), // ours 8.435778e0, +0.00 %
        (1.000000e+06, 7.089129e0), // ours 7.089129e0, +0.00 %
        (2.000000e+06, 7.281519e0), // ours 7.281518e0, −0.00 %
        (1.400000e+07, 5.867194e0), // ours 5.867194e0, +0.00 %
    ];
    /// MT=2, elastic.
    pub const ELASTIC: &[(f64, f64)] = &[
        (2.530000e-02, 9.260055e0),
        (1.000000e+00, 9.075288e0),
        (6.674000e+00, 3.628279e2), // ours 3.627348e2, −0.03 % (the worst)
        (2.087000e+01, 2.240051e3),
        (3.668000e+01, 6.048971e3),
        (6.603000e+01, 1.672177e3),
        (1.026000e+02, 3.401129e3),
        (1.000000e+03, 2.129172e1),
        (1.000000e+04, 1.181773e1),
        (1.900000e+04, 1.023603e1),
        (2.000000e+04, 1.189319e1),
        (3.000000e+04, 1.339700e1),
        (5.000000e+04, 1.263400e1),
        (1.000000e+05, 1.110700e1),
        (1.500000e+05, 1.035010e1),
        (5.000000e+05, 6.603770e0),
        (1.000000e+06, 4.255680e0),
        (2.000000e+06, 3.546014e0),
        (1.400000e+07, 2.857930e0),
    ];
    /// MT=102, radiative capture — the resonance-absorption channel the whole
    /// study turns on.
    pub const CAPTURE: &[(f64, f64)] = &[
        (2.530000e-02, 2.682721e0),
        (1.000000e+00, 4.963043e-1),
        (6.674000e+00, 5.387062e3), // the 6.7 eV resonance peak
        (2.087000e+01, 5.043842e3),
        (3.668000e+01, 4.006410e3),
        (6.603000e+01, 1.535573e3),
        (1.026000e+02, 1.102190e3),
        (1.000000e+03, 1.936138e-1),
        (1.000000e+04, 1.385712e0),
        (1.900000e+04, 1.194841e-1), // ours 1.192862e-1, −0.17 % (the worst)
        (2.000000e+04, 4.054252e-1),
        (3.000000e+04, 4.344800e-1),
        (5.000000e+04, 3.234450e-1),
        (1.000000e+05, 1.787900e-1),
        (1.500000e+05, 1.411400e-1),
        (5.000000e+05, 1.111100e-1),
        (1.000000e+06, 1.278100e-1),
        (2.000000e+06, 4.787900e-2),
        (1.400000e+07, 8.244478e-4),
    ];
    /// MT=18, fission.
    ///
    /// **Most of this column is sub-threshold and carries no information.**
    /// U-238 fission has a ~1 MeV threshold, so below it the tabulated values
    /// are the evaluation's small sub-threshold tail — 1.354e-7 b at 1 keV, for
    /// instance. A *relative* comparison against a number that size measures
    /// reconstruction round-off, not physics: this crate gives 1.453e-7 b there,
    /// +7.31 %, which is 1e-8 b in absolute terms and means nothing.
    ///
    /// Any gate on this table must therefore carry a **significance floor** on
    /// the oracle magnitude and must assert *how many* points that floor
    /// excluded — otherwise a data change that silently zeroed the whole column
    /// would slip through as "everything passed". At a 1e-6 b floor, exactly one
    /// point (1 keV) is excluded.
    pub const FISSION: &[(f64, f64)] = &[
        (2.530000e-02, 1.851002e-5),
        (1.000000e+00, 3.214900e-6),
        (6.674000e+00, 2.344786e-3),
        (2.087000e+01, 1.210449e-2),
        (3.668000e+01, 1.749551e-3),
        (6.603000e+01, 3.555648e-3),
        (1.026000e+02, 6.129576e-4),
        (1.000000e+03, 1.354363e-7), // ours 1.453388e-7, +7.31 % — see the test
        (1.000000e+04, 2.335126e-5),
        (1.900000e+04, 4.735709e-5),
        (2.000000e+04, 7.454015e-5),
        (3.000000e+04, 2.498100e-5),
        (5.000000e+04, 1.150433e-4),
        (1.000000e+05, 5.393800e-5),
        (1.500000e+05, 1.153124e-4),
        (5.000000e+05, 2.781100e-4),
        (1.000000e+06, 1.459200e-2),
        (2.000000e+06, 5.378600e-1),
        (1.400000e+07, 1.150600e0),
    ];
}
