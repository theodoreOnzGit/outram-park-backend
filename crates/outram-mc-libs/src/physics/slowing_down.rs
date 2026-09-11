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

use crate::geometry::position::Direction;
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

    // Energies inside the band, as increasing lethargy from the source.
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

    // Cap the lethargy step. A resonance-adaptive energy grid is dense at
    // resonances and sparse between them, and it is the sparse stretches that
    // silently break the solve.
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
