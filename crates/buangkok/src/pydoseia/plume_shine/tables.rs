// SPDX-License-Identifier: GPL-3.0-only
//! Photon tables for plume shine: gamma lines per nuclide and the air
//! attenuation coefficients. Ported from pyDOSEIA `raddcffunc.py`
//! (`gamma_energy_abundaces`, `add_zero_energy_for_pure_beta`, `atten_coeff`,
//! `get_k_mu_mua_MFP`), commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`,
//! Copyright (c) 2024 Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).
//! No table data ships here; see the parent module for why.

use crate::pydoseia::csv::{col, num, split_csv};

/// Air density upstream multiplies the mass coefficients by, g/cm^3.
pub const AIR_DENSITY_G_PER_CM3: f64 = 1.225e-3;

/// One gamma line: energy (MeV) and emission probability per decay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GammaLine {
    /// Photon energy, MeV (upstream divides the table's keV by 1000).
    pub energy_mev: f64,
    /// Emission probability per decay.
    pub emission_probability: f64,
}

/// One row of upstream's `gamma_energy_radionuclide` sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct GammaLineRow {
    /// Nuclide name as written.
    pub nuclide: String,
    /// Energy, keV.
    pub energy_kev: f64,
    /// Emission probability.
    pub emission_probability: f64,
}

/// Upstream's gamma-line table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GammaLineTable {
    /// Rows in file order.
    pub rows: Vec<GammaLineRow>,
}

/// The lines [`GammaLineTable::lines_for`] found for one nuclide.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NuclideGammaLines {
    /// Lines kept (energy >= 0.05 MeV and probability >= 1e-3), in file order.
    pub kept: Vec<GammaLine>,
    /// Lines dropped by that cut, in file order. (Upstream records only the
    /// last one per nuclide, in a log.)
    pub neglected: Vec<GammaLine>,
    /// Whether any row matched the nuclide.
    pub found: bool,
}

impl GammaLineTable {
    /// Read a CSV in upstream's column layout: `nuclide, energy_kev,
    /// std_energy_kev, emmission_prob, std_emmission_prob` (upstream's
    /// spelling; the `std_*` columns are not used).
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let n = col(&h, "nuclide")?;
        let e = col(&h, "energy_kev")?;
        let p = col(&h, "emmission_prob")?;
        Ok(Self {
            rows: rows
                .into_iter()
                .map(|r| GammaLineRow {
                    nuclide: r.get(n).cloned().unwrap_or_default(),
                    energy_kev: num(r.get(e)),
                    emission_probability: num(r.get(p)),
                })
                .collect(),
        })
    }

    /// Upstream's `gamma_energy_abundaces` for one nuclide.
    ///
    /// **Rows are matched by substring** (`str.contains`), so `Co-6` would
    /// also pick up `Co-60` rows (defect D24, the pattern of D4). A line is
    /// neglected when `E/1000 < 0.05` (i.e. below **50 keV**; upstream's
    /// comment says 5 keV: defect D17) or its probability is below `1e-3`.
    #[must_use]
    pub fn lines_for(&self, nuclide: &str) -> NuclideGammaLines {
        let mut out = NuclideGammaLines::default();
        for r in self.rows.iter().filter(|r| r.nuclide.contains(nuclide)) {
            out.found = true;
            let line = GammaLine {
                energy_mev: r.energy_kev / 1000.0,
                emission_probability: r.emission_probability,
            };
            if r.energy_kev / 1000.0 < 0.05 || r.emission_probability < 0.001 {
                out.neglected.push(line);
            } else {
                out.kept.push(line);
            }
        }
        out
    }

    /// The lines plume shine integrates for one nuclide: [`Self::lines_for`]'s
    /// kept lines, or a single zero-energy, zero-yield placeholder when there
    /// are none (upstream's `[0]` for a nuclide not in the table, and
    /// `add_zero_energy_for_pure_beta` for one whose lines were all cut).
    #[must_use]
    pub fn plume_shine_lines(&self, nuclide: &str) -> Vec<GammaLine> {
        let kept = self.lines_for(nuclide).kept;
        if kept.is_empty() {
            vec![GammaLine {
                energy_mev: 0.0,
                emission_probability: 0.0,
            }]
        } else {
            kept
        }
    }
}

/// `numpy.interp(x, xp, fp)` for one point, with numpy's edge handling:
/// `fp[0]` left of the table, `fp[last]` right of it (and at `xp[last]`),
/// `fp[j]` at an exact knot, and numpy's slope formula
/// `slope * (x - xp[j]) + fp[j]` elsewhere. `xp` must be increasing.
#[must_use]
pub fn numpy_interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    let n = xp.len();
    if n == 0 {
        return f64::NAN;
    }
    if x.is_nan() {
        return x;
    }
    if n == 1 {
        return fp[0];
    }
    if x > xp[n - 1] {
        return fp[n - 1];
    }
    if x < xp[0] {
        return fp[0];
    }
    // dx[j] <= x < dx[j+1], or j = n-1 at the last knot.
    let j = xp.partition_point(|v| *v <= x) - 1;
    if j == n - 1 || xp[j] == x {
        return fp[j];
    }
    let slope = (fp[j + 1] - fp[j]) / (xp[j + 1] - xp[j]);
    let mut r = slope * (x - xp[j]) + fp[j];
    if r.is_nan() {
        r = slope * (x - xp[j + 1]) + fp[j + 1];
        if r.is_nan() && fp[j] == fp[j + 1] {
            r = fp[j];
        }
    }
    r
}

/// Upstream's `mass_attenuation_coeff` sheet: energy (MeV), mass attenuation
/// coefficient `mu/rho` and mass energy-absorption coefficient `mu_en/rho`
/// (cm^2/g), for air.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttenuationTable {
    /// Energies, MeV, increasing.
    pub energy_mev: Vec<f64>,
    /// `mu/rho`, cm^2/g.
    pub total_cm2_per_g: Vec<f64>,
    /// `mu_en/rho`, cm^2/g.
    pub energy_absorption_cm2_per_g: Vec<f64>,
}

/// The coefficients upstream derives for one photon energy
/// (`get_k_mu_mua_MFP`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirPhotonCoefficients {
    /// Build-up coefficient `k = (mu - mu_a) / mu_a`.
    pub k: f64,
    /// Linear attenuation coefficient `mu`, 1/m.
    pub mu_per_m: f64,
    /// Linear energy-absorption coefficient `mu_a`, 1/m.
    pub mu_a_per_m: f64,
    /// Mean free path `1 / mu`, m.
    pub mfp_m: f64,
}

impl AttenuationTable {
    /// Read a CSV with columns `energy, total_atten_coeff, energy_atten_coeff`
    /// (upstream's column names).
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let e = col(&h, "energy")?;
        let t = col(&h, "total_atten_coeff")?;
        let a = col(&h, "energy_atten_coeff")?;
        let mut out = Self::default();
        for r in rows {
            out.energy_mev.push(num(r.get(e)));
            out.total_cm2_per_g.push(num(r.get(t)));
            out.energy_absorption_cm2_per_g.push(num(r.get(a)));
        }
        Ok(out)
    }

    /// Upstream's `atten_coeff` + `get_k_mu_mua_MFP` for one energy (MeV):
    /// interpolate linearly (`numpy.interp`) and multiply by
    /// [`AIR_DENSITY_G_PER_CM3`] and 100 (cm^-1 to m^-1), then
    /// `k = (mu - mu_a) / mu_a` and `MFP = 1 / mu`.
    #[must_use]
    pub fn air_coefficients(&self, energy_mev: f64) -> AirPhotonCoefficients {
        let mu = numpy_interp(energy_mev, &self.energy_mev, &self.total_cm2_per_g)
            * AIR_DENSITY_G_PER_CM3
            * 100.0;
        let mu_a = numpy_interp(
            energy_mev,
            &self.energy_mev,
            &self.energy_absorption_cm2_per_g,
        ) * AIR_DENSITY_G_PER_CM3
            * 100.0;
        AirPhotonCoefficients {
            k: (mu - mu_a) / mu_a,
            mu_per_m: mu,
            mu_a_per_m: mu_a,
            mfp_m: 1.0 / mu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interp_edges_follow_numpy() {
        let xp = [1.0, 2.0, 4.0];
        let fp = [10.0, 20.0, 0.0];
        assert_eq!(numpy_interp(0.5, &xp, &fp), 10.0);
        assert_eq!(numpy_interp(4.0, &xp, &fp), 0.0);
        assert_eq!(numpy_interp(5.0, &xp, &fp), 0.0);
        assert_eq!(numpy_interp(2.0, &xp, &fp), 20.0);
        assert_eq!(numpy_interp(1.5, &xp, &fp), 15.0);
        assert_eq!(numpy_interp(3.0, &xp, &fp), 10.0);
    }

    #[test]
    fn missing_nuclide_gets_the_placeholder_line() {
        let t = GammaLineTable::from_csv(
            "nuclide,energy_kev,std_energy_kev,emmission_prob,std_emmission_prob\nX-1,40,0,0.5,0\n",
        )
        .unwrap();
        // the only line is below 50 keV, so the nuclide falls back to the placeholder
        assert_eq!(t.lines_for("X-1").neglected.len(), 1);
        assert_eq!(t.plume_shine_lines("X-1")[0].energy_mev, 0.0);
        assert_eq!(t.plume_shine_lines("Y-2")[0].emission_probability, 0.0);
    }
}
