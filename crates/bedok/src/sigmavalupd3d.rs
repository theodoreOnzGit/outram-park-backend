//! Cross-section feedback — rebuild the material table from a per-node state.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `sigmavalupd3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::error::Result;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array3};
use crate::pauseonnan::pauseonnan;
use crate::types::{Params, SigmaValues};

/// The per-material feedback slopes, plus the state they are referenced to.
///
/// `deltasigmavalues` in the reference. Each row is a material; the columns
/// match [`SigmaValues`].
#[derive(Clone, Debug, Default)]
pub struct DeltaSigmaValues {
    /// Slope of the total cross section against the feedback variable.
    pub tot: Array2<f64>,
    /// Slope of the fission cross section.
    pub f: Array2<f64>,
    /// Slope of the prompt fission cross section.
    pub fp: Array2<f64>,
    /// Slope of the scattering matrix, indexed `(material, gt, g)`.
    pub s: Array3<f64>,
    /// `deltasigmavalues.ref` — the reference state the slopes are taken about.
    ///
    /// Named `reference` because `ref` is a Rust keyword.
    pub reference: f64,
}

/// `real(a^m)` with MATLAB's complex-power semantics.
///
/// **This is the trap in this file.** For a negative `a` and a non-integer `m`,
/// MATLAB's `^` produces a *complex* result and the reference wraps every use
/// in `real(...)`. Rust's [`f64::powf`] returns `NaN` for the same input, so a
/// naive transcription would turn a physically meaningful feedback term into
/// `NaN` and trip the `pauseonnan` guard at the end of this very function.
///
/// The principal value is `|a|^m (cos(pi m) + i sin(pi m))`, so
///
/// ```text
/// real(a^m) = |a|^m * cos(pi * m)      for a < 0
/// ```
///
/// which reduces to the ordinary signed power when `m` is an integer, and to
/// `0` at `m = 0.5` — a negative argument under a square-root feedback law
/// contributes nothing rather than erroring.
///
/// # Why it matters here
///
/// `m` defaults to `1`, where none of this bites. It is passed explicitly for
/// feedback laws like the square-root Doppler dependence, where `m = 0.5` and a
/// `currval` that dips below zero is exactly the case this handles.
fn real_pow(a: f64, m: f64) -> f64 {
    if a < 0.0 {
        a.abs().powf(m) * (std::f64::consts::PI * m).cos()
    } else {
        a.powf(m)
    }
}

/// `[sigmavalues, whichsigma] = sigmavalupd3d(params, sigmavaluesold, whichsigmaold, whichsigmaref, deltasigmavalues, currval, m)`.
///
/// Applies the thermal-hydraulic feedback to the cross sections and, in doing
/// so, **re-numbers the material table one row per fuelled node**.
///
/// # What it actually does to the material numbering
///
/// This is the important structural point. On the way in, several nodes may
/// share a material row. On the way out, every fuelled node has been given its
/// **own** row, numbered in the scan order `ix`, `iy`, `iz`, and the returned
/// `whichsigma` points each node at its private row. That is what lets each
/// node carry a different temperature or density.
///
/// The returned table therefore has exactly as many rows as there are fuelled
/// nodes, and `whichsigma` is a fresh 1-based numbering with `0` for void —
/// the same convention [`crate::calcdiffvalues3d`] and
/// [`crate::makesigmadfxyz`] consume.
///
/// # Arguments
///
/// - `sigmavaluesold` — the current table, indexed by `whichsigmaold`.
/// - `whichsigmaold` — material per node for `sigmavaluesold`.
/// - `whichsigmaref` — material per node for `deltasigmavalues`, and the mask
///   deciding which nodes are fuelled at all.
/// - `deltasigmavalues` — feedback slopes and their reference state.
/// - `currval` — the feedback variable per node, `es` long. The reference
///   accepts a scalar and broadcasts it; pass a filled vector for that case.
/// - `m` — exponent applied to the feedback variable. `None` selects the
///   reference's default of `1`.
///
/// # The feedback law
///
/// For each perturbed quantity,
///
/// $$ \Sigma = \Sigma_{old} + \frac{d\Sigma}{dv}\left(\mathrm{Re}(v^m) - \mathrm{Re}(v_{ref}^m)\right) $$
///
/// **`nu` and `chi` are not perturbed** — they are copied straight from
/// `sigmavaluesold`. Only `tot`, `f`, `fp` and `s` carry feedback.
///
/// # Two index spaces, and they are not the same
///
/// `sigmavaluesold` is indexed by `whichsigmaold`, while `deltasigmavalues` is
/// indexed by `whichsigmaref`. A node reads its base value from one table and
/// its slope from the other, at different row numbers. Conflating them would
/// pair each node with the wrong slope, and — because both are valid rows —
/// would produce plausible numbers rather than an error.
///
/// # Absent `fp`
///
/// [`SigmaValues::fp`] is optional, matching the reference's `isfield` guard in
/// `makesigmadfxyz`. This function reads it unguarded, so an absent `fp` is
/// treated as zeros here and the output carries a zero `fp` column. The
/// reference would raise `Reference to non-existent field`.
///
/// # Errors
///
/// [`crate::error::BedokError::NanEncountered`] if any output quantity contains
/// `NaN` — the reference runs `pauseonnan` over all six on the way out.
#[allow(clippy::too_many_arguments)]
pub fn sigmavalupd3d(
    params: &Params,
    sigmavaluesold: &SigmaValues,
    whichsigmaold: &Array3<usize>,
    whichsigmaref: &Array3<usize>,
    deltasigmavalues: &DeltaSigmaValues,
    currval: &[f64],
    m: Option<f64>,
) -> Result<(SigmaValues, Array3<usize>)> {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;

    let m = m.unwrap_or(1.0);
    let reference = real_pow(deltasigmavalues.reference, m);

    // Count the fuelled nodes so the table can be sized exactly, where the
    // reference over-allocates to `es` and truncates.
    let mut fuelled = 0usize;
    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                if whichsigmaref.get(ix, iy, iz) != 0 {
                    fuelled += 1;
                }
            }
        }
    }

    let mut out = SigmaValues {
        tot: Array2::<f64>::zeros(fuelled, g_count),
        f: Array2::<f64>::zeros(fuelled, g_count),
        s: Array3::<f64>::zeros(fuelled, g_count, g_count),
        nu: Array2::<f64>::zeros(fuelled, g_count),
        chi: Array2::<f64>::zeros(fuelled, g_count),
        fp: Some(Array2::<f64>::zeros(fuelled, g_count)),
    };
    let mut whichsigma = Array3::<usize>::zeros(maxix, maxiy, maxiz);

    let mut counter = 0usize;
    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                let w = whichsigmaref.get(ix, iy, iz);
                if w == 0 {
                    continue;
                }
                counter += 1;
                let wold = whichsigmaold.get(ix, iy, iz);
                // The fresh per-node numbering is 1-based with 0 for void.
                whichsigma.set(ix, iy, iz, counter);

                let idx = ix * xstep + iy * maxiz + iz;
                let drive = real_pow(currval[idx], m) - reference;

                // Rows: `counter` is 1-based, the arrays are 0-based; `w` and
                // `wold` index two *different* tables.
                let r = counter - 1;
                let rw = w - 1;
                let rold = wold - 1;

                for g in 0..g_count {
                    out.tot.set(
                        r,
                        g,
                        sigmavaluesold.tot.get(rold, g) + deltasigmavalues.tot.get(rw, g) * drive,
                    );
                    out.f.set(
                        r,
                        g,
                        sigmavaluesold.f.get(rold, g) + deltasigmavalues.f.get(rw, g) * drive,
                    );
                    let fp_old = sigmavaluesold
                        .fp
                        .as_ref()
                        .map(|a| a.get(rold, g))
                        .unwrap_or(0.0);
                    if let Some(fp) = out.fp.as_mut() {
                        fp.set(r, g, fp_old + deltasigmavalues.fp.get(rw, g) * drive);
                    }
                    for gt in 0..g_count {
                        out.s.set(
                            r,
                            gt,
                            g,
                            sigmavaluesold.s.get(rold, gt, g)
                                + deltasigmavalues.s.get(rw, gt, g) * drive,
                        );
                    }
                    // nu and chi carry no feedback.
                    out.nu.set(r, g, sigmavaluesold.nu.get(rold, g));
                    out.chi.set(r, g, sigmavaluesold.chi.get(rold, g));
                }
            }
        }
    }

    pauseonnan(out.tot.as_slice())?;
    pauseonnan(out.f.as_slice())?;
    if let Some(fp) = out.fp.as_ref() {
        pauseonnan(fp.as_slice())?;
    }
    pauseonnan(out.s.as_slice())?;
    pauseonnan(out.nu.as_slice())?;
    pauseonnan(out.chi.as_slice())?;

    Ok((out, whichsigma))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            maxix: Some(2),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 1,
            ..Default::default()
        }
    }

    /// One material shared by two nodes, with distinct feedback values.
    fn setup() -> (SigmaValues, Array3<usize>, Array3<usize>, DeltaSigmaValues) {
        let mut tot = Array2::<f64>::zeros(1, 1);
        tot.set(0, 0, 1.0);
        let mut nu = Array2::<f64>::zeros(1, 1);
        nu.set(0, 0, 2.4);
        let old = SigmaValues {
            tot,
            f: Array2::<f64>::zeros(1, 1),
            s: Array3::<f64>::zeros(1, 1, 1),
            nu,
            chi: Array2::<f64>::zeros(1, 1),
            fp: None,
        };

        let mut which = Array3::<usize>::zeros(2, 1, 1);
        which.set(0, 0, 0, 1);
        which.set(1, 0, 0, 1);

        let mut dtot = Array2::<f64>::zeros(1, 1);
        dtot.set(0, 0, 0.5);
        let delta = DeltaSigmaValues {
            tot: dtot,
            f: Array2::<f64>::zeros(1, 1),
            fp: Array2::<f64>::zeros(1, 1),
            s: Array3::<f64>::zeros(1, 1, 1),
            reference: 100.0,
        };

        (old, which.clone(), which, delta)
    }

    /// Every fuelled node gets its own material row, and `whichsigma` is
    /// renumbered to match.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Two nodes sharing material 1 come out as rows 1 and 2 with different
    /// total cross sections, driven by their different `currval`.
    #[test]
    fn each_fuelled_node_gets_its_own_material_row() {
        let (old, wold, wref, delta) = setup();
        let (out, which) =
            sigmavalupd3d(&params(), &old, &wold, &wref, &delta, &[100.0, 120.0], None).unwrap();

        assert_eq!(which.get(0, 0, 0), 1);
        assert_eq!(which.get(1, 0, 0), 2);
        // Node 0 sits at the reference, so it is unperturbed.
        assert_eq!(out.tot.get(0, 0), 1.0);
        // Node 1 is 20 above it: 1.0 + 0.5*20 = 11.0.
        assert_eq!(out.tot.get(1, 0), 11.0);
    }

    /// `nu` and `chi` carry no feedback.
    #[test]
    fn nu_and_chi_are_copied_not_perturbed() {
        let (old, wold, wref, delta) = setup();
        let (out, _) =
            sigmavalupd3d(&params(), &old, &wold, &wref, &delta, &[100.0, 120.0], None).unwrap();
        assert_eq!(out.nu.get(0, 0), 2.4);
        assert_eq!(out.nu.get(1, 0), 2.4);
    }

    /// Void nodes are excluded from the table and keep a `whichsigma` of zero.
    #[test]
    fn void_nodes_are_excluded() {
        let (old, wold, mut wref, delta) = setup();
        wref.set(1, 0, 0, 0);
        let (out, which) =
            sigmavalupd3d(&params(), &old, &wold, &wref, &delta, &[100.0, 120.0], None).unwrap();
        assert_eq!(which.get(1, 0, 0), 0);
        assert_eq!(out.tot.rows(), 1);
    }

    /// Pins the MATLAB complex-power semantics: a negative argument under a
    /// square-root feedback law contributes `0`, where `f64::powf` would give
    /// `NaN` and trip the `pauseonnan` guard.
    ///
    /// # Methodology
    ///
    /// `real((-4)^0.5) = |-4|^0.5 * cos(pi/2) = 2 * 0 = 0`. Integer exponents
    /// must still behave as ordinary signed powers.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `real_pow(-4, 0.5)` is zero to within 1e-15; `real_pow(-2, 3)` is `-8`.
    #[test]
    fn negative_arguments_follow_matlabs_complex_power() {
        assert!(real_pow(-4.0, 0.5).abs() < 1e-15);
        assert!((real_pow(-2.0, 3.0) + 8.0).abs() < 1e-12);
        assert!((real_pow(-2.0, 2.0) - 4.0).abs() < 1e-12);
        assert!((real_pow(9.0, 0.5) - 3.0).abs() < 1e-12);
    }

    /// A square-root feedback law with a negative `currval` must produce finite
    /// output rather than failing the `NaN` guard.
    #[test]
    fn a_negative_currval_under_a_root_law_stays_finite() {
        let (old, wold, wref, delta) = setup();
        let r = sigmavalupd3d(
            &params(),
            &old,
            &wold,
            &wref,
            &delta,
            &[-4.0, 121.0],
            Some(0.5),
        );
        let (out, _) = r.expect("should not trip the NaN guard");
        assert!(out.tot.as_slice().iter().all(|v| v.is_finite()));
    }
}
