//! Variably-saturated **characteristic curves** — retention `S_e(p_c)` and
//! liquid relative permeability `k_r(S_e)`.
//!
//! RICHARDS flow closes the mass-conservation equation with two constitutive
//! relations of the capillary pressure `p_c`:
//!
//! - a **retention** (water-characteristic) curve giving effective saturation
//!   `S_e(p_c)`, and
//! - a **relative-permeability** curve `k_r(S_e)` scaling intrinsic
//!   permeability.
//!
//! Two closed-form model families are provided, dispatched by the
//! [`CharacteristicCurves`] enum (no trait objects, per the workspace rule):
//!
//! - [`VanGenuchten`] (van Genuchten 1980 retention + Mualem 1976 rel-perm),
//! - [`BrooksCorey`] (Brooks & Corey 1964 retention + Burdine rel-perm), and
//! - [`Haverkamp`] (Haverkamp et al. 1977 retention + conductivity — the Celia
//!   1990 benchmark model).
//!
//! # Conventions
//!
//! - Capillary pressure `p_c = p_gas - p_liq`. In the **unsaturated** zone
//!   `p_c >= 0`; `p_c <= 0` means the medium is **fully saturated**
//!   (`S_e = 1`, `dS_e/dp_c = 0`).
//! - **Effective saturation** `S_e = (S_l - S_r)/(1 - S_r)` lives in `[0, 1]`,
//!   where `S_l` is liquid saturation and `S_r` the residual liquid saturation.
//!   The retention curves here return `S_e` directly; converting to `S_l`
//!   (`S_l = S_r + (1 - S_r) S_e`) is the caller's job and uses
//!   [`CharacteristicCurves::residual_saturation`].
//! - `k_r(S_e)` is the **liquid** relative permeability, `k_r(1) = 1`,
//!   `k_r(0) = 0`.
//!
//! # Clamping
//!
//! On output `S_e` is clamped to `[0, 1]` and `k_r` to `[0, 1]`; the
//! `k_r`/`dk_r` inputs clamp `S_e` to `[0, 1]` first. The saturated
//! (`p_c <= 0`) and residual (`S_e -> 0`) limits are handled explicitly so no
//! `NaN` is produced from a `pow` of zero. See the per-derivative notes for the
//! one genuinely singular case (van Genuchten–Mualem `dk_r/dS_e` as
//! `S_e -> 1`).

use crate::error::PflotranError;
use crate::units::{CapillaryPressure, RelativePermeability, Saturation};
use uom::si::f64::Ratio;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// Internal `f64`-level contract every retention/rel-perm model satisfies.
///
/// The compiler checks each concrete model implements the full set; the public
/// [`CharacteristicCurves`] enum then dispatches over them with an exhaustive
/// `match`. All quantities here are plain SI `f64` (pascal, dimensionless) — the
/// `uom` wrapping happens once at the enum boundary. Not exported: it is a
/// compile-time contract, not a dispatch surface.
trait CharacteristicCurveModel {
    /// Effective saturation `S_e(p_c)` in `[0, 1]`; `p_c` in Pa.
    fn effective_saturation(&self, pc_pa: f64) -> f64;
    /// `dS_e/dp_c` in 1/Pa (`<= 0`); `p_c` in Pa.
    fn d_se_d_pc(&self, pc_pa: f64) -> f64;
    /// Liquid relative permeability `k_r(S_e)` in `[0, 1]` (`S_e` dimensionless).
    fn relative_permeability(&self, se: f64) -> f64;
    /// `dk_r/dS_e` (dimensionless); `S_e` dimensionless.
    fn d_kr_d_se(&self, se: f64) -> f64;
    /// Residual liquid saturation `S_r` in `[0, 1)`.
    fn residual_saturation(&self) -> f64;
}

/// Saturation & relative-permeability characteristic curves (enum dispatch).
///
/// A closed set of retention/relative-permeability model families. Adding a
/// family is a new variant, which the compiler then forces every `match` to
/// handle. See the module header for the sign/clamping conventions.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum CharacteristicCurves {
    /// van Genuchten (1980) retention + Mualem (1976) relative permeability.
    VanGenuchten(VanGenuchten),
    /// Brooks & Corey (1964) retention + Burdine relative permeability.
    BrooksCorey(BrooksCorey),
    /// Haverkamp et al. (1977) retention + conductivity — the constitutive model
    /// of the canonical Celia (1990) infiltration benchmark.
    Haverkamp(Haverkamp),
}

impl CharacteristicCurves {
    /// Effective saturation `S_e(p_c)` in `[0, 1]`.
    ///
    /// For `p_c <= 0` (saturated) returns `S_e = 1`. The result is clamped to
    /// `[0, 1]`. Input `p_c` and the return are `uom`-typed.
    pub fn effective_saturation(&self, pc: CapillaryPressure) -> Saturation {
        let pc_pa = pc.get::<pascal>();
        let se = match self {
            Self::VanGenuchten(m) => m.effective_saturation(pc_pa),
            Self::BrooksCorey(m) => m.effective_saturation(pc_pa),
            Self::Haverkamp(m) => m.effective_saturation(pc_pa),
        };
        Ratio::new::<ratio>(se.clamp(0.0, 1.0))
    }

    /// Derivative `dS_e/dp_c`, in 1/Pa (always `<= 0`; `0` in the saturated
    /// zone `p_c <= 0`). Returned as a plain `f64` (a per-pascal rate).
    pub fn d_se_d_pc(&self, pc: CapillaryPressure) -> f64 {
        let pc_pa = pc.get::<pascal>();
        match self {
            Self::VanGenuchten(m) => m.d_se_d_pc(pc_pa),
            Self::BrooksCorey(m) => m.d_se_d_pc(pc_pa),
            Self::Haverkamp(m) => m.d_se_d_pc(pc_pa),
        }
    }

    /// Liquid relative permeability `k_r(S_e)` in `[0, 1]`.
    ///
    /// The input effective saturation is clamped to `[0, 1]` and the output to
    /// `[0, 1]`. `k_r(1) = 1`, `k_r(0) = 0`. Input and return are `uom`-typed.
    pub fn relative_permeability(&self, se: Saturation) -> RelativePermeability {
        let se = se.get::<ratio>().clamp(0.0, 1.0);
        let kr = match self {
            Self::VanGenuchten(m) => m.relative_permeability(se),
            Self::BrooksCorey(m) => m.relative_permeability(se),
            Self::Haverkamp(m) => m.relative_permeability(se),
        };
        Ratio::new::<ratio>(kr.clamp(0.0, 1.0))
    }

    /// Derivative `dk_r/dS_e` (dimensionless), as a plain `f64`.
    ///
    /// The input effective saturation is clamped to `[0, 1]`. Note the van
    /// Genuchten–Mualem form has an **infinite** slope as `S_e -> 1` (a known
    /// property of that model); see [`VanGenuchten`] for how the endpoints are
    /// handled without producing `NaN`.
    pub fn d_kr_d_se(&self, se: Saturation) -> f64 {
        let se = se.get::<ratio>().clamp(0.0, 1.0);
        match self {
            Self::VanGenuchten(m) => m.d_kr_d_se(se),
            Self::BrooksCorey(m) => m.d_kr_d_se(se),
            Self::Haverkamp(m) => m.d_kr_d_se(se),
        }
    }

    /// Residual liquid saturation `S_r` in `[0, 1)`.
    pub fn residual_saturation(&self) -> f64 {
        match self {
            Self::VanGenuchten(m) => m.residual_saturation(),
            Self::BrooksCorey(m) => m.residual_saturation(),
            Self::Haverkamp(m) => m.residual_saturation(),
        }
    }
}

/// van Genuchten (1980) retention + Mualem (1976) relative permeability.
///
/// # Retention curve
///
/// $$ S_e(p_c) = \left[\, 1 + (\alpha \, p_c)^{n} \,\right]^{-m}, \quad p_c > 0 $$
///
/// with `S_e = 1` for `p_c <= 0`, and (by default) `m = 1 - 1/n`. Its
/// derivative is
///
/// $$ \frac{dS_e}{dp_c} = -m \, n \, \alpha \, (\alpha p_c)^{n-1} \left[\, 1 + (\alpha p_c)^{n} \,\right]^{-m-1} \le 0 . $$
///
/// # Relative permeability (Mualem)
///
/// $$ k_r(S_e) = \sqrt{S_e}\,\left[\, 1 - \left(1 - S_e^{1/m}\right)^{m} \,\right]^{2} $$
///
/// with the analytical derivative
///
/// $$ \frac{dk_r}{dS_e} = \tfrac{1}{2} S_e^{-1/2} g^{2} + 2\, S_e^{1/2}\, g \, (1 - S_e^{1/m})^{m-1} \, S_e^{1/m - 1}, \quad g = 1 - (1 - S_e^{1/m})^{m}. $$
///
/// > Written above with `\tfrac` for readability only — the codebase's
/// > GitHub-math rule bans `\tfrac` in README/`docs` prose, but this is a
/// > source doc comment, not a rendered README.
///
/// **Endpoint note (human review):** `dk_r/dS_e` diverges to `+inf` as
/// `S_e -> 1` — this is an intrinsic feature of the van Genuchten–Mualem model,
/// not a bug. The implementation returns `f64::INFINITY` at `S_e = 1` and the
/// finite limit `0` at `S_e = 0`, so no `NaN` is produced. A Newton solver using
/// this curve near full saturation should be aware of the singular slope.
///
/// # Units and ranges
///
/// - `alpha` — 1/Pa, inverse of the air-entry pressure; must be `> 0`.
/// - `n` — dimensionless pore-size-distribution parameter; must be `> 1`.
/// - `m` — dimensionless; typically `1 - 1/n` (set by [`VanGenuchten::new`]).
/// - `sr` — residual liquid saturation in `[0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VanGenuchten {
    /// Inverse air-entry pressure `alpha`. SI unit: 1/Pa. Strictly positive.
    pub alpha: f64,
    /// Pore-size-distribution parameter `n > 1`. Dimensionless.
    pub n: f64,
    /// Shape parameter `m`, typically `1 - 1/n`. Dimensionless, in `(0, 1)`.
    pub m: f64,
    /// Residual liquid saturation `S_r` in `[0, 1)`. Dimensionless.
    pub sr: f64,
}

impl VanGenuchten {
    /// Construct a validated van Genuchten model, setting `m = 1 - 1/n`.
    ///
    /// # Parameters
    ///
    /// - `alpha` — 1/Pa, must be finite and `> 0`.
    /// - `n` — dimensionless, must be finite and `> 1`.
    /// - `sr` — residual liquid saturation, must be finite and in `[0, 1)`.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if any parameter is non-finite or out of
    /// range.
    pub fn new(alpha: f64, n: f64, sr: f64) -> Result<Self, PflotranError> {
        if !alpha.is_finite() || alpha <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "van Genuchten alpha must be finite and > 0 1/Pa, got {alpha}"
            )));
        }
        if !n.is_finite() || n <= 1.0 {
            return Err(PflotranError::InvalidInput(format!(
                "van Genuchten n must be finite and > 1, got {n}"
            )));
        }
        if !sr.is_finite() || sr < 0.0 || sr >= 1.0 {
            return Err(PflotranError::InvalidInput(format!(
                "residual saturation sr must be finite and in [0, 1), got {sr}"
            )));
        }
        Ok(Self {
            alpha,
            n,
            m: 1.0 - 1.0 / n,
            sr,
        })
    }
}

impl CharacteristicCurveModel for VanGenuchten {
    fn effective_saturation(&self, pc_pa: f64) -> f64 {
        if pc_pa <= 0.0 {
            return 1.0;
        }
        let ap = self.alpha * pc_pa; // > 0
        let f = 1.0 + ap.powf(self.n);
        f.powf(-self.m)
    }

    fn d_se_d_pc(&self, pc_pa: f64) -> f64 {
        if pc_pa <= 0.0 {
            return 0.0;
        }
        let ap = self.alpha * pc_pa; // > 0
        let f = 1.0 + ap.powf(self.n);
        // dSe/dpc = -m * n * alpha * ap^(n-1) * f^(-m-1)
        -self.m * self.n * self.alpha * ap.powf(self.n - 1.0) * f.powf(-self.m - 1.0)
    }

    fn relative_permeability(&self, se: f64) -> f64 {
        let se = se.clamp(0.0, 1.0);
        if se <= 0.0 {
            return 0.0;
        }
        if se >= 1.0 {
            return 1.0;
        }
        let sm = se.powf(1.0 / self.m); // Se^(1/m)
        let inner = 1.0 - sm; // 1 - Se^(1/m), in (0,1)
        let g = 1.0 - inner.powf(self.m); // 1 - (1 - Se^(1/m))^m
        (se.sqrt() * g * g).clamp(0.0, 1.0)
    }

    fn d_kr_d_se(&self, se: f64) -> f64 {
        let se = se.clamp(0.0, 1.0);
        // Finite limit 0 at Se -> 0 (guard Se^(-1/2) blow-up).
        if se <= 0.0 {
            return 0.0;
        }
        // Genuinely singular endpoint: dkr/dSe -> +inf as Se -> 1 (VG–Mualem).
        if se >= 1.0 {
            return f64::INFINITY;
        }
        let sm = se.powf(1.0 / self.m); // Se^(1/m)
        let inner = 1.0 - sm; // in (0,1)
        let g = 1.0 - inner.powf(self.m);
        let term1 = 0.5 * se.powf(-0.5) * g * g;
        let term2 =
            se.sqrt() * 2.0 * g * inner.powf(self.m - 1.0) * se.powf(1.0 / self.m - 1.0);
        term1 + term2
    }

    fn residual_saturation(&self) -> f64 {
        self.sr
    }
}

/// Brooks & Corey (1964) retention + Burdine relative permeability.
///
/// # Retention curve
///
/// $$ S_e(p_c) = (\alpha \, p_c)^{-\lambda} \quad \text{for } p_c \ge 1/\alpha, \qquad S_e = 1 \quad \text{for } p_c < 1/\alpha $$
///
/// where `1/alpha` is the air-entry (bubbling) pressure. Below air entry the
/// medium is fully saturated. Its derivative (for `p_c > 1/alpha`) is
///
/// $$ \frac{dS_e}{dp_c} = -\lambda \, \alpha \, (\alpha p_c)^{-\lambda - 1} \le 0, $$
///
/// and `0` for `p_c <= 1/alpha`. **Note:** the curve has a *kink* at the
/// air-entry pressure `p_c = 1/alpha` — `dS_e/dp_c` jumps from `0` (saturated
/// side) to `-lambda*alpha` (unsaturated side). At exactly `p_c = 1/alpha` this
/// implementation returns the saturated-side value `0`.
///
/// # Relative permeability (Burdine)
///
/// $$ k_r(S_e) = S_e^{\,3 + 2/\lambda}, \qquad \frac{dk_r}{dS_e} = \left(3 + \tfrac{2}{\lambda}\right) S_e^{\,2 + 2/\lambda}. $$
///
/// Both are smooth and finite on `[0, 1]`: `k_r(1) = 1`, `k_r(0) = 0`, and the
/// derivative is `0` at `S_e = 0` and `3 + 2/lambda` at `S_e = 1`.
///
/// # Units and ranges
///
/// - `alpha` — 1/Pa, inverse air-entry pressure; must be `> 0`.
/// - `lambda` — dimensionless pore-size-distribution index; must be `> 0`.
/// - `sr` — residual liquid saturation in `[0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrooksCorey {
    /// Inverse air-entry pressure `alpha`. SI unit: 1/Pa. Strictly positive.
    pub alpha: f64,
    /// Pore-size-distribution index `lambda > 0`. Dimensionless.
    pub lambda: f64,
    /// Residual liquid saturation `S_r` in `[0, 1)`. Dimensionless.
    pub sr: f64,
}

impl BrooksCorey {
    /// Construct a validated Brooks–Corey model.
    ///
    /// # Parameters
    ///
    /// - `alpha` — 1/Pa, must be finite and `> 0`.
    /// - `lambda` — dimensionless, must be finite and `> 0`.
    /// - `sr` — residual liquid saturation, must be finite and in `[0, 1)`.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if any parameter is non-finite or out of
    /// range.
    pub fn new(alpha: f64, lambda: f64, sr: f64) -> Result<Self, PflotranError> {
        if !alpha.is_finite() || alpha <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Brooks–Corey alpha must be finite and > 0 1/Pa, got {alpha}"
            )));
        }
        if !lambda.is_finite() || lambda <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Brooks–Corey lambda must be finite and > 0, got {lambda}"
            )));
        }
        if !sr.is_finite() || sr < 0.0 || sr >= 1.0 {
            return Err(PflotranError::InvalidInput(format!(
                "residual saturation sr must be finite and in [0, 1), got {sr}"
            )));
        }
        Ok(Self { alpha, lambda, sr })
    }
}

impl CharacteristicCurveModel for BrooksCorey {
    fn effective_saturation(&self, pc_pa: f64) -> f64 {
        let ap = self.alpha * pc_pa;
        if ap <= 1.0 {
            // At or below air-entry pressure (includes pc <= 0): saturated.
            1.0
        } else {
            ap.powf(-self.lambda)
        }
    }

    fn d_se_d_pc(&self, pc_pa: f64) -> f64 {
        let ap = self.alpha * pc_pa;
        if ap <= 1.0 {
            0.0
        } else {
            // dSe/dpc = -lambda * alpha * (alpha pc)^(-lambda-1)
            -self.lambda * self.alpha * ap.powf(-self.lambda - 1.0)
        }
    }

    fn relative_permeability(&self, se: f64) -> f64 {
        let se = se.clamp(0.0, 1.0);
        if se <= 0.0 {
            return 0.0;
        }
        se.powf(3.0 + 2.0 / self.lambda).clamp(0.0, 1.0)
    }

    fn d_kr_d_se(&self, se: f64) -> f64 {
        let se = se.clamp(0.0, 1.0);
        if se <= 0.0 {
            // Finite limit 0 (exponent 2 + 2/lambda > 0).
            return 0.0;
        }
        (3.0 + 2.0 / self.lambda) * se.powf(2.0 + 2.0 / self.lambda)
    }

    fn residual_saturation(&self) -> f64 {
        self.sr
    }
}

/// Haverkamp et al. (1977) retention + conductivity — the constitutive model of
/// the canonical Celia, Bouloutas & Zarba (1990) infiltration benchmark.
///
/// The Haverkamp relations are naturally written in terms of **pressure head**
/// `psi` (suction, in centimetres). This crate works in capillary pressure
/// `p_c` (Pa), converted with `|psi_cm| = c * p_c`, where `c = cm_per_pa` (for
/// water, `100 / (rho g) ≈ 0.0101972 cm/Pa`).
///
/// # Retention curve
///
/// $$ S_e(p_c) = \frac{\alpha}{\alpha + |\psi|^{\beta}}, \quad p_c > 0 $$
///
/// with `S_e = 1` for `p_c <= 0`. (Equivalently `theta = theta_r + (theta_s -
/// theta_r) S_e`.) Its derivative w.r.t. `p_c` (`psi = c\,p_c`) is
///
/// $$ \frac{dS_e}{dp_c} = -\,\frac{\alpha\,\beta\,c\,|\psi|^{\beta-1}}{\left(\alpha + |\psi|^{\beta}\right)^{2}} \le 0 . $$
///
/// # Relative permeability (conductivity)
///
/// Haverkamp's conductivity `K(psi) = K_s A / (A + |psi|^gamma)` gives
/// `k_r = K/K_s = A / (A + |psi|^{gamma})`. Because `S_e` is a strict bijection
/// of `|psi|` (`|psi|^{beta} = alpha(1 - S_e)/S_e`), `k_r` can be written as a
/// function of `S_e` alone, keeping the shared `k_r(S_e)` interface:
///
/// $$ k_r(S_e) = \frac{A}{A + \left[\alpha (1 - S_e)/S_e\right]^{\gamma/\beta}} . $$
///
/// This composes with `S_e(p_c)` to reproduce `A/(A + |\psi|^{\gamma})` exactly,
/// so no separate `k_r(p_c)` entry point is needed.
///
/// **Provenance of the standard parameters** ([`Haverkamp::celia_1977_sand`]):
/// Haverkamp et al. (1977) sand as used by Celia, Bouloutas & Zarba (1990),
/// *WRR* 26(7):1483–1496 — `alpha = 1.611e6`, `beta = 3.96`, `A = 1.175e6`,
/// `gamma = 4.74`, `theta_s = 0.287`, `theta_r = 0.075` (head in **cm**). These
/// are widely-reproduced public-literature values. **Verification-only**: the
/// curve is implemented and FD-checked, but the full benchmark comparison is
/// bead op-v6s.9.2/.9.3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Haverkamp {
    /// Retention fitting parameter `alpha` (dimensionless; pairs with head in cm).
    pub alpha: f64,
    /// Retention exponent `beta` (> 0).
    pub beta: f64,
    /// Conductivity fitting parameter `A` (dimensionless; pairs with head in cm).
    pub a: f64,
    /// Conductivity exponent `gamma` (> 0).
    pub gamma: f64,
    /// Residual water content `theta_r` (>= 0).
    pub theta_r: f64,
    /// Saturated water content `theta_s` (in `(theta_r, 1]`).
    pub theta_s: f64,
    /// Head-per-pressure conversion `|psi_cm| = cm_per_pa * p_c[Pa]` (> 0).
    pub cm_per_pa: f64,
}

impl Haverkamp {
    /// Water conversion factor `100 / (rho g)` (cm of head per pascal), for
    /// `rho = 1000 kg/m^3`, `g = 9.80665 m/s^2`.
    pub const WATER_CM_PER_PA: f64 = 100.0 / (1000.0 * 9.806_65);

    /// Construct a Haverkamp model for water (uses [`Self::WATER_CM_PER_PA`]).
    ///
    /// Errors unless `alpha, beta, a, gamma > 0` and `0 <= theta_r < theta_s <= 1`.
    pub fn new(
        alpha: f64,
        beta: f64,
        a: f64,
        gamma: f64,
        theta_r: f64,
        theta_s: f64,
    ) -> Result<Self, PflotranError> {
        Self::with_conversion(alpha, beta, a, gamma, theta_r, theta_s, Self::WATER_CM_PER_PA)
    }

    /// As [`Self::new`] but with an explicit head-per-pressure factor `cm_per_pa`
    /// (for a non-water fluid or a different unit convention). `cm_per_pa > 0`.
    pub fn with_conversion(
        alpha: f64,
        beta: f64,
        a: f64,
        gamma: f64,
        theta_r: f64,
        theta_s: f64,
        cm_per_pa: f64,
    ) -> Result<Self, PflotranError> {
        if !(alpha > 0.0 && beta > 0.0 && a > 0.0 && gamma > 0.0) {
            return Err(PflotranError::InvalidInput(
                "Haverkamp: alpha, beta, a, gamma must all be positive".into(),
            ));
        }
        if !(theta_r >= 0.0 && theta_r < theta_s && theta_s <= 1.0) {
            return Err(PflotranError::InvalidInput(format!(
                "Haverkamp: require 0 <= theta_r < theta_s <= 1, got theta_r={theta_r}, theta_s={theta_s}"
            )));
        }
        if !(cm_per_pa > 0.0) || !cm_per_pa.is_finite() {
            return Err(PflotranError::InvalidInput(
                "Haverkamp: cm_per_pa must be positive and finite".into(),
            ));
        }
        Ok(Self { alpha, beta, a, gamma, theta_r, theta_s, cm_per_pa })
    }

    /// The standard Haverkamp (1977) sand of the Celia et al. (1990) benchmark
    /// (`alpha=1.611e6, beta=3.96, A=1.175e6, gamma=4.74, theta_s=0.287,
    /// theta_r=0.075`, head in cm). See the type-level provenance note.
    pub fn celia_1977_sand() -> Self {
        // Unwrap is safe: these literals satisfy every validity constraint.
        Self::new(1.611e6, 3.96, 1.175e6, 4.74, 0.075, 0.287).unwrap()
    }

    /// Absolute head `|psi|` in cm for a capillary pressure `p_c` (Pa). Zero at
    /// or below saturation (`p_c <= 0`).
    #[inline]
    fn abs_head_cm(&self, pc_pa: f64) -> f64 {
        if pc_pa <= 0.0 {
            0.0
        } else {
            self.cm_per_pa * pc_pa
        }
    }
}

impl CharacteristicCurveModel for Haverkamp {
    fn effective_saturation(&self, pc_pa: f64) -> f64 {
        if pc_pa <= 0.0 {
            return 1.0;
        }
        let psi_b = self.abs_head_cm(pc_pa).powf(self.beta);
        self.alpha / (self.alpha + psi_b)
    }

    fn d_se_d_pc(&self, pc_pa: f64) -> f64 {
        if pc_pa <= 0.0 {
            return 0.0;
        }
        let psi = self.abs_head_cm(pc_pa);
        let psi_b = psi.powf(self.beta);
        let denom = self.alpha + psi_b;
        // dSe/dpc = -alpha*beta*c*psi^(beta-1) / (alpha+psi^beta)^2
        -self.alpha * self.beta * self.cm_per_pa * psi.powf(self.beta - 1.0) / (denom * denom)
    }

    fn relative_permeability(&self, se: f64) -> f64 {
        // kr = A / (A + [alpha(1-se)/se]^(gamma/beta)); guard the endpoints.
        if se <= 0.0 {
            return 0.0;
        }
        if se >= 1.0 {
            return 1.0;
        }
        let u = self.alpha * (1.0 - se) / se; // = |psi|^beta
        let w = u.powf(self.gamma / self.beta); // = |psi|^gamma
        self.a / (self.a + w)
    }

    fn d_kr_d_se(&self, se: f64) -> f64 {
        // dkr/dse = A*r*alpha*u^(r-1) / (se^2 * (A+g)^2), r=gamma/beta, g=u^r.
        // -> 0 at both endpoints; guard to avoid 0*inf / inf*0 forms.
        if se <= 0.0 || se >= 1.0 {
            return 0.0;
        }
        let r = self.gamma / self.beta;
        let u = self.alpha * (1.0 - se) / se;
        let g = u.powf(r);
        let ag = self.a + g;
        let num = self.a * r * self.alpha * u.powf(r - 1.0);
        let val = num / (se * se * ag * ag);
        if val.is_finite() {
            val
        } else {
            0.0
        }
    }

    fn residual_saturation(&self) -> f64 {
        // S_r = theta_r / theta_s (liquid saturation at residual water content).
        self.theta_r / self.theta_s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Pressure;

    fn pc(pa: f64) -> CapillaryPressure {
        Pressure::new::<pascal>(pa)
    }
    fn sat(x: f64) -> Saturation {
        Ratio::new::<ratio>(x)
    }
    fn se_val(c: &CharacteristicCurves, pc_pa: f64) -> f64 {
        c.effective_saturation(pc(pc_pa)).get::<ratio>()
    }
    fn kr_val(c: &CharacteristicCurves, se: f64) -> f64 {
        c.relative_permeability(sat(se)).get::<ratio>()
    }

    fn vg() -> CharacteristicCurves {
        CharacteristicCurves::VanGenuchten(VanGenuchten::new(1.0e-4, 2.0, 0.1).unwrap())
    }
    fn bc() -> CharacteristicCurves {
        CharacteristicCurves::BrooksCorey(BrooksCorey::new(1.0e-4, 2.0, 0.15).unwrap())
    }

    #[test]
    fn vg_new_sets_m() {
        let m = VanGenuchten::new(1.0e-4, 2.0, 0.1).unwrap();
        assert!((m.m - 0.5).abs() < 1e-15);
    }

    #[test]
    fn saturated_zone_is_fully_saturated() {
        for c in [vg(), bc()] {
            assert!((se_val(&c, -1.0) - 1.0).abs() < 1e-15);
            assert!((se_val(&c, 0.0) - 1.0).abs() < 1e-15);
            assert_eq!(c.d_se_d_pc(pc(-1.0)), 0.0);
        }
    }

    #[test]
    fn se_in_unit_interval_and_monotone_decreasing() {
        for c in [vg(), bc()] {
            let mut prev = 2.0;
            for k in 0..200 {
                let p = k as f64 * 1.0e3; // 0 .. ~2e5 Pa
                let s = se_val(&c, p);
                assert!((0.0..=1.0).contains(&s), "Se out of [0,1]: {s} at pc={p}");
                assert!(s <= prev + 1e-12, "Se not non-increasing at pc={p}");
                prev = s;
            }
        }
    }

    #[test]
    fn kr_in_unit_interval_and_endpoints() {
        for c in [vg(), bc()] {
            assert!((kr_val(&c, 1.0) - 1.0).abs() < 1e-12, "kr(1) != 1");
            assert!(kr_val(&c, 0.0).abs() < 1e-12, "kr(0) != 0");
            let mut prev = -1.0;
            for k in 0..=100 {
                let se = k as f64 / 100.0;
                let kr = kr_val(&c, se);
                assert!((0.0..=1.0).contains(&kr), "kr out of [0,1]: {kr} at Se={se}");
                assert!(kr >= prev - 1e-12, "kr not non-decreasing at Se={se}");
                prev = kr;
            }
        }
    }

    #[test]
    fn residual_saturation_reported() {
        assert!((vg().residual_saturation() - 0.1).abs() < 1e-15);
        assert!((bc().residual_saturation() - 0.15).abs() < 1e-15);
    }

    #[test]
    fn d_se_d_pc_matches_finite_difference() {
        // Points strictly in the smooth unsaturated branch of BOTH models:
        // all lie above the Brooks–Corey air-entry pressure 1/alpha = 1e4 Pa,
        // so the FD does not straddle that kink, and van Genuchten is smooth
        // everywhere for pc > 0. Both give 0 < Se < 1 here.
        let points = [1.5e4, 3.0e4, 5.0e4, 8.0e4, 1.2e5];
        for c in [vg(), bc()] {
            for &p0 in &points {
                let h = p0 * 1e-6;
                let fd = (se_val(&c, p0 + h) - se_val(&c, p0 - h)) / (2.0 * h);
                let analytic = c.d_se_d_pc(pc(p0));
                let denom = analytic.abs().max(1e-30);
                let rel = (fd - analytic).abs() / denom;
                assert!(
                    rel < 1e-6,
                    "dSe/dpc FD mismatch at pc={p0}: fd={fd}, analytic={analytic}, rel={rel}"
                );
            }
        }
    }

    #[test]
    fn d_kr_d_se_matches_finite_difference() {
        let points = [0.2, 0.35, 0.5, 0.65, 0.8, 0.9];
        for c in [vg(), bc()] {
            for &se0 in &points {
                let h = se0 * 1e-6;
                let fd = (kr_val(&c, se0 + h) - kr_val(&c, se0 - h)) / (2.0 * h);
                let analytic = c.d_kr_d_se(sat(se0));
                let denom = analytic.abs().max(1e-30);
                let rel = (fd - analytic).abs() / denom;
                assert!(
                    rel < 1e-6,
                    "dkr/dSe FD mismatch at Se={se0}: fd={fd}, analytic={analytic}, rel={rel}"
                );
            }
        }
    }

    #[test]
    fn vg_dse_dpc_is_nonpositive() {
        let c = vg();
        for k in 1..200 {
            let p = k as f64 * 1.0e3;
            assert!(c.d_se_d_pc(pc(p)) <= 0.0);
        }
    }

    #[test]
    fn bc_air_entry_saturated_below_bubbling_pressure() {
        // alpha = 1e-4 => air-entry pressure = 1/alpha = 1e4 Pa.
        let c = bc();
        assert!((se_val(&c, 9.9e3) - 1.0).abs() < 1e-15, "should be saturated below air entry");
        assert!(se_val(&c, 1.1e4) < 1.0, "should desaturate above air entry");
    }

    #[test]
    fn vg_dkr_endpoints_no_nan() {
        // Se=1 slope diverges (documented); must be +inf, not NaN.
        let d1 = vg().d_kr_d_se(sat(1.0));
        assert!(d1.is_infinite() && d1 > 0.0);
        // Se=0 slope is a finite 0, not NaN.
        assert_eq!(vg().d_kr_d_se(sat(0.0)), 0.0);
    }

    #[test]
    fn inputs_are_clamped() {
        for c in [vg(), bc()] {
            // Over-unity / negative Se clamp on kr.
            assert!((kr_val(&c, 1.5) - 1.0).abs() < 1e-12);
            assert!(kr_val(&c, -0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn constructors_reject_bad_inputs() {
        assert!(VanGenuchten::new(0.0, 2.0, 0.1).is_err());
        assert!(VanGenuchten::new(1e-4, 1.0, 0.1).is_err()); // n must be > 1
        assert!(VanGenuchten::new(1e-4, 2.0, 1.0).is_err()); // sr must be < 1
        assert!(BrooksCorey::new(1e-4, 0.0, 0.1).is_err()); // lambda > 0
        assert!(BrooksCorey::new(1e-4, 2.0, -0.1).is_err()); // sr >= 0
    }

    // --- Haverkamp (Celia 1990 benchmark model) ---

    fn hv() -> CharacteristicCurves {
        CharacteristicCurves::Haverkamp(Haverkamp::celia_1977_sand())
    }

    #[test]
    fn haverkamp_se_bounds_and_monotone() {
        let c = hv();
        assert!((se_val(&c, -10.0) - 1.0).abs() < 1e-15, "saturated at pc<=0");
        let mut prev = 1.0;
        // Sweep capillary pressure (Pa) across the unsaturated range.
        for k in 0..40 {
            let pcv = 100.0 * 1.2f64.powi(k);
            let s = se_val(&c, pcv);
            assert!((0.0..=1.0).contains(&s), "Se out of range: {s}");
            assert!(s <= prev + 1e-12, "Se not monotone decreasing");
            prev = s;
        }
    }

    #[test]
    fn haverkamp_kr_bounds_and_endpoints() {
        let c = hv();
        assert!((kr_val(&c, 1.0) - 1.0).abs() < 1e-12, "kr(1) = 1");
        assert!(kr_val(&c, 0.0).abs() < 1e-12, "kr(0) = 0");
        let mut prev = 0.0;
        for k in 1..20 {
            let se = k as f64 / 20.0;
            let kr = kr_val(&c, se);
            assert!((0.0..=1.0).contains(&kr), "kr out of range: {kr}");
            assert!(kr >= prev - 1e-12, "kr not monotone increasing");
            prev = kr;
        }
    }

    #[test]
    fn haverkamp_residual_is_theta_ratio() {
        let m = Haverkamp::celia_1977_sand();
        let c = CharacteristicCurves::Haverkamp(m);
        assert!((c.residual_saturation() - 0.075 / 0.287).abs() < 1e-12);
    }

    #[test]
    fn haverkamp_d_se_d_pc_matches_finite_difference() {
        let c = hv();
        for &pcv in &[500.0, 1500.0, 5000.0, 20000.0] {
            let h = pcv * 1e-6;
            let fd = (se_val(&c, pcv + h) - se_val(&c, pcv - h)) / (2.0 * h);
            let ana = c.d_se_d_pc(pc(pcv));
            assert!(
                (ana - fd).abs() <= 1e-6 * fd.abs().max(1e-12),
                "dSe/dpc FD mismatch at pc={pcv}: analytic {ana}, fd {fd}"
            );
        }
    }

    #[test]
    fn haverkamp_d_kr_d_se_matches_finite_difference() {
        let c = hv();
        for &se in &[0.2, 0.4, 0.6, 0.8] {
            let h = 1e-7;
            let fd = (kr_val(&c, se + h) - kr_val(&c, se - h)) / (2.0 * h);
            let ana = c.d_kr_d_se(sat(se));
            assert!(
                (ana - fd).abs() <= 1e-5 * fd.abs().max(1e-12),
                "dkr/dSe FD mismatch at se={se}: analytic {ana}, fd {fd}"
            );
        }
    }

    #[test]
    fn haverkamp_rejects_bad_inputs() {
        assert!(Haverkamp::new(0.0, 3.96, 1.175e6, 4.74, 0.075, 0.287).is_err());
        assert!(Haverkamp::new(1.611e6, 3.96, 1.175e6, 4.74, 0.3, 0.287).is_err()); // theta_r >= theta_s
        assert!(Haverkamp::with_conversion(1.611e6, 3.96, 1.175e6, 4.74, 0.075, 0.287, -1.0).is_err());
    }
}
