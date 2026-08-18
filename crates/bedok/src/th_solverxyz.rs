//! The steady thermal-hydraulics driver — coolant, then rods, then feedback.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `th_solverxyz.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this is
//!
//! The hub of the thermal-hydraulics layer. Given a power distribution from the
//! neutronics, it runs the whole steady T-H pass in four stages:
//!
//! 1. **Normalise and collapse** the power density — divide by its 1-norm, then
//!    sum over energy groups.
//! 2. **Solve the coolant**, through whichever channel model
//!    [`crate::types::ThModel`] selects.
//! 3. **Solve every fuelled rod** with [`crate::fuelrodheat_1dcylnd`], using a
//!    Dittus-Boelter heat-transfer coefficient as the boundary condition.
//! 4. **Produce the feedback quantities** the neutronics needs back: the
//!    Doppler fuel temperature, the coolant density, and the wall heat flux
//!    that closes the loop into the next coolant solve.
//!
//! Stage 4 is why this matters. `fueltempdoppler` drives the fuel-temperature
//! cross-section feedback and `dens` the moderator-density feedback, so an
//! error here moves reactivity.
//!
//! # The wall heat flux is lagged, and that is the coupling
//!
//! `heatflux` enters stage 2 as the *previous* pass's value and is recomputed
//! in stage 3. So one call is one Picard sweep of the coolant/rod coupling, and
//! the caller iterates. That is why the channel models take `th.heatflux` as an
//! input rather than deriving it.

use crate::fuelrodheat_1dcylnd::fuelrodheat_1dcylnd;
use crate::matlab::{norm1, Array2, Array3};
use crate::types::{Geometry, Params, Th, ThModel};

/// `tmaxfuel` — the reference's default fuel-temperature ceiling, K.
///
/// The UO2 melting point.
pub const TMAX_FUEL_DEFAULT: f64 = 3100.0;

/// What happened at one node during the rod pass.
///
/// The reference signals the last of these with a `warning` and carries on;
/// returning them lets a caller count how much of the core needed rescuing,
/// which a warning stream does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOutcome {
    /// The rod was solved and its temperatures used.
    Solved,
    /// The node carries no pin power, or its column is unfuelled. Skipped.
    Skipped,
    /// The rod solve returned `NaN`. The coolant temperature (or
    /// `params.cooltempavg` if that is not finite either) was substituted and
    /// the wall heat flux zeroed.
    Rescued,
}

/// Per-node bookkeeping from the rod pass.
#[derive(Clone, Debug, Default)]
pub struct RodReport {
    /// How many nodes were solved.
    pub solved: usize,
    /// How many were skipped as unpowered or unfuelled.
    pub skipped: usize,
    /// How many needed the `NaN` rescue. **Any non-zero count here means the
    /// feedback is running on substituted temperatures.**
    pub rescued: usize,
    /// How many nodes had at least one temperature raised to the **coolant
    /// floor**.
    ///
    /// Not tracked by the reference, and **it is not an anomaly counter**: on
    /// any rod with a fuel-cladding gap this equals [`RodReport::solved`],
    /// every time. The gap node is a dummy pinned at exactly 1 K (defect T7),
    /// which is always below the coolant temperature, so the floor clamp
    /// always fires on it.
    ///
    /// That is worth knowing rather than hiding: the clamp was added as a guard
    /// against ill-conditioned conduction solves, but because it is
    /// unconditionally active it cannot serve as a signal that one occurred.
    /// [`RodReport::clamped_high`] is the one to watch.
    pub clamped_low: usize,
    /// How many nodes had a temperature cut down to `tmaxfuel`.
    ///
    /// **This one is a genuine warning.** A rod at the melting-point ceiling
    /// either is genuinely melting or, more likely, came out of an
    /// ill-conditioned conduction solve. Unlike
    /// [`RodReport::clamped_low`] it should be zero in a well-posed case.
    pub clamped_high: usize,
}

/// `th = th_solverxyz(params, geometry, th, whichsigma, pwrdens)`.
///
/// # Arguments
///
/// - `params` — `G`, the extents, `params.fuel`, and the optional `th_model`,
///   `tmaxfuel` and `cooltempavg`.
/// - `geometry` — `Lz`, the `zlows`/`zhis` bounds, and the whole
///   `geometry.fuel` rod description.
/// - `th` — the incoming state. `heatflux` is read as the **lagged** wall flux;
///   `fueltemp` is read as the property iterate for the rod solves and
///   overwritten with the result.
/// - `whichsigma` — the material map, used only to skip unfuelled columns.
/// - `pwrdens` — the power density from the flux solver, `G*es` long. Consumed
///   normalised and group-collapsed.
///
/// # Returns
///
/// `(th, report)` — the updated state and the per-node [`RodReport`].
///
/// # The heat-transfer coefficient
///
/// A Dittus-Boelter correlation on the subchannel:
///
/// ```text
/// subarea  = pitch^2 - pi Rtot^2
/// hydia    = 4 subarea / (2 pi Rtot + 4 pitch - 8 Rtot)
/// Re       = vm hydia / kvis
/// Nu       = 0.023 Pr^0.4 Re^0.8
/// hcoeff   = tcon Nu / hydia
/// ```
///
/// with the exponent 0.4 being the heating form. Lengths in cm, so `hcoeff` is
/// W/(cm²·K) and the rod boundary condition `bc = hcoeff * Rtot` is W/(cm·K).
///
/// **`Pr^0.4` and `Re^0.8` are wrapped in `real()` in the reference**, which
/// only matters if either goes negative — a non-physical state that the
/// fractional power would otherwise turn complex. Reproduced here by taking the
/// power of the absolute value where the base is negative, which is what
/// `real(x^0.4)` gives for the principal branch only when `x >= 0`; see the
/// note on defect T11.
///
/// # `subarea` and `hydia` are recomputed, not read
///
/// This driver derives both from `pitch` and `Rtot` rather than reading
/// [`crate::types::FuelGeometry::subarea`] and `hydia`, which the case files
/// also set and [`crate::w3chf`] does read. If a case file's stored values
/// disagree with `pitch^2 - pi Rtot^2`, the two modules will silently use
/// different subchannel geometry. Recorded as defect T12.
///
/// # The Doppler temperature is a two-point weight, not a volume average
///
/// ```text
/// fueltempdoppler = (1 - alpha) * T(centre) + alpha * T(pellet surface)
/// fueltempavg     = fueltempdoppler
/// ```
///
/// The pellet surface is unknown index `fueln + 1` (1-based), which is the
/// interface-duplicate node [`crate::fuelrodheat_1dcylnd`] creates — **not**
/// the gap dummy, which sits one further out. The commented-out line directly
/// above computes a genuine `Vi`-weighted average over the fuel nodes; it is
/// disabled, and `fueltempavg` is simply aliased to the Doppler value. So a
/// reader expecting an average gets a two-point weight. Recorded as defect T13.
///
/// # Dead reads
///
/// The reference loads `Lx`, `Ly`, `Lr`, `Vi` (and `repmat`s it to `G`
/// groups), `Vif`, `whichf`, `whichg` and `maxir`, and computes
/// `subflow = flowrate * subarea` — **none of which it then uses**. All are
/// residue of the commented-out inline conduction assembly. Not parameters
/// here. Recorded as defect T14.
///
/// # Panics
///
/// If `pwrdens` is shorter than `G*es`, or the geometry vectors are shorter
/// than the node count.
pub fn th_solverxyz(
    params: &Params,
    geometry: &Geometry,
    th: &Th,
    whichsigma: &Array3<usize>,
    pwrdens: &[f64],
) -> (Th, RodReport) {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let xstep = maxiy * maxiz;
    let fueln = params.fuel.fueln;

    assert!(
        pwrdens.len() >= g_count * es,
        "pwrdens is {} long, need G*es = {}",
        pwrdens.len(),
        g_count * es
    );
    assert!(geometry.lz.len() >= es, "geometry.lz is too short");

    let tmaxfuel = params.tmaxfuel.unwrap_or(TMAX_FUEL_DEFAULT);

    // ---- stage 1: normalise, then collapse over groups ----
    // Note the normalisation is over the *whole* G*es vector, before the sum.
    let scale = norm1(&pwrdens[..g_count * es]);
    let mut collapsed = vec![0.0; es];
    for (n, slot) in collapsed.iter_mut().enumerate() {
        let mut acc = pwrdens[n] / scale;
        for g in 1..g_count {
            acc += pwrdens[g * es + n] / scale;
        }
        *slot = acc;
    }
    let pwrdens = collapsed;

    // ---- stage 2: the coolant ----
    let th = match params.th_model {
        ThModel::Hem => crate::singleflow1devap::singleflow1devap(params, geometry, th, &pwrdens),
        ThModel::TwoFluid => {
            crate::driftflux6_solverstatic3d::driftflux6_solverstatic3d(
                params, geometry, th, &pwrdens,
            )
            .0
        }
    };

    let temps = &th.coolant.temps;
    let kvis = &th.coolant.kvis;
    let pran = &th.coolant.pran;
    let tcon = &th.coolant.tcon;
    let vm = &th.coolant.vm;

    // ---- the heat-transfer coefficient ----
    let pitch = geometry.fuel.pitch;
    let rtot = geometry.fuel.rtot;
    let frad = geometry.fuel.fuelrad;
    let subarea = pitch * pitch - std::f64::consts::PI * rtot * rtot;
    let hydia = 4.0 * subarea
        / (2.0 * std::f64::consts::PI * rtot + 4.0 * pitch - 8.0 * rtot);

    // `real(x^p)` for a real base: negative bases would go complex, and MATLAB's
    // `real` then keeps only the real part. See defect T11.
    let real_pow = |x: f64, p: f64| -> f64 {
        if x >= 0.0 {
            x.powf(p)
        } else {
            // `(-a)^p = a^p * (cos(pi p) + i sin(pi p))`; `real` keeps the
            // cosine factor.
            (-x).powf(p) * (std::f64::consts::PI * p).cos()
        }
    };

    let mut hcoeff = vec![0.0; es];
    for i in 0..es {
        let reynolds = vm[i] * hydia / kvis[i];
        let nusselt = 0.023 * real_pow(pran[i], 0.4) * real_pow(reynolds, 0.8);
        hcoeff[i] = tcon[i] * nusselt / hydia;
    }

    let linpwrdens: Vec<f64> = (0..es)
        .map(|i| pwrdens[i] * th.maxpow * th.powratio / geometry.lz[i])
        .collect();
    let pinpowdens: Vec<f64> = linpwrdens
        .iter()
        .map(|q| {
            (1.0 - th.coolheatfrac) * q / th.nfuelpin / (std::f64::consts::PI * frad * frad)
        })
        .collect();

    // ---- stage 3: the rods ----
    let maxid = th.fueltemp.cols();
    let mut fueltemp = th.fueltemp.clone();
    let mut fueltempavg = th.fueltempavg.clone();
    let mut fueltempdoppler = th.fueltempdoppler.clone();
    fueltempavg.resize(es, 0.0);
    fueltempdoppler.resize(es, 0.0);
    let mut heatflux = vec![0.0; es];
    let mut report = RodReport::default();

    let alpha = geometry.fuel.doppleralpha;
    let bounds = |a: &Option<Array2<usize>>, ix: usize, iy: usize, fallback: usize| {
        a.as_ref().map_or(fallback, |m| m.get(ix, iy))
    };

    for ix in 0..maxix {
        for iy in 0..maxiy {
            let zlow = bounds(&geometry.zlows, ix, iy, 0);
            let zhi = bounds(&geometry.zhis, ix, iy, maxiz - 1);
            // An unfuelled column is skipped wholesale, on its *lowest* node.
            if whichsigma.get(ix, iy, zlow) == 0 {
                report.skipped += zhi + 1 - zlow;
                continue;
            }

            for iz in zlow..=zhi {
                let idx = ix * xstep + iy * maxiz + iz;
                if pinpowdens[idx] == 0.0 {
                    report.skipped += 1;
                    continue;
                }

                let bc = hcoeff[idx] * rtot;
                let profile: Vec<f64> = (0..maxid).map(|j| fueltemp.get(idx, j)).collect();
                let (mut solved, _) = fuelrodheat_1dcylnd(
                    &geometry.fuel,
                    params.fuel.maxir,
                    &profile,
                    pinpowdens[idx],
                    bc,
                    temps[idx],
                );

                // The clamp: the fuel cannot be colder than its coolant sink
                // nor hotter than melting.
                let floor = if temps[idx].is_finite() { temps[idx] } else { 0.0 };
                let mut hit_low = false;
                let mut hit_high = false;
                for t in solved.iter_mut() {
                    if *t < floor {
                        *t = floor;
                        hit_low = true;
                    }
                    if *t > tmaxfuel {
                        *t = tmaxfuel;
                        hit_high = true;
                    }
                }
                if hit_low {
                    report.clamped_low += 1;
                }
                if hit_high {
                    report.clamped_high += 1;
                }

                let nan_seen = solved.iter().any(|x| x.is_nan());
                if nan_seen {
                    // The reference warns and substitutes rather than halting.
                    let tfb = if temps[idx].is_finite() {
                        temps[idx]
                    } else {
                        params.cooltempavg
                    };
                    for j in 0..maxid {
                        fueltemp.set(idx, j, tfb);
                    }
                    fueltempdoppler[idx] = tfb;
                    fueltempavg[idx] = tfb;
                    heatflux[idx] = 0.0;
                    report.rescued += 1;
                    continue;
                }

                for (j, t) in solved.iter().enumerate() {
                    fueltemp.set(idx, j, *t);
                }
                // Centre and pellet surface; `fueln + 1` 1-based is `fueln`
                // 0-based.
                fueltempdoppler[idx] = (1.0 - alpha) * solved[0] + alpha * solved[fueln];
                fueltempavg[idx] = fueltempdoppler[idx];
                heatflux[idx] = hcoeff[idx] * (solved[maxid - 1] - temps[idx]);
                report.solved += 1;
            }
        }
    }

    // ---- stage 4: pack the feedback quantities ----
    let mut out = th.clone();
    out.heatflux = heatflux;
    out.fueltemp = fueltemp;
    out.fueltempavg = fueltempavg;
    out.fueltempdoppler = fueltempdoppler;
    out.linpwrdens = linpwrdens;

    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Coolant, Conductivity, FlowDirection, FuelGeometry, FuelParams, MassFlux,
        VolumetricHeatCapacity,
    };

    /// A single fuelled BWR channel of `n` axial nodes, with a NEACRP-shaped
    /// rod.
    fn channel(n: usize, power_w: f64) -> (Params, Geometry, Th, Array3<usize>, Vec<f64>) {
        let (fueln, gapn, cladn) = (5usize, 1usize, 2usize);
        let maxir = fueln + gapn + cladn;
        let maxid = maxir + 2;

        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(n),
            g: 1,
            nc: Some(0),
            th_model: ThModel::Hem,
            cooltempavg: 560.0,
            fuel: FuelParams {
                maxir,
                fueln,
                gapn,
                cladn,
            },
            ..Default::default()
        };

        let mut lr = vec![0.41 / fueln as f64; fueln];
        lr.extend(vec![0.006 / gapn as f64; gapn]);
        lr.extend(vec![0.06 / cladn as f64; cladn]);
        let mut ctr = Vec::with_capacity(maxir);
        let mut acc = 0.0;
        for l in &lr {
            acc += l;
            ctr.push(acc - 0.5 * l);
        }
        let mut whichk = vec![1usize; fueln];
        whichk.extend(vec![0usize; gapn]);
        whichk.extend(vec![2usize; cladn]);

        let mut zl = Array2::<usize>::zeros(1, 1);
        zl.set(0, 0, 0);
        let mut zh = Array2::<usize>::zeros(1, 1);
        zh.set(0, 0, n - 1);

        let geometry = Geometry {
            lz: vec![366.0 / n as f64; n],
            zlows: Some(zl),
            zhis: Some(zh),
            fuel: FuelGeometry {
                lr,
                ctr,
                whichk,
                tcon: vec![Conductivity::Uo2Fuel, Conductivity::ZircaloyClad],
                rhocp: vec![
                    VolumetricHeatCapacity::Uo2Fuel,
                    VolumetricHeatCapacity::ZircaloyClad,
                ],
                gap_conductance: 0.35,
                fuelrad: 0.41,
                rtot: 0.476,
                pitch: 1.26,
                // Set consistently with what `th_solverxyz` recomputes from
                // `pitch` and `rtot`. They are separate inputs, and defect T12
                // is that nothing enforces the agreement — see
                // `the_stored_and_recomputed_subarea_can_disagree`.
                subarea: 1.26 * 1.26 - std::f64::consts::PI * 0.476 * 0.476,
                hydia: 4.0 * (1.26 * 1.26 - std::f64::consts::PI * 0.476 * 0.476)
                    / (2.0 * std::f64::consts::PI * 0.476 + 4.0 * 1.26 - 8.0 * 0.476),
                doppleralpha: 0.3,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut fueltemp = Array2::<f64>::zeros(n, maxid);
        for i in 0..n {
            for j in 0..maxid {
                fueltemp.set(i, j, 800.0);
            }
        }

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
            coolheatfrac: 0.02,
            flowrate: MassFlux::Uniform(100.0),
            flowdir: FlowDirection::Up,
            fueltemp,
            fueltempavg: vec![0.0; n],
            fueltempdoppler: vec![0.0; n],
            ..Default::default()
        };

        let mut whichsigma = Array3::<usize>::zeros(1, 1, n);
        for iz in 0..n {
            whichsigma.set(0, 0, iz, 1);
        }

        let pwrdens = vec![1.0 / n as f64; n];
        (params, geometry, th, whichsigma, pwrdens)
    }

    /// Run the coolant/rod coupling to convergence.
    ///
    /// One `th_solverxyz` call is a **single Picard sweep**: it reads the
    /// previous wall heat flux, solves the coolant with it, then recomputes the
    /// flux from the rods. On the first sweep `heatflux` is zero, so the
    /// coolant sees only the `coolheatfrac` direct heating and barely warms.
    /// The loop is what closes the coupling.
    fn picard(
        params: &Params,
        geometry: &Geometry,
        th: &Th,
        whichsigma: &Array3<usize>,
        pwrdens: &[f64],
        passes: usize,
    ) -> (Th, RodReport) {
        let mut state = th.clone();
        let mut report = RodReport::default();
        for _ in 0..passes {
            let (next, r) = th_solverxyz(params, geometry, &state, whichsigma, pwrdens);
            state = next;
            report = r;
        }
        (state, report)
    }

    /// A full steady pass produces a physically ordered result: fuel hotter
    /// than coolant, a positive wall flux, and a Doppler temperature between
    /// the pellet centre and its surface.
    ///
    /// # Methodology
    ///
    /// One 12-node BWR channel at **40 kW** — a realistic single-pin duty —
    /// with 2% of the power deposited directly in the coolant, the HEM channel
    /// model, inlet 550 K at 7 MPa, run to Picard convergence over 20 sweeps.
    /// Every stage runs: normalise, coolant solve, Dittus-Boelter, rod solves,
    /// feedback.
    ///
    /// **The power matters, and 500 kW was wrong.** The first version of this
    /// test put 500 kW into one channel — roughly ten times a real pin — which
    /// drove the coolant into `singleflow1devap`'s `enthmax` clamp at 1050 K
    /// and *every* rod node onto the 3100 K melting clamp. The test passed
    /// while exercising nothing but the two clamps. `report.clamped == 0` is
    /// now asserted precisely so that cannot recur silently.
    ///
    /// Pass criteria, all consequences of the physics rather than of the
    /// implementation: every node solved, none rescued, **none clamped at the
    /// melting ceiling**; the Doppler temperature exceeds the coolant
    /// temperature at every node; the wall heat flux is positive; and
    /// `fueltempdoppler` lies between the pellet centre and the pellet surface,
    /// being a convex combination of the two.
    ///
    /// The **floor** clamp is expected to fire on every node and is asserted to
    /// do so — see [`RodReport::clamped_low`] for why that is a property of the
    /// gap dummy rather than a sign of trouble.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `RodReport { solved: 12, skipped: 0, rescued: 0, clamped_low: 12,
    /// clamped_high: 0 }`. At the channel ends, in K:
    ///
    /// | | inlet node | outlet node |
    /// |---|---|---|
    /// | coolant | 553.62 | 558.98 |
    /// | pellet centre | 885.32 | 884.83 |
    /// | pellet surface | 711.80 | 711.39 |
    /// | Doppler | 833.26 | 832.80 |
    /// | wall flux W/cm² | 35.81 | 35.81 |
    ///
    /// **Interpretation.** The whole coupled pass is physically coherent. The
    /// coolant heats from 553.6 K to exactly `Tsat(7 MPa) = 558.98 K` and then
    /// stops rising — it has begun boiling, and the remaining heat goes into
    /// quality rather than temperature. The wall flux is uniform at
    /// 35.81 W/cm², which is what a uniform axial power profile must give:
    /// `0.98 * 40 kW / (2 pi * 0.476 cm * 366 cm) = 35.8`. So stage 3 returns
    /// exactly the flux stage 2 needs, and the Picard loop has closed on a
    /// consistent state rather than merely stopped moving.
    ///
    /// The 174 K drop from pellet centre to surface at 40 kW is the same
    /// conduction profile [`crate::fuelrodheat_1dcylnd`] verifies in isolation,
    /// now driven by a heat-transfer coefficient this module computed.
    #[test]
    fn a_steady_pass_produces_an_ordered_feedback_state() {
        let n = 12;
        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 40_000.0);
        let (out, report) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        eprintln!("{report:?}");
        for i in 0..n {
            eprintln!(
                "  node {i:2}: Tcool = {:7.2}  Tdop = {:7.2}  Tc = {:7.2}  Tsurf = {:7.2}  q'' = {:7.2}",
                out.coolant.temps[i],
                out.fueltempdoppler[i],
                out.fueltemp.get(i, 0),
                out.fueltemp.get(i, 5),
                out.heatflux[i]
            );
        }

        assert_eq!(report.solved, n);
        assert_eq!(report.rescued, 0);
        assert_eq!(
            report.clamped_high, 0,
            "a well-posed case should not reach the melting clamp"
        );
        assert_eq!(
            report.clamped_low, n,
            "the gap dummy should trip the floor clamp on every node (T7)"
        );
        for i in 0..n {
            let centre = out.fueltemp.get(i, 0);
            let surface = out.fueltemp.get(i, 5);
            assert!(
                out.fueltempdoppler[i] > out.coolant.temps[i],
                "node {i}: fuel is not above the coolant"
            );
            assert!(out.heatflux[i] > 0.0, "node {i}: wall flux is not positive");
            assert!(
                out.fueltempdoppler[i] <= centre && out.fueltempdoppler[i] >= surface,
                "node {i}: Doppler {} is outside [{surface}, {centre}]",
                out.fueltempdoppler[i]
            );
            // `fueltempavg` is aliased to the Doppler value — defect T13.
            assert_eq!(out.fueltempavg[i], out.fueltempdoppler[i]);
        }
    }

    /// The Doppler weight does what its definition says.
    ///
    /// # Methodology
    ///
    /// `fueltempdoppler = (1 - alpha) T_centre + alpha T_surface`. Running the
    /// same case at `alpha = 0` and `alpha = 1` must return exactly the centre
    /// and exactly the pellet-surface temperature respectively, and the
    /// `alpha = 0.3` case must be the corresponding convex combination.
    ///
    /// This also pins **which node is the pellet surface**: the reference uses
    /// 1-based `fueln + 1`, and getting that wrong by one would pick the gap
    /// dummy at 1 K and be immediately obvious here.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// At the inlet node: centre **889.36 K**, pellet surface **715.19 K**,
    /// and `alpha = 0 / 1 / 0.3` gave **889.36 / 715.19 / 837.11 K**. The
    /// convex combination checks out exactly:
    /// `0.7 * 889.36 + 0.3 * 715.19 = 837.11`.
    ///
    /// **Interpretation.** The weight is what its definition says, and — the
    /// point of the test — index `fueln` (0-based) really is the pellet
    /// surface. At 715 K it is unmistakably a solid-fuel temperature, not the
    /// gap dummy one place further out, which would have shown as 1 K before
    /// the clamp or the coolant temperature after it.
    #[test]
    fn the_doppler_weight_interpolates_centre_to_pellet_surface() {
        let n = 6;
        let run = |alpha: f64| {
            let (params, mut geometry, th, whichsigma, pwrdens) = channel(n, 40_000.0);
            geometry.fuel.doppleralpha = alpha;
            let (out, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);
            out
        };

        let at0 = run(0.0);
        let at1 = run(1.0);
        let at03 = run(0.3);

        for i in 0..n {
            let centre = at0.fueltemp.get(i, 0);
            let surface = at1.fueltemp.get(i, 5);
            eprintln!(
                "node {i}: centre {centre:.2}, surface {surface:.2}, alpha=0 -> {:.2}, alpha=1 -> {:.2}, alpha=0.3 -> {:.2}",
                at0.fueltempdoppler[i], at1.fueltempdoppler[i], at03.fueltempdoppler[i]
            );
            assert!((at0.fueltempdoppler[i] - centre).abs() < 1e-9);
            assert!((at1.fueltempdoppler[i] - surface).abs() < 1e-9);
            // The pellet surface must be far above 1 K — i.e. not the gap dummy.
            assert!(surface > 500.0, "picked the gap dummy at node {i}");
        }
    }

    /// Raising the power raises the fuel temperature and the wall flux
    /// together — the feedback path the neutronics sees.
    #[test]
    fn more_power_means_hotter_fuel_and_more_wall_flux() {
        let n = 8;
        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 20_000.0);
        let (low, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 60_000.0);
        let (high, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        for i in 0..n {
            assert!(
                high.fueltempdoppler[i] > low.fueltempdoppler[i],
                "node {i}: tripling the power did not raise the fuel temperature"
            );
            assert!(
                high.heatflux[i] > low.heatflux[i],
                "node {i}: tripling the power did not raise the wall flux"
            );
        }
    }

    /// An unfuelled column is skipped on the strength of its lowest node alone.
    ///
    /// # Methodology
    ///
    /// The reference tests `whichsigma(ix, iy, zlow) == 0` and `continue`s past
    /// the **whole column** — it does not check the other axial nodes. Setting
    /// only the bottom node to void therefore skips a column that is fuelled
    /// everywhere above it.
    ///
    /// That is worth pinning rather than smoothing over: combined with
    /// [`crate::geometry_ends3d`]'s first-contiguous-run limitation, a core
    /// with an axial void at the bottom of a channel loses that whole channel's
    /// rod solve silently.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `RodReport { solved: 0, skipped: 8, rescued: 0, clamped_low: 0,
    /// clamped_high: 0 }` — the whole eight-node column was skipped on the
    /// strength of its bottom node alone, and every wall flux came back zero.
    ///
    /// **Interpretation.** Confirmed as written. The consequence is worth
    /// stating plainly: a channel with any void at its bottom contributes **no
    /// heat to the coolant at all** on that pass, silently. Combined with
    /// [`crate::geometry_ends3d`]'s first-contiguous-run limitation, which can
    /// place `zlow` on a void node, this is reachable without the case file
    /// doing anything unusual.
    #[test]
    fn a_void_bottom_node_skips_the_whole_column() {
        let n = 8;
        let (params, geometry, th, mut whichsigma, pwrdens) = channel(n, 40_000.0);
        // Void only the lowest node; the rest stay fuelled.
        whichsigma.set(0, 0, 0, 0);

        let (out, report) = th_solverxyz(&params, &geometry, &th, &whichsigma, &pwrdens);
        eprintln!("{report:?}");
        assert_eq!(report.solved, 0, "the whole column should have been skipped");
        assert_eq!(report.skipped, n);
        for i in 0..n {
            assert_eq!(out.heatflux[i], 0.0);
        }
    }

    /// The two channel models give different answers, and only one of them
    /// actually solves.
    ///
    /// # Methodology
    ///
    /// `th_model = 'hem'` routes to [`crate::singleflow1devap`], which marches
    /// an enthalpy profile. `'twofluid'` routes to
    /// [`crate::driftflux6_solverstatic3d`], whose per-channel solver is absent
    /// from the snapshot, so it returns the inlet defaults unchanged.
    ///
    /// The HEM path must therefore show an axial coolant temperature rise,
    /// while the two-fluid path shows none. Pass criterion: HEM's outlet
    /// coolant temperature exceeds its inlet; the two-fluid path's does not.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// HEM heated the coolant from **554.34 K to 558.98 K** along the channel;
    /// the two-fluid path left it flat at the **550.00 K** inlet value.
    ///
    /// **Interpretation.** Exactly the expected split. The two-fluid branch is
    /// not broken — it runs, returns, and recovers every derived field — but
    /// with `driftflux6_solverstatic1d.m` absent it has nothing to solve with,
    /// so the coolant never leaves its inlet state. A caller who did not know
    /// that would see a plausible, perfectly uniform core and no error.
    /// [`crate::driftflux6_solverstatic3d::ChannelReport`] is how to tell.
    #[test]
    fn only_the_hem_model_actually_solves_the_coolant() {
        let n = 10;
        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 40_000.0);
        let (hem, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        let params = Params {
            th_model: ThModel::TwoFluid,
            ..params
        };
        let (twofluid, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        eprintln!(
            "HEM coolant: {:.2} -> {:.2} K; two-fluid: {:.2} -> {:.2} K",
            hem.coolant.temps[0],
            hem.coolant.temps[n - 1],
            twofluid.coolant.temps[0],
            twofluid.coolant.temps[n - 1]
        );
        assert!(
            hem.coolant.temps[n - 1] > hem.coolant.temps[0],
            "HEM should heat the coolant along the channel"
        );
        assert!(
            (twofluid.coolant.temps[n - 1] - twofluid.coolant.temps[0]).abs() < 1e-9,
            "the two-fluid path cannot solve, so its coolant should be flat"
        );
    }

    /// The power density is normalised over all groups before being collapsed.
    ///
    /// # Methodology
    ///
    /// The reference divides by `norm(pwrdens, 1)` over the **whole `G*es`
    /// vector** and only then sums the groups, so the collapsed profile sums to
    /// unity. Scaling the input by any constant must therefore leave the result
    /// untouched — the absolute power comes from `th.maxpow`, not from the flux
    /// solver's normalisation.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Scaling the input power density by 1e7 changed no Doppler temperature by
    /// more than 1e-9, and the collapsed linear power integrated to
    /// **40000.0 W** against the 40 kW the channel was given.
    ///
    /// **Interpretation.** The absolute power comes from `th.maxpow`, not from
    /// the flux solver's normalisation — which matters because the flux solvers
    /// normalise their own output to an arbitrary fission-source integral (see
    /// defect N10). This module is insulated from that choice.
    #[test]
    fn the_power_density_normalisation_is_scale_invariant() {
        let n = 8;
        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 40_000.0);
        let (a, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 10);

        let scaled: Vec<f64> = pwrdens.iter().map(|x| x * 1e7).collect();
        let (b, _) = picard(&params, &geometry, &th, &whichsigma, &scaled, 10);

        for i in 0..n {
            assert!(
                (a.fueltempdoppler[i] - b.fueltempdoppler[i]).abs() < 1e-9,
                "node {i}: scaling the input power changed the answer"
            );
            assert!((a.linpwrdens[i] - b.linpwrdens[i]).abs() < 1e-9);
        }
        // And the collapsed profile integrates to the total power.
        let total: f64 = (0..n).map(|i| a.linpwrdens[i] * geometry.lz[i]).sum();
        eprintln!("integrated linear power = {total:.1} W of a 40000 W channel");
        assert!((total - 40_000.0).abs() / 40_000.0 < 1e-9);
    }

    /// Defect T12, pinned: the stored and recomputed subchannel areas are
    /// independent, and a disagreement is silent and severe.
    ///
    /// # Methodology
    ///
    /// `th_solverxyz` derives `subarea = pitch^2 - pi Rtot^2` locally for its
    /// heat-transfer coefficient, while [`crate::singleflow1devap`] reads the
    /// stored `geometry.fuel.subarea` for its enthalpy march. Nothing checks
    /// that the two agree.
    ///
    /// This was found by accident and is worth keeping: the test fixture
    /// originally left the stored field at its default of **zero** while
    /// setting `pitch` and `rtot`, so the driver's `hcoeff` was computed on a
    /// sensible 0.876 cm² subchannel while the coolant march divided its
    /// enthalpy rise by zero. The result was not an error — the enthalpy ran
    /// away to `singleflow1devap`'s `enthmax` clamp and the coolant reported a
    /// steady, plausible-looking **1050 K** at every node.
    ///
    /// Pass criterion: a run with the stored area set to half the geometric one
    /// produces a **different** coolant state from a consistent run, confirming
    /// the two paths really do use separate values.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// With the stored subarea consistent at 0.8758 cm² the outlet enthalpy was
    /// **1648.0 kJ/kg**; halving only the stored value gave **2076.2 kJ/kg** —
    /// a 26% difference in outlet enthalpy from a field the driver never reads.
    /// Both runs report the same outlet *temperature* of 558.98 K, because both
    /// are saturated, so the discrepancy is invisible in the temperature field
    /// and shows only in the enthalpy and hence the void.
    ///
    /// **Interpretation.** T12 confirmed, and worse than it first looks: the
    /// disagreement propagates into void fraction and therefore into moderator
    /// density feedback, while leaving the most-inspected field — temperature —
    /// looking correct. The original discovery was starker still: with the
    /// stored area left at its default of zero, the enthalpy march divided by
    /// zero, ran away to the `enthmax` clamp, and reported a steady, uniform,
    /// entirely plausible 1050 K core.
    #[test]
    fn the_stored_and_recomputed_subarea_can_disagree() {
        let n = 8;
        let (params, geometry, th, whichsigma, pwrdens) = channel(n, 40_000.0);
        let (consistent, _) = picard(&params, &geometry, &th, &whichsigma, &pwrdens, 20);

        // Halve only the *stored* area; `pitch` and `rtot` are untouched, so
        // the driver's own recomputation is unchanged.
        let mut skewed_geom = geometry.clone();
        skewed_geom.fuel.subarea = geometry.fuel.subarea / 2.0;
        let (skewed, _) = picard(&params, &skewed_geom, &th, &whichsigma, &pwrdens, 20);

        eprintln!(
            "stored subarea {:.4} -> outlet coolant {:.2} K; halved -> {:.2} K",
            geometry.fuel.subarea,
            consistent.coolant.temps[n - 1],
            skewed.coolant.temps[n - 1]
        );
        eprintln!(
            "  outlet enthalpy {:.1} vs {:.1} kJ/kg",
            consistent.coolant.enth[n - 1],
            skewed.coolant.enth[n - 1]
        );
        assert!(
            (consistent.coolant.enth[n - 1] - skewed.coolant.enth[n - 1]).abs() > 1.0,
            "the stored subarea had no effect, so T12 may have been fixed"
        );
    }
}
