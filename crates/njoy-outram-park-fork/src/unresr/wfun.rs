//! The complex probability integral `w(z)` (Faddeeva function) and the J/K
//! width-fluctuation integrals built on it — the special-function machinery
//! `unresl` needs to average unresolved-resonance cross sections over the
//! Porter-Thomas (χ²) width distributions.
//!
//! Ported from `unresr.f90`: [`uw`] (`uw`, lines 1488-1662), the 62×62
//! precomputed table + biquadratic lookup ([`WTable::new`]/[`WTable::lookup`],
//! from `uwtab`/`quikw`, lines 1450-1486 and 1291-1375), and the actual J/K
//! fluctuation-integral quadrature ([`ajk`], from `ajku`, lines 1377-1448).
//!
//! `w(z) = e^{-z²}·erfc(-iz)` is the same "complex probability integral" this
//! crate's [`crate::wmp::faddeeva`] also evaluates (there for Doppler
//! broadening of pointwise cross sections via WMP). **This module does not
//! reuse that evaluator.** NJOY's `uw`/`uwtab`/`quikw` scheme — a dual-series
//! (asymptotic continued-fraction / Taylor) evaluator, pre-tabulated on a
//! 62×62 grid and looked up via local biquadratic interpolation — is a
//! distinct 1970s-era *numerical* path to the same mathematical function, and
//! `docs/porting-plan.md` §5 is explicit that a faithful port must not
//! substitute a different (even if already-validated) implementation for the
//! one actually being translated: doing so would make it impossible for the
//! verification pass to localise a discrepancy to *this* code. Both are
//! documented here as a cross-reference for whoever eventually verifies this
//! module against the golden oracle.

use crate::common::phys::PI;

const EPS: f64 = 1.0e-7;
const UP: f64 = 1.0e15;
const DN: f64 = 1.0e-15;

/// Continued-fraction recurrence state threaded through both series in
/// [`uw`]. Ported from the shared block at `unresr.f90:1625-1658` (label
/// 480), which the asymptotic-series loop (label 380/390) and the
/// Taylor-series loop (label 430/440) both jump into via a computed `GOTO`
/// — Fortran's way of sharing one block of code between two call sites
/// without a nested procedure. Rust has no `GOTO`, so that sharing becomes an
/// explicit method ([`Self::step`]); the arithmetic is unchanged.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WRecurrence {
    // "previous" terms
    a: f64,
    b: f64,
    g: f64,
    h: f64,
    // "current" terms
    c: f64,
    d: f64,
    am: f64,
    el: f64,
}

impl WRecurrence {
    /// Build the recurrence state for the asymptotic-series branch's initial
    /// values (`unresr.f90:1552-1561`, label 370). Exposed so `purr::wfun`'s
    /// `uw2` (near-identical to `uw` but for a `rez==0` exactness shortcut in
    /// its own convergence loop) can reuse this delicate recurrence rather
    /// than re-derive it.
    pub(crate) fn init_asymptotic(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (Self, f64, f64) {
        const C1: f64 = 1.1283792;
        let rv = 2.0 * (r2 - ai2);
        let ak = 4.0 * rez * aimz;
        (Self { a: 0.0, b: 0.0, g: 1.0, h: 0.0, c: -C1 * aimz, d: C1 * rez, am: rv - 1.0, el: ak }, ak, rv)
    }

    /// Build the recurrence state for the Taylor-series branch's initial
    /// values (`unresr.f90:1586-1595`, label 420). See
    /// [`Self::init_asymptotic`] for why this is exposed `pub(crate)`.
    pub(crate) fn init_taylor() -> Self {
        Self { a: 1.0, b: 0.0, g: 0.0, h: 0.0, c: 0.0, d: 0.0, am: 1.0, el: 0.0 }
    }

    /// One continued-fraction step (`unresr.f90:1625-1658`): given the
    /// series-specific `(ajp, temp4, ak)` for this iteration, advance the
    /// recurrence, rescaling by [`UP`]/[`DN`] to avoid over/underflow (only
    /// the *ratios* of these tracked quantities feed the final `w(z)`, so a
    /// uniform rescale changes nothing but the exponent range).
    pub(crate) fn step(&mut self, ajp: f64, temp4: f64, ak: f64) {
        let tempc = ajp * self.c + temp4 * self.a - ak * self.d;
        let tempd = ajp * self.d + temp4 * self.b + ak * self.c;
        let temel = ajp * self.el + temp4 * self.h + ak * self.am;
        let tempm = ajp * self.am + temp4 * self.g - ak * self.el;

        self.a = self.c;
        self.b = self.d;
        self.g = self.am;
        self.h = self.el;
        self.c = tempc;
        self.d = tempd;
        self.am = tempm;
        self.el = temel;

        let mag = tempm.abs() + temel.abs();
        if mag >= UP {
            self.c *= DN;
            self.d *= DN;
            self.am *= DN;
            self.el *= DN;
        } else if mag <= DN {
            self.c *= UP;
            self.d *= UP;
            self.am *= UP;
            self.el *= UP;
        }
    }

    /// `rew, aimw` from the current recurrence state (`unresr.f90:1574-1576`
    /// / `1615-1618`'s common `tempc*tempm+... / amagn` shape).
    pub(crate) fn ratio(&self) -> (f64, f64) {
        let amagn = self.am * self.am + self.el * self.el;
        let rew = (self.c * self.am + self.d * self.el) / amagn;
        let aimw = (self.am * self.d - self.el * self.c) / amagn;
        (rew, aimw)
    }

    /// Raw `(c, d, am, el)` recurrence state — needed by the Taylor branch's
    /// `ref`/`aimf` correction terms (`unresr.f90:1612-1614` /
    /// `purr.f90:2729-2730`), which use these four values directly rather
    /// than through [`Self::ratio`]'s combined form.
    pub(crate) fn raw_cd_am_el(&self) -> (f64, f64, f64, f64) {
        (self.c, self.d, self.am, self.el)
    }

    /// `am² + el²`, the denominator term the Taylor branch's `ref`/`aimf`
    /// correction needs (`unresr.f90:1612`'s `am*am+el*el`).
    pub(crate) fn am_el_mag_squared(&self) -> f64 {
        self.am * self.am + self.el * self.el
    }
}

/// Asymptotic-series branch (`kw=1`, `unresr.f90:1549-1579`, labels 370-390).
/// Valid away from the origin; `aimz` here is `|Im(z)|` (unsigned).
fn w_asymptotic(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    let (mut state, ak, rv) = WRecurrence::init_asymptotic(rez, aimz, r2, ai2);

    let mut aak: f64 = 1.0;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let ajtemp = 2.0 * aak;
        let temp4 = (1.0 - ajtemp) * ajtemp;
        let ajp = rv - (4.0 * aak + 1.0);
        state.step(ajp, temp4, ak);
        aak += 1.0;

        let (pr, pim) = (rew, aimw);
        let (new_rew, new_aimw) = state.ratio();
        rew = new_rew;
        aimw = new_aimw;
        if (rew - pr).abs() < EPS && (aimw - pim).abs() < EPS {
            return (rew, aimw);
        }
    }
}

/// Taylor-series branch (`kw=2`, `unresr.f90:1581-1624`, labels 420-440).
/// Valid near the origin; `aimz` here is the **signed** `Im(z)` (`aim1`).
fn w_taylor(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    const C2: f64 = 1.5;
    let rpi = PI.sqrt();

    let temp1 = r2 + ai2;
    let temp2 = 2.0 * temp1 * temp1;
    let aj0 = -(r2 - ai2) / temp2;
    let ak = 2.0 * rez * aimz / temp2;

    let mut state = WRecurrence { a: 1.0, b: 0.0, g: 0.0, h: 0.0, c: 0.0, d: 0.0, am: 1.0, el: 0.0 };

    let expon = (temp2 * aj0).exp();
    let expc = expon * (temp2 * ak).cos();
    let exps = -expon * (temp2 * ak).sin();

    let mut ajsig: f64 = 0.0;
    let mut sig2p: f64 = 2.0 * C2;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let aj4sig = 4.0 * ajsig;
        let aj4sm1 = aj4sig - 1.0;
        let temp3 = 1.0 / (aj4sm1 * (aj4sig + 3.0));
        let tt4 = sig2p * (2.0 * ajsig - 1.0);
        let temp4 = tt4 / (aj4sm1 * (aj4sig + 1.0) * (aj4sig - 3.0) * aj4sm1);
        let ajp = aj0 + temp3;
        state.step(ajp, temp4, ak);

        ajsig += 1.0;
        let temp7 = rpi * (state.am * state.am + state.el * state.el);
        let ref_ = (aimz * (state.c * state.am + state.d * state.el)
            - rez * (state.am * state.d - state.el * state.c))
            / temp7
            / temp1;
        let aimf = (aimz * (state.am * state.d - state.el * state.c)
            + rez * (state.c * state.am + state.d * state.el))
            / temp7
            / temp1;

        let (pr, pim) = (rew, aimw);
        rew = expc - ref_;
        aimw = exps - aimf;
        if (rew - pr).abs() < EPS && (aimw - pim).abs() < EPS {
            return (rew, aimw);
        }
        sig2p = 2.0 * ajsig;
    }
}

/// The complex probability integral `w(z) = e^{-z²}·erfc(-iz)`, returned as
/// `(Re w, Im w)`. Ported from `uw` (`unresr.f90:1488-1662`).
///
/// Selects between the asymptotic (continued-fraction) and Taylor-series
/// branches by the region of the `(Re z, |Im z|)` plane, exactly mirroring
/// the six break-point inequalities at `unresr.f90:1532-1536`; `aim1 < 0`
/// forces the Taylor branch regardless of region (the asymptotic branch's
/// derivation assumes `Im z ≥ 0`).
pub fn uw(rez: f64, aim1: f64) -> (f64, f64) {
    let aimz = aim1.abs();
    let abrez = rez.abs();
    if abrez + aimz == 0.0 {
        return (1.0, 0.0);
    }

    let r2 = rez * rez;
    let ai2 = aimz * aimz;

    const BRK1: f64 = 1.25;
    const BRK2: f64 = 5.0;
    const BRK3: f64 = 1.863_636;
    const BRK4: f64 = 4.1;
    const BRK5: f64 = 1.71;
    const BRK6: f64 = 2.89;
    const BRK7: f64 = 1.18;
    const BRK8: f64 = 5.76;
    const BRK9: f64 = 1.5;

    // unresr.f90:1532-1536 — region selection (kw=1 asymptotic, kw=2 taylor).
    let use_taylor = if abrez + BRK1 * aimz - BRK2 > 0.0 {
        false
    } else if abrez + BRK3 * aimz - BRK4 > 0.0 {
        false
    } else if r2 + BRK5 * ai2 - BRK6 < 0.0 {
        true
    } else if r2 + BRK7 * ai2 - BRK8 >= 0.0 {
        true
    } else {
        aimz - BRK9 >= 0.0
    };

    if use_taylor {
        // unresr.f90:1540/1546 — taylor branch uses the SIGNED Im(z).
        w_taylor(rez, aim1, r2, ai2)
    } else if aim1 >= 0.0 {
        // unresr.f90:1544 (kw=1 asymptotic) — uses |Im(z)| (== aimz already).
        w_asymptotic(rez, aimz, r2, ai2)
    } else {
        // unresr.f90:1545-1546 — asymptotic branch invalid for Im(z)<0.
        w_taylor(rez, aim1, r2, ai2)
    }
}

/// A precomputed 62×62 table of `w(z)` on the grid `x,y ∈
/// {-0.1, 0.0, 0.1, ..., 6.0}` (step 0.1), with local biquadratic
/// interpolation for off-grid points — NJOY's speed optimisation for
/// evaluating `w(z)` at the many quadrature points [`ajk`] needs, avoiding a
/// full [`uw`] evaluation at each one. Ported from `uwtab`
/// (`unresr.f90:1450-1486`) and `quikw` (`unresr.f90:1291-1375`).
pub struct WTable {
    /// `tr[i][j]`, `ti[i][j]` = Re/Im `w(x[i], y[j])`, 0-indexed (Fortran's
    /// `tr(62,62)`/`ti(62,62)` are 1-indexed; see [`WTable::lookup`] for the
    /// index-base translation).
    tr: Vec<Vec<f64>>,
    ti: Vec<Vec<f64>>,
}

impl WTable {
    const N: usize = 62;
    const X0: f64 = -0.1;
    const Y0: f64 = -0.1;
    const D: f64 = 0.1;

    /// Build the table (`uwtab`, `unresr.f90:1450-1486`). Grid point `(i, j)`
    /// (0-indexed) sits at `x = X0 + i*D`, `y = Y0 + j*D`.
    pub fn new() -> Self {
        let mut tr = vec![vec![0.0; Self::N]; Self::N];
        let mut ti = vec![vec![0.0; Self::N]; Self::N];
        for i in 0..Self::N {
            let xi = Self::X0 + i as f64 * Self::D;
            for j in 0..Self::N {
                let yj = Self::Y0 + j as f64 * Self::D;
                let (rwt, aimt) = uw(xi, yj);
                tr[i][j] = rwt;
                ti[i][j] = aimt;
            }
        }
        WTable { tr, ti }
    }

    /// Look up `w(x, y)` via local biquadratic interpolation on the table
    /// (`quikw`, `unresr.f90:1291-1375`).
    ///
    /// `ki`: whether the imaginary part is needed at all — `quikw`'s callers
    /// (`ajku`) sometimes only need `Re w`; skip the (identical-cost, in this
    /// port) imaginary-part interpolation when `false` by returning `0.0` for
    /// it, matching the Fortran's `if (ki.gt.0)` guard.
    ///
    /// `x` may be negative (`quikw` takes `|x|` internally and flips the sign
    /// of the imaginary part back using the sign of the original `x` — the
    /// symmetry `w(-x+iy) = conj(w(x-iy))`-adjacent relation NJOY exploits so
    /// the table only needs `x ≥ 0`).
    pub fn lookup(&self, x: f64, y: f64, ki: bool) -> (f64, f64) {
        const BREAK1: f64 = 36.0;
        const BREAK2: f64 = 144.0;
        const BREAK3: f64 = 10000.0;
        const C1: f64 = 0.275_255_1;
        const C2: f64 = 2.724_745;
        const C3: f64 = 0.512_424_2;
        const C4: f64 = 0.051_765_36;
        const C5: f64 = 1.128_379_2;

        let rpi = PI.sqrt();
        let aki = if x < 0.0 { -1.0 } else { 1.0 };
        let ax = x.abs();
        let test = ax * ax + y * y;

        if test < BREAK1 {
            // Local biquadratic (6-point) interpolation on the tabulated grid.
            let ii = (ax * 10.0) as i64;
            let jj = (y * 10.0) as i64;
            // Table indices in this port are 0-based; Fortran's `tr(i,n)` etc.
            // use `i=ii+2`, `j=jj+2` against a table whose first grid point is
            // index 1 at x0=-0.1 — i.e. Fortran index `k` <-> our index `k-1`.
            // `i = ii+2` (Fortran) <-> `ii+1` (ours, 0-based), likewise `j`.
            let i = (ii + 1) as usize;
            let j = (jj + 1) as usize;
            let n = j - 1;
            let p = 10.0 * ax - ii as f64;
            let q = 10.0 * y - jj as f64;
            let p2 = p * p;
            let q2 = q * q;
            let pq = p * q;
            let hp = p / 2.0;
            let hq = q / 2.0;
            let hq2 = q2 / 2.0;
            let hp2 = p2 / 2.0;
            let a1 = hq2 - hq;
            let a2 = hp2 - hp;
            let a3 = 1.0 + pq - p2 - q2;
            let a4 = hp2 - pq + hp;
            let a5 = hq2 - pq + hq;

            let get = |t: &Vec<Vec<f64>>, i: usize, j: usize| t[i][j];
            let rew = a1 * get(&self.tr, i, n)
                + a2 * get(&self.tr, i - 1, j)
                + a3 * get(&self.tr, i, j)
                + a4 * get(&self.tr, i + 1, j)
                + a5 * get(&self.tr, i, j + 1)
                + pq * get(&self.tr, i + 1, j + 1);
            let aimw = if ki {
                let w = a1 * get(&self.ti, i, n)
                    + a2 * get(&self.ti, i - 1, j)
                    + a3 * get(&self.ti, i, j)
                    + a4 * get(&self.ti, i + 1, j)
                    + a5 * get(&self.ti, i, j + 1)
                    + pq * get(&self.ti, i + 1, j + 1);
                w * aki
            } else {
                0.0
            };
            (rew, aimw)
        } else if test < BREAK2 {
            let a1 = ax * ax - y * y;
            let a2 = 2.0 * ax * y;
            let a3 = a2 * a2;
            let a4 = a1 - C1;
            let a5 = a1 - C2;
            let d1 = C3 / (a4 * a4 + a3);
            let d2 = C4 / (a5 * a5 + a3);
            let rew = d1 * (a2 * ax - a4 * y) + d2 * (a2 * ax - a5 * y);
            let aimw = if ki { (d1 * (a4 * ax + a2 * y) + d2 * (a5 * ax + a2 * y)) * aki } else { 0.0 };
            (rew, aimw)
        } else if test < BREAK3 {
            let a1 = (ax * ax - y * y) * 2.0;
            let a2 = 4.0 * ax * y;
            let a4 = a1 - 1.0;
            let d1 = C5 / (a4 * a4 + a2 * a2);
            let rew = d1 * (a2 * ax - a4 * y);
            let aimw = if ki { d1 * (a4 * ax + a2 * y) * aki } else { 0.0 };
            (rew, aimw)
        } else {
            let a1 = 1.0 / (rpi * test);
            let rew = y * a1;
            let aimw = if ki { ax * a1 * aki } else { 0.0 };
            (rew, aimw)
        }
    }
}

impl Default for WTable {
    fn default() -> Self {
        Self::new()
    }
}

/// 10-point quadrature abscissas for the Porter-Thomas (χ²) width-fluctuation
/// integrals, one column per number of degrees of freedom `μ ∈ {1,2,3,4}`
/// (`qp`, `unresr.f90:884-898`). `QP[k][mu-1]` is node `k+1` (Fortran 1-indexed)
/// for `μ` degrees of freedom.
const QP: [[f64; 4]; 10] = [
    [3.0013465e-3, 1.3219203e-2, 1.0004488e-3, 1.3219203e-2],
    [7.8592886e-2, 7.2349624e-2, 2.6197629e-2, 7.2349624e-2],
    [4.3282415e-1, 1.9089473e-1, 1.4427472e-1, 1.9089473e-1],
    [1.3345267e0, 3.9528842e-1, 4.4484223e-1, 3.9528842e-1],
    [3.0481846e0, 7.4083443e-1, 1.0160615e0, 7.4083443e-1],
    [5.8263198e0, 1.3498293e0, 1.9421066e0, 1.3498293e0],
    [9.9452656e0, 2.5297983e0, 3.3150885e0, 2.5297983e0],
    [1.5782128e1, 5.2384894e0, 5.2607092e0, 5.2384894e0],
    [2.3996824e1, 1.3821772e1, 7.9989414e0, 1.3821772e1],
    [3.6216208e1, 7.5647525e1, 1.2072069e1, 7.5647525e1],
];

/// 10-point quadrature weights matching [`QP`] (`qw`, `unresr.f90:869-883`).
const QW: [[f64; 4]; 10] = [
    [1.1120413e-1, 3.3773418e-2, 3.3376214e-4, 1.7623788e-3],
    [2.3546798e-1, 7.9932171e-2, 1.8506108e-2, 2.1517749e-2],
    [2.8440987e-1, 1.2835937e-1, 1.2309946e-1, 8.0979849e-2],
    [2.2419127e-1, 1.7652616e-1, 2.9918923e-1, 1.8797998e-1],
    [1.0967668e-1, 2.1347043e-1, 3.3431475e-1, 3.0156335e-1],
    [3.0493789e-2, 2.1154965e-1, 1.7766657e-1, 2.9616091e-1],
    [4.2930874e-3, 1.3365186e-1, 4.2695894e-2, 1.0775649e-1],
    [2.5827047e-4, 2.2630659e-2, 4.0760575e-3, 2.5171914e-3],
    [4.9031965e-6, 1.6313638e-5, 1.1766115e-4, 8.9630388e-10],
    [1.4079206e-8, 2.745383e-31, 5.0989546e-7, 0.0],
];

/// Quadrature node `k` (0-indexed, `k < 10`) for `mu` (1..=4) degrees of
/// freedom — `qp(k+1, mu)` in Fortran (`unresr.f90:884-898`).
pub fn qp_node(k: usize, mu: i32) -> f64 {
    QP[k][(mu - 1).clamp(0, 3) as usize]
}

/// Quadrature weight `k` (0-indexed, `k < 10`) for `mu` (1..=4) degrees of
/// freedom — `qw(k+1, mu)` in Fortran (`unresr.f90:869-883`).
pub fn qw_weight(k: usize, mu: i32) -> f64 {
    QW[k][(mu - 1).clamp(0, 3) as usize]
}

/// Gauss-Legendre 8-point nodes on `[-1,1]` used by [`ajk`] (`xg`,
/// `unresr.f90:1389-1391`).
const GAUSS8_X: [f64; 8] =
    [0.095_012_51, 0.281_603_55, 0.458_016_78, 0.617_876_24, 0.755_404_41, 0.865_631_2, 0.944_575_02, 0.989_400_93];
/// Gauss-Legendre 8-point weights matching [`GAUSS8_X`] (`wg`,
/// `unresr.f90:1392-1394`).
const GAUSS8_W: [f64; 8] =
    [0.189_450_61, 0.182_603_42, 0.169_156_52, 0.149_596_00, 0.124_628_97, 0.095_158_51, 0.062_253_52, 0.027_152_46];

/// The `J` and `K` width-fluctuation integrals (Bondarenko/ETOX method),
/// `J(β, θ) = (π/2)·∫₀^∞ (1/(1+β/χ_a(x)))·φ(x) dx` in the standard notation
/// (`χ_a` the "chi" line-shape function built from `w(z)`), and `K` its
/// current-weighted (transport) counterpart. Ported from `ajku`
/// (`unresr.f90:1377-1448`).
///
/// - `b` — `β = σ̄/s₀ᵤ`, the ratio of the background+dilution cross section to
///   the sequence's peak cross-section scale.
/// - `st` — `θ = Γ_total/D`, total width over level spacing (the resonance
///   overlap parameter).
///
/// Returns `(J, K)`.
pub fn ajk(table: &WTable, b: f64, st: f64) -> (f64, f64) {
    const EPS_AJK: f64 = 0.0002;

    let pi2 = PI / 2.0;
    let sqpi = PI.sqrt();

    // unresr.f90:1400-1405 — small-θ / small-β early-out via the "ep" test.
    let term = if -st / 2.0 < f64::MIN_POSITIVE.ln() { 0.0 } else { (-st / 2.0).exp() };
    let ep = (1.0 - term) / EPS_AJK - b;

    if ep <= 0.0 {
        let aj = pi2 / b;
        return (aj, 2.0 * aj);
    }

    let y1 = st / 2.0;
    let b1 = b / (sqpi * y1);
    let a = ((1.0 + b) / b).sqrt();
    let a2 = a * a;
    let c = 200.0 / (a * st);
    let remj = (pi2 - c.atan()) / (b * a);
    let remk = ((1.0 + a2) * remj - (1.0 - a2) * c / ((c * c + 1.0) * b * a)) / (2.0 * a2);

    let (mut y, mut z, mut yk, mut zk) = (0.0, 0.0, 0.0, 0.0);
    for n in 0..8 {
        let xg = GAUSS8_X[n];
        let wg = GAUSS8_W[n];

        let ax = 5.0 * xg + 5.0;
        let (rew, _) = table.lookup(ax, y1, false);
        let x1 = 1.0 / (1.0 + b1 / rew);
        let z1 = x1 * x1 / rew;

        let ax = -5.0 * xg + 5.0;
        let (rew, _) = table.lookup(ax, y1, false);
        let x2 = 1.0 / (1.0 + b1 / rew);
        let z2 = x2 * x2 / rew;

        let ax = 45.0 * xg + 55.0;
        let (rew, _) = table.lookup(ax, y1, false);
        let x3 = 1.0 / (1.0 + b1 / rew);
        let z3 = x3 * x3 / rew;

        let ax = -45.0 * xg + 55.0;
        let (rew, _) = table.lookup(ax, y1, false);
        let x4 = 1.0 / (1.0 + b1 / rew);
        let z4 = x4 * x4 / rew;

        y += wg * (x3 + x4);
        yk += wg * (z3 + z4);
        z += wg * (x1 + x2);
        zk += wg * (z1 + z2);
    }

    let aj = 5.0 * (z + 9.0 * y) / y1 + remj;
    let ak = aj + 5.0 * b1 * (zk + 9.0 * yk) / y1 + remk;
    (aj, ak)
}
