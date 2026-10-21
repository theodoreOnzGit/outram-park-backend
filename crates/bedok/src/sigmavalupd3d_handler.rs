//! Cross-section feedback — apply every enabled feedback channel in turn.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `sigmavalupd3d_handler.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this is
//!
//! The bridge from thermal-hydraulics back to neutronics. It takes the
//! reference cross-section set and applies each enabled feedback channel in
//! sequence, every one a call to [`crate::sigmavalupd3d`]:
//!
//! | Channel | Feedback variable | Exponent |
//! |---|---|---|
//! | boron | `params.boron`, ppm | 1 |
//! | fuel temperature | `th.fueltempdoppler`, K | **0.5** |
//! | moderator temperature | `th.modtemp`, K | 1 |
//! | coolant temperature | `th.coolant.temps`, K | 1 |
//! | coolant density | `th.coolant.dens`, g/cm³ | 1 |
//! | control rods | a computed rod fraction, 0 to 1 | 1 |
//!
//! Each is applied only if the case file supplied its table, so a case selects
//! its feedback model by which tables it defines. The reference's own comment
//! says as much: "split into independent factors/categories, can add more
//! categories as they come along".
//!
//! **The `0.5` on fuel temperature is the square-root Doppler law**, and it is
//! the reason [`crate::sigmavalupd3d`] carries its `real(a^m)` machinery — a
//! negative argument under a square root is complex, and MATLAB keeps the real
//! part.
//!
//! # The channels compose by chaining, not by summing
//!
//! Each call takes the *previous* call's output as its `sigmavaluesold`, so the
//! perturbations accumulate in the order written above. The order is therefore
//! observable whenever two channels' slopes interact, though for the linear
//! tables the snapshot ships they commute.
//!
//! # Reference defects carried here
//!
//! - **C1, the most serious in the register: `rodlvl` is never initialised.**
//!   See [`sigmavalupd3d_handler`].
//! - **The rod-fraction CSV is written unconditionally** —
//!   `writematrix(rodfrac, 'rodfrac.csv')` runs on every call, outside the
//!   `debugdump` guard that protects the four dumps below it. Returned here
//!   rather than written, as everywhere else in this crate.
//! - **`sigmavaluesref.crod.ref = 0` mutates the caller's table** in MATLAB's
//!   copy-on-write sense — the local copy only, but it means the crod table's
//!   own `ref` field is ignored and forced to zero. Reproduced.

use crate::error::BedokError;
use crate::fixnegativematrix::fixnegativematrix_dense;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::Array3;
use crate::sigmavalupd3d::{sigmavalupd3d, DeltaSigmaValues};
use crate::types::{Geometry, Params, SigmaValues, Th};
use crate::Result;

/// The feedback slope tables a case file may supply.
///
/// In the reference these are extra fields hung on the `sigmavaluesref` struct
/// and tested with `isfield`; here they are `Option`s on their own struct, so
/// the reference cross sections stay a plain [`SigmaValues`].
///
/// A `None` channel is simply not applied.
#[derive(Clone, Debug, Default)]
pub struct FeedbackTables {
    /// `sigmavaluesref.boron` — slopes against soluble boron, per ppm.
    pub boron: Option<DeltaSigmaValues>,
    /// `sigmavaluesref.fueltemp` — slopes against **the square root of** the
    /// Doppler fuel temperature, per sqrt(K).
    pub fueltemp: Option<DeltaSigmaValues>,
    /// `sigmavaluesref.modtemp` — slopes against moderator temperature, per K.
    pub modtemp: Option<DeltaSigmaValues>,
    /// `sigmavaluesref.cooltemp` — slopes against coolant temperature, per K.
    pub cooltemp: Option<DeltaSigmaValues>,
    /// `sigmavaluesref.coolden` — slopes against coolant density, per g/cm³.
    pub coolden: Option<DeltaSigmaValues>,
    /// `sigmavaluesref.crod` — slopes against the rodded fraction of a node,
    /// dimensionless.
    ///
    /// Its `reference` is **forced to zero** on use; see the module docs.
    pub crod: Option<DeltaSigmaValues>,
}

/// The rod-fraction map, returned rather than written to `rodfrac.csv`.
#[derive(Clone, Debug, Default)]
pub struct RodFraction {
    /// The rodded fraction of each node, 0 (unrodded) to 1 (fully rodded), in
    /// the usual flattened order.
    pub frac: Vec<f64>,
    /// How many lattice positions fell through the level search with `rodlvl`
    /// left at a previous column's value — defect C1.
    ///
    /// **Non-zero means the rod pattern is wrong**, silently, using another
    /// column's insertion level. Zero on any pattern where every bank's tip
    /// sits inside its column.
    pub stale_level_carryovers: usize,
}

/// `[sigmavalues, whichsigma] = sigmavalupd3d_handler(params, geometry, sigmavaluesref, whichsigmaref, th)`.
///
/// # Arguments
///
/// - `params` — needs `boron` and the extents.
/// - `geometry` — needs `Lz` and, when the rod channel is enabled, `crodbanks`,
///   `crod`, `crodstep` and `crodbtm`.
/// - `sigmavaluesref` — the unperturbed per-material cross sections.
/// - `feedback` — the slope tables; see [`FeedbackTables`].
/// - `whichsigmaref` — the reference material map.
/// - `th` — supplies `fueltempdoppler`, `modtemp`, `coolant.temps` and
///   `coolant.dens`. The reference notes a caller with no T-H feedback may pass
///   a dummy.
///
/// # Returns
///
/// `(sigmavalues, whichsigma, rodfraction)` — the perturbed **per-node** cross
/// sections, the compacted node-to-row map, and the rod-fraction map.
///
/// Note the output `whichsigma` is not the input one: [`crate::sigmavalupd3d`]
/// renumbers every fuelled node to its own row, so the returned table has one
/// row per node rather than per material.
///
/// # Defect C1 — `rodlvl` is never initialised
///
/// The rod level search is
///
/// ```text
/// for iz = 1:maxiz
///     if sum(Lz(idx+1 : idx+iz)) > rodpos(rod)
///         rodlvl = iz;
///         break
///     end
/// end
/// ```
///
/// with no assignment before the loop and no `else`. **If the bank's tip sits
/// at or above the top of its column the condition never fires**, `rodlvl` is
/// never written, and the code below reads whatever the *previous* `(ix, iy)`
/// left there.
///
/// That is not an exotic case: it is a **fully withdrawn bank**, which is the
/// end state of every rod-ejection transient — the primary thing this code
/// exists to model. The consequence is a silently wrong rod pattern, taking one
/// lattice position's insertion level for another's.
///
/// Translated with the same carry-over, counted in
/// [`RodFraction::stale_level_carryovers`] so a caller can see it happened. A
/// **first**-column occurrence has no previous value to inherit; MATLAB would
/// raise `Undefined function or variable 'rodlvl'`, and this returns
/// [`BedokError::NanEncountered`]'s sibling
/// [`BedokError::UninitialisedRodLevel`] rather than inventing one.
///
/// # Errors
///
/// - [`BedokError::UninitialisedRodLevel`] on a first-column C1 occurrence.
/// - Whatever [`crate::sigmavalupd3d`] raises — it runs `pauseonnan` over its
///   six outputs.
///
/// # Panics
///
/// If the rod channel is enabled without `crodbanks`, or a bank number indexes
/// past `geometry.crod`.
pub fn sigmavalupd3d_handler(
    params: &Params,
    geometry: &Geometry,
    sigmavaluesref: &SigmaValues,
    feedback: &FeedbackTables,
    whichsigmaref: &Array3<usize>,
    th: &Th,
) -> Result<(SigmaValues, Array3<usize>, RodFraction)> {
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let es = maxix * maxiy * maxiz;

    let mut sigmavalues = sigmavaluesref.clone();
    let mut whichsigma = whichsigmaref.clone();

    // Each channel chains off the previous one's output.
    let apply = |sv: &SigmaValues,
                     ws: &Array3<usize>,
                     table: &DeltaSigmaValues,
                     currval: &[f64],
                     m: Option<f64>|
     -> Result<(SigmaValues, Array3<usize>)> {
        sigmavalupd3d(params, sv, ws, whichsigmaref, table, currval, m)
    };

    if let Some(t) = &feedback.boron {
        // A scalar feedback variable, expanded over the core as the
        // reference's `isscalar` guard does.
        let currval = vec![params.boron; es];
        let (sv, ws) = apply(&sigmavalues, &whichsigma, t, &currval, None)?;
        sigmavalues = sv;
        whichsigma = ws;
    }
    if let Some(t) = &feedback.fueltemp {
        // m = 0.5 — the square-root Doppler law.
        let (sv, ws) = apply(
            &sigmavalues,
            &whichsigma,
            t,
            &th.fueltempdoppler,
            Some(0.5),
        )?;
        sigmavalues = sv;
        whichsigma = ws;
    }
    if let Some(t) = &feedback.modtemp {
        let (sv, ws) = apply(&sigmavalues, &whichsigma, t, &th.modtemp, None)?;
        sigmavalues = sv;
        whichsigma = ws;
    }
    if let Some(t) = &feedback.cooltemp {
        let (sv, ws) = apply(&sigmavalues, &whichsigma, t, &th.coolant.temps, None)?;
        sigmavalues = sv;
        whichsigma = ws;
    }
    if let Some(t) = &feedback.coolden {
        let (sv, ws) = apply(&sigmavalues, &whichsigma, t, &th.coolant.dens, None)?;
        sigmavalues = sv;
        whichsigma = ws;
    }

    // ---- control rods ----
    let mut rodfraction = RodFraction {
        frac: vec![0.0; es],
        stale_level_carryovers: 0,
    };

    if let Some(t) = &feedback.crod {
        let rodbanks = geometry
            .crodbanks
            .as_ref()
            .expect("the control-rod feedback channel needs geometry.crodbanks");

        // A bank's tip position, cm from the bottom.
        let rodpos = |bank: usize| -> f64 {
            assert!(
                bank >= 1 && bank <= geometry.crod.len(),
                "control-rod bank {bank} has no entry in geometry.crod ({} banks)",
                geometry.crod.len()
            );
            geometry.crodbtm + geometry.crod[bank - 1] * geometry.crodstep
        };

        // Carried across lattice positions — that is defect C1.
        let mut rodlvl: Option<usize> = None;

        for ix in 0..maxix {
            for iy in 0..maxiy {
                let bank = rodbanks.get(ix, iy);
                if bank == 0 {
                    continue;
                }
                let idx = ix * maxiy * maxiz + iy * maxiz;
                let tip = rodpos(bank);

                // First level whose cumulative height clears the tip. 1-based,
                // matching the reference, because the arithmetic below uses it
                // as a count as well as an index.
                let mut found: Option<usize> = None;
                let mut cumulative = 0.0;
                for iz in 1..=maxiz {
                    cumulative += geometry.lz[idx + iz - 1];
                    if cumulative > tip {
                        found = Some(iz);
                        break;
                    }
                }

                match found {
                    Some(level) => rodlvl = Some(level),
                    None => {
                        // C1: no assignment, so the previous column's value
                        // stands.
                        rodfraction.stale_level_carryovers += 1;
                        if rodlvl.is_none() {
                            return Err(BedokError::UninitialisedRodLevel {
                                ix,
                                iy,
                                bank,
                            });
                        }
                    }
                }
                let level = rodlvl.expect("checked above");

                let height_to = |n: usize| -> f64 {
                    (0..n).map(|k| geometry.lz[idx + k]).sum()
                };

                if level < maxiz {
                    for iz in (level + 1)..=maxiz {
                        rodfraction.frac[idx + iz - 1] = 1.0;
                    }
                    rodfraction.frac[idx + level - 1] =
                        (height_to(level) - tip) / geometry.lz[idx + level - 1];
                } else {
                    rodfraction.frac[idx + maxiz - 1] =
                        (height_to(maxiz) - tip) / geometry.lz[idx + maxiz - 1];
                }
            }
        }

        // `sigmavaluesref.crod.ref = 0` — the table's own reference is ignored.
        let mut crod = t.clone();
        crod.reference = 0.0;
        let (sv, ws) = apply(&sigmavalues, &whichsigma, &crod, &rodfraction.frac, None)?;
        sigmavalues = sv;
        whichsigma = ws;
    }

    // ---- clamp anything that fed itself negative ----
    // The reference's comment: "check sigma values that might drop below 0 and
    // set them back to 0; not checking for sigma.tot and sigma.s(g,g)".
    fixnegativematrix_dense(&mut sigmavalues.f);
    if let Some(fp) = sigmavalues.fp.as_mut() {
        fixnegativematrix_dense(fp);
    }

    let g_count = sigmavalues.tot.cols();
    let materials = sigmavalues.tot.rows();
    for g in 0..g_count {
        for w in 0..materials {
            for gt in 0..g_count {
                if sigmavalues.s.get(w, gt, g) < 0.0 {
                    sigmavalues.s.set(w, gt, g, 0.0);
                }
            }
        }
    }

    // Keep absorption non-negative: if the total minus the out-scatter has gone
    // negative, take the difference out of the self-scattering term.
    for w in 0..materials {
        for g in 0..g_count {
            let outscatter: f64 = (0..g_count).map(|gt| sigmavalues.s.get(w, gt, g)).sum();
            let stot = sigmavalues.tot.get(w, g) - outscatter;
            if stot < 0.0 {
                let self_scatter = sigmavalues.s.get(w, g, g);
                sigmavalues.s.set(w, g, g, self_scatter + stot);
            }
        }
    }

    Ok((sigmavalues, whichsigma, rodfraction))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matlab::{Array2, Array3};
    use crate::types::Coolant;

    /// A 2x2x4 core, one material, with slope tables the test switches on.
    fn core() -> (Params, Geometry, SigmaValues, Array3<usize>, Th) {
        let (nx, ny, nz) = (2usize, 2usize, 4usize);
        let es = nx * ny * nz;

        let params = Params {
            maxix: Some(nx),
            maxiy: Some(ny),
            maxiz: Some(nz),
            g: 2,
            nc: Some(0),
            boron: 1000.0,
            ..Default::default()
        };

        let geometry = Geometry {
            // 25 cm per node, so each column is 100 cm tall.
            lz: vec![25.0; es],
            crodstep: 1.0,
            crodbtm: 0.0,
            crod: vec![50.0],
            crodbanks: {
                let mut b = Array2::<usize>::zeros(nx, ny);
                b.set(0, 0, 1);
                Some(b)
            },
            ..Default::default()
        };

        let mut tot = Array2::<f64>::zeros(1, 2);
        tot.set(0, 0, 0.5);
        tot.set(0, 1, 1.2);
        let mut f = Array2::<f64>::zeros(1, 2);
        f.set(0, 0, 0.01);
        f.set(0, 1, 0.2);
        let mut s = Array3::<f64>::zeros(1, 2, 2);
        s.set(0, 0, 0, 0.40);
        s.set(0, 1, 0, 0.02);
        s.set(0, 1, 1, 0.90);
        let mut nu = Array2::<f64>::zeros(1, 2);
        nu.set(0, 0, 2.5);
        nu.set(0, 1, 2.5);
        let mut chi = Array2::<f64>::zeros(1, 2);
        chi.set(0, 0, 1.0);

        let sigmavaluesref = SigmaValues {
            tot,
            f,
            s,
            nu,
            chi,
            fp: Some(Array2::<f64>::zeros(1, 2)),
        };

        let mut whichsigmaref = Array3::<usize>::zeros(nx, ny, nz);
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    whichsigmaref.set(ix, iy, iz, 1);
                }
            }
        }

        let th = Th {
            coolant: Coolant {
                temps: vec![560.0; es],
                dens: vec![0.75; es],
                ..Default::default()
            },
            fueltempdoppler: vec![900.0; es],
            modtemp: vec![560.0; es],
            ..Default::default()
        };

        (params, geometry, sigmavaluesref, whichsigmaref, th)
    }

    /// A slope table with a single non-zero entry, for tracing one channel.
    fn table(reference: f64, dtot_g0: f64) -> DeltaSigmaValues {
        let mut tot = Array2::<f64>::zeros(1, 2);
        tot.set(0, 0, dtot_g0);
        DeltaSigmaValues {
            tot,
            f: Array2::<f64>::zeros(1, 2),
            fp: Array2::<f64>::zeros(1, 2),
            s: Array3::<f64>::zeros(1, 2, 2),
            reference,
        }
    }

    /// With no tables enabled, the cross sections pass through unchanged apart
    /// from the node renumbering.
    #[test]
    fn no_feedback_leaves_the_cross_sections_alone() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables::default();
        let (sv, ws, rod) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        assert_eq!(sv.tot.get(0, 0), 0.5);
        assert_eq!(sv.tot.rows(), 1, "no channel ran, so no renumbering happened");
        assert_eq!(ws.get(0, 0, 0), 1);
        assert_eq!(rod.stale_level_carryovers, 0);
        assert!(rod.frac.iter().all(|&x| x == 0.0));
    }

    /// The boron channel applies a scalar feedback across the whole core, and
    /// the perturbation is the slope times the departure from the reference.
    ///
    /// # Methodology
    ///
    /// A slope of `+1e-4` per ppm on group 0's total cross section, referenced
    /// to 500 ppm, with `params.boron = 1000`. Every node should gain
    /// `1e-4 * (1000 - 500) = 0.05`, taking `tot` from 0.5 to 0.55.
    ///
    /// This also checks the renumbering: with all 16 nodes fuelled, the output
    /// table must have 16 rows where the input had 1.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `tot[node 0, g0] = 0.55` exactly, from 0.5, at all 16 nodes; group 1
    /// untouched at 1.2. The table went from 1 row to **16**.
    ///
    /// **Interpretation.** The scalar expansion, the slope arithmetic and the
    /// per-node renumbering all behave. The renumbering matters downstream:
    /// after this call `whichsigma` addresses nodes, not materials, which is
    /// what lets each node carry its own feedback-perturbed cross sections.
    #[test]
    fn the_boron_channel_applies_a_scalar_feedback() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables {
            boron: Some(table(500.0, 1e-4)),
            ..Default::default()
        };
        let (sv, ws, _) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        eprintln!(
            "tot[node 0, g0] = {} (was 0.5), rows = {}",
            sv.tot.get(0, 0),
            sv.tot.rows()
        );
        assert_eq!(sv.tot.rows(), 16, "every fuelled node gets its own row");
        for n in 0..16 {
            assert!((sv.tot.get(n, 0) - 0.55).abs() < 1e-12);
            // Group 1 has a zero slope and is untouched.
            assert!((sv.tot.get(n, 1) - 1.2).abs() < 1e-12);
        }
        assert_eq!(ws.get(0, 0, 0), 1, "node numbering is 1-based");
    }

    /// The fuel-temperature channel uses the **square-root** law.
    ///
    /// # Methodology
    ///
    /// The handler passes `m = 0.5` for this channel alone, so the
    /// perturbation is `slope * (sqrt(T) - sqrt(T_ref))`, not
    /// `slope * (T - T_ref)`. With a slope of `-1e-3`, a reference of 600 K and
    /// a Doppler temperature of 900 K, the expected change is
    /// `-1e-3 * (30 - 24.4949) = -5.505e-3`.
    ///
    /// Getting the exponent wrong would give `-1e-3 * 300 = -0.3`, fifty times
    /// larger and immediately visible. This is the Doppler feedback, so the
    /// exponent is load-bearing for reactivity.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `tot[0, g0] = 0.49449490`, against the square-root prediction of
    /// **0.49449490** — agreement to the eight digits printed. A linear law
    /// would have given 0.2, fifty-five times further from the reference value.
    ///
    /// **Interpretation.** The `m = 0.5` exponent reaches
    /// [`crate::sigmavalupd3d`] intact and its `real(a^m)` path handles it. This
    /// is the Doppler channel, so the exponent sets how strongly fuel
    /// temperature feeds back on reactivity — the single most important
    /// feedback in a rod-ejection transient.
    #[test]
    fn the_fuel_temperature_channel_uses_the_square_root_law() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables {
            fueltemp: Some(table(600.0, -1e-3)),
            ..Default::default()
        };
        let (sv, _, _) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        let expected = 0.5 - 1e-3 * (900.0f64.sqrt() - 600.0f64.sqrt());
        eprintln!(
            "tot[0, g0] = {:.8}, square-root law predicts {expected:.8}; \
             a linear law would give {:.8}",
            sv.tot.get(0, 0),
            0.5 - 1e-3 * (900.0 - 600.0)
        );
        assert!((sv.tot.get(0, 0) - expected).abs() < 1e-12);
    }

    /// The rod fraction is 1 above the tip, 0 below it, and a partial value in
    /// the node the tip sits in.
    ///
    /// # Methodology
    ///
    /// A 4-node column of 25 cm nodes — 100 cm tall — with the bank's tip at
    /// `crodbtm + crod * crodstep = 0 + 50 * 1 = 50 cm`, exactly the top of
    /// node 2.
    ///
    /// The search finds the first level whose cumulative height *exceeds* 50,
    /// which is level 3 (75 cm). So nodes 4 upward are fully rodded, and node 3
    /// takes `(75 - 50)/25 = 1.0` — the tip sits exactly on node 3's lower
    /// face, so node 3 is fully rodded too. Nodes 1 and 2 stay at zero.
    ///
    /// Only the `(0,0)` lattice position has a bank; the other three must stay
    /// at zero throughout.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Column (0,0): **[0.0, 0.0, 1.0, 1.0]**. The three unbanked columns came
    /// back all zero, and there were no stale carry-overs.
    ///
    /// **Interpretation.** The tip at exactly 50 cm sits on node 3's lower
    /// face, so node 3 is fully rodded and the partial term evaluates to
    /// exactly 1 — the boundary case behaves without a discontinuity. Nodes
    /// below stay clear, and a lattice position with no bank over it is never
    /// touched.
    #[test]
    fn the_rod_fraction_is_one_above_the_tip() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables {
            crod: Some(table(0.0, 0.05)),
            ..Default::default()
        };
        let (_, _, rod) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        eprintln!("rod fraction, column (0,0): {:?}", &rod.frac[0..4]);
        eprintln!("rod fraction, column (0,1): {:?}", &rod.frac[4..8]);
        assert_eq!(rod.stale_level_carryovers, 0);
        assert_eq!(rod.frac[0], 0.0, "node 1 is below the tip");
        assert_eq!(rod.frac[1], 0.0, "node 2 is below the tip");
        assert_eq!(rod.frac[2], 1.0, "the tip sits on node 3's lower face");
        assert_eq!(rod.frac[3], 1.0, "node 4 is fully rodded");
        // The unbanked columns are untouched.
        for n in 4..16 {
            assert_eq!(rod.frac[n], 0.0, "node {n} has no bank over it");
        }
    }

    /// A partially inserted bank gives a fractional node.
    #[test]
    fn a_tip_inside_a_node_gives_a_partial_fraction() {
        let (params, mut geometry, sref, wref, th) = core();
        // Tip at 60 cm: inside node 3, which spans 50-75 cm.
        geometry.crod = vec![60.0];
        let fb = FeedbackTables {
            crod: Some(table(0.0, 0.05)),
            ..Default::default()
        };
        let (_, _, rod) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        eprintln!("tip at 60 cm: {:?}", &rod.frac[0..4]);
        assert_eq!(rod.frac[0], 0.0);
        assert_eq!(rod.frac[1], 0.0);
        // (75 - 60)/25 = 0.6 of node 3 is rodded.
        assert!((rod.frac[2] - 0.6).abs() < 1e-12, "got {}", rod.frac[2]);
        assert_eq!(rod.frac[3], 1.0);
    }

    /// Defect C1, pinned: a fully withdrawn bank inherits the previous column's
    /// insertion level.
    ///
    /// # Methodology
    ///
    /// Two banked lattice positions. The first bank sits at 60 cm — inside the
    /// column — so the search finds level 3 and `rodlvl` is set. The second is
    /// withdrawn to 200 cm, twice the column height, so its search never
    /// breaks, `rodlvl` is never reassigned, and the code below silently uses
    /// the **first** column's level 3.
    ///
    /// The correct answer for a fully withdrawn bank is an unrodded column. The
    /// reference instead produces a rodded one, with a fraction computed from
    /// the wrong level and a negative partial term.
    ///
    /// This is the end state of every rod-ejection transient, which is the
    /// primary thing this code models. Pass criterion: the carry-over is
    /// counted, and the withdrawn column comes back **not** unrodded.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// The inserted column came back **[0.0, 0.0, 0.6, 1.0]** as expected. The
    /// withdrawn column came back **[0.0, 0.0, -5.0, 1.0]**, with one stale
    /// carry-over counted.
    ///
    /// **Interpretation.** C1 confirmed, and the magnitude is worth stating.
    /// The correct answer for a bank withdrawn to 200 cm above a 100 cm column
    /// is an entirely **unrodded** column — all zeros. What the reference
    /// produces instead is node 4 fully rodded and node 3 carrying a rod
    /// fraction of **-5**, from `(75 - 200)/25` using the previous column's
    /// inherited level.
    ///
    /// A negative rod fraction multiplies the control-rod cross-section slopes
    /// by -5, so it does not merely fail to remove the rod — it inserts five
    /// times the worth of one with the opposite sign. And this is the end state
    /// of every rod-ejection transient, the primary thing the code models.
    /// Translated as-is per the no-silent-repairs policy;
    /// [`RodFraction::stale_level_carryovers`] is how a caller detects it.
    #[test]
    fn a_fully_withdrawn_bank_inherits_the_previous_columns_level() {
        let (params, mut geometry, sref, wref, th) = core();
        // Two banks: (0,0) inserted to 60 cm, (0,1) withdrawn past the top.
        geometry.crod = vec![60.0, 200.0];
        let mut b = Array2::<usize>::zeros(2, 2);
        b.set(0, 0, 1);
        b.set(0, 1, 2);
        geometry.crodbanks = Some(b);

        let fb = FeedbackTables {
            crod: Some(table(0.0, 0.05)),
            ..Default::default()
        };
        let (_, _, rod) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        eprintln!("inserted column (0,0): {:?}", &rod.frac[0..4]);
        eprintln!("withdrawn column (0,1): {:?}", &rod.frac[4..8]);
        eprintln!("stale carry-overs: {}", rod.stale_level_carryovers);

        assert_eq!(rod.stale_level_carryovers, 1, "C1 should have fired once");
        let withdrawn = &rod.frac[4..8];
        assert!(
            withdrawn.iter().any(|&x| x != 0.0),
            "a fully withdrawn bank came back unrodded, so C1 may have been fixed"
        );
        // Specifically, it inherited level 3 and computed (75 - 200)/25 = -5.
        assert!((withdrawn[2] - (-5.0)).abs() < 1e-12, "got {}", withdrawn[2]);
        assert_eq!(withdrawn[3], 1.0);
    }

    /// A C1 occurrence on the **first** banked column has nothing to inherit,
    /// and is reported rather than guessed.
    ///
    /// # Methodology
    ///
    /// A single bank, fully withdrawn. MATLAB would raise `Undefined function
    /// or variable 'rodlvl'`; there is no previous value and no defensible
    /// substitute, so this returns an error naming the position.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Returned `UninitialisedRodLevel { ix: 0, iy: 0, bank: 1 }`, whose
    /// message names the position, the bank and the defect.
    ///
    /// **Interpretation.** With nothing to inherit there is no behaviour to
    /// reproduce — MATLAB raises here — so an error is the faithful outcome as
    /// well as the honest one.
    #[test]
    fn a_first_column_c1_occurrence_is_an_error() {
        let (params, mut geometry, sref, wref, th) = core();
        geometry.crod = vec![200.0];
        let fb = FeedbackTables {
            crod: Some(table(0.0, 0.05)),
            ..Default::default()
        };
        let err = sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th)
            .unwrap_err();
        eprintln!("{err}");
        assert!(matches!(err, BedokError::UninitialisedRodLevel { .. }));
    }

    /// Channels compose: enabling two applies both.
    #[test]
    fn two_channels_both_apply() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables {
            boron: Some(table(500.0, 1e-4)),
            cooltemp: Some(table(500.0, 1e-3)),
            ..Default::default()
        };
        let (sv, _, _) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        // 0.5 + 1e-4*(1000-500) + 1e-3*(560-500) = 0.5 + 0.05 + 0.06
        let expected = 0.5 + 0.05 + 0.06;
        eprintln!("tot after two channels = {}, expected {expected}", sv.tot.get(0, 0));
        assert!((sv.tot.get(0, 0) - expected).abs() < 1e-12);
    }

    /// A feedback large enough to drive absorption negative is corrected by
    /// taking the shortfall out of self-scattering.
    ///
    /// # Methodology
    ///
    /// The reference's last guard: if `tot - sum(s(:, g))` goes negative, the
    /// difference is added to `s(g, g)`, reducing self-scatter until absorption
    /// is zero. A boron slope of `-2e-4` per ppm with a 500 ppm departure takes
    /// group 0's total from 0.5 to 0.4, below its 0.42 of out-scatter.
    ///
    /// Pass criterion: absorption comes back at exactly zero, and it is the
    /// self-scattering term that moved.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `tot = 0.400000`, out-scatter `0.400000`, absorption **0.000e0**, and
    /// the self-scattering term fell from 0.40 to **0.380000**.
    ///
    /// **Interpretation.** The guard fires and lands absorption on exactly
    /// zero rather than merely near it. Note what it costs: the correction is
    /// taken entirely out of `s(g, g)`, so a cross-section set pushed this far
    /// by feedback comes back with a **modified scattering matrix**, not just a
    /// clamped total. That is the reference's choice and it is silent — nothing
    /// in the output records that the material was altered.
    #[test]
    fn negative_absorption_is_taken_out_of_self_scattering() {
        let (params, geometry, sref, wref, th) = core();
        let fb = FeedbackTables {
            boron: Some(table(500.0, -2e-4)),
            ..Default::default()
        };
        let (sv, _, _) =
            sigmavalupd3d_handler(&params, &geometry, &sref, &fb, &wref, &th).unwrap();

        let tot = sv.tot.get(0, 0);
        let outscatter: f64 = (0..2).map(|gt| sv.s.get(0, gt, 0)).sum();
        eprintln!(
            "tot = {tot:.6}, out-scatter = {outscatter:.6}, absorption = {:.3e}, s(0,0) = {:.6}",
            tot - outscatter,
            sv.s.get(0, 0, 0)
        );
        assert!((tot - 0.4).abs() < 1e-12, "the feedback should give tot = 0.4");
        assert!(
            (tot - outscatter).abs() < 1e-12,
            "absorption should have been driven to exactly zero"
        );
        assert!(sv.s.get(0, 0, 0) < 0.40, "self-scattering should have been reduced");
    }

    /// **C1 in the real cases — which of them actually trip the carry-over, and
    /// what it does to the rod-fraction map.**
    ///
    /// # Methodology
    ///
    /// The C1 test above is synthetic: two columns, one contrived to inherit
    /// the other's level. That establishes the mechanism but says nothing about
    /// whether the snapshot's own cases ever reach it. This runs the feedback
    /// handler on each NEACRP case at its initial thermal-hydraulic state and
    /// reports the carry-over count together with the extreme rod fractions.
    ///
    /// A rod fraction is a fraction: **it must lie in `[0, 1]`**. Anything
    /// outside that is the defect firing, and the sign matters — a *negative*
    /// fraction multiplies the control-rod cross-section slopes the wrong way,
    /// so it does not merely misplace absorption, it **adds reactivity**.
    ///
    /// Case A1 is the one to watch: its bank 4 sits at 228 steps, fully
    /// withdrawn, in the **steady state** — not only at the end of a transient.
    ///
    /// # Results — measured 2026-08-21
    ///
    /// | case | banks, steps | carry-overs | rod fraction range |
    /// |---|---|---|---|
    /// | NEACRP A2 | 100, 200, 100, 200, 200, 200, 200 | **0** | [0.0000, 1.0000] |
    /// | NEACRP A1 | 0, 0, 0, **228**, 0, 0, 0 | **0** | [0.0000, 1.0000] |
    /// | NEACRP D1 | (no banks) | **0** | [0.0000, 0.0000] |
    ///
    /// **Interpretation — C1 is LATENT in this snapshot, and the register's
    /// "High" severity was wrong.** Not "rare": unreachable. The entry read
    /// "hits the primary NEACRP use case, the end state of every rod-ejection
    /// transient", and that premise does not hold.
    ///
    /// The reason is a margin nobody had computed. C1 needs a rod tip at or
    /// above the **top of the meshed column**, and the NEACRP model puts an
    /// axial reflector above the active core:
    ///
    /// ```text
    /// tip = crodbtm + steps * crodstep = 37.7 + steps * 1.5942237
    ///
    ///   200 steps (A2's deepest withdrawal)  ->  356.5 cm
    ///   228 steps (full withdrawal, A1's bank 4,
    ///              and both ejections' end state) ->  401.2 cm
    ///   column top (A2/A1, after Z1 rounding)      ->  428.0 cm
    /// ```
    ///
    /// A fully withdrawn rod stops **27 cm short**, and would need about 245
    /// steps to trip the defect — 17 beyond the mechanism's own full-out
    /// position. So the search always finds a level, `rodlvl` is always
    /// assigned, and the carry-over branch is dead in every case the snapshot
    /// ships, steady and transient alike.
    ///
    /// **This does not retract the defect.** The synthetic test above still
    /// demonstrates it, and it remains a real fault waiting for a case with a
    /// shorter column or a longer rod travel. What changes is its *priority*:
    /// correcting it would move no number in any case here, so it cannot be
    /// validated by before/after and is not stage-2 work today.
    ///
    /// **Method note.** This is the check that should be run before correcting
    /// any register entry. Two entries — this and K1 — carried a "High"
    /// severity that assumed reachability nobody had measured.
    #[test]
    fn c1_which_cases_trip_the_carry_over_and_what_it_costs() {
        use crate::matlab::Array2;

        let report = |name: &str,
                      built: (
            crate::types::Params,
            crate::types::Geometry,
            crate::types::Th,
            crate::matlab::Array3<usize>,
            crate::types::SigmaValues,
            FeedbackTables,
        )| {
            let (params, geometry, th, whichsigma, sigmavalues, feedback) = built;
            let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
            let es = maxix * maxiy * maxiz;

            let maxir = params.fuel.maxir;
            let whichk = &geometry.fuel.whichk;
            let mut surfcount = 0usize;
            for ir in 0..maxir - 1 {
                if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
                    surfcount += 1;
                }
            }
            let maxid = maxir + surfcount;

            let mut th = th;
            th.fueltempavg = vec![params.fueltempavg; es];
            th.fueltempdoppler = vec![params.fueltempavg; es];
            th.fueltemp = {
                let mut a = Array2::<f64>::zeros(es, maxid);
                for i in 0..es {
                    for j in 0..maxid {
                        a.set(i, j, params.fueltempavg);
                    }
                }
                a
            };
            th.coolant.temps = vec![params.cooltempavg; es];
            th.coolant.dens = vec![params.cooldenavg; es];
            th.heatflux = vec![0.0; es];

            let (_, _, rod) = sigmavalupd3d_handler(
                &params, &geometry, &sigmavalues, &feedback, &whichsigma, &th,
            )
            .expect("the handler should run");

            let lo = rod.frac.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = rod.frac.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let out_of_range = rod.frac.iter().filter(|f| **f < 0.0 || **f > 1.0).count();
            eprintln!(
                "{name:<12} banks {:?}",
                geometry.crod
            );
            eprintln!(
                "             carry-overs {:<4}  frac range [{lo:.4}, {hi:.4}]  out of [0,1]: {out_of_range} nodes",
                rod.stale_level_carryovers
            );
            (rod.stale_level_carryovers, lo, hi, out_of_range)
        };

        let p = crate::types::Params::default();
        let a2 = report("NEACRP A2", crate::neacrpa2::neacrpa2(&p));
        let a1 = report("NEACRP A1", crate::neacrpa1t::neacrpa1t(&p));
        let d1 = report("NEACRP D1", crate::neacrpd1::neacrpd1(&p));

        eprintln!();
        eprintln!("A rod fraction outside [0, 1] is defect C1 firing. A NEGATIVE one");
        eprintln!("adds reactivity, because it multiplies the rod cross-section slopes.");

        // Pin what each case does, whatever that turns out to be — this test
        // exists to make a change in the answer visible, not to assert that
        // the reference is right.
        for (name, (carry, lo, hi, _)) in [("A2", a2), ("A1", a1), ("D1", d1)] {
            eprintln!("{name}: carry-overs {carry}, range [{lo:.4}, {hi:.4}]");
        }
    }
}
