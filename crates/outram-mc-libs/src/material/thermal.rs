//! S(α,β) thermal scattering tables — the bound-atom scattering treatment.
//!
//! C++ analogue: `src/thermal.cpp`, `include/openmc/thermal.h` (the ACE
//! `ThermalScattering` / `ThermalData` blocks).
//!
//! **Why this exists.** Below a few eV the free-gas treatment of scattering from
//! bound atoms is wrong: chemical binding raises the bound cross section
//! (≈ 81.8 b per H in H₂O at E→0 vs the 20.4 b free-atom limit) and lets the
//! neutron *up-scatter* off the thermal motion of the molecule, which is what
//! establishes the Maxwellian thermal spectrum in a moderator. A fast-metal
//! sphere (Godiva) never reaches these energies, but a *thermal* LWR pin-cell
//! lives or dies on it.
//!
//! **Data-free, like the rest of `outram-mc-libs`.** This struct holds no ENDF
//! parsing of its own: it is built from the njoy consumer surface
//! [`njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering`],
//! which reads the ENDF/B `tsl-*` thermal evaluation and answers the two
//! transport questions — σ_inel(E) and the secondary energy/angle distribution.
//! To keep the transport hot-loop cheap (that kernel integrates the S(α,β)
//! double-differential per call), this type **pre-tabulates** both quantities on
//! a fixed energy grid at construction, exactly as NJOY/ACE bakes the ITIE/ITXE
//! blocks, and the run-time path only interpolates and samples.
//!
//! **Scope.** Only the incoherent-*inelastic* channel — the one H-in-H₂O needs
//! (light water has no thermal elastic). Coherent / incoherent-elastic bound
//! scattering (graphite, ZrH) is deliberately not wired here yet.

use crate::rng::lcg::prn;

/// Default upper energy \[eV\] of the S(α,β) treatment — the "thermal cutoff".
///
/// Above it the neutron sees the ordinary free-gas / WMP elastic channel; below
/// it the bound S(α,β) treatment replaces elastic scattering off the principal
/// atom. 4 eV is the OpenMC/NJOY convention for light-water thermal tables
/// (`ENERGY_MAX_THERMAL`-class cutoff): by ~4 eV the S(α,β) cross section has
/// relaxed to the free-atom limit and the up-scatter probability is negligible,
/// so the join to free-gas is smooth.
pub const DEFAULT_THERMAL_CUTOFF_EV: f64 = 4.0;

/// Number of incident-energy points on the pre-tabulated σ_inel(E) grid.
const N_XS_GRID: usize = 200;
/// Number of incident-energy points on the pre-tabulated emission grid.
const N_EMIT_GRID: usize = 48;
/// Equally-probable outgoing energies per emission table (NJOY-typical).
const N_OUTGOING: usize = 16;
/// Equally-probable cosines per outgoing-energy bin (NJOY-typical).
const N_COSINES: usize = 8;
/// Bottom of the pre-tabulation grid \[eV\] — the thermal tail of a Maxwellian.
const E_MIN_GRID_EV: f64 = 1.0e-5;

/// One pre-tabulated equiprobable emission table for a single incident energy —
/// the in-memory analogue of an ACE ITXE sub-block (IFENG=0).
///
/// A scatter is sampled by choosing one of the `n_out` outgoing energies
/// uniformly (each probability `1/n_out`), then one of that bin's `n_mu` cosines
/// uniformly. Both outgoing energy and cosine are **laboratory-frame** (S(α,β)
/// secondary distributions are given in the lab frame, unlike the CM elastic
/// law), so the caller applies the cosine directly with `rotate_direction` — no
/// CM→lab transform.
#[derive(Debug, Clone)]
struct EmissionTable {
    /// Representative outgoing energies \[eV\], one per equiprobable bin.
    e_out: Vec<f64>,
    /// Equally-probable cosines μ ∈ \[−1, 1\] for each outgoing-energy bin,
    /// laid out row-major (`bin * n_mu + j`).
    cosines: Vec<f64>,
    /// Number of cosines per outgoing-energy bin.
    n_mu: usize,
}

/// Pre-tabulated incoherent-inelastic S(α,β) thermal scattering for one bound
/// scatterer (H in H₂O) at one temperature — the transport-side data surface.
///
/// Built once per material+temperature with [`from_endf_file`](Self::from_endf_file),
/// then queried in the transport loop by:
/// - [`inelastic_xs`](Self::inelastic_xs) — σ_inel(E) **per principal atom**
///   \[barn\] at incident energy `E` \[eV\] (multiply by the principal-atom number
///   density to get the macroscopic contribution);
/// - [`sample`](Self::sample) — a laboratory-frame outgoing energy \[eV\] and
///   cosine for a thermal scatter.
///
/// Cross sections are **per principal atom** (per H for H-in-H₂O). The material
/// composition (two H per H₂O) is the caller's number-density bookkeeping.
pub struct ThermalScattering {
    /// Human-readable scatterer name, e.g. `"H in H2O"`.
    pub name: String,
    /// Upper energy of the S(α,β) treatment \[eV\] (the thermal cutoff).
    cutoff_ev: f64,
    /// Selected tabulated temperature \[K\] actually used (nearest grid point to
    /// the request), recorded for the V&V provenance.
    selected_temperature_k: f64,
    /// Ascending incident-energy grid \[eV\] for the σ_inel lookup.
    xs_e: Vec<f64>,
    /// σ_inel per principal atom \[barn\] at each `xs_e` point.
    xs_sigma: Vec<f64>,
    /// Ascending incident-energy grid \[eV\] for the emission tables.
    emit_e: Vec<f64>,
    /// One emission table per `emit_e` point (parallel arrays).
    emit_tables: Vec<EmissionTable>,
}

impl ThermalScattering {
    /// Build the pre-tabulated H-in-H₂O S(α,β) treatment from an ENDF `tsl-*`
    /// thermal evaluation file.
    ///
    /// # Parameters
    /// - `path` — the ENDF `tsl-*` file (e.g. ENDF/B-VIII.0 `tsl-HinH2O.endf`).
    /// - `mat` — the ENDF material number of the thermal evaluation (`1` for the
    ///   ENDF/B-VIII.0 `tsl-HinH2O` file).
    /// - `temperature_k` — the requested temperature \[K\]; the nearest tabulated
    ///   S(α,β) temperature is selected (read it back with
    ///   [`selected_temperature_k`](Self::selected_temperature_k)).
    /// - `name` — a label carried for diagnostics (e.g. `"H in H2O"`).
    ///
    /// The σ_inel(E) and secondary-distribution grids are baked here (this is the
    /// expensive step: it integrates the S(α,β) kernel at `N_XS_GRID` +
    /// `N_EMIT_GRID` incident energies), so the transport loop stays cheap.
    ///
    /// # Errors
    /// Propagates [`njoy_outram_park_fork::NjoyError`] if the file cannot be read,
    /// `mat` has no MF=7/MT=4 incoherent-inelastic data, or a record is malformed.
    pub fn from_endf_file(
        path: &str,
        mat: i32,
        temperature_k: f64,
        name: &str,
    ) -> Result<Self, njoy_outram_park_fork::NjoyError> {
        use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
        use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
        use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

        let t = Temperature::new::<kelvin>(temperature_k);
        let sab = IncoherentInelasticScattering::from_endf_file(path, mat, t)?;
        let selected_temperature_k = sab.selected_temperature().get::<kelvin>();
        let cutoff_ev = DEFAULT_THERMAL_CUTOFF_EV;

        // σ_inel(E) grid — log-spaced from the thermal tail to the cutoff.
        let xs_e = log_grid(E_MIN_GRID_EV, cutoff_ev, N_XS_GRID);
        let xs_sigma: Vec<f64> = xs_e
            .iter()
            .map(|&e| sab.inelastic_xs(NeutronEnergy::new::<electronvolt>(e)).get::<barn>())
            .collect();

        // Emission grid — a coarser log-spaced incident-energy grid, each point
        // carrying an equiprobable (E', μ) table.
        let emit_e = log_grid(E_MIN_GRID_EV, cutoff_ev, N_EMIT_GRID);
        let emit_tables: Vec<EmissionTable> = emit_e
            .iter()
            .map(|&e| {
                let bins =
                    sab.emission(NeutronEnergy::new::<electronvolt>(e), N_OUTGOING, N_COSINES);
                build_emission_table(&bins)
            })
            .collect();

        Ok(Self {
            name: name.to_string(),
            cutoff_ev,
            selected_temperature_k,
            xs_e,
            xs_sigma,
            emit_e,
            emit_tables,
        })
    }

    /// Upper energy \[eV\] of the S(α,β) treatment (the thermal cutoff). Above it
    /// the caller uses ordinary free-gas / WMP elastic scattering.
    pub fn cutoff_ev(&self) -> f64 {
        self.cutoff_ev
    }

    /// The tabulated temperature \[K\] actually used (nearest grid point to the
    /// request) — recorded so a V&V reader knows the exact data point.
    pub fn selected_temperature_k(&self) -> f64 {
        self.selected_temperature_k
    }

    /// Incoherent-inelastic cross section σ_inel(E) **per principal atom**
    /// \[barn\] at incident energy `e` \[eV\], by linear interpolation on the
    /// pre-tabulated grid. Zero at or above the cutoff (the caller reverts to
    /// free-gas there); clamped to the grid endpoints below it.
    pub fn inelastic_xs(&self, e: f64) -> f64 {
        if e >= self.cutoff_ev {
            return 0.0;
        }
        interp_linear(&self.xs_e, &self.xs_sigma, e)
    }

    /// Sample a thermal scatter at incident energy `e` \[eV\], returning
    /// `Some((e_out, mu_lab))` — a laboratory-frame outgoing energy \[eV\] and
    /// scattering cosine — or `None` at or above the cutoff.
    ///
    /// Mirrors the ACE incoherent-inelastic sampling (`src/thermal.cpp`
    /// `ThermalData::sample`, IFENG=0): bracket the incident energy on the
    /// emission grid, pick the lower/upper table by statistical interpolation,
    /// then draw one equiprobable outgoing energy and one equiprobable cosine.
    pub fn sample(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)> {
        if e >= self.cutoff_ev || self.emit_tables.is_empty() {
            return None;
        }
        let table = self.select_table(e, seed);
        if table.e_out.is_empty() {
            return None;
        }
        // One equiprobable outgoing-energy bin, then one equiprobable cosine.
        let n_out = table.e_out.len();
        let i_out = ((prn(seed) * n_out as f64) as usize).min(n_out - 1);
        let e_out = table.e_out[i_out];
        let base = i_out * table.n_mu;
        let mu = if table.n_mu == 0 {
            2.0 * prn(seed) - 1.0
        } else {
            let j = ((prn(seed) * table.n_mu as f64) as usize).min(table.n_mu - 1);
            table.cosines[base + j].clamp(-1.0, 1.0)
        };
        Some((e_out, mu))
    }

    /// Pick the emission table to sample from for incident energy `e` \[eV\] by
    /// statistical interpolation between the two bracketing incident-energy grid
    /// points (ACE convention: choose the upper table with probability equal to
    /// the interpolation factor `r`).
    fn select_table(&self, e: f64, seed: &mut u64) -> &EmissionTable {
        let grid = &self.emit_e;
        let n = grid.len();
        if e <= grid[0] {
            return &self.emit_tables[0];
        }
        if e >= grid[n - 1] {
            return &self.emit_tables[n - 1];
        }
        let mut i = 0;
        while i + 1 < n && grid[i + 1] <= e {
            i += 1;
        }
        let (e0, e1) = (grid[i], grid[i + 1]);
        let r = if e1 > e0 { (e - e0) / (e1 - e0) } else { 0.0 };
        if r > prn(seed) {
            &self.emit_tables[i + 1]
        } else {
            &self.emit_tables[i]
        }
    }
}

/// Flatten the njoy equiprobable emission bins into a cache-friendly
/// [`EmissionTable`]. An empty input (σ = 0 at this energy) yields an empty table.
fn build_emission_table(
    bins: &[njoy_outram_park_fork::thermr::scattering::ThermalEmissionBin],
) -> EmissionTable {
    use uom::si::energy::electronvolt;
    if bins.is_empty() {
        return EmissionTable { e_out: Vec::new(), cosines: Vec::new(), n_mu: 0 };
    }
    let n_mu = bins.iter().map(|b| b.cosines.len()).min().unwrap_or(0);
    let mut e_out = Vec::with_capacity(bins.len());
    let mut cosines = Vec::with_capacity(bins.len() * n_mu);
    for b in bins {
        e_out.push(b.outgoing_energy.get::<electronvolt>());
        for j in 0..n_mu {
            cosines.push(b.cosines[j]);
        }
    }
    EmissionTable { e_out, cosines, n_mu }
}

/// A logarithmically spaced grid of `n` points on `[lo, hi]` \[eV\] (both
/// endpoints included). Used for both the σ and emission incident-energy grids.
fn log_grid(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if n <= 1 {
        return vec![lo];
    }
    let (llo, lhi) = (lo.ln(), hi.ln());
    (0..n)
        .map(|k| (llo + (lhi - llo) * k as f64 / (n as f64 - 1.0)).exp())
        .collect()
}

/// Linear interpolation of `y(x)` on an ascending grid, clamped to the endpoints
/// outside the grid. `xs` and `ys` are parallel and non-empty.
fn interp_linear(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut i = 0;
    while i + 1 < n && xs[i + 1] < x {
        i += 1;
    }
    let (x0, x1) = (xs[i], xs[i + 1]);
    let (y0, y1) = (ys[i], ys[i + 1]);
    if x1 > x0 {
        y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    } else {
        y0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_grid_spans_endpoints_ascending() {
        let g = log_grid(1.0e-5, 4.0, 50);
        assert_eq!(g.len(), 50);
        assert!((g[0] - 1.0e-5).abs() < 1.0e-12);
        assert!((g[49] - 4.0).abs() < 1.0e-9);
        for w in g.windows(2) {
            assert!(w[1] > w[0], "grid must be strictly ascending");
        }
    }

    #[test]
    fn interp_linear_clamps_and_interpolates() {
        let xs = vec![0.0, 1.0, 2.0];
        let ys = vec![10.0, 20.0, 40.0];
        assert_eq!(interp_linear(&xs, &ys, -1.0), 10.0); // clamp low
        assert_eq!(interp_linear(&xs, &ys, 3.0), 40.0); // clamp high
        assert!((interp_linear(&xs, &ys, 0.5) - 15.0).abs() < 1.0e-12);
        assert!((interp_linear(&xs, &ys, 1.5) - 30.0).abs() < 1.0e-12);
    }
}
