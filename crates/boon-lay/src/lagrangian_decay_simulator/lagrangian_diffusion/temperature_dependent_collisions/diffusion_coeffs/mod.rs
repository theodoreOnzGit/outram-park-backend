use petir::mathf::RealMath;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial;
use uom::si::areal_number_density::per_square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::*;
use uom::si::molar_energy::kilojoule_per_mole;
use uom::si::thermodynamic_temperature::degree_celsius;
/// from Jiang 2023
/// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
/// Novascone, S. R. (2023). Fission product transport in TRISO particles
/// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
/// Idaho Falls, ID (United States).
///

/// from Jiang 2023
/// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
/// Novascone, S. R. (2023). Fission product transport in TRISO particles
/// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
/// Idaho Falls, ID (United States).
///
/// table on page 13 of 105
#[inline]
pub fn get_d1_for_ag(triso_layer: TrisoPebbleLayerMaterial) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 6.7e-9,
        TrisoPebbleLayerMaterial::PyC => 5.3e-9,
        TrisoPebbleLayerMaterial::SiC => 3.6e-9,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 1e-6,
        TrisoPebbleLayerMaterial::Buffer => 1e-8,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_q1_for_ag(triso_layer: TrisoPebbleLayerMaterial) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 165.0,
        TrisoPebbleLayerMaterial::PyC => 154.0,
        TrisoPebbleLayerMaterial::SiC => 215.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_d1_for_cs(
    triso_layer: TrisoPebbleLayerMaterial,
    gamma_fast_neutron_fluence: ArealNumberDensity,
) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 5.6e-8,
        TrisoPebbleLayerMaterial::PyC => 6.3e-8,
        TrisoPebbleLayerMaterial::SiC => {
            let gamma_neutron_fluence_neutrons_per_sqm =
                gamma_fast_neutron_fluence.get::<per_square_meter>();

            let exponential_factor =
                (gamma_neutron_fluence_neutrons_per_sqm * 1e-25 * 1.1 / 5.0).r_exp();

            5.5e-14 * exponential_factor
        }
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 1e-6,
        TrisoPebbleLayerMaterial::Buffer => 1e-8,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_q1_for_cs(triso_layer: TrisoPebbleLayerMaterial) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 209.0,
        TrisoPebbleLayerMaterial::PyC => 222.0,
        TrisoPebbleLayerMaterial::SiC => 125.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_d2_for_cs(triso_layer: TrisoPebbleLayerMaterial) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 5.2e-4,
        TrisoPebbleLayerMaterial::PyC => 0.0,
        TrisoPebbleLayerMaterial::SiC => 1.6e-2,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_q2_for_cs(triso_layer: TrisoPebbleLayerMaterial) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 362.0,
        TrisoPebbleLayerMaterial::PyC => 0.0,
        TrisoPebbleLayerMaterial::SiC => 514.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_d1_for_sr(triso_layer: TrisoPebbleLayerMaterial) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 2.2e-3,
        TrisoPebbleLayerMaterial::PyC => 2.3e-6,
        TrisoPebbleLayerMaterial::SiC => 1.2e-9,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 1e-6,
        TrisoPebbleLayerMaterial::Buffer => 1e-8,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_q1_for_sr(triso_layer: TrisoPebbleLayerMaterial) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 488.0,
        TrisoPebbleLayerMaterial::PyC => 197.0,
        TrisoPebbleLayerMaterial::SiC => 205.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_d2_for_sr(triso_layer: TrisoPebbleLayerMaterial) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 0.0,
        TrisoPebbleLayerMaterial::PyC => 0.0,
        TrisoPebbleLayerMaterial::SiC => 1.8e6,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
#[inline]
pub fn get_q2_for_sr(triso_layer: TrisoPebbleLayerMaterial) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => 0.0,
        TrisoPebbleLayerMaterial::PyC => 0.0,
        TrisoPebbleLayerMaterial::SiC => 791.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

/// Krypton diffusion coefficient in the TRISO **buffer**, m^2/s, with no
/// temperature dependence (`Q = 0`) and no second term (gh:#638).
///
/// **Source (the value).** IAEA-TECDOC-978 (1997), *Fuel performance and
/// fission product behaviour in gas cooled reactors*, Appendix A, section
/// A.1.2 "Pyrocarbon", printed p. 440: "The carbonaceous buffer layer is
/// assumed in the diffusion model to have no significant retention
/// capability for fission products. In FRG calculations, a value of the
/// diffusion coefficient of 10^-8 m^2/s with no temperature dependence
/// (activation energy equal to zero) is used." The same appendix's FRG
/// tables give krypton in the kernel (Table A-1, "I, Kr, Xe") and in PyC
/// (Table A-2, "Kr, Xe"), the values this module already carries.
///
/// **Source (that it applies to fission gases).** IAEA-TECDOC-1674 (2012),
/// *Advances in High Temperature Gas Cooled Reactor Fuel Technology*
/// (the CRP-6 report), section 10.2.2 "Diffusion coefficients", printed
/// p. 538: the diffusive transport of "fission products (caesium,
/// strontium, silver, fission gases)" uses "the diffusion coefficients for
/// 'Germany' as listed in [1]", where [1] is IAEA-TECDOC-978. Its Table 10.5
/// (p. 543) asks for Kr-85 through a whole TRISO particle (cases 3e, 4d).
///
/// **Not a measurement.** It is a modelling convention ("no significant
/// retention"); TECDOC-978 gives the US recommendation as `1e-10 m^2/s` in
/// the same sentence and calls the difference negligible, because both far
/// exceed the dense layers' coefficients. The German value is used for
/// consistency with Ag, Cs and Sr here (`Buffer => 1e-8`) and with CRP-6.
pub const KR_BUFFER_D_M2_PER_S: f64 = 1e-8;

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
//
// Buffer: `1e-8 m^2/s` (with `Q1 = 0`, `D2 = 0`, `Q2 = 0`), the German
// (FRG) buffer value, cited in [`KR_BUFFER_D_M2_PER_S`] (gh:#638, maintainer
// decision 2026-11-10 "Find a cited value"). It is NOT from Jiang et al.
//
// # Panics
// ~~For the buffer and both graphite variants~~ **CORRECTED 2026-11-10
// (gh:#638):** for both graphite variants only (`todo!()`, no cited value
// in the German set; see [`KR_BUFFER_D_M2_PER_S`]).
// [`super::get_diffusion_coeff_jiang`] does not call it there; it returns
// `MissingDiffusionData` instead (gh:#638).
#[inline]
pub fn get_d1_for_kr(
    triso_layer: TrisoPebbleLayerMaterial,
    temperature: ThermodynamicTemperature,
) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => {
            let (a, b, c) = (1.3e-12, 8.8e-15, 700.0);

            let d_below_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(a);
            let d_above_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_d_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::PyC => 2.9e-8,
        TrisoPebbleLayerMaterial::SiC => {
            let (a, b, c) = (8.6e-10, 3.7e1, 1353.0);

            let d_below_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(a);
            let d_above_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_d_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 1e-6,
        TrisoPebbleLayerMaterial::Buffer => KR_BUFFER_D_M2_PER_S,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
//
// Buffer: `Q1 = 0` (no temperature dependence), from the same German
// buffer convention as [`KR_BUFFER_D_M2_PER_S`], not from Jiang et al.
#[inline]
pub fn get_q1_for_kr(
    triso_layer: TrisoPebbleLayerMaterial,
    temperature: ThermodynamicTemperature,
) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => {
            let (a, b, c) = (126.0, 54.0, 700.0);

            let d_below_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(a);
            let d_above_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_q_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::PyC => 291.0,
        TrisoPebbleLayerMaterial::SiC => {
            let (a, b, c) = (326.0, 657.0, 1353.0);

            let d_below_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(a);
            let d_above_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_q_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
//
// Buffer: `D2 = 0`, the German buffer convention has a single,
// temperature-independent term (see [`KR_BUFFER_D_M2_PER_S`]).
//
// # Panics
// ~~For the buffer and both graphite variants~~ **CORRECTED 2026-11-10
// (gh:#638):** for both graphite variants only (`todo!()`, no cited value).
// [`super::get_diffusion_coeff_jiang`] does not call it there; it returns
// `MissingDiffusionData` instead (gh:#638).
#[inline]
pub fn get_d2_for_kr(
    triso_layer: TrisoPebbleLayerMaterial,
    temperature: ThermodynamicTemperature,
) -> DiffusionCoefficient {
    let coeff_m2_per_s: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => {
            let (a, b, c) = (0.0, 6e-1, 700.0);

            let d_below_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(a);
            let d_above_threshold_temperature =
                DiffusionCoefficient::new::<square_meter_per_second>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_d_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::PyC => 2e5,
        TrisoPebbleLayerMaterial::SiC => 0.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return DiffusionCoefficient::new::<square_meter_per_second>(coeff_m2_per_s);
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
//
// Buffer: `Q2 = 0` (the second term is zero; see
// [`KR_BUFFER_D_M2_PER_S`]).
//
// # Panics
// ~~For the buffer and both graphite variants~~ **CORRECTED 2026-11-10
// (gh:#638):** for both graphite variants only (`todo!()`, no cited value).
// [`super::get_diffusion_coeff_jiang`] does not call it there; it returns
// `MissingDiffusionData` instead (gh:#638).
#[inline]
pub fn get_q2_for_kr(
    triso_layer: TrisoPebbleLayerMaterial,
    temperature: ThermodynamicTemperature,
) -> MolarEnergy {
    let coeff_kj_per_mol: f64 = match triso_layer {
        TrisoPebbleLayerMaterial::KernelUO2 => {
            let (a, b, c) = (0.0, 480.0, 700.0);

            let d_below_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(a);
            let d_above_threshold_temperature = MolarEnergy::new::<kilojoule_per_mole>(b);

            let threshold_temperature = ThermodynamicTemperature::new::<degree_celsius>(c);

            return get_s_for_q_in_krypton(
                d_below_threshold_temperature,
                d_above_threshold_temperature,
                threshold_temperature,
                temperature,
            );
        }
        TrisoPebbleLayerMaterial::PyC => 923.0,
        TrisoPebbleLayerMaterial::SiC => 0.0,
        TrisoPebbleLayerMaterial::MatrixGraphite => todo!(),
        TrisoPebbleLayerMaterial::StructuralGraphite => todo!(),
        TrisoPebbleLayerMaterial::CrackedMaterial => 0.0,
        TrisoPebbleLayerMaterial::Buffer => 0.0,
    };

    return MolarEnergy::new::<kilojoule_per_mole>(coeff_kj_per_mol);
}
// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
/// see table 2.10 function
/// s(a,b,c)
/// s(a, b, c) gives a if temperature
/// is less than c (°C) and b otherwise.
#[inline]
fn get_s_for_d_in_krypton(
    d_below_threshold_temperature: DiffusionCoefficient,
    d_above_threshold_temperature: DiffusionCoefficient,
    threshold_temperature: ThermodynamicTemperature,
    temperature: ThermodynamicTemperature,
) -> DiffusionCoefficient {
    if temperature < threshold_temperature {
        return d_below_threshold_temperature;
    }

    return d_above_threshold_temperature;
}

// from Jiang 2023
// Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
// Novascone, S. R. (2023). Fission product transport in TRISO particles
// and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
// Idaho Falls, ID (United States).
//
// table on page 13 of 105
/// see table 2.10 function
/// s(a,b,c)
/// s(a, b, c) gives a if temperature
/// is less than c (°C) and b otherwise.
#[inline]
fn get_s_for_q_in_krypton(
    q_below_threshold_temperature: MolarEnergy,
    q_above_threshold_temperature: MolarEnergy,
    threshold_temperature: ThermodynamicTemperature,
    temperature: ThermodynamicTemperature,
) -> MolarEnergy {
    if temperature < threshold_temperature {
        return q_below_threshold_temperature;
    }

    return q_above_threshold_temperature;
}

/// https://inldigitallibrary.inl.gov/sites/sti/sti/7245704.pdf
///
/// Data is obtained using GraphReader for plots from:
///
///
/// Collin, B. P. (2016). Diffusivities of Ag, Cs, Sr, and
/// Kr in TRISO fuel particles and graphite (No. INL/EXT-16-39548).
/// Idaho National Lab.(INL), Idaho Falls, ID (United States).
///
/// This is to ensure that values of the triso are reasonable
///
/// This is largely vibe coded
#[cfg(test)]
mod cesium_tests;

/// https://inldigitallibrary.inl.gov/sites/sti/sti/7245704.pdf
///
/// Data is obtained using GraphReader for plots from:
///
///
/// Collin, B. P. (2016). Diffusivities of Ag, Cs, Sr, and
/// Kr in TRISO fuel particles and graphite (No. INL/EXT-16-39548).
/// Idaho National Lab.(INL), Idaho Falls, ID (United States).
///
/// This is to ensure that values of the triso are reasonable
///
/// This is largely vibe coded
#[cfg(test)]
mod strontium_tests;

/// https://inldigitallibrary.inl.gov/sites/sti/sti/7245704.pdf
///
/// Data is obtained using GraphReader for plots from:
///
///
/// Collin, B. P. (2016). Diffusivities of Ag, Cs, Sr, and
/// Kr in TRISO fuel particles and graphite (No. INL/EXT-16-39548).
/// Idaho National Lab.(INL), Idaho Falls, ID (United States).
///
/// This is to ensure that values of the triso are reasonable
///
/// This is largely vibe coded
#[cfg(test)]
mod silver_tests;
