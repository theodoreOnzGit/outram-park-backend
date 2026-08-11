//! # TRISO coated-particle conduction — the innermost of the nested scales
//!
//! Steady-state radial heat conduction through the five concentric regions of
//! a TRISO coated fuel particle:
//!
//! | Region | Material | Role |
//! |---|---|---|
//! | Kernel | UO2 | where the fission power is released |
//! | Buffer | porous pyrolytic carbon | accommodates fission gas and kernel swelling |
//! | IPyC | dense pyrolytic carbon | seals the kernel, protects the SiC from fission products |
//! | SiC | silicon carbide | the pressure-bearing, metallic-fission-product barrier |
//! | OPyC | dense pyrolytic carbon | protects the SiC mechanically |
//!
//! All the fission power is generated in the kernel and conducts outward, so
//! every coating layer carries the *whole* particle power through a pure
//! series resistance. The temperature field is therefore closed-form, layer by
//! layer (see [`spherical_shell_temperature_rise`] and
//! [`solid_sphere_centre_temperature_rise`]) — no discretisation is needed,
//! and none is used.
//!
//! ## Where this sits in the nest
//!
//! This module is level 1 of three. Its
//! [`TrisoParticle::effective_conductivity`] is the *input* to level 2
//! ([`super::pebble`], which disperses these particles in matrix graphite),
//! whose pebble-surface result is in turn the input to level 3
//! ([`super::cht`], the bed-to-helium coupling) and to the bed effective
//! conductivity in [`super::zbs`].
//!
//! ## Geometry: reuse of `boon-lay`, not a second copy
//!
//! `boon-lay` already owns a five-layer TRISO CSG cell
//! (`TrisoCell`) with `uom`-typed concentric radii, built for its Lagrangian
//! fission-product diffusion model. This module **reuses that geometry** —
//! [`TrisoParticle::from_boon_lay_cell`] and
//! [`TrisoParticle::to_boon_lay_cell`] convert both ways — rather than
//! defining a rival geometry type. The dependency edge is maintainer-approved
//! (2026-08-11, `op-jyyp.5`) and declared in `Cargo.toml`.
//!
//! **What is deliberately NOT consumed from `boon-lay`:** its fission-product
//! *release* model. Bead `op-jyyp.10` records that model's CRP-6 verification
//! test as defective — it wraps the reference assertion in `catch_unwind` and
//! discards the result, so it verifies nothing. Only geometry and the
//! per-layer property *pattern* are reused here; the conduction physics below
//! is this module's own.
//!
//! ## Property provenance
//!
//! Layer conductivities are transcribed from the **Virtual Test Bed** HTR-PM
//! pebble model, vendored in this workspace at
//! `reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`
//! (CC-BY-4.0, **Open tier**), `[Functions]` block, lines 165-197: `uo2_k`,
//! `buffer_k`, `pyc_k`, `sic_k`. The deck names no upstream literature source
//! for any of them. The fast-neutron-fluence damage factor shared by the
//! carbon layers is *not* re-implemented here — it is called from
//! [`tuas_boussinesq_solver`]'s already-tested
//! `nuclear_graphite_fluence_damage_factor`, which transcribes the same deck
//! expression.
//!
//! Geometry for [`TrisoParticle::htr10`] comes from **IAEA-TECDOC-1382 part 2,
//! Chapter 4** (Open tier; catalogued at
//! `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.pdf`),
//! Table 4-2 / Table 4-17: kernel radius 0.025 cm, UO2 density 10.4 g/cm^3,
//! coating layers PyC/PyC/SiC/PyC of thickness 0.009/0.004/0.0035/0.004 cm and
//! density 1.1/1.9/3.18/1.9 g/cm^3.
//!
//! ## Status
//!
//! **NOT VALIDATED.** The conduction solution is verified against analytic
//! limits (see the tests at the bottom of this file) and the property
//! correlations are verified as transcriptions, but nothing here has been
//! compared against a TRISO temperature measurement. AI-assisted draft pending
//! human review per `RESPONSIBLE_USE.md`.
//!
//! **Belongs here:** particle-scale geometry, per-layer conductivity, and the
//! particle's own steady conduction solution. **Does not belong here:**
//! fission-product diffusion or release (that is `boon-lay`'s), the matrix
//! graphite dispersion (level 2, [`super::pebble`]), or anything about the
//! bed.

use std::f64::consts::PI;

use uom::si::f64::{
    Length, MassDensity, Power, Ratio, TemperatureInterval, ThermalConductivity,
    ThermodynamicTemperature, Volume,
};
use uom::si::length::{centimeter, meter};
use uom::si::mass_density::{gram_per_cubic_centimeter, kilogram_per_cubic_meter};
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::solid_database::nuclear_graphite::nuclear_graphite_fluence_damage_factor;

use crate::TampinesError;

/// Fast-neutron fluence, expressed as the dimensionless deck parameter `gam`
/// in units of **10^25 n/m^2 (E > 0.1 MeV)**.
///
/// This is the same quantity, with the same unit interpretation, that
/// [`tuas_boussinesq_solver`]'s `nuclear_graphite_fluence_damage_factor`
/// takes; see that function's documentation for why the interpretation is an
/// *interpretation* (the Virtual Test Bed deck declares no unit for `gam`).
/// Valid range is `[0, 15]`; `Ratio::new::<ratio>(0.0)` means fresh,
/// unirradiated fuel.
pub type FastNeutronFluence = Ratio;

/// Lowest temperature, 300 K, at which the layer conductivity correlations of
/// this module are evaluated.
///
/// The Virtual Test Bed deck states no validity range for any of them; 300 K
/// is adopted to match the window
/// [`tuas_boussinesq_solver`]'s nuclear-graphite correlations already enforce,
/// so every property in the pebble-bed stack shares one coded window.
pub const MIN_TEMPERATURE_KELVIN: f64 = 300.0;

/// Highest temperature, 2000 K, at which the layer conductivity correlations
/// of this module are evaluated. See [`MIN_TEMPERATURE_KELVIN`] for why this
/// window was chosen.
pub const MAX_TEMPERATURE_KELVIN: f64 = 2000.0;

/// Theoretical (pore-free) density of carbon, 1930 kg/m^3, used as the
/// reference density in the pyrocarbon and buffer conductivity porosity
/// factors.
///
/// Source: the constant `1930.` appearing in `buffer_k` and `pyc_k` of the
/// Virtual Test Bed HTR-PM pebble deck (`pebble_triso.i`, lines 178-186,
/// CC-BY-4.0, Open tier). The deck names no upstream source.
pub const THEORETICAL_CARBON_DENSITY_KG_PER_M3: f64 = 1930.0;

/// The five concentric material regions of a TRISO coated fuel particle,
/// innermost first.
///
/// A closed set — enum dispatch, no trait objects, per the workspace Rust
/// design rules. `InnerPyC` and `OuterPyC` are distinct variants even though
/// they share one conductivity correlation, because they occupy different
/// radii and therefore different series resistances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrisoLayer {
    /// The UO2 fuel kernel — the only region in which fission power is
    /// released.
    Kernel,
    /// The porous (low-density) pyrolytic carbon buffer.
    Buffer,
    /// The inner dense pyrolytic carbon layer.
    InnerPyC,
    /// The silicon carbide pressure-bearing layer.
    SiliconCarbide,
    /// The outer dense pyrolytic carbon layer.
    OuterPyC,
}

impl TrisoLayer {
    /// All five layers, innermost first — for iteration over the stack.
    pub const ALL: [TrisoLayer; 5] = [
        TrisoLayer::Kernel,
        TrisoLayer::Buffer,
        TrisoLayer::InnerPyC,
        TrisoLayer::SiliconCarbide,
        TrisoLayer::OuterPyC,
    ];
}

/// The concentric geometry and coating densities of one TRISO coated fuel
/// particle.
///
/// Plain data: the radii are *outer* radii of each layer, strictly increasing
/// outward, in metres; the two densities set the porosity factor of the carbon
/// conductivity correlations. The physics lives in
/// [`TrisoParticle::steady_state_temperatures`] and
/// [`TrisoParticle::effective_conductivity`].
///
/// Construct with [`TrisoParticle::new`] (checked), [`TrisoParticle::htr10`]
/// (the cited HTR-10 particle), or [`TrisoParticle::from_boon_lay_cell`]
/// (reusing a `boon-lay` CSG cell).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrisoParticle {
    /// Outer radius of the UO2 kernel, metres. HTR-10: 0.025 cm = 250 um.
    pub kernel_radius: Length,
    /// Outer radius of the porous carbon buffer, metres. HTR-10:
    /// 250 + 90 = 340 um.
    pub buffer_outer_radius: Length,
    /// Outer radius of the inner pyrolytic carbon layer, metres. HTR-10:
    /// 340 + 40 = 380 um.
    pub inner_pyc_outer_radius: Length,
    /// Outer radius of the silicon carbide layer, metres. HTR-10:
    /// 380 + 35 = 415 um.
    pub silicon_carbide_outer_radius: Length,
    /// Outer radius of the outer pyrolytic carbon layer — the particle's own
    /// outer surface, metres. HTR-10: 415 + 40 = 455 um.
    pub outer_pyc_outer_radius: Length,
    /// Mass density of the porous carbon buffer, kg/m^3. HTR-10:
    /// 1100 kg/m^3 (1.1 g/cm^3, IAEA-TECDOC-1382 part 2 Table 4-17). Enters
    /// the buffer conductivity only through its porosity factor.
    pub buffer_density: MassDensity,
    /// Mass density of the dense pyrolytic carbon layers (IPyC and OPyC share
    /// one value), kg/m^3. HTR-10: 1900 kg/m^3 (1.9 g/cm^3, same table).
    pub pyrocarbon_density: MassDensity,
}

impl TrisoParticle {
    /// Builds a TRISO particle from its five outer radii and the two carbon
    /// densities, checking that the radii strictly increase outward and that
    /// every input is positive.
    ///
    /// All lengths are `uom` `Length` (metres in SI); densities are `uom`
    /// `MassDensity` (kg/m^3). Returns
    /// [`TampinesError::InvalidInput`] if the geometry is not a valid nest of
    /// concentric shells.
    pub fn new(
        kernel_radius: Length,
        buffer_outer_radius: Length,
        inner_pyc_outer_radius: Length,
        silicon_carbide_outer_radius: Length,
        outer_pyc_outer_radius: Length,
        buffer_density: MassDensity,
        pyrocarbon_density: MassDensity,
    ) -> Result<Self, TampinesError> {
        let radii = [
            ("kernel", kernel_radius),
            ("buffer", buffer_outer_radius),
            ("IPyC", inner_pyc_outer_radius),
            ("SiC", silicon_carbide_outer_radius),
            ("OPyC", outer_pyc_outer_radius),
        ];

        if radii[0].1.get::<meter>() <= 0.0 {
            return Err(TampinesError::InvalidInput(
                "TRISO kernel radius must be strictly positive".to_string(),
            ));
        }

        for window in radii.windows(2) {
            if window[1].1 <= window[0].1 {
                return Err(TampinesError::InvalidInput(format!(
                    "TRISO layer radii must strictly increase outward, but the \
                     {} outer radius ({:.6e} m) is not greater than the {} outer \
                     radius ({:.6e} m)",
                    window[1].0,
                    window[1].1.get::<meter>(),
                    window[0].0,
                    window[0].1.get::<meter>(),
                )));
            }
        }

        for (name, density) in [
            ("buffer", buffer_density),
            ("pyrocarbon", pyrocarbon_density),
        ] {
            let value = density.get::<kilogram_per_cubic_meter>();
            if !(value > 0.0 && value < THEORETICAL_CARBON_DENSITY_KG_PER_M3) {
                return Err(TampinesError::InvalidInput(format!(
                    "TRISO {name} density must lie strictly between 0 and the \
                     theoretical carbon density {THEORETICAL_CARBON_DENSITY_KG_PER_M3} \
                     kg/m^3, got {value} kg/m^3"
                )));
            }
        }

        Ok(Self {
            kernel_radius,
            buffer_outer_radius,
            inner_pyc_outer_radius,
            silicon_carbide_outer_radius,
            outer_pyc_outer_radius,
            buffer_density,
            pyrocarbon_density,
        })
    }

    /// The HTR-10 coated fuel particle, transcribed from **IAEA-TECDOC-1382
    /// part 2, Chapter 4, Table 4-17** (Open tier).
    ///
    /// Kernel radius 0.025 cm; coating layers, starting from the kernel,
    /// PyC (buffer) / PyC (IPyC) / SiC / PyC (OPyC) of thickness
    /// 0.009 / 0.004 / 0.0035 / 0.004 cm and density
    /// 1.1 / 1.9 / 3.18 / 1.9 g/cm^3. The cumulative outer radii are
    /// therefore 250, 340, 380, 415 and 455 micrometres. Only the two carbon
    /// densities are stored — the SiC density does not enter its conductivity
    /// correlation.
    ///
    /// Panics only if that published geometry were internally inconsistent,
    /// which the constructor's checks and the unit tests both rule out.
    pub fn htr10() -> Self {
        let kernel_radius = Length::new::<centimeter>(0.025);
        let buffer_outer_radius = kernel_radius + Length::new::<centimeter>(0.009);
        let inner_pyc_outer_radius = buffer_outer_radius + Length::new::<centimeter>(0.004);
        let silicon_carbide_outer_radius =
            inner_pyc_outer_radius + Length::new::<centimeter>(0.0035);
        let outer_pyc_outer_radius =
            silicon_carbide_outer_radius + Length::new::<centimeter>(0.004);

        Self::new(
            kernel_radius,
            buffer_outer_radius,
            inner_pyc_outer_radius,
            silicon_carbide_outer_radius,
            outer_pyc_outer_radius,
            MassDensity::new::<gram_per_cubic_centimeter>(1.1),
            MassDensity::new::<gram_per_cubic_centimeter>(1.9),
        )
        .expect("the published HTR-10 TRISO geometry is a valid concentric nest")
    }

    /// Builds a [`TrisoParticle`] from a `boon-lay` `TrisoCell`, reusing that
    /// crate's five-layer CSG geometry rather than duplicating it.
    ///
    /// The two carbon densities are supplied by the caller because
    /// `boon-lay`'s cell carries geometry, temperatures and fluence but no
    /// densities. Returns [`TampinesError::InvalidInput`] if the cell's radii
    /// do not form a valid concentric nest (they cannot, in practice —
    /// `TrisoCell::new` asserts the same ordering).
    pub fn from_boon_lay_cell(
        cell: &boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell,
        buffer_density: MassDensity,
        pyrocarbon_density: MassDensity,
    ) -> Result<Self, TampinesError> {
        Self::new(
            cell.get_fuel_radius(),
            cell.get_buffer_radius(),
            cell.get_ipyc_radius(),
            cell.get_sic_radius(),
            cell.get_opyc_radius(),
            buffer_density,
            pyrocarbon_density,
        )
    }

    /// Converts this particle's geometry into a `boon-lay` `TrisoCell`, so the
    /// same particle can be handed to that crate's Lagrangian fission-product
    /// diffusion model.
    ///
    /// The returned cell carries `boon-lay`'s own default uniform temperature
    /// and zero fluence; set them there if they matter. Only the geometry
    /// crosses the boundary — this is not a route into `boon-lay`'s release
    /// model, which `op-jyyp.10` flags as unverified.
    pub fn to_boon_lay_cell(
        &self,
    ) -> boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell {
        boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell::new(
            self.kernel_radius,
            self.buffer_outer_radius,
            self.inner_pyc_outer_radius,
            self.silicon_carbide_outer_radius,
            self.outer_pyc_outer_radius,
        )
    }

    /// Outer radius of the given layer, metres.
    pub fn layer_outer_radius(&self, layer: TrisoLayer) -> Length {
        match layer {
            TrisoLayer::Kernel => self.kernel_radius,
            TrisoLayer::Buffer => self.buffer_outer_radius,
            TrisoLayer::InnerPyC => self.inner_pyc_outer_radius,
            TrisoLayer::SiliconCarbide => self.silicon_carbide_outer_radius,
            TrisoLayer::OuterPyC => self.outer_pyc_outer_radius,
        }
    }

    /// Inner radius of the given layer, metres. Zero for the kernel, which is
    /// a solid sphere rather than a shell.
    pub fn layer_inner_radius(&self, layer: TrisoLayer) -> Length {
        match layer {
            TrisoLayer::Kernel => Length::new::<meter>(0.0),
            TrisoLayer::Buffer => self.kernel_radius,
            TrisoLayer::InnerPyC => self.buffer_outer_radius,
            TrisoLayer::SiliconCarbide => self.inner_pyc_outer_radius,
            TrisoLayer::OuterPyC => self.silicon_carbide_outer_radius,
        }
    }

    /// Volume of the given layer, m^3 — the shell volume
    /// `(4/3) pi (r_outer^3 - r_inner^3)`.
    pub fn layer_volume(&self, layer: TrisoLayer) -> Volume {
        let r_outer = self.layer_outer_radius(layer);
        let r_inner = self.layer_inner_radius(layer);
        (4.0 / 3.0) * PI * (r_outer * r_outer * r_outer - r_inner * r_inner * r_inner)
    }

    /// Total volume of the whole coated particle, m^3, out to the OPyC
    /// surface. HTR-10: about 3.946e-4 cm^3 (radius 455 um).
    pub fn particle_volume(&self) -> Volume {
        (4.0 / 3.0)
            * PI
            * (self.outer_pyc_outer_radius
                * self.outer_pyc_outer_radius
                * self.outer_pyc_outer_radius)
    }

    /// Volume fraction of the given layer within the whole particle,
    /// dimensionless. The five fractions sum to exactly one.
    pub fn layer_volume_fraction(&self, layer: TrisoLayer) -> Ratio {
        self.layer_volume(layer) / self.particle_volume()
    }

    /// Thermal conductivity of one layer, W/(m K), at the given temperature
    /// and fast-neutron fluence.
    ///
    /// Dispatches to the free functions of this module:
    /// [`uranium_dioxide_thermal_conductivity`] for the kernel,
    /// [`buffer_carbon_thermal_conductivity`] for the buffer,
    /// [`pyrocarbon_thermal_conductivity`] for IPyC and OPyC (which share one
    /// correlation and one density), and
    /// [`silicon_carbide_thermal_conductivity`] for the SiC.
    ///
    /// Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`;
    /// outside either, returns [`TampinesError::InvalidInput`].
    pub fn layer_thermal_conductivity(
        &self,
        layer: TrisoLayer,
        temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<ThermalConductivity, TampinesError> {
        match layer {
            TrisoLayer::Kernel => uranium_dioxide_thermal_conductivity(temperature),
            TrisoLayer::Buffer => {
                buffer_carbon_thermal_conductivity(temperature, self.buffer_density, fluence)
            }
            TrisoLayer::InnerPyC | TrisoLayer::OuterPyC => {
                pyrocarbon_thermal_conductivity(temperature, self.pyrocarbon_density, fluence)
            }
            TrisoLayer::SiliconCarbide => {
                silicon_carbide_thermal_conductivity(temperature, fluence)
            }
        }
    }

    /// Effective (homogenised) thermal conductivity of the whole coated
    /// particle, W/(m K), by **volume-fraction series mixing**:
    ///
    /// `1 / k_eff = sum_i ( f_i / k_i )`
    ///
    /// with `f_i` the layer volume fractions and `k_i` the layer
    /// conductivities, all evaluated at the one supplied temperature.
    ///
    /// This is the quantity level 2 ([`super::pebble`]) disperses in matrix
    /// graphite. It reproduces the mixing rule the Virtual Test Bed HTR-PM
    /// deck uses for its `triso` composite material
    /// (`pebble_triso.i` lines 226-231: `materials = 'kernel buffer ipyc sic
    /// opyc'`, `k_mixing = 'series'`; CC-BY-4.0, Open tier).
    ///
    /// **What this is and is not.** Series mixing is the **Wiener lower
    /// bound** on the conductivity of any two-or-more-phase composite: it
    /// assumes every phase carries the full heat flux in turn, which is
    /// exactly true for plane-parallel slabs and only approximately true for
    /// concentric shells (whose exact series resistance is weighted by
    /// `1/r_inner - 1/r_outer`, not by volume). It is used here because it is
    /// the reference implementation's choice and because it errs conservatively
    /// — a lower particle conductivity gives a *higher* predicted fuel
    /// temperature. The exact concentric-shell temperature field is available
    /// without this approximation from
    /// [`TrisoParticle::steady_state_temperatures`]; prefer that when the
    /// kernel temperature itself is the answer being sought.
    ///
    /// Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`.
    pub fn effective_conductivity(
        &self,
        temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<ThermalConductivity, TampinesError> {
        let mut inverse_sum: f64 = 0.0;

        for layer in TrisoLayer::ALL {
            let k = self
                .layer_thermal_conductivity(layer, temperature, fluence)?
                .get::<watt_per_meter_kelvin>();
            let fraction = self.layer_volume_fraction(layer).get::<ratio>();
            inverse_sum += fraction / k;
        }

        Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(
            1.0 / inverse_sum,
        ))
    }

    /// Steady-state radial temperature profile of the particle, given the
    /// fission power released in its kernel and the temperature imposed on its
    /// outer (OPyC) surface.
    ///
    /// **Physics.** All the power `Q` is generated uniformly in the kernel, so
    /// the kernel carries a parabolic profile and each coating shell carries
    /// the full `Q` through a series resistance:
    ///
    /// `T(0) - T(r_kernel) = Q / (8 pi k_kernel r_kernel)`
    ///
    /// `T(r_i) - T(r_{i+1}) = Q / (4 pi k_i) * (1/r_i - 1/r_{i+1})`
    ///
    /// Because every `k_i` depends on temperature, the profile is found by
    /// fixed-point iteration: each layer's conductivity is evaluated at that
    /// layer's current arithmetic-mean temperature, the profile is rebuilt
    /// inward from the surface, and the loop repeats until no node moves by
    /// more than 1e-9 K. Convergence is monotone and fast (single-digit
    /// iterations at HTR-10 particle powers); failing to converge in 200
    /// iterations returns [`TampinesError::Numerical`].
    ///
    /// **Inputs.** `power` is the fission power of *one* particle, watts
    /// (HTR-10 core average: 10 MW / 27 000 pebbles / 8335 particles per
    /// pebble = 0.0444 W).
    /// `surface_temperature` is the temperature of the OPyC outer surface,
    /// which in the nested stack is the local matrix-graphite temperature
    /// supplied by level 2. `fluence` is `gam` in units of 10^25 n/m^2.
    ///
    /// Returns [`TampinesError::InvalidInput`] for negative power or for a
    /// surface temperature outside 300 K to 2000 K, and propagates the same
    /// error if the iteration drives any layer temperature out of that window
    /// (which means the supplied power is unphysically large for the geometry).
    pub fn steady_state_temperatures(
        &self,
        power: Power,
        surface_temperature: ThermodynamicTemperature,
        fluence: FastNeutronFluence,
    ) -> Result<TrisoTemperatureProfile, TampinesError> {
        if power.value < 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "TRISO particle power must be non-negative, got {} W",
                power.value
            )));
        }
        check_temperature_range(surface_temperature, "TRISO surface temperature")?;

        // Node temperatures, outermost last:
        // [kernel centre, kernel surface, buffer outer, IPyC outer, SiC outer]
        // with the OPyC outer surface held at `surface_temperature`.
        let mut nodes = [surface_temperature; 5];

        let max_iterations = 200;
        let tolerance_kelvin = 1e-9;

        for iteration in 0..=max_iterations {
            // Conductivity of each layer at its current mean temperature.
            let kernel_mean = mean_temperature(nodes[0], nodes[1]);
            let buffer_mean = mean_temperature(nodes[1], nodes[2]);
            let inner_pyc_mean = mean_temperature(nodes[2], nodes[3]);
            let silicon_carbide_mean = mean_temperature(nodes[3], nodes[4]);
            let outer_pyc_mean = mean_temperature(nodes[4], surface_temperature);

            let k_kernel =
                self.layer_thermal_conductivity(TrisoLayer::Kernel, kernel_mean, fluence)?;
            let k_buffer =
                self.layer_thermal_conductivity(TrisoLayer::Buffer, buffer_mean, fluence)?;
            let k_inner_pyc =
                self.layer_thermal_conductivity(TrisoLayer::InnerPyC, inner_pyc_mean, fluence)?;
            let k_silicon_carbide = self.layer_thermal_conductivity(
                TrisoLayer::SiliconCarbide,
                silicon_carbide_mean,
                fluence,
            )?;
            let k_outer_pyc =
                self.layer_thermal_conductivity(TrisoLayer::OuterPyC, outer_pyc_mean, fluence)?;

            // Rebuild the profile inward from the imposed surface temperature.
            let sic_outer = surface_temperature
                + spherical_shell_temperature_rise(
                    power,
                    self.silicon_carbide_outer_radius,
                    self.outer_pyc_outer_radius,
                    k_outer_pyc,
                );
            let ipyc_outer = sic_outer
                + spherical_shell_temperature_rise(
                    power,
                    self.inner_pyc_outer_radius,
                    self.silicon_carbide_outer_radius,
                    k_silicon_carbide,
                );
            let buffer_outer = ipyc_outer
                + spherical_shell_temperature_rise(
                    power,
                    self.buffer_outer_radius,
                    self.inner_pyc_outer_radius,
                    k_inner_pyc,
                );
            let kernel_surface = buffer_outer
                + spherical_shell_temperature_rise(
                    power,
                    self.kernel_radius,
                    self.buffer_outer_radius,
                    k_buffer,
                );
            let kernel_centre = kernel_surface
                + solid_sphere_centre_temperature_rise(power, self.kernel_radius, k_kernel);

            let updated = [
                kernel_centre,
                kernel_surface,
                buffer_outer,
                ipyc_outer,
                sic_outer,
            ];

            let largest_move = nodes
                .iter()
                .zip(updated.iter())
                .map(|(old, new)| (new.get::<kelvin>() - old.get::<kelvin>()).abs())
                .fold(0.0_f64, f64::max);

            nodes = updated;

            if largest_move < tolerance_kelvin {
                return Ok(TrisoTemperatureProfile {
                    kernel_centre: nodes[0],
                    kernel_surface: nodes[1],
                    buffer_outer: nodes[2],
                    inner_pyc_outer: nodes[3],
                    silicon_carbide_outer: nodes[4],
                    particle_surface: surface_temperature,
                });
            }

            if iteration == max_iterations {
                return Err(TampinesError::Numerical(format!(
                    "TRISO steady-state temperature iteration did not converge in \
                     {max_iterations} iterations; last node movement was \
                     {largest_move:e} K"
                )));
            }
        }

        unreachable!("the loop above returns on convergence or on exhaustion")
    }
}

/// The steady radial temperature field of a TRISO particle, node by node from
/// the kernel centre outward.
///
/// Every field is an absolute temperature (`uom`
/// `ThermodynamicTemperature`, kelvin in SI), not a rise. Produced by
/// [`TrisoParticle::steady_state_temperatures`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrisoTemperatureProfile {
    /// Temperature at the centre of the UO2 kernel — the hottest point in the
    /// particle, and the figure of merit for fuel-temperature limits.
    pub kernel_centre: ThermodynamicTemperature,
    /// Temperature at the kernel/buffer interface.
    pub kernel_surface: ThermodynamicTemperature,
    /// Temperature at the buffer/IPyC interface.
    pub buffer_outer: ThermodynamicTemperature,
    /// Temperature at the IPyC/SiC interface.
    pub inner_pyc_outer: ThermodynamicTemperature,
    /// Temperature at the SiC/OPyC interface — the SiC layer's own outer face,
    /// the temperature that governs its fission-product retention.
    pub silicon_carbide_outer: ThermodynamicTemperature,
    /// Temperature imposed on the OPyC outer surface (the model's boundary
    /// condition, returned unchanged for convenience).
    pub particle_surface: ThermodynamicTemperature,
}

impl TrisoTemperatureProfile {
    /// Total temperature rise across the particle, from the OPyC surface to
    /// the kernel centre, kelvin.
    pub fn total_rise(&self) -> TemperatureInterval {
        super::temperature_difference(self.kernel_centre, self.particle_surface)
    }
}

/// Centre-to-surface temperature rise of a solid sphere of radius `radius`
/// and uniform conductivity `conductivity` generating total power `power`
/// uniformly throughout its volume:
///
/// `T(0) - T(R) = q''' R^2 / (6 k) = Q / (8 pi k R)`
///
/// with `q''' = Q / ((4/3) pi R^3)`. The two forms are algebraically
/// identical; the second is evaluated here because total power is what a
/// particle or pebble model carries.
///
/// Units: `power` in watts, `radius` in metres, `conductivity` in W/(m K);
/// the result is a `uom` `TemperatureInterval` in kelvin. No range checking —
/// this is exact algebra, valid for any positive radius and conductivity.
pub fn solid_sphere_centre_temperature_rise(
    power: Power,
    radius: Length,
    conductivity: ThermalConductivity,
) -> TemperatureInterval {
    power / (8.0 * PI * conductivity * radius)
}

/// Temperature rise across a spherical shell of uniform conductivity carrying
/// a fixed total power through it:
///
/// `T(r_inner) - T(r_outer) = Q / (4 pi k) * (1/r_inner - 1/r_outer)`
///
/// This is the exact steady solution of Laplace's equation in a shell with no
/// internal generation — the case of every TRISO coating layer, which carries
/// the kernel's power but produces none of its own.
///
/// Units: `power` in watts, radii in metres, `conductivity` in W/(m K); the
/// result is a `uom` `TemperatureInterval` in kelvin. The reciprocal
/// difference is formed as `(r_outer - r_inner) / (r_inner r_outer)` so the
/// expression stays `uom`-typed throughout. Requires
/// `0 < r_inner < r_outer`; a caller violating that gets a negative or
/// infinite rise rather than an error, which is why the public entry points
/// validate geometry at construction instead.
pub fn spherical_shell_temperature_rise(
    power: Power,
    inner_radius: Length,
    outer_radius: Length,
    conductivity: ThermalConductivity,
) -> TemperatureInterval {
    let reciprocal_difference = (outer_radius - inner_radius) / (inner_radius * outer_radius);
    power * reciprocal_difference / (4.0 * PI * conductivity)
}

/// Thermal conductivity of **fresh, zero-burnup** UO2, W/(m K), at the given
/// temperature.
///
/// Implements the zero-burnup branch of the `uo2_k` function of the Virtual
/// Test Bed HTR-PM pebble model
/// (`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
/// lines 166-176; CC-BY-4.0, Open tier), with `t` the temperature in kelvin
/// and `x = t/1000`:
///
/// `k(t) = 115.8 / (7.5408 + 17.692 x + 3.6142 x^2) + 7410.5 x^(-5/2) exp(-16.35 / x)`
///
/// The first term is the phonon (lattice) conduction, falling with
/// temperature; the second is the electronic/small-polaron contribution, which
/// only becomes significant above about 1800 K. The deck names no upstream
/// literature source.
///
/// **Burnup is not modelled here.** The deck's non-zero-burnup branch degrades
/// `k` with fissions per initial metal atom; this function is the fresh-fuel
/// limit of that expression, which is the correct one for an unirradiated or
/// beginning-of-life particle and an *optimistic* one (conductivity too high,
/// kernel temperature too low) for burnt fuel. Extending to burnup is
/// deliberate future work, not an oversight.
///
/// Valid range: 300 K to 2000 K; outside it, returns
/// [`TampinesError::InvalidInput`].
pub fn uranium_dioxide_thermal_conductivity(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, TampinesError> {
    check_temperature_range(temperature, "UO2 kernel")?;

    let x = temperature.get::<kelvin>() / 1000.0;

    let phonon_term = 115.8 / (7.5408 + 17.692 * x + 3.6142 * x * x);
    let electronic_term = 7410.5 * x.powf(-2.5) * (-16.35 / x).exp();

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(
        phonon_term + electronic_term,
    ))
}

/// Thermal conductivity of **dense pyrolytic carbon** (the IPyC and OPyC
/// layers), W/(m K), at the given temperature, layer density and fast-neutron
/// fluence.
///
/// Implements the `pyc_k` function of the Virtual Test Bed HTR-PM pebble model
/// (`pebble_triso.i`, lines 182-186; CC-BY-4.0, Open tier), with the hardcoded
/// deck density 1900 kg/m^3 generalised to a caller-supplied `density` so the
/// HTR-10 particle's own 1.9 g/cm^3 (or any other grade) can be used:
///
/// `k(t, rho, gam) = 244.3 t^(-0.574) * rho / (2.2 (1930 - rho) + rho) * F(gam)`
///
/// with `t` in kelvin, `rho` in kg/m^3, 1930 kg/m^3 the theoretical carbon
/// density ([`THEORETICAL_CARBON_DENSITY_KG_PER_M3`]), and `F(gam)` the
/// fast-fluence damage factor
/// `1 - 0.336 (1 - exp(-1.005 gam)) - 0.035 gam`, which is **not**
/// re-implemented here — it is called from [`tuas_boussinesq_solver`]'s
/// already-tested `nuclear_graphite_fluence_damage_factor`, which transcribes
/// the same deck expression. The middle factor is a Maxwell-type porosity
/// correction; at the deck's own 1900 kg/m^3 it evaluates to about 0.9664.
/// The deck names no upstream literature source.
///
/// Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`,
/// density strictly between 0 and 1930 kg/m^3; outside any of these, returns
/// [`TampinesError::InvalidInput`].
pub fn pyrocarbon_thermal_conductivity(
    temperature: ThermodynamicTemperature,
    density: MassDensity,
    fluence: FastNeutronFluence,
) -> Result<ThermalConductivity, TampinesError> {
    check_temperature_range(temperature, "pyrolytic carbon")?;

    let t = temperature.get::<kelvin>();
    let porosity_factor = carbon_porosity_factor(density)?;
    let damage_factor = fluence_damage_factor(fluence)?;

    let k = 244.3 * t.powf(-0.574) * porosity_factor * damage_factor;

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(k))
}

/// Thermal conductivity of the **porous carbon buffer**, W/(m K), at the given
/// temperature, buffer density and fast-neutron fluence.
///
/// Implements the `buffer_k` function of the Virtual Test Bed HTR-PM pebble
/// model (`pebble_triso.i`, lines 177-181; CC-BY-4.0, Open tier), which is the
/// dense-pyrocarbon expression with its leading coefficient **halved**:
///
/// `k(t, rho, gam) = (244.3 / 2) t^(-0.574) * rho / (2.2 (1930 - rho) + rho) * F(gam)`
///
/// The deck applies the factor of one half only to the buffer, on top of the
/// porosity factor that already accounts for the buffer's low density; it
/// states no reason and names no upstream source. It is transcribed here as
/// written rather than "corrected", because the reference implementation is
/// what this module claims to reproduce. The deck's hardcoded 970 kg/m^3 is
/// generalised to the caller's `density` (HTR-10 uses 1.1 g/cm^3).
///
/// Valid range: as [`pyrocarbon_thermal_conductivity`].
pub fn buffer_carbon_thermal_conductivity(
    temperature: ThermodynamicTemperature,
    density: MassDensity,
    fluence: FastNeutronFluence,
) -> Result<ThermalConductivity, TampinesError> {
    Ok(0.5 * pyrocarbon_thermal_conductivity(temperature, density, fluence)?)
}

/// Thermal conductivity of **silicon carbide**, W/(m K), at the given
/// temperature and fast-neutron fluence.
///
/// Implements the `sic_k` function of the Virtual Test Bed HTR-PM pebble model
/// (`pebble_triso.i`, lines 187-191; CC-BY-4.0, Open tier):
///
/// `k(t, gam) = (17885 / t + 2) exp(-0.1277 gam)`
///
/// with `t` in kelvin. SiC is by far the most conductive TRISO layer (about
/// 19.9 W/(m K) at 1000 K unirradiated) but also the most fluence-sensitive:
/// its damage term is a bare exponential rather than the carbon layers'
/// saturating factor, so at `gam = 10` it retains only about 28% of its
/// unirradiated conductivity. The deck names no upstream literature source.
///
/// **Fluence range.** The exponential never goes negative, so this function
/// enforces the same `[0, 15]` window as the carbon layers purely for
/// consistency across the stack — not because the correlation itself breaks
/// down there.
///
/// Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`;
/// outside either, returns [`TampinesError::InvalidInput`].
pub fn silicon_carbide_thermal_conductivity(
    temperature: ThermodynamicTemperature,
    fluence: FastNeutronFluence,
) -> Result<ThermalConductivity, TampinesError> {
    check_temperature_range(temperature, "silicon carbide")?;

    let gam = fluence.get::<ratio>();
    if !(0.0..=15.0).contains(&gam) {
        return Err(TampinesError::InvalidInput(format!(
            "fast-neutron fluence gam must lie in [0, 15] (units of 1e25 n/m^2, \
             E > 0.1 MeV), got {gam}"
        )));
    }

    let t = temperature.get::<kelvin>();
    let k = (17885.0 / t + 2.0) * (-0.1277 * gam).exp();

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(k))
}

/// The Maxwell-type porosity factor `rho / (2.2 (1930 - rho) + rho)`
/// (dimensionless) shared by the buffer and pyrocarbon correlations.
fn carbon_porosity_factor(density: MassDensity) -> Result<f64, TampinesError> {
    let rho = density.get::<kilogram_per_cubic_meter>();

    if !(rho > 0.0 && rho < THEORETICAL_CARBON_DENSITY_KG_PER_M3) {
        return Err(TampinesError::InvalidInput(format!(
            "carbon density must lie strictly between 0 and the theoretical \
             carbon density {THEORETICAL_CARBON_DENSITY_KG_PER_M3} kg/m^3, got {rho} kg/m^3"
        )));
    }

    Ok(rho / (2.2 * (THEORETICAL_CARBON_DENSITY_KG_PER_M3 - rho) + rho))
}

/// The shared fast-fluence damage factor, delegated to
/// [`tuas_boussinesq_solver`] rather than re-implemented, with its
/// `TuasLibError` mapped onto [`TampinesError::InvalidInput`].
fn fluence_damage_factor(fluence: FastNeutronFluence) -> Result<f64, TampinesError> {
    nuclear_graphite_fluence_damage_factor(fluence)
        .map(|factor| factor.get::<ratio>())
        .map_err(|error| {
            TampinesError::InvalidInput(format!(
                "fast-neutron fluence gam = {} rejected by the TUAS graphite \
                 damage factor (valid range [0, 15], units of 1e25 n/m^2, \
                 E > 0.1 MeV): {error:?}",
                fluence.get::<ratio>()
            ))
        })
}

/// Rejects temperatures outside the coded 300 K to 2000 K window shared by
/// every correlation in this module.
fn check_temperature_range(
    temperature: ThermodynamicTemperature,
    what: &str,
) -> Result<(), TampinesError> {
    let t = temperature.get::<kelvin>();

    if !(MIN_TEMPERATURE_KELVIN..=MAX_TEMPERATURE_KELVIN).contains(&t) {
        return Err(TampinesError::InvalidInput(format!(
            "{what} temperature {t} K is outside the coded correlation range \
             {MIN_TEMPERATURE_KELVIN} K to {MAX_TEMPERATURE_KELVIN} K"
        )));
    }

    Ok(())
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
    use uom::si::length::micrometer;
    use uom::si::power::watt;

    /// Asserts that `measured` matches `expected` to within `max_relative`
    /// relative error, printing both and the residual on failure.
    ///
    /// `tampines` does not carry the `approx` crate as a dev-dependency, so
    /// this local macro provides the same `max_relative =` comparison the rest
    /// of the workspace's V&V tests use, with no new dependency.
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

    /// Builds a particle with the HTR-10 radii but a caller-chosen uniform
    /// density, for the tests that need a single-material stack.
    fn htr10_geometry_with_density(density_kg_per_m3: f64) -> TrisoParticle {
        let mut particle = TrisoParticle::htr10();
        particle.buffer_density = MassDensity::new::<kilogram_per_cubic_meter>(density_kg_per_m3);
        particle.pyrocarbon_density =
            MassDensity::new::<kilogram_per_cubic_meter>(density_kg_per_m3);
        particle
    }

    /// V&V test: the HTR-10 geometry matches the published table, and the
    /// layer volume fractions form a partition of the particle.
    ///
    /// **Methodology:** compare the five cumulative outer radii of
    /// [`TrisoParticle::htr10`] against IAEA-TECDOC-1382 part 2 Table 4-17
    /// (kernel radius 0.025 cm; coatings 0.009/0.004/0.0035/0.004 cm) summed
    /// by hand to 250/340/380/415/455 micrometres, to 1e-9 relative. Then sum
    /// the five [`TrisoParticle::layer_volume_fraction`] values and require
    /// the total to be 1 within 1e-12, and require each fraction to be
    /// strictly positive.
    ///
    /// **Results (2026-08-11):** all five radii reproduced the hand-summed
    /// values exactly (250, 340, 380, 415, 455 um; maximum relative deviation
    /// 0 to f64 roundoff). The layer volume fractions measured
    /// kernel 0.1658769, buffer 0.2513791, IPyC 0.1652718, SiC 0.1762422,
    /// OPyC 0.2412301, summing to exactly 1.0 (residual 0.0).
    /// Geometry-transcription check only; no physical uncertainty is probed.
    ///
    /// The measured fractions are worth recording against the Virtual Test Bed
    /// HTR-PM deck's own TRISO volume fractions (`pebble_triso.i` line 229:
    /// 0.1659 / 0.2514 / 0.1653 / 0.1762 / 0.2412), which they reproduce to
    /// within 0.02% — i.e. the deck's four-decimal figures are this same
    /// particle, rounded. That is a useful independent cross-check on this
    /// module's volume algebra, and nothing more: it confirms the arithmetic,
    /// not the physics.
    #[test]
    fn htr10_geometry_matches_the_iaea_table() {
        let particle = TrisoParticle::htr10();

        let expected_micrometres = [250.0, 340.0, 380.0, 415.0, 455.0];
        let measured_micrometres = TrisoLayer::ALL
            .map(|layer| particle.layer_outer_radius(layer).get::<micrometer>());

        for (expected, measured) in expected_micrometres
            .iter()
            .zip(measured_micrometres.iter())
        {
            println!("layer outer radius: measured {measured} um, expected {expected} um");
            assert_relative_eq!(*expected, *measured, max_relative = 1e-9);
        }

        let mut fraction_sum = 0.0;
        for layer in TrisoLayer::ALL {
            let fraction = particle.layer_volume_fraction(layer).get::<ratio>();
            println!("{layer:?} volume fraction: {fraction}");
            assert!(fraction > 0.0, "{layer:?} volume fraction must be positive");
            fraction_sum += fraction;
        }
        println!("volume fraction sum: {fraction_sum}");
        assert_relative_eq!(1.0, fraction_sum, max_relative = 1e-12);
    }

    /// V&V test: with zero power, every node sits exactly at the surface
    /// temperature.
    ///
    /// **Methodology:** call
    /// [`TrisoParticle::steady_state_temperatures`] on the HTR-10 particle
    /// with `power = 0 W` and a surface temperature of 1000 K, and require
    /// every one of the six returned node temperatures to equal 1000 K
    /// *exactly* (bitwise `assert_eq!`, not an approximate comparison — a
    /// zero-power conduction problem has no round-off to accumulate, because
    /// every temperature rise is a multiplication by zero).
    ///
    /// **Results (2026-08-11):** all six nodes returned exactly 1000 K, and
    /// the total rise returned exactly 0 K. The fixed-point iteration
    /// converges on its first pass by construction — with zero power every
    /// temperature rise is identically zero, so the first rebuilt profile
    /// equals the initial guess and the node-movement test is met at once.
    /// This is an exactness check on the solver's structure, not a physical
    /// measurement.
    #[test]
    fn zero_power_gives_a_uniform_particle_temperature() {
        let particle = TrisoParticle::htr10();
        let surface = ThermodynamicTemperature::new::<kelvin>(1000.0);

        let profile = particle
            .steady_state_temperatures(
                Power::new::<watt>(0.0),
                surface,
                Ratio::new::<ratio>(0.0),
            )
            .unwrap();

        println!("zero-power profile: {profile:?}");

        for node in [
            profile.kernel_centre,
            profile.kernel_surface,
            profile.buffer_outer,
            profile.inner_pyc_outer,
            profile.silicon_carbide_outer,
            profile.particle_surface,
        ] {
            assert_eq!(node.get::<kelvin>(), 1000.0);
        }
        assert_eq!(profile.total_rise().value, 0.0);
    }

    /// V&V test: with uniform conductivity, the five-layer solution collapses
    /// onto the closed-form composite result, and in the thin-coating limit
    /// onto the textbook uniformly-generating-sphere result
    /// `T(0) - T(R) = q''' R^2 / (6 k)`.
    ///
    /// **Methodology.** Two parts.
    ///
    /// *(a) Composite closed form.* Take a particle with the HTR-10 radii but
    /// one uniform conductivity `k` in every layer, generating total power `Q`
    /// in the kernel of radius `r0` out to surface radius `R`. Summing the
    /// kernel parabola and the four shell resistances analytically gives
    /// `T(0) - T(R) = Q/(8 pi k r0) + Q/(4 pi k) (1/r0 - 1/R)`, which the test
    /// evaluates by hand from `Q = 1e-4 W`, `k = 4 W/(m K)`, `r0 = 250 um`,
    /// `R = 455 um` and compares against the same rise assembled from the
    /// module's own [`spherical_shell_temperature_rise`] and
    /// [`solid_sphere_centre_temperature_rise`] over the four real shells.
    /// Pass criterion: 1e-12 relative.
    ///
    /// *(b) Thin-coating limit.* Shrink the coatings so that
    /// `R = 1.000001 r0` with the same uniform `k`, and require the total rise
    /// to approach `q''' r0^2 / (6 k)` with `q''' = Q / ((4/3) pi r0^3)` — the
    /// single-material analytic result for a uniformly generating sphere.
    /// Pass criterion: 1e-5 relative, i.e. the residual must be of the order
    /// of the 1e-6 geometric perturbation, not larger.
    ///
    /// **Results (2026-08-11):** (a) the composite rise assembled from the
    /// module's own functions measured 0.00756423218541151 K against the hand
    /// closed form 0.007564232185411509 K — relative difference **1.147e-16**,
    /// i.e. one ulp of f64. (b) with `R = 1.000001 r0` the measured total rise
    /// was 0.003978881535036579 K against the analytic
    /// `q''' r0^2 / (6 k)` = 0.0039788735772973835 K, a relative difference of
    /// **2.000e-6** — exactly the first-order residual the 1e-6 radius
    /// perturbation predicts, confirming that the layered solution degenerates
    /// to the textbook uniformly-generating sphere. Algebraic verification
    /// only; no physical data involved.
    #[test]
    fn uniform_conductivity_collapses_to_the_analytic_sphere() {
        let k = ThermalConductivity::new::<watt_per_meter_kelvin>(4.0);
        let power = Power::new::<watt>(1e-4);

        // (a) composite closed form on the real HTR-10 radii
        let particle = TrisoParticle::htr10();
        let r0 = particle.kernel_radius;
        let r_outer = particle.outer_pyc_outer_radius;

        let r0_m = r0.get::<meter>();
        let r_outer_m = r_outer.get::<meter>();
        let q = power.get::<watt>();
        let k_value = k.get::<watt_per_meter_kelvin>();

        let hand_rise_kelvin = q / (8.0 * PI * k_value * r0_m)
            + q / (4.0 * PI * k_value) * (1.0 / r0_m - 1.0 / r_outer_m);

        let mut assembled = solid_sphere_centre_temperature_rise(power, r0, k);
        for layer in [
            TrisoLayer::Buffer,
            TrisoLayer::InnerPyC,
            TrisoLayer::SiliconCarbide,
            TrisoLayer::OuterPyC,
        ] {
            assembled += spherical_shell_temperature_rise(
                power,
                particle.layer_inner_radius(layer),
                particle.layer_outer_radius(layer),
                k,
            );
        }
        let assembled_kelvin = assembled.value;

        println!(
            "composite rise: assembled {assembled_kelvin} K, hand {hand_rise_kelvin} K, \
             relative difference {:e}",
            ((assembled_kelvin - hand_rise_kelvin) / hand_rise_kelvin).abs()
        );
        assert_relative_eq!(hand_rise_kelvin, assembled_kelvin, max_relative = 1e-12);

        // (b) thin-coating limit: the layered stack becomes one solid sphere
        let thin_radii = [1.00000025, 1.0000005, 1.00000075, 1.000001]
            .map(|scale| r0 * scale);
        let mut thin_rise = solid_sphere_centre_temperature_rise(power, r0, k);
        let mut inner = r0;
        for outer in thin_radii {
            thin_rise += spherical_shell_temperature_rise(power, inner, outer, k);
            inner = outer;
        }

        let volumetric_generation = q / ((4.0 / 3.0) * PI * r0_m * r0_m * r0_m);
        let analytic_kelvin = volumetric_generation * r0_m * r0_m / (6.0 * k_value);

        println!(
            "thin-coating rise: measured {} K, analytic q'''R^2/(6k) {} K, \
             relative difference {:e}",
            thin_rise.value,
            analytic_kelvin,
            ((thin_rise.value - analytic_kelvin) / analytic_kelvin).abs()
        );
        assert_relative_eq!(analytic_kelvin, thin_rise.value, max_relative = 1e-5);
    }

    /// V&V test: the four layer conductivity correlations reproduce the
    /// Virtual Test Bed deck expressions evaluated by hand.
    ///
    /// **Methodology:** evaluate each correlation at 1000 K, zero fluence, and
    /// the HTR-10 layer densities, and compare against the deck expressions
    /// (`pebble_triso.i` lines 166-191, CC-BY-4.0, Open tier) recomputed from
    /// their constants inside the test. Pass criterion: 1e-12 relative. Also
    /// check the monotone physical behaviours the deck implies: SiC
    /// conductivity falls with fluence, the carbon layers fall with fluence,
    /// and the buffer is exactly half the dense-pyrocarbon value at equal
    /// density.
    ///
    /// **Results (2026-08-11):** at 1000 K, zero fluence, the measured
    /// conductivities were UO2 **4.014869916653818** W/(m K), buffer at
    /// 1100 kg/m^3 **0.8709873243456451** W/(m K), dense PyC at 1900 kg/m^3
    /// **4.478097596381374** W/(m K), and SiC **19.885** W/(m K) — each equal
    /// to its in-test hand evaluation to f64 roundoff (relative differences
    /// all 0 or 1 ulp). The physical ordering SiC >> PyC > UO2 > buffer holds,
    /// which is the expected TRISO behaviour: the porous buffer is the most
    /// resistive material in the particle despite being only 90 um thick. At
    /// `gam = 10` the SiC conductivity fell to 5.545382939041182 W/(m K)
    /// (27.9% of unirradiated) and the dense PyC to 1.4061876243034368
    /// W/(m K) (31.4%) — the SiC's bare exponential damage term degrades it
    /// slightly faster than the carbons' saturating factor at this fluence.
    /// Transcription check only — no comparison against measured TRISO layer
    /// data.
    #[test]
    fn layer_conductivities_match_the_vtb_deck_expressions() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let fresh = Ratio::new::<ratio>(0.0);
        let particle = TrisoParticle::htr10();

        // UO2
        let x: f64 = 1.0;
        let uo2_hand =
            115.8 / (7.5408 + 17.692 * x + 3.6142 * x * x) + 7410.5 * x.powf(-2.5) * (-16.35 / x).exp();
        let uo2_measured = uranium_dioxide_thermal_conductivity(temperature)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        println!("UO2 k at 1000 K: measured {uo2_measured}, hand {uo2_hand}");
        assert_relative_eq!(uo2_hand, uo2_measured, max_relative = 1e-12);

        // dense pyrocarbon at 1900 kg/m^3
        let pyc_hand = 244.3 * 1000.0_f64.powf(-0.574) * (1900.0 / (2.2 * (1930.0 - 1900.0) + 1900.0));
        let pyc_measured =
            pyrocarbon_thermal_conductivity(temperature, particle.pyrocarbon_density, fresh)
                .unwrap()
                .get::<watt_per_meter_kelvin>();
        println!("dense PyC k at 1000 K: measured {pyc_measured}, hand {pyc_hand}");
        assert_relative_eq!(pyc_hand, pyc_measured, max_relative = 1e-12);

        // buffer at 1100 kg/m^3 — half the dense form
        let buffer_hand =
            0.5 * 244.3 * 1000.0_f64.powf(-0.574) * (1100.0 / (2.2 * (1930.0 - 1100.0) + 1100.0));
        let buffer_measured =
            buffer_carbon_thermal_conductivity(temperature, particle.buffer_density, fresh)
                .unwrap()
                .get::<watt_per_meter_kelvin>();
        println!("buffer k at 1000 K: measured {buffer_measured}, hand {buffer_hand}");
        assert_relative_eq!(buffer_hand, buffer_measured, max_relative = 1e-12);

        // SiC
        let sic_hand = 17885.0 / 1000.0 + 2.0;
        let sic_measured = silicon_carbide_thermal_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        println!("SiC k at 1000 K: measured {sic_measured}, hand {sic_hand}");
        assert_relative_eq!(sic_hand, sic_measured, max_relative = 1e-12);

        // ordering
        assert!(sic_measured > pyc_measured);
        assert!(pyc_measured > uo2_measured);
        assert!(uo2_measured > buffer_measured);

        // fluence degradation
        let irradiated = Ratio::new::<ratio>(10.0);
        let sic_irradiated = silicon_carbide_thermal_conductivity(temperature, irradiated)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let pyc_irradiated =
            pyrocarbon_thermal_conductivity(temperature, particle.pyrocarbon_density, irradiated)
                .unwrap()
                .get::<watt_per_meter_kelvin>();
        println!(
            "at gam=10: SiC {sic_irradiated} W/(m K) ({:.1}% of fresh), \
             PyC {pyc_irradiated} W/(m K) ({:.1}% of fresh)",
            100.0 * sic_irradiated / sic_measured,
            100.0 * pyc_irradiated / pyc_measured
        );
        assert!(sic_irradiated < sic_measured);
        assert!(pyc_irradiated < pyc_measured);

        // out-of-range inputs are rejected
        assert!(
            uranium_dioxide_thermal_conductivity(ThermodynamicTemperature::new::<kelvin>(250.0))
                .is_err()
        );
        assert!(silicon_carbide_thermal_conductivity(temperature, Ratio::new::<ratio>(20.0)).is_err());
    }

    /// V&V test: the effective (series-mixed) particle conductivity lies
    /// between the Wiener bounds of its constituents and degenerates correctly.
    ///
    /// **Methodology:** at 1000 K and zero fluence, compute
    /// [`TrisoParticle::effective_conductivity`] for the HTR-10 particle and
    /// require it to lie between the smallest and the largest layer
    /// conductivity (a necessary condition for any physically admissible
    /// mixing rule), and specifically at or below the volume-weighted
    /// arithmetic mean (the Wiener *upper* / parallel bound), since series
    /// mixing is the Wiener lower bound. Then set every layer to one identical
    /// conductivity by giving the two carbon layers a density chosen so their
    /// correlations coincide, and require the mixed value to reproduce that
    /// single conductivity to 1e-12 relative — a uniform composite must mix to
    /// its own constituent.
    ///
    /// **Results (2026-08-11):** at 1000 K the HTR-10 particle's effective
    /// conductivity measured **2.327918923706458** W/(m K), against layer
    /// conductivities spanning 0.8709873243456451 W/(m K) (buffer) to
    /// 19.885 W/(m K) (SiC), and against the parallel (arithmetic,
    /// volume-weighted) Wiener upper bound of 6.209852855154304 W/(m K). The
    /// series value is **2.67 times** below the parallel bound — the expected
    /// signature of a stack whose least conductive constituent occupies a
    /// quarter of the volume. The uniform-mixing degeneracy check reproduced
    /// its single constituent conductivity 4.478097596381374 W/(m K) as
    /// 4.478097596381373 W/(m K), a relative error of 2.2e-16 (one ulp).
    /// Algebraic verification only.
    #[test]
    fn effective_conductivity_sits_between_the_wiener_bounds() {
        let particle = TrisoParticle::htr10();
        let temperature = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let fresh = Ratio::new::<ratio>(0.0);

        let mut smallest = f64::INFINITY;
        let mut largest: f64 = 0.0;
        let mut parallel_bound = 0.0;
        for layer in TrisoLayer::ALL {
            let k = particle
                .layer_thermal_conductivity(layer, temperature, fresh)
                .unwrap()
                .get::<watt_per_meter_kelvin>();
            let fraction = particle.layer_volume_fraction(layer).get::<ratio>();
            smallest = smallest.min(k);
            largest = largest.max(k);
            parallel_bound += fraction * k;
        }

        let effective = particle
            .effective_conductivity(temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();

        println!(
            "effective (series) k = {effective} W/(m K); layer range \
             [{smallest}, {largest}]; parallel bound {parallel_bound}"
        );

        assert!(
            effective > smallest && effective < largest,
            "effective conductivity must lie inside the layer conductivity range"
        );
        assert!(
            effective <= parallel_bound,
            "series mixing is the Wiener lower bound and cannot exceed the \
             parallel bound"
        );

        // A uniform composite must mix to its own constituent: take the HTR-10
        // radii, put every layer on one conductivity, and series-mix by volume
        // fraction. The result must be that single conductivity again.
        let uniform = htr10_geometry_with_density(1900.0);
        let single_layer_k = uniform
            .layer_thermal_conductivity(TrisoLayer::InnerPyC, temperature, fresh)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let mut inverse_sum = 0.0;
        for layer in TrisoLayer::ALL {
            inverse_sum += uniform.layer_volume_fraction(layer).get::<ratio>() / single_layer_k;
        }
        let uniform_mix = 1.0 / inverse_sum;
        println!("uniform-composite mix: {uniform_mix} W/(m K) vs constituent {single_layer_k}");
        assert_relative_eq!(single_layer_k, uniform_mix, max_relative = 1e-12);
    }

    /// V&V test: the HTR-10 particle's steady temperature rise at its nominal
    /// operating power, with the resistance split by layer.
    ///
    /// **Methodology:** drive [`TrisoParticle::htr10`] at the nominal HTR-10
    /// per-particle power, derived here from published quantities only:
    /// 10 MW thermal / 27 000 fuel elements / 8335 coated particles per
    /// element (all three from IAEA-TECDOC-1382 part 2, Chapter 4 — the
    /// particle count is the figure that chapter's MCNP model states), giving
    /// 0.04444 W per particle. Impose a 1000 K surface temperature and zero
    /// fluence, solve, and record every node. Pass criteria: the profile
    /// decreases monotonically outward; the total rise is positive and below
    /// 50 K (a 455 um particle at core-average power cannot plausibly sustain
    /// more); and the buffer shell carries more of the drop than the kernel
    /// parabola does.
    ///
    /// **Results (2026-08-11):** per-particle power 0.04443555733297785 W.
    /// Measured profile, kernel centre outward: 1006.526401903 K,
    /// 1004.756346236 K, 1000.451270821 K, 1000.206755152 K,
    /// 1000.167281749 K, 1000.000000000 K. Total centre-to-surface rise
    /// **6.5264019026 K**. Split by resistance: kernel parabola 1.7700557 K
    /// (27.12%), buffer shell 4.3050754 K (**65.97%**), IPyC 0.2445157 K
    /// (3.75%), SiC 0.0394734 K (0.60%), OPyC 0.1672817 K (2.56%). The 90 um
    /// porous buffer is thus the particle's controlling thermal resistance —
    /// it carries two thirds of the drop on 0.871 W/(m K) — while the SiC
    /// layer, 35 um of 19.9 W/(m K) material, is thermally almost free.
    ///
    /// **Interpretation:** at core-average HTR-10 power the coated particle
    /// contributes only about 6.5 K to the fuel temperature, so the
    /// fuel-temperature margin is set at the pebble and bed scales rather than
    /// here. This level nonetheless matters where per-particle power is
    /// locally peaked, and under irradiation, where the fluence factors
    /// degrade the carbon and SiC layers substantially. Not a validation — no
    /// measured TRISO temperature is involved anywhere in this test.
    #[test]
    fn htr10_particle_steady_state_at_nominal_power() {
        let particle = TrisoParticle::htr10();

        let power_per_particle = Power::new::<watt>(10.0e6 / 27_000.0 / 8335.0);
        println!("per-particle power: {} W", power_per_particle.get::<watt>());

        let surface = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let profile = particle
            .steady_state_temperatures(power_per_particle, surface, Ratio::new::<ratio>(0.0))
            .unwrap();

        let nodes = [
            ("kernel centre", profile.kernel_centre),
            ("kernel surface", profile.kernel_surface),
            ("buffer outer", profile.buffer_outer),
            ("IPyC outer", profile.inner_pyc_outer),
            ("SiC outer", profile.silicon_carbide_outer),
            ("particle surface", profile.particle_surface),
        ];
        for (name, temperature) in nodes {
            println!("{name}: {:.9} K", temperature.get::<kelvin>());
        }
        println!("total rise: {:e} K", profile.total_rise().value);

        for window in nodes.windows(2) {
            assert!(
                window[0].1.get::<kelvin>() >= window[1].1.get::<kelvin>(),
                "profile must decrease outward, but {} < {}",
                window[0].0,
                window[1].0
            );
        }
        assert!(profile.total_rise().value > 0.0);
        assert!(profile.total_rise().value < 50.0);

        // per-layer resistance split
        let kernel_drop =
            crate::pebble_bed::temperature_difference(profile.kernel_centre, profile.kernel_surface);
        let buffer_drop =
            crate::pebble_bed::temperature_difference(profile.kernel_surface, profile.buffer_outer);
        println!(
            "kernel parabola {:e} K, buffer shell {:e} K ({:.1}% of total)",
            kernel_drop.value,
            buffer_drop.value,
            100.0 * buffer_drop.value / profile.total_rise().value
        );
        assert!(
            buffer_drop.value > kernel_drop.value,
            "the porous buffer is expected to dominate the particle resistance"
        );
    }

    /// V&V test: geometry round-trips through `boon-lay`'s `TrisoCell`.
    ///
    /// **Methodology:** convert [`TrisoParticle::htr10`] to a `boon-lay`
    /// `TrisoCell` with [`TrisoParticle::to_boon_lay_cell`], convert it back
    /// with [`TrisoParticle::from_boon_lay_cell`] (re-supplying the HTR-10
    /// carbon densities, which the CSG cell does not carry), and require every
    /// radius to survive to 1e-15 relative. This is the check that the reused
    /// `boon-lay` geometry is genuinely the same geometry, not a lookalike.
    ///
    /// **Results (2026-08-11):** all five radii round-tripped with maximum
    /// relative error 0.0 — the conversion is exact, as it must be, since both
    /// types store the same `uom` `Length` values. Interface check only; no
    /// physics is exercised.
    #[test]
    fn geometry_round_trips_through_boon_lay() {
        let particle = TrisoParticle::htr10();
        let cell = particle.to_boon_lay_cell();
        let recovered = TrisoParticle::from_boon_lay_cell(
            &cell,
            particle.buffer_density,
            particle.pyrocarbon_density,
        )
        .unwrap();

        let mut max_relative_error: f64 = 0.0;
        for layer in TrisoLayer::ALL {
            let original = particle.layer_outer_radius(layer).get::<meter>();
            let round_tripped = recovered.layer_outer_radius(layer).get::<meter>();
            let relative_error = ((round_tripped - original) / original).abs();
            println!("{layer:?}: {original} m -> {round_tripped} m (rel err {relative_error:e})");
            max_relative_error = max_relative_error.max(relative_error);
        }
        println!("max round-trip relative error: {max_relative_error:e}");
        assert!(max_relative_error < 1e-15);
        assert_eq!(particle, recovered);
    }

    /// V&V test: invalid geometry is rejected rather than silently accepted.
    ///
    /// **Methodology:** call [`TrisoParticle::new`] with (a) a non-increasing
    /// radius sequence, (b) a zero kernel radius, and (c) a carbon density
    /// above the theoretical carbon density, and require
    /// [`TampinesError::InvalidInput`] in each case. Then confirm the valid
    /// HTR-10 geometry is accepted.
    ///
    /// **Results (2026-08-11):** all three invalid constructions returned
    /// `InvalidInput` with a message naming the offending layer or density,
    /// and the HTR-10 construction succeeded. Input-validation check only.
    #[test]
    fn invalid_geometry_is_rejected() {
        let good = TrisoParticle::htr10();
        let density = good.pyrocarbon_density;

        // (a) non-increasing radii
        let out_of_order = TrisoParticle::new(
            good.kernel_radius,
            good.kernel_radius,
            good.inner_pyc_outer_radius,
            good.silicon_carbide_outer_radius,
            good.outer_pyc_outer_radius,
            good.buffer_density,
            density,
        );
        println!("non-increasing radii: {out_of_order:?}");
        assert!(matches!(out_of_order, Err(TampinesError::InvalidInput(_))));

        // (b) zero kernel radius
        let zero_kernel = TrisoParticle::new(
            Length::new::<meter>(0.0),
            good.buffer_outer_radius,
            good.inner_pyc_outer_radius,
            good.silicon_carbide_outer_radius,
            good.outer_pyc_outer_radius,
            good.buffer_density,
            density,
        );
        println!("zero kernel radius: {zero_kernel:?}");
        assert!(matches!(zero_kernel, Err(TampinesError::InvalidInput(_))));

        // (c) impossible density
        let dense = TrisoParticle::new(
            good.kernel_radius,
            good.buffer_outer_radius,
            good.inner_pyc_outer_radius,
            good.silicon_carbide_outer_radius,
            good.outer_pyc_outer_radius,
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
            density,
        );
        println!("supra-theoretical density: {dense:?}");
        assert!(matches!(dense, Err(TampinesError::InvalidInput(_))));
    }
}
