//! The fluid selector — an **enum**, matching each fluid to its hardcoded
//! [`FluidEos`]. This is the enum-dispatch replacement for CoolProp's
//! string-keyed fluid lookup / backend polymorphism: adding a fluid is a new
//! variant, and every `match` on `Fluid` becomes exhaustive.
//!
//! All of CoolProp's pure fluids are wired in. Variant names follow the
//! CoolProp fluid name with non-alphanumerics removed (e.g. `n-Heptane` →
//! `NHeptane`, `R1234ze(E)` → `R1234zeE`); [`Fluid::name`] returns the
//! original CoolProp name.

use crate::ancillaries::FluidAncillaries;
use crate::eos::FluidEos;
use crate::fluids;
use crate::transport::FluidTransport;

/// A supported pure fluid (one per CoolProp pure-fluid EOS). Each variant maps
/// to a hardcoded `const` [`FluidEos`] via [`Fluid::eos`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
pub enum Fluid {
    /// CoolProp `Acetone`.
    Acetone,
    /// CoolProp `Air`.
    Air,
    /// CoolProp `Ammonia`.
    Ammonia,
    /// CoolProp `Argon`.
    Argon,
    /// CoolProp `Benzene`.
    Benzene,
    /// CoolProp `CarbonDioxide`.
    CarbonDioxide,
    /// CoolProp `CarbonMonoxide`.
    CarbonMonoxide,
    /// CoolProp `CarbonylSulfide`.
    CarbonylSulfide,
    /// CoolProp `Chlorine`.
    Chlorine,
    /// CoolProp `cis-2-Butene`.
    Cis2Butene,
    /// CoolProp `CycloHexane`.
    CycloHexane,
    /// CoolProp `CycloPropane`.
    CycloPropane,
    /// CoolProp `Cyclopentane`.
    Cyclopentane,
    /// CoolProp `D4`.
    D4,
    /// CoolProp `D5`.
    D5,
    /// CoolProp `D6`.
    D6,
    /// CoolProp `Deuterium`.
    Deuterium,
    /// CoolProp `Dichloroethane`.
    Dichloroethane,
    /// CoolProp `DiethylEther`.
    DiethylEther,
    /// CoolProp `DimethylCarbonate`.
    DimethylCarbonate,
    /// CoolProp `DimethylEther`.
    DimethylEther,
    /// CoolProp `Ethane`.
    Ethane,
    /// CoolProp `Ethanol`.
    Ethanol,
    /// CoolProp `EthylBenzene`.
    EthylBenzene,
    /// CoolProp `Ethylene`.
    Ethylene,
    /// CoolProp `EthyleneOxide`.
    EthyleneOxide,
    /// CoolProp `1-Butene`.
    F1Butene,
    /// CoolProp `Fluorine`.
    Fluorine,
    /// CoolProp `HFE143m`.
    HFE143m,
    /// CoolProp `HeavyWater`.
    HeavyWater,
    /// Helium (Ortiz-Vega et al.). Power + Gaussian residual only (no
    /// non-analytic term), so it is accurate even at the critical point.
    Helium,
    /// CoolProp `Hydrogen`.
    Hydrogen,
    /// CoolProp `HydrogenChloride`.
    HydrogenChloride,
    /// CoolProp `HydrogenSulfide`.
    HydrogenSulfide,
    /// CoolProp `IsoButane`.
    IsoButane,
    /// CoolProp `IsoButene`.
    IsoButene,
    /// CoolProp `Isohexane`.
    Isohexane,
    /// CoolProp `Isopentane`.
    Isopentane,
    /// CoolProp `Krypton`.
    Krypton,
    /// CoolProp `MD2M`.
    MD2M,
    /// CoolProp `MD3M`.
    MD3M,
    /// CoolProp `MD4M`.
    MD4M,
    /// CoolProp `MDM`.
    MDM,
    /// CoolProp `MM`.
    MM,
    /// CoolProp `m-Xylene`.
    MXylene,
    /// CoolProp `Methane`.
    Methane,
    /// CoolProp `Methanol`.
    Methanol,
    /// CoolProp `MethylLinoleate`.
    MethylLinoleate,
    /// CoolProp `MethylLinolenate`.
    MethylLinolenate,
    /// CoolProp `MethylOleate`.
    MethylOleate,
    /// CoolProp `MethylPalmitate`.
    MethylPalmitate,
    /// CoolProp `MethylStearate`.
    MethylStearate,
    /// CoolProp `n-Butane`.
    NButane,
    /// CoolProp `n-Decane`.
    NDecane,
    /// CoolProp `n-Dodecane`.
    NDodecane,
    /// CoolProp `n-Heptane`.
    NHeptane,
    /// CoolProp `n-Hexane`.
    NHexane,
    /// CoolProp `n-Nonane`.
    NNonane,
    /// CoolProp `n-Octane`.
    NOctane,
    /// CoolProp `n-Pentane`.
    NPentane,
    /// CoolProp `n-Perfluorobutane`.
    NPerfluorobutane,
    /// CoolProp `n-Perfluorohexane`.
    NPerfluorohexane,
    /// CoolProp `n-Perfluoropentane`.
    NPerfluoropentane,
    /// CoolProp `n-Propane`.
    NPropane,
    /// CoolProp `n-Undecane`.
    NUndecane,
    /// CoolProp `Neon`.
    Neon,
    /// CoolProp `Neopentane`.
    Neopentane,
    /// CoolProp `Nitrogen`.
    Nitrogen,
    /// CoolProp `NitrousOxide`.
    NitrousOxide,
    /// CoolProp `Novec649`.
    Novec649,
    /// CoolProp `o-Xylene`.
    OXylene,
    /// CoolProp `OrthoDeuterium`.
    OrthoDeuterium,
    /// CoolProp `OrthoHydrogen`.
    OrthoHydrogen,
    /// CoolProp `Oxygen`.
    Oxygen,
    /// CoolProp `p-Xylene`.
    PXylene,
    /// CoolProp `ParaDeuterium`.
    ParaDeuterium,
    /// CoolProp `ParaHydrogen`.
    ParaHydrogen,
    /// CoolProp `Propylene`.
    Propylene,
    /// CoolProp `PropyleneGlycol`.
    PropyleneGlycol,
    /// CoolProp `Propyne`.
    Propyne,
    /// CoolProp `R11`.
    R11,
    /// CoolProp `R1123`.
    R1123,
    /// CoolProp `R113`.
    R113,
    /// CoolProp `R1130(E)`.
    R1130E,
    /// CoolProp `R1132(E)`.
    R1132E,
    /// CoolProp `R1132a`.
    R1132a,
    /// CoolProp `R114`.
    R114,
    /// CoolProp `R115`.
    R115,
    /// CoolProp `R116`.
    R116,
    /// CoolProp `R12`.
    R12,
    /// CoolProp `R1224yd(Z)`.
    R1224ydZ,
    /// CoolProp `R123`.
    R123,
    /// CoolProp `R1233zd(E)`.
    R1233zdE,
    /// CoolProp `R1234yf`.
    R1234yf,
    /// CoolProp `R1234ze(E)`.
    R1234zeE,
    /// CoolProp `R1234ze(Z)`.
    R1234zeZ,
    /// CoolProp `R124`.
    R124,
    /// CoolProp `R1243zf`.
    R1243zf,
    /// CoolProp `R125`.
    R125,
    /// CoolProp `R13`.
    R13,
    /// CoolProp `R1336mzz(E)`.
    R1336mzzE,
    /// CoolProp `R1336mzz(Z)`.
    R1336mzzZ,
    /// CoolProp `R134a`.
    R134a,
    /// CoolProp `R13I1`.
    R13I1,
    /// CoolProp `R14`.
    R14,
    /// CoolProp `R141b`.
    R141b,
    /// CoolProp `R142b`.
    R142b,
    /// CoolProp `R143a`.
    R143a,
    /// CoolProp `R152A`.
    R152A,
    /// CoolProp `R161`.
    R161,
    /// CoolProp `R21`.
    R21,
    /// CoolProp `R218`.
    R218,
    /// CoolProp `R22`.
    R22,
    /// CoolProp `R227EA`.
    R227EA,
    /// CoolProp `R23`.
    R23,
    /// CoolProp `R236EA`.
    R236EA,
    /// CoolProp `R236FA`.
    R236FA,
    /// CoolProp `R245ca`.
    R245ca,
    /// CoolProp `R245fa`.
    R245fa,
    /// CoolProp `R32`.
    R32,
    /// CoolProp `R365MFC`.
    R365MFC,
    /// CoolProp `R40`.
    R40,
    /// CoolProp `R404A`.
    R404A,
    /// CoolProp `R407C`.
    R407C,
    /// CoolProp `R41`.
    R41,
    /// CoolProp `R410A`.
    R410A,
    /// CoolProp `R507A`.
    R507A,
    /// CoolProp `RC318`.
    RC318,
    /// CoolProp `SES36`.
    SES36,
    /// CoolProp `SulfurDioxide`.
    SulfurDioxide,
    /// CoolProp `SulfurHexafluoride`.
    SulfurHexafluoride,
    /// CoolProp `Tetrahydrofuran`.
    Tetrahydrofuran,
    /// CoolProp `Toluene`.
    Toluene,
    /// CoolProp `trans-2-Butene`.
    Trans2Butene,
    /// CoolProp `VinylChloride`.
    VinylChloride,
    /// Water (IAPWS-95). Its residual includes non-analytic critical-region
    /// terms (currently a no-op stub), so accuracy is degraded within ~1 % of Tc.
    Water,
    /// CoolProp `Xenon`.
    Xenon,
}

impl Fluid {
    /// Every supported fluid, for enumeration (137 in total).
    pub const ALL: &'static [Fluid] = &[
        Fluid::Acetone,
        Fluid::Air,
        Fluid::Ammonia,
        Fluid::Argon,
        Fluid::Benzene,
        Fluid::CarbonDioxide,
        Fluid::CarbonMonoxide,
        Fluid::CarbonylSulfide,
        Fluid::Chlorine,
        Fluid::Cis2Butene,
        Fluid::CycloHexane,
        Fluid::CycloPropane,
        Fluid::Cyclopentane,
        Fluid::D4,
        Fluid::D5,
        Fluid::D6,
        Fluid::Deuterium,
        Fluid::Dichloroethane,
        Fluid::DiethylEther,
        Fluid::DimethylCarbonate,
        Fluid::DimethylEther,
        Fluid::Ethane,
        Fluid::Ethanol,
        Fluid::EthylBenzene,
        Fluid::Ethylene,
        Fluid::EthyleneOxide,
        Fluid::F1Butene,
        Fluid::Fluorine,
        Fluid::HFE143m,
        Fluid::HeavyWater,
        Fluid::Helium,
        Fluid::Hydrogen,
        Fluid::HydrogenChloride,
        Fluid::HydrogenSulfide,
        Fluid::IsoButane,
        Fluid::IsoButene,
        Fluid::Isohexane,
        Fluid::Isopentane,
        Fluid::Krypton,
        Fluid::MD2M,
        Fluid::MD3M,
        Fluid::MD4M,
        Fluid::MDM,
        Fluid::MM,
        Fluid::MXylene,
        Fluid::Methane,
        Fluid::Methanol,
        Fluid::MethylLinoleate,
        Fluid::MethylLinolenate,
        Fluid::MethylOleate,
        Fluid::MethylPalmitate,
        Fluid::MethylStearate,
        Fluid::NButane,
        Fluid::NDecane,
        Fluid::NDodecane,
        Fluid::NHeptane,
        Fluid::NHexane,
        Fluid::NNonane,
        Fluid::NOctane,
        Fluid::NPentane,
        Fluid::NPerfluorobutane,
        Fluid::NPerfluorohexane,
        Fluid::NPerfluoropentane,
        Fluid::NPropane,
        Fluid::NUndecane,
        Fluid::Neon,
        Fluid::Neopentane,
        Fluid::Nitrogen,
        Fluid::NitrousOxide,
        Fluid::Novec649,
        Fluid::OXylene,
        Fluid::OrthoDeuterium,
        Fluid::OrthoHydrogen,
        Fluid::Oxygen,
        Fluid::PXylene,
        Fluid::ParaDeuterium,
        Fluid::ParaHydrogen,
        Fluid::Propylene,
        Fluid::PropyleneGlycol,
        Fluid::Propyne,
        Fluid::R11,
        Fluid::R1123,
        Fluid::R113,
        Fluid::R1130E,
        Fluid::R1132E,
        Fluid::R1132a,
        Fluid::R114,
        Fluid::R115,
        Fluid::R116,
        Fluid::R12,
        Fluid::R1224ydZ,
        Fluid::R123,
        Fluid::R1233zdE,
        Fluid::R1234yf,
        Fluid::R1234zeE,
        Fluid::R1234zeZ,
        Fluid::R124,
        Fluid::R1243zf,
        Fluid::R125,
        Fluid::R13,
        Fluid::R1336mzzE,
        Fluid::R1336mzzZ,
        Fluid::R134a,
        Fluid::R13I1,
        Fluid::R14,
        Fluid::R141b,
        Fluid::R142b,
        Fluid::R143a,
        Fluid::R152A,
        Fluid::R161,
        Fluid::R21,
        Fluid::R218,
        Fluid::R22,
        Fluid::R227EA,
        Fluid::R23,
        Fluid::R236EA,
        Fluid::R236FA,
        Fluid::R245ca,
        Fluid::R245fa,
        Fluid::R32,
        Fluid::R365MFC,
        Fluid::R40,
        Fluid::R404A,
        Fluid::R407C,
        Fluid::R41,
        Fluid::R410A,
        Fluid::R507A,
        Fluid::RC318,
        Fluid::SES36,
        Fluid::SulfurDioxide,
        Fluid::SulfurHexafluoride,
        Fluid::Tetrahydrofuran,
        Fluid::Toluene,
        Fluid::Trans2Butene,
        Fluid::VinylChloride,
        Fluid::Water,
        Fluid::Xenon,
    ];

    /// The hardcoded Helmholtz EOS for this fluid.
    pub fn eos(self) -> &'static FluidEos {
        match self {
            Fluid::Acetone => &fluids::acetone::ACETONE,
            Fluid::Air => &fluids::air::AIR,
            Fluid::Ammonia => &fluids::ammonia::AMMONIA,
            Fluid::Argon => &fluids::argon::ARGON,
            Fluid::Benzene => &fluids::benzene::BENZENE,
            Fluid::CarbonDioxide => &fluids::carbondioxide::CARBONDIOXIDE,
            Fluid::CarbonMonoxide => &fluids::carbonmonoxide::CARBONMONOXIDE,
            Fluid::CarbonylSulfide => &fluids::carbonylsulfide::CARBONYLSULFIDE,
            Fluid::Chlorine => &fluids::chlorine::CHLORINE,
            Fluid::Cis2Butene => &fluids::cis_2_butene::CIS_2_BUTENE,
            Fluid::CycloHexane => &fluids::cyclohexane::CYCLOHEXANE,
            Fluid::CycloPropane => &fluids::cyclopropane::CYCLOPROPANE,
            Fluid::Cyclopentane => &fluids::cyclopentane::CYCLOPENTANE,
            Fluid::D4 => &fluids::d4::D4,
            Fluid::D5 => &fluids::d5::D5,
            Fluid::D6 => &fluids::d6::D6,
            Fluid::Deuterium => &fluids::deuterium::DEUTERIUM,
            Fluid::Dichloroethane => &fluids::dichloroethane::DICHLOROETHANE,
            Fluid::DiethylEther => &fluids::diethylether::DIETHYLETHER,
            Fluid::DimethylCarbonate => &fluids::dimethylcarbonate::DIMETHYLCARBONATE,
            Fluid::DimethylEther => &fluids::dimethylether::DIMETHYLETHER,
            Fluid::Ethane => &fluids::ethane::ETHANE,
            Fluid::Ethanol => &fluids::ethanol::ETHANOL,
            Fluid::EthylBenzene => &fluids::ethylbenzene::ETHYLBENZENE,
            Fluid::Ethylene => &fluids::ethylene::ETHYLENE,
            Fluid::EthyleneOxide => &fluids::ethyleneoxide::ETHYLENEOXIDE,
            Fluid::F1Butene => &fluids::f_1_butene::F_1_BUTENE,
            Fluid::Fluorine => &fluids::fluorine::FLUORINE,
            Fluid::HFE143m => &fluids::hfe143m::HFE143M,
            Fluid::HeavyWater => &fluids::heavywater::HEAVYWATER,
            Fluid::Helium => &fluids::helium::HELIUM,
            Fluid::Hydrogen => &fluids::hydrogen::HYDROGEN,
            Fluid::HydrogenChloride => &fluids::hydrogenchloride::HYDROGENCHLORIDE,
            Fluid::HydrogenSulfide => &fluids::hydrogensulfide::HYDROGENSULFIDE,
            Fluid::IsoButane => &fluids::isobutane::ISOBUTANE,
            Fluid::IsoButene => &fluids::isobutene::ISOBUTENE,
            Fluid::Isohexane => &fluids::isohexane::ISOHEXANE,
            Fluid::Isopentane => &fluids::isopentane::ISOPENTANE,
            Fluid::Krypton => &fluids::krypton::KRYPTON,
            Fluid::MD2M => &fluids::md2m::MD2M,
            Fluid::MD3M => &fluids::md3m::MD3M,
            Fluid::MD4M => &fluids::md4m::MD4M,
            Fluid::MDM => &fluids::mdm::MDM,
            Fluid::MM => &fluids::mm::MM,
            Fluid::MXylene => &fluids::m_xylene::M_XYLENE,
            Fluid::Methane => &fluids::methane::METHANE,
            Fluid::Methanol => &fluids::methanol::METHANOL,
            Fluid::MethylLinoleate => &fluids::methyllinoleate::METHYLLINOLEATE,
            Fluid::MethylLinolenate => &fluids::methyllinolenate::METHYLLINOLENATE,
            Fluid::MethylOleate => &fluids::methyloleate::METHYLOLEATE,
            Fluid::MethylPalmitate => &fluids::methylpalmitate::METHYLPALMITATE,
            Fluid::MethylStearate => &fluids::methylstearate::METHYLSTEARATE,
            Fluid::NButane => &fluids::n_butane::N_BUTANE,
            Fluid::NDecane => &fluids::n_decane::N_DECANE,
            Fluid::NDodecane => &fluids::n_dodecane::N_DODECANE,
            Fluid::NHeptane => &fluids::n_heptane::N_HEPTANE,
            Fluid::NHexane => &fluids::n_hexane::N_HEXANE,
            Fluid::NNonane => &fluids::n_nonane::N_NONANE,
            Fluid::NOctane => &fluids::n_octane::N_OCTANE,
            Fluid::NPentane => &fluids::n_pentane::N_PENTANE,
            Fluid::NPerfluorobutane => &fluids::n_perfluorobutane::N_PERFLUOROBUTANE,
            Fluid::NPerfluorohexane => &fluids::n_perfluorohexane::N_PERFLUOROHEXANE,
            Fluid::NPerfluoropentane => &fluids::n_perfluoropentane::N_PERFLUOROPENTANE,
            Fluid::NPropane => &fluids::n_propane::N_PROPANE,
            Fluid::NUndecane => &fluids::n_undecane::N_UNDECANE,
            Fluid::Neon => &fluids::neon::NEON,
            Fluid::Neopentane => &fluids::neopentane::NEOPENTANE,
            Fluid::Nitrogen => &fluids::nitrogen::NITROGEN,
            Fluid::NitrousOxide => &fluids::nitrousoxide::NITROUSOXIDE,
            Fluid::Novec649 => &fluids::novec649::NOVEC649,
            Fluid::OXylene => &fluids::o_xylene::O_XYLENE,
            Fluid::OrthoDeuterium => &fluids::orthodeuterium::ORTHODEUTERIUM,
            Fluid::OrthoHydrogen => &fluids::orthohydrogen::ORTHOHYDROGEN,
            Fluid::Oxygen => &fluids::oxygen::OXYGEN,
            Fluid::PXylene => &fluids::p_xylene::P_XYLENE,
            Fluid::ParaDeuterium => &fluids::paradeuterium::PARADEUTERIUM,
            Fluid::ParaHydrogen => &fluids::parahydrogen::PARAHYDROGEN,
            Fluid::Propylene => &fluids::propylene::PROPYLENE,
            Fluid::PropyleneGlycol => &fluids::propyleneglycol::PROPYLENEGLYCOL,
            Fluid::Propyne => &fluids::propyne::PROPYNE,
            Fluid::R11 => &fluids::r11::R11,
            Fluid::R1123 => &fluids::r1123::R1123,
            Fluid::R113 => &fluids::r113::R113,
            Fluid::R1130E => &fluids::r1130_e_::R1130_E_,
            Fluid::R1132E => &fluids::r1132_e_::R1132_E_,
            Fluid::R1132a => &fluids::r1132a::R1132A,
            Fluid::R114 => &fluids::r114::R114,
            Fluid::R115 => &fluids::r115::R115,
            Fluid::R116 => &fluids::r116::R116,
            Fluid::R12 => &fluids::r12::R12,
            Fluid::R1224ydZ => &fluids::r1224yd_z_::R1224YD_Z_,
            Fluid::R123 => &fluids::r123::R123,
            Fluid::R1233zdE => &fluids::r1233zd_e_::R1233ZD_E_,
            Fluid::R1234yf => &fluids::r1234yf::R1234YF,
            Fluid::R1234zeE => &fluids::r1234ze_e_::R1234ZE_E_,
            Fluid::R1234zeZ => &fluids::r1234ze_z_::R1234ZE_Z_,
            Fluid::R124 => &fluids::r124::R124,
            Fluid::R1243zf => &fluids::r1243zf::R1243ZF,
            Fluid::R125 => &fluids::r125::R125,
            Fluid::R13 => &fluids::r13::R13,
            Fluid::R1336mzzE => &fluids::r1336mzz_e_::R1336MZZ_E_,
            Fluid::R1336mzzZ => &fluids::r1336mzz_z_::R1336MZZ_Z_,
            Fluid::R134a => &fluids::r134a::R134A,
            Fluid::R13I1 => &fluids::r13i1::R13I1,
            Fluid::R14 => &fluids::r14::R14,
            Fluid::R141b => &fluids::r141b::R141B,
            Fluid::R142b => &fluids::r142b::R142B,
            Fluid::R143a => &fluids::r143a::R143A,
            Fluid::R152A => &fluids::r152a::R152A,
            Fluid::R161 => &fluids::r161::R161,
            Fluid::R21 => &fluids::r21::R21,
            Fluid::R218 => &fluids::r218::R218,
            Fluid::R22 => &fluids::r22::R22,
            Fluid::R227EA => &fluids::r227ea::R227EA,
            Fluid::R23 => &fluids::r23::R23,
            Fluid::R236EA => &fluids::r236ea::R236EA,
            Fluid::R236FA => &fluids::r236fa::R236FA,
            Fluid::R245ca => &fluids::r245ca::R245CA,
            Fluid::R245fa => &fluids::r245fa::R245FA,
            Fluid::R32 => &fluids::r32::R32,
            Fluid::R365MFC => &fluids::r365mfc::R365MFC,
            Fluid::R40 => &fluids::r40::R40,
            Fluid::R404A => &fluids::r404a::R404A,
            Fluid::R407C => &fluids::r407c::R407C,
            Fluid::R41 => &fluids::r41::R41,
            Fluid::R410A => &fluids::r410a::R410A,
            Fluid::R507A => &fluids::r507a::R507A,
            Fluid::RC318 => &fluids::rc318::RC318,
            Fluid::SES36 => &fluids::ses36::SES36,
            Fluid::SulfurDioxide => &fluids::sulfurdioxide::SULFURDIOXIDE,
            Fluid::SulfurHexafluoride => &fluids::sulfurhexafluoride::SULFURHEXAFLUORIDE,
            Fluid::Tetrahydrofuran => &fluids::tetrahydrofuran::TETRAHYDROFURAN,
            Fluid::Toluene => &fluids::toluene::TOLUENE,
            Fluid::Trans2Butene => &fluids::trans_2_butene::TRANS_2_BUTENE,
            Fluid::VinylChloride => &fluids::vinylchloride::VINYLCHLORIDE,
            Fluid::Water => &fluids::water::WATER,
            Fluid::Xenon => &fluids::xenon::XENON,
        }
    }

    /// Saturation ancillaries (for VLE / saturation lookups), if this fluid
    /// ships them (131 of 137 do).
    pub fn ancillaries(self) -> Option<&'static FluidAncillaries> {
        match self {
            Fluid::Acetone => Some(&fluids::acetone::ACETONE_ANCILLARIES),
            Fluid::Ammonia => Some(&fluids::ammonia::AMMONIA_ANCILLARIES),
            Fluid::Argon => Some(&fluids::argon::ARGON_ANCILLARIES),
            Fluid::Benzene => Some(&fluids::benzene::BENZENE_ANCILLARIES),
            Fluid::CarbonDioxide => Some(&fluids::carbondioxide::CARBONDIOXIDE_ANCILLARIES),
            Fluid::CarbonMonoxide => Some(&fluids::carbonmonoxide::CARBONMONOXIDE_ANCILLARIES),
            Fluid::CarbonylSulfide => Some(&fluids::carbonylsulfide::CARBONYLSULFIDE_ANCILLARIES),
            Fluid::Chlorine => Some(&fluids::chlorine::CHLORINE_ANCILLARIES),
            Fluid::Cis2Butene => Some(&fluids::cis_2_butene::CIS_2_BUTENE_ANCILLARIES),
            Fluid::CycloHexane => Some(&fluids::cyclohexane::CYCLOHEXANE_ANCILLARIES),
            Fluid::CycloPropane => Some(&fluids::cyclopropane::CYCLOPROPANE_ANCILLARIES),
            Fluid::Cyclopentane => Some(&fluids::cyclopentane::CYCLOPENTANE_ANCILLARIES),
            Fluid::D4 => Some(&fluids::d4::D4_ANCILLARIES),
            Fluid::D5 => Some(&fluids::d5::D5_ANCILLARIES),
            Fluid::D6 => Some(&fluids::d6::D6_ANCILLARIES),
            Fluid::Deuterium => Some(&fluids::deuterium::DEUTERIUM_ANCILLARIES),
            Fluid::Dichloroethane => Some(&fluids::dichloroethane::DICHLOROETHANE_ANCILLARIES),
            Fluid::DiethylEther => Some(&fluids::diethylether::DIETHYLETHER_ANCILLARIES),
            Fluid::DimethylCarbonate => Some(&fluids::dimethylcarbonate::DIMETHYLCARBONATE_ANCILLARIES),
            Fluid::DimethylEther => Some(&fluids::dimethylether::DIMETHYLETHER_ANCILLARIES),
            Fluid::Ethane => Some(&fluids::ethane::ETHANE_ANCILLARIES),
            Fluid::Ethanol => Some(&fluids::ethanol::ETHANOL_ANCILLARIES),
            Fluid::EthylBenzene => Some(&fluids::ethylbenzene::ETHYLBENZENE_ANCILLARIES),
            Fluid::Ethylene => Some(&fluids::ethylene::ETHYLENE_ANCILLARIES),
            Fluid::EthyleneOxide => Some(&fluids::ethyleneoxide::ETHYLENEOXIDE_ANCILLARIES),
            Fluid::F1Butene => Some(&fluids::f_1_butene::F_1_BUTENE_ANCILLARIES),
            Fluid::Fluorine => Some(&fluids::fluorine::FLUORINE_ANCILLARIES),
            Fluid::HFE143m => Some(&fluids::hfe143m::HFE143M_ANCILLARIES),
            Fluid::HeavyWater => Some(&fluids::heavywater::HEAVYWATER_ANCILLARIES),
            Fluid::Helium => Some(&fluids::helium::HELIUM_ANCILLARIES),
            Fluid::Hydrogen => Some(&fluids::hydrogen::HYDROGEN_ANCILLARIES),
            Fluid::HydrogenChloride => Some(&fluids::hydrogenchloride::HYDROGENCHLORIDE_ANCILLARIES),
            Fluid::HydrogenSulfide => Some(&fluids::hydrogensulfide::HYDROGENSULFIDE_ANCILLARIES),
            Fluid::IsoButane => Some(&fluids::isobutane::ISOBUTANE_ANCILLARIES),
            Fluid::IsoButene => Some(&fluids::isobutene::ISOBUTENE_ANCILLARIES),
            Fluid::Isohexane => Some(&fluids::isohexane::ISOHEXANE_ANCILLARIES),
            Fluid::Isopentane => Some(&fluids::isopentane::ISOPENTANE_ANCILLARIES),
            Fluid::Krypton => Some(&fluids::krypton::KRYPTON_ANCILLARIES),
            Fluid::MD2M => Some(&fluids::md2m::MD2M_ANCILLARIES),
            Fluid::MD3M => Some(&fluids::md3m::MD3M_ANCILLARIES),
            Fluid::MD4M => Some(&fluids::md4m::MD4M_ANCILLARIES),
            Fluid::MDM => Some(&fluids::mdm::MDM_ANCILLARIES),
            Fluid::MM => Some(&fluids::mm::MM_ANCILLARIES),
            Fluid::MXylene => Some(&fluids::m_xylene::M_XYLENE_ANCILLARIES),
            Fluid::Methane => Some(&fluids::methane::METHANE_ANCILLARIES),
            Fluid::Methanol => Some(&fluids::methanol::METHANOL_ANCILLARIES),
            Fluid::MethylLinoleate => Some(&fluids::methyllinoleate::METHYLLINOLEATE_ANCILLARIES),
            Fluid::MethylLinolenate => Some(&fluids::methyllinolenate::METHYLLINOLENATE_ANCILLARIES),
            Fluid::MethylOleate => Some(&fluids::methyloleate::METHYLOLEATE_ANCILLARIES),
            Fluid::MethylPalmitate => Some(&fluids::methylpalmitate::METHYLPALMITATE_ANCILLARIES),
            Fluid::MethylStearate => Some(&fluids::methylstearate::METHYLSTEARATE_ANCILLARIES),
            Fluid::NButane => Some(&fluids::n_butane::N_BUTANE_ANCILLARIES),
            Fluid::NDecane => Some(&fluids::n_decane::N_DECANE_ANCILLARIES),
            Fluid::NDodecane => Some(&fluids::n_dodecane::N_DODECANE_ANCILLARIES),
            Fluid::NHeptane => Some(&fluids::n_heptane::N_HEPTANE_ANCILLARIES),
            Fluid::NHexane => Some(&fluids::n_hexane::N_HEXANE_ANCILLARIES),
            Fluid::NNonane => Some(&fluids::n_nonane::N_NONANE_ANCILLARIES),
            Fluid::NOctane => Some(&fluids::n_octane::N_OCTANE_ANCILLARIES),
            Fluid::NPentane => Some(&fluids::n_pentane::N_PENTANE_ANCILLARIES),
            Fluid::NPerfluorobutane => Some(&fluids::n_perfluorobutane::N_PERFLUOROBUTANE_ANCILLARIES),
            Fluid::NPerfluorohexane => Some(&fluids::n_perfluorohexane::N_PERFLUOROHEXANE_ANCILLARIES),
            Fluid::NPerfluoropentane => Some(&fluids::n_perfluoropentane::N_PERFLUOROPENTANE_ANCILLARIES),
            Fluid::NPropane => Some(&fluids::n_propane::N_PROPANE_ANCILLARIES),
            Fluid::NUndecane => Some(&fluids::n_undecane::N_UNDECANE_ANCILLARIES),
            Fluid::Neon => Some(&fluids::neon::NEON_ANCILLARIES),
            Fluid::Neopentane => Some(&fluids::neopentane::NEOPENTANE_ANCILLARIES),
            Fluid::Nitrogen => Some(&fluids::nitrogen::NITROGEN_ANCILLARIES),
            Fluid::NitrousOxide => Some(&fluids::nitrousoxide::NITROUSOXIDE_ANCILLARIES),
            Fluid::Novec649 => Some(&fluids::novec649::NOVEC649_ANCILLARIES),
            Fluid::OXylene => Some(&fluids::o_xylene::O_XYLENE_ANCILLARIES),
            Fluid::OrthoDeuterium => Some(&fluids::orthodeuterium::ORTHODEUTERIUM_ANCILLARIES),
            Fluid::OrthoHydrogen => Some(&fluids::orthohydrogen::ORTHOHYDROGEN_ANCILLARIES),
            Fluid::Oxygen => Some(&fluids::oxygen::OXYGEN_ANCILLARIES),
            Fluid::PXylene => Some(&fluids::p_xylene::P_XYLENE_ANCILLARIES),
            Fluid::ParaDeuterium => Some(&fluids::paradeuterium::PARADEUTERIUM_ANCILLARIES),
            Fluid::ParaHydrogen => Some(&fluids::parahydrogen::PARAHYDROGEN_ANCILLARIES),
            Fluid::Propylene => Some(&fluids::propylene::PROPYLENE_ANCILLARIES),
            Fluid::PropyleneGlycol => Some(&fluids::propyleneglycol::PROPYLENEGLYCOL_ANCILLARIES),
            Fluid::Propyne => Some(&fluids::propyne::PROPYNE_ANCILLARIES),
            Fluid::R11 => Some(&fluids::r11::R11_ANCILLARIES),
            Fluid::R1123 => Some(&fluids::r1123::R1123_ANCILLARIES),
            Fluid::R113 => Some(&fluids::r113::R113_ANCILLARIES),
            Fluid::R1130E => Some(&fluids::r1130_e_::R1130_E__ANCILLARIES),
            Fluid::R1132E => Some(&fluids::r1132_e_::R1132_E__ANCILLARIES),
            Fluid::R1132a => Some(&fluids::r1132a::R1132A_ANCILLARIES),
            Fluid::R114 => Some(&fluids::r114::R114_ANCILLARIES),
            Fluid::R115 => Some(&fluids::r115::R115_ANCILLARIES),
            Fluid::R116 => Some(&fluids::r116::R116_ANCILLARIES),
            Fluid::R12 => Some(&fluids::r12::R12_ANCILLARIES),
            Fluid::R1224ydZ => Some(&fluids::r1224yd_z_::R1224YD_Z__ANCILLARIES),
            Fluid::R123 => Some(&fluids::r123::R123_ANCILLARIES),
            Fluid::R1233zdE => Some(&fluids::r1233zd_e_::R1233ZD_E__ANCILLARIES),
            Fluid::R1234yf => Some(&fluids::r1234yf::R1234YF_ANCILLARIES),
            Fluid::R1234zeE => Some(&fluids::r1234ze_e_::R1234ZE_E__ANCILLARIES),
            Fluid::R1234zeZ => Some(&fluids::r1234ze_z_::R1234ZE_Z__ANCILLARIES),
            Fluid::R124 => Some(&fluids::r124::R124_ANCILLARIES),
            Fluid::R1243zf => Some(&fluids::r1243zf::R1243ZF_ANCILLARIES),
            Fluid::R125 => Some(&fluids::r125::R125_ANCILLARIES),
            Fluid::R13 => Some(&fluids::r13::R13_ANCILLARIES),
            Fluid::R1336mzzE => Some(&fluids::r1336mzz_e_::R1336MZZ_E__ANCILLARIES),
            Fluid::R1336mzzZ => Some(&fluids::r1336mzz_z_::R1336MZZ_Z__ANCILLARIES),
            Fluid::R134a => Some(&fluids::r134a::R134A_ANCILLARIES),
            Fluid::R13I1 => Some(&fluids::r13i1::R13I1_ANCILLARIES),
            Fluid::R14 => Some(&fluids::r14::R14_ANCILLARIES),
            Fluid::R141b => Some(&fluids::r141b::R141B_ANCILLARIES),
            Fluid::R142b => Some(&fluids::r142b::R142B_ANCILLARIES),
            Fluid::R143a => Some(&fluids::r143a::R143A_ANCILLARIES),
            Fluid::R152A => Some(&fluids::r152a::R152A_ANCILLARIES),
            Fluid::R161 => Some(&fluids::r161::R161_ANCILLARIES),
            Fluid::R21 => Some(&fluids::r21::R21_ANCILLARIES),
            Fluid::R218 => Some(&fluids::r218::R218_ANCILLARIES),
            Fluid::R22 => Some(&fluids::r22::R22_ANCILLARIES),
            Fluid::R227EA => Some(&fluids::r227ea::R227EA_ANCILLARIES),
            Fluid::R23 => Some(&fluids::r23::R23_ANCILLARIES),
            Fluid::R236EA => Some(&fluids::r236ea::R236EA_ANCILLARIES),
            Fluid::R236FA => Some(&fluids::r236fa::R236FA_ANCILLARIES),
            Fluid::R245ca => Some(&fluids::r245ca::R245CA_ANCILLARIES),
            Fluid::R245fa => Some(&fluids::r245fa::R245FA_ANCILLARIES),
            Fluid::R32 => Some(&fluids::r32::R32_ANCILLARIES),
            Fluid::R365MFC => Some(&fluids::r365mfc::R365MFC_ANCILLARIES),
            Fluid::R40 => Some(&fluids::r40::R40_ANCILLARIES),
            Fluid::R41 => Some(&fluids::r41::R41_ANCILLARIES),
            Fluid::RC318 => Some(&fluids::rc318::RC318_ANCILLARIES),
            Fluid::SulfurDioxide => Some(&fluids::sulfurdioxide::SULFURDIOXIDE_ANCILLARIES),
            Fluid::SulfurHexafluoride => Some(&fluids::sulfurhexafluoride::SULFURHEXAFLUORIDE_ANCILLARIES),
            Fluid::Tetrahydrofuran => Some(&fluids::tetrahydrofuran::TETRAHYDROFURAN_ANCILLARIES),
            Fluid::Toluene => Some(&fluids::toluene::TOLUENE_ANCILLARIES),
            Fluid::Trans2Butene => Some(&fluids::trans_2_butene::TRANS_2_BUTENE_ANCILLARIES),
            Fluid::VinylChloride => Some(&fluids::vinylchloride::VINYLCHLORIDE_ANCILLARIES),
            Fluid::Water => Some(&fluids::water::WATER_ANCILLARIES),
            Fluid::Xenon => Some(&fluids::xenon::XENON_ANCILLARIES),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Transport models (viscosity/conductivity), if this fluid has a
    /// supported model (42 of 137 do; the rest are hardcoded or
    /// unimplemented — see `crate::transport`).
    pub fn transport(self) -> Option<&'static FluidTransport> {
        match self {
            Fluid::Air => Some(&fluids::air::AIR_TRANSPORT),
            Fluid::Ammonia => Some(&fluids::ammonia::AMMONIA_TRANSPORT),
            Fluid::Argon => Some(&fluids::argon::ARGON_TRANSPORT),
            Fluid::Benzene => Some(&fluids::benzene::BENZENE_TRANSPORT),
            Fluid::CarbonDioxide => Some(&fluids::carbondioxide::CARBONDIOXIDE_TRANSPORT),
            Fluid::Cyclopentane => Some(&fluids::cyclopentane::CYCLOPENTANE_TRANSPORT),
            Fluid::DimethylEther => Some(&fluids::dimethylether::DIMETHYLETHER_TRANSPORT),
            Fluid::Ethanol => Some(&fluids::ethanol::ETHANOL_TRANSPORT),
            Fluid::EthylBenzene => Some(&fluids::ethylbenzene::ETHYLBENZENE_TRANSPORT),
            Fluid::Helium => Some(&fluids::helium::HELIUM_TRANSPORT),
            Fluid::Hydrogen => Some(&fluids::hydrogen::HYDROGEN_TRANSPORT),
            Fluid::IsoButane => Some(&fluids::isobutane::ISOBUTANE_TRANSPORT),
            Fluid::Isopentane => Some(&fluids::isopentane::ISOPENTANE_TRANSPORT),
            Fluid::MXylene => Some(&fluids::m_xylene::M_XYLENE_TRANSPORT),
            Fluid::Methanol => Some(&fluids::methanol::METHANOL_TRANSPORT),
            Fluid::NButane => Some(&fluids::n_butane::N_BUTANE_TRANSPORT),
            Fluid::NDecane => Some(&fluids::n_decane::N_DECANE_TRANSPORT),
            Fluid::NDodecane => Some(&fluids::n_dodecane::N_DODECANE_TRANSPORT),
            Fluid::NHeptane => Some(&fluids::n_heptane::N_HEPTANE_TRANSPORT),
            Fluid::NHexane => Some(&fluids::n_hexane::N_HEXANE_TRANSPORT),
            Fluid::NNonane => Some(&fluids::n_nonane::N_NONANE_TRANSPORT),
            Fluid::NOctane => Some(&fluids::n_octane::N_OCTANE_TRANSPORT),
            Fluid::NPentane => Some(&fluids::n_pentane::N_PENTANE_TRANSPORT),
            Fluid::NPropane => Some(&fluids::n_propane::N_PROPANE_TRANSPORT),
            Fluid::Nitrogen => Some(&fluids::nitrogen::NITROGEN_TRANSPORT),
            Fluid::OXylene => Some(&fluids::o_xylene::O_XYLENE_TRANSPORT),
            Fluid::Oxygen => Some(&fluids::oxygen::OXYGEN_TRANSPORT),
            Fluid::PXylene => Some(&fluids::p_xylene::P_XYLENE_TRANSPORT),
            Fluid::ParaHydrogen => Some(&fluids::parahydrogen::PARAHYDROGEN_TRANSPORT),
            Fluid::R123 => Some(&fluids::r123::R123_TRANSPORT),
            Fluid::R1234yf => Some(&fluids::r1234yf::R1234YF_TRANSPORT),
            Fluid::R1234zeE => Some(&fluids::r1234ze_e_::R1234ZE_E__TRANSPORT),
            Fluid::R125 => Some(&fluids::r125::R125_TRANSPORT),
            Fluid::R134a => Some(&fluids::r134a::R134A_TRANSPORT),
            Fluid::R152A => Some(&fluids::r152a::R152A_TRANSPORT),
            Fluid::R404A => Some(&fluids::r404a::R404A_TRANSPORT),
            Fluid::R407C => Some(&fluids::r407c::R407C_TRANSPORT),
            Fluid::R410A => Some(&fluids::r410a::R410A_TRANSPORT),
            Fluid::R507A => Some(&fluids::r507a::R507A_TRANSPORT),
            Fluid::SulfurHexafluoride => Some(&fluids::sulfurhexafluoride::SULFURHEXAFLUORIDE_TRANSPORT),
            Fluid::Toluene => Some(&fluids::toluene::TOLUENE_TRANSPORT),
            Fluid::Water => Some(&fluids::water::WATER_TRANSPORT),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The fluid's name (as in CoolProp).
    pub fn name(self) -> &'static str {
        self.eos().name
    }
}
