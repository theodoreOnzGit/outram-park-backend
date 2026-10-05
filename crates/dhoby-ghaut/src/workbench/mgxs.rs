//! # Step 8: multigroup cross sections per region, at state points
//!
//! Monte Carlo is run at a few **state points** (temperatures); at each, the
//! flux and reaction rates are tallied on the Step 7 neutronics mesh, and
//! condensed into one set of macroscopic multigroup cross sections per
//! **region** (the regions of [`super::meshes::RegionMap`]). Between state
//! points the solver interpolates, by default **linearly in `ln T`**
//! ([`InterpLaw::LnT`]); a more general parametrisation is deferred
//! (gh:#573).
//!
//! This module is the **solver-independent half** (plan, results, hand-off).
//! The Monte Carlo runs are in the `dhoby-ghaut` binary
//! (`src/bin/dhoby-ghaut/mgxs_run.rs`), which calls `outram-mc-libs` and
//! condenses with `nee_soon::mgxs` (no physics is re-implemented here).
//!
//! ## Hand-off to Steps 9 and 10 (the multiphysics agent reads THIS)
//!
//! [`MgxsSet`] is what Step 8 produces:
//!
//! - `nuclear_data_path`: a GeN-Foam `constant/neutroRegion/nuclearData`
//!   dictionary (MKSA, per metre), one `zones` entry per region, named by
//!   [`super::meshes::Region::id`] = the neutronics polyMesh's cellZone
//!   names, one `states` entry per state point, `xsVariables { TFuel log; }`.
//!   Read it with `outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data`
//!   and build the solver's data with `CrossSectionData::from_input`; then
//!   `zone_of_cell` comes from `read_cell_zones` on the neutronics mesh.
//!   `precGroups 0`: no delayed-neutron data yet (gh:#595), so a transient
//!   cannot be run on it.
//! - the same numbers in memory, in **cm⁻¹**, group 0 the **fastest**
//!   (GeN-Foam order), with relative standard deviations: [`MgxsSet::states`].
//!   [`MgxsSet::interpolate`] evaluates a region at any temperature with the
//!   chosen law.
//!
//! **What a state point is.** Each state point processes the nuclear data at
//! one temperature and puts **every** material at it (the Step 5 data path,
//! `nee_soon::htr10_rmc::data`). The temperature dependence is therefore of
//! an isothermal core; the file attributes it to `TFuel`, the variable
//! GeN-Foam's Doppler feedback reads. Separating fuel, moderator and coolant
//! temperatures needs per-material data temperatures (gh:#595).

use serde::{Deserialize, Serialize};

/// How cross sections are interpolated between state points (GeN-Foam's
/// `xsVariables` laws).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpLaw {
    /// Linear in `ln T` (GeN-Foam `log`): the default, for Doppler.
    LnT,
    /// Linear in `sqrt T` (GeN-Foam `sqrt`).
    SqrtT,
    /// Linear in `T` (GeN-Foam `lin`).
    Linear,
}

impl InterpLaw {
    /// The coordinate interpolation is linear in.
    #[must_use]
    pub fn transform(self, t_k: f64) -> f64 {
        match self {
            Self::LnT => t_k.ln(),
            Self::SqrtT => t_k.sqrt(),
            Self::Linear => t_k,
        }
    }

    /// GeN-Foam keyword.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::LnT => "log",
            Self::SqrtT => "sqrt",
            Self::Linear => "lin",
        }
    }

    /// Human label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::LnT => "linear in ln T (default)",
            Self::SqrtT => "linear in sqrt T",
            Self::Linear => "linear in T",
        }
    }
}

/// Energy group structures offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupPreset {
    /// Two groups, split at 0.625 eV.
    Two,
    /// Four groups: 0.625 eV, 9.118 keV, 0.821 MeV (a common LWR/HTGR split,
    /// a subset of the WIMS 69-group edges).
    Four,
    /// The 8-group WIMS-derived structure `nee_soon`'s HTR-10 deterministic
    /// comparison uses.
    Eight,
}

impl GroupPreset {
    /// Ascending edges \[eV\].
    #[must_use]
    pub fn edges_ev(self) -> Vec<f64> {
        match self {
            Self::Two => vec![1.0e-5, 0.625, 2.0e7],
            Self::Four => vec![1.0e-5, 0.625, 9.118e3, 8.21e5, 2.0e7],
            Self::Eight => vec![1.0e-5, 0.14, 0.625, 4.0, 29.0, 130.0, 9.12e3, 1.35e6, 2.0e7],
        }
    }

    /// Human label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Two => "2 groups (0.625 eV)",
            Self::Four => "4 groups",
            Self::Eight => "8 groups (WIMS-derived)",
        }
    }
}

/// Step 8's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MgxsPlan {
    /// Group structure.
    pub groups: GroupPreset,
    /// State-point temperatures \[K\]; the first is the GeN-Foam `reference`.
    pub temperatures_k: Vec<f64>,
    /// Interpolation law.
    pub law: InterpLaw,
    /// Histories per generation.
    pub particles: usize,
    /// Inactive generations.
    pub inactive: usize,
    /// Active generations.
    pub active: usize,
    /// Seed (both tally passes of a state use it, so their histories match).
    pub seed: u64,
    /// Threads.
    pub threads: usize,
}

impl Default for MgxsPlan {
    /// Short: a few minutes per state point on a desktop, dominated by the
    /// nuclear data. 300.15 K is the HTR-10 benchmark temperature; 600 K a
    /// second point for the `ln T` slope. Production runs need far more
    /// histories (see the σ the results show).
    fn default() -> Self {
        Self {
            groups: GroupPreset::Two,
            temperatures_k: vec![300.15, 600.0],
            law: InterpLaw::LnT,
            particles: 2000,
            inactive: 10,
            active: 20,
            seed: 1,
            threads: 8,
        }
    }
}

/// One region's constants at one state, **cm⁻¹**, group 0 the fastest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionXs {
    /// Region id.
    pub region: String,
    /// Region volume on the neutronics mesh \[cm³\].
    pub volume_cm3: f64,
    /// Volume-integrated flux per group (per source neutron, track-length).
    pub flux: Vec<f64>,
    /// Relative σ of `flux`.
    pub flux_rel_sigma: Vec<f64>,
    /// Σ_t.
    pub total: Vec<f64>,
    /// Σ_a.
    pub absorption: Vec<f64>,
    /// νΣ_f.
    pub nu_fission: Vec<f64>,
    /// κΣ_f \[eV/cm\].
    pub kappa_fission: Vec<f64>,
    /// χ (measured fission spectrum; zero in a region with no fission).
    pub chi: Vec<f64>,
    /// P0 scattering `[from][to]`.
    pub scatter: Vec<Vec<f64>>,
    /// Relative σ of Σ_t, Σ_a and νΣ_f (first-order ratio estimate, cells
    /// of the region treated as independent; see the binary's docs).
    pub rel_sigma_total: Vec<f64>,
    /// Relative σ of Σ_a.
    pub rel_sigma_absorption: Vec<f64>,
    /// Relative σ of νΣ_f.
    pub rel_sigma_nu_fission: Vec<f64>,
}

impl RegionXs {
    /// D = 1/(3 Σ_t), no transport correction (as `nee_soon::mgxs`).
    #[must_use]
    pub fn diffusion(&self, g: usize) -> f64 {
        1.0 / (3.0 * self.total[g])
    }

    /// Σ_r = Σ_t − Σ_s,g→g.
    #[must_use]
    pub fn removal(&self, g: usize) -> f64 {
        self.total[g] - self.scatter[g][g]
    }
}

/// One state point's results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateXs {
    /// Temperature \[K\] (every material).
    pub temperature_k: f64,
    /// k_eff of the run and its σ.
    pub k: f64,
    /// σ of k.
    pub k_sigma: f64,
    /// Histories transported (per pass).
    pub histories: u64,
    /// Seconds: nuclear data, transport (both passes).
    pub data_s: f64,
    /// Transport seconds.
    pub transport_s: f64,
    /// Per region, in [`super::meshes::RegionMap`] order (regions the mesh
    /// does not hold are absent).
    pub regions: Vec<RegionXs>,
}

/// Everything Step 8 produced: the hand-off to Steps 9-10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MgxsSet {
    /// Group edges \[eV\], **descending** (group 0 fastest).
    pub edges_ev_desc: Vec<f64>,
    /// Interpolation law.
    pub law: InterpLaw,
    /// State points, ascending temperature.
    pub states: Vec<StateXs>,
    /// The written GeN-Foam `nuclearData`, if written.
    pub nuclear_data_path: Option<String>,
    /// Modelling notes (data layout, ablations, simplifications).
    pub notes: Vec<String>,
}

impl MgxsSet {
    /// Number of groups.
    #[must_use]
    pub fn n_groups(&self) -> usize {
        self.edges_ev_desc.len().saturating_sub(1)
    }

    /// Region `id` at state `s`.
    #[must_use]
    pub fn region(&self, s: usize, id: &str) -> Option<&RegionXs> {
        self.states.get(s)?.regions.iter().find(|r| r.region == id)
    }

    /// Region `id` at `t_k`: each constant interpolated linearly in the law's
    /// coordinate between the bracketing state points. Inside the state range
    /// this equals GeN-Foam's `polyharmonicSplineMode 1` (`phi = |r|`) in one
    /// variable, which is the piecewise-linear interpolant; **outside it this
    /// holds the nearest state's values** (GeN-Foam extrapolates linearly),
    /// and says so in the second return value.
    #[must_use]
    pub fn interpolate(&self, id: &str, t_k: f64) -> Option<(RegionXs, bool)> {
        let n = self.states.len();
        if n == 0 {
            return None;
        }
        let x = self.law.transform(t_k);
        let xs: Vec<f64> = self
            .states
            .iter()
            .map(|s| self.law.transform(s.temperature_k))
            .collect();
        if n == 1 || x <= xs[0] {
            return Some((self.region(0, id)?.clone(), n > 1 && x < xs[0]));
        }
        if x >= xs[n - 1] {
            return Some((self.region(n - 1, id)?.clone(), x > xs[n - 1]));
        }
        let i = (0..n - 1).find(|&i| x <= xs[i + 1]).unwrap_or(n - 2);
        let w = (x - xs[i]) / (xs[i + 1] - xs[i]);
        let (a, b) = (self.region(i, id)?, self.region(i + 1, id)?);
        let mix = |p: &[f64], q: &[f64]| {
            p.iter()
                .zip(q)
                .map(|(u, v)| u + w * (v - u))
                .collect::<Vec<f64>>()
        };
        Some((
            RegionXs {
                region: a.region.clone(),
                volume_cm3: a.volume_cm3,
                flux: mix(&a.flux, &b.flux),
                flux_rel_sigma: mix(&a.flux_rel_sigma, &b.flux_rel_sigma),
                total: mix(&a.total, &b.total),
                absorption: mix(&a.absorption, &b.absorption),
                nu_fission: mix(&a.nu_fission, &b.nu_fission),
                kappa_fission: mix(&a.kappa_fission, &b.kappa_fission),
                chi: mix(&a.chi, &b.chi),
                scatter: a
                    .scatter
                    .iter()
                    .zip(&b.scatter)
                    .map(|(p, q)| mix(p, q))
                    .collect(),
                rel_sigma_total: mix(&a.rel_sigma_total, &b.rel_sigma_total),
                rel_sigma_absorption: mix(&a.rel_sigma_absorption, &b.rel_sigma_absorption),
                rel_sigma_nu_fission: mix(&a.rel_sigma_nu_fission, &b.rel_sigma_nu_fission),
            },
            false,
        ))
    }
}

/// Gaps of Step 8, shown as NOT available in the UI.
#[must_use]
pub fn not_available() -> Vec<super::meshes::NotAvailable> {
    vec![
        super::meshes::NotAvailable {
            what: "Delayed-neutron data (Beta, lambda) per region",
            why: "the Monte Carlo tallies give prompt+delayed nu only; the file is written with precGroups 0, enough for a steady eigenvalue, not a transient",
            issue: 595,
        },
        super::meshes::NotAvailable {
            what: "Separate fuel / moderator / coolant temperatures (and density) as state variables",
            why: "a state point processes the nuclear data at one temperature for every material; the more general parametrisation is deferred",
            issue: 595,
        },
        super::meshes::NotAvailable {
            what: "P1 scattering, transport-corrected D, discontinuity factors",
            why: "nee_soon::mgxs condenses P0 only, D = 1/(3 Σ_t), discFactor 1",
            issue: 595,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rx(v: f64) -> RegionXs {
        RegionXs {
            region: "bed".into(),
            volume_cm3: 1.0,
            flux: vec![1.0],
            flux_rel_sigma: vec![0.0],
            total: vec![v],
            absorption: vec![v],
            nu_fission: vec![v],
            kappa_fission: vec![v],
            chi: vec![1.0],
            scatter: vec![vec![v]],
            rel_sigma_total: vec![0.0],
            rel_sigma_absorption: vec![0.0],
            rel_sigma_nu_fission: vec![0.0],
        }
    }

    fn st(t: f64, v: f64) -> StateXs {
        StateXs {
            temperature_k: t,
            k: 1.0,
            k_sigma: 0.0,
            histories: 0,
            data_s: 0.0,
            transport_s: 0.0,
            regions: vec![rx(v)],
        }
    }

    /// Linear in ln T: at the geometric mean of two state temperatures the
    /// value is the arithmetic mean (the same check as the GeN-Foam port's
    /// `doppler_log_interpolation_hits_arithmetic_mean_at_geometric_mean_temperature`).
    #[test]
    fn ln_t_interpolation_hits_the_mean_at_the_geometric_mean() {
        let set = MgxsSet {
            edges_ev_desc: vec![2e7, 1e-5],
            law: InterpLaw::LnT,
            states: vec![st(300.0, 1.0), st(1200.0, 3.0)],
            nuclear_data_path: None,
            notes: vec![],
        };
        let (x, extrapolated) = set.interpolate("bed", 600.0).unwrap();
        assert!(!extrapolated);
        assert!((x.total[0] - 2.0).abs() < 1e-12);
        let (y, e2) = set.interpolate("bed", 2000.0).unwrap();
        assert!(e2 && (y.total[0] - 3.0).abs() < 1e-12);
    }
}
