// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/getrb.f90, src/getrc.f90, src/partdep.f90,
//                    src/getvdep.f90, src/get_settling.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Dry deposition velocity: the resistance model for gases (Wesely 1989), the
//! particle scheme (Slinn 1982), their composition per grid cell, and the
//! Reynolds-dependent settling velocity.
//!
//! | Function | Upstream | Returns |
//! |---|---|---|
//! | [`getrb`] | `getrb.f90` | quasi-laminar sublayer resistance `r_b`, s/m |
//! | [`getrc`] | `getrc.f90` | bulk surface resistance `r_c`, s/m |
//! | [`partdep`] | `partdep.f90` | particle deposition velocity, m/s |
//! | [`getvdep`] | `getvdep.f90` | deposition velocity for a grid cell, m/s |
//! | [`get_settling`] | `get_settling.f90` | settling velocity with drag iteration, m/s |
//!
//! # Inputs that upstream reads from `com_mod`
//!
//! Upstream reads the Wesely resistance tables, landuse fractions, roughness
//! lengths and species properties from global module arrays filled by
//! `readlanduse.f90`, `readdepo.f90` and `readreleases.f90`. The port takes
//! them as arguments: [`SurfaceResistances`] per season and class,
//! [`GasSpecies`] and [`DepositionSpecies`] per species. **The tables are data,
//! not physics**; nothing here ships FLEXPART's `surfdepo.t` values.
//!
//! # Units
//!
//! FLEXPART's, as bare `f64`: resistances s/m, velocities m/s, temperatures K
//! except where a doc says Celsius, pressure Pa, kinematic viscosity m²/s.

use super::aerosol::AerosolBins;
use super::calendar::caldate;
use super::constants::{GA, KARMAN, NI, NUMCLASS};
use super::surface_layer::raerod;
use super::thermo::viscosity_kelvin;

/// Number of Wesely seasons (`1` midsummer .. `5` transitional spring).
pub const NSEASON: usize = 5;

/// Prandtl number of air used by `getrb.f90`.
const PR: f64 = 0.72;

/// `getrb.f90`: quasi-laminar sublayer resistance for one gas.
///
/// `r_b = 2 (Sc / Pr)^0.67 / (kappa u*)`, with `Sc = (nu / D_H2O) * reldiff`.
///
/// # Arguments
/// - `ustar` — friction velocity, m/s.
/// - `nyl` — kinematic viscosity of air, m²/s.
/// - `diffh2o` — molecular diffusivity of water vapour, m²/s.
/// - `reldiff` — diffusivity of water relative to the species.
///
/// # Returns
/// `None` when `reldiff <= 0`: upstream then leaves `rb` unassigned, which is
/// how it marks a species that is not a gas.
#[must_use]
pub fn getrb(ustar: f64, nyl: f64, diffh2o: f64, reldiff: f64) -> Option<f64> {
    if reldiff > 0.0 {
        let schmidt = nyl / diffh2o * reldiff;
        Some(2.0 * (schmidt / PR).powf(0.67) / (KARMAN * ustar))
    } else {
        None
    }
}

/// Wesely (1989) resistances for one season, one landuse class and one
/// species, s/m. Upstream's `ri(season,class)`, `rac(season,class)` and the
/// species-specific `rcl`, `rgs`, `rlu(species,season,class)`.
///
/// `9999` and `1e25` are upstream's "no such pathway" markers and are used as
/// they are, as very large resistances.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SurfaceResistances {
    /// Minimum bulk canopy stomatal resistance for water vapour.
    pub ri: f64,
    /// In-canopy aerodynamic resistance.
    pub rac: f64,
    /// Resistance of the lower canopy.
    pub rcl: f64,
    /// Ground surface resistance.
    pub rgs: f64,
    /// Leaf cuticle resistance.
    pub rlu: f64,
}

/// Gas-phase deposition properties of one species (from `SPECIES_nnn`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasSpecies {
    /// Diffusivity of water vapour relative to the species, dimensionless.
    /// A gas when `> 0`.
    pub reldiff: f64,
    /// Effective Henry's constant, M/atm.
    pub henry: f64,
    /// Reactivity relative to ozone, dimensionless.
    pub f0: f64,
    /// Mesophyll resistance, s/m.
    pub rm: f64,
}

/// `getrc.f90`: Wesely (1989) bulk surface resistance for one gas.
///
/// # Arguments
/// - `cell` — resistances for the current season and landuse class.
/// - `species` — the gas.
/// - `t_celsius` — 2 m temperature, **°C**.
/// - `gr` — global radiation, W/m².
/// - `rh` — relative humidity, fraction.
/// - `rr` — precipitation rate, mm/h.
///
/// # Returns
/// `None` when `species.reldiff <= 0` (upstream leaves `rc` unassigned).
/// Otherwise `r_c >= 10 s/m`; upstream floors it there.
#[must_use]
pub fn getrc(
    cell: &SurfaceResistances,
    species: &GasSpecies,
    t_celsius: f64,
    gr: f64,
    rh: f64,
    rr: f64,
) -> Option<f64> {
    let t = t_celsius;
    // Stomatal resistance; outside 0..40 C the stomata are closed.
    let mut rs = if t > 0.0 && t < 40.0 {
        let g = 200.0 / (gr + 0.1);
        cell.ri * (1.0 + g * g) * (400.0 / (t * (40.0 - t)))
    } else {
        1.0e25
    };
    if rh > 0.9 || rr > 0.0 {
        rs *= 3.0;
    }
    let rdc = 100.0 * (1.0 + 1000.0 / (gr + 10.0));
    let corr = 1000.0 * (-t - 4.0).exp();

    if species.reldiff <= 0.0 {
        return None;
    }
    let rsm = rs * species.reldiff + species.rm;
    let mut rluc = cell.rlu + corr;
    let rclc = cell.rcl + corr;
    let rgsc = cell.rgs + corr;
    if rr > 0.0 {
        let rluo = 1.0 / (1.0 / 1000.0 + 1.0 / (3.0 * rluc));
        rluc = 1.0 / (1.0 / (3.0 * rluc) + 1.0e-7 * species.henry + species.f0 / rluo);
    } else if rh > 0.9 {
        let rluo = 1.0 / (1.0 / 3000.0 + 1.0 / (3.0 * rluc));
        rluc = 1.0 / (1.0 / (3.0 * rluc) + 1.0e-7 * species.henry + species.f0 / rluo);
    }
    let rc = 1.0 / (1.0 / rsm + 1.0 / rluc + 1.0 / (rdc + rclc) + 1.0 / (cell.rac + rgsc));
    Some(if rc < 10.0 { 10.0 } else { rc })
}

/// `partdep.f90`: particle dry deposition velocity, summed over the size bins.
///
/// Per bin, with Stokes number `St = v_s u*^2 / (g nu)`:
/// `r_dp = 1 / ((Sc^-2/3 + 10^(-3/St)) u*)` and
/// `v_d = v_s + 1 / (r_a + r_dp + r_a r_dp v_s)`; below `u* = 1e-5` only
/// settling remains. The bins are mass-weighted.
///
/// # Arguments
/// - `vdep` — value to accumulate into; upstream **adds** to its `vdepo`
///   (`getvdep` zeroes it first). Kept as an argument because the order of
///   that sum is part of what is verified.
/// - `density` — particle density, kg/m³; `<= 0` marks a non-particle species
///   and returns `vdep` unchanged.
/// - `bins` — from [`super::aerosol::part0`]: uses `settling_velocity`,
///   `schmidt_factor` and `mass_fraction`.
/// - `ra` — aerodynamic resistance, s/m.
/// - `ustar` — friction velocity, m/s.
/// - `nyl` — kinematic viscosity of air, m²/s.
#[must_use]
pub fn partdep(vdep: f64, density: f64, bins: &AerosolBins, ra: f64, ustar: f64, nyl: f64) -> f64 {
    const EPS: f64 = 1.0e-5;
    let mut vdep = vdep;
    if density > 0.0 {
        for j in 0..NI {
            let vset = bins.settling_velocity[j];
            let schmi = bins.schmidt_factor[j];
            let vdepj = if ustar > EPS {
                let stokes = vset / GA * ustar * ustar / nyl;
                let alpha = -3.0 / stokes;
                let rdp = if alpha <= EPS.log10() {
                    1.0 / (schmi * ustar)
                } else {
                    1.0 / ((schmi + 10.0_f64.powf(alpha)) * ustar)
                };
                vset + 1.0 / (ra + rdp + ra * rdp * vset)
            } else {
                vset
            };
            vdep += vdepj * bins.mass_fraction[j];
        }
    }
    vdep
}

/// How a species deposits, mapped from upstream's flag combination.
///
/// Upstream encodes this in three numbers per species: `reldiff > 0` marks a
/// gas, `density > 0` a particle, and `reldiff < 0`, `density < 0`,
/// `dryvel > 0` a species with a prescribed velocity.
// The particle variant carries its 11 size bins inline (352 bytes). The
// workspace rules forbid `Box`, and the type is passed by reference.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepositionSpecies {
    /// Wesely resistance model.
    Gas(GasSpecies),
    /// Slinn particle scheme with the given density (kg/m³) and size bins.
    Particle {
        /// Particle density, kg/m³.
        density: f64,
        /// Size bins from [`super::aerosol::part0`].
        bins: AerosolBins,
    },
    /// Constant deposition velocity, m/s (`dryvel`). Upstream applies it only
    /// when it is `> 0`; otherwise the species does not deposit.
    Prescribed(f64),
}

/// Surface meteorology `getvdep` needs at one grid cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceMet {
    /// Friction velocity, m/s.
    pub ust: f64,
    /// 2 m temperature, K.
    pub temp: f64,
    /// Surface pressure, Pa.
    pub pa: f64,
    /// Obukhov length, m.
    pub ol: f64,
    /// Global radiation, W/m².
    pub gr: f64,
    /// Relative humidity, fraction.
    pub rh: f64,
    /// Precipitation rate, mm/h.
    pub rr: f64,
    /// Snow depth, m water equivalent. Above `0.001` the cell is treated as
    /// fully snow-covered (landuse class 12).
    pub snow: f64,
}

/// The Wesely season (1-based, as upstream numbers them) `getvdep` uses for a
/// Julian date and latitude.
///
/// Reproduces upstream exactly, including two simplifications worth knowing:
/// the southern hemisphere is handled by adding **182 days** (`365/2` in
/// integer arithmetic), and everything between 20°S and 20°N is **always
/// summer** (`mmdd = 600`).
#[must_use]
pub fn wesely_season(jul: f64, ylat: f64) -> usize {
    let mut jul = jul;
    if ylat < 0.0 {
        jul += f64::from(365 / 2);
    }
    let (yyyymmdd, _) = caldate(jul);
    let yyyy = yyyymmdd / 10000;
    let mut mmdd = yyyymmdd - 10000 * yyyy;
    if ylat > -20.0 && ylat < 20.0 {
        mmdd = 600;
    }
    if mmdd >= 1201 || mmdd <= 301 {
        4
    } else if mmdd >= 1101 || mmdd <= 331 {
        3
    } else if (401..=515).contains(&mmdd) {
        5
    } else if (516..=915).contains(&mmdd) {
        1
    } else {
        2
    }
}

/// `getvdep.f90`: dry deposition velocity of one species at one grid cell,
/// m/s.
///
/// # Arguments
/// - `jul` — Julian date of the meteorological field (`bdate + wftime/86400`).
/// - `ylat` — latitude of the cell, degrees.
/// - `met` — surface meteorology.
/// - `landuse` — fraction of each of the `NUMCLASS` landuse classes.
/// - `z0` — roughness length of each class, m.
/// - `table` — Wesely resistances `[season][class]` for this species.
/// - `species` — how the species deposits.
///
/// # Notes
/// The particle scheme uses the landuse-weighted **mean** `r_a`, as upstream
/// does, not a per-class velocity.
#[must_use]
pub fn getvdep(
    jul: f64,
    ylat: f64,
    met: &SurfaceMet,
    landuse: &[f64; NUMCLASS],
    z0: &[f64; NUMCLASS],
    table: &[[SurfaceResistances; NUMCLASS]; NSEASON],
    species: &DepositionSpecies,
) -> f64 {
    const EPS: f64 = 1.0e-5;
    let lseason = wesely_season(jul, ylat);
    let temp = met.temp;
    let pa = met.pa;

    let diffh2o = 2.11e-5 * (temp / 273.15).powf(1.94) * (101_325.0 / pa);
    let tc = temp - 273.15;
    let myl = if tc < 0.0 {
        (1.718 + 0.0049 * tc - 1.2e-05 * (tc * tc)) * 1.0e-05
    } else {
        (1.718 + 0.0049 * tc) * 1.0e-05
    };
    let rhoa = pa / (287.0 * temp);
    let nyl = myl / rhoa;

    let mut vdepo = 0.0;
    let gas = match species {
        DepositionSpecies::Gas(g) => Some(g),
        _ => None,
    };
    let rb = gas.and_then(|g| getrb(met.ust, nyl, diffh2o, g.reldiff));

    let mut slanduse = [0.0; NUMCLASS];
    for (j, s) in slanduse.iter_mut().enumerate() {
        *s = if met.snow > 0.001 {
            // class 12 (1-based) is snow and ice
            if j == 11 {
                1.0
            } else {
                0.0
            }
        } else {
            landuse[j]
        };
    }

    let mut raquer = 0.0;
    for j in 0..NUMCLASS {
        if slanduse[j] > EPS {
            let ra = raerod(met.ol, met.ust, z0[j]);
            raquer += ra * slanduse[j];
            if let (Some(g), Some(rb)) = (gas, rb) {
                let rc = getrc(&table[lseason - 1][j], g, tc, met.gr, met.rh, met.rr)
                    .expect("reldiff > 0 for a gas");
                let vd = if ra + rb + rc > 0.0 {
                    1.0 / (ra + rb + rc)
                } else {
                    9.999
                };
                vdepo += vd * slanduse[j];
            }
        }
    }

    match species {
        DepositionSpecies::Particle { density, bins } => {
            vdepo = partdep(vdepo, *density, bins, raquer, met.ust, nyl);
        }
        DepositionSpecies::Prescribed(dryvel) if *dryvel > 0.0 => vdepo = *dryvel,
        _ => {}
    }
    vdepo
}

/// `get_settling.f90`: settling velocity of a particle species at height `zt`,
/// m/s, **negative** (downward).
///
/// Starts from the Stokes estimate `vsetaver` and iterates the drag
/// coefficient (`24/Re`, `18.5/Re^0.6`, `0.44` for `Re` below `1.917`, below
/// `500`, and above) up to 20 times, stopping at a 1 % change. Air temperature
/// and density are interpolated linearly from the column.
///
/// # Arguments
/// - `zt` — particle height, m.
/// - `height` — model level heights, m, increasing (upstream's `height(1:nz)`).
/// - `tt`, `rho` — temperature (K) and air density (kg/m³) on those levels.
/// - `dquer` — mean particle diameter, **µm**.
/// - `density` — particle density, kg/m³.
/// - `cunningham` — Cunningham slip factor of the species.
/// - `vsetaver` — the Stokes settling velocity from `readreleases`, m/s
///   (negative).
///
/// # Returns
/// `None` if `zt` is not below the top level. Upstream then uses an
/// **undefined** level index; the port refuses rather than guess.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn get_settling(
    zt: f64,
    height: &[f64],
    tt: &[f64],
    rho: &[f64],
    dquer: f64,
    density: f64,
    cunningham: f64,
    vsetaver: f64,
) -> Option<f64> {
    let indz = (1..height.len()).find(|&i| height[i] > zt)? - 1;
    let dz = 1.0 / (height[indz + 1] - height[indz]);
    let dz1 = (zt - height[indz]) * dz;
    let dz2 = (height[indz + 1] - zt) * dz;
    let temperature = dz2 * tt[indz] + dz1 * tt[indz + 1];
    let airdens = dz2 * rho[indz] + dz1 * rho[indz + 1];
    let vis_dyn = viscosity_kelvin(temperature);
    let vis_kin = vis_dyn / airdens;

    let mut reynolds = dquer / 1.0e6 * vsetaver.abs() / vis_kin;
    let mut settling_old = vsetaver;
    let mut settling = vsetaver;
    for _ in 0..20 {
        let c_d = if reynolds < 1.917 {
            24.0 / reynolds
        } else if reynolds < 500.0 {
            18.5 / reynolds.powf(0.6)
        } else {
            0.44
        };
        settling =
            -(4.0 * GA * dquer / 1.0e6 * density * cunningham / (3.0 * c_d * airdens)).sqrt();
        if ((settling - settling_old) / settling).abs() < 0.01 {
            break;
        }
        reynolds = dquer / 1.0e6 * settling.abs() / vis_kin;
        settling_old = settling;
    }
    Some(settling)
}
