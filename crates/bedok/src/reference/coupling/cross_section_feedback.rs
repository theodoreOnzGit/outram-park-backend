//! Cross-section feedback — how the thermal-hydraulic state moves the
//! cross sections.
//!
//! # Provenance
//!
//! Translated from `sigmavalupd3d.m` and `sigmavalupd3d_handler.m` in Than Yan
//! Ren's (SNRSI) BEDOK MATLAB snapshot (`BEDOKfiles.zip`, sha256
//! `e45cd6f57be2087c…`, received 2026-08-05). Original author: **Than Yan
//! Ren**, Singapore Nuclear Research and Safety Institute. Translated with
//! permission; see `docs/bedok-port-scoping.md` §6.
//!
//! # What the feedback model is
//!
//! Every cross section is a **linear function of each feedback variable about
//! a tabulated reference point**, and the channels compose by summation:
//!
//! ```text
//! Sigma(node) = Sigma_ref(composition) + sum_channels  dSigma/dx * (x(node)^m - x_ref^m)
//! ```
//!
//! with `m = 1` for boron, moderator temperature, coolant temperature, coolant
//! density and rod fraction, and `m = 0.5` for the Doppler fuel temperature —
//! the usual square-root Doppler law. The channels are applied in a fixed
//! order, each consuming the previous channel's output, so the composition is
//! sequential rather than parallel; that ordering is preserved exactly.
//!
//! # The table-compaction side effect
//!
//! [`apply_feedback_channel`] does two things at once, and the second is easy
//! to miss: it applies one feedback channel **and** it rewrites the material
//! map. On the way in, `which_sigma` holds composition ids and the tables have
//! one row per composition; on the way out, `which_sigma` holds a row index
//! into a table with one row **per non-void node**. Every later channel
//! therefore reads per-node rows, while the derivative tables stay indexed by
//! composition throughout (they come from `which_sigma_ref`, never from the
//! running map).

use super::error::Result;
use super::seam::{
    fix_negative, pause_on_nan, CaseParams, CoreGeometry, FeedbackTable, FeedbackTables,
    MaterialMap, SigmaValues, ThermalState,
};
use std::f64::consts::PI;

/// `real(x^m)` with MATLAB's complex-power semantics.
///
/// MATLAB evaluates `x^m` for negative real `x` and non-integer `m` in the
/// complex plane, `x^m = |x|^m * exp(i*m*pi)`, and `real()` then keeps
/// `|x|^m * cos(m*pi)`. For `m = 1` that is `x` itself; for the Doppler
/// `m = 0.5` a negative temperature collapses to (numerically) zero rather
/// than producing NaN, which is why the MATLAB wraps every power in `real()`.
///
/// Reproduced rather than replaced by `x.powf(m)`, which would return NaN for
/// a negative base and change the behaviour on any node that goes unphysical.
#[must_use]
pub fn real_power(x: f64, m: f64) -> f64 {
    if x < 0.0 {
        x.abs().powf(m) * (m * PI).cos()
    } else {
        x.powf(m)
    }
}

/// Values of one feedback variable — a scalar applied everywhere, or one value
/// per spatial node.
///
/// MATLAB writes this as `if isscalar(currval); currval = currval*ones(es,1);`;
/// enum dispatch makes the two cases explicit instead.
#[derive(Debug, Clone)]
pub enum FeedbackVariable {
    /// One value for the whole core — how boron enters \[ppm\].
    Uniform(f64),
    /// One value per spatial node, `grid.nodes()` entries, indexed
    /// `Grid::index(0, ix, iy, iz)`. Temperatures \[K\], densities \[g/cm³\],
    /// rod fractions \[-\].
    PerNode(Vec<f64>),
}

impl FeedbackVariable {
    /// Value at spatial node index `idx`.
    #[must_use]
    fn at(&self, idx: usize) -> f64 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerNode(v) => v[idx],
        }
    }
}

/// Apply one feedback channel and compact the cross-section table to one row
/// per node.
///
/// MATLAB `sigmavalupd3d.m`:
/// `[sigmavalues, whichsigma] = sigmavalupd3d(params, sigmavaluesold,
/// whichsigmaold, whichsigmaref, deltasigmavalues, currval, m)`.
///
/// # Arguments
///
/// - `sigma_values_old` — cross sections to perturb. On the first channel these
///   are the case's per-composition tables; afterwards they are the previous
///   channel's per-node table.
/// - `which_sigma_old` — the map that indexes `sigma_values_old`: composition
///   ids on the first channel, per-node row indices afterwards.
/// - `which_sigma_ref` — the case's composition map, which always indexes
///   `delta`.
/// - `delta` — derivatives with respect to this channel's variable, one row per
///   composition.
/// - `current_value` — the variable's present value.
/// - `exponent` — the `m` of `x^m`; MATLAB `varargin{1}`, default 1.
///
/// # Returns
///
/// The perturbed cross sections (one row per non-void node, counted in
/// `ix, iy, iz` order) and the rewritten material map.
///
/// # Note on the feedback tables
///
/// The returned [`SigmaValues`] has **empty**
/// [`feedback`](SigmaValues::feedback) tables, matching the MATLAB: the
/// function builds a fresh `sigmavalues` struct and never copies the
/// `.boron` / `.fueltemp` / … sub-structs across. The handler is unaffected
/// because it always tests and reads them on the *reference* struct.
///
/// # Errors
///
/// [`super::error::CouplingError::NotANumber`] from the `pauseonnan` guards,
/// with the MATLAB's column semantics (see [`pause_on_nan`]).
pub fn apply_feedback_channel(
    params: &CaseParams,
    sigma_values_old: &SigmaValues,
    which_sigma_old: &MaterialMap,
    which_sigma_ref: &MaterialMap,
    delta: &FeedbackTable,
    current_value: &FeedbackVariable,
    exponent: f64,
) -> Result<(SigmaValues, MaterialMap)> {
    let grid = params.grid;
    let ngroups = grid.ngroups;
    let nodes = grid.nodes();

    let reference = real_power(delta.reference_value, exponent);

    // MATLAB preallocates to es rows and truncates to `counter` afterwards;
    // pushing rows in the same order is the same thing.
    let mut tot: Vec<f64> = Vec::with_capacity(nodes * ngroups);
    let mut f: Vec<f64> = Vec::with_capacity(nodes * ngroups);
    let mut fp: Vec<f64> = Vec::with_capacity(nodes * ngroups);
    let mut s: Vec<f64> = Vec::with_capacity(nodes * ngroups * ngroups);
    let mut nu: Vec<f64> = Vec::with_capacity(nodes * ngroups);
    let mut chi: Vec<f64> = Vec::with_capacity(nodes * ngroups);

    let mut which_sigma = MaterialMap::zeros(grid);
    let mut counter = 0_usize;

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                if which_sigma_ref.at(ix, iy, iz) == 0 {
                    continue;
                }
                counter += 1;
                // MATLAB `w` and `wold` are 1-based table rows.
                let w = which_sigma_ref.at(ix, iy, iz) - 1;
                let w_old = which_sigma_old.at(ix, iy, iz) - 1;
                which_sigma.set(ix, iy, iz, counter);

                let idx = grid.index(0, ix, iy, iz);
                let drive = real_power(current_value.at(idx), exponent) - reference;

                for g in 0..ngroups {
                    tot.push(
                        sigma_values_old.tot[w_old * ngroups + g]
                            + delta.tot[w * ngroups + g] * drive,
                    );
                    f.push(
                        sigma_values_old.f[w_old * ngroups + g] + delta.f[w * ngroups + g] * drive,
                    );
                    fp.push(
                        sigma_values_old.fp[w_old * ngroups + g]
                            + delta.fp[w * ngroups + g] * drive,
                    );
                    nu.push(sigma_values_old.nu[w_old * ngroups + g]);
                    chi.push(sigma_values_old.chi[w_old * ngroups + g]);
                }
                for to in 0..ngroups {
                    for from in 0..ngroups {
                        let old = sigma_values_old.scattering(w_old, to, from);
                        let d = delta.s[w * ngroups * ngroups + to * ngroups + from];
                        s.push(old + d * drive);
                    }
                }
            }
        }
    }

    let updated = SigmaValues {
        ngroups,
        tot,
        f,
        fp,
        s,
        nu,
        chi,
        feedback: FeedbackTables::default(),
    };

    // MATLAB `pauseonnan` on each field, in this order.
    pause_on_nan("sigmavalues.tot", &updated.tot, ngroups)?;
    pause_on_nan("sigmavalues.f", &updated.f, ngroups)?;
    pause_on_nan("sigmavalues.fp", &updated.fp, ngroups)?;
    pause_on_nan("sigmavalues.s", &updated.s, ngroups * ngroups)?;
    pause_on_nan("sigmavalues.nu", &updated.nu, ngroups)?;
    pause_on_nan("sigmavalues.chi", &updated.chi, ngroups)?;

    Ok((updated, which_sigma))
}

/// Fraction of each node covered by an inserted control rod \[-\], `0` (clear)
/// to `1` (fully rodded).
///
/// The control-rod part of MATLAB `sigmavalupd3d_handler.m` (lines 40–75).
/// Bank tip position is `crodbtm + crod*crodstep` \[cm\] measured from the
/// bottom of the column; nodes entirely above the tip are fully rodded, the
/// node straddling the tip gets the partial fraction, and nodes below it are
/// clear.
///
/// # Unfinished in the MATLAB — uninitialised `rodlvl`
///
/// The search for the straddling node,
///
/// ```text
/// for iz=1:maxiz
///     if sum(Lz(idx+1:idx+iz))>rodpos(rod); rodlvl=iz; break; end
/// end
/// ```
///
/// **never assigns `rodlvl` when the bank tip sits at or above the top of the
/// column** (a fully withdrawn bank, exactly the end state of a rod-ejection
/// transient). MATLAB then either errors with an undefined variable — if this
/// is the first rodded column — or, far worse, silently reuses the `rodlvl`
/// **left over from the previous `(ix,iy)` column**, producing a rod fraction
/// for a rod that is not there.
///
/// Translated as-is: `last_rod_level` carries across columns exactly as the
/// MATLAB workspace variable does, and a first-column occurrence returns
/// [`super::error::CouplingError::MissingCaseData`] where MATLAB would raise
/// its undefined-variable error. **Not fixed** — see
/// `docs/bedok-port-scoping.md` §1.0.
///
/// # Deviation — the `rodfrac.csv` dump
///
/// `sigmavalupd3d_handler.m:71` calls `writematrix(rodfrac,'rodfrac.csv')`
/// **unconditionally**, inside the feedback update — so the MATLAB rewrites
/// that file on every cross-section update, including once per Picard pass per
/// transient time step. It is a leftover debug dump, not an output; no file is
/// written here.
///
/// # Errors
///
/// [`super::error::CouplingError::MissingCaseData`] if the straddling-node
/// search fails before any column has ever set a level.
pub fn control_rod_fraction(params: &CaseParams, geometry: &CoreGeometry) -> Result<Vec<f64>> {
    let grid = params.grid;
    let mut rod_fraction = vec![0.0_f64; grid.nodes()];

    // rodpos(bank) = crodbtm + crod(bank)*crodstep  [cm]
    let rod_position: Vec<f64> = geometry
        .crod
        .iter()
        .map(|steps| geometry.crod_btm + steps * geometry.crod_step)
        .collect();

    // MATLAB's `rodlvl` lives in the function workspace, not the loop body.
    let mut last_rod_level: Option<usize> = None;

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let bank = geometry.crod_banks[ix * grid.ny + iy];
            if bank == 0 {
                continue;
            }
            let base = grid.index(0, ix, iy, 0);
            let tip = rod_position[bank - 1];

            // Cumulative column height from the bottom, up to and including iz.
            let mut cumulative = 0.0_f64;
            let mut found: Option<usize> = None;
            for iz in 0..grid.nz {
                cumulative += geometry.base.lz[base + iz];
                if cumulative > tip {
                    found = Some(iz + 1); // 1-based, as MATLAB's rodlvl
                    break;
                }
            }
            if let Some(level) = found {
                last_rod_level = Some(level);
            }
            let Some(rod_level) = last_rod_level else {
                return Err(super::error::CouplingError::MissingCaseData {
                    what: "control_rod_fraction: rodlvl never assigned (MATLAB \
                           sigmavalupd3d_handler.m leaves it undefined for a bank \
                           withdrawn above the top of the column)",
                });
            };

            // Column height up to and including `rod_level` (1-based).
            let height_to_level: f64 = (0..rod_level).map(|k| geometry.base.lz[base + k]).sum();

            if rod_level < grid.nz {
                for iz in rod_level..grid.nz {
                    rod_fraction[base + iz] = 1.0;
                }
                rod_fraction[base + rod_level - 1] =
                    (height_to_level - tip) / geometry.base.lz[base + rod_level - 1];
            } else {
                let full_height: f64 = (0..grid.nz).map(|k| geometry.base.lz[base + k]).sum();
                rod_fraction[base + grid.nz - 1] =
                    (full_height - tip) / geometry.base.lz[base + grid.nz - 1];
            }
        }
    }

    Ok(rod_fraction)
}

/// Apply every feedback channel the case defines, in the MATLAB's order.
///
/// MATLAB `sigmavalupd3d_handler.m`:
/// `[sigmavalues, whichsigma] = sigmavalupd3d_handler(params, geometry,
/// sigmavaluesref, whichsigmaref, th)`.
///
/// # Order — and why it is load-bearing
///
/// Channels are applied strictly in this sequence, each consuming the previous
/// one's output: **boron → Doppler fuel temperature → moderator temperature →
/// coolant temperature → coolant density → control rods**. Because each
/// perturbation is linear about the *reference* value and the derivative tables
/// are always read at the composition id, the sum is order-independent in exact
/// arithmetic — but the floating-point accumulation is not, and neither are the
/// clamps applied afterwards. The order is preserved.
///
/// # Post-processing clamps
///
/// After all channels:
///
/// 1. Negative fission (`f`), power (`fp`) and scattering entries are zeroed.
/// 2. Any composition whose absorption `Sigma_tot - sum_to Sigma_s(:,g)` has
///    gone negative has its **within-group scattering** reduced by the
///    shortfall, forcing the absorption to exactly zero.
///
/// Neither `tot` nor the within-group scattering is clamped directly, which is
/// what step 2 exists to compensate for.
///
/// # Arguments
///
/// - `sigma_values_ref` — the case's per-composition tables **and** the
///   derivative tables. Never modified.
/// - `which_sigma_ref` — the case's composition map. Never modified.
/// - `th` — the thermal-hydraulic state the feedback is evaluated at.
///
/// # Errors
///
/// [`super::error::CouplingError::NotANumber`] from the `pauseonnan` guards;
/// [`super::error::CouplingError::MissingCaseData`] if a `modtemp` channel is
/// declared but `th.mod_temp` is absent, or from
/// [`control_rod_fraction`].
pub fn update_cross_sections(
    params: &CaseParams,
    geometry: &CoreGeometry,
    sigma_values_ref: &SigmaValues,
    which_sigma_ref: &MaterialMap,
    th: &ThermalState,
) -> Result<(SigmaValues, MaterialMap)> {
    let mut sigma_values = sigma_values_ref.clone();
    let mut which_sigma = which_sigma_ref.clone();

    // ----- boron (linear, uniform) -----
    if let Some(table) = &sigma_values_ref.feedback.boron {
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            table,
            &FeedbackVariable::Uniform(params.boron),
            1.0,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- Doppler fuel temperature (square-root law) -----
    if let Some(table) = &sigma_values_ref.feedback.fuel_temp {
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            table,
            &FeedbackVariable::PerNode(th.fuel_temp_doppler.clone()),
            0.5,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- moderator temperature -----
    if let Some(table) = &sigma_values_ref.feedback.mod_temp {
        let Some(mod_temp) = &th.mod_temp else {
            return Err(super::error::CouplingError::MissingCaseData {
                what: "th.modtemp, required by the modtemp feedback channel",
            });
        };
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            table,
            &FeedbackVariable::PerNode(mod_temp.clone()),
            1.0,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- coolant temperature -----
    if let Some(table) = &sigma_values_ref.feedback.cool_temp {
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            table,
            &FeedbackVariable::PerNode(th.coolant.temps.clone()),
            1.0,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- coolant density -----
    if let Some(table) = &sigma_values_ref.feedback.cool_den {
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            table,
            &FeedbackVariable::PerNode(th.coolant.dens.clone()),
            1.0,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- control rods -----
    if let Some(table) = &sigma_values_ref.feedback.crod {
        let rod_fraction = control_rod_fraction(params, geometry)?;
        // MATLAB overrides the tabulated reference on a local copy:
        // `sigmavaluesref.crod.ref = 0` — rod fraction is perturbed about the
        // fully-unrodded state regardless of what the case tabulated.
        let mut table = table.clone();
        table.reference_value = 0.0;
        let (v, w) = apply_feedback_channel(
            params,
            &sigma_values,
            &which_sigma,
            which_sigma_ref,
            &table,
            &FeedbackVariable::PerNode(rod_fraction),
            1.0,
        )?;
        sigma_values = v;
        which_sigma = w;
    }

    // ----- clamps -----
    fix_negative(&mut sigma_values.f);
    fix_negative(&mut sigma_values.fp);
    // MATLAB loops `for g=1:G` over `sigmavalues.s(:,:,g)`, covering the whole
    // array; one sweep is the same operation.
    fix_negative(&mut sigma_values.s);

    let ngroups = sigma_values.ngroups;
    for row in 0..sigma_values.n_rows() {
        for g in 0..ngroups {
            let scatter_out: f64 = (0..ngroups)
                .map(|to| sigma_values.scattering(row, to, g))
                .sum();
            let absorption = sigma_values.tot[row * ngroups + g] - scatter_out;
            if absorption < 0.0 {
                *sigma_values.scattering_mut(row, g, g) += absorption;
            }
        }
    }

    // MATLAB `pauseonnan` on each field, in this order.
    pause_on_nan("sigmavalues.tot", &sigma_values.tot, ngroups)?;
    pause_on_nan("sigmavalues.f", &sigma_values.f, ngroups)?;
    pause_on_nan("sigmavalues.fp", &sigma_values.fp, ngroups)?;
    pause_on_nan("sigmavalues.s", &sigma_values.s, ngroups * ngroups)?;
    pause_on_nan("sigmavalues.nu", &sigma_values.nu, ngroups)?;
    pause_on_nan("sigmavalues.chi", &sigma_values.chi, ngroups)?;

    Ok((sigma_values, which_sigma))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_power_reproduces_matlab_complex_semantics() {
        // m = 1: identity, including for negative arguments.
        assert_eq!(real_power(-4.0, 1.0), -4.0);
        assert_eq!(real_power(4.0, 1.0), 4.0);
        // m = 0.5 on a positive argument is the ordinary square root.
        assert!((real_power(900.0, 0.5) - 30.0).abs() < 1e-12);
        // m = 0.5 on a negative argument: |x|^m * cos(m*pi), which is ~0 rather
        // than NaN. MATLAB `real((-4)^0.5)` gives the same ~1.2e-16.
        let negative = real_power(-4.0, 0.5);
        assert!(negative.abs() < 1e-15, "got {negative}");
        assert!(negative.is_finite());
    }

    #[test]
    fn zero_base_is_finite_under_the_doppler_exponent() {
        assert_eq!(real_power(0.0, 0.5), 0.0);
    }
}
