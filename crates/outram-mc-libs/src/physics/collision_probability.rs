//! First-flight **collision probabilities** for concentric spheres, computed by
//! exact track quadrature — the geometric half of an oracle for resonance
//! absorption in an optically thick lump.
//!
//! # Why this module exists
//!
//! [`crate::physics::slowing_down::solve_on_grid`] gives the **exact** answer
//! for an infinite *homogeneous* medium, and `examples/slowing_down_oracle.rs`
//! showed the Monte Carlo reproduces it at every dilution from σ_b = 30 b to
//! 10 000 b. That closed the *energy* half of self-shielding.
//!
//! The *spatial* half had no oracle at the working point.
//! `examples/lump_self_shielding_scan.rs` anchors the **transparent** limit
//! (−0.51 % against the exact homogeneous solution at 0.10 mean free paths) and
//! then reports a monotone rise with lump size with nothing to check it against.
//! The FHR ring-RPT fuel annulus is 1.4934–1.7531 cm and is **≈ 6.6 mean free
//! paths thick at the 6.674 eV U-238 resonance peak**, where that concentration
//! of the whole heavy-metal inventory is worth +3327 pcm over naive
//! homogenisation. Nothing checked whether it is the *right* +3327 pcm.
//!
//! This module supplies what was missing: a way to compute, without any Monte
//! Carlo, the probability that a neutron born uniformly and isotropically in
//! shell `i` of a concentric-sphere cell has its **first collision** in shell
//! `j`. Feeding those into a multi-region slowing-down solve
//! ([`crate::physics::slowing_down::solve_deterministic_multiregion`]) gives a
//! deterministic reference for the lumped resonance absorption itself.
//!
//! # The quadrature, and why it is exact rather than approximate
//!
//! For a flat isotropic source in region `i`,
//!
//! ```text
//! V_i Σ_i P_ij = ∫_{V_i} dr ∫_{V_j} dr′ Σ_i Σ_j e^{−τ(r,r′)} / (4π|r−r′|²)
//! ```
//!
//! Writing `r′ = r + sΩ` and reorganising `∫_{V_i} dr ∫ dΩ/4π` into families of
//! parallel lines (`∫ dΩ/4π ∫ dA_⊥ ∫ dt`) collapses the six-dimensional
//! integral onto straight **tracks**. On one track the double segment integral
//! is elementary,
//!
//! ```text
//! ∫_{t∈a} dt ∫_{s∈b, s>t} ds Σ_i Σ_j e^{−τ} = (1−e^{−τ_a})(1−e^{−τ_b}) e^{−τ_between}
//! ```
//!
//! — the cross sections cancel — and for a **spherically symmetric** geometry
//! every direction `Ω` gives the same track family, so `∫dΩ/4π` is free and what
//! is left is a **one-dimensional** integral over the impact parameter `p`:
//!
//! ```text
//! V_i Σ_i P_ij = ∫_0^{R} 2πp dp · Σ_{a∈i} Σ_{b∈j, b after a} (1−e^{−τ_a})(1−e^{−τ_b}) e^{−τ_between}
//! ```
//!
//! with the same-segment term `τ_a − 1 + e^{−τ_a}` for `a = b`. The only
//! approximation left is the quadrature in `p`, and the integrand is smooth once
//! each shell interval is mapped through `p² = r_k² − s²` (which is exactly the
//! substitution that removes the square-root turning point at `p = r_k`). Gauss–
//! Legendre in `s` then converges geometrically; [`first_flight`] takes the node
//! count so convergence can be **demonstrated** rather than asserted.
//!
//! Only "flat and isotropic **within a shell**" is physics rather than
//! quadrature — and that is a discretisation, removed by subdividing, not a
//! modelling choice. [`crate::physics::slowing_down::solve_deterministic_multiregion`]
//! exposes the subdivision for exactly that reason.
//!
//! # The white-boundary closure
//!
//! A Wigner–Seitz cell is closed with a **white** outer boundary (re-entry at a
//! random point with a cosine-distributed inward direction), which is what
//! `LumpCellMc` does and what
//! `examples/lump_self_shielding_scan.rs` documents at length: a *specular*
//! sphere conserves the impact parameter and silently starves the lump.
//!
//! Tracks that leave the cell are therefore not lost, they are reinjected. The
//! standard closure is exact and needs only two more quadratures on the same
//! tracks — the escape probabilities `P_iS` and the surface-to-region
//! probabilities `P_Sj` for an isotropic incident flux:
//!
//! ```text
//! P_ij = P_ij⁰ + P_iS · P_Sj / (1 − P_SS),      P_SS = 1 − Σ_j P_Sj
//! ```
//!
//! Because `Σ_j P_Sj/(1−P_SS) = 1` by construction, `Σ_j P_ij = Σ_j P_ij⁰ +
//! P_iS = 1` identically: the closed cell conserves neutrons exactly, and
//! [`CollisionProbabilities::conservation_defect`] measures it as a check on the
//! quadrature rather than as an assumption.
//!
//! # What it was measured against (2026-09-12)
//!
//! - the closed-form escape probability of a bare sphere,
//!   `P_esc(x) = (3/4x)[1 − 1/(2x²) + (1/x + 1/(2x²))e^{−2x}]`, over
//!   `x = 10⁻³ … 10²` at ten points per decade — worst relative departure
//!   **1.09e-4** at 8 Gauss nodes per shell interval, **6.62e-6** at 16 and
//!   **1.54e-13** at 48, so the convergence is demonstrated rather than
//!   asserted;
//! - the reciprocity identity `4 V_i Σ_i P_iS = A P_Si` on a four-shell cell at
//!   three cross-section sets spanning four decades (including a near-void
//!   shell next to a near-black one, which is what a resonance peak makes):
//!   worst **3.7e-16** relative;
//! - conservation `Σ_j P_ij = 1` after the white closure: worst **8.9e-16**;
//! - invariance under splitting a homogeneous sphere of `Σ_t R = 5` into 1, 2,
//!   5 and 20 shells: worst **4.4e-16** on the aggregate escape probability;
//! - the Gauss–Legendre generator against `∫_{−1}^{1} x^{2m} dx` for every even
//!   moment a rule is exact for, `n = 2 … 64`: worst **1.68e-14**.
//!
//! And, through
//! [`crate::physics::slowing_down::solve_deterministic_multiregion`], against
//! the infinite-medium solution: a cell whose every shell carries the **same**
//! material must return [`crate::physics::slowing_down::solve_on_grid`]'s
//! answer identically, and does, to **1.1e-12** at cell radii from 0.01 cm to
//! 100 cm and 3 to 24 shells.
//!
//! The regression form of all of these is
//! `tests/lump_collision_probability.rs`.
//!
//! # The closed form was the thing that was wrong
//!
//! The first draft of [`sphere_escape_probability`]'s small-`x` series carried
//! `−(1/5)x³` where the expansion gives `−(1/6)x³`. [`first_flight`] disagreed
//! with it by 2.1e-6 at `x = 0.0398` and 3.3e-8 at `x = 0.01` — exactly
//! `(1/5 − 1/6)x³`. The numerical method was right and the hand-derived oracle
//! was wrong, which is the argument for keeping two independent routes to the
//! same number even when one of them is "analytic".

/// First-flight collision probabilities of a concentric-sphere cell, already
/// closed with a white outer boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionProbabilities {
    /// Number of shells (= number of flux regions).
    pub n: usize,
    /// `p[i * n + j]` = probability that a neutron born flat-isotropic in shell
    /// `i` has its first collision in shell `j`, **after** white re-entry. Rows
    /// sum to 1.
    pub p: Vec<f64>,
    /// `p_open[i * n + j]` — the same, but with the outer surface treated as a
    /// vacuum (no re-entry). Rows sum to `1 − p_escape[i]`.
    pub p_open: Vec<f64>,
    /// Probability that a neutron born flat-isotropic in shell `i` reaches the
    /// outer surface uncollided.
    pub p_escape: Vec<f64>,
    /// Probability that a neutron entering through the outer surface with an
    /// isotropic incident flux has its first collision in shell `j`.
    pub p_surface_to: Vec<f64>,
    /// Shell volumes \[cm³\], for reciprocity checks and for volume weighting.
    pub volume: Vec<f64>,
}

impl CollisionProbabilities {
    /// `P_{i→j}` under the white closure.
    #[inline]
    pub fn p_at(&self, i: usize, j: usize) -> f64 {
        self.p[i * self.n + j]
    }

    /// Worst `|Σ_j P_ij − 1|` over the rows. Exactly zero is the analytic value;
    /// what is left is quadrature error, so this is a live accuracy monitor.
    pub fn conservation_defect(&self) -> f64 {
        (0..self.n)
            .map(|i| {
                let s: f64 = (0..self.n).map(|j| self.p_at(i, j)).sum();
                (s - 1.0).abs()
            })
            .fold(0.0_f64, f64::max)
    }

    /// Worst relative departure from the surface reciprocity identity
    /// `4 V_i Σ_i P_iS = A P_Si`, given the `sigma_t` the probabilities were
    /// built with and the cell's outer radius.
    ///
    /// This is an *independent* identity: `P_iS` comes from a volume source and
    /// `P_Si` from a surface source, and nothing in the quadrature enforces the
    /// relation between them.
    pub fn reciprocity_defect(&self, sigma_t: &[f64], r_out: f64) -> f64 {
        let area = 4.0 * std::f64::consts::PI * r_out * r_out;
        (0..self.n)
            .map(|i| {
                let lhs = 4.0 * self.volume[i] * sigma_t[i] * self.p_escape[i];
                let rhs = area * self.p_surface_to[i];
                let scale = lhs.abs().max(rhs.abs()).max(1.0e-300);
                (lhs - rhs).abs() / scale
            })
            .fold(0.0_f64, f64::max)
    }
}

/// Closed-form escape probability of a **bare homogeneous sphere** of optical
/// radius `x = Σ_t R`, for a uniform isotropic source.
///
/// ```text
/// P_esc(x) = (3 / 4x) · [ 1 − 1/(2x²) + (1/x + 1/(2x²)) e^{−2x} ]
/// ```
///
/// `P_esc → 1 − (3/4)x + (2/5)x²` as `x → 0` and `→ 3/(4x) = 1/(Σ_t l̄)` as
/// `x → ∞`, with `l̄ = 4V/S = 4R/3` the mean chord. The series form is used
/// below `x = 0.05`, where the closed form cancels catastrophically.
///
/// This is the oracle [`first_flight`] is checked against; it is *not* used to
/// compute anything.
pub fn sphere_escape_probability(x: f64) -> f64 {
    assert!(x >= 0.0, "optical radius must be non-negative, got {x}");
    if x < 0.05 {
        // 1 − (3/4)x + (2/5)x² − (1/6)x³ + (2/35)x⁴ − (1/60)x⁵ + (4/945)x⁶,
        // from expanding e^{−2x} in the closed form and cancelling the 1/(2x²)
        // and 1/x poles analytically. Truncation below x = 0.05 is ~5e-13,
        // which is better than the closed form manages there: at x = 0.05 the
        // bracket is 0.064 built from terms of size 200, so it loses ~3.5
        // digits to cancellation.
        //
        // The earlier draft of this series had −(1/5)x³ instead of −(1/6)x³.
        // `first_flight` found it: the quadrature and the "oracle" disagreed by
        // 2.1e-6 at x = 0.0398 and by 3.3e-8 at x = 0.01, which is exactly
        // (1/5 − 1/6)x³ — the numerical method was right and the closed form
        // was wrong. Keep the two independent.
        let x2 = x * x;
        let x3 = x2 * x;
        return 1.0 - 0.75 * x + 0.4 * x2 - x3 / 6.0 + (2.0 / 35.0) * x2 * x2 - x3 * x2 / 60.0
            + (4.0 / 945.0) * x3 * x3;
    }
    let inv = 1.0 / x;
    let inv2 = 0.5 * inv * inv;
    0.75 * inv * (1.0 - inv2 + (inv + inv2) * (-2.0 * x).exp())
}

/// Compute the first-flight collision probabilities of a concentric-sphere cell.
///
/// `radii` are the **outer** radii of the shells, strictly increasing and all
/// positive; `sigma_t[k]` is the macroscopic total cross section \[cm⁻¹\] of
/// shell `k`. `nodes` is the Gauss–Legendre order used on each impact-parameter
/// interval — 32 is already at round-off for smooth cases, and the parameter
/// exists so convergence can be shown.
///
/// Cost is `O(nodes · n_shells³)`: each of the `nodes · n_shells` tracks carries
/// up to `2·n_shells − 1` segments and the segment pair loop is quadratic in
/// that. No transcendental function is evaluated inside the pair loop — the
/// running attenuation between two segments is accumulated multiplicatively —
/// so this is cheap enough to call at every point of a resonance-resolved
/// lethargy grid.
pub fn first_flight(radii: &[f64], sigma_t: &[f64], nodes: usize) -> CollisionProbabilities {
    let n = radii.len();
    assert!(n > 0, "a cell needs at least one shell");
    assert_eq!(
        sigma_t.len(),
        n,
        "sigma_t has {} entries for {n} shells",
        sigma_t.len()
    );
    assert!(radii[0] > 0.0, "innermost radius must be positive");
    for k in 1..n {
        assert!(
            radii[k] > radii[k - 1],
            "radii must be strictly increasing, got {:?}",
            radii
        );
    }
    for (k, s) in sigma_t.iter().enumerate() {
        assert!(
            *s > 0.0 && s.is_finite(),
            "shell {k} has Σ_t = {s}; a void region has no collision probability \
             (the 0/0 limit is finite but is not what this quadrature computes)"
        );
    }
    assert!(nodes >= 2, "need at least 2 quadrature nodes, got {nodes}");

    let four_pi_over_3 = 4.0 / 3.0 * std::f64::consts::PI;
    let volume: Vec<f64> = (0..n)
        .map(|k| {
            let inner = if k == 0 { 0.0 } else { radii[k - 1] };
            four_pi_over_3 * (radii[k].powi(3) - inner.powi(3))
        })
        .collect();

    let (gx, gw) = gauss_legendre(nodes);

    // Track accumulators, all in "∫ 2πp dp ·" units.
    let mut acc = vec![0.0_f64; n * n];
    let mut esc = vec![0.0_f64; n];
    let mut surf = vec![0.0_f64; n];

    // Scratch, reused across tracks.
    let max_seg = 2 * n - 1;
    let mut region = vec![0_usize; max_seg];
    let mut tau = vec![0.0_f64; max_seg];
    let mut a_of = vec![0.0_f64; max_seg]; // 1 − e^{−τ}
    let mut em = vec![0.0_f64; max_seg]; // e^{−τ}
    let mut tail = vec![0.0_f64; max_seg + 1]; // e^{−Σ_{t>s} τ_t}

    for k in 0..n {
        // Impact parameters in (r_{k−1}, r_k] — tracks whose innermost shell is k.
        let lo = if k == 0 { 0.0 } else { radii[k - 1] };
        let hi = radii[k];
        // p² = hi² − s², so s ∈ [0, s_max] and 2πp dp = 2πs ds. The substitution
        // is what makes the integrand smooth at the turning point p = hi.
        let s_max = (hi * hi - lo * lo).sqrt();
        if s_max <= 0.0 {
            continue;
        }
        for (node, weight) in gx.iter().zip(&gw) {
            // Map the [−1, 1] node onto [0, s_max].
            let s = 0.5 * s_max * (node + 1.0);
            let w = 0.5 * s_max * weight * (2.0 * std::f64::consts::PI * s);
            let p2 = (hi * hi - s * s).max(0.0);

            // Build inbound leg (outermost shell first), the turning segment,
            // then the outbound leg (mirror of the inbound).
            let mut h = vec![0.0_f64; n];
            for (j, hj) in h.iter_mut().enumerate().take(n).skip(k) {
                *hj = (radii[j] * radii[j] - p2).max(0.0).sqrt();
            }
            let mut nseg = 0usize;
            for j in (k + 1..n).rev() {
                region[nseg] = j;
                tau[nseg] = sigma_t[j] * (h[j] - h[j - 1]);
                nseg += 1;
            }
            region[nseg] = k;
            tau[nseg] = sigma_t[k] * 2.0 * h[k];
            nseg += 1;
            for j in k + 1..n {
                region[nseg] = j;
                tau[nseg] = sigma_t[j] * (h[j] - h[j - 1]);
                nseg += 1;
            }

            for s_idx in 0..nseg {
                let t = tau[s_idx];
                let e = (-t).exp();
                em[s_idx] = e;
                a_of[s_idx] = 1.0 - e;
            }
            tail[nseg] = 1.0;
            for s_idx in (0..nseg).rev() {
                tail[s_idx] = tail[s_idx + 1] * em[s_idx];
            }

            // Pairs, plus the same-segment term, plus escape and surface-in.
            let mut before = 1.0_f64; // e^{−Σ_{t<b} τ_t}
            for b in 0..nseg {
                surf[region[b]] += w * a_of[b] * before;
                before *= em[b];
            }
            for a in 0..nseg {
                let ra = region[a];
                // a == b: born and collided in the same segment.
                acc[ra * n + ra] += w * (tau[a] - a_of[a]);
                // Escape: leave the cell without colliding after the birth segment.
                esc[ra] += w * a_of[a] * tail[a + 1];
                let mut between = 1.0_f64;
                for b in a + 1..nseg {
                    acc[ra * n + region[b]] += w * a_of[a] * a_of[b] * between;
                    between *= em[b];
                }
            }
        }
    }

    let r_out = radii[n - 1];
    let projected = std::f64::consts::PI * r_out * r_out;

    let mut p_open = vec![0.0_f64; n * n];
    let mut p_escape = vec![0.0_f64; n];
    for i in 0..n {
        let norm = 1.0 / (volume[i] * sigma_t[i]);
        for j in 0..n {
            p_open[i * n + j] = acc[i * n + j] * norm;
        }
        p_escape[i] = esc[i] * norm;
    }
    let p_surface_to: Vec<f64> = surf.iter().map(|s| s / projected).collect();
    let p_ss = 1.0 - p_surface_to.iter().sum::<f64>();
    assert!(
        p_ss < 1.0 - 1.0e-12,
        "the cell is transparent to a surface source (P_SS = {p_ss}); the white \
         closure 1/(1 − P_SS) does not exist"
    );
    let reinject = 1.0 / (1.0 - p_ss);

    let mut p = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            p[i * n + j] = p_open[i * n + j] + p_escape[i] * p_surface_to[j] * reinject;
        }
    }

    CollisionProbabilities {
        n,
        p,
        p_open,
        p_escape,
        p_surface_to,
        volume,
    }
}

/// Gauss–Legendre nodes and weights on `[−1, 1]`, by Newton iteration on the
/// Legendre polynomial with the standard Tricomi starting guess.
///
/// Written here rather than pulled in so the quadrature the oracle rests on has
/// no external dependency; it is checked against `∫_{−1}^{1} x^{2m} dx` in
/// `tests/lump_collision_probability.rs`.
pub fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    assert!(n >= 1, "Gauss-Legendre needs at least one node");
    let mut x = vec![0.0_f64; n];
    let mut w = vec![0.0_f64; n];
    let m = n.div_ceil(2);
    for i in 0..m {
        // Tricomi's asymptotic guess for the i-th root (1-based).
        let mut z = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        for _ in 0..100 {
            // Legendre P_n(z) and its derivative by the recurrence.
            let (mut p0, mut p1) = (1.0_f64, 0.0_f64);
            for j in 0..n {
                let p2 = p1;
                p1 = p0;
                p0 = ((2.0 * j as f64 + 1.0) * z * p1 - j as f64 * p2) / (j as f64 + 1.0);
            }
            let dp = n as f64 * (z * p0 - p1) / (z * z - 1.0);
            let dz = p0 / dp;
            z -= dz;
            if dz.abs() < 1.0e-15 {
                break;
            }
        }
        let (mut p0, mut p1) = (1.0_f64, 0.0_f64);
        for j in 0..n {
            let p2 = p1;
            p1 = p0;
            p0 = ((2.0 * j as f64 + 1.0) * z * p1 - j as f64 * p2) / (j as f64 + 1.0);
        }
        let dp = n as f64 * (z * p0 - p1) / (z * z - 1.0);
        x[i] = -z;
        x[n - 1 - i] = z;
        let wi = 2.0 / ((1.0 - z * z) * dp * dp);
        w[i] = wi;
        w[n - 1 - i] = wi;
    }
    (x, w)
}
