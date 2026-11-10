use petir::uom_math::QuantityMath;
use petir::mathf::RealMath;
use std::f64::consts::PI;

use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::heat_capacity::boltzmann_constant;
use uom::si::molar_heat_capacity::molar_gas_constant;
use uom::si::ratio::ratio;
use uom::ConstZero;

/// Mean speed (Maxwell–Boltzmann) at temperature T for a particle of mass m:
/// v_mean = sqrt(8 k_B T / (pi m))
///
/// used uom si botlzmann constant
pub fn mean_speed(medium_temperature: ThermodynamicTemperature, particle_mass: Mass) -> Velocity {
    // k_B * T has dimension of energy
    let k_b_t: Energy = HeatCapacity::new::<boltzmann_constant>(1.0) * medium_temperature;
    // specific energy (m^2/s^2)
    let specific = (8.0 * k_b_t) / (PI * particle_mass);
    // sqrt to get velocity
    specific.r_sqrt()
}

/// Expected number of collisions in time t with mean free path ℓ:
/// E[N(t)] = (t / ℓ) * E[v]
/// Returns a dimensionless count (f64).
///
///
/// now this is not quite atomic jumps as chatGPT suggested,
/// D = 1/6 a^2 * nu
///
/// However, atomic jumps assume diffusion is only within monocrystalline
/// material without defects.
///
/// In reality, there are defects, grain boundaries, dislocations etc.
/// Therefore, we need an effective diffusion coefficient to consider
/// this
pub fn expected_collisions_atomic_jumps(
    medium_temperature: ThermodynamicTemperature,
    particle_mass: Mass,
    mean_free_path: Length,
    t: Time,
) -> f64 {
    let v_mean = mean_speed(medium_temperature, particle_mass);
    // Compute in scalar form to avoid needing reciprocal-velocity quantity:
    // (t/ell) has units s/m, v_mean has m/s -> dimensionless.

    return (t / mean_free_path * v_mean).get::<ratio>();
}

/// diffusion coefficient
/// from Jiang 2023
/// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
/// Novascone, S. R. (2023). Fission product transport in TRISO particles
/// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
/// Idaho Falls, ID (United States).
///
/// D = D1 exp (-Q1/RT) + D2 exp (-Q2/RT)
///
/// Neutron fluence is also a factor,
/// but if there is no neutron fluence, just give the None enum
///
/// Returns `None` where [`get_diffusion_coeff_jiang`] returns
/// [`MissingDiffusionData`] (since 2026-11-10, gh:#638: krypton in the
/// buffer). A caller that must tell "no cited coefficient" apart from a
/// value should call [`get_diffusion_coeff_jiang`] instead.
pub fn try_get_diffusion_coeff_jiang(
    triso_layer: TrisoPebbleLayerMaterial,
    nuclide: Nuclide,
    temperature: ThermodynamicTemperature,
    gamma_neutron_fluence: Option<ArealNumberDensity>,
) -> Option<DiffusionCoefficient> {
    get_diffusion_coeff_jiang(triso_layer, nuclide, temperature, gamma_neutron_fluence).ok()
}

/// The (element, layer) pair has no cited diffusion coefficient, so no `D`
/// is returned rather than an invented one (gh:#638).
///
/// The one case today is **krypton (Z = 36) in the buffer** and in the two
/// graphite variants. Jiang et al. (2023) tabulate Kr for the kernel, PyC
/// and SiC only (as does Hales et al. (2022) Table I, reproduced in Toptan
/// et al., INL/RPT-22-69706, p. 20). The `1e-8 m^2/s`, `Q = 0` buffer value
/// carried by Ag, Cs and Sr is the CRP-6 "buffer is fast" convention, not a
/// measured Kr value, so it is not applied to Kr without a source that says
/// it applies (a maintainer decision, asked on gh:#638).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingDiffusionData {
    /// Atomic number of the diffusing element.
    pub element_z: u32,
    /// Layer the coefficient was asked for.
    pub layer: TrisoPebbleLayerMaterial,
}

impl std::fmt::Display for MissingDiffusionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "no cited diffusion coefficient for element Z = {} in layer {:?} (gh:#638)",
            self.element_z, self.layer
        )
    }
}

impl std::error::Error for MissingDiffusionData {}

/// Whether Jiang et al. (2023) give krypton coefficients for `layer`.
///
/// `true` for the kernel, PyC, SiC and the cracked-material convention
/// (`1e-6 m^2/s`, Hales et al. 2021, CRP-6 cases 3d/3e); `false` for the
/// buffer and both graphite variants, whose lookups in
/// [`diffusion_coeffs`] are not defined.
#[inline]
pub fn krypton_has_cited_coefficients(layer: TrisoPebbleLayerMaterial) -> bool {
    match layer {
        TrisoPebbleLayerMaterial::KernelUO2
        | TrisoPebbleLayerMaterial::PyC
        | TrisoPebbleLayerMaterial::SiC
        | TrisoPebbleLayerMaterial::CrackedMaterial => true,
        TrisoPebbleLayerMaterial::Buffer
        | TrisoPebbleLayerMaterial::MatrixGraphite
        | TrisoPebbleLayerMaterial::StructuralGraphite => false,
    }
}

/// [`try_get_diffusion_coeff_jiang`] with a typed error in place of a panic
/// where no cited coefficient exists (gh:#638).
///
/// Same correlation and sources: `D = D1 exp(-Q1/RT) + D2 exp(-Q2/RT)`
/// from Jiang et al. (2023). Elements other than Ag, Cs, Sr and Kr still
/// fall back to silver (gh:#541), and the graphite variants of Ag, Cs and Sr
/// are still undefined (gh:#541, out of scope here).
pub fn get_diffusion_coeff_jiang(
    triso_layer: TrisoPebbleLayerMaterial,
    nuclide: Nuclide,
    temperature: ThermodynamicTemperature,
    gamma_neutron_fluence: Option<ArealNumberDensity>,
) -> Result<DiffusionCoefficient, MissingDiffusionData> {
    let (z, _a) = nuclide.get_z_a();

    if z == 36 && !krypton_has_cited_coefficients(triso_layer) {
        return Err(MissingDiffusionData {
            element_z: z,
            layer: triso_layer,
        });
    }

    let d: Option<DiffusionCoefficient> = match z {
        // Silver
        47 => {
            let d1: DiffusionCoefficient = get_d1_for_ag(triso_layer);
            let q1: MolarEnergy = get_q1_for_ag(triso_layer);
            let d2: DiffusionCoefficient = DiffusionCoefficient::ZERO;
            let q2: MolarEnergy = MolarEnergy::ZERO;

            // 8.314 J/mol K
            let r = MolarHeatCapacity::new::<molar_gas_constant>(1.0);

            let rt: MolarEnergy = r * temperature;

            let q1_by_rt: Ratio = q1 / rt;
            let q2_by_rt: Ratio = q2 / rt;

            let d = d1 * (-q1_by_rt).get::<ratio>().r_exp() + d2 * (-q2_by_rt).get::<ratio>().r_exp();

            Some(d)
        }
        // cesium
        55 => {
            // if no neutron fluence is supplied, just do zero
            let neutron_fluence: ArealNumberDensity = match gamma_neutron_fluence {
                Some(fluence) => fluence,
                None => ArealNumberDensity::ZERO,
            };
            let d1: DiffusionCoefficient = get_d1_for_cs(triso_layer, neutron_fluence);
            let q1: MolarEnergy = get_q1_for_cs(triso_layer);
            let d2: DiffusionCoefficient = get_d2_for_cs(triso_layer);
            let q2: MolarEnergy = get_q2_for_cs(triso_layer);

            // 8.314 J/mol K
            let r = MolarHeatCapacity::new::<molar_gas_constant>(1.0);

            let rt: MolarEnergy = r * temperature;

            let q1_by_rt: Ratio = q1 / rt;
            let q2_by_rt: Ratio = q2 / rt;

            let d = d1 * (-q1_by_rt).get::<ratio>().r_exp() + d2 * (-q2_by_rt).get::<ratio>().r_exp();

            Some(d)
        }
        // strontium
        38 => {
            let d1: DiffusionCoefficient = get_d1_for_sr(triso_layer);
            let q1: MolarEnergy = get_q1_for_sr(triso_layer);
            let d2: DiffusionCoefficient = get_d2_for_sr(triso_layer);
            let q2: MolarEnergy = get_q2_for_sr(triso_layer);

            // 8.314 J/mol K
            let r = MolarHeatCapacity::new::<molar_gas_constant>(1.0);

            let rt: MolarEnergy = r * temperature;

            let q1_by_rt: Ratio = q1 / rt;
            let q2_by_rt: Ratio = q2 / rt;

            let d = d1 * (-q1_by_rt).get::<ratio>().r_exp() + d2 * (-q2_by_rt).get::<ratio>().r_exp();

            Some(d)
        }
        // krypton
        //
        36 => {
            let d1: DiffusionCoefficient = get_d1_for_kr(triso_layer, temperature);
            let q1: MolarEnergy = get_q1_for_kr(triso_layer, temperature);
            let d2: DiffusionCoefficient = get_d2_for_kr(triso_layer, temperature);
            let q2: MolarEnergy = get_q2_for_kr(triso_layer, temperature);

            // 8.314 J/mol K
            let r = MolarHeatCapacity::new::<molar_gas_constant>(1.0);

            let rt: MolarEnergy = r * temperature;

            let q1_by_rt: Ratio = q1 / rt;
            let q2_by_rt: Ratio = q2 / rt;

            let d = d1 * (-q1_by_rt).get::<ratio>().r_exp() + d2 * (-q2_by_rt).get::<ratio>().r_exp();

            Some(d)
        }
        // for anything else, just assume it's silver
        _ => {
            let nuclide = Nuclide::Ag110m;
            return get_diffusion_coeff_jiang(
                triso_layer,
                nuclide,
                temperature,
                gamma_neutron_fluence,
            );
        }
    };

    // every arm above yields `Some`; the `Option` is kept from the original
    // body so the arms read unchanged.
    return d.ok_or(MissingDiffusionData {
        element_z: z,
        layer: triso_layer,
    });
}

pub mod diffusion_coeffs;
use diffusion_coeffs::*;

/// triso layer for diffusion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrisoPebbleLayerMaterial {
    KernelUO2,
    PyC,
    SiC,
    MatrixGraphite,
    StructuralGraphite,
    /// from CRP 6 tests within
    /// Hales, J. D., Jiang, W., Toptan, A., & Gamble,
    /// K. A. (2021). Modeling fission product
    /// diffusion in TRISO fuel particles with BISON.
    /// Journal of Nuclear Materials, 548, 152840.
    ///
    /// Tests 3d and 3e have cracked material,
    /// wherein the diffusion coefficient is
    /// 1e-6 m2/s
    CrackedMaterial,
    /// buffer layer
    Buffer,
}

/// boltzmann collision test
#[test]
fn boltzmann_test() {
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;
    // Example: nitrogen molecule at room temperature
    // Temperature T = 300 K
    let room_temp = ThermodynamicTemperature::new::<kelvin>(300.0);

    // Mass m: take N2 with molar mass ~28 g/mol -> per molecule m = 28e-3 kg / N_A
    // For demonstration, use m ≈ 4.65e-26 kg (approx for N2).
    let m = Mass::new::<kilogram>(4.65e-26);

    // Mean free path ℓ (example): 70 nm in air at STP (order of magnitude), or any value you need.
    let ell = Length::new::<meter>(70e-9);

    // Time horizon t: 1 microsecond
    let t = Time::new::<second>(1e-6);

    let v_mean = mean_speed(room_temp, m);
    let n_expected = expected_collisions_atomic_jumps(room_temp, m, ell, t);

    println!("Inputs:");
    println!("  T = {:.3} K", room_temp.get::<kelvin>());
    println!("  m = {:.3e} kg", m.get::<kilogram>());
    println!("  ℓ = {:.3e} m", ell.get::<meter>());
    println!("  t = {:.3e} s", t.get::<second>());

    println!("\nResults:");
    println!(
        "  Mean speed (Maxwell–Boltzmann): {:.3} m/s",
        v_mean.get::<meter_per_second>()
    );
    println!(
        "  Expected collisions in time t: {:.6} (dimensionless count)",
        n_expected
    );
}
