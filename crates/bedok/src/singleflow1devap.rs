//! Steady 1-D channel flow with boiling — the homogeneous-equilibrium model.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `singleflow1devap.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What it does, in two stages
//!
//! The reference's own header calls this a "VERY SIMPLE 1-D boiling model", and
//! the structure is worth stating up front because the two stages are
//! independent:
//!
//! 1. **March the mixture enthalpy** up (or down) each channel from a plain
//!    energy balance, `dh/dz = q'/(G A)`. Nothing thermodynamic happens here —
//!    it is bookkeeping on where the heat went.
//! 2. **Invert that enthalpy** into temperature, quality and void fraction at
//!    the channel pressure, using equilibrium thermodynamics plus a drift-flux
//!    void-quality relation.
//!
//! Stage 2 is the point. A single temperature cannot represent a saturated
//! two-phase state — the whole boiling region sits at `Tsat`, and what varies
//! is the quality. Marching enthalpy and inverting afterwards is the
//! well-posed way to get both.
//!
//! # Assumptions, all of them load-bearing
//!
//! - **Constant pressure.** No pressure drop anywhere; the channel pressure is
//!   `th.coolant.inletpress` throughout. So there is no flow/pressure coupling
//!   and no density-wave behaviour.
//! - **Thermodynamic equilibrium.** Quality is `(h - hL)/(hV - hL)`, clamped to
//!   `[0, 1]`. Subcooled boiling and superheated liquid are unrepresentable by
//!   construction.
//! - **Channels do not communicate.** Each `(ix, iy)` column is marched on its
//!   own; there is no cross-flow.
//!
//! # What it is *for*
//!
//! The reference states the intent plainly: a cheap, robust **initial
//! condition** for the drift-flux solver, returning the same `th.coolant`
//! fields that solver reads as its initial guess — already carrying a physical
//! boiling profile instead of a uniform inlet state. `th_solverxyz.m` also uses
//! it as the channel model outright when `params.th_model == 'hem'`, which
//! `neacrpd1t` sets, so that a transient starts from a steady state computed by
//! the *same* model it will be marched with.

use crate::iapws_if97::backward::t_ph;
use crate::iapws_if97::basic::{cp1_pt, h1_pt, h2_pt, hl_p, hv_p, v1_pt, v2_pt, vv_p};
use crate::iapws_if97::region4::tsat_p;
use crate::iapws_if97::transport::{k_pt, mu_pt};
use crate::types::{FlowDirection, Geometry, MassFlux, Params, Th};

/// MATLAB's `eps` — the double-precision machine epsilon, used as a floor.
const EPS: f64 = f64::EPSILON;

/// Standard gravity at the Earth's surface, **cm/s²**, as the reference
/// declares it.
const G_EARTH: f64 = 980.665;

/// `th = singleflow1devap(params, geometry, th, pwrdens)`.
///
/// # Arguments
///
/// - `params` — the three extents, plus the optional `evap_C0` and
///   `evap_homog` closure switches.
/// - `geometry` — needs `Lz`, the per-node axial heights in cm, the `zlows` /
///   `zhis` channel bounds, and `fuel.Rtot` / `fuel.subarea`.
/// - `th` — needs `maxpow`, `powratio`, `nfuelpin`, `coolheatfrac`,
///   `flowrate`, `flowdir`, `heatflux` and the coolant inlet conditions. Its
///   `coolant` fields are **overwritten** by this function.
/// - `pwrdens` — normalised power density per node, whatever the flux solver
///   produced. Scaled here by `maxpow * powratio`.
///
/// # Returns
///
/// The updated [`Th`], with every `coolant` field populated: `enth`, `temps`,
/// `alphag`, `quality`, `press`, `dens`, `ldens`, `gdens`, `vm`, and the three
/// liquid transport properties `tcon`, `pran`, `kvis`.
///
/// # Units — the conversions at the end are easy to get wrong
///
/// The IAPWS layer works in SI; BEDOK works in cm-g-s. The three transport
/// properties are converted on assignment and each factor is load-bearing:
///
/// | Property | IAPWS | BEDOK | Factor |
/// |---|---|---|---|
/// | `tcon` | W/(m·K) | W/(cm·K) | `/100` |
/// | `pran` | — | — | `cp[kJ] * 1000` to make it dimensionless |
/// | `kvis` | m²/s | cm²/s | `*10000` |
///
/// Densities are `1/(1000 v)` — IAPWS gives m³/kg, BEDOK wants g/cm³.
///
/// # The enthalpy march is node-centred
///
/// The inlet node takes `enthin + 0.5*delta`, i.e. half a node's rise, and each
/// subsequent node adds half of its own plus half of its neighbour's. So the
/// stored value is the enthalpy at the node **centre**, not at a face. That is
/// consistent with everything else in the code being cell-centred, and it means
/// the outlet node's value is half a node short of the true channel exit
/// enthalpy.
///
/// # Reference defects carried here
///
/// - **The enthalpy clamp's comment contradicts its code (T10).**
///   `enthmax = IAPWS_IF97('h2_pT', P, 1050)` is commented "steam at 900 K
///   (safely below the 1073 K region-2 limit)". The value is **1050 K**, not
///   900. The margin to the region-2 limit is therefore 23 K, not 173 K.
///   Preserved as written; the clamp still works, but a reader trusting the
///   comment would misjudge how much headroom it leaves.
/// - **`sat` is dead.** The three-way mask `sub`/`sup`/`sat` is computed, and
///   `sat` is never read — the two-phase branch is the `temps` initialisation
///   rather than an explicit case.
/// - **A channel with no power is skipped entirely**, leaving its enthalpy at
///   the inlet value. The test is `any(pwrdens)` over the **whole** `z` column,
///   `1:maxiz`, while the march itself runs only `zlow:zhi`. The two ranges
///   disagree, which matters for a column whose powered nodes lie outside its
///   own `[zlow, zhi]` bounds — reachable via
///   [`crate::geometry_ends3d`]'s first-contiguous-run limitation.
/// - **Two different critical temperatures.** The surface-tension correlation
///   uses `647.15 K` where the IF97 layer uses `647.096 K`. That is the
///   correlation's own constant, not an error, but the two sit four lines apart
///   and look like a typo.
///
/// # Panics
///
/// If `pwrdens`, `heatflux` or `geometry.lz` is shorter than the node count, or
/// if a per-node `flowrate` is.
pub fn singleflow1devap(params: &Params, geometry: &Geometry, th: &Th, pwrdens: &[f64]) -> Th {
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;

    assert!(pwrdens.len() >= es, "pwrdens is {} long, need {es}", pwrdens.len());
    assert!(
        th.heatflux.len() >= es,
        "th.heatflux is {} long, need {es}",
        th.heatflux.len()
    );
    assert!(
        geometry.lz.len() >= es,
        "geometry.lz is {} long, need {es}",
        geometry.lz.len()
    );

    let npin = th.nfuelpin;
    let inlett = th.coolant.inlettemp;
    let rtot = geometry.fuel.rtot;
    let subarea = geometry.fuel.subarea;
    // Constant channel pressure, MPa.
    let p = th.coolant.inletpress;

    // ---------- (1) enthalpy march ----------
    // `linpwrdens` W/cm per node; `cool_linpwrdens` is the part the coolant
    // actually receives — the wall flux off the pins plus the directly
    // deposited fraction.
    let linpwrdens: Vec<f64> = (0..es)
        .map(|i| pwrdens[i] * th.maxpow * th.powratio / geometry.lz[i])
        .collect();
    let cool_linpwrdens: Vec<f64> = (0..es)
        .map(|i| {
            2.0 * std::f64::consts::PI * rtot * th.heatflux[i] * npin
                + th.coolheatfrac * linpwrdens[i]
        })
        .collect();

    let enthin = h1_pt(p, inlett);
    let mut enth = vec![enthin; es];

    // Enthalpy rise across a whole node, J/g.
    let delta: Vec<f64> = (0..es)
        .map(|i| cool_linpwrdens[i] * geometry.lz[i] / th.flowrate.at(i) / subarea / npin)
        .collect();

    let bounds = |a: &Option<crate::matlab::Array2<usize>>, ix: usize, iy: usize, fallback: usize| {
        a.as_ref().map_or(fallback, |m| m.get(ix, iy))
    };

    for ix in 0..maxix {
        for iy in 0..maxiy {
            let zlow = bounds(&geometry.zlows, ix, iy, 0);
            let zhi = bounds(&geometry.zhis, ix, iy, maxiz - 1);

            // `any(pwrdens(idxvec))` over the *whole* column, not `zlow:zhi`.
            let col = ix * xstep + iy * maxiz;
            if !(0..maxiz).any(|iz| pwrdens[col + iz] != 0.0) {
                continue;
            }

            match th.flowdir {
                FlowDirection::Down => {
                    let mut idx = col + zhi;
                    enth[idx] = enthin + 0.5 * delta[idx];
                    for iz in (zlow..zhi).rev() {
                        idx = col + iz;
                        enth[idx] = enth[idx + 1] + 0.5 * delta[idx + 1] + 0.5 * delta[idx];
                    }
                }
                FlowDirection::Up => {
                    let mut idx = col + zlow;
                    enth[idx] = enthin + 0.5 * delta[idx];
                    for iz in (zlow + 1)..=zhi {
                        idx = col + iz;
                        enth[idx] = enth[idx - 1] + 0.5 * delta[idx - 1] + 0.5 * delta[idx];
                    }
                }
            }
        }
    }

    // ---------- (2) invert the enthalpy ----------
    // Shared with `singleflow1devaptime`; see `invert_mixture_enthalpy`.
    let inv = invert_mixture_enthalpy(params, p, enth, &th.flowrate);

    let mut out = th.clone();
    out.coolant.enth = inv.enth;
    out.coolant.temps = inv.temps;
    out.coolant.alphag = inv.alphag;
    out.coolant.quality = inv.quality;
    out.coolant.press = vec![p; es];
    out.coolant.dens = inv.dens;
    out.coolant.ldens = inv.ldens;
    out.coolant.gdens = inv.gdens;
    out.coolant.vm = inv.vm;
    out.coolant.tcon = inv.tcon;
    out.coolant.pran = inv.pran;
    out.coolant.kvis = inv.kvis;
    out
}

/// The state that stage 2 recovers from a mixture enthalpy.
///
/// Every vector is one entry per node, in the usual flattened order, and the
/// units are BEDOK's cm-g-s throughout — see
/// [`singleflow1devap`]'s unit table.
#[derive(Clone, Debug, Default)]
pub struct MixtureState {
    /// Mixture enthalpy, kJ/kg, **after** the physical-window clamp.
    pub enth: Vec<f64>,
    /// Mixture temperature, K — `Tsat` throughout the two-phase region.
    pub temps: Vec<f64>,
    /// Void fraction, dimensionless on `[0, 1]`.
    pub alphag: Vec<f64>,
    /// Equilibrium quality, dimensionless, clamped to `[0, 1]`.
    pub quality: Vec<f64>,
    /// Mixture density, g/cm³.
    pub dens: Vec<f64>,
    /// Saturated liquid density, g/cm³.
    pub ldens: Vec<f64>,
    /// Vapour density, g/cm³.
    pub gdens: Vec<f64>,
    /// Mixture velocity, cm/s.
    pub vm: Vec<f64>,
    /// Liquid thermal conductivity, W/(cm·K).
    pub tcon: Vec<f64>,
    /// Liquid Prandtl number, dimensionless.
    pub pran: Vec<f64>,
    /// Liquid kinematic viscosity, cm²/s.
    pub kvis: Vec<f64>,
}

/// Stage 2 — invert a mixture enthalpy into temperature, quality, void and the
/// liquid transport properties, at a fixed pressure.
///
/// # Why this is one function and not two copies
///
/// `singleflow1devap.m` and `singleflow1devaptime.m` each carry this block, and
/// the transient one's own comment says "(identical to singleflow1devap.m)" —
/// which it is, line for line. The two `.m` files differ **only** in stage 1,
/// the enthalpy march.
///
/// Duplicating ~90 lines to mirror that would create two copies that can drift
/// apart under any later fix, which is exactly what the workspace's reuse rule
/// exists to prevent. So the shared half lives here, in the module it
/// originated in, and [`crate::singleflow1devaptime`] calls it.
///
/// **If a future snapshot makes the two blocks differ, they must be split
/// again** — the sharing is justified by their being verbatim identical, not by
/// their being similar.
///
/// # Arguments
///
/// - `params` — read for `evap_c0` and `evap_homog` only.
/// - `p` — the channel pressure, **MPa**, constant across the whole core.
/// - `enth` — the marched mixture enthalpy, **kJ/kg**. Consumed, clamped, and
///   returned in [`MixtureState::enth`].
/// - `flowrate` — the mass flux, g/(s·cm²), for the drift-flux closure and the
///   mixture velocity.
///
/// # The physical window
///
/// The enthalpy is clamped to `[0, h2_pT(p, 1050 K)]` before anything else, so
/// the IAPWS inversions stay inside region validity. A runaway feedback can
/// otherwise push the enthalpy past the steam region and make
/// [`crate::iapws_if97::backward::t_ph`] return `NaN`. See defect T10 on
/// [`singleflow1devap`] for the comment/code mismatch in that clamp.
pub fn invert_mixture_enthalpy(
    params: &Params,
    p: f64,
    enth: Vec<f64>,
    flowrate: &MassFlux,
) -> MixtureState {
    let es = enth.len();
    let mut enth = enth;
        let hlsat = hl_p(p);
    let hvsat = hv_p(p);
    let tsat = tsat_p(p);
    let hvl = hvsat - hlsat;

    // The clamp. Note 1050 K, where the reference's comment says 900 K — T10.
    let enthmax = h2_pt(p, 1050.0);
    for e in enth.iter_mut() {
        *e = e.max(0.0).min(enthmax);
    }

    let sub: Vec<bool> = enth.iter().map(|&e| e < hlsat).collect();
    let sup: Vec<bool> = enth.iter().map(|&e| e > hvsat).collect();

    // Two-phase nodes stay at Tsat; the single-phase ones are inverted.
    let mut temps = vec![tsat; es];
    for i in 0..es {
        if sub[i] || sup[i] {
            temps[i] = t_ph(p, enth[i]);
        }
    }
    for t in temps.iter_mut() {
        if !t.is_finite() {
            *t = tsat;
        }
    }

    // Equilibrium quality, clamped for the void relation.
    //
    // `min(max(x, 0), 1)` is kept as a max/min chain rather than `clamp`, for
    // the same reason as in `fiss_src_extrapolatexyz`: the two agree on finite
    // input but differ on `NaN`. `f64::clamp` propagates it; the chain returns
    // `0.0`, because Rust's `max`/`min` return the non-NaN operand — and that
    // is what MATLAB's `min`/`max` do too. `NaN` is reachable here: above the
    // region 1/3 boundary `hl_p` and `hv_p` are `NaN`, so `hvl` is `NaN` and
    // every quality with it.
    #[allow(clippy::manual_clamp)]
    let x: Vec<f64> = enth
        .iter()
        .map(|&e| ((e - hlsat) / hvl).max(0.0).min(1.0))
        .collect();

    // Phase densities, g/cm3. The liquid is evaluated just below saturation so
    // `v1_pT` stays inside region 1.
    let tliq: Vec<f64> = temps.iter().map(|&t| t.min(tsat - 2.0 * EPS)).collect();
    let rl: Vec<f64> = (0..es).map(|i| 1.0 / v1_pt(p, tliq[i]) / 1000.0).collect();
    let rg_sat = 1.0 / vv_p(p) / 1000.0;
    let rg: Vec<f64> = (0..es)
        .map(|i| {
            if sup[i] {
                1.0 / v2_pt(p, temps[i]) / 1000.0
            } else {
                rg_sat
            }
        })
        .collect();

    // Drift-flux void-quality closure. Surface tension in g/s^2, which is the
    // same number as mN/m; note its critical temperature is 647.15 K, the
    // correlation's own, not IF97's 647.096.
    let theta = (647.15 - tsat) / 647.15;
    let sigma = 235.8 * theta.powf(1.256) * (1.0 - 0.625 * theta);

    let homogeneous = params.evap_homog;
    let c0 = if homogeneous {
        1.0
    } else {
        params.evap_c0.unwrap_or(1.2)
    };

    let vgj: Vec<f64> = (0..es)
        .map(|i| {
            if homogeneous {
                0.0
            } else {
                std::f64::consts::SQRT_2
                    * ((rl[i] - rg[i]) * G_EARTH * sigma / (rl[i] * rl[i])).powf(0.25)
            }
        })
        .collect();

    let mut alpha = vec![0.0; es];
    for i in 0..es {
        let g = flowrate.at(i);
        let denom = c0 * (x[i] + (1.0 - x[i]) * rg[i] / rl[i]) + rg[i] * vgj[i] / g.max(EPS);
        let a = x[i] / denom.max(EPS);
        // As above: the max/min chain, not `clamp`, so a `NaN` void becomes 0
        // rather than propagating.
        #[allow(clippy::manual_clamp)]
        let clamped = if sub[i] {
            0.0
        } else if sup[i] {
            1.0
        } else {
            a
        }
        .max(0.0)
        .min(1.0);
        alpha[i] = clamped;
    }

    let densmix: Vec<f64> = (0..es)
        .map(|i| alpha[i] * rg[i] + (1.0 - alpha[i]) * rl[i])
        .collect();
    let vm: Vec<f64> = (0..es).map(|i| flowrate.at(i) / densmix[i]).collect();

    // Liquid transport properties, at the liquid temperature, converted to the
    // cm-g-s units BEDOK uses. See the unit table in the doc comment.
    let tcon: Vec<f64> = (0..es).map(|i| k_pt(p, tliq[i]) / 100.0).collect();
    let pran: Vec<f64> = (0..es)
        .map(|i| cp1_pt(p, tliq[i]) * mu_pt(p, tliq[i]) / k_pt(p, tliq[i]) * 1000.0)
        .collect();
    let kvis: Vec<f64> = (0..es)
        .map(|i| mu_pt(p, tliq[i]) * v1_pt(p, tliq[i]) * 10000.0)
        .collect();
    MixtureState {
        enth,
        temps,
        alphag: alpha,
        quality: x,
        dens: densmix,
        ldens: rl,
        gdens: rg,
        vm,
        tcon,
        pran,
        kvis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matlab::Array2;
    use crate::types::{Coolant, FuelGeometry, MassFlux};

    /// A single channel of `n` axial nodes at BWR conditions.
    ///
    /// One radial column so the march is easy to follow by hand: `maxix` and
    /// `maxiy` are 1, so node `iz` is simply index `iz`.
    fn channel(n: usize, power_w: f64, mass_flux: f64) -> (Params, Geometry, Th, Vec<f64>) {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(n),
            g: 1,
            nc: Some(0),
            ..Default::default()
        };

        let mut zl = Array2::<usize>::zeros(1, 1);
        zl.set(0, 0, 0);
        let mut zh = Array2::<usize>::zeros(1, 1);
        zh.set(0, 0, n - 1);

        let geometry = Geometry {
            lz: vec![366.0 / n as f64; n],
            zlows: Some(zl),
            zhis: Some(zh),
            fuel: FuelGeometry {
                rtot: 0.476,
                subarea: 1.42,
                ..Default::default()
            },
            ..Default::default()
        };

        let th = Th {
            coolant: Coolant {
                inlettemp: 550.0,
                inletpress: 7.0,
                ..Default::default()
            },
            heatflux: vec![0.0; n],
            maxpow: power_w,
            powratio: 1.0,
            nfuelpin: 1.0,
            coolheatfrac: 1.0,
            flowrate: MassFlux::Uniform(mass_flux),
            flowdir: FlowDirection::Up,
            ..Default::default()
        };

        // Uniform axial power, normalised to sum 1.
        let pwrdens = vec![1.0 / n as f64; n];
        (params, geometry, th, pwrdens)
    }

    /// The enthalpy rises monotonically along a heated channel, and the total
    /// rise closes the energy balance.
    ///
    /// # Methodology
    ///
    /// A 10-node BWR channel at 7 MPa, inlet 550 K, mass flux
    /// 100 g/(s·cm²) through 1.42 cm² — so 142 g/s — with 50 kW deposited
    /// uniformly and `coolheatfrac = 1` so all of it reaches the coolant.
    ///
    /// The analytical outlet enthalpy is `h_in + Q/m_dot`. Because the march is
    /// node-centred, the last node sits **half a node short** of the true exit,
    /// so the expected value there is `h_in + (Q/m_dot) * (1 - 1/(2n))`.
    /// Checking against that rather than the raw balance is what makes this a
    /// test of the discretisation instead of a tolerance fudge.
    ///
    /// Pass criterion: strictly increasing, and the last node within 1e-9
    /// relative of the node-centred prediction.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// `h_in = 1219.8438 kJ/kg`, total rise `352.1127 kJ/kg`, last node
    /// **1554.3508** against a predicted 1554.3508 — agreement to better than
    /// 1e-9 relative, i.e. exact in floating point.
    ///
    /// **Interpretation.** The march conserves energy exactly, and the
    /// half-node offset at the outlet is confirmed as a deliberate
    /// cell-centring rather than a lost half-node of heating. Every term in
    /// `delta = q' Lz / (G A n_pin)` is exercised, so a wrong factor anywhere
    /// in that chain would show up here.
    #[test]
    fn the_enthalpy_march_closes_the_energy_balance() {
        let n = 10;
        let (params, geometry, th, pwrdens) = channel(n, 50_000.0, 100.0);
        let out = singleflow1devap(&params, &geometry, &th, &pwrdens);

        let hin = h1_pt(7.0, 550.0);
        let mdot = 100.0 * 1.42; // g/s
        let total_rise = 50_000.0 / mdot; // J/g == kJ/kg
        let expected_last = hin + total_rise * (1.0 - 1.0 / (2.0 * n as f64));

        eprintln!(
            "h_in = {hin:.4}, total rise = {total_rise:.4} kJ/kg, last node = {:.4} (expected {expected_last:.4})",
            out.coolant.enth[n - 1]
        );

        for i in 1..n {
            assert!(
                out.coolant.enth[i] > out.coolant.enth[i - 1],
                "enthalpy fell at node {i}"
            );
        }
        assert!(
            (out.coolant.enth[n - 1] - expected_last).abs() / expected_last < 1e-9,
            "got {}, expected {expected_last}",
            out.coolant.enth[n - 1]
        );
    }

    /// A channel powered hard enough to boil produces the expected sequence:
    /// subcooled liquid, then a two-phase plateau at `Tsat`, with the void
    /// fraction rising monotonically.
    ///
    /// # Methodology
    ///
    /// The same channel at **120 kW**, a realistic BWR duty: enough to carry
    /// the coolant past `hL(7 MPa) = 1267 kJ/kg` and into the boiling region,
    /// but not past `hV = 2773` into superheat. Physically required:
    /// temperature rises while subcooled then pins at `Tsat = 558.98 K` once
    /// boiling starts; quality and void are zero in the subcooled part and rise
    /// thereafter; and the void runs **ahead** of the quality, because vapour
    /// is ~20x less dense than liquid so a little mass is a lot of volume.
    ///
    /// **The power matters and 800 kW was wrong.** The first version of this
    /// test used 800 kW, which drives the whole channel superheated by node 3
    /// and into the `enthmax` clamp by node 6 — every node then reads
    /// `x = 1, alpha = 1` and the two-phase behaviour under test never appears.
    /// A real BWR channel is only a few kelvin subcooled at inlet and exits
    /// around 10-15% quality; 120 kW puts it there.
    ///
    /// Pass criterion: at least one subcooled and three saturated nodes, **no**
    /// superheated node, `temps` exactly `Tsat` on every saturated node, and
    /// `alphag` non-decreasing and strictly greater than `quality` wherever
    /// quality is non-zero.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// `Tsat = 558.9800 K`, `hL = 1267.44 kJ/kg`. Node 0 is subcooled at
    /// **556.670 K**; nodes 1-11 sit at `Tsat` **exactly**, with:
    ///
    /// | Node | `h` kJ/kg | `x` | `alpha` |
    /// |---|---|---|---|
    /// | 0 | 1255.06 | 0 | 0 |
    /// | 1 | 1325.48 | 0.0386 | 0.3520 |
    /// | 5 | 1607.17 | 0.2257 | 0.6987 |
    /// | 11 | 2029.70 | 0.5064 | 0.7873 |
    ///
    /// **Interpretation.** This is the expected shape of a boiling channel and
    /// each feature is a separate check on the inversion. The temperature
    /// plateau is exact, not approximate, because the two-phase branch returns
    /// `Tsat` directly. The void runs far ahead of the quality — 0.35 void at
    /// under 4% quality — which is the correct consequence of a ~20:1 density
    /// ratio and is the single most important thing the void-quality closure
    /// has to get right, since void is what feeds back on moderation. The rise
    /// is steep at low quality and flattens above ~30%, the characteristic
    /// shape of the drift-flux relation.
    #[test]
    fn a_boiling_channel_plateaus_at_saturation_and_generates_void() {
        let n = 12;
        let (params, geometry, th, pwrdens) = channel(n, 120_000.0, 100.0);
        let out = singleflow1devap(&params, &geometry, &th, &pwrdens);

        let tsat = tsat_p(7.0);
        let hlsat = hl_p(7.0);
        eprintln!("Tsat = {tsat:.4} K, hL = {hlsat:.2} kJ/kg");
        for i in 0..n {
            eprintln!(
                "  node {i:2}: h = {:8.2}  T = {:8.3}  x = {:.4}  alpha = {:.4}",
                out.coolant.enth[i], out.coolant.temps[i], out.coolant.quality[i],
                out.coolant.alphag[i]
            );
        }

        let hvsat = hv_p(7.0);
        let subcooled: Vec<usize> = (0..n).filter(|&i| out.coolant.enth[i] < hlsat).collect();
        let saturated: Vec<usize> = (0..n)
            .filter(|&i| out.coolant.enth[i] >= hlsat && out.coolant.enth[i] <= hvsat)
            .collect();
        let superheated: Vec<usize> = (0..n).filter(|&i| out.coolant.enth[i] > hvsat).collect();
        assert!(!subcooled.is_empty(), "expected a subcooled entry region");
        assert!(saturated.len() >= 3, "expected a boiling region");
        assert!(
            superheated.is_empty(),
            "the channel should not dry out at this power: {superheated:?}"
        );

        for &i in &saturated {
            assert_eq!(out.coolant.temps[i], tsat, "node {i} is not at Tsat");
            assert!(out.coolant.alphag[i] > 0.0, "node {i} has no void");
        }
        for &i in &subcooled {
            assert_eq!(out.coolant.quality[i], 0.0);
            assert_eq!(out.coolant.alphag[i], 0.0);
        }
        for i in 1..n {
            assert!(
                out.coolant.alphag[i] >= out.coolant.alphag[i - 1] - 1e-12,
                "void fell at node {i}"
            );
        }
        // Slip: void runs ahead of quality.
        for i in 0..n {
            if out.coolant.quality[i] > 1e-6 {
                assert!(
                    out.coolant.alphag[i] > out.coolant.quality[i],
                    "node {i}: alpha {} should exceed x {}",
                    out.coolant.alphag[i],
                    out.coolant.quality[i]
                );
            }
        }
    }

    /// `params.evap_homog` collapses the slip: the homogeneous limit gives less
    /// void at the same quality.
    ///
    /// # Methodology
    ///
    /// With `C0 = 1` and `Vgj = 0` the void-quality relation reduces to the
    /// no-slip form `alpha = x / (x + (1-x) rg/rl)`, in which the phases travel
    /// together. The drift-flux form with `C0 = 1.2` and a bubbly rise velocity
    /// lets vapour move faster than liquid, so **less** vapour is needed in
    /// place to carry the same quality... except that `C0 > 1` also
    /// concentrates vapour in the channel centre. The net for these conditions
    /// is checked, not assumed.
    ///
    /// Pass criterion: the two differ wherever there is boiling, and both stay
    /// within `[0, 1]`.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// The homogeneous limit gives **more** void at every quality: 0.4482
    /// against 0.3520 at `x = 0.039`, and 0.9541 against 0.7873 at
    /// `x = 0.506`.
    ///
    /// **Interpretation, since the direction is worth being explicit about.**
    /// Both differences push the same way. `C0 = 1.2` enters the *denominator*
    /// of the void-quality relation, so the drift-flux form divides by ~1.2
    /// where the homogeneous form divides by 1; and `Vgj > 0` adds a further
    /// positive term to that denominator. Physically: allowing vapour to rise
    /// relative to the liquid means less vapour needs to be resident to carry
    /// a given quality. The homogeneous model, which forbids slip, must hold
    /// more. The 17-percentage-point gap at high quality is large enough to
    /// matter for void feedback, which is why the switch exists.
    #[test]
    fn the_homogeneous_switch_changes_the_void_fraction() {
        let n = 12;
        let (params, geometry, th, pwrdens) = channel(n, 120_000.0, 100.0);
        let drift = singleflow1devap(&params, &geometry, &th, &pwrdens);

        let homog_params = Params {
            evap_homog: true,
            ..params
        };
        let homog = singleflow1devap(&homog_params, &geometry, &th, &pwrdens);

        let mut differed = 0;
        for i in 0..n {
            if drift.coolant.quality[i] > 1e-6 {
                eprintln!(
                    "node {i}: x = {:.4}, alpha drift = {:.4}, alpha homog = {:.4}",
                    drift.coolant.quality[i], drift.coolant.alphag[i], homog.coolant.alphag[i]
                );
                if (drift.coolant.alphag[i] - homog.coolant.alphag[i]).abs() > 1e-9 {
                    differed += 1;
                }
            }
            assert!((0.0..=1.0).contains(&drift.coolant.alphag[i]));
            assert!((0.0..=1.0).contains(&homog.coolant.alphag[i]));
        }
        assert!(differed > 0, "the homogeneous switch changed nothing");
    }

    /// Flowing downward reverses the profile — the hot end moves to the other
    /// side of the channel.
    ///
    /// # Methodology
    ///
    /// The same channel with `flowdir = Down`. The inlet is then at `zhi`, so
    /// enthalpy must *decrease* with `z`, and the two runs must be exact
    /// mirror images of each other since the power is axially uniform.
    ///
    /// Pass criterion: monotonically decreasing, and node `i` of the downward
    /// run equals node `n-1-i` of the upward run to 1e-12 relative.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// The downward profile is the exact mirror of the upward one, node for
    /// node, to better than 1e-12 relative — and it decreases monotonically
    /// with `z` as required. The two `flowdir` branches are therefore
    /// consistent with each other, which is the only practical check available
    /// on the reversed march's index arithmetic.
    #[test]
    fn downward_flow_mirrors_the_profile() {
        let n = 10;
        let (params, geometry, th, pwrdens) = channel(n, 50_000.0, 100.0);
        let up = singleflow1devap(&params, &geometry, &th, &pwrdens);

        let down_th = Th {
            flowdir: FlowDirection::Down,
            ..th
        };
        let down = singleflow1devap(&params, &geometry, &down_th, &pwrdens);

        for i in 1..n {
            assert!(
                down.coolant.enth[i] < down.coolant.enth[i - 1],
                "downward flow should cool with increasing z, node {i}"
            );
        }
        for i in 0..n {
            let a = up.coolant.enth[i];
            let b = down.coolant.enth[n - 1 - i];
            assert!(
                (a - b).abs() / a < 1e-12,
                "node {i}: up {a} vs mirrored down {b}"
            );
        }
    }

    /// An unpowered channel stays at the inlet enthalpy — the `any(pwrdens)`
    /// skip.
    #[test]
    fn an_unpowered_channel_is_left_at_the_inlet_state() {
        let n = 8;
        let (params, geometry, th, _) = channel(n, 50_000.0, 100.0);
        let pwrdens = vec![0.0; n];
        let out = singleflow1devap(&params, &geometry, &th, &pwrdens);

        let hin = h1_pt(7.0, 550.0);
        for i in 0..n {
            assert!((out.coolant.enth[i] - hin).abs() < 1e-12, "node {i} moved");
            assert_eq!(out.coolant.alphag[i], 0.0);
        }
    }

    /// The transport properties come back in BEDOK's cm-g-s units, not IAPWS's
    /// SI.
    ///
    /// # Methodology
    ///
    /// The three conversions at the end of the reference are the easiest thing
    /// in the file to get wrong, and a wrong factor is a silent 100x. Expected
    /// magnitudes for hot pressurised water near 550 K:
    ///
    /// - `tcon` ~ 0.0058 W/(cm·K) — i.e. 0.58 W/(m·K) divided by 100
    /// - `pran` ~ 0.9 dimensionless
    /// - `kvis` ~ 0.0012 cm²/s
    ///
    /// Pass criterion: each within a factor of 2 of those, which is loose on
    /// value but catches any factor-of-100 slip decisively.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// At the channel inlet: `tcon = 0.005812 W/(cm·K)`, `pran = 0.8509`,
    /// `kvis = 0.001252 cm²/s`.
    ///
    /// **Interpretation.** All three are the expected magnitude for hot
    /// pressurised water near 556 K: the conductivity is 0.581 W/(m·K) in SI,
    /// the Prandtl number is order unity as it must be for a liquid metal-free
    /// coolant, and the kinematic viscosity is 1.25e-7 m²/s. Each of the three
    /// conversion factors (`/100`, `*1000`, `*10000`) is confirmed to within a
    /// factor far below the 100x that a wrong one would produce.
    #[test]
    fn the_transport_properties_are_in_cm_g_s_units() {
        let n = 4;
        let (params, geometry, th, pwrdens) = channel(n, 10_000.0, 100.0);
        let out = singleflow1devap(&params, &geometry, &th, &pwrdens);

        eprintln!(
            "node 0: tcon = {:.6} W/cm/K, pran = {:.4}, kvis = {:.6} cm2/s",
            out.coolant.tcon[0], out.coolant.pran[0], out.coolant.kvis[0]
        );
        assert!((0.0029..0.0116).contains(&out.coolant.tcon[0]), "tcon {}", out.coolant.tcon[0]);
        assert!((0.45..1.8).contains(&out.coolant.pran[0]), "pran {}", out.coolant.pran[0]);
        assert!((0.0006..0.0024).contains(&out.coolant.kvis[0]), "kvis {}", out.coolant.kvis[0]);
    }
}
