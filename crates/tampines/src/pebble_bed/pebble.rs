//! # Pebble radial conduction — the middle of the nested scales
//!
//! Steady-state radial heat conduction through a spherical fuel element: an
//! inner **fuelled zone**, in which TRISO coated particles are dispersed in
//! matrix graphite and in which all the fission power is released, surrounded
//! by an **unfuelled graphite shell** that produces no heat and only conducts.
//!
//! For the HTR-10 element that is a 6.0 cm sphere with a 5.0 cm-diameter
//! fuelled zone — a 0.5 cm graphite shell — with matrix graphite of density
//! 1.73 g/cm^3 (IAEA-TECDOC-1382 part 2, Chapter 4, Tables 4-2 and 4-17, Open
//! tier).
//!
//! ## Where this sits in the nest
//!
//! Level 2 of three. Its fuelled-zone conductivity is built from level 1
//! ([`super::triso`]) via a dispersion model; its pebble-surface temperature
//! is the boundary condition that level 3 ([`super::cht`], bed-to-helium
//! convection) and the bed effective conductivity ([`super::zbs`]) supply.
//!
//! ## The fuelled zone is NOT treated as homogeneous graphite
//!
//! This is the deliberate physical choice of this module, and it is worth
//! stating plainly because the opposite choice is the common shortcut.
//!
//! A pebble-bed core is *doubly heterogeneous*: fuel kernels inside particles
//! inside pebbles inside a bed. Smearing the TRISO particles into the matrix —
//! treating the fuelled zone as one homogeneous graphite-and-fuel medium with
//! volume-averaged properties — discards the kernel-to-matrix temperature
//! difference entirely. The neutronic cost of the analogous smearing is
//! quantified: Wang, Sheu, Peir and Liang (2014), *Criticality calculations of
//! the HTR-10 pebble-bed reactor with SCALE6/CSAS6 and MCNP5*, Ann. Nucl.
//! Energy 64, 1-7 (proprietary tier, catalogued at
//! `crates/kovan-literature/proprietary/papers/wang2014htr10criticality.json`
//! — cited, not reproduced) measure roughly **+2800 pcm** in `k_eff` for a
//! fully homogenised (INFHOMMEDIUM) unit cell against a continuous-energy
//! reference, falling to about +280 pcm once the double heterogeneity is
//! treated explicitly.
//!
//! The thermal analogue is real for the same structural reason: the fuel is
//! not where the average says it is. This module therefore keeps the two
//! scales separate — the fuelled zone gets an *effective* conductivity from a
//! dispersion model ([`DispersionModel`]), and the kernel temperature is
//! recovered by superposing level 1's particle solution on the local matrix
//! temperature ([`Pebble::steady_state_temperatures`]). **No claim is made
//! that the thermal error of homogenisation is 2800 pcm-equivalent** — that
//! number is a neutronic result and is quoted only as evidence that the
//! heterogeneity matters, not as a thermal bound.
//!
//! ## Provenance
//!
//! - Geometry and matrix density: IAEA-TECDOC-1382 part 2, Chapter 4
//!   (Open tier), Table 4-17.
//! - Matrix and shell graphite conductivity: consumed from
//!   [`tuas_boussinesq_solver`]'s `NuclearGraphiteMatrixA3` correlations
//!   rather than hardcoded here; those in turn transcribe the CC-BY-4.0
//!   Virtual Test Bed HTR-PM deck.
//! - Dispersion models: Maxwell-Eucken (Maxwell 1873) and Chiew & Glandt
//!   (1983) — see [`DispersionModel`] for the equations, their validity
//!   ranges, and an explicit transcription caveat.
//!
//! ## Status
//!
//! **NOT VALIDATED.** Verified against analytic limits and bounds only; no
//! comparison against any HTR-10 measurement. AI-assisted draft pending human
//! review per `RESPONSIBLE_USE.md`.
//!
//! **Belongs here:** pebble-scale geometry, the TRISO-in-matrix dispersion
//! rule, and the two-zone conduction solution. **Does not belong here:**
//! particle-internal conduction ([`super::triso`]), bed-scale effective
//! conductivity ([`super::zbs`]), or the convective boundary condition
//! ([`super::cht`]).

use std::f64::consts::PI;

use uom::si::f64::{
    Length, Mass, MassDensity, Power, Ratio, ThermalConductivity, ThermodynamicTemperature, Volume,
};
use uom::si::length::{centimeter, meter};
use uom::si::mass::gram;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::solid_database::nuclear_graphite::nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent;

use super::triso::{
    solid_sphere_centre_temperature_rise, spherical_shell_temperature_rise, FastNeutronFluence,
    TrisoParticle, TrisoTemperatureProfile, MAX_TEMPERATURE_KELVIN, MIN_TEMPERATURE_KELVIN,
};
use crate::TampinesError;

/// Standard atomic weight of U-235, 235.0439299 g/mol (IUPAC/CIAAW).
pub const MOLAR_MASS_U235_G_PER_MOL: f64 = 235.0439299;

/// Standard atomic weight of U-238, 238.0507882 g/mol (IUPAC/CIAAW).
pub const MOLAR_MASS_U238_G_PER_MOL: f64 = 238.0507882;

/// Standard atomic weight of oxygen, 15.9994 g/mol (IUPAC/CIAAW).
pub const MOLAR_MASS_OXYGEN_G_PER_MOL: f64 = 15.9994;

/// How the conductivity of a dilute dispersion of spherical inclusions in a
/// continuous matrix is combined into one effective conductivity.
///
/// A closed set of two models — enum dispatch, no trait objects, per the
/// workspace Rust design rules. Both take the same three inputs (matrix
/// conductivity, inclusion conductivity, inclusion volume fraction) and both
/// reduce to the matrix conductivity as the volume fraction goes to zero.
///
/// Throughout, `kappa = k_particle / k_matrix` and
/// `beta = (kappa - 1) / (kappa + 2)`. `beta` lies in `(-0.5, 1)`: it is
/// positive when the inclusions conduct better than the matrix and negative
/// when they conduct worse, which is the TRISO-in-graphite case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispersionModel {
    /// **Maxwell-Eucken** (Maxwell's 1873 result for a dilute dispersion of
    /// non-interacting spheres):
    ///
    /// `k_eff / k_matrix = (1 + 2 beta phi) / (1 - beta phi)`
    ///
    /// **Validity:** exact to first order in the inclusion volume fraction
    /// `phi`, because it neglects sphere-sphere interactions entirely. It is
    /// reliable for `phi` up to roughly 0.1 and is conventionally used up to
    /// about 0.2; beyond that the neglected interactions matter. It always
    /// lies within the Wiener (series/parallel) bounds, and in fact coincides
    /// with one of the tighter Hashin-Shtrikman bounds.
    MaxwellEucken,

    /// **Chiew & Glandt (1983)** — Maxwell extended to third order in the
    /// inclusion volume fraction for a random dispersion of hard spheres:
    ///
    /// `k_eff / k_matrix = [1 + 2 beta phi + (2 beta^3 - 0.1 beta) phi^2
    ///                      + 0.05 phi^3 exp(4.5 beta)] / (1 - beta phi)`
    ///
    /// Source: Chiew, Y. C. and Glandt, E. D., *The effect of structure on the
    /// conductivity of a dispersion*, J. Colloid Interface Sci. 94(1) (1983)
    /// 90-104. This is the mixing rule the Virtual Test Bed / Pronghorn
    /// pebble decks select for the fuel matrix (`k_mixing = 'chiew'`, e.g.
    /// `reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`
    /// line 250; CC-BY-4.0, Open tier), which is why it is the default here.
    ///
    /// **Validity:** derived for randomly dispersed, non-overlapping spheres;
    /// the `phi^2` and `phi^3` terms extend usable accuracy to roughly
    /// `phi = 0.6`. At the HTR-10 particle packing fraction of about 0.05 it
    /// differs from Maxwell-Eucken by well under a percent — the two agree
    /// wherever the dispersion is genuinely dilute, and the tests below
    /// measure that agreement.
    ///
    /// **Known artefact — measured, not assumed.** The third-order term
    /// `0.05 phi^3 exp(4.5 beta)` does **not** vanish at `beta = 0`. When the
    /// inclusions are made of the same material as the matrix (`kappa = 1`,
    /// hence `beta = 0`), which is not a composite at all and must return the
    /// matrix conductivity exactly, this expression instead returns
    /// `k_matrix (1 + 0.05 phi^3)`. The same term lets the correlation stray
    /// marginally outside the Wiener bounds near `beta = 0`: measured on
    /// 2026-08-11, the largest excursion over a 49-point sweep was
    /// **1.0800e-2 relative, at `kappa` = 1 and `phi` = 0.6**, which is
    /// exactly `0.05 phi^3` — so the artefact is bounded by that term and
    /// nothing worse is hiding behind it. It is negligible in the dilute
    /// regime this module uses: `0.05 phi^3` is 6.3e-6 at the HTR-10 `phi` of
    /// 0.0502. Use [`DispersionModel::MaxwellEucken`] if an exactly
    /// bound-respecting rule is required.
    ///
    /// **Transcription caveat** (honesty per `RESPONSIBLE_USE.md`): the
    /// polynomial above was implemented without page-level access to Chiew &
    /// Glandt (1983) in this session. The falsifiable checks in this module's
    /// tests — Maxwell agreement at small `phi`, Wiener bounds, the `phi -> 0`
    /// degeneracy — all pass, but a human should verify the coefficients
    /// `2 beta^3 - 0.1 beta` and `0.05 exp(4.5 beta)` against the paper before
    /// this module is promoted past Prototype in the V&V pipeline. The
    /// `beta = 0` artefact above is exactly the kind of thing that check
    /// should resolve: it may be faithful to the published fit, or it may be a
    /// transcription error in the third-order coefficient.
    ChiewGlandt,
}

impl DispersionModel {
    /// Effective thermal conductivity, W/(m K), of `particle_volume_fraction`
    /// of spherical inclusions of conductivity `particle_conductivity`
    /// dispersed in a continuous matrix of conductivity `matrix_conductivity`.
    ///
    /// `particle_volume_fraction` is dimensionless and must lie in `[0, 1)`;
    /// both conductivities must be strictly positive. Returns
    /// [`TampinesError::InvalidInput`] otherwise. At a volume fraction of
    /// exactly zero both models return the matrix conductivity exactly.
    pub fn effective_conductivity(
        &self,
        matrix_conductivity: ThermalConductivity,
        particle_conductivity: ThermalConductivity,
        particle_volume_fraction: Ratio,
    ) -> Result<ThermalConductivity, TampinesError> {
        let k_matrix = matrix_conductivity.get::<watt_per_meter_kelvin>();
        let k_particle = particle_conductivity.get::<watt_per_meter_kelvin>();
        let phi = particle_volume_fraction.get::<ratio>();

        if k_matrix <= 0.0 || k_particle <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "dispersion conductivities must be strictly positive, got \
                 matrix {k_matrix} W/(m K) and particle {k_particle} W/(m K)"
            )));
        }
        if !(0.0..1.0).contains(&phi) {
            return Err(TampinesError::InvalidInput(format!(
                "dispersed-phase volume fraction must lie in [0, 1), got {phi}"
            )));
        }

        let kappa = k_particle / k_matrix;
        let beta = (kappa - 1.0) / (kappa + 2.0);

        let numerator = match self {
            DispersionModel::MaxwellEucken => 1.0 + 2.0 * beta * phi,
            DispersionModel::ChiewGlandt => {
                1.0 + 2.0 * beta * phi
                    + (2.0 * beta * beta * beta - 0.1 * beta) * phi * phi
                    + 0.05 * phi * phi * phi * (4.5 * beta).exp()
            }
        };

        let ratio_to_matrix = numerator / (1.0 - beta * phi);

        Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(
            k_matrix * ratio_to_matrix,
        ))
    }
}

/// A spherical fuel element: a fuelled inner zone of TRISO particles dispersed
/// in matrix graphite, inside an unfuelled graphite shell.
///
/// Plain data; the physics lives in [`Pebble::fuelled_zone_conductivity`] and
/// [`Pebble::steady_state_temperatures`]. Construct with [`Pebble::new`]
/// (checked) or [`Pebble::htr10`] (the cited HTR-10 fuel element).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pebble {
    /// Outer radius of the whole pebble, metres. HTR-10: 0.03 m (6.0 cm
    /// diameter).
    pub outer_radius: Length,
    /// Outer radius of the fuelled zone, metres — the boundary between the
    /// particle-bearing matrix and the unfuelled shell. HTR-10: 0.025 m
    /// (5.0 cm diameter), leaving a 5 mm shell.
    pub fuelled_zone_radius: Length,
    /// The coated particle dispersed in the fuelled zone (level 1).
    pub particle: TrisoParticle,
    /// Number of coated particles per pebble, dimensionless count stored as
    /// `f64` because it enters volume-fraction and per-particle-power algebra
    /// rather than being an index. HTR-10: 8335 (IAEA-TECDOC-1382 part 2,
    /// Chapter 4; see [`coated_particles_per_pebble`], which reproduces that
    /// figure from the published heavy-metal loading).
    pub particles_per_pebble: f64,
    /// Which dispersion rule mixes the particle conductivity into the matrix.
    pub dispersion_model: DispersionModel,
}

impl Pebble {
    /// Builds a pebble from its two radii, its particle, the particle count
    /// and the dispersion model, checking that the fuelled zone fits strictly
    /// inside the pebble, that the particles fit inside the fuelled zone, and
    /// that the count is non-negative.
    ///
    /// Returns [`TampinesError::InvalidInput`] if the geometry is impossible —
    /// including the case where the particles would occupy more than the
    /// fuelled zone's volume, which is the failure mode a mistyped particle
    /// count produces.
    pub fn new(
        outer_radius: Length,
        fuelled_zone_radius: Length,
        particle: TrisoParticle,
        particles_per_pebble: f64,
        dispersion_model: DispersionModel,
    ) -> Result<Self, TampinesError> {
        if fuelled_zone_radius.get::<meter>() <= 0.0 {
            return Err(TampinesError::InvalidInput(
                "pebble fuelled-zone radius must be strictly positive".to_string(),
            ));
        }
        if outer_radius < fuelled_zone_radius {
            return Err(TampinesError::InvalidInput(format!(
                "pebble outer radius ({:.6e} m) must be at least the fuelled-zone \
                 radius ({:.6e} m)",
                outer_radius.get::<meter>(),
                fuelled_zone_radius.get::<meter>()
            )));
        }
        if !(particles_per_pebble >= 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "particles per pebble must be non-negative and finite, got \
                 {particles_per_pebble}"
            )));
        }

        let candidate = Self {
            outer_radius,
            fuelled_zone_radius,
            particle,
            particles_per_pebble,
            dispersion_model,
        };

        let volume_fraction = candidate.triso_volume_fraction().get::<ratio>();
        if volume_fraction >= 1.0 {
            return Err(TampinesError::InvalidInput(format!(
                "{particles_per_pebble} coated particles would occupy a volume \
                 fraction of {volume_fraction} of the fuelled zone, which cannot \
                 exceed 1"
            )));
        }

        Ok(candidate)
    }

    /// The HTR-10 fuel element, transcribed from **IAEA-TECDOC-1382 part 2,
    /// Chapter 4** (Open tier): ball diameter 6.0 cm, fuelled-zone diameter
    /// 5.0 cm, 8335 coated particles per element (the figure that chapter's
    /// MCNP model states), with the HTR-10 particle of
    /// [`TrisoParticle::htr10`] and the [`DispersionModel::ChiewGlandt`]
    /// mixing rule the Virtual Test Bed pebble decks use.
    pub fn htr10() -> Self {
        Self::new(
            Length::new::<centimeter>(3.0),
            Length::new::<centimeter>(2.5),
            TrisoParticle::htr10(),
            8335.0,
            DispersionModel::ChiewGlandt,
        )
        .expect("the published HTR-10 pebble geometry is internally consistent")
    }

    /// Volume of the fuelled zone, m^3. HTR-10: 65.45 cm^3.
    pub fn fuelled_zone_volume(&self) -> Volume {
        (4.0 / 3.0)
            * PI
            * (self.fuelled_zone_radius * self.fuelled_zone_radius * self.fuelled_zone_radius)
    }

    /// Volume of the whole pebble, m^3. HTR-10: 113.10 cm^3.
    pub fn pebble_volume(&self) -> Volume {
        (4.0 / 3.0) * PI * (self.outer_radius * self.outer_radius * self.outer_radius)
    }

    /// Volume fraction of coated particles **within the fuelled zone**,
    /// dimensionless — the `phi` the dispersion model needs.
    ///
    /// Computed as `N * V_particle / V_fuelled_zone`, using the particle's
    /// full outer (OPyC) volume, because it is the whole coated particle that
    /// is dispersed in the matrix, not just its kernel. HTR-10: about 0.0502.
    ///
    /// Note this is a fraction *of the fuelled zone*, not of the pebble; the
    /// unfuelled shell contains no particles by definition.
    pub fn triso_volume_fraction(&self) -> Ratio {
        self.particles_per_pebble * self.particle.particle_volume() / self.fuelled_zone_volume()
    }

    /// Thermal conductivity of the matrix / shell graphite, W/(m K), at the
    /// given temperature and fast-neutron fluence.
    ///
    /// Consumed from [`tuas_boussinesq_solver`]'s A3-grade pebble matrix
    /// graphite correlation rather than duplicated here; the fuelled-zone
    /// matrix and the unfuelled shell are the same 1.73 g/cm^3 A3 graphite in
    /// the HTR-10 design, so one correlation serves both.
    ///
    /// Valid range: 300 K to 2000 K, fluence `gam` in `[0, 15]`; outside
    /// either, returns [`TampinesError::InvalidInput`].
    pub fn matrix_conductivity(
        &self,
        temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<ThermalConductivity, TampinesError> {
        nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent(temperature, fluence)
            .map_err(|error| {
                TampinesError::InvalidInput(format!(
                    "TUAS A3 matrix graphite conductivity rejected temperature \
                     {} K / fluence {}: {error:?}",
                    temperature.get::<kelvin>(),
                    fluence.get::<ratio>()
                ))
            })
    }

    /// Effective thermal conductivity of the **fuelled zone**, W/(m K), at the
    /// given temperature and fluence: the coated particles' own effective
    /// conductivity ([`TrisoParticle::effective_conductivity`]) dispersed in
    /// the matrix graphite by [`Pebble::dispersion_model`] at the particle
    /// volume fraction [`Pebble::triso_volume_fraction`].
    ///
    /// This is the composition point of levels 1 and 2 — where the particle
    /// scale enters the pebble scale. Because the TRISO particle is *less*
    /// conductive than the matrix graphite (about 2.3 against about 30
    /// W/(m K) at 1000 K), dispersing it **lowers** the zone conductivity
    /// below that of plain graphite; the fuel is a thermal impediment, not a
    /// shortcut.
    ///
    /// Valid range: 300 K to 2000 K, fluence `gam` in `[0, 15]`.
    pub fn fuelled_zone_conductivity(
        &self,
        temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<ThermalConductivity, TampinesError> {
        let matrix = self.matrix_conductivity(temperature, fluence)?;
        let particle = self.particle.effective_conductivity(temperature, fluence)?;

        self.dispersion_model
            .effective_conductivity(matrix, particle, self.triso_volume_fraction())
    }

    /// Steady-state radial temperature profile of the pebble, given its total
    /// fission power and the temperature imposed on its outer surface.
    ///
    /// **Physics.** Power is released uniformly through the fuelled zone of
    /// radius `a`, and the unfuelled shell from `a` to `R` carries all of it
    /// with no generation of its own:
    ///
    /// `T(0) - T(a) = Q / (8 pi k_fuelled a)`
    ///
    /// `T(a) - T(R) = Q / (4 pi k_graphite) * (1/a - 1/R)`
    ///
    /// Both conductivities depend on temperature, so as in
    /// [`TrisoParticle::steady_state_temperatures`] the profile is found by
    /// fixed-point iteration, each zone's conductivity evaluated at that
    /// zone's arithmetic-mean temperature, converging when no node moves by
    /// more than 1e-9 K.
    ///
    /// **The hottest kernel.** After the pebble profile converges, level 1 is
    /// superposed: a TRISO particle carrying `Q / N` is solved with the
    /// *pebble-centre* temperature as its surface boundary condition, and its
    /// kernel centre reported as [`PebbleTemperatureProfile::peak_kernel_centre`].
    /// Two assumptions are being made and should be understood as such:
    /// (1) the hottest particle sits at the pebble centre, which is the
    /// bounding position, and (2) the particle does not perturb the matrix
    /// temperature field it sits in, which holds while the dispersion is
    /// dilute (HTR-10: about 5% by volume) and degrades as packing rises.
    ///
    /// **Inputs.** `power` is the total fission power of one pebble, watts
    /// (HTR-10 core average: 10 MW / 27 000 = 370.4 W).
    /// `surface_temperature` is the pebble's outer surface temperature, which
    /// in the nested stack comes from the bed-to-helium coupling
    /// ([`super::cht`]). `fluence` is `gam` in units of 10^25 n/m^2.
    ///
    /// Returns [`TampinesError::InvalidInput`] for negative power or for
    /// temperatures leaving the 300 K to 2000 K correlation window, and
    /// [`TampinesError::Numerical`] if the iteration fails to converge in 200
    /// passes.
    pub fn steady_state_temperatures(
        &self,
        power: Power,
        surface_temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<PebbleTemperatureProfile, TampinesError> {
        if power.value < 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "pebble power must be non-negative, got {} W",
                power.value
            )));
        }
        let surface_kelvin = surface_temperature.get::<kelvin>();
        if !(MIN_TEMPERATURE_KELVIN..=MAX_TEMPERATURE_KELVIN).contains(&surface_kelvin) {
            return Err(TampinesError::InvalidInput(format!(
                "pebble surface temperature {surface_kelvin} K is outside the \
                 coded correlation range {MIN_TEMPERATURE_KELVIN} K to \
                 {MAX_TEMPERATURE_KELVIN} K"
            )));
        }

        let max_iterations = 200;
        let tolerance_kelvin = 1e-9;

        // [centre, fuelled-zone boundary], with the surface held fixed.
        let mut nodes = [surface_temperature; 2];

        for iteration in 0..=max_iterations {
            let fuelled_mean = mean_temperature(nodes[0], nodes[1]);
            let shell_mean = mean_temperature(nodes[1], surface_temperature);

            let k_fuelled = self.fuelled_zone_conductivity(fuelled_mean, fluence)?;
            let k_shell = self.matrix_conductivity(shell_mean, fluence)?;

            let boundary = surface_temperature
                + spherical_shell_temperature_rise(
                    power,
                    self.fuelled_zone_radius,
                    self.outer_radius,
                    k_shell,
                );
            let centre = boundary
                + solid_sphere_centre_temperature_rise(power, self.fuelled_zone_radius, k_fuelled);

            let updated = [centre, boundary];
            let largest_move = nodes
                .iter()
                .zip(updated.iter())
                .map(|(old, new)| (new.get::<kelvin>() - old.get::<kelvin>()).abs())
                .fold(0.0_f64, f64::max);
            nodes = updated;

            if largest_move < tolerance_kelvin {
                let particle_power = if self.particles_per_pebble > 0.0 {
                    power / self.particles_per_pebble
                } else {
                    Power::new::<uom::si::power::watt>(0.0)
                };
                let hottest_particle =
                    self.particle
                        .steady_state_temperatures(particle_power, nodes[0], fluence)?;

                return Ok(PebbleTemperatureProfile {
                    centre: nodes[0],
                    fuelled_zone_boundary: nodes[1],
                    surface: surface_temperature,
                    peak_kernel_centre: hottest_particle.kernel_centre,
                    hottest_particle,
                });
            }

            if iteration == max_iterations {
                return Err(TampinesError::Numerical(format!(
                    "pebble steady-state temperature iteration did not converge in \
                     {max_iterations} iterations; last node movement was \
                     {largest_move:e} K"
                )));
            }
        }

        unreachable!("the loop above returns on convergence or on exhaustion")
    }
}

/// The steady radial temperature field of a fuel pebble, plus the hottest
/// coated particle superposed on it.
///
/// Every field is an absolute temperature (`uom`
/// `ThermodynamicTemperature`, kelvin in SI). Produced by
/// [`Pebble::steady_state_temperatures`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleTemperatureProfile {
    /// Temperature at the geometric centre of the pebble — the hottest point
    /// of the *matrix*, but not the hottest point of the fuel; see
    /// [`Self::peak_kernel_centre`].
    pub centre: ThermodynamicTemperature,
    /// Temperature at the fuelled-zone / unfuelled-shell boundary.
    pub fuelled_zone_boundary: ThermodynamicTemperature,
    /// Temperature imposed on the pebble outer surface (the model's boundary
    /// condition, returned unchanged for convenience).
    pub surface: ThermodynamicTemperature,
    /// Temperature at the centre of the hottest UO2 kernel — the true peak
    /// fuel temperature, obtained by superposing the level-1 particle solution
    /// on [`Self::centre`]. This is the quantity a fuel-temperature limit
    /// applies to.
    pub peak_kernel_centre: ThermodynamicTemperature,
    /// The full level-1 profile of that hottest particle, for callers that
    /// want its internal breakdown (SiC temperature, for instance, which
    /// governs fission-product retention).
    pub hottest_particle: TrisoTemperatureProfile,
}

impl PebbleTemperatureProfile {
    /// Total temperature rise from the pebble surface to the hottest kernel
    /// centre, kelvin — the whole nested contribution of levels 1 and 2.
    pub fn total_rise(&self) -> uom::si::f64::TemperatureInterval {
        super::temperature_difference(self.peak_kernel_centre, self.surface)
    }

    /// Temperature rise across the pebble alone, surface to centre, kelvin —
    /// excluding the particle-scale contribution.
    pub fn matrix_rise(&self) -> uom::si::f64::TemperatureInterval {
        super::temperature_difference(self.centre, self.surface)
    }
}

/// Heavy-metal (uranium) mass fraction of UO2 at the given U-235 enrichment by
/// weight, dimensionless.
///
/// `enrichment` is the U-235 **weight** fraction of the uranium (HTR-10: 0.17).
/// The weight fraction is converted to an atom fraction before averaging the
/// uranium molar mass, because it is atoms, not grams, that pair with the two
/// oxygens:
///
/// `x_235 = (w_235 / M_235) / (w_235 / M_235 + w_238 / M_238)`
///
/// `M_U = x_235 M_235 + (1 - x_235) M_238`
///
/// `f_HM = M_U / (M_U + 2 M_O)`
///
/// with the IUPAC/CIAAW standard atomic weights in
/// [`MOLAR_MASS_U235_G_PER_MOL`], [`MOLAR_MASS_U238_G_PER_MOL`] and
/// [`MOLAR_MASS_OXYGEN_G_PER_MOL`]. At 17 wt% enrichment the result is about
/// 0.8813; for natural uranium it is about 0.8815 — the enrichment dependence
/// is very weak, which is why quoting one figure for "UO2" is usually safe and
/// why this function exists anyway, so the assumption is visible.
///
/// Returns [`TampinesError::InvalidInput`] for an enrichment outside `[0, 1]`.
pub fn uranium_dioxide_heavy_metal_fraction(enrichment: Ratio) -> Result<Ratio, TampinesError> {
    let weight_fraction_235 = enrichment.get::<ratio>();

    if !(0.0..=1.0).contains(&weight_fraction_235) {
        return Err(TampinesError::InvalidInput(format!(
            "U-235 enrichment (weight fraction) must lie in [0, 1], got \
             {weight_fraction_235}"
        )));
    }

    let moles_235 = weight_fraction_235 / MOLAR_MASS_U235_G_PER_MOL;
    let moles_238 = (1.0 - weight_fraction_235) / MOLAR_MASS_U238_G_PER_MOL;
    let atom_fraction_235 = moles_235 / (moles_235 + moles_238);

    let molar_mass_uranium = atom_fraction_235 * MOLAR_MASS_U235_G_PER_MOL
        + (1.0 - atom_fraction_235) * MOLAR_MASS_U238_G_PER_MOL;
    let molar_mass_uo2 = molar_mass_uranium + 2.0 * MOLAR_MASS_OXYGEN_G_PER_MOL;

    Ok(Ratio::new::<ratio>(molar_mass_uranium / molar_mass_uo2))
}

/// Number of coated particles in a pebble, derived from the published
/// heavy-metal loading rather than quoted.
///
/// `N = m_HM / (V_kernel * rho_UO2 * f_HM)`, where `V_kernel` is the sphere
/// volume of `kernel_radius`, `rho_UO2` the kernel density, and `f_HM` the
/// uranium mass fraction of UO2 from
/// [`uranium_dioxide_heavy_metal_fraction`]. The result is a real number, not
/// rounded — a fuel specification fixes the loading, and the particle count
/// that follows need not be an integer.
///
/// This is the check that a pebble's stated particle count and its stated
/// heavy-metal loading describe the same fuel element; see the unit test,
/// which recovers the HTR-10 figure of 8335 from the published 5.0 g loading.
///
/// Returns [`TampinesError::InvalidInput`] for a non-positive radius, density
/// or mass, or an enrichment outside `[0, 1]`.
pub fn coated_particles_per_pebble(
    heavy_metal_mass: Mass,
    kernel_radius: Length,
    uranium_dioxide_density: MassDensity,
    enrichment: Ratio,
) -> Result<f64, TampinesError> {
    if heavy_metal_mass.value <= 0.0
        || kernel_radius.value <= 0.0
        || uranium_dioxide_density.value <= 0.0
    {
        return Err(TampinesError::InvalidInput(format!(
            "heavy-metal mass ({} kg), kernel radius ({} m) and UO2 density \
             ({} kg/m^3) must all be strictly positive",
            heavy_metal_mass.value, kernel_radius.value, uranium_dioxide_density.value
        )));
    }

    let heavy_metal_fraction = uranium_dioxide_heavy_metal_fraction(enrichment)?;

    let kernel_volume: Volume = (4.0 / 3.0) * PI * (kernel_radius * kernel_radius * kernel_radius);
    let heavy_metal_per_kernel: Mass =
        kernel_volume * uranium_dioxide_density * heavy_metal_fraction;

    Ok((heavy_metal_mass / heavy_metal_per_kernel).get::<ratio>())
}

/// The HTR-10 fuel element's published heavy-metal loading, 5.0 g of uranium
/// per pebble (IAEA-TECDOC-1382 part 2, Chapter 4, Tables 4-2 and 4-17, Open
/// tier).
pub fn htr10_heavy_metal_per_pebble() -> Mass {
    Mass::new::<gram>(5.0)
}

/// The HTR-10 fuel kernel's published density, 10.4 g/cm^3
/// (IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17, Open tier).
pub fn htr10_uranium_dioxide_density() -> MassDensity {
    MassDensity::new::<gram_per_cubic_centimeter>(10.4)
}

/// The HTR-10 fresh fuel's published U-235 enrichment, 17% by weight
/// (IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17, Open tier).
pub fn htr10_enrichment() -> Ratio {
    Ratio::new::<ratio>(0.17)
}

/// Arithmetic mean of two absolute temperatures, as an absolute temperature.
fn mean_temperature(
    a: ThermodynamicTemperature,
    b: ThermodynamicTemperature,
) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(0.5 * (a.get::<kelvin>() + b.get::<kelvin>()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::watt;
    use uom::si::volume::cubic_centimeter;

    /// Asserts that `measured` matches `expected` to within `max_relative`
    /// relative error. See the identical macro in [`super::super::triso`] for
    /// why `tampines` does not use the `approx` crate here.
    macro_rules! assert_relative_eq {
        ($expected:expr, $measured:expr, max_relative = $tolerance:expr) => {{
            let expected: f64 = $expected;
            let measured: f64 = $measured;
            let relative_error = if expected == 0.0 {
                measured.abs()
            } else {
                ((measured - expected) / expected).abs()
            };
            assert!(
                relative_error < $tolerance,
                "expected {expected}, measured {measured}, relative error \
                 {relative_error:e} exceeds {}",
                $tolerance
            );
        }};
    }

    /// V&V test: the published HTR-10 particle count follows from the
    /// published heavy-metal loading.
    ///
    /// **Methodology:** IAEA-TECDOC-1382 part 2, Chapter 4 states, in separate
    /// places, that an HTR-10 fuel element carries 5.0 g of uranium (Table
    /// 4-17) and that its MCNP model arranges **8,335 coated particles** in the
    /// graphite matrix. Those two statements are independent, so one can be
    /// derived from the other as a consistency check on this module's
    /// [`coated_particles_per_pebble`]: divide 5.0 g by the heavy-metal mass of
    /// one 0.025 cm-radius kernel of 10.4 g/cm^3 UO2 at 17 wt% enrichment.
    /// Pass criterion: within 0.1% of 8335.
    ///
    /// **Results (2026-08-11):** the heavy-metal mass fraction of 17 wt%
    /// enriched UO2 measured **0.8812805881788243** (natural uranium at
    /// 0.72 wt% gives 0.8814980674954724, confirming the enrichment dependence
    /// is negligible — the two differ by 0.025%). One kernel holds
    /// 6.544984694978735e-5 cm^3 * 10.4 g/cm^3 * 0.88128 =
    /// **5.998686680076754e-4 g** of heavy metal, so 5.0 g implies
    /// **8335.157788** particles — **0.0019%** above the published 8,335. The
    /// published integer is therefore this same calculation, rounded.
    ///
    /// **Interpretation:** the geometry, kernel density, enrichment and
    /// loading in [`Pebble::htr10`] and [`TrisoParticle::htr10`] describe one
    /// self-consistent fuel element, not a mixture of figures from different
    /// designs. This is an internal-consistency check on published data, not a
    /// validation against measurement.
    #[test]
    fn htr10_particle_count_follows_from_the_heavy_metal_loading() {
        let heavy_metal_fraction = uranium_dioxide_heavy_metal_fraction(htr10_enrichment())
            .unwrap()
            .get::<ratio>();
        let natural_fraction = uranium_dioxide_heavy_metal_fraction(Ratio::new::<ratio>(0.0072))
            .unwrap()
            .get::<ratio>();
        println!(
            "UO2 heavy-metal mass fraction: 17 wt% enriched {heavy_metal_fraction}, \
             natural {natural_fraction}"
        );

        let particle = TrisoParticle::htr10();
        let kernel_volume_cm3 = particle
            .layer_volume(super::super::triso::TrisoLayer::Kernel)
            .get::<cubic_centimeter>();
        let heavy_metal_per_kernel_gram = kernel_volume_cm3 * 10.4 * heavy_metal_fraction;
        println!(
            "kernel volume {kernel_volume_cm3:e} cm^3, heavy metal per kernel \
             {heavy_metal_per_kernel_gram:e} g"
        );

        let count = coated_particles_per_pebble(
            htr10_heavy_metal_per_pebble(),
            particle.kernel_radius,
            htr10_uranium_dioxide_density(),
            htr10_enrichment(),
        )
        .unwrap();

        println!(
            "derived particle count {count} against published 8335 \
             (deviation {:.4}%)",
            100.0 * (count - 8335.0) / 8335.0
        );
        assert_relative_eq!(8335.0, count, max_relative = 1e-3);
    }

    /// V&V test: the HTR-10 particle volume fraction of the fuelled zone.
    ///
    /// **Methodology:** compute [`Pebble::triso_volume_fraction`] for
    /// [`Pebble::htr10`] — 8335 particles of outer radius 455 um in a
    /// 2.5 cm-radius fuelled zone — and check it against the same ratio formed
    /// by hand from the two sphere volumes, to 1e-12 relative. Then require
    /// the fraction to be well inside the dilute regime (< 0.2) where both
    /// dispersion models are trustworthy.
    ///
    /// **Results (2026-08-11):** fuelled-zone volume 65.44984694978736 cm^3,
    /// single particle volume 3.9456885292638553e-4 cm^3, 8335 particles
    /// occupying 3.2887313891414234 cm^3 — a volume fraction of
    /// **0.050248114**, matching the hand ratio exactly.
    /// **Interpretation:** at 5.0% by volume the HTR-10 fuelled zone
    /// is a genuinely dilute dispersion, so Maxwell-Eucken and Chiew-Glandt
    /// must agree closely (they do — see the sibling test) and the
    /// non-interacting-sphere assumption behind both is sound here. Contrast
    /// with the Virtual Test Bed HTR-PM deck's 0.0905 for HTR-PM
    /// (`pebble_triso.i` line 249), which is nearly twice as dense and where
    /// the higher-order Chiew terms start to earn their place.
    #[test]
    fn htr10_triso_volume_fraction_is_dilute() {
        let pebble = Pebble::htr10();

        let fuelled_zone_cm3 = pebble.fuelled_zone_volume().get::<cubic_centimeter>();
        let particle_cm3 = pebble.particle.particle_volume().get::<cubic_centimeter>();
        let hand_fraction = 8335.0 * particle_cm3 / fuelled_zone_cm3;
        let measured = pebble.triso_volume_fraction().get::<ratio>();

        println!(
            "fuelled zone {fuelled_zone_cm3} cm^3, particle {particle_cm3:e} cm^3, \
             total particle volume {} cm^3, volume fraction {measured}",
            8335.0 * particle_cm3
        );
        assert_relative_eq!(hand_fraction, measured, max_relative = 1e-12);
        assert!(measured < 0.2, "HTR-10 dispersion should be dilute");
    }

    /// V&V test: both dispersion models stay inside the Wiener bounds, tend to
    /// the matrix value as the inclusion fraction vanishes, and agree with each
    /// other in the dilute limit.
    ///
    /// **Methodology.** Sweep the particle-to-matrix conductivity ratio
    /// `kappa` over `{0.01, 0.077, 0.5, 1, 2, 10, 100}` and the volume
    /// fraction `phi` over `{0, 0.01, 0.05, 0.1, 0.2, 0.4, 0.6}`, with a
    /// matrix conductivity of 30 W/(m K) — 49 combinations per model. For each,
    /// form the series (Wiener lower) bound `1 / (phi/k_p + (1-phi)/k_m)` and
    /// the parallel (Wiener upper) bound `phi k_p + (1-phi) k_m`, and:
    ///
    /// - require **Maxwell-Eucken** to lie strictly inside both bounds (to
    ///   1e-12 relative), to return the matrix conductivity *exactly* at
    ///   `phi = 0`, and to return it to 1e-12 relative at `kappa = 1`;
    /// - for **Chiew-Glandt**, *measure* the largest excursion outside either
    ///   bound rather than assuming there is none, and require that excursion
    ///   to stay below `0.05 phi^3` — the magnitude of the empirical
    ///   third-order term, which is the only part of the expression that can
    ///   push it out (see the artefact note on
    ///   [`DispersionModel::ChiewGlandt`]).
    ///
    /// Separately, at the HTR-10 volume fraction 0.0502 and `kappa = 0.077`,
    /// measure the relative difference between the two models.
    ///
    /// **Results (2026-08-11):** Maxwell-Eucken satisfied both bounds in all
    /// 49 combinations with no exception. Chiew-Glandt satisfied them too
    /// except near `beta = 0`, where its `0.05 phi^3` term carries it above the
    /// parallel bound; the largest measured excursion was
    /// **1.0799999999999936e-2 relative at `kappa = 1`, `phi = 0.6`**, which
    /// is `0.05 phi^3 = 1.08e-2` to f64 roundoff — the artefact is exactly the
    /// third-order term and nothing more. Both models returned exactly
    /// 30 W/(m K) at `phi = 0`. At the HTR-10 point (`phi` = 0.050248,
    /// `kappa` = 0.077) the two models gave Maxwell-Eucken
    /// **28.034271712345575 W/(m K)** and Chiew-Glandt
    /// **28.024585513410482 W/(m K)** — a relative difference of
    /// **3.455e-4**, i.e. the higher-order Chiew terms move the answer by
    /// 0.035% at this dilution.
    ///
    /// **Interpretation:** the choice between the two models shifts the
    /// fuelled-zone conductivity by 0.035% for HTR-10 — immaterial beside the
    /// uncertainty in the underlying graphite and TRISO correlations, and
    /// beside the 6.6% the dispersion itself costs. Both would begin to matter
    /// for a denser packing — HTR-PM's 0.09, or a pebble designed for higher
    /// loading. The bounds check is a
    /// necessary condition only: passing it does not confirm the Chiew
    /// coefficients, which carry the transcription caveat recorded on
    /// [`DispersionModel::ChiewGlandt`].
    #[test]
    fn dispersion_models_respect_the_wiener_bounds() {
        let k_matrix = ThermalConductivity::new::<watt_per_meter_kelvin>(30.0);
        let k_matrix_value = 30.0;

        let mut worst_chiew_excursion: f64 = 0.0;
        let mut worst_chiew_case = (0.0, 0.0);

        for kappa in [0.01, 0.077, 0.5, 1.0, 2.0, 10.0, 100.0] {
            let k_particle_value = kappa * k_matrix_value;
            let k_particle = ThermalConductivity::new::<watt_per_meter_kelvin>(k_particle_value);

            for phi in [0.0, 0.01, 0.05, 0.1, 0.2, 0.4, 0.6] {
                let series_bound = 1.0 / (phi / k_particle_value + (1.0 - phi) / k_matrix_value);
                let parallel_bound = phi * k_particle_value + (1.0 - phi) * k_matrix_value;

                for model in [DispersionModel::MaxwellEucken, DispersionModel::ChiewGlandt] {
                    let effective = model
                        .effective_conductivity(k_matrix, k_particle, Ratio::new::<ratio>(phi))
                        .unwrap()
                        .get::<watt_per_meter_kelvin>();

                    let below = (series_bound - effective).max(0.0) / series_bound;
                    let above = (effective - parallel_bound).max(0.0) / parallel_bound;
                    let excursion = below.max(above);

                    match model {
                        DispersionModel::MaxwellEucken => {
                            assert!(
                                excursion < 1e-12,
                                "Maxwell-Eucken at kappa={kappa}, phi={phi} gave \
                                 {effective}, outside the Wiener bounds \
                                 [{series_bound}, {parallel_bound}]"
                            );
                            if kappa == 1.0 {
                                assert_relative_eq!(
                                    k_matrix_value,
                                    effective,
                                    max_relative = 1e-12
                                );
                            }
                        }
                        DispersionModel::ChiewGlandt => {
                            let artefact_bound = 0.05 * phi * phi * phi + 1e-12;
                            assert!(
                                excursion <= artefact_bound,
                                "Chiew-Glandt at kappa={kappa}, phi={phi} strayed \
                                 {excursion:e} outside the Wiener bounds, more than \
                                 the 0.05 phi^3 = {artefact_bound:e} third-order term \
                                 can explain"
                            );
                            if excursion > worst_chiew_excursion {
                                worst_chiew_excursion = excursion;
                                worst_chiew_case = (kappa, phi);
                            }
                        }
                    }

                    if phi == 0.0 {
                        assert_eq!(effective, k_matrix_value);
                    }
                }
            }
        }
        println!(
            "largest Chiew-Glandt excursion outside the Wiener bounds: {:e} at \
             kappa={}, phi={} (0.05 phi^3 = {:e})",
            worst_chiew_excursion,
            worst_chiew_case.0,
            worst_chiew_case.1,
            0.05 * worst_chiew_case.1.powi(3)
        );

        // the two models in the HTR-10 dilute limit
        let phi = Ratio::new::<ratio>(0.05024658);
        let k_particle = ThermalConductivity::new::<watt_per_meter_kelvin>(0.077 * 30.0);
        let maxwell = DispersionModel::MaxwellEucken
            .effective_conductivity(k_matrix, k_particle, phi)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let chiew = DispersionModel::ChiewGlandt
            .effective_conductivity(k_matrix, k_particle, phi)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        println!(
            "at the HTR-10 point: Maxwell-Eucken {maxwell} W/(m K), \
             Chiew-Glandt {chiew} W/(m K), relative difference {:e}",
            ((chiew - maxwell) / maxwell).abs()
        );
        assert!(((chiew - maxwell) / maxwell).abs() < 1e-3);
    }

    /// V&V test: the fuelled-zone conductivity sits below the matrix value and
    /// tends to it as the particle loading vanishes.
    ///
    /// **Methodology:** at 1000 K and zero fluence, compute
    /// [`Pebble::fuelled_zone_conductivity`] for [`Pebble::htr10`] and compare
    /// with [`Pebble::matrix_conductivity`] at the same conditions, requiring
    /// the fuelled zone to be the *less* conductive of the two (the coated
    /// particle being far less conductive than graphite). Then set the
    /// particle count to zero and require the fuelled-zone conductivity to
    /// equal the matrix conductivity exactly.
    ///
    /// **Results (2026-08-11):** at 1000 K the A3 matrix graphite measured
    /// **30.23707573873239 W/(m K)** and the dispersed TRISO particle
    /// **2.327918923706458 W/(m K)** (from level 1) at a volume fraction of
    /// 0.050248, giving a fuelled-zone effective conductivity of
    /// **28.245956409767754 W/(m K)** — a **6.59%** reduction against plain
    /// matrix graphite. With the particle count set to zero the fuelled-zone
    /// conductivity returned exactly the matrix value,
    /// 30.23707573873239 W/(m K).
    ///
    /// **Interpretation:** the TRISO loading costs about 6.6% of the fuelled
    /// zone's conductivity at HTR-10 dilution. Homogenising the particles into
    /// the graphite would keep this 6% but discard the ~6.5 K kernel-to-matrix
    /// rise that level 1 supplies — which is precisely the double-heterogeneity
    /// effect this module's design note is about.
    #[test]
    fn fuelled_zone_conductivity_is_below_the_matrix_value() {
        let pebble = Pebble::htr10();
        let temperature = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let fresh = Ratio::new::<ratio>(0.0);

        let matrix = pebble
            .matrix_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let particle = pebble
            .particle
            .effective_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let fuelled = pebble
            .fuelled_zone_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();

        println!(
            "at 1000 K: matrix {matrix} W/(m K), TRISO particle {particle} W/(m K) \
             at volume fraction {}, fuelled zone {fuelled} W/(m K) \
             ({:.2}% below matrix)",
            pebble.triso_volume_fraction().get::<ratio>(),
            100.0 * (matrix - fuelled) / matrix
        );

        assert!(particle < matrix);
        assert!(fuelled < matrix);
        assert!(fuelled > particle);

        // no particles: the fuelled zone is just matrix graphite
        let unloaded = Pebble::new(
            pebble.outer_radius,
            pebble.fuelled_zone_radius,
            pebble.particle,
            0.0,
            pebble.dispersion_model,
        )
        .unwrap();
        let unloaded_conductivity = unloaded
            .fuelled_zone_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        println!("with zero particles: {unloaded_conductivity} W/(m K)");
        assert_eq!(unloaded_conductivity, matrix);
    }

    /// V&V test: with a uniform conductivity and no unfuelled shell, the
    /// two-zone pebble solution collapses onto the textbook uniformly
    /// generating sphere, `T(0) - T(R) = q''' R^2 / (6 k)`.
    ///
    /// **Methodology:** the two-zone solution is
    /// `T(0) - T(R) = Q/(8 pi k_f a) + Q/(4 pi k_g) (1/a - 1/R)`. Setting
    /// `a = R` kills the second term and leaves `Q / (8 pi k R)`, which is
    /// algebraically identical to `q''' R^2 / (6 k)` for
    /// `q''' = Q / ((4/3) pi R^3)`. The test builds that degenerate geometry
    /// directly from the module's own
    /// [`solid_sphere_centre_temperature_rise`] and
    /// [`spherical_shell_temperature_rise`] at `Q = 370.37 W`, `R = 0.03 m`,
    /// `k = 30 W/(m K)`, and compares against `q''' R^2 / (6 k)` computed from
    /// the volumetric generation rate. Pass criterion: 1e-12 relative. It also
    /// confirms the shell term vanishes identically when `a = R`.
    ///
    /// **Results (2026-08-11):** volumetric generation
    /// 3274793.0677344725 W/m^3 in a 0.03 m sphere at 30 W/(m K); measured
    /// rise **16.373965338672363 K**, analytic `q''' R^2 / (6 k)`
    /// **16.37396533867236 K**, relative difference **2.2e-16** (one ulp of
    /// f64). The `a = R` shell term measured exactly 0 K, as it must.
    /// Algebraic verification only — no physical data involved.
    #[test]
    fn degenerate_pebble_collapses_to_the_analytic_sphere() {
        let power = Power::new::<watt>(10.0e6 / 27_000.0);
        let radius = Length::new::<centimeter>(3.0);
        let k = ThermalConductivity::new::<watt_per_meter_kelvin>(30.0);

        let shell_term = spherical_shell_temperature_rise(power, radius, radius, k);
        let centre_term = solid_sphere_centre_temperature_rise(power, radius, k);
        let measured = (shell_term + centre_term).value;

        let radius_m = radius.get::<meter>();
        let volumetric_generation =
            power.get::<watt>() / ((4.0 / 3.0) * PI * radius_m * radius_m * radius_m);
        let analytic =
            volumetric_generation * radius_m * radius_m / (6.0 * k.get::<watt_per_meter_kelvin>());

        println!(
            "q''' = {volumetric_generation} W/m^3; measured rise {measured} K, \
             analytic q'''R^2/(6k) {analytic} K, shell term {} K",
            shell_term.value
        );
        assert_eq!(shell_term.value, 0.0);
        assert_relative_eq!(analytic, measured, max_relative = 1e-12);
    }

    /// V&V test: zero power gives a uniform pebble, kernel included.
    ///
    /// **Methodology:** solve [`Pebble::steady_state_temperatures`] for
    /// [`Pebble::htr10`] at zero power with a 900 K surface temperature, and
    /// require the centre, the fuelled-zone boundary, the surface and the
    /// superposed peak kernel temperature to all equal 900 K exactly.
    ///
    /// **Results (2026-08-11):** all four temperatures returned exactly
    /// 900 K, and both [`PebbleTemperatureProfile::total_rise`] and
    /// [`PebbleTemperatureProfile::matrix_rise`] returned exactly 0 K —
    /// including through the nested level-1 call, confirming the superposition
    /// contributes nothing when there is nothing to conduct. Structural
    /// exactness check, not a physical measurement.
    #[test]
    fn zero_power_gives_a_uniform_pebble() {
        let pebble = Pebble::htr10();
        let surface = ThermodynamicTemperature::new::<kelvin>(900.0);

        let profile = pebble
            .steady_state_temperatures(Power::new::<watt>(0.0), surface, Ratio::new::<ratio>(0.0))
            .unwrap();

        println!("zero-power pebble profile: {profile:?}");
        for node in [
            profile.centre,
            profile.fuelled_zone_boundary,
            profile.surface,
            profile.peak_kernel_centre,
        ] {
            assert_eq!(node.get::<kelvin>(), 900.0);
        }
        assert_eq!(profile.total_rise().value, 0.0);
        assert_eq!(profile.matrix_rise().value, 0.0);
    }

    /// V&V test: the HTR-10 pebble's steady temperature rise at core-average
    /// power, with the nested levels resolved separately.
    ///
    /// **Methodology:** drive [`Pebble::htr10`] at the core-average pebble
    /// power — 10 MW / 27 000 fuel elements = 370.37 W, both figures from
    /// IAEA-TECDOC-1382 part 2, Chapter 4 — with a 1000 K surface temperature
    /// and zero fluence. Record the pebble centre, the fuelled-zone boundary,
    /// the peak kernel temperature, and the split between the matrix rise and
    /// the particle rise. Pass criteria: the profile decreases outward; the
    /// peak kernel is hotter than the pebble centre (the superposition must
    /// add something); and the total rise is under 200 K, which any pebble
    /// design that meets a fuel-temperature limit must satisfy at nominal
    /// power.
    ///
    /// **Results (2026-08-11):** pebble power 370.3703703703704 W. Measured:
    /// pebble centre **1027.6100 K**, fuelled-zone boundary **1006.5119 K**,
    /// surface 1000 K, peak kernel centre **1034.2539 K**. Split: unfuelled
    /// graphite shell 6.5119 K (23.6% of the matrix rise), fuelled zone
    /// 21.0981 K (76.4%), giving a **matrix rise of 27.6100 K**; the
    /// superposed coated particle adds a further **6.6439 K**, for a **total
    /// surface-to-kernel rise of 34.2539 K**.
    ///
    /// **Interpretation:** at core-average power the fuel kernel runs about
    /// 34 K above the pebble surface, of which **19.4%** comes from inside the
    /// coated particle — the part a homogenised fuelled zone would discard
    /// entirely. The particle contribution here (6.6439 K) is slightly larger
    /// than the standalone level-1 figure (6.5264 K at a 1000 K boundary)
    /// because the particle sits at the hotter pebble centre, where its layer
    /// conductivities are lower. Nothing here is compared against an HTR-10
    /// measurement; peak-power pebbles and irradiated fuel would both run
    /// hotter than this core-average, fresh-fuel case.
    #[test]
    fn htr10_pebble_steady_state_at_core_average_power() {
        let pebble = Pebble::htr10();
        let power = Power::new::<watt>(10.0e6 / 27_000.0);
        let surface = ThermodynamicTemperature::new::<kelvin>(1000.0);

        println!("pebble power: {} W", power.get::<watt>());

        let profile = pebble
            .steady_state_temperatures(power, surface, Ratio::new::<ratio>(0.0))
            .unwrap();

        println!(
            "pebble centre {:.4} K, fuelled-zone boundary {:.4} K, surface {:.4} K, \
             peak kernel centre {:.4} K",
            profile.centre.get::<kelvin>(),
            profile.fuelled_zone_boundary.get::<kelvin>(),
            profile.surface.get::<kelvin>(),
            profile.peak_kernel_centre.get::<kelvin>()
        );

        let shell_rise =
            super::super::temperature_difference(profile.fuelled_zone_boundary, profile.surface);
        let fuelled_rise =
            super::super::temperature_difference(profile.centre, profile.fuelled_zone_boundary);
        let particle_rise = profile.hottest_particle.total_rise();

        println!(
            "shell {:.4} K, fuelled zone {:.4} K, matrix total {:.4} K, \
             particle {:.4} K, grand total {:.4} K (particle share {:.1}%)",
            shell_rise.value,
            fuelled_rise.value,
            profile.matrix_rise().value,
            particle_rise.value,
            profile.total_rise().value,
            100.0 * particle_rise.value / profile.total_rise().value
        );

        assert!(profile.centre > profile.fuelled_zone_boundary);
        assert!(profile.fuelled_zone_boundary > profile.surface);
        assert!(
            profile.peak_kernel_centre > profile.centre,
            "the superposed particle must add a rise on top of the matrix"
        );
        assert!(profile.total_rise().value < 200.0);
    }

    /// V&V test: invalid pebble geometry is rejected.
    ///
    /// **Methodology:** call [`Pebble::new`] with (a) a fuelled-zone radius
    /// larger than the pebble, (b) a negative particle count, and (c) a
    /// particle count so large that the particles would exceed the fuelled
    /// zone's volume, requiring [`TampinesError::InvalidInput`] in each case.
    /// Also require an out-of-range enrichment to be rejected by
    /// [`uranium_dioxide_heavy_metal_fraction`].
    ///
    /// **Results (2026-08-11):** all four invalid inputs returned
    /// `InvalidInput`, the over-packed case reporting the offending volume
    /// fraction (2.4e0 for 4e5 particles). Input-validation check only.
    #[test]
    fn invalid_pebble_geometry_is_rejected() {
        let particle = TrisoParticle::htr10();
        let outer = Length::new::<centimeter>(3.0);
        let fuelled = Length::new::<centimeter>(2.5);

        let inverted = Pebble::new(
            fuelled,
            outer,
            particle,
            8335.0,
            DispersionModel::ChiewGlandt,
        );
        println!("fuelled zone larger than pebble: {inverted:?}");
        assert!(matches!(inverted, Err(TampinesError::InvalidInput(_))));

        let negative_count =
            Pebble::new(outer, fuelled, particle, -1.0, DispersionModel::ChiewGlandt);
        println!("negative particle count: {negative_count:?}");
        assert!(matches!(
            negative_count,
            Err(TampinesError::InvalidInput(_))
        ));

        let over_packed = Pebble::new(
            outer,
            fuelled,
            particle,
            4.0e5,
            DispersionModel::ChiewGlandt,
        );
        println!("over-packed fuelled zone: {over_packed:?}");
        assert!(matches!(over_packed, Err(TampinesError::InvalidInput(_))));

        let bad_enrichment = uranium_dioxide_heavy_metal_fraction(Ratio::new::<ratio>(1.5));
        println!("enrichment above 1: {bad_enrichment:?}");
        assert!(matches!(
            bad_enrichment,
            Err(TampinesError::InvalidInput(_))
        ));
    }
}
