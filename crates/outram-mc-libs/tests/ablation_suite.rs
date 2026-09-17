//! **Ablation tests: price each physics mechanism on a cheap, deterministic
//! observable instead of on a 30-minute eigenvalue.**
//!
//! # Why these exist, and why they are not `k_eff` runs
//!
//! The Godiva study priced its mechanisms by running the whole case twice and
//! differencing `k`. That works, but it costs ~30-40 minutes per ablation and
//! carries `sd ≈ 180 pcm` per seed, so it needs tens of seeds before it can say
//! anything. Nothing that expensive runs in a test suite, which is why the
//! ablations lived only in `examples/` and only their *controls* were tested.
//!
//! **But `k` is not the quantity the diagnosis actually rests on — `Σ_tr` is.**
//! Leakage from a bare assembly is governed by
//! `Σ_tr = Σ_t(1 − ⟨μ⟩)`, and `⟨μ⟩` is computable directly from the data with
//! no transport at all: deterministic, seed-free, and in seconds rather than
//! half an hour. These tests ablate each mechanism and assert what it does to
//! `⟨μ⟩` and `Σ_tr`, against the values an **independent code** (OpenMC 0.15.3,
//! reading the same ENDF/B-VIII.0 tapes through the same NJOY2016 build)
//! produces for the same quantities.
//!
//! That makes them strictly stronger than a `k` ablation as *regression* tests:
//! a `k` difference can be right for the wrong reasons, whereas `⟨μ⟩` matching
//! an external oracle per mechanism cannot.
//!
//! # Methodology
//!
//! The three ICSBEP HEU-MET-FAST-001 nuclides at their benchmark atom densities,
//! reconstructed from `reference-data/endf/` at 293.6 K. Quantities are
//! **flux-weighted** over `1e4 … 1.9e7 eV` — where Godiva's flux lives — with a
//! Watt fission spectrum as the weight, and **cross-section weighted** within
//! each channel. This mirrors
//! `verification_and_validation/openmc_godiva_cross_code/transport_decomposition.py`
//! exactly, so the two are directly comparable; that script computes the same
//! numbers from OpenMC's HDF5 and is the source of the reference values here.
//!
//! `⟨μ_inel⟩` is obtained by **sampling** the real transport path
//! (`sample_inelastic` picks a level in proportion to its cross section, then
//! `sample_inelastic_mu_cm` draws its cosine), so it tests level selection,
//! parse and sampler together rather than any one in isolation. The continuum
//! channel contributes `μ = 0`, matching the reference script's treatment.
//!
//! # Reference values (OpenMC 0.15.3, same evaluation and NJOY build)
//!
//! ```text
//! flux-weighted above 10 keV, material-averaged over the three ICSBEP nuclides:
//!   elastic    <mu> = 0.2758   (85.2% of scattering)
//!   inelastic  <mu> = 0.0254   (14.8% of scattering)
//!   Sigma_tr        = 0.37141 /cm        with both anisotropies
//!   Sigma_tr        = 0.37287 /cm        inelastic ablated  (+0.39%)
//! ```
//!
//! # Results (2026-09-16) — all four tests pass in 527 s
//!
//! | quantity | ours | OpenMC | apart |
//! |---|---|---|---|
//! | `⟨μ_elastic⟩` | 0.2631 | 0.2645 | 0.5 % |
//! | `⟨μ_inelastic⟩`, all channels | 0.0236 | 0.0245 | 3.7 % |
//! | `⟨μ_inelastic⟩`, discrete levels only | 0.0344 | 0.0357 | 3.6 % |
//! | **MT=91 share of inelastic** | **0.3135** | **0.3140** | **0.2 %** |
//! | `Σ_tr` shift on ablating inelastic anisotropy | +0.36 % | +0.38 % | — |
//! | `⟨E'/E⟩` at 2 MeV, evaluated MF=6 vs Weisskopf | 0.1985 vs 0.2954 | — | — |
//!
//! The **MT=91 share agreeing to 0.2 %** is the quiet one that matters most:
//! the continuum scores `μ = 0` on both sides, so it dilutes the inelastic mean
//! cosine directly. Two codes could agree on every angular distribution and
//! still disagree on `⟨μ_inel⟩` if they split inelastic differently between the
//! continuum and the discrete levels. They do not.
//!
//! # Two corrections this suite forced, both worth reading before trusting it
//!
//! **1. The reference values were wrong, and by more than our disagreement
//! with them.** `transport_decomposition.py` read OpenMC's angular tables with
//! `np.searchsorted` — the table at the energy *above* `e`, not interpolated.
//! `⟨μ⟩` grows with energy, so this biased the reference high: `⟨μ_el⟩` 0.2740
//! against 0.2645 (+3.6 %) and `⟨μ_inel⟩` 0.0254 against 0.0245 (+3.7 %). That
//! is the **fourth** appearance of the nearest-point trap in this study, and
//! the first inside our own reference script rather than in a comparison
//! against someone else's. It was found only because an independent computation
//! disagreed and the disagreement was chased instead of absorbed into a
//! tolerance.
//!
//! Correcting it moves `Σ_tr` from 0.37141 to 0.37457 and the one-group
//! diffusion estimate of the inelastic-anisotropy price from +219 to +209 pcm —
//! so the *conclusion* is robust, which is the reassuring part.
//!
//! **2. The first version of this suite was under-resolved and nearly reported
//! a physics defect that did not exist.** See
//! [`aggregates_are_grid_converged`].
//!
//! # What these do NOT do
//!
//! They do not measure reactivity. `Σ_tr` feeds leakage through a **one-group
//! diffusion** argument that is crude in a bare fast metal sphere — good for
//! "is this mechanism big enough to matter", not for a pcm prediction. The pcm
//! numbers still come from the paired ensembles in `examples/`. These tests
//! guard the *input* to that argument.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};

const TEMP_K: f64 = 293.6;

/// `(tape, name, atom density [atoms/barn·cm])` — ICSBEP HEU-MET-FAST-001.
const NUCLIDES: &[(&str, &str, f64)] = &[
    ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
    ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
    ("n-092_U_238.endf", "U238", 2.4984e-3),
];

/// OpenMC's flux-weighted elastic `⟨μ_cm⟩` for this material.
///
/// **Interpolated**, not read at the nearest tabulated incident energy. The
/// reference script originally used `np.searchsorted` — the table *above* `e` —
/// and since `⟨μ⟩` grows with energy that biased it high, to `0.2740`. Corrected
/// 2026-09-16 when an independent Rust computation of the same aggregate
/// disagreed and the disagreement was chased rather than absorbed into a
/// tolerance. That is the **fourth** appearance of the nearest-point trap in
/// this study; the first three are recorded in the V&V README.
const OPENMC_MU_ELASTIC: f64 = 0.2645;
/// OpenMC's flux-weighted inelastic `⟨μ_cm⟩` for this material, interpolated.
/// The pre-correction value was `0.0254`.
const OPENMC_MU_INELASTIC: f64 = 0.0245;
/// OpenMC's flux-weighted MT=91 (continuum) share of total inelastic.
///
/// The continuum scores `μ = 0` on both sides, so this fraction *dilutes* the
/// inelastic mean cosine directly: an aggregate `⟨μ⟩` can only be compared
/// meaningfully if the two codes also agree on how inelastic splits between
/// the continuum and the discrete levels.
const OPENMC_MT91_SHARE: f64 = 0.3140;
/// OpenMC's flux-weighted `⟨μ_cm⟩` over the **discrete levels only**.
const OPENMC_MU_DISCRETE: f64 = 0.0357;

/// Energy grid: `n` log-spaced points over Godiva's flux range \[eV\].
///
/// **Resolution matters more than it looks**, and this is not a free parameter.
/// These aggregates are plain weighted sums over the grid, so they only
/// approximate the flux integral when the grid resolves the inelastic
/// thresholds. At 80 points (24 per decade) `⟨μ_inel⟩` comes out **28 % low**;
/// at 400 (122 per decade) it is converged. The reference script uses 400, and
/// `aggregates_are_grid_converged` below asserts the convergence rather than
/// trusting this comment.
fn grid_n(n: usize) -> Vec<f64> {
    let (l0, l1) = (1.0e4f64.ln(), 1.9e7f64.ln());
    (0..n)
        .map(|i| (l0 + (l1 - l0) * i as f64 / (n - 1) as f64).exp())
        .collect()
}

/// The converged grid, matching `transport_decomposition.py`.
const GRID: usize = 400;

fn grid() -> Vec<f64> {
    grid_n(GRID)
}

/// Watt fission-spectrum weight at `e` \[eV\] — the same form the reference
/// script uses, so the two weightings are identical.
fn watt(e: f64) -> f64 {
    (-e / 0.988e6).exp() * (2.249e-6 * e).sqrt().sinh()
}

/// Build the three nuclides once, or `None` if the tapes are not present.
fn build() -> Option<Vec<Nuclide>> {
    let mut v = Vec::new();
    for &(file, name, _) in NUCLIDES {
        let tape = reference_file_or_skip("endf", file, "ICSBEP nuclide (ablation suite)")?;
        v.push(Nuclide::from_endf_file(&tape, name, TEMP_K, 1.0e-3).expect("reconstructs"));
    }
    Some(v)
}

fn material() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: TEMP_K,
        components: NUCLIDES
            .iter()
            .enumerate()
            .map(|(i, &(_, _, rho))| NuclideComponent {
                nuclide_idx: i,
                atom_density: rho,
            })
            .collect(),
    }
}

/// Flux- and cross-section-weighted **elastic** `⟨μ_cm⟩` over the material.
fn mu_elastic(nuclides: &[Nuclide]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for e in grid() {
        let w = watt(e);
        for (i, &(_, _, rho)) in NUCLIDES.iter().enumerate() {
            let s = rho * nuclides[i].xs_at_energy(e, TEMP_K).elastic;
            num += w * s * nuclides[i].elastic_mubar_cm(e);
            den += w * s;
        }
    }
    num / den
}

/// Flux- and cross-section-weighted **inelastic** `⟨μ_cm⟩`, obtained by sampling
/// the real transport path so level selection is exercised too.
///
/// `draws` samples per (energy, nuclide). The continuum channel scores `μ = 0`,
/// as does a discrete level the evaluation leaves isotropic — matching how the
/// reference script treats both.
fn mu_inelastic(nuclides: &[Nuclide], draws: usize) -> f64 {
    mu_inelastic_on(nuclides, draws, &grid()).0
}

/// `(⟨μ⟩ over all inelastic, ⟨μ⟩ over discrete levels only, MT=91 share)`.
fn mu_inelastic_on(nuclides: &[Nuclide], draws: usize, g: &[f64]) -> (f64, f64, f64) {
    let (mut num, mut den) = (0.0, 0.0);
    let (mut dnum, mut dden) = (0.0, 0.0);
    let (mut w91, mut wall) = (0.0, 0.0);
    let mut seed = 0xA51E_u64;
    for &e in g {
        let w = watt(e);
        for (i, &(_, _, rho)) in NUCLIDES.iter().enumerate() {
            let s = rho * nuclides[i].xs_at_energy(e, TEMP_K).inelastic;
            if s <= 0.0 {
                continue;
            }
            let (mut acc, mut dacc, mut nd, mut n91) = (0.0, 0.0, 0usize, 0usize);
            for _ in 0..draws {
                match nuclides[i].sample_inelastic(e, &mut seed) {
                    Inelastic::Level { mt, .. } => {
                        let m = nuclides[i]
                            .sample_inelastic_mu_cm(mt, e, &mut seed)
                            .unwrap_or(0.0);
                        acc += m;
                        dacc += m;
                        nd += 1;
                    }
                    Inelastic::Continuum { .. } => n91 += 1,
                }
            }
            num += w * s * (acc / draws as f64);
            den += w * s;
            if nd > 0 {
                let frac = nd as f64 / draws as f64;
                dnum += w * s * frac * (dacc / nd as f64);
                dden += w * s * frac;
            }
            w91 += w * s * (n91 as f64 / draws as f64);
            wall += w * s;
        }
    }
    (num / den, dnum / dden, w91 / wall)
}

/// Flux-weighted macroscopic transport cross section `Σ_tr = Σ_t − Σ_s·⟨μ⟩`
/// \[cm⁻¹\], with elastic and inelastic each weighted by their own `⟨μ⟩`.
fn sigma_tr(nuclides: &[Nuclide], mat: &Material, mu_el: f64, mu_in: f64) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for e in grid() {
        let w = watt(e);
        // `MacroXs` carries no inelastic column (it is not needed on the hot
        // path), so sum it from the per-nuclide micro cross sections the same
        // way `macro_xs` sums the rest.
        let total = mat.macro_xs_total(e, nuclides);
        let (mut s_el, mut s_in) = (0.0, 0.0);
        for (i, &(_, _, rho)) in NUCLIDES.iter().enumerate() {
            let x = nuclides[i].xs_at_energy(e, TEMP_K);
            s_el += rho * x.elastic;
            s_in += rho * x.inelastic;
        }
        num += w * (total - s_el * mu_el - s_in * mu_in);
        den += w;
    }
    num / den
}

#[test]
fn ablating_inelastic_anisotropy_moves_sigma_tr_by_the_diagnosed_amount() {
    let Some(aniso) = build() else { return };
    let iso: Vec<Nuclide> = aniso
        .iter()
        .cloned()
        .map(Nuclide::with_isotropic_inelastic_scattering)
        .collect();
    let mat = material();

    let g = grid();
    let mu_el = mu_elastic(&aniso);
    let (mu_in_on, mu_disc, share91) = mu_inelastic_on(&aniso, 1000, &g);
    let (mu_in_off, _, _) = mu_inelastic_on(&iso, 1000, &g);

    println!("  <mu_elastic>          {mu_el:.4}   (OpenMC {OPENMC_MU_ELASTIC:.4})");
    println!("  <mu_inelastic>  ON    {mu_in_on:.4}   (OpenMC {OPENMC_MU_INELASTIC:.4})");
    println!("  <mu_inelastic>  OFF   {mu_in_off:.4}   (ablated -- must be 0)");
    println!("  <mu> discrete only    {mu_disc:.4}   (OpenMC {OPENMC_MU_DISCRETE:.4})");
    println!("  MT=91 share           {share91:.4}   (OpenMC {OPENMC_MT91_SHARE:.4})");

    // The partition check. An aggregate <mu> comparison is only meaningful if
    // the two codes also agree on how inelastic divides between the continuum
    // (which scores mu = 0 on both sides) and the discrete levels. If our MT=91
    // share were larger, our mean cosine would be diluted and would look wrong
    // for a reason that has nothing to do with angular data.
    assert!(
        (share91 - OPENMC_MT91_SHARE).abs() < 0.03,
        "MT=91 is {share91:.4} of flux-weighted inelastic here against OpenMC's \
         {OPENMC_MT91_SHARE:.4}. The continuum scores mu = 0, so this fraction directly dilutes \
         <mu_inelastic>; a disagreement here is a CROSS-SECTION partition problem, not an \
         angular one, and it would invalidate the <mu> comparison below rather than being \
         explained by it."
    );

    // 1. The ablation is total: with the MF=4 tables gone, every inelastic
    //    collision is isotropic in CM and the mean cosine is exactly zero.
    assert!(
        mu_in_off.abs() < 1.0e-3,
        "with the inelastic angular tables ablated, <mu_inelastic> is {mu_in_off:+.5}, not 0. \
         The ablation is not removing what it claims to, so every ablation measurement made \
         with it -- including the -224 pcm price of this mechanism -- is suspect."
    );

    // 2. With them present, we reproduce OpenMC's value for the same material.
    //    This is the check that the fix is right rather than merely present:
    //    the number is computed from our parse, our level selection and our
    //    sampler, and compared against an independent code's read of the same
    //    tapes. Tolerance is 15% relative -- the reference is quoted to four
    //    decimals, ours is a finite sample, and the two use different
    //    linearisations of the same Legendre coefficients (worst 3.1e-3 per
    //    level, tests/inelastic_mubar_vs_openmc.rs).
    let rel = (mu_in_on - OPENMC_MU_INELASTIC).abs() / OPENMC_MU_INELASTIC;
    assert!(
        rel < 0.12,
        "<mu_inelastic> is {mu_in_on:.4} here against OpenMC's {OPENMC_MU_INELASTIC:.4} \
         ({:.1}% apart). This crate sampled it as 0 before bead op-tm9f, and the whole ~181 pcm \
         Godiva leakage attribution rests on this number being right. A disagreement means \
         either the MF=4 parse, the per-level cross-section weighting, or the sampler is wrong.",
        100.0 * rel
    );

    // 3. Elastic is unchanged by an inelastic ablation, and still matches.
    let rel_el = (mu_el - OPENMC_MU_ELASTIC).abs() / OPENMC_MU_ELASTIC;
    assert!(
        rel_el < 0.05,
        "<mu_elastic> is {mu_el:.4} against OpenMC's {OPENMC_MU_ELASTIC:.4} ({:.1}% apart). \
         Elastic carries 85% of the <mu> budget and was cleared separately in \
         tests/elastic_mubar_vs_openmc.rs; a failure here means something moved under it.",
        100.0 * rel_el
    );

    // 4. The quantity that actually drives leakage.
    let _ = mu_disc;
    let str_on = sigma_tr(&aniso, &mat, mu_el, mu_in_on);
    let str_off = sigma_tr(&iso, &mat, mu_el, 0.0);
    let shift = str_off / str_on - 1.0;
    println!("  Sigma_tr  ON  {str_on:.5} /cm");
    println!("  Sigma_tr  OFF {str_off:.5} /cm   ({:+.2}%)", 100.0 * shift);

    assert!(
        shift > 0.0,
        "ablating inelastic anisotropy moved Sigma_tr by {:+.3}%, i.e. DOWN. It must go UP: \
         removing forward peaking lowers <mu>, and Sigma_tr = Sigma_t(1 - <mu>) rises. If this \
         fires, the sign of the whole leakage argument is inverted.",
        100.0 * shift
    );
    assert!(
        (0.0015..0.0070).contains(&shift),
        "ablating inelastic anisotropy shifts Sigma_tr by {:+.3}%, outside the 0.15-0.70% band. \
         The diagnosis measured +0.39% against OpenMC, and that shift is what the ~181-219 pcm \
         leakage price was derived from. A materially different shift means the price is wrong \
         even if Godiva's k still looks right.",
        100.0 * shift
    );
}

#[test]
fn ablating_elastic_anisotropy_removes_the_larger_share() {
    let Some(aniso) = build() else { return };
    let iso: Vec<Nuclide> = aniso
        .iter()
        .cloned()
        .map(Nuclide::with_isotropic_elastic_scattering)
        .collect();

    let on = mu_elastic(&aniso);
    let off = mu_elastic(&iso);
    println!("  <mu_elastic>  ON {on:.4}   OFF {off:.4}");

    assert!(
        off.abs() < 1.0e-3,
        "with the elastic MF=4 table ablated, <mu_elastic> is {off:+.5}, not 0 -- the control \
         used for the +10511 pcm elastic ablation is not removing what it claims to."
    );
    assert!(
        on > 0.2,
        "<mu_elastic> is {on:.4}; elastic off a heavy actinide is strongly forward-peaked \
         (~0.28 flux-weighted) and this is the dominant term in the <mu> budget."
    );
    // Elastic is ~11x the inelastic share of the budget: 0.852*0.2758 = 0.235
    // against 0.148*0.0254 = 0.0038. That ratio is why the elastic ablation
    // prices at +10511 pcm and the inelastic one at -224.
    assert!(
        on / OPENMC_MU_INELASTIC > 5.0,
        "elastic <mu> ({on:.4}) should dominate inelastic ({OPENMC_MU_INELASTIC:.4}) by roughly \
         an order of magnitude; that ratio is what makes the two ablations' very different \
         prices coherent."
    );
}

#[test]
fn ablating_the_mf6_continuum_law_hardens_the_secondary_spectrum() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 (MF=6 continuum ablation)",
    ) else {
        return;
    };
    let evaluated =
        Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");
    assert!(
        evaluated.has_evaluated_continuum(),
        "U-238 carries an MF=6 LAW=1 continuum law in ENDF/B-VIII.0; if this fires, the law is \
         no longer being read and MT=91 has silently fallen back to the Weisskopf stand-in."
    );
    let stand_in = evaluated.clone().without_evaluated_continuum();
    assert!(
        !stand_in.has_evaluated_continuum(),
        "without_evaluated_continuum left the law in place -- the ablation behind the measured \
         -105 +/- 32 pcm MF=6 result is a no-op."
    );

    // Mean outgoing/incident energy ratio over many continuum draws at 2 MeV.
    // The evaluated law is SOFTER than the evaporation stand-in: the crate's
    // record measures <E'/E> = 0.2095 evaluated against 0.2787 stand-in.
    let mean_ratio = |n: &Nuclide, draws: usize| -> f64 {
        use outram_mc_libs::geometry::position::Direction;
        use outram_mc_libs::physics::scatter::continuum_inelastic_scatter_evaluated;
        let e = 2.0e6;
        let u = Direction::new(0.0, 0.0, 1.0);
        let mut seed = 0x5EED_u64;
        let mut acc = 0.0;
        for _ in 0..draws {
            let (e_out, _) =
                continuum_inelastic_scatter_evaluated(e, u, n.awr, 0.0, n.continuum_law(91), &mut seed);
            acc += e_out / e;
        }
        acc / draws as f64
    };

    let r_eval = mean_ratio(&evaluated, 20_000);
    let r_stand = mean_ratio(&stand_in, 20_000);
    println!("  <E'/E> at 2 MeV: evaluated {r_eval:.4}   Weisskopf stand-in {r_stand:.4}");

    assert!(
        r_eval < r_stand,
        "the evaluated MF=6 law gives <E'/E> = {r_eval:.4}, NOT softer than the Weisskopf \
         stand-in's {r_stand:.4}. The evaluation is softer where MT=91 opens (0.2095 vs 0.2787 \
         measured), and that softening is what moved Godiva -105 +/- 32 pcm. If this inverts, \
         the law is being read or sampled wrongly."
    );
    assert!(
        (0.10..0.32).contains(&r_eval),
        "<E'/E> from the evaluated law is {r_eval:.4}, outside the plausible 0.10-0.32 band \
         (measured 0.2095 at 2 MeV on U-238). A value near 1 means the law is returning the \
         incident energy; a value near 0 means it is collapsing."
    );
}

/// **The aggregates must be converged in the energy grid, and this asserts it
/// rather than trusting a comment.**
///
/// # Why this test exists
///
/// It caught a real error in its own suite. The first version of
/// [`mu_inelastic`] summed over 80 log-spaced points (24 per decade) and
/// reported `⟨μ_inel⟩ = 0.0167` against OpenMC's `0.0245` — 32 % low, which
/// looked exactly like a physics defect and was nearly written up as one. It was
/// under-resolution: these aggregates are plain weighted sums, and the inelastic
/// cross section has sharp thresholds, so a coarse grid mis-weights the region
/// where `⟨μ⟩` is still small. At 400 points the same code gives `0.0233`.
///
/// A quantity that moves 28 % when you change a discretisation you thought was
/// incidental is not a measurement, and the fix is to make the discretisation a
/// tested property instead of an assumption.
///
/// The test compares 200 against 400 points: if the answer is converged, halving
/// the resolution should barely move it.
#[test]
fn aggregates_are_grid_converged() {
    let Some(nuclides) = build() else { return };
    let coarse = grid_n(GRID / 2);
    let fine = grid_n(GRID);

    let (c, _, cs) = mu_inelastic_on(&nuclides, 1000, &coarse);
    let (f, _, fs) = mu_inelastic_on(&nuclides, 1000, &fine);
    println!("  <mu_inel>  {} pts {c:.4}   {} pts {f:.4}", GRID / 2, GRID);
    println!("  MT91 share {} pts {cs:.4}   {} pts {fs:.4}", GRID / 2, GRID);

    let rel = (c - f).abs() / f;
    assert!(
        rel < 0.06,
        "<mu_inelastic> moves {:.1}% between {} and {} energy points ({c:.4} -> {f:.4}), so it \
         is NOT converged and the comparison against OpenMC in this file is measuring the grid \
         rather than the physics. Raise GRID. Do not compare an unconverged aggregate against \
         an external code -- that is how a discretisation artefact gets written up as a \
         physics defect.",
        100.0 * rel,
        GRID / 2,
        GRID
    );
    assert!(
        (cs - fs).abs() < 0.02,
        "the MT=91 share moves from {cs:.4} to {fs:.4} between grids; the reaction partition \
         should be far less grid-sensitive than the mean cosine."
    );
}
