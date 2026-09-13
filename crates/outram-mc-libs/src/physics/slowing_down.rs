//! Epithermal slowing down in an **infinite homogeneous medium**, solved two
//! independent ways — deterministically and by Monte Carlo — so the two can be
//! made to check each other.
//!
//! # Why this module exists
//!
//! The FHR ring-RPT study (`verification_and_validation/ring_rpt/`, bead
//! `op-mzvp.2.12`, GitHub #178) ran out of eigenvalues. It established, against
//! four measured criticality benchmarks and NJOY2016's own PENDF, that
//!
//! - `σ_γ(E)` for U-238 is correct — ±0.04 % pointwise, 0.12 % worst in
//!   resonance *shape* over six resonances, **+0.00 %** in infinitely-dilute
//!   resonance integral;
//! - the tracking method is not implicated (delta-tracked and surface-tracked
//!   models are both high, and agree with each other to 18 pcm);
//! - and yet the **self-shielded** absorption is ~11 % low, reproduced on a
//!   measured critical experiment (ICSBEP LEU-COMP-THERM-008, +2950 ± 61 pcm).
//!
//! So the defect sits between a correct cross section and the absorption rate it
//! produces once the medium is optically thick — and no `k` can say where,
//! because `k` is one number. What is needed is a **spectrum**, judged against
//! something that is exact rather than merely different.
//!
//! An infinite homogeneous medium is the one geometry where that exists. The
//! slowing-down equation has no spatial variable, so it can be integrated
//! directly on a fine energy grid to whatever accuracy the grid allows, using
//! **this crate's own reconstructed cross sections**. Running the Monte Carlo
//! collision kernel on the same medium then compares two solutions of the same
//! equation with the same data, and any difference is the transport, not the
//! data and not the geometry.
//!
//! That splits the remaining search cleanly:
//!
//! - **They agree** ⇒ the collision physics and the cross-section lookup are
//!   right in a homogeneous medium, and the defect is **spatial** — how a lump's
//!   interior flux is built. That is already the pattern the benchmark table
//!   shows: HEU-SOL-THERM-009 is the one *homogeneous* thermal case and it is
//!   the one that comes out right.
//! - **They disagree** ⇒ the defect is in the energy treatment, and because the
//!   deterministic solution gives the flux bin by bin, the disagreement **names
//!   the energies**.
//!
//! # The equation
//!
//! Write lethargy `u = ln(E_top/E)` and let `F(u) = Σ_t(E) φ(E) E` be the
//! collision density per unit lethargy. For elastic scattering that is isotropic
//! in the centre of mass off a target **at rest**, a collision at `u′` lands
//! uniformly in `E′ ∈ [α_i E, E]`, and the balance is
//!
//! ```text
//! F(u) = Σ_i ∫_{u−ε_i}^{u} (Σ_s,i/Σ_t)(u′) · F(u′) · e^{−(u−u′)}/(1−α_i) du′ + S(u)
//!
//!     α_i = ((A_i−1)/(A_i+1))²        ε_i = ln(1/α_i)
//! ```
//!
//! and the absorption rate per unit lethargy in nuclide `i` is
//! `F(u)·(Σ_a,i/Σ_t)(u)`.
//!
//! **The kernel is separable**, which is what makes this cheap: since
//! `e^{−(u−u′)} = e^{−u}·e^{u′}`, the inner integral is
//! `e^{−u}·[G_i(u) − G_i(u−ε_i)]` with `G_i` a running cumulative integral. The
//! solve is therefore **O(N)** in the number of grid points, not O(N × window),
//! and a grid fine enough to resolve the resolved resonances costs seconds
//! rather than hours.
//!
//! # What is deliberately *not* modelled, and why the comparison is still fair
//!
//! The deterministic side assumes isotropic-CM elastic scattering off a target
//! at rest, and no inelastic or (n,2n) channels. Rather than assume the Monte
//! Carlo side matches, [`InfiniteMediumMc`] offers the same medium under three
//! successively more complete kernels ([`ScatterKernel`]), so each ingredient is
//! *measured* instead of waved away:
//!
//! | kernel | what it adds | what the difference measures |
//! |---|---|---|
//! | [`ScatterKernel::IsotropicCmAtRest`] | — | this is the deterministic model exactly; **the oracle comparison** |
//! | [`ScatterKernel::AnisotropicCmAtRest`] | the nuclide's own ENDF MF=4 law | anisotropy of elastic scattering |
//! | [`ScatterKernel::Production`] | S(α,β) and free-gas target motion | exactly what `transport_csg::transport_history` does |
//!
//! Keeping the band above the inelastic thresholds' reach (U-238's first level
//! is at 44.9 keV, C-12's at 4.44 MeV) removes the remaining channels, and
//! [`InfiniteMediumMc`] asserts it never took one.
//!
//! # What it found (2026-09-11)
//!
//! U-238 + C-12 at 293.6 K, a 10 keV source scored down to 1 eV, with the
//! background cross section `σ_b = N_C σ_p,C / N_8` scanned over three decades:
//!
//! ```text
//! sigma_b [b]   deterministic p_esc      Monte Carlo          diff      z
//!        30           0.02940        0.02880 ± 0.00037      −2.05 %   −1.61
//!       100           0.17579        0.17730 ± 0.00085      +0.86 %   +1.77
//!       300           0.38763        0.38904 ± 0.00109      +0.36 %   +1.29
//!      1000           0.60543        0.60582 ± 0.00109      +0.06 %   +0.35
//!     10000           0.88390        0.88402 ± 0.00072      +0.01 %   +0.17
//! ```
//!
//! **They agree at every dilution**, worst 1.8σ, with the deterministic side
//! converged to 0.014 % under a bisected lethargy grid. Read as effective
//! resonance integrals through `p = exp(−I_eff/(ξ σ_b))`, the same solve gives
//! 194.9 b, 79.2 b, 44.9 b, 27.5 b and 16.7 b over that scan — an **11.7×
//! collapse** — so this is agreement in the strongly self-shielded regime, not
//! in the dilute limit where the interesting physics is switched off.
//!
//! So the first branch is the one that happened: **the energy treatment is
//! sound, and the ring-RPT residual is spatial.** The regression form of this
//! measurement, with the assertions, is
//! `tests/ring_rpt_hunt_lessons.rs::monte_carlo_matches_the_deterministic_slowing_down_solution`.
//!
//! # The spatial half, 2026-09-12: [`solve_deterministic_multiregion`]
//!
//! "The residual is spatial" was where the previous paragraph left it, and the
//! spatial problem then had an oracle in the **transparent limit only**
//! (`examples/lump_self_shielding_scan.rs`: the thin-lump row reproduces the
//! exact homogeneous answer to −0.51 %, and past it there was a monotone trend
//! with nothing to check it against). The FHR ring-RPT fuel annulus is
//! 1.4934–1.7531 cm and **6.65 mean free paths thick at the 6.674 eV U-238
//! peak**, and it carries the entire heavy-metal inventory — worth +3327 pcm
//! over naive homogenisation on this code's own numbers.
//!
//! [`solve_deterministic_multiregion`] closes that gap. It is [`solve_on_grid`]
//! with one thing added: the spatial coupling `C_i(u) = Σ_j s_j(u) P_{j→i}(u)`,
//! where `P_{j→i}` are the **exact** first-flight collision probabilities of a
//! concentric-sphere cell from
//! [`crate::physics::collision_probability::first_flight`], recomputed at every
//! lethargy point and closed with a white outer boundary. There is no Monte
//! Carlo in it.
//!
//! Set `P_{j→i} = δ_{ji}` and it is [`solve_on_grid`] line for line — and giving
//! every shell the **same** material does exactly that by way of the physics,
//! because `Σ_j V_j P_{j→i} = V_i` makes `C_i ∝ V_i` the solution. That is the
//! test that cannot be fudged, and it passes to **1.1e-12** at cell radii from
//! 0.01 cm to 100 cm and 3 to 24 shells
//! (`tests/lump_collision_probability.rs`).

use crate::geometry::cell::SurfaceToken;
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::scatter::{
    free_gas_elastic_scatter, two_body_scatter_with_mu, K_BOLTZMANN_EV_PER_K,
};
use crate::rng::lcg::prn;

/// One nuclide of an infinite homogeneous mixture: which nuclide, and how much.
#[derive(Debug, Clone, Copy)]
pub struct MixComponent {
    /// Index into the caller's nuclide array.
    pub nuclide_idx: usize,
    /// Atom density \[atoms·barn⁻¹·cm⁻¹\], the same unit
    /// [`crate::material::material::Material`] uses.
    pub atom_density: f64,
}

/// Where the answer is scored: a mono-energetic source at `e_top`, and neutrons
/// followed until they are absorbed or fall below `e_bot`.
///
/// `e_top` should sit **below the lowest inelastic threshold** of every nuclide
/// in the mixture (44.9 keV for U-238), so the only channels open are elastic
/// scattering and absorption — which is what the deterministic solution models.
#[derive(Debug, Clone, Copy)]
pub struct SlowingDownBand {
    /// Source energy \[eV\]; also the top of the scored band.
    pub e_top: f64,
    /// Bottom of the scored band \[eV\]. A neutron reaching it has escaped.
    pub e_bot: f64,
}

/// Where the source neutrons ended up, as fractions of one source neutron.
///
/// `absorbed_by[i]` is indexed like the mixture's component array, so the
/// per-nuclide split is available and not just the total. The three outcomes
/// sum to 1 by construction.
#[derive(Debug, Clone, PartialEq)]
pub struct SlowingDownResult {
    /// Fraction absorbed in each component, in the mixture's own order.
    pub absorbed_by: Vec<f64>,
    /// Fraction that reached `e_bot` still alive.
    pub escaped: f64,
    /// Collision density per unit lethargy, on the solve grid. Empty for the
    /// Monte Carlo, which scores outcomes rather than a flux.
    pub collision_density: Vec<f64>,
    /// Lethargy grid `u = ln(e_top/E)` the flux is given on. Empty for the
    /// Monte Carlo.
    pub lethargy: Vec<f64>,
}

impl SlowingDownResult {
    /// Total absorbed fraction — one minus the escape probability.
    pub fn absorbed(&self) -> f64 {
        self.absorbed_by.iter().sum()
    }

    /// Resonance escape probability over the band: the fraction of source
    /// neutrons that reach `e_bot` without being absorbed.
    pub fn resonance_escape(&self) -> f64 {
        self.escaped
    }
}

/// Which elastic-scattering kernel the Monte Carlo walker uses.
///
/// The point of the enum is that the deterministic solution models exactly one
/// of these, so the other two turn "the deterministic model omits anisotropy and
/// target motion" from an excuse into a measurement. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScatterKernel {
    /// Isotropic in the centre of mass, target at rest. **This is the
    /// deterministic model's own kernel**, so a disagreement here is a defect
    /// rather than a modelling difference.
    IsotropicCmAtRest,
    /// The nuclide's own ENDF MF=4 angular law, target still at rest. The
    /// difference from [`Self::IsotropicCmAtRest`] is the worth of anisotropy.
    AnisotropicCmAtRest,
    /// Exactly what `transport_csg::transport_history` does: the bound-atom
    /// S(α,β) law where the nuclide has one, otherwise free-gas with the
    /// target's own thermal motion sampled below `400·kT`.
    Production,
}

/// The slowing-down problem reduced to numbers: everything
/// [`solve_on_grid`] needs, and nothing about where it came from.
///
/// Keeping this separate from the nuclear data is what lets the solver be
/// checked against a **closed-form** answer rather than only against itself:
/// a medium with constant cross sections has an exact solution (see
/// `tests/ring_rpt_hunt_lessons.rs`), and constructing one here takes three
/// lines and no ENDF tape.
#[derive(Debug, Clone)]
pub struct SlowingDownGrid {
    /// Lethargy `u = ln(e_top/E)`, strictly increasing and starting at 0.
    pub lethargy: Vec<f64>,
    /// Per component, per grid point: the probability that a collision is a
    /// **scatter off this component**, `N_i σ_s,i / Σ_t`.
    pub scatter_frac: Vec<Vec<f64>>,
    /// Per component, per grid point: the probability that a collision is an
    /// **absorption in this component**, `N_i σ_a,i / Σ_t`.
    pub absorb_frac: Vec<Vec<f64>>,
    /// Per component: `α = ((A−1)/(A+1))²`, the maximum fractional energy loss.
    pub alpha: Vec<f64>,
}

/// Solve the infinite-medium slowing-down equation on a prepared grid.
///
/// See the module docs for the equation and for why the separable kernel makes
/// this O(N) in the grid size.
///
/// # The source
///
/// A mono-energetic source at the top of the band enters as a delta in the
/// collision density: its first collision happens at `u = 0` with unit weight,
/// so the absorbed fraction picks up `(Σ_a,i/Σ_t)(0)` directly and the scattered
/// part seeds the running integrals. [`InfiniteMediumMc`] starts its histories
/// the same way, so the two share the transient rather than having to argue
/// about it.
///
/// # Accuracy, and the one way to get it badly wrong
///
/// The only approximation is the grid, and the failure mode is specific: the
/// scatter-in window for component `i` is `ε_i = ln(1/α_i)` wide, which for
/// **U-238 is 0.0169 lethargy**. A grid interval wider than that cannot
/// represent the integral at all, and the solve does not diverge — it quietly
/// returns a far too *absorbing* answer. A resonance-adaptive energy grid is
/// exactly the kind that has such intervals, because it is dense at resonances
/// and sparse between them.
///
/// [`slowing_down_grid`] therefore caps the lethargy step at a fraction of the
/// narrowest window, and [`solve_on_grid`] asserts the cap held. Grid
/// convergence must still be demonstrated rather than assumed — solve twice, the
/// second time on [`refine_lethargy_grid`]'s output.
pub fn solve_on_grid(g: &SlowingDownGrid) -> SlowingDownResult {
    let n_mix = g.alpha.len();
    let n = g.lethargy.len();
    assert!(n_mix > 0, "an empty mixture has no slowing-down solution");
    assert!(n > 2, "grid has {n} points — nothing to solve");
    assert_eq!(g.scatter_frac.len(), n_mix);
    assert_eq!(g.absorb_frac.len(), n_mix);

    let u = &g.lethargy;
    let eps: Vec<f64> = g.alpha.iter().map(|a| -a.ln()).collect();
    let inv_1ma: Vec<f64> = g.alpha.iter().map(|a| 1.0 / (1.0 - a)).collect();
    let du_limit = eps.iter().cloned().fold(f64::INFINITY, f64::min);

    // `c` is the scatter branch divided by (1 − α) as the kernel wants it.
    let c: Vec<Vec<f64>> = (0..n_mix)
        .map(|i| g.scatter_frac[i].iter().map(|s| s * inv_1ma[i]).collect())
        .collect();
    let a = &g.absorb_frac;

    let mut gi = vec![vec![0.0_f64; n]; n_mix];
    let mut f = vec![0.0_f64; n];
    let mut absorbed_by = vec![0.0_f64; n_mix];
    for i in 0..n_mix {
        gi[i][0] = c[i][0]; // the delta source's own scatter, impulse × e^0
        absorbed_by[i] += a[i][0]; // and its own first-collision absorption
    }
    // The CONTINUOUS collision density just below the source is not zero: the
    // delta's first scatter lands in `(0, ε_i]` for every component, so
    // `F(0⁺) = Σ_i (Σ_s,i/Σ_t)(0)/(1−α_i)` — the impulse's scatter-in evaluated
    // at `u → 0⁺`, where `e^{−(u−u′)} → 1` and no component's window has closed
    // yet. Seeding `f[0] = 0` instead silently halves the first trapezoid and
    // shows up as an O(Δu) error on the closed-form hydrogen solution (the term
    // `c·Δu` comes out with coefficient ½ instead of 1).
    f[0] = (0..n_mix).map(|i| c[i][0]).sum();
    // Rolling lookback pointers for G_i(u − ε_i); the shifted point advances
    // monotonically with j, so this stays O(1) amortised.
    let mut back = vec![0_usize; n_mix];

    for j in 1..n {
        let du = u[j] - u[j - 1];
        assert!(
            du <= 0.25 * du_limit,
            "grid interval Δu = {du:.3e} at u = {} exceeds a quarter of the narrowest \
             scatter window ε = {du_limit:.3e}; the scatter-in integral cannot be \
             represented — build the grid with `slowing_down_grid`, which caps this",
            u[j]
        );
        let e_uj = u[j].exp();
        let e_ujm = u[j - 1].exp();

        let mut rhs = 0.0;
        let mut diag = 0.0;
        for i in 0..n_mix {
            // The part of G_i(u_j) already known: everything up to u_{j−1}, plus
            // the trapezoid's left half over the current interval.
            let known = gi[i][j - 1] + 0.5 * du * c[i][j - 1] * f[j - 1] * e_ujm;
            let shifted = lookback(&gi[i], u, u[j] - eps[i], &mut back[i]);
            rhs += (known - shifted) / e_uj;
            // The trapezoid's right half depends on F_j itself.
            diag += 0.5 * du * c[i][j];
        }
        assert!(
            diag < 1.0,
            "slowing-down solve is unstable at u = {} (within-cell scatter weight {diag})",
            u[j]
        );
        let fj = rhs / (1.0 - diag);
        f[j] = fj;

        for i in 0..n_mix {
            gi[i][j] =
                gi[i][j - 1] + 0.5 * du * (c[i][j - 1] * f[j - 1] * e_ujm + c[i][j] * fj * e_uj);
            absorbed_by[i] += 0.5 * du * (a[i][j - 1] * f[j - 1] + a[i][j] * fj);
        }
    }

    let total: f64 = absorbed_by.iter().sum();
    SlowingDownResult {
        absorbed_by,
        escaped: (1.0 - total).max(0.0),
        collision_density: f,
        lethargy: u.clone(),
    }
}

/// Build a [`SlowingDownGrid`] from real nuclear data.
///
/// `grid_ev` supplies the *energy* resolution — pass the union of the nuclides'
/// own [`Nuclide::native_energy_grid`]s, which resolves every resonance the
/// reconstruction resolved. This function then supplies the *lethargy*
/// resolution the solver needs, subdividing any interval wider than
/// `ε_min / steps_per_window` (see [`solve_on_grid`] for why that matters).
/// `steps_per_window = 20` is a sound default.
///
/// The branch probabilities are built exactly as the Monte Carlo's reaction
/// partition computes them: the nuclide is sampled ∝ `N_i σ_t,i` and the channel
/// within it ∝ `σ_x/σ_t,i`, so a scatter off `i` has per-collision probability
/// `N_i σ_s,i / Σ_t`, with `σ_s` the transport loop's own residual branch —
/// total minus absorption, inelastic and (n,2n).
pub fn slowing_down_grid(
    nuclides: &[Nuclide],
    mix: &[MixComponent],
    band: SlowingDownBand,
    temp_k: f64,
    grid_ev: &[f64],
    steps_per_window: usize,
) -> SlowingDownGrid {
    let n_mix = mix.len();
    assert!(n_mix > 0, "an empty mixture has no slowing-down solution");
    assert!(
        band.e_bot > 0.0 && band.e_top > band.e_bot,
        "band must be a positive energy interval, got {band:?}"
    );
    assert!(steps_per_window >= 2, "steps_per_window must be at least 2");

    let alpha: Vec<f64> = mix
        .iter()
        .map(|c| {
            let a = nuclides[c.nuclide_idx].awr;
            ((a - 1.0) / (a + 1.0)).powi(2)
        })
        .collect();
    // A = 1 gives α = 0 and an unbounded window; fall back to a sane cap.
    let du_max = alpha
        .iter()
        .filter(|a| **a > 0.0)
        .map(|a| -a.ln())
        .fold(f64::INFINITY, f64::min)
        / steps_per_window as f64;
    let du_max = if du_max.is_finite() { du_max } else { 0.01 };

    // Energies inside the band as increasing lethargy, with the step capped: a
    // resonance-adaptive energy grid is dense at resonances and sparse between
    // them, and it is the sparse stretches that silently break the solve.
    // `build_lethargy_grid` is shared with the multi-region solve below so the
    // homogeneous and lumped answers discretise identically.
    let (uu, ee) = build_lethargy_grid(band, grid_ev, du_max);

    let n = uu.len();
    let mut scatter_frac = vec![vec![0.0_f64; n]; n_mix];
    let mut absorb_frac = vec![vec![0.0_f64; n]; n_mix];
    for j in 0..n {
        let xs: Vec<_> = mix
            .iter()
            .map(|m| nuclides[m.nuclide_idx].xs_at_energy(ee[j], temp_k))
            .collect();
        let sigma_t: f64 = mix
            .iter()
            .zip(&xs)
            .map(|(m, x)| m.atom_density * x.total)
            .sum();
        assert!(
            sigma_t > 0.0,
            "total cross section vanishes at {} eV — the mixture is transparent",
            ee[j]
        );
        for i in 0..n_mix {
            let scat = (xs[i].total - xs[i].absorption - xs[i].inelastic - xs[i].n2n).max(0.0);
            scatter_frac[i][j] = mix[i].atom_density * scat / sigma_t;
            absorb_frac[i][j] = mix[i].atom_density * xs[i].absorption / sigma_t;
        }
    }

    SlowingDownGrid {
        lethargy: uu,
        scatter_frac,
        absorb_frac,
        alpha,
    }
}

/// [`slowing_down_grid`] + [`solve_on_grid`] in one call, with the default
/// 20 lethargy steps per scattering window.
pub fn solve_deterministic(
    nuclides: &[Nuclide],
    mix: &[MixComponent],
    band: SlowingDownBand,
    temp_k: f64,
    grid_ev: &[f64],
) -> SlowingDownResult {
    let g = slowing_down_grid(nuclides, mix, band, temp_k, grid_ev, 20);
    solve_on_grid(&g)
}

/// `G(target)` by linear interpolation, or 0 when `target` is above the source
/// (i.e. negative lethargy — nothing has slowed down that far yet).
///
/// `cursor` is a rolling index the caller owns; because the callers ask for a
/// monotonically increasing `target`, it never walks backwards and the whole
/// solve stays linear in the grid size.
fn lookback(g: &[f64], u: &[f64], target: f64, cursor: &mut usize) -> f64 {
    if target < 0.0 {
        return 0.0;
    }
    while *cursor + 1 < u.len() && u[*cursor + 1] <= target {
        *cursor += 1;
    }
    let k = *cursor;
    if k + 1 >= u.len() {
        return g[u.len() - 1];
    }
    let (u0, u1) = (u[k], u[k + 1]);
    if u1 <= u0 {
        return g[k];
    }
    let w = (target - u0) / (u1 - u0);
    g[k] + w * (g[k + 1] - g[k])
}

/// Bisect every interval of an energy grid in **lethargy**, returning a grid
/// with one extra point between each neighbouring pair.
///
/// This is the grid-convergence lever for [`solve_deterministic`]: solve on
/// `grid`, solve again on `refine_lethargy_grid(grid)`, and the difference
/// bounds the discretisation error. Bisecting in lethargy rather than in energy
/// keeps the refinement uniform in the variable the equation is written in.
pub fn refine_lethargy_grid(grid_ev: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(grid_ev.len() * 2);
    for w in grid_ev.windows(2) {
        out.push(w[0]);
        out.push((w[0] * w[1]).sqrt()); // geometric mean = lethargy midpoint
    }
    if let Some(&last) = grid_ev.last() {
        out.push(last);
    }
    out
}

/// Monte-Carlo random walk through the same infinite homogeneous medium, using
/// the crate's own collision kernels.
///
/// There is no geometry: in an infinite homogeneous medium the flight length
/// never matters to the outcome, only the sequence of collisions does, so a
/// history is a loop over collisions. What *is* shared with the real transport
/// loop is everything that decides those collisions — the ∝ `N_i σ_t,i` nuclide
/// sampling, the fission/absorption/inelastic/(n,2n)/elastic partition against a
/// uniform `ξ·σ_t`, and (under [`ScatterKernel::Production`]) the very same
/// `sample_thermal` → `free_gas_elastic_scatter` branch. That is the point: this
/// is not a reimplementation of the physics, it is the physics under a geometry
/// simple enough to have an exact answer.
#[derive(Debug, Clone, Copy)]
pub struct InfiniteMediumMc {
    /// Source neutrons to follow.
    pub histories: usize,
    /// Master RNG seed.
    pub seed: u64,
    /// Which elastic kernel to use — see [`ScatterKernel`].
    pub kernel: ScatterKernel,
    /// Guard against a history that will not terminate. A neutron off U-238
    /// loses at most 1.7 % per collision, so a band two decades wide needs a few
    /// hundred collisions at worst; this is far above that, and being hit is a
    /// defect, not a tuning parameter.
    pub max_collisions: u32,
}

impl Default for InfiniteMediumMc {
    fn default() -> Self {
        Self {
            histories: 200_000,
            seed: 0x5EED_1234_ABCD_0001,
            kernel: ScatterKernel::IsotropicCmAtRest,
            max_collisions: 100_000,
        }
    }
}

impl InfiniteMediumMc {
    /// Follow `histories` neutrons from `band.e_top` until each is absorbed or
    /// falls below `band.e_bot`.
    ///
    /// Panics if any history opens an inelastic or (n,2n) channel — those are
    /// outside what [`solve_deterministic`] models, so hitting one means the
    /// band was chosen above a threshold and the comparison would be invalid.
    pub fn run(
        &self,
        nuclides: &[Nuclide],
        mix: &[MixComponent],
        band: SlowingDownBand,
        temp_k: f64,
    ) -> SlowingDownResult {
        let n_mix = mix.len();
        let mut absorbed_by = vec![0.0_f64; n_mix];
        let mut escaped = 0.0_f64;
        let mut seed = self.seed;
        let kt = K_BOLTZMANN_EV_PER_K * temp_k;

        for _ in 0..self.histories {
            let mut e = band.e_top;
            let mut dir = Direction::new(0.0, 0.0, 1.0);
            let mut collisions = 0u32;

            loop {
                collisions += 1;
                assert!(
                    collisions <= self.max_collisions,
                    "history exceeded {} collisions at {e} eV — the walk is not terminating",
                    self.max_collisions
                );

                let xs: Vec<_> = mix
                    .iter()
                    .map(|m| nuclides[m.nuclide_idx].xs_at_energy(e, temp_k))
                    .collect();
                let mut sigma_t = 0.0;
                for (m, x) in mix.iter().zip(&xs) {
                    sigma_t += m.atom_density * x.total;
                }
                assert!(sigma_t > 0.0, "transparent mixture at {e} eV");

                // Which nuclide: ∝ N_i σ_t,i, as `Material::sample_nuclide` does.
                let mut pick = prn(&mut seed) * sigma_t;
                let mut i = n_mix - 1;
                for (k, (m, x)) in mix.iter().zip(&xs).enumerate() {
                    let w = m.atom_density * x.total;
                    if pick < w {
                        i = k;
                        break;
                    }
                    pick -= w;
                }
                let nuc = &nuclides[mix[i].nuclide_idx];
                let x = &xs[i];

                // Which channel: the same partition as
                // `transport_csg::transport_history`.
                let xi = prn(&mut seed) * x.total;
                if xi < x.absorption {
                    // Fission and capture both remove the neutron here; below
                    // 20 keV U-238's fission is sub-threshold and negligible,
                    // and either way the neutron is gone.
                    absorbed_by[i] += 1.0;
                    break;
                }
                assert!(
                    xi >= x.absorption + x.inelastic + x.n2n,
                    "an inelastic or (n,2n) channel opened at {e} eV — the band reaches a \
                     threshold and the deterministic comparison is not valid there"
                );

                let (e2, d2) = match self.kernel {
                    ScatterKernel::IsotropicCmAtRest => {
                        let mu = 2.0 * prn(&mut seed) - 1.0;
                        two_body_scatter_with_mu(e, dir, nuc.awr, 0.0, mu, &mut seed)
                    }
                    ScatterKernel::AnisotropicCmAtRest => {
                        let mu = nuc
                            .sample_elastic_mu_cm(e, &mut seed)
                            .unwrap_or_else(|| 2.0 * prn(&mut seed) - 1.0);
                        two_body_scatter_with_mu(e, dir, nuc.awr, 0.0, mu, &mut seed)
                    }
                    ScatterKernel::Production => {
                        if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, &mut seed) {
                            (
                                e_out,
                                crate::physics::scatter::rotate_direction(dir, mu_lab, &mut seed),
                            )
                        } else {
                            let mu_cm = nuc
                                .sample_elastic_mu_cm(e, &mut seed)
                                .unwrap_or_else(|| 2.0 * prn(&mut seed) - 1.0);
                            free_gas_elastic_scatter(e, dir, nuc.awr, kt, mu_cm, &mut seed)
                        }
                    }
                };
                e = e2;
                dir = d2;

                if e < band.e_bot {
                    escaped += 1.0;
                    break;
                }
            }
        }

        let inv = 1.0 / self.histories as f64;
        SlowingDownResult {
            absorbed_by: absorbed_by.iter().map(|x| x * inv).collect(),
            escaped: escaped * inv,
            collision_density: Vec::new(),
            lethargy: Vec::new(),
        }
    }

    /// 1σ on a scored fraction `p` from `histories` independent binomial trials.
    pub fn stderr_of(&self, p: f64) -> f64 {
        (p * (1.0 - p) / self.histories as f64).sqrt().max(1.0e-12)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Two-region: the same slowing down, but with the fuel LUMPED
// ─────────────────────────────────────────────────────────────────────────────

/// Monte-Carlo slowing down through a **Wigner-Seitz cell** — a fuel lump inside
/// a moderator shell with a reflective outer boundary — using the crate's real
/// CSG transport.
///
/// # Why this exists
///
/// [`InfiniteMediumMc`] against [`solve_on_grid`] established that this crate's
/// *energy* treatment of self-shielded resonance absorption is right (agreement
/// at every dilution from `σ_b = 30 b` to 10 000 b, an 11.7× collapse of the
/// effective resonance integral). That leaves the **spatial** half: what happens
/// to the flux inside an optically thick lump, which is the one mechanism the
/// FHR pebble and ICSBEP LEU-COMP-THERM-008 share and the three reproduced
/// benchmarks do not.
///
/// This walker is deliberately *not* a re-implementation. Its flights, boundary
/// crossings and collisions go through [`Geometry::locate`],
/// [`Geometry::distance_to_boundary`] and [`Geometry::cross_surface`] — the same
/// calls `transport_csg::transport_history` makes — so what is under test is the
/// real spatial machinery. Only fission banking and the tally plumbing are left
/// out, because a resonance-escape measurement is a per-history outcome rather
/// than a track-length score.
///
/// # The oracle: the thin-lump limit is exact
///
/// Scale the lump and the cell down together at fixed composition and the
/// optical thickness of both regions goes to zero, so the cell becomes
/// **exactly** the homogeneous medium [`solve_on_grid`] solves. That gives a
/// reference that owes nothing to equivalence theory, rational approximations or
/// any remembered correlation:
///
/// - at small scale the answer **must** converge on the homogeneous solution, and
///   if it does not, the spatial machinery is broken and the scan says by how
///   much;
/// - as the scale grows the absorption must fall monotonically, because that is
///   what lumping *is*;
/// - and the size of the fall at a realistic scale is the lumping reactivity,
///   the quantity the ring-RPT residual is now narrowed to.
///
/// # The outer boundary must be WHITE, and that is not a detail
///
/// A **specularly** reflective sphere is not a valid Wigner-Seitz boundary, and
/// the way it fails is silent. Specular reflection off a sphere concentric with
/// the lump conserves the impact parameter `b = r·sin θ` exactly: a neutron
/// leaves at the same angle to the radius it arrived at, so its closest approach
/// to the centre never changes. **A neutron with `b > R_lump` can therefore
/// never enter the lump, at any energy, for the whole of its life.** The lump is
/// starved of exactly the neutrons that should be sampling it, and the effect is
/// *scale-invariant* — it does not weaken as the lump shrinks, because it is
/// geometry, not optics.
///
/// Measured here before the fix: a lump **0.034 mean free paths** across at the
/// 6.67 eV resonance peak — optically transparent, so the cell is provably the
/// homogeneous mixture — returned a resonance escape probability **39 % above**
/// the exact homogeneous answer, and the same 39 % at 0.10, 0.86 and 2.57 mean
/// free paths. A flat offset across two decades of optical thickness is the
/// fingerprint: self-shielding cannot do that, and geometry can.
///
/// [`CellBoundary::White`] is therefore the default: a neutron reaching the
/// outer surface re-enters at a **random** point on it with a cosine-distributed
/// inward direction, which is the standard Wigner-Seitz closure and destroys the
/// invariant. [`CellBoundary::Specular`] is kept only so the defect can be
/// demonstrated rather than described.
///
/// (The FHR ring-RPT pebble is run on a specularly reflective sphere by *both*
/// this crate and its OpenMC reference, so that comparison stays like-for-like;
/// the artefact is in the shared model, not in one side of it.)
///
/// # Source
///
/// Uniform in the cell volume at `band.e_top`, matching [`InfiniteMediumMc`]'s
/// unit source so the thin-lump limit is a like-for-like comparison. At 10 keV —
/// well above the resolved resonances that matter — the real slowing-down source
/// is close to spatially flat, so this is also the physically right choice, not
/// only the convenient one.
#[derive(Debug, Clone, Copy)]
pub struct LumpCellMc {
    /// Source neutrons to follow.
    pub histories: usize,
    /// Master RNG seed.
    pub seed: u64,
    /// Which elastic kernel to use — see [`ScatterKernel`].
    pub kernel: ScatterKernel,
    /// How the outer surface returns a neutron to the cell. **Leave this
    /// [`CellBoundary::White`]** unless you are demonstrating the specular
    /// defect — see the type docs.
    pub boundary: CellBoundary,
    /// Guard against a history that will not terminate (flights *and*
    /// collisions, so a neutron trapped on a surface trips it).
    pub max_events: u32,
}

/// How a neutron reaching the cell's outer surface is returned to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellBoundary {
    /// Re-enter at a **random** point on the outer sphere with a
    /// cosine-distributed inward direction — the standard Wigner-Seitz closure.
    /// Build the geometry with a **vacuum** outer surface and this variant
    /// intercepts the escape.
    White,
    /// Let the geometry's own reflective boundary handle it. **For a sphere this
    /// is wrong**, in the specific and silent way described on [`LumpCellMc`]:
    /// it conserves the impact parameter and can starve the lump entirely.
    /// Retained so the defect stays reproducible.
    Specular,
}

impl Default for LumpCellMc {
    fn default() -> Self {
        Self {
            histories: 100_000,
            seed: 0x5EED_1234_ABCD_0002,
            kernel: ScatterKernel::IsotropicCmAtRest,
            boundary: CellBoundary::White,
            max_events: 1_000_000,
        }
    }
}

impl LumpCellMc {
    /// Follow `histories` neutrons from `band.e_top`, born uniformly in the ball
    /// of radius `cell_radius`, until each is absorbed or falls below
    /// `band.e_bot`.
    ///
    /// `absorbed_by` is indexed by **material**, not by nuclide, so the fuel and
    /// moderator shares are separable; `escaped` is the resonance escape
    /// probability over the band. A neutron that leaves the geometry is a defect
    /// here (the outer boundary is meant to be reflective) and is counted into
    /// `lost` rather than silently folded into either.
    pub fn run(
        &self,
        geom: &Geometry,
        materials: &[Material],
        nuclides: &[Nuclide],
        band: SlowingDownBand,
        temp_k: f64,
        cell_radius: f64,
    ) -> LumpCellResult {
        let mut absorbed_by = vec![0.0_f64; materials.len()];
        let (mut escaped, mut lost) = (0.0_f64, 0.0_f64);
        let mut seed = self.seed;
        let kt = K_BOLTZMANN_EV_PER_K * temp_k;
        const NUDGE: f64 = 1.0e-9;

        for _ in 0..self.histories {
            // Uniform in the ball: r ∝ ξ^{1/3}, isotropic direction.
            let rr = cell_radius * prn(&mut seed).cbrt();
            let (sx, sy, sz) = isotropic(&mut seed);
            let mut pos = Position::new(rr * sx, rr * sy, rr * sz);
            let (dx, dy, dz) = isotropic(&mut seed);
            let mut dir = Direction::new(dx, dy, dz);
            let mut e = band.e_top;
            let mut on_surface = SurfaceToken::NONE;
            let mut events = 0u32;

            'history: loop {
                events += 1;
                assert!(
                    events <= self.max_events,
                    "history exceeded {} events at {e} eV, r = {:?} — the walk is not \
                     terminating",
                    self.max_events,
                    pos
                );

                let Some(path) = geom.locate(pos, dir, on_surface) else {
                    lost += 1.0;
                    break 'history;
                };
                let Some(m) = path.material else {
                    // Void: stream to the next boundary. The cells here are all
                    // filled, so this is a modelling error rather than physics.
                    lost += 1.0;
                    break 'history;
                };
                let sigma_t = materials[m].macro_xs_total(e, nuclides);
                assert!(sigma_t > 0.0, "transparent material {m} at {e} eV");

                let d_bound = geom.distance_to_boundary(&path);
                let d_col = -prn(&mut seed).max(f64::MIN_POSITIVE).ln() / sigma_t;

                if d_col < d_bound.distance {
                    pos = advance(pos, dir, d_col);
                    on_surface = SurfaceToken::NONE;

                    let material = &materials[m];
                    let ci = material.sample_nuclide(e, &mut seed, nuclides);
                    let nuc = &nuclides[material.components[ci].nuclide_idx];
                    let x = nuc.xs_at_energy(e, temp_k);
                    let xi = prn(&mut seed) * x.total;
                    if xi < x.absorption {
                        absorbed_by[m] += 1.0;
                        break 'history;
                    }
                    assert!(
                        xi >= x.absorption + x.inelastic + x.n2n,
                        "an inelastic or (n,2n) channel opened at {e} eV — the band \
                         reaches a threshold and the comparison is not valid there"
                    );
                    let (e2, d2) = self.scatter(nuc, e, dir, kt, &mut seed);
                    e = e2;
                    dir = d2;
                    if e < band.e_bot {
                        escaped += 1.0;
                        break 'history;
                    }
                } else {
                    pos = advance(pos, dir, d_bound.distance);
                    match d_bound.crossing {
                        Crossing::Surface(i_surf) => {
                            let crossed = geom.cross_surface(i_surf, pos, dir);
                            if !crossed.alive {
                                if self.boundary == CellBoundary::White {
                                    // Re-enter at a RANDOM point with a cosine
                                    // inward direction. Randomising the point as
                                    // well as the direction is what breaks the
                                    // impact-parameter invariant; see the type
                                    // docs for what happens when it is not
                                    // broken.
                                    let (nx, ny, nz) = isotropic(&mut seed);
                                    let shrink = cell_radius * (1.0 - 1.0e-12);
                                    pos = Position::new(shrink * nx, shrink * ny, shrink * nz);
                                    let inward = Direction::new(-nx, -ny, -nz);
                                    let mu = prn(&mut seed).sqrt(); // cosine law
                                    dir = crate::physics::scatter::rotate_direction(
                                        inward, mu, &mut seed,
                                    );
                                    on_surface = SurfaceToken::NONE;
                                    continue;
                                }
                                lost += 1.0;
                                break 'history;
                            }
                            pos = crossed.r;
                            dir = crossed.u;
                            on_surface = crossed.on_surface;
                        }
                        Crossing::Lattice => {
                            pos = advance(pos, dir, NUDGE);
                            on_surface = SurfaceToken::NONE;
                        }
                        Crossing::None => {
                            lost += 1.0;
                            break 'history;
                        }
                    }
                }
            }
        }

        let inv = 1.0 / self.histories as f64;
        LumpCellResult {
            absorbed_by: absorbed_by.iter().map(|x| x * inv).collect(),
            escaped: escaped * inv,
            lost: lost * inv,
            histories: self.histories,
        }
    }

    fn scatter(
        &self,
        nuc: &Nuclide,
        e: f64,
        dir: Direction,
        kt: f64,
        seed: &mut u64,
    ) -> (f64, Direction) {
        match self.kernel {
            ScatterKernel::IsotropicCmAtRest => {
                let mu = 2.0 * prn(seed) - 1.0;
                two_body_scatter_with_mu(e, dir, nuc.awr, 0.0, mu, seed)
            }
            ScatterKernel::AnisotropicCmAtRest => {
                let mu = nuc
                    .sample_elastic_mu_cm(e, seed)
                    .unwrap_or_else(|| 2.0 * prn(seed) - 1.0);
                two_body_scatter_with_mu(e, dir, nuc.awr, 0.0, mu, seed)
            }
            ScatterKernel::Production => {
                if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
                    (
                        e_out,
                        crate::physics::scatter::rotate_direction(dir, mu_lab, seed),
                    )
                } else {
                    let mu_cm = nuc
                        .sample_elastic_mu_cm(e, seed)
                        .unwrap_or_else(|| 2.0 * prn(seed) - 1.0);
                    free_gas_elastic_scatter(e, dir, nuc.awr, kt, mu_cm, seed)
                }
            }
        }
    }
}

/// Outcome of a [`LumpCellMc`] run, as fractions of one source neutron.
#[derive(Debug, Clone, PartialEq)]
pub struct LumpCellResult {
    /// Fraction absorbed in each **material**, in the geometry's own order.
    pub absorbed_by: Vec<f64>,
    /// Fraction that reached `band.e_bot` still alive — the resonance escape
    /// probability over the band.
    pub escaped: f64,
    /// Fraction that left the geometry or was lost in it. Should be zero for a
    /// reflective cell; anything else is a geometry defect, not physics.
    pub lost: f64,
    /// Histories run, for the binomial standard error.
    pub histories: usize,
}

impl LumpCellResult {
    /// 1σ on a scored fraction `p`.
    pub fn stderr_of(&self, p: f64) -> f64 {
        (p * (1.0 - p) / self.histories as f64).sqrt().max(1.0e-12)
    }
}

/// A point advanced `d` along `u`. (`transport_csg`'s `stream`, which is private
/// to that module.)
#[inline]
fn advance(r: Position, u: Direction, d: f64) -> Position {
    Position::new(r.x + d * u.u, r.y + d * u.v, r.z + d * u.w)
}

/// A direction sampled uniformly on the unit sphere.
#[inline]
fn isotropic(seed: &mut u64) -> (f64, f64, f64) {
    let mu = 2.0 * prn(seed) - 1.0;
    let phi = std::f64::consts::TAU * prn(seed);
    let s = (1.0 - mu * mu).max(0.0).sqrt();
    (s * phi.cos(), s * phi.sin(), mu)
}

// ─────────────────────────────────────────────────────────────────────────────
// Multi-region: the same slowing down, solved DETERMINISTICALLY in a lump
// ─────────────────────────────────────────────────────────────────────────────

/// A concentric-sphere cell for [`solve_deterministic_multiregion`]: the shell
/// radii and which material fills each shell.
///
/// Shells are flux regions as well as material regions, so subdividing one
/// material into several shells is how the flat-flux approximation is refined.
/// See [`solve_deterministic_multiregion`] for why that matters at 6.6 mean free
/// paths.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellCell {
    /// **Outer** radius of each shell \[cm\], strictly increasing.
    pub radii: Vec<f64>,
    /// Index into the caller's material array, one per shell.
    pub material: Vec<usize>,
}

impl ShellCell {
    /// Subdivide every shell into `k` sub-shells of **equal volume**, keeping
    /// the material assignment. Equal volume rather than equal thickness is what
    /// keeps the flat-flux error evenly spread.
    pub fn subdivide(&self, k: usize) -> ShellCell {
        assert!(k >= 1, "subdivision factor must be at least 1");
        let mut radii = Vec::with_capacity(self.radii.len() * k);
        let mut material = Vec::with_capacity(self.radii.len() * k);
        for (s, &r_out) in self.radii.iter().enumerate() {
            let r_in = if s == 0 { 0.0 } else { self.radii[s - 1] };
            let (v_in, v_out) = (r_in.powi(3), r_out.powi(3));
            for m in 1..=k {
                let v = v_in + (v_out - v_in) * m as f64 / k as f64;
                radii.push(v.cbrt());
                material.push(self.material[s]);
            }
        }
        // The cube-root round-trip can perturb the outer radius in the last bit;
        // pin the original shell boundaries back exactly.
        for (s, &r) in self.radii.iter().enumerate() {
            radii[(s + 1) * k - 1] = r;
        }
        ShellCell { radii, material }
    }

    /// Subdivide shell `s` into `k[s]` equal-volume sub-shells.
    ///
    /// The per-shell form of [`Self::subdivide`], for when only one region needs
    /// the flat-flux discretisation refined: the fuel lump is optically thick at
    /// a resonance peak and an optically thin moderator is not, and the cost of
    /// [`solve_deterministic_multiregion`] grows as the **cube** of the shell
    /// count, so refining both is wasteful rather than conservative.
    pub fn subdivide_each(&self, k: &[usize]) -> ShellCell {
        assert_eq!(
            k.len(),
            self.radii.len(),
            "subdivide_each got {} factors for {} shells",
            k.len(),
            self.radii.len()
        );
        let mut radii = Vec::new();
        let mut material = Vec::new();
        for (s, &r_out) in self.radii.iter().enumerate() {
            assert!(k[s] >= 1, "subdivision factor must be at least 1");
            let r_in = if s == 0 { 0.0 } else { self.radii[s - 1] };
            let (v_in, v_out) = (r_in.powi(3), r_out.powi(3));
            for m in 1..=k[s] {
                let v = v_in + (v_out - v_in) * m as f64 / k[s] as f64;
                radii.push(v.cbrt());
                material.push(self.material[s]);
            }
            // Pin the original shell boundary back exactly: the cube-root round
            // trip can move it in the last bit, and a material interface that
            // drifts is a different geometry.
            let last = radii.len() - 1;
            radii[last] = r_out;
        }
        ShellCell { radii, material }
    }

    /// Shell volumes \[cm³\].
    pub fn volumes(&self) -> Vec<f64> {
        let c = 4.0 / 3.0 * std::f64::consts::PI;
        (0..self.radii.len())
            .map(|k| {
                let r_in = if k == 0 { 0.0 } else { self.radii[k - 1] };
                c * (self.radii[k].powi(3) - r_in.powi(3))
            })
            .collect()
    }
}

/// Where the source neutrons ended up in a [`ShellCell`], as fractions of one
/// source neutron.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiRegionResult {
    /// Fraction absorbed in each **shell**, in the cell's own order.
    pub absorbed_by_shell: Vec<f64>,
    /// Fraction absorbed in each **material**, in the caller's own order.
    pub absorbed_by_material: Vec<f64>,
    /// Fraction that reached `band.e_bot` still alive — the resonance escape
    /// probability over the band, directly comparable with
    /// [`LumpCellResult::escaped`] and with [`SlowingDownResult::escaped`].
    pub escaped: f64,
    /// Worst `|Σ_j P_ij − 1|` seen over the whole lethargy sweep. Analytically
    /// zero; what is left is the impact-parameter quadrature's own error, so
    /// this is the solve's live accuracy monitor.
    pub worst_conservation_defect: f64,
    /// Lethargy points used.
    pub grid_points: usize,
}

impl MultiRegionResult {
    /// Total absorbed fraction — one minus the escape probability.
    pub fn absorbed(&self) -> f64 {
        self.absorbed_by_shell.iter().sum()
    }
}

/// Solve the slowing-down equation on a **concentric-sphere cell** — the lumped
/// problem — deterministically, with no Monte Carlo anywhere in it.
///
/// # What this is for
///
/// [`solve_on_grid`] is exact for an infinite homogeneous medium and settled the
/// *energy* half of self-shielding. This is its spatial extension, and it exists
/// because the FHR ring-RPT fuel annulus (1.4934–1.7531 cm, ≈ 6.6 mean free
/// paths at the 6.674 eV U-238 peak) is **deeply self-shielded and had no
/// reference**: `examples/lump_self_shielding_scan.rs` anchors only the
/// transparent limit and then reports a monotone trend with nothing to check it
/// against.
///
/// # The equations
///
/// Per unit lethargy, with `C_i(u)` the total collision rate in shell `i` and
/// `s_i(u)` the total emission rate,
///
/// ```text
/// C_i(u) = Σ_j s_j(u) · P_{j→i}(u)
/// s_i(u) = q_i δ(u) + Σ_k (1/(1−α_k)) ∫_{u−ε_k}^{u} c_{i,k}(u′) C_i(u′) e^{−(u−u′)} du′
/// ```
///
/// with `c_{i,k} = Σ_{s,k}/Σ_{t,i}` the probability that a collision in shell `i`
/// is a scatter off nuclide `k`, and `q_i = V_i/V_cell` the uniform unit source.
/// Setting `P_{j→i} = δ_{ji}` recovers [`solve_on_grid`] line for line, which is
/// the point: the *only* new ingredient is the spatial coupling.
///
/// `P_{j→i}(u)` are the **exact** first-flight collision probabilities of the
/// cell at that energy, from
/// [`crate::physics::collision_probability::first_flight`] — an impact-parameter
/// track quadrature of the analytic six-fold integral, closed with a white outer
/// boundary. They are recomputed at every lethargy point, so a resonance is
/// self-shielded spatially as well as in energy.
///
/// The lethargy march, the separable `e^{−(u−u′)}` kernel and the `ε_k`-wide
/// lookback are [`solve_on_grid`]'s, so the two share their discretisation and
/// the homogeneous limit below is a like-for-like test of the *coupling*.
/// Because `d_i = Σ_k ½Δu·c_{i,k}` is O(10⁻⁴) at the grid spacing this solver
/// insists on, the per-point `n_shell × n_shell` implicit system is solved by
/// four fixed-point sweeps, converged to O(10⁻¹⁵).
///
/// # The one approximation, and how it is removed
///
/// Flat flux and flat source **within a shell**. That is a discretisation, not a
/// modelling choice: [`ShellCell::subdivide`] splits every shell into equal-volume
/// sub-shells, and the answer must converge. Refinement is the accuracy
/// statement, and callers are expected to demonstrate it rather than assume it —
/// `examples/lump_self_shielding_scan.rs` does.
///
/// # The exact test this construction admits
///
/// Give every shell the **same** material. Then `Σ_j V_j P_{j→i} = V_i` follows
/// from reciprocity plus conservation, so `C_i ∝ V_i` solves the coupled system
/// and the cell must return the infinite-medium answer **identically, at every
/// geometric size**. Nothing in the quadrature, the closure or the coupling is
/// free to be wrong and still pass that, which is why it is the first gate in
/// `tests/lump_collision_probability.rs`.
#[allow(clippy::too_many_arguments)]
pub fn solve_deterministic_multiregion(
    cell: &ShellCell,
    materials: &[Material],
    nuclides: &[Nuclide],
    band: SlowingDownBand,
    temp_k: f64,
    grid_ev: &[f64],
    cp_nodes: usize,
    steps_per_window: usize,
) -> MultiRegionResult {
    let n_shell = cell.radii.len();
    assert!(n_shell > 0, "an empty cell has no slowing-down solution");
    assert_eq!(
        cell.material.len(),
        n_shell,
        "the cell has {n_shell} shells but {} material assignments",
        cell.material.len()
    );
    assert!(
        band.e_bot > 0.0 && band.e_top > band.e_bot,
        "band must be a positive energy interval, got {band:?}"
    );
    assert!(steps_per_window >= 2, "steps_per_window must be at least 2");

    // Distinct materials actually used, and the global nuclide each component
    // refers to. `alpha` is indexed by the caller's global nuclide array so the
    // scatter window ε_k = ln(1/α_k) is a property of the nuclide, not of where
    // it happens to sit.
    let alpha: Vec<f64> = nuclides
        .iter()
        .map(|n| ((n.awr - 1.0) / (n.awr + 1.0)).powi(2))
        .collect();
    let used: Vec<usize> = {
        let mut v: Vec<usize> = cell.material.clone();
        v.sort_unstable();
        v.dedup();
        v
    };
    let mut eps_min = f64::INFINITY;
    for &m in &used {
        for c in &materials[m].components {
            let a = alpha[c.nuclide_idx];
            if a > 0.0 {
                eps_min = eps_min.min(-a.ln());
            }
        }
    }
    let du_max = if eps_min.is_finite() {
        eps_min / steps_per_window as f64
    } else {
        0.01
    };
    let (u, e) = build_lethargy_grid(band, grid_ev, du_max);
    let n = u.len();
    assert!(
        n > 2,
        "grid has {n} points inside the band — nothing to solve"
    );

    let volume = cell.volumes();
    let v_cell: f64 = volume.iter().sum();

    // Per-material scratch, refilled at every lethargy point.
    let n_nuc = nuclides.len();
    let mut sigma_t_mat = vec![0.0_f64; materials.len()];
    let mut absorb_frac_mat = vec![0.0_f64; materials.len()];
    // c[m * n_nuc + k] = (N_k σ_s,k / Σ_t,m) / (1 − α_k), the kernel's own form.
    let mut c_mat = vec![0.0_f64; materials.len() * n_nuc];
    let mut c_prev = vec![0.0_f64; materials.len() * n_nuc];
    let mut absorb_prev = vec![0.0_f64; materials.len()];
    let mut sigma_t_shell = vec![0.0_f64; n_shell];

    // G_{i,k}(u) running integrals, one column per lethargy point. Only the
    // (shell, nuclide) pairs that exist are ever touched.
    let mut gi = vec![0.0_f64; n_shell * n_nuc * n];
    let mut back = vec![0_usize; n_shell * n_nuc];

    let mut c_now = vec![0.0_f64; n_shell];
    let mut c_next = vec![0.0_f64; n_shell];
    let mut c_last = vec![0.0_f64; n_shell];
    let mut known = vec![0.0_f64; n_shell];
    let mut diag = vec![0.0_f64; n_shell];
    let mut absorbed_by_shell = vec![0.0_f64; n_shell];
    let mut worst_conservation = 0.0_f64;

    let fill_materials =
        |ee: f64, sigma_t_mat: &mut [f64], absorb_frac_mat: &mut [f64], c_mat: &mut [f64]| {
            for &m in &used {
                let mat = &materials[m];
                for k in 0..n_nuc {
                    c_mat[m * n_nuc + k] = 0.0;
                }
                // One cross-section lookup per component, not two: the reconstructed
                // evaluation is the dominant cost of the whole solve at 2e5 lethargy
                // points, and `Σ_t` is needed before the branch fractions can be
                // normalised, so the unnormalised scatter is accumulated first.
                let mut st = 0.0;
                let mut sa = 0.0;
                for comp in &mat.components {
                    let k = comp.nuclide_idx;
                    let x = nuclides[k].xs_at_energy(ee, temp_k);
                    st += comp.atom_density * x.total;
                    sa += comp.atom_density * x.absorption;
                    let scat = (x.total - x.absorption - x.inelastic - x.n2n).max(0.0);
                    c_mat[m * n_nuc + k] += comp.atom_density * scat / (1.0 - alpha[k]);
                }
                assert!(
                    st > 0.0,
                    "material {m} is transparent at {ee} eV — no collision \
                 probability is defined there"
                );
                sigma_t_mat[m] = st;
                absorb_frac_mat[m] = sa / st;
                // Over all nuclide slots rather than over components: a material
                // is allowed to list the same nuclide twice, and dividing per
                // component would then divide that entry twice.
                let inv = 1.0 / st;
                for k in 0..n_nuc {
                    c_mat[m * n_nuc + k] *= inv;
                }
            }
        };

    // ── The source point, u = 0 ──────────────────────────────────────────────
    fill_materials(e[0], &mut sigma_t_mat, &mut absorb_frac_mat, &mut c_mat);
    for i in 0..n_shell {
        sigma_t_shell[i] = sigma_t_mat[cell.material[i]];
    }
    let cp0 =
        crate::physics::collision_probability::first_flight(&cell.radii, &sigma_t_shell, cp_nodes);
    worst_conservation = worst_conservation.max(cp0.conservation_defect());
    // The delta source emits q_j = V_j/V_cell; its FIRST collisions land in i.
    for i in 0..n_shell {
        let mut ci = 0.0;
        for j in 0..n_shell {
            ci += volume[j] / v_cell * cp0.p_at(j, i);
        }
        c_now[i] = ci;
        absorbed_by_shell[i] += ci * absorb_frac_mat[cell.material[i]];
        for k in 0..n_nuc {
            gi[(i * n_nuc + k) * n] = c_mat[cell.material[i] * n_nuc + k] * ci;
        }
    }
    // The CONTINUOUS collision density just below the source: the impulse's own
    // scatter-in evaluated at u → 0⁺, then transported once. `solve_on_grid`'s
    // `f[0]` term, with the spatial coupling applied to it.
    for i in 0..n_shell {
        let m = cell.material[i];
        let mut si = 0.0;
        for k in 0..n_nuc {
            si += c_mat[m * n_nuc + k] * c_now[i];
        }
        known[i] = si; // reused as the emission density at 0⁺
    }
    for i in 0..n_shell {
        let mut ci = 0.0;
        for j in 0..n_shell {
            ci += known[j] * cp0.p_at(j, i);
        }
        c_last[i] = ci;
    }
    c_prev.copy_from_slice(&c_mat);
    absorb_prev.copy_from_slice(&absorb_frac_mat);

    // ── The lethargy march ───────────────────────────────────────────────────
    let eps: Vec<f64> = alpha
        .iter()
        .map(|a| if *a > 0.0 { -a.ln() } else { f64::INFINITY })
        .collect();
    for jx in 1..n {
        let du = u[jx] - u[jx - 1];
        assert!(
            du <= 0.25 * eps_min,
            "grid interval Δu = {du:.3e} at u = {} exceeds a quarter of the \
             narrowest scatter window ε = {eps_min:.3e}; the scatter-in integral \
             cannot be represented",
            u[jx]
        );
        fill_materials(e[jx], &mut sigma_t_mat, &mut absorb_frac_mat, &mut c_mat);
        for i in 0..n_shell {
            sigma_t_shell[i] = sigma_t_mat[cell.material[i]];
        }
        let cp = crate::physics::collision_probability::first_flight(
            &cell.radii,
            &sigma_t_shell,
            cp_nodes,
        );
        worst_conservation = worst_conservation.max(cp.conservation_defect());

        let e_uj = u[jx].exp();
        let e_ujm = u[jx - 1].exp();
        for i in 0..n_shell {
            let m = cell.material[i];
            let mut rhs = 0.0;
            let mut d = 0.0;
            for k in 0..n_nuc {
                let cj = c_mat[m * n_nuc + k];
                let cjm = c_prev[m * n_nuc + k];
                if cj == 0.0 && cjm == 0.0 {
                    continue;
                }
                let row = (i * n_nuc + k) * n;
                let g = &gi[row..row + n];
                let known_g = g[jx - 1] + 0.5 * du * cjm * c_last[i] * e_ujm;
                let shifted = if eps[k].is_finite() {
                    lookback(g, &u, u[jx] - eps[k], &mut back[i * n_nuc + k])
                } else {
                    0.0
                };
                rhs += (known_g - shifted) / e_uj;
                d += 0.5 * du * cj;
            }
            known[i] = rhs;
            diag[i] = d;
            assert!(
                d < 1.0,
                "slowing-down solve is unstable at u = {} in shell {i} \
                 (within-cell scatter weight {d})",
                u[jx]
            );
        }
        // C_i = Σ_j P_{j→i} (known_j + d_j C_j). `d` is O(Δu) ≈ 4e-4 here, so
        // four fixed-point sweeps are converged to O(1e-15).
        for i in 0..n_shell {
            let mut ci = 0.0;
            for j in 0..n_shell {
                ci += known[j] * cp.p_at(j, i);
            }
            c_now[i] = ci;
        }
        for _ in 0..4 {
            for i in 0..n_shell {
                let mut ci = 0.0;
                for j in 0..n_shell {
                    ci += (known[j] + diag[j] * c_now[j]) * cp.p_at(j, i);
                }
                c_next[i] = ci;
            }
            c_now.copy_from_slice(&c_next);
        }

        for i in 0..n_shell {
            let m = cell.material[i];
            for k in 0..n_nuc {
                let cj = c_mat[m * n_nuc + k];
                let cjm = c_prev[m * n_nuc + k];
                let row = (i * n_nuc + k) * n;
                gi[row + jx] =
                    gi[row + jx - 1] + 0.5 * du * (cjm * c_last[i] * e_ujm + cj * c_now[i] * e_uj);
            }
            absorbed_by_shell[i] +=
                0.5 * du * (absorb_prev[m] * c_last[i] + absorb_frac_mat[m] * c_now[i]);
        }
        c_last.copy_from_slice(&c_now);
        c_prev.copy_from_slice(&c_mat);
        absorb_prev.copy_from_slice(&absorb_frac_mat);
    }

    let mut absorbed_by_material = vec![0.0_f64; materials.len()];
    for (i, a) in absorbed_by_shell.iter().enumerate() {
        absorbed_by_material[cell.material[i]] += a;
    }
    let total: f64 = absorbed_by_shell.iter().sum();
    MultiRegionResult {
        absorbed_by_shell,
        absorbed_by_material,
        escaped: (1.0 - total).max(0.0),
        worst_conservation_defect: worst_conservation,
        grid_points: n,
    }
}

/// The band's lethargy grid: the nuclides' own energy points that fall inside
/// it, subdivided so no interval exceeds `du_max`.
///
/// Shared by [`slowing_down_grid`] and [`solve_deterministic_multiregion`] so
/// the homogeneous and lumped solves discretise identically and their difference
/// is the geometry rather than the mesh. Returns `(lethargy, energy)`.
fn build_lethargy_grid(
    band: SlowingDownBand,
    grid_ev: &[f64],
    du_max: f64,
) -> (Vec<f64>, Vec<f64>) {
    let mut u: Vec<f64> = vec![0.0];
    let mut e: Vec<f64> = vec![band.e_top];
    for &ee in grid_ev.iter().rev() {
        if ee < band.e_top && ee >= band.e_bot {
            let uu = (band.e_top / ee).ln();
            if uu > u[u.len() - 1] + 1.0e-13 {
                u.push(uu);
                e.push(ee);
            }
        }
    }
    assert!(
        u.len() > 2,
        "grid has {} points inside the band — nothing to solve",
        u.len()
    );
    let mut uu: Vec<f64> = Vec::with_capacity(u.len() * 2);
    let mut ee: Vec<f64> = Vec::with_capacity(u.len() * 2);
    for k in 0..u.len() - 1 {
        uu.push(u[k]);
        ee.push(e[k]);
        let span = u[k + 1] - u[k];
        let steps = (span / du_max).ceil() as usize;
        for m in 1..steps {
            let uq = u[k] + span * m as f64 / steps as f64;
            uu.push(uq);
            ee.push(band.e_top * (-uq).exp());
        }
    }
    uu.push(u[u.len() - 1]);
    ee.push(e[e.len() - 1]);
    (uu, ee)
}
