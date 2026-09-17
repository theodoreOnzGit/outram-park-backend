//! Incoherent-inelastic thermal scattering from S(α,β) — the core THERMR physics.
//!
//! In the incoherent approximation a bound scatterer's double-differential
//! scattering cross section is (ENDF-102 §7; `sig`/`sigl`/`calcem` in
//! `thermr.f90`):
//!
//! ```text
//!   d²σ/dE'dμ (E→E',μ) = (σ_b / 2kT)·√(E'/E)·S̃(α,β)·exp(−β/2)
//! ```
//!
//! with the dimensionless momentum- and energy-transfer variables
//!
//! ```text
//!   α = (E + E' − 2μ√(E·E')) / (A·kT),      β = (E' − E) / kT,
//! ```
//!
//! where `A = B(3)` is the mass ratio, `kT` the temperature in eV, and `σ_b` the
//! bound cross section. `S̃(α,β)` is the (β-symmetric) scattering law stored in
//! MF=7/MT=4; the `exp(−β/2)` factor restores detailed balance for the physical
//! (asymmetric) `S`. When `LAT=1` the tabulated `α,β` are scaled to the reference
//! `kT₀ = 0.0253 eV`, so the lookup uses `α·kT/kT₀`, `β·kT/kT₀`.
//!
//! Integrating over angle gives `σ(E→E')`; integrating that over `E'` gives the
//! incoherent-inelastic cross section `σ_inel(E)`. This module computes all three.
//!
//! ## Numerics
//!
//! Fixed-grid quadratures (dense trapezoid over μ and a kinematically-bounded E'
//! grid) — robust and adequate for the cross section. NJOY refines both grids
//! adaptively to a tolerance; that refinement (and the liquid `cliq` small-α
//! correction) are future accuracy work, noted where they apply.

use super::mf7::{interp_s_temperature, AlphaTable, IncoherentInelastic};
use crate::common::phys::BK_EV_PER_K;

/// Reference temperature kT₀ = 0.0253 eV (`therm`/`tevz` in `thermr.f90`).
const TEVZ: f64 = 0.0253;

/// One equally-probable outgoing-energy bin of a thermal inelastic emission:
/// an outgoing energy and its equally-probable scattering cosines. This is the
/// per-bin content of the ACE ITXE block (IFENG=0).
#[derive(Debug, Clone)]
pub struct OutgoingBin {
    /// Representative outgoing neutron energy \[eV\] for this equiprobable bin.
    pub e_out_ev: f64,
    /// Equally-probable scattering cosines (ascending), length `nang`.
    pub cosines: Vec<f64>,
}

/// Invert a monotone CDF: find `x` where the cumulative `cum` reaches `target`,
/// by linear interpolation. `x` and `cum` are the same length and ascending.
fn invert_cdf(x: &[f64], cum: &[f64], target: f64) -> f64 {
    match cum.iter().position(|&c| c >= target) {
        None => x[x.len() - 1],
        Some(0) => x[0],
        Some(i) => {
            let (c0, c1) = (cum[i - 1], cum[i]);
            if (c1 - c0).abs() < 1e-30 {
                x[i]
            } else {
                x[i - 1] + (target - c0) / (c1 - c0) * (x[i] - x[i - 1])
            }
        }
    }
}

/// `ln S` floor for `S = 0` table entries (`sabflg` in `thermr.f90`).
const SABFLG: f64 = -225.0;

impl IncoherentInelastic {
    /// Mass ratio `A` of the principal scatterer (`B(3)`).
    pub fn mass_ratio(&self) -> f64 {
        self.b[2]
    }

    /// Bound scattering cross section `σ_b` \[barn\] of the principal scatterer:
    /// `(B(1)/natom)·((A+1)/A)²`. `natom` is the number of principal atoms in the
    /// material (`B(6)` records it; pass `1.0` for a monatomic scatterer).
    pub fn sigma_bound(&self, natom: f64) -> f64 {
        let a = self.mass_ratio();
        (self.b[0] / natom) * ((a + 1.0) / a).powi(2)
    }

    /// Principal-scatterer effective temperature `T_eff` \[eV\] at physical
    /// temperature `temp_k` \[K\], linearly interpolated from the MF=7/MT=4
    /// effective-temperature table ([`teff_table`]). Falls back to the physical
    /// temperature (pure free-gas SCT) when the evaluation omits the table.
    ///
    /// `T_eff ≥ T` (Egelstaff–Schofield): it carries the mean squared
    /// vibrational speed of the bound atom, so the short-collision-time kernel
    /// reproduces the free-gas second energy moment at high incident energy.
    ///
    /// [`teff_table`]: super::mf7::IncoherentInelastic::teff_table
    pub fn teff_ev(&self, temp_k: f64) -> f64 {
        let teff_k = if self.teff_table.is_empty() {
            temp_k
        } else {
            interp_linear(&self.teff_table, temp_k)
        };
        BK_EV_PER_K * teff_k
    }

    /// The double-differential cross section `d²σ/dE'dμ` \[barn\] for scattering
    /// from incident energy `e` \[eV\] to outgoing `ep` \[eV\] through cosine `mu`,
    /// at temperature `temp_k` \[K\].
    ///
    /// Inside the tabulated `(α,β)` grid the kernel comes from the interpolated
    /// scattering law `S̃(α,β)`; **beyond** the grid (large momentum/energy
    /// transfer, reached at higher incident energy) it comes from the
    /// short-collision-time (SCT) analytic kernel — this is what carries `σ(E)`
    /// back to the free-gas limit at high `E` instead of falling to zero at the
    /// edge of the table (thermr.f90 `sig`, tabulated branch + label 170).
    ///
    /// At an interpolated temperature ([`temperature_bracket`] present) the
    /// kernel is evaluated at each bracketing tabulated temperature — that
    /// temperature's own `S(α,β)`, kinematics and `T_eff` — and the two
    /// results are interpolated in `T` with the evaluation's `LI` law (see
    /// the `mf7` module docs, policy step 2). `temp_k` is then the
    /// interpolation abscissa; passing the struct's own
    /// [`temperature_k`](super::mf7::IncoherentInelastic::temperature_k) is
    /// the intended use.
    ///
    /// [`temperature_bracket`]: super::mf7::IncoherentInelastic::temperature_bracket
    pub fn double_differential(&self, e: f64, ep: f64, mu: f64, temp_k: f64, natom: f64) -> f64 {
        match &self.temperature_bracket {
            None => self.double_differential_with(&self.s_tables, e, ep, mu, temp_k, natom),
            Some(br) => {
                let k_lo = self.double_differential_with(&br.s_lo, e, ep, mu, br.t_lo_k, natom);
                let k_hi = self.double_differential_with(&br.s_hi, e, ep, mu, br.t_hi_k, natom);
                interp_s_temperature(br.t_lo_k, k_lo, br.t_hi_k, k_hi, temp_k, br.li).max(0.0)
            }
        }
    }

    /// [`double_differential`](Self::double_differential) against an explicit
    /// set of `S(α)` tables at their own temperature `temp_k` — the single-
    /// temperature worker the public method dispatches to.
    fn double_differential_with(
        &self,
        tables: &[AlphaTable],
        e: f64,
        ep: f64,
        mu: f64,
        temp_k: f64,
        natom: f64,
    ) -> f64 {
        if e <= 0.0 || ep <= 0.0 {
            return 0.0;
        }
        let tev = BK_EV_PER_K * temp_k;
        let a_mass = self.mass_ratio();

        let beta = (ep - e) / tev; // signed β (physical)
        let alpha = (e + ep - 2.0 * mu * (e * ep).sqrt()) / (a_mass * tev); // physical α

        // LAT=1: the table is stored at kT₀, so scale the lookup variables.
        let (a_tab, b_tab) = if self.lat == 1 {
            (alpha * tev / TEVZ, beta.abs() * tev / TEVZ)
        } else {
            (alpha, beta.abs())
        };

        // Outside the tabulated grid → SCT (the free-gas-limit tail).
        let alpha_max = tables
            .first()
            .and_then(|t| t.alpha.last())
            .copied()
            .unwrap_or(0.0);
        let beta_max = self.beta.last().copied().unwrap_or(0.0);
        if a_tab > alpha_max || b_tab > beta_max {
            return self.sct_double_differential(e, ep, mu, temp_k, natom);
        }

        // NJOY's liquid small-α branch (`thermr.f90` `sig`, the test just before
        // label 150). Below the first tabulated α a liquid's S(α,β) follows the
        // diffusive limit rather than the tabulated curve, and NJOY substitutes
        // the whole (α,β) evaluation with
        //
        //   s = sab(1,1) + ln(α₁/α)/2 − cliq·β²/α
        //
        // under four conditions: `cliq ≠ 0`, `α < α₁`, `LASYM = 0`, and `|β| ≤ 0.2`.
        if let Some(s) = self.liquid_small_alpha_ln_s(tables, a_tab, b_tab) {
            let arg = s - beta / 2.0;
            if arg > 20.0 {
                return 0.0;
            }
            let sigc = (ep / e).sqrt() / (2.0 * tev);
            let sig = sigc * self.sigma_bound(natom) * arg.exp();
            return if sig.is_finite() && sig > 0.0 {
                sig
            } else {
                0.0
            };
        }
        // THE FLOOR CHECK IS GATED ON `test2`, AND COMES BEFORE THE INTERPOLATION.
        //
        // `thermr.f90`'s `sig` at label 150 reads:
        //
        //   150: if (a*az.lt.test2 .and. b.lt.test2) go to 155   ! test2 = 30
        //        if (sab(ia,ib)  .le.sabflg) go to 170           ! any floored
        //        if (sab(ia+1,ib).le.sabflg) go to 170           ! corner of the
        //        if (sab(ia,ib+1).le.sabflg) go to 170           ! 2x2 bracket
        //        if (sab(ia+1,ib+1).le.sabflg) go to 170         ! sends it to SCT
        //   155: <interpolate>
        //
        // So NJOY only abandons the table for the short-collision-time kernel
        // where the transfer is already large — `α·A ≥ 30` or `β ≥ 30`. Inside
        // that box it interpolates whatever the table holds, floors included.
        //
        // This port had no `test2` guard and tested the floor *after*
        // interpolating, so it substituted SCT in a region NJOY interpolates.
        // That is a Gaussian tail where the evaluation says essentially nothing,
        // i.e. weight added where NJOY has none. See GitHub #188.
        if self.floored_corner(tables, a_tab, b_tab) {
            return self.sct_double_differential(e, ep, mu, temp_k, natom);
        }
        let ln_s = self.interp_ln_s(tables, a_tab, b_tab);
        // ln of the detailed-balance factor S̃·exp(−β/2). A large positive value
        // means deep downscatter into the many-phonon tail where S̃ has fallen
        // below the numerical floor — the physical product is negligible, but
        // exp(−β/2) would overflow first, so treat it as zero.
        let arg = ln_s - beta / 2.0;
        if arg > 20.0 {
            return 0.0;
        }
        let sigc = (ep / e).sqrt() / (2.0 * tev);
        let sig = sigc * self.sigma_bound(natom) * arg.exp();
        if sig.is_finite() && sig > 0.0 {
            sig
        } else {
            0.0
        }
    }

    /// Convergence tolerance for the adaptive `E'` linearisation in
    /// [`Self::ep_profile`] — thermr.f90 `sigl`'s `tol`, which is half the user's
    /// `tolin` (THERMR's default reconstruction tolerance is 0.5 %, so 0.25 %;
    /// 1e-3 here is tighter and costs ~4 % more grid points).
    const EP_REFINE_TOL: f64 = 1.0e-3;

    /// Bisection depth cap for the same loop. Upstream bounds its stack at
    /// `imax = 20` points per interval rather than by depth; 8 levels is 256
    /// subdivisions of one β interval, well past where the tolerance stops it on
    /// every case measured, and it guarantees termination if σ is pathological.
    const EP_REFINE_MAX_DEPTH: u32 = 8;

    /// Short-collision-time (SCT) double-differential kernel `d²σ/dE'dμ` \[barn\]
    /// for `(α,β)` beyond the tabulated grid — a faithful port of thermr.f90
    /// `sig` label 170. The principal scatterer's bound cross section `σ_b` and
    /// effective temperature `T_eff` set a Gaussian in `(α−|β|)` that reproduces
    /// the free-gas kernel as `T_eff → T`:
    ///
    /// ```text
    ///   S_sct(α,β) = exp[ −(α−|β|)²·T/(4·α·T_eff) − (|β|+β)/2 ] / √(4π·α·T_eff/T)
    ///   d²σ/dE'dμ  = √(E'/E)/(2kT) · σ_b · S_sct(α,β)
    /// ```
    ///
    /// A tabulated *secondary* SCT scatterer (`B(7)=0`, e.g. some polyethylene
    /// evaluations) would add a second such term with its own `σ_b2`, mass ratio
    /// and `T_eff2`; H-in-H₂O sets `B(7)=1` (free-gas oxygen, handled outside
    /// THERMR) so `σ_b2 = 0` and only the principal term contributes. Because the
    /// secondary `T_eff2` table is not retained, a nonzero secondary term is
    /// skipped with its share dropped — see the module caveats.
    fn sct_double_differential(&self, e: f64, ep: f64, mu: f64, temp_k: f64, natom: f64) -> f64 {
        const AMIN: f64 = 1.0e-6;
        let tev = BK_EV_PER_K * temp_k;
        let a_mass = self.mass_ratio();
        let rtev = 1.0 / tev;
        let bb = (ep - e) * rtev; // signed β
        let mut a = (e + ep - 2.0 * mu * (e * ep).sqrt()) / (a_mass * tev); // physical α
        if a < AMIN {
            a = AMIN;
        }
        let b = bb.abs(); // |β|
        let sigc = (ep / e).sqrt() * rtev / 2.0;
        let c = (4.0 * std::f64::consts::PI).sqrt();

        // Principal scatterer (thermr.f90 lines 2584–2587).
        let teff = self.teff_ev(temp_k); // T_eff in eV
        let arg = (a - b).powi(2) * tev / (4.0 * a * teff) + (b + bb) / 2.0;
        let mut sig = 0.0;
        if -arg > SABFLG {
            let s = (-arg).exp() / (c * (a * teff * rtev).sqrt());
            sig += sigc * self.sigma_bound(natom) * s;
        }
        if sig.is_finite() && sig > 0.0 {
            sig
        } else {
            0.0
        }
    }

    /// `σ(E→E')` \[barn\]: the double-differential integrated over cosine `μ`.
    pub fn sigma_e_to_ep(&self, e: f64, ep: f64, temp_k: f64, natom: f64) -> f64 {
        // Dense trapezoid over μ ∈ [−1, 1]; the integrand is smooth but can peak
        // near the μ that minimises α, so use a fine grid.
        const NMU: usize = 200;
        let mut sum = 0.0;
        let mut prev = self.double_differential(e, ep, -1.0, temp_k, natom);
        for k in 1..=NMU {
            let mu = -1.0 + 2.0 * k as f64 / NMU as f64;
            let cur = self.double_differential(e, ep, mu, temp_k, natom);
            sum += 0.5 * (cur + prev) * (2.0 / NMU as f64);
            prev = cur;
        }
        sum
    }

    /// The incoherent-inelastic cross section `σ_inel(E)` \[barn\] at incident
    /// energy `e` \[eV\] and temperature `temp_k` \[K\]: `σ(E→E')` integrated over
    /// the kinematically-allowed outgoing energies `E'`.
    ///
    /// At an interpolated temperature ([`temperature_bracket`] present) the
    /// integral is formed at each bracketing tabulated temperature — its own
    /// `S(α,β)`, kinematics and `T_eff` — and the two values are interpolated
    /// in `T` with the evaluation's `LI` law, so the result lies between
    /// `σ_inel(E,T_lo)` and `σ_inel(E,T_hi)` by construction (`op-55lj`: the
    /// fixed-`(α,β)` interpolant integrated with the target temperature's
    /// kinematics fell 4 % below its bracket at 3.9 eV on graphite).
    ///
    /// [`temperature_bracket`]: super::mf7::IncoherentInelastic::temperature_bracket
    pub fn cross_section(&self, e: f64, temp_k: f64, natom: f64) -> f64 {
        let integrate = |tables: &[AlphaTable], t: f64| {
            let (eps, sig) = self.sigma_ep_profile_with(tables, e, t, natom);
            let mut sum = 0.0;
            for k in 1..eps.len() {
                sum += 0.5 * (sig[k] + sig[k - 1]) * (eps[k] - eps[k - 1]);
            }
            sum
        };
        match &self.temperature_bracket {
            None => integrate(&self.s_tables, temp_k),
            Some(br) => {
                let s_lo = integrate(&br.s_lo, br.t_lo_k);
                let s_hi = integrate(&br.s_hi, br.t_hi_k);
                interp_s_temperature(br.t_lo_k, s_lo, br.t_hi_k, s_hi, temp_k, br.li).max(0.0)
            }
        }
    }

    /// The outgoing-energy profile `(E'[], σ(E→E')[])` used to build the
    /// equiprobable emission bins — through the (bracket-aware)
    /// [`double_differential`](Self::double_differential), so the emission
    /// distribution at an interpolated temperature is the same `LI`
    /// interpolation of the two bracketing kernels, pointwise.
    fn sigma_ep_profile(&self, e: f64, temp_k: f64, natom: f64) -> (Vec<f64>, Vec<f64>) {
        self.ep_profile(e, temp_k, |ep| self.sigma_e_to_ep(e, ep, temp_k, natom))
    }

    /// [`sigma_ep_profile`](Self::sigma_ep_profile) against one explicit set
    /// of `S(α)` tables at their own temperature — the single-temperature
    /// worker [`cross_section`](Self::cross_section) integrates at each end
    /// of a temperature bracket.
    fn sigma_ep_profile_with(
        &self,
        tables: &[AlphaTable],
        e: f64,
        temp_k: f64,
        natom: f64,
    ) -> (Vec<f64>, Vec<f64>) {
        const NMU: usize = 200;
        self.ep_profile(e, temp_k, |ep| {
            // Same dense μ trapezoid as `sigma_e_to_ep`, on these tables.
            let mut sum = 0.0;
            let mut prev = self.double_differential_with(tables, e, ep, -1.0, temp_k, natom);
            for k in 1..=NMU {
                let mu = -1.0 + 2.0 * k as f64 / NMU as f64;
                let cur = self.double_differential_with(tables, e, ep, mu, temp_k, natom);
                sum += 0.5 * (cur + prev) * (2.0 / NMU as f64);
                prev = cur;
            }
            sum
        })
    }

    /// The outgoing-energy grid for incident energy `e` with `sigma(E')`
    /// evaluated on it.
    ///
    /// The E' grid is the table's own β grid mapped to both scatter directions
    /// (`|E'−E| = β·D`, `D = kT₀` for LAT=1 else kT) — the points where S(α,β) has
    /// structure. A naive uniform dE' grid wastes all its resolution on the empty
    /// high-energy tail.
    ///
    /// # The raw β map is not accurate enough at thermal energies — it is refined
    ///
    /// The β grid maps to an E' grid that is far too coarse where the kernel is
    /// narrow, i.e. at thermal incident energies. Measured against a kernel whose
    /// answer is known in closed form (the **analytic free gas**, so the
    /// S(α,β) evaluation plays no part), using graphite's own 400-point β grid at
    /// 600 K:
    ///
    /// ```text
    ///   E [eV]     <E'> vs exact   shape <E'2>/<E'>2 vs exact   grid points
    ///   0.0253       +1.047 %              -0.369 %                 425
    ///   0.1          +0.365 %              -0.041 %                 499
    ///   0.5          +0.090 %              -0.001 %                 656
    ///   1.0          +0.047 %              +0.000 %                 699
    /// ```
    ///
    /// Both signs are the ones GitHub #188 complains of — mean too **high**,
    /// width too **narrow** — and because this is a property of the *grid* it is
    /// present on every evaluation, which is the other thing #188 observes.
    ///
    /// So the grid is now **adaptively refined**, the way `calcem`/`sigl` linearise
    /// (thermr.f90 label 110: bisect while
    /// `|σ(x_m) − chord| > tol·|σ(x_m)| + tol·σ_max/50`). On the same oracle that
    /// removes the error entirely, for 4 % more points:
    ///
    /// ```text
    ///   E [eV]     <E'> vs exact   shape vs exact   grid points
    ///   0.0253       -0.003 %         +0.006 %          442
    ///   0.1          -0.006 %         +0.002 %          510
    ///   0.5          +0.000 %         +0.000 %          700
    ///   1.0          +0.000 %         +0.000 %          854
    /// ```
    ///
    /// # This fixes a real grid error and does NOT fix GitHub #188 — measured
    ///
    /// The refinement above is correct and it changes the #188 symptom by
    /// **nothing**. Graphite's stationary-distribution fixed point
    /// (`outram-mc-libs/tests/thermal_kernel_stationary_distribution.rs`, the
    /// detailed-balance oracle) reads **604.83 K / shape 1.6424** with the
    /// refinement against **604.76 K / 1.6425** without it — 0.06 sigma on a
    /// shape whose standard error is 0.0016, while the deficit being chased is
    /// 1.46 %. Water likewise: 294.62 K / 1.6397 against 294.62 K / 1.6394.
    ///
    /// The refinement is verified to fire, so this is a null result and not an
    /// inert change: graphite's grid at 600 K goes from **400 raw points to 742**
    /// refined. A 0.37 % per-energy trapezoid bias simply does not survive into
    /// the relaxed distribution, which is worth knowing — it means the two are
    /// not the same measurement and a per-energy oracle cannot stand in for the
    /// fixed point.
    ///
    /// It is kept because it is right, not because it helped: the grid was
    /// provably losing width against a closed-form kernel, and leaving a known
    /// bias in place because its downstream effect is small is how the next
    /// search gets confounded.
    ///
    /// So the earlier refutations stand and this joins them. Uniformly
    /// subdividing every E' interval 4× moved graphite's width from −1.96 % to
    /// −2.01 % and H₂O's from −4.02 % to −3.85 %; `NMU` 200 → 800 moved graphite
    /// not at all; adaptive refinement moves the fixed point by 0.06 sigma. **It
    /// is not the E' grid, it is not the μ quadrature, and it is not the E'
    /// linearisation.** The remainder is in the kernel evaluation itself — the
    /// S(α,β) interpolation, its β-axis scheme, or the small-α extension.
    fn ep_profile(&self, e: f64, temp_k: f64, sigma: impl Fn(f64) -> f64) -> (Vec<f64>, Vec<f64>) {
        if e <= 0.0 || self.beta.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let tev = BK_EV_PER_K * temp_k;
        let d = if self.lat == 1 { TEVZ } else { tev };
        let mut eps: Vec<f64> = Vec::with_capacity(2 * self.beta.len() + 1);
        eps.push(e); // quasi-elastic point β = 0
        for &bj in &self.beta {
            let down = e - bj * d;
            if down > 1.0e-5 {
                eps.push(down);
            }
            eps.push(e + bj * d);
        }
        eps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eps.dedup_by(|a, b| (*a - *b).abs() < 1e-10 * b.abs().max(1.0));

        // Adaptive linearisation, ported from thermr.f90's `sigl` label 110: an
        // interval is bisected while the midpoint's true value differs from the
        // chord by more than `tol·|σ(x_m)| + tol·σ_max/50`. The absolute term is
        // what stops the test from chasing relative accuracy far out in a tail
        // where σ is already negligible; it is upstream's, not an invention here.
        let mut sig: Vec<f64> = eps.iter().map(|&ep| sigma(ep)).collect();
        let smax = sig.iter().copied().fold(0.0_f64, f64::max);
        if smax > 0.0 {
            let mut ref_eps: Vec<f64> = Vec::with_capacity(eps.len() * 2);
            let mut ref_sig: Vec<f64> = Vec::with_capacity(eps.len() * 2);
            for k in 1..eps.len() {
                ref_eps.push(eps[k - 1]);
                ref_sig.push(sig[k - 1]);
                let mut add: Vec<(f64, f64)> = Vec::new();
                let mut stack = vec![(eps[k - 1], sig[k - 1], eps[k], sig[k], 0u32)];
                while let Some((x0, y0, x1, y1, depth)) = stack.pop() {
                    if depth >= Self::EP_REFINE_MAX_DEPTH {
                        continue;
                    }
                    let xm = 0.5 * (x0 + x1);
                    if xm <= x0 || xm >= x1 {
                        continue; // exhausted f64 resolution
                    }
                    let ym = sigma(xm);
                    let chord = 0.5 * (y0 + y1);
                    if (ym - chord).abs() > Self::EP_REFINE_TOL * (ym.abs() + smax / 50.0) {
                        add.push((xm, ym));
                        stack.push((x0, y0, xm, ym, depth + 1));
                        stack.push((xm, ym, x1, y1, depth + 1));
                    }
                }
                add.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for (x, y) in add {
                    ref_eps.push(x);
                    ref_sig.push(y);
                }
            }
            ref_eps.push(eps[eps.len() - 1]);
            ref_sig.push(sig[eps.len() - 1]);
            eps = ref_eps;
            sig = ref_sig;
        }
        (eps, sig)
    }

    /// Build the ACE thermal inelastic emission for incident energy `e` \[eV\]:
    /// `nieb` equally-probable outgoing energies, each with `nang` equally-probable
    /// scattering cosines. This is the ITXE-block data (IFENG=0), the equiprobable
    /// form `sigl`/`calcem` produce.
    ///
    /// Returns an empty vector when the cross section is zero (no emission).
    pub fn equiprobable_emission(
        &self,
        e: f64,
        temp_k: f64,
        natom: f64,
        nieb: usize,
        nang: usize,
    ) -> Vec<OutgoingBin> {
        let (eps, sig) = self.sigma_ep_profile(e, temp_k, natom);
        if eps.len() < 2 {
            return Vec::new();
        }
        // Cumulative ∫σ(E→E')dE'.
        let mut cum = vec![0.0f64; eps.len()];
        for k in 1..eps.len() {
            cum[k] = cum[k - 1] + 0.5 * (sig[k] + sig[k - 1]) * (eps[k] - eps[k - 1]);
        }
        let total = cum[eps.len() - 1];
        if total <= 0.0 {
            return Vec::new();
        }
        // Equally-probable E' at the bin midpoints (k−0.5)/nieb of the CDF.
        (0..nieb)
            .map(|k| {
                let target = (k as f64 + 0.5) / nieb as f64 * total;
                let ep = invert_cdf(&eps, &cum, target);
                let cosines = self.equiprobable_cosines(e, ep, temp_k, natom, nang);
                OutgoingBin {
                    e_out_ev: ep,
                    cosines,
                }
            })
            .collect()
    }

    /// `nang` equally-probable scattering cosines for scattering `e → ep` at
    /// temperature `temp_k` — the CDF of the angular distribution inverted at the
    /// bin midpoints. Falls back to a uniform spread when the angular distribution
    /// integrates to zero.
    fn equiprobable_cosines(
        &self,
        e: f64,
        ep: f64,
        temp_k: f64,
        natom: f64,
        nang: usize,
    ) -> Vec<f64> {
        const NMU: usize = 200;
        let mu: Vec<f64> = (0..=NMU)
            .map(|k| -1.0 + 2.0 * k as f64 / NMU as f64)
            .collect();
        let f: Vec<f64> = mu
            .iter()
            .map(|&m| self.double_differential(e, ep, m, temp_k, natom))
            .collect();
        let mut cum = vec![0.0f64; mu.len()];
        for k in 1..mu.len() {
            cum[k] = cum[k - 1] + 0.5 * (f[k] + f[k - 1]) * (mu[k] - mu[k - 1]);
        }
        let total = cum[mu.len() - 1];
        if total <= 0.0 {
            // Isotropic fallback: uniform cosines at the bin midpoints.
            return (0..nang)
                .map(|j| -1.0 + 2.0 * (j as f64 + 0.5) / nang as f64)
                .collect();
        }
        (0..nang)
            .map(|j| {
                let target = (j as f64 + 0.5) / nang as f64 * total;
                invert_cdf(&mu, &cum, target).clamp(-1.0, 1.0)
            })
            .collect()
    }

    /// Interpolate `ln S̃(α, β)` at table coordinates `(a, b)` (both ≥ 0). Bilinear
    /// in `ln S` over the `(α, β)` grid; returns [`SABFLG`] (⇒ `S ≈ 0`) outside the
    /// tabulated range.
    /// True when NJOY would abandon the tabulated `S(α,β)` for the
    /// short-collision-time kernel because a corner of the interpolation bracket
    /// is floored — **and** the transfer is large enough for that test to apply.
    ///
    /// `thermr.f90` `sig`, label 150: the four-corner floor test is skipped
    /// entirely while `α·A < test2` and `|β| < test2` with `test2 = 30`. Inside
    /// that box the table is interpolated whatever it holds.
    ///
    /// `a` and `b` are the *table* variables (already scaled by `kT₀/kT` when
    /// `LAT = 1`), matching how they are used for the bracket search; the `α·A`
    /// in NJOY's test is the physical α times the mass ratio, which is why the
    /// mass ratio appears here rather than in the caller.
    fn floored_corner(&self, tables: &[AlphaTable], a: f64, b: f64) -> bool {
        /// NJOY's `test2`.
        const TEST2: f64 = 30.0;
        if a * self.mass_ratio() < TEST2 && b < TEST2 {
            return false;
        }
        let nb = self.beta.len();
        if nb == 0 || tables.is_empty() {
            return false;
        }
        let ib = match self.beta.iter().position(|&bj| b < bj) {
            Some(0) => 0,
            Some(j) => j - 1,
            None => nb - 1,
        };
        let ib1 = (ib + 1).min(nb - 1);
        let alpha = &tables[ib].alpha;
        let na = alpha.len();
        if na == 0 {
            return false;
        }
        let ia = match alpha.iter().position(|&aj| a < aj) {
            Some(0) => 0,
            Some(i) => i - 1,
            None => na - 1,
        };
        let ia1 = (ia + 1).min(na - 1);
        let floored = |t: &AlphaTable, i: usize| {
            let v = t.s.get(i).copied().unwrap_or(0.0);
            !(v > 0.0) || v.ln() <= SABFLG
        };
        floored(&tables[ib], ia)
            || floored(&tables[ib], ia1)
            || floored(&tables[ib1], ia)
            || floored(&tables[ib1], ia1)
    }

    /// **NJOY's liquid small-α form**, or `None` when its conditions do not hold.
    ///
    /// A port of the branch in `thermr.f90`'s `sig` immediately before label 150:
    ///
    /// ```text
    ///   if (cliq.eq.zero.or.a.ge.alpha(1)) go to 150
    ///   if (lasym.eq.1) go to 150
    ///   if (b.gt.test1) go to 150            ! test1 = 0.2
    ///   s = sab(1,1) + log(alpha(1)/a)/2 - cliq*b**2/a
    /// ```
    ///
    /// The `ln(α₁/α)/2` term is the `1/√α` divergence of a diffusing liquid, and
    /// `cliq` is the diffusion coefficient NJOY derives **from the table itself**
    /// rather than from any flag (`calcem`):
    ///
    /// ```text
    ///   cliq = 0
    ///   if (sab(1,1).gt.sab(2,1)) cliq = (sab(1,1)-sab(1,2))*alpha(1)/beta(2)**2
    /// ```
    ///
    /// So the test for "is this a liquid" is whether `ln S` *falls* with α at the
    /// first node — which is a property of the evaluation and can be true for
    /// materials nobody would call liquids. `sab` holds `ln S`, and the indices
    /// are `sab(α-index, β-index)`.
    ///
    /// # Why it was missing, and why it may matter more for graphite than water
    ///
    /// `interp_alpha_ln_s` used to clamp below the first α node, with a comment
    /// saying NJOY added a liquid correction here. It does, and this is it. The
    /// corner it governs is `α < α₁`, whose size is set by the evaluation's own
    /// grid: `tsl-HinH2O` starts at `α₁ = 1.0023e-5`, but
    /// `tsl-crystalline-graphite` starts at `α₁ = 3.322e-3`, **300 times larger**,
    /// so the region this branch covers is far bigger for graphite. GitHub #188.
    fn liquid_small_alpha_ln_s(&self, tables: &[AlphaTable], a: f64, b: f64) -> Option<f64> {
        /// NJOY's `test1`: above this `|β|` the diffusive form no longer applies.
        const TEST1: f64 = 0.2;
        if self.lasym == 1 || a >= tables.first()?.alpha.first().copied()? {
            return None;
        }
        if b > TEST1 {
            return None;
        }
        let t0 = tables.first()?;
        let t1 = tables.get(1)?;
        if t0.alpha.len() < 2 || self.beta.len() < 2 {
            return None;
        }
        let ln = |v: f64| if v > 0.0 { v.ln() } else { SABFLG };
        let (s11, s21, s12) = (ln(t0.s[0]), ln(t0.s[1]), ln(t1.s[0]));
        if !(s11 > s21) {
            return None; // cliq = 0: not the diffusive case
        }
        let alpha1 = t0.alpha[0];
        let beta2 = self.beta[1];
        if !(beta2.abs() > 0.0) || !(a > 0.0) {
            return None;
        }
        let cliq = (s11 - s12) * alpha1 / (beta2 * beta2);
        let s = s11 + (alpha1 / a).ln() / 2.0 - cliq * b * b / a;
        Some(s.max(SABFLG))
    }

    fn interp_ln_s(&self, tables: &[AlphaTable], a: f64, b: f64) -> f64 {
        let beta = &self.beta;
        if beta.is_empty() || b > *beta.last().unwrap() {
            return SABFLG;
        }
        // β bracket.
        let jb = match beta.iter().position(|&bj| b < bj) {
            Some(0) => 0,
            Some(j) => j - 1,
            None => beta.len() - 1,
        };
        let jb1 = (jb + 1).min(beta.len() - 1);

        // NJOY's `sig` evaluates α with `terpq` at THREE consecutive β slices and
        // then runs `terpq` again across β on those three results. Two points on
        // either axis is a chord across a curved `ln S`, which is what GitHub #188
        // measured as a one-signed narrow kernel on all eight evaluations.
        let n = beta.len();
        if n < 3 {
            let s_lo = interp_alpha_ln_s(&tables[jb], a);
            if jb1 == jb {
                return s_lo;
            }
            let s_hi = interp_alpha_ln_s(&tables[jb1], a);
            let (b0, b1) = (beta[jb], beta[jb1]);
            if (b1 - b0).abs() < 1e-30 {
                return s_lo;
            }
            return terp1_lin_lin(b0, s_lo, b1, s_hi, b).max(SABFLG);
        }
        let jb = if jb + 2 >= n { n - 3 } else { jb };
        let s1 = interp_alpha_ln_s(&tables[jb], a);
        let s2 = interp_alpha_ln_s(&tables[jb + 1], a);
        let s3 = interp_alpha_ln_s(&tables[jb + 2], a);
        terpq(beta[jb], s1, beta[jb + 1], s2, beta[jb + 2], s3, b)
    }
}

/// Linear interpolation of a `(x, y)` table at `x = arg`, clamped to the end
/// values outside the tabulated range. Used for the `T_eff(T)` table.
fn interp_linear(table: &[(f64, f64)], arg: f64) -> f64 {
    match table {
        [] => arg,
        [only] => only.1,
        _ => {
            if arg <= table[0].0 {
                return table[0].1;
            }
            if arg >= table[table.len() - 1].0 {
                return table[table.len() - 1].1;
            }
            let i = table.partition_point(|&(x, _)| x <= arg) - 1;
            let (x0, y0) = table[i];
            let (x1, y1) = table[i + 1];
            if (x1 - x0).abs() < 1e-30 {
                y0
            } else {
                y0 + (arg - x0) / (x1 - x0) * (y1 - y0)
            }
        }
    }
}

/// ENDF interpolation law 2 (lin-lin) on `(x, y)` — here `y` is always `ln S`.
fn terp1_lin_lin(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if (x2 - x1).abs() < 1.0e-30 {
        return y1;
    }
    y1 + (y2 - y1) * (x - x1) / (x2 - x1)
}

/// ENDF interpolation law 3 (lin-log): `y` linear in `ln x`.
fn terp1_lin_log(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if !(x > 0.0 && x1 > 0.0 && x2 > 0.0) {
        return terp1_lin_lin(x1, y1, x2, y2, x);
    }
    let (l1, l2, lx) = (x1.ln(), x2.ln(), x.ln());
    if (l2 - l1).abs() < 1.0e-30 {
        return y1;
    }
    y1 + (y2 - y1) * (lx - l1) / (l2 - l1)
}

/// **`terpq` — the three-point interpolation NJOY uses for `ln S(α,β)` on both
/// axes.**
///
/// This is a port of NJOY2016 `src/thermr.f90`'s `terpq`, which its own header
/// describes as: *"Compute y(x) by quadratic interpolation, except use log-lin if
/// x.lt.x1 and lin-lin if x.gt.x3, and use lin-lin if the function takes big
/// steps (corners)."* `y` is `ln S` throughout, so "lin-lin" means linear in
/// `ln S`.
///
/// The four branches, in NJOY's order:
///
/// 1. `x < x1` — below the stencil. Clamp to `y1` if the function is *rising*
///    toward small `x` (`y1 > y2`), else extrapolate with law 3 (`ln S` linear in
///    `ln x`).
/// 2. `x > x3` — above the stencil. Clamp to `y3` if `y3 > y2`, else extrapolate
///    lin-lin off the upper pair.
/// 3. **Corner detection.** If `ln S` steps by more than `STEP = 2` across either
///    interval the surface has an edge there and a parabola through it will
///    overshoot, so fall back to lin-lin on the bracketing pair.
/// 4. Otherwise the parabola through all three points.
///
/// Finally the result is floored at [`SABFLG`].
///
/// # Why this replaced two-point linear interpolation (GitHub #188)
///
/// This port interpolated `ln S` **linearly between two points** on each axis,
/// which matches the `INT = 4` (log-lin) law both evaluations declare and is
/// therefore not *wrong* in the ENDF sense — but it is not what NJOY does, and
/// the comparison being made is against NJOY. `ln S` is curved in both α and β,
/// and a chord across a curved function misses it one-signed, which is what the
/// fixed-point sweep measured: every one of the eight S(α,β) evaluations in
/// `reference-data/endf/` came out **narrow, by 0.5 % to 2.3 %**, and materials
/// with nothing physical in common cannot share a defect by coincidence.
fn terpq(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x: f64) -> f64 {
    /// NJOY's `step`: a jump larger than this in `ln S` is a corner, not curvature.
    const STEP: f64 = 2.0;
    let y = if x < x1 {
        if y1 > y2 {
            y1
        } else {
            terp1_lin_log(x1, y1, x2, y2, x)
        }
    } else if x > x3 {
        if y3 > y2 {
            y3
        } else {
            terp1_lin_lin(x2, y2, x3, y3, x)
        }
    } else if (y1 - y2).abs() > STEP || (y2 - y3).abs() > STEP {
        if x < x2 {
            terp1_lin_lin(x1, y1, x2, y2, x)
        } else {
            terp1_lin_lin(x2, y2, x3, y3, x)
        }
    } else {
        let d21 = x2 - x1;
        let d31 = x3 - x1;
        let d32 = x3 - x2;
        if d21.abs() < 1.0e-30 || d32.abs() < 1.0e-30 || d31.abs() < 1.0e-30 {
            return terp1_lin_lin(x1, y1, x2, y2, x).max(SABFLG);
        }
        let b = (y2 - y1) * d31 / (d21 * d32) - (y3 - y1) * d21 / (d31 * d32);
        let c = (y3 - y1) / (d31 * d32) - (y2 - y1) / (d21 * d32);
        let dx = x - x1;
        y1 + b * dx + c * dx * dx
    };
    y.max(SABFLG)
}

/// Interpolate `ln S(α)` within one `β` slice at momentum transfer `a`, with
/// NJOY's three-point [`terpq`] over the `α` grid.
///
/// Returns [`SABFLG`] at or above the top of the grid, matching the caller, which
/// routes `α > α_max` to the short-collision-time kernel before this is reached.
/// Below the first tabulated α the stencil's own law-3 extrapolation applies —
/// NJOY additionally has a *liquid* small-α correction there
/// (`s = sab(1,1) + log(alpha(1)/a)/2 - cliq*b**2/a` when `cliq ≠ 0`), which this
/// port does not yet carry; for `tsl-HinH2O` the first α node is `1.0023e-5`, so
/// that corner is vanishingly small.
fn interp_alpha_ln_s(table: &AlphaTable, a: f64) -> f64 {
    let alpha = &table.alpha;
    let s = &table.s;
    let n = alpha.len();
    if n == 0 {
        return SABFLG;
    }
    let ln = |v: f64| if v > 0.0 { v.ln() } else { SABFLG };
    if a >= alpha[n - 1] {
        return SABFLG;
    }
    if n < 3 {
        if a <= alpha[0] {
            return ln(s[0]);
        }
        return terp1_lin_lin(alpha[0], ln(s[0]), alpha[1], ln(s[1]), a).max(SABFLG);
    }
    // NJOY's bracket, then its stencil shift: `ia` is the last index with
    // `alpha(ia) <= a`, backed off by one at the top so `ia..ia+2` exists.
    let mut i = if a <= alpha[0] {
        0
    } else {
        alpha.partition_point(|&x| x <= a) - 1
    };
    if i + 2 >= n {
        i = n - 3;
    }
    terpq(
        alpha[i],
        ln(s[i]),
        alpha[i + 1],
        ln(s[i + 1]),
        alpha[i + 2],
        ln(s[i + 2]),
        a,
    )
}

#[cfg(test)]
mod tests {
    use super::super::mf7::parse_mf7;
    use super::*;
    use crate::endf::tape::Tape;
    use std::fs::File;

    fn al27() -> IncoherentInelastic {
        let p = crate::reference_data::reference_endf_dir().join("tsl-013_Al_027-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        parse_mf7(&tape, 53).unwrap().incoherent_inelastic.unwrap()
    }

    #[test]
    fn bound_cross_section_is_physical() {
        let ii = al27();
        // Al: A ≈ 26.75, B(1) ≈ 1.348 ⇒ σ_b ≈ 1.45 b (Al bound scattering ~1.5 b).
        let sb = ii.sigma_bound(1.0);
        assert!((1.2..1.7).contains(&sb), "Al σ_b ≈ 1.45 b, got {sb}");
        assert!((ii.mass_ratio() - 26.75).abs() < 0.1);
    }

    #[test]
    fn double_differential_nonnegative_and_finite() {
        let ii = al27();
        let temp = ii.temperature_k;
        for &mu in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
            for &ep in &[0.001, 0.0253, 0.1] {
                let s = ii.double_differential(0.0253, ep, mu, temp, 1.0);
                assert!(s >= 0.0 && s.is_finite(), "d²σ ≥ 0 finite, got {s}");
            }
        }
    }

    #[test]
    fn inelastic_cross_section_is_positive_and_finite() {
        let ii = al27();
        let temp = ii.temperature_k;
        for &e in &[0.001, 0.0253, 0.1, 0.5, 1.0] {
            let xs = ii.cross_section(e, temp, 1.0);
            assert!(
                xs.is_finite() && xs >= 0.0,
                "σ_inel(E={e}) finite ≥ 0, got {xs}"
            );
        }
    }

    #[test]
    fn cross_section_rises_toward_free_gas_limit() {
        // For this cold (T=20 K) crystal the incoherent-inelastic cross section is
        // *suppressed* at thermal energies (few phonons to exchange) and rises
        // toward the free-atom cross section σ_free = B(1)/natom as the incident
        // energy climbs above the phonon spectrum — the correct free-gas limit.
        let ii = al27();
        let temp = ii.temperature_k;
        let sigma_free = ii.b[0]; // = B(1)/natom (natom = 1) = free-atom σ
        let a = ii.cross_section(0.0253, temp, 1.0);
        let b = ii.cross_section(0.5, temp, 1.0);
        let c = ii.cross_section(1.5, temp, 1.0);
        assert!(a < b && b < c, "σ_inel rises with E: {a} < {b} < {c}");
        assert!(
            c > 0.5 * sigma_free && c < 1.3 * sigma_free,
            "σ_inel(1.5 eV)={c} should approach σ_free={sigma_free}"
        );
    }
}
