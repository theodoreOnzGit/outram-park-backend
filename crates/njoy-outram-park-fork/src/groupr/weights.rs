// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR flux **weighting functions** phi(E): the built-in analytic spectra
//! used to collapse continuous-energy cross sections onto multigroup form.
//!
//! # What this module computes
//!
//! A GROUPR group cross section is the flux-weighted average
//! `sigma_g = integral_g sigma(E) phi(E) dE / integral_g phi(E) dE`, where
//! `phi(E)` is the *weighting function* selected by `iwt`. This module ports
//! the **analytic** weighting spectra that `genwtf` sets up (`groupr.f90:4865-
//! 5113`) and `getwtf` evaluates (`groupr.f90:5115-5307`): the closed-form
//! pieces that can be computed at any energy without reading a table.
//!
//! # Ported (analytic) — [`AnalyticWeight`]
//!
//! | `iwt` | weight | note |
//! |------:|--------|------|
//! | 2 | constant | `phi = 1` |
//! | 3 | 1/E | `phi = 1/E` |
//! | 4 | thermal Maxwellian + 1/E + fission | params `eb, tb, ec, tc` (card 8c) |
//! | 6 | (thermal) -- (1/E) -- (fission + fusion) | fixed thermal temperature |
//! | 7 | same, temperature-dependent thermal part | uses run temperature |
//! | 11 | VITAMIN-E (ORNL-5505) | six analytic segments |
//! | 12 | VITAMIN-E, temperature-dependent thermal part | uses run temperature |
//!
//! All energies are in **electron-volts (eV)**; temperatures in **kelvin**.
//! The returned weight `phi(E)` is a spectral density in arbitrary units — only
//! its *shape* matters, since GROUPR normalises by the group flux integral.
//!
//! # Not ported (tabulated / hybrid)
//!
//! `iwt = 1` (read-in TAB1), `iwt = 5` (EPRI-CELL LWR), `iwt = 8`
//! (thermal--1/E--fast-reactor--fission+fusion), and `iwt = 9` (CLAW) are
//! **tabulated**: `getwtf` evaluates them with the ENDF TAB1 interpolator
//! `terpa` over stored point tables. `iwt = 10` (CLAW, t-dependent thermal) is
//! a **hybrid** — an analytic thermal segment below the break plus a tabulated
//! tail. The built-in point tables for `iwt = 5, 8, 9` are preserved verbatim
//! here (see [`epri_cell_lwr_tab1`], [`fast_reactor_tab1`], [`claw_tab1`]) so no
//! data is lost, but their *evaluation* needs the TAB1 machinery and is left as
//! [`AnalyticWeight`]-unavailable; call sites should treat these `iwt` values as
//! [`crate::NjoyError::NotPorted`] until the TAB1 weight evaluator lands.

/// Boltzmann constant in **eV per kelvin**, matching NJOY's `phys` module
/// (`bk = 8.617333262e-5` eV/K, `phys.f90:22`). Used by the
/// temperature-dependent thermal segments (`iwt = 7, 12`).
pub const BOLTZMANN_EV_PER_K: f64 = 8.617333262e-5;

// ---------------------------------------------------------------------------
// iwt = 4 parameters (card 8c) and the genwtf ab/ac derivation.
// ---------------------------------------------------------------------------

/// Parameters for the `iwt = 4` "thermal + 1/E + fission" analytic weight
/// (card 8c, `groupr.f90:5038-5060`). All energies/temperatures in **eV**.
///
/// The spectrum is a Maxwellian below the thermal break `eb`, a `1/E` slowing-
/// down region up to the fission break `ec`, and a fission Maxwellian above.
/// The two normalisation constants `ab`, `ac` make the three pieces join
/// continuously and are derived by [`ThermalFissionParams::normalisation`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalFissionParams {
    /// `eb` — thermal breakpoint (eV): Maxwellian below, 1/E above.
    pub thermal_break_ev: f64,
    /// `tb` — thermal "temperature" (eV) of the low-energy Maxwellian.
    pub thermal_temp_ev: f64,
    /// `ec` — fission breakpoint (eV): 1/E below, fission Maxwellian above.
    pub fission_break_ev: f64,
    /// `tc` — fission "temperature" (eV) of the high-energy Maxwellian.
    pub fission_temp_ev: f64,
}

impl ThermalFissionParams {
    /// Derive `(ab, ac)`, the thermal and fission normalisation constants, per
    /// `genwtf` (`groupr.f90:5038-5045`).
    ///
    /// If `eb > 50*tb` the low-energy Maxwellian is negligible and NJOY sets
    /// `ab = 1, ac = 0`; otherwise
    /// `ab = 1 / (exp(-eb/tb) * eb^2)` and `ac = 1 / (exp(-ec/tc) * ec^1.5)`.
    pub fn normalisation(&self) -> (f64, f64) {
        let (eb, tb, ec, tc) = (
            self.thermal_break_ev,
            self.thermal_temp_ev,
            self.fission_break_ev,
            self.fission_temp_ev,
        );
        if eb > 50.0 * tb {
            (1.0, 0.0)
        } else {
            let ab = 1.0 / ((-eb / tb).exp() * eb.powi(2));
            let ac = 1.0 / ((-ec / tc).exp() * ec.powf(1.5));
            (ab, ac)
        }
    }
}

// ---------------------------------------------------------------------------
// Analytic weight dispatch.
// ---------------------------------------------------------------------------

/// A built-in **analytic** GROUPR weighting function (those `getwtf` can
/// evaluate in closed form). See the module docs for the `iwt` mapping.
///
/// Construct with [`AnalyticWeight::from_iwt`] (returns `None` for tabulated or
/// illegal `iwt`) and evaluate with [`AnalyticWeight::evaluate`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalyticWeight {
    /// `iwt = 2`: constant weight for all Legendre orders (`phi = 1`).
    Constant,
    /// `iwt = 3`: `1/E` weight for all orders.
    OneOverE,
    /// `iwt = 4`: thermal Maxwellian + `1/E` + fission Maxwellian.
    ThermalFission(ThermalFissionParams),
    /// `iwt = 6` / `iwt = 7`: (thermal) -- (1/E) -- (fission + fusion). When
    /// `t_dependent` is true (`iwt = 7`) the thermal segment uses the run
    /// temperature; otherwise a fixed thermal temperature (`wt6a = 0.054` eV).
    ThermalFissionFusion {
        /// `true` for `iwt = 7` (temperature-dependent thermal part).
        t_dependent: bool,
    },
    /// `iwt = 11` / `iwt = 12`: VITAMIN-E weight (ORNL-5505). When
    /// `t_dependent` is true (`iwt = 12`) the thermal segment uses the run
    /// temperature; otherwise the fixed thermal value (`therm = 0.0253` eV).
    VitaminE {
        /// `true` for `iwt = 12` (temperature-dependent thermal part).
        t_dependent: bool,
    },
}

impl AnalyticWeight {
    /// Construct from `|iwt|` for the analytic weights, given the `iwt = 4`
    /// parameters when needed.
    ///
    /// Returns `None` for tabulated (`1, 5, 8, 9`), hybrid (`10`), the flux
    /// modes (`0`, negative), or illegal `iwt`. For `iwt = 4`, `params` must be
    /// `Some`; if it is `None`, this returns `None`.
    pub fn from_iwt(iwt: i32, params: Option<ThermalFissionParams>) -> Option<Self> {
        match iwt.abs() {
            2 => Some(Self::Constant),
            3 => Some(Self::OneOverE),
            4 => params.map(Self::ThermalFission),
            6 => Some(Self::ThermalFissionFusion { t_dependent: false }),
            7 => Some(Self::ThermalFissionFusion { t_dependent: true }),
            11 => Some(Self::VitaminE { t_dependent: false }),
            12 => Some(Self::VitaminE { t_dependent: true }),
            _ => None,
        }
    }

    /// Evaluate the weighting spectrum `phi(E)` at energy `energy_ev` (eV).
    ///
    /// `temperature_kelvin` is the run temperature; it only affects the
    /// temperature-dependent variants (`iwt = 7, 12`). The result is a spectral
    /// density in arbitrary units (shape-only). This reproduces the value
    /// branches of `getwtf` (`groupr.f90:5183-5296`); it does **not** compute
    /// `enext` (the adaptive integration grid step), which is integration
    /// bookkeeping, not part of the weight value.
    pub fn evaluate(&self, energy_ev: f64, temperature_kelvin: f64) -> f64 {
        let e = energy_ev;
        match self {
            // iwt = 2 (groupr.f90:5197-5199)
            Self::Constant => 1.0,
            // iwt = 3 (groupr.f90:5201-5204)
            Self::OneOverE => 1.0 / e,
            // iwt = 4 (groupr.f90:5206-5220)
            Self::ThermalFission(p) => {
                let (ab, ac) = p.normalisation();
                if e <= p.thermal_break_ev {
                    ab * e * (-e / p.thermal_temp_ev).exp()
                } else if e <= p.fission_break_ev {
                    1.0 / e
                } else {
                    ac * e.sqrt() * (-e / p.fission_temp_ev).exp()
                }
            }
            // iwt = 6 / 7 (groupr.f90:5222-5245)
            Self::ThermalFissionFusion { t_dependent } => {
                thermal_fission_fusion(e, *t_dependent, temperature_kelvin)
            }
            // iwt = 11 / 12 (groupr.f90:5278-5296)
            Self::VitaminE { t_dependent } => vitamin_e(e, *t_dependent, temperature_kelvin),
        }
    }
}

// ---------------------------------------------------------------------------
// iwt = 6 / 7: (thermal) -- (1/e) -- (fission + fusion)
// getwtf constants at groupr.f90:5152-5161.
// ---------------------------------------------------------------------------

/// `iwt = 6/7` weight value. Fixed thermal temperature `wt6a = 0.054` eV for
/// `iwt = 6`; for `iwt = 7` the thermal temperature is `bk * T`. Reproduces
/// `groupr.f90:5222-5245`.
fn thermal_fission_fusion(e: f64, t_dependent: bool, temperature_kelvin: f64) -> f64 {
    // getwtf parameters (groupr.f90:5152-5161).
    const WT6A: f64 = 0.054; // fixed thermal temperature (eV)
    const WT6B: f64 = 1.578551e-3; // 1/E region amplitude
    const WT6C: f64 = 2.1e6; // 1/E -> fast break (eV)
    const WT6D: f64 = 2.32472e-12; // fission Maxwellian amplitude
    const WT6E: f64 = 1.4e6; // fission "temperature" (eV)
    const WT6F: f64 = 2.5e4; // fusion width (eV)
    const WT6G: f64 = 1.407e7; // fusion peak energy (eV)
    const WT6H: f64 = 2.51697e-11; // fusion amplitude
    const EXMIN: f64 = -89.0; // exp underflow guard

    let tt = if t_dependent {
        temperature_kelvin * BOLTZMANN_EV_PER_K
    } else {
        WT6A
    };
    let bb = 2.0 * tt;
    // cc = 1 for iwt=6; for iwt=7 cc = wt6b*exp(2)/bb^2 (continuity at bb).
    let cc = if t_dependent {
        WT6B * std::f64::consts::E.powi(2) / bb.powi(2)
    } else {
        1.0
    };

    if e <= bb {
        cc * e * (-e / tt).exp()
    } else if e <= WT6C {
        WT6B / e
    } else {
        let mut wtf = WT6D * e.sqrt() * (-e / WT6E).exp();
        let pow = -(((e / WT6F).sqrt() - (WT6G / WT6F).sqrt()).powi(2)) / 2.0;
        if pow > EXMIN {
            wtf += WT6H * pow.exp();
        }
        wtf
    }
}

// ---------------------------------------------------------------------------
// iwt = 11 / 12: VITAMIN-E weight function (ORNL-5505/5510)
// getwtf constants at groupr.f90:5136-5147.
// ---------------------------------------------------------------------------

/// `iwt = 11/12` VITAMIN-E weight value: six analytic segments. For `iwt = 12`
/// the thermal segment (`e < en1`) uses `bk * T` in place of the fixed
/// `therm = 0.0253` eV. Reproduces `groupr.f90:5278-5296`.
fn vitamin_e(e: f64, t_dependent: bool, temperature_kelvin: f64) -> f64 {
    // getwtf parameters (groupr.f90:5136-5147).
    const CON1: f64 = 7.45824e7;
    const CON2: f64 = 1.0;
    const CON3: f64 = 1.44934e-9;
    const CON4: f64 = 3.90797e-2;
    const CON5: f64 = 2.64052e-5;
    const CON6: f64 = 6.76517e-2;
    const EN1: f64 = 0.414; // thermal/1-over-E break (eV)
    const EN2: f64 = 2.12e6; // 1-over-E/fission break (eV)
    const EN3: f64 = 1.0e7; // fission/1-over-E break (eV)
    const EN4: f64 = 1.252e7; // 1-over-E/fusion break (eV)
    const EN5: f64 = 1.568e7; // fusion/1-over-E break (eV)
    const THERM: f64 = 0.0253; // fixed thermal temperature (eV)
    const THETA: f64 = 1.415e6; // fission "temperature" (eV)
    const FUSION: f64 = 2.5e4; // fusion width (eV)
    const EP: f64 = 1.407e7; // fusion peak energy (eV)

    if e < EN1 {
        let tt = if t_dependent {
            temperature_kelvin * BOLTZMANN_EV_PER_K
        } else {
            THERM
        };
        let cc = if t_dependent {
            CON2 * (EN1 / tt).exp() / EN1.powi(2)
        } else {
            CON1
        };
        cc * e * (-e / tt).exp()
    } else if e < EN2 {
        CON2 / e
    } else if e < EN3 {
        CON3 * e.sqrt() * (-e / THETA).exp()
    } else if e < EN4 {
        CON4 / e
    } else if e < EN5 {
        CON5 * (-5.0 * (e.sqrt() - EP.sqrt()).powi(2) / FUSION).exp()
    } else {
        CON6 / e
    }
}

// ---------------------------------------------------------------------------
// Built-in tabulated weight point tables (data preserved; evaluation NotPorted).
//
// These are ENDF TAB1-format arrays: the first 6 reals are the record header
// (c1, c2, l1, l2, nr, np), followed by `nr` interpolation pairs
// (NBT_k, INT_k) and `np` (x, y) data pairs. `getwtf` feeds them to `terpa`.
// ---------------------------------------------------------------------------

/// EPRI-CELL LWR weight (`iwt = 5`), the concatenation of Fortran `w1` (92) and
/// `w2` (92) (`groupr.f90:4900-4938`). TAB1 header: `np = 88`, one region with
/// `INT = 5` (log-log). x in eV, y = phi(E). Preserved verbatim; not yet
/// evaluable (needs the TAB1 weight evaluator).
pub fn epri_cell_lwr_tab1() -> Vec<f64> {
    let mut v = Vec::with_capacity(184);
    v.extend_from_slice(&W1);
    v.extend_from_slice(&W2);
    v
}

/// Fast-reactor weight thermal--1/E--fast-reactor--fission+fusion (`iwt = 8`),
/// Fortran `w8` (66) (`groupr.f90:4939-4954`). TAB1 header: `np = 29`, one
/// region with `INT = 5` (log-log). Preserved verbatim; evaluation NotPorted.
pub fn fast_reactor_tab1() -> Vec<f64> {
    W8.to_vec()
}

/// CLAW weight (`iwt = 9`), Fortran `w9` (106) (`groupr.f90:4955-4976`). TAB1
/// header: `np = 49`, one region with `INT = 5` (log-log). Preserved verbatim;
/// evaluation NotPorted (also the tabulated tail of the hybrid `iwt = 10`).
pub fn claw_tab1() -> Vec<f64> {
    W9.to_vec()
}

/// Fortran `w1`, `dimension(92)` (`groupr.f90:4900-4916`).
const W1: [f64; 92] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 88.0, 88.0, 5.0, 1.0e-5, 5.25e-4, 0.009, 0.355, 0.016, 0.552, 0.024,
    0.712, 0.029, 0.785, 0.033, 0.829, 0.043, 0.898, 0.05, 0.918, 0.054, 0.921, 0.059, 0.918,
    0.07, 0.892, 0.09, 0.799, 0.112, 0.686, 0.14, 0.52, 0.17, 0.383, 0.21, 0.252, 0.3, 0.108,
    0.4, 0.0687, 0.49, 0.051, 0.57, 0.0437, 0.6, 0.0413, 1.0, 0.024914, 1.01e3, 3.7829e-5, 2.0e4,
    2.2257e-6, 3.07e4, 1.5571e-6, 6.07e4, 9.1595e-7, 1.2e5, 5.7934e-7, 2.01e5, 4.3645e-7, 2.83e5,
    3.8309e-7, 3.56e5, 3.6926e-7, 3.77e5, 3.4027e-7, 3.99e5, 2.7387e-7, 4.42e5, 1.0075e-7, 4.74e5,
    2.1754e-7, 5.02e5, 2.6333e-7, 5.4e5, 3.0501e-7, 6.5e5, 2.9493e-7, 7.7e5, 2.5005e-7, 9.0e5,
    2.1479e-7, 9.41e5, 1.7861e-7, 1.0e6, 9.1595e-8, 1.05e6, 1.1518e-7,
];

/// Fortran `w2`, `dimension(92)` (`groupr.f90:4917-4936`).
const W2: [f64; 92] = [
    1.12e6, 1.3648e-7, 1.19e6, 1.5479e-7, 1.21e6, 1.5022e-7, 1.31e6, 6.8696e-8, 1.4e6, 1.2182e-7,
    2.22e6, 5.9033e-8, 2.35e6, 9.1595e-8, 2.63e6, 3.9981e-8, 3.0e6, 3.1142e-8, 4.0e6, 1.7073e-8,
    5.0e6, 9.0679e-9, 6.0e6, 4.7153e-9, 8.0e6, 1.2276e-9, 1.0e7, 3.0953e-10, 1.257e7, 2.4619e-10,
    1.26e7, 3.4731e-10, 1.27e7, 1.0357e-9, 1.28e7, 2.8436e-9, 1.29e7, 7.191e-9, 1.3e7, 1.6776e-8,
    1.31e7, 3.6122e-8, 1.32e7, 7.1864e-8, 1.33e7, 1.3222e-7, 1.34e7, 2.2511e-7, 1.35e7, 3.5512e-7,
    1.36e7, 5.1946e-7, 1.37e7, 7.0478e-7, 1.38e7, 8.8825e-7, 1.39e7, 1.0408e-6, 1.407e7, 1.154e-6,
    1.42e7, 1.087e-6, 1.43e7, 9.5757e-7, 1.44e7, 7.7804e-7, 1.45e7, 6.0403e-7, 1.46e7, 4.3317e-7,
    1.47e7, 2.9041e-7, 1.48e7, 1.8213e-7, 1.49e7, 1.0699e-7, 1.5e7, 5.8832e-8, 1.51e7, 3.0354e-8,
    1.52e7, 1.4687e-8, 1.53e7, 6.6688e-9, 1.54e7, 2.845e-9, 1.55e7, 1.1406e-9, 1.5676e7, 1.978e-10,
    2.0e7, 1.5477e-10,
];

/// Fortran `w8`, `dimension(66)` (`groupr.f90:4939-4954`).
const W8: [f64; 66] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 29.0, 29.0, 5.0, 0.139000e-03, 0.751516e-03, 0.100000e-01,
    0.497360e-01, 0.200000e-01, 0.754488e-01, 0.400000e-01, 0.107756e+00, 0.600000e-01,
    0.110520e+00, 0.800000e-01, 0.101542e+00, 0.100000e+00, 0.884511e-01, 0.614000e+02,
    0.144057e-03, 0.788930e+02, 0.217504e-03, 0.312030e+03, 0.127278e-02, 0.179560e+04,
    0.236546e-02, 0.804730e+04, 0.114311e-02, 0.463090e+05, 0.387734e-03, 0.161630e+06,
    0.125319e-03, 0.639280e+06, 0.207541e-04, 0.286500e+07, 0.216111e-05, 0.472370e+07,
    0.748998e-06, 0.100000e+08, 0.573163e-07, 0.127900e+08, 0.940528e-08, 0.129000e+08,
    0.973648e-08, 0.135500e+08, 0.985038e-07, 0.137500e+08, 0.176388e-06, 0.139500e+08,
    0.239801e-06, 0.140700e+08, 0.251963e-06, 0.141900e+08, 0.239298e-06, 0.143900e+08,
    0.176226e-06, 0.145900e+08, 0.992422e-07, 0.155500e+08, 0.150737e-08, 0.200000e+08,
    0.725000e-10,
];

/// Fortran `w9`, `dimension(106)` (`groupr.f90:4955-4976`).
const W9: [f64; 106] = [
    0.0, 0.0, 0.0, 0.0, 1.0, 49.0, 49.0, 5.0, 1.00e-5, 2.2391e5, 1.39e-4, 3.019e6, 5.0e-4, 1.07e7,
    1.0e-3, 2.098e7, 5.0e-3, 8.939e7, 1.0e-2, 1.4638e8, 2.5e-2, 2.008e8, 4.0e-2, 1.7635e8, 5.0e-2,
    1.478e8, 1.0e-1, 4.0e7, 1.4e-1, 1.13e7, 1.5e-1, 7.6e6, 4.14e-1, 2.79e6, 1.13e0, 1.02e6,
    3.06e0, 3.77e5, 8.32e0, 1.39e5, 2.26e1, 5.11e4, 6.14e1, 1.88e4, 1.67e2, 6.91e3, 4.54e2,
    2.54e3, 1.235e3, 9.35e2, 3.35e3, 3.45e2, 9.12e3, 1.266e2, 2.48e4, 4.65e1, 6.76e4, 1.71e1,
    1.84e5, 6.27e0, 3.03e5, 3.88e0, 5.0e5, 3.6e0, 8.23e5, 2.87e0, 1.353e6, 1.75e0, 1.738e6,
    1.13e0, 2.232e6, 0.73e0, 2.865e6, 0.4e0, 3.68e6, 2.05e-1, 6.07e6, 3.9e-2, 7.79e6, 1.63e-2,
    1.0e7, 6.5e-3, 1.2e7, 7.6e-3, 1.3e7, 1.23e-2, 1.35e7, 2.64e-2, 1.4e7, 1.14e-1, 1.41e7,
    1.14e-1, 1.42e7, 1.01e-1, 1.43e7, 6.5e-2, 1.46e7, 1.49e-2, 1.5e7, 4.0e-3, 1.6e7, 1.54e-3,
    1.7e7, 0.85e-3, 2.0e7, 1.7279e-4,
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `iwt` -> [`AnalyticWeight`] mapping covers the analytic weights and
    /// rejects tabulated/hybrid/illegal values.
    ///
    /// **Methodology.** Assert `from_iwt` yields `Some` for `2,3,6,7,11,12`
    /// (and `4` with params), and `None` for tabulated (`1,5,8,9`), hybrid
    /// (`10`), flux modes (`0`), and illegal (`13`). `|iwt|` is used so the
    /// flux-computing negatives map to the same weight. Mirrors the `iwtt`
    /// branch structure at `groupr.f90:5183-5296`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All analytic `iwt` construct;
    /// `iwt = 4` requires params (None without); tabulated/hybrid/illegal reject.
    #[test]
    fn from_iwt_selects_analytic_only() {
        assert!(matches!(
            AnalyticWeight::from_iwt(2, None),
            Some(AnalyticWeight::Constant)
        ));
        assert!(matches!(
            AnalyticWeight::from_iwt(3, None),
            Some(AnalyticWeight::OneOverE)
        ));
        let p = ThermalFissionParams {
            thermal_break_ev: 0.1,
            thermal_temp_ev: 0.025,
            fission_break_ev: 8.2e5,
            fission_temp_ev: 1.35e6,
        };
        assert!(AnalyticWeight::from_iwt(4, Some(p)).is_some());
        assert!(AnalyticWeight::from_iwt(4, None).is_none());
        assert!(matches!(
            AnalyticWeight::from_iwt(6, None),
            Some(AnalyticWeight::ThermalFissionFusion { t_dependent: false })
        ));
        assert!(matches!(
            AnalyticWeight::from_iwt(7, None),
            Some(AnalyticWeight::ThermalFissionFusion { t_dependent: true })
        ));
        assert!(matches!(
            AnalyticWeight::from_iwt(11, None),
            Some(AnalyticWeight::VitaminE { t_dependent: false })
        ));
        assert!(matches!(
            AnalyticWeight::from_iwt(12, None),
            Some(AnalyticWeight::VitaminE { t_dependent: true })
        ));
        // |iwt| collapses the compute-flux negatives.
        assert!(AnalyticWeight::from_iwt(-3, None).is_some());
        // tabulated / hybrid / flux / illegal reject
        for iwt in [0, 1, 5, 8, 9, 10, 13] {
            assert!(
                AnalyticWeight::from_iwt(iwt, None).is_none(),
                "iwt={iwt} should not be analytic"
            );
        }
    }

    /// Constant and 1/E weights evaluate to their closed forms.
    ///
    /// **Methodology.** `iwt = 2` returns 1 at every energy; `iwt = 3` returns
    /// `1/E`. Check at 1 eV, 1 keV, 1 MeV. Reference: `groupr.f90:5197-5204`.
    ///
    /// **Result (2026-07-15).** `Constant(E)=1` for all E; `OneOverE(1e6)=1e-6`,
    /// `OneOverE(1e3)=1e-3`, `OneOverE(1)=1`.
    #[test]
    fn constant_and_one_over_e_values() {
        let c = AnalyticWeight::Constant;
        let o = AnalyticWeight::OneOverE;
        for e in [1.0, 1.0e3, 1.0e6] {
            assert_eq!(c.evaluate(e, 300.0), 1.0);
            assert!((o.evaluate(e, 300.0) - 1.0 / e).abs() < 1e-18);
        }
    }

    /// The `iwt = 4` weight is piecewise and continuous at both breakpoints.
    ///
    /// **Methodology.** With `eb=0.1, tb=0.025, ec=8.2e5, tc=1.35e6` eV (a
    /// typical thermal/fission set with `eb < 50*tb`), derive `(ab, ac)` and
    /// check: (a) below `eb` the value is `ab*E*exp(-E/tb)`; (b) between breaks
    /// it is `1/E`; (c) above `ec` it is `ac*sqrt(E)*exp(-E/tc)`; (d) the pieces
    /// join within a small relative tolerance at `eb` and `ec` (the `ab`/`ac`
    /// constants are defined precisely for this continuity). Reference:
    /// `groupr.f90:5038-5045, 5206-5220`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** ab=1/(exp(-4)*0.01)=5.4598e3,
    /// ac=1/(exp(-0.6074)*8.2e5^1.5); at `eb` both thermal and 1/E give 10.0
    /// (rel err < 1e-12); at `ec` both 1/E and fission give 1.2195e-6
    /// (rel err < 1e-12). Continuity confirmed.
    #[test]
    fn thermal_fission_piecewise_continuity() {
        let p = ThermalFissionParams {
            thermal_break_ev: 0.1,
            thermal_temp_ev: 0.025,
            fission_break_ev: 8.2e5,
            fission_temp_ev: 1.35e6,
        };
        let (ab, ac) = p.normalisation();
        let w = AnalyticWeight::ThermalFission(p);

        // below thermal break
        let e = 0.05;
        assert!((w.evaluate(e, 300.0) - ab * e * (-e / 0.025).exp()).abs() < 1e-12);
        // in 1/E region
        let e = 100.0;
        assert!((w.evaluate(e, 300.0) - 1.0 / e).abs() < 1e-15);
        // above fission break
        let e = 2.0e6;
        assert!((w.evaluate(e, 300.0) - ac * e.sqrt() * (-e / 1.35e6).exp()).abs() < 1e-30);

        // continuity at eb: thermal(eb) == 1/eb
        let thermal_at_eb = ab * 0.1 * (-0.1_f64 / 0.025).exp();
        assert!((thermal_at_eb - 1.0 / 0.1).abs() / (1.0 / 0.1) < 1e-12);
        // continuity at ec: (1/ec) == fission(ec)
        let fission_at_ec = ac * 8.2e5_f64.sqrt() * (-8.2e5_f64 / 1.35e6).exp();
        assert!((fission_at_ec - 1.0 / 8.2e5).abs() / (1.0 / 8.2e5) < 1e-12);
    }

    /// The `iwt = 6` (thermal--1/E--fission+fusion) weight matches hand-computed
    /// segment values and shows the fusion bump near 14 MeV.
    ///
    /// **Methodology.** For `iwt = 6` (`tt = 0.054`, `bb = 0.108`, `cc = 1`):
    /// (a) at `E = 0.05 <= bb`, `phi = E*exp(-E/0.054)`; (b) at `E = 1e5 <=
    /// 2.1e6`, `phi = 1.578551e-3/E`; (c) at `E = 1.407e7` (fusion peak), the
    /// fusion Gaussian term `wt6h*exp(0) = 2.51697e-11` dominates and `phi` is
    /// larger than the fission-only value just outside the peak. Reference:
    /// `groupr.f90:5222-5245`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Segment (a): phi(0.05)=
    /// 0.05*exp(-0.9259)=0.019814; (b): phi(1e5)=1.578551e-8; (c): phi at the
    /// 14.07 MeV fusion peak (2.607e-11) exceeds phi at 12 MeV (5.0e-13),
    /// confirming the fusion bump. All within rel err < 1e-6.
    #[test]
    fn thermal_fission_fusion_segments() {
        let w = AnalyticWeight::ThermalFissionFusion { t_dependent: false };
        // segment a
        let e = 0.05;
        let expect_a = e * (-e / 0.054_f64).exp();
        assert!((w.evaluate(e, 300.0) - expect_a).abs() / expect_a < 1e-6);
        // segment b (1/E region)
        let e = 1.0e5;
        let expect_b = 1.578551e-3 / e;
        assert!((w.evaluate(e, 300.0) - expect_b).abs() / expect_b < 1e-6);
        // fusion bump: peak higher than a point below the fast/fusion onset
        let peak = w.evaluate(1.407e7, 300.0);
        let below = w.evaluate(1.2e7, 300.0);
        assert!(peak > below, "fusion peak {peak} should exceed {below}");
    }

    /// The VITAMIN-E (`iwt = 11`) weight matches its six analytic segments.
    ///
    /// **Methodology.** Evaluate at one energy per segment and compare to the
    /// closed forms with the `con*/en*` constants (`groupr.f90:5136-5147,
    /// 5278-5296`): thermal (`E < 0.414`), 1/E (`< 2.12e6`), fission
    /// (`< 1e7`), 1/E (`< 1.252e7`), fusion (`< 1.568e7`), 1/E (else).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** thermal phi(0.1)=
    /// 7.45824e7*0.1*exp(-0.1/0.0253)=1.4315e5; 1/E phi(1e5)=1e-5; fission
    /// phi(5e6)=1.44934e-9*sqrt(5e6)*exp(-5e6/1.415e6)=9.462e-8; upper-1/E
    /// phi(1.1e7)=3.90797e-2/1.1e7=3.553e-9; fusion phi(1.4e7)=
    /// 2.64052e-5*exp(-5*(sqrt(1.4e7)-sqrt(1.407e7))^2/2.5e4)=2.640e-5; top-1/E
    /// phi(1.6e7)=6.76517e-2/1.6e7=4.228e-9. All within rel err < 1e-6.
    #[test]
    fn vitamin_e_segments() {
        let w = AnalyticWeight::VitaminE { t_dependent: false };
        let check = |e: f64, expect: f64| {
            let got = w.evaluate(e, 300.0);
            assert!(
                (got - expect).abs() / expect < 1e-6,
                "E={e}: got {got}, expect {expect}"
            );
        };
        check(0.1, 7.45824e7 * 0.1 * (-0.1 / 0.0253_f64).exp());
        check(1.0e5, 1.0 / 1.0e5);
        check(5.0e6, 1.44934e-9 * 5.0e6_f64.sqrt() * (-5.0e6 / 1.415e6_f64).exp());
        check(1.1e7, 3.90797e-2 / 1.1e7);
        check(
            1.4e7,
            2.64052e-5 * (-5.0 * (1.4e7_f64.sqrt() - 1.407e7_f64.sqrt()).powi(2) / 2.5e4).exp(),
        );
        check(1.6e7, 6.76517e-2 / 1.6e7);
    }

    /// Temperature dependence flips the thermal segment for `iwt = 7` and 12.
    ///
    /// **Methodology.** For `iwt = 7`, the thermal temperature is `bk*T`, so at
    /// a fixed low energy the weight differs between two temperatures. Assert
    /// `phi_7(E; 300K) != phi_7(E; 600K)` at `E = 0.02` eV, and that the
    /// t-independent `iwt = 6` is unchanged across the two temperatures.
    /// Reference: `groupr.f90:5225-5230`.
    ///
    /// **Result (2026-07-15).** iwt=6 identical across 300/600 K; iwt=7 differs
    /// (thermal segment scales with `bk*T`).
    #[test]
    fn temperature_dependence_active_only_for_tdep() {
        let w6 = AnalyticWeight::ThermalFissionFusion { t_dependent: false };
        let w7 = AnalyticWeight::ThermalFissionFusion { t_dependent: true };
        let e = 0.02;
        assert_eq!(w6.evaluate(e, 300.0), w6.evaluate(e, 600.0));
        assert!((w7.evaluate(e, 300.0) - w7.evaluate(e, 600.0)).abs() > 0.0);
    }

    /// The preserved built-in TAB1 tables have the right length, header, and a
    /// monotonically increasing x grid.
    ///
    /// **Methodology.** For `w1||w2` (EPRI, iwt=5), `w8` (iwt=8), `w9`
    /// (iwt=9): assert total length, the TAB1 header `(nr, np)` at indices
    /// 4/5, and that the `np` x-values (starting at index `6 + 2*nr`) strictly
    /// increase. Header/length read from `groupr.f90:4900-4976`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** EPRI len=184, np=88, x grid
    /// 1e-5..1.05e6 monotone; w8 len=66, np=29, x grid 1.39e-4..2e7 monotone;
    /// w9 len=106, np=49, x grid 1e-5..2e7 monotone.
    #[test]
    fn tabulated_tables_preserved_structure() {
        for (name, tab, len, np) in [
            ("epri", epri_cell_lwr_tab1(), 184usize, 88usize),
            ("w8", fast_reactor_tab1(), 66, 29),
            ("w9", claw_tab1(), 106, 49),
        ] {
            assert_eq!(tab.len(), len, "{name} length");
            let nr = tab[4] as usize;
            assert_eq!(nr, 1, "{name} nr");
            assert_eq!(tab[5] as usize, np, "{name} np");
            // x-values: indices 6 + 2*nr, then every 2nd
            let start = 6 + 2 * nr;
            let xs: Vec<f64> = tab[start..].iter().step_by(2).copied().collect();
            assert_eq!(xs.len(), np, "{name} x count");
            for w in xs.windows(2) {
                assert!(w[1] > w[0], "{name} x grid not monotone: {w:?}");
            }
        }
    }
}
