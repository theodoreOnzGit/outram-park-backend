//! The Mark I containment on `tampines`'s control-volume network (gh:#1097,
//! epic gh:#1091): drywell, wetwell (torus) and reactor building as
//! [`ControlVolume`]s, the downcomers, vacuum breakers, wetwell vent and
//! drywell head-flange leak as [`FlowPath`]s, the stack and the environment
//! as boundaries. After MELCOR 2.2's CVH/FL packages (Reference Manual §2,
//! §5, §6; cited by section, not quoted; no MELCOR source used).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! ~~**Built alongside [`super::containment::Containment`], not replacing
//! it.** The plant ([`super::Plant`]) still runs the old model and its
//! fixtures are unchanged.~~ **CORRECTED 2026-11-02 (gh:#1099, maintainer:
//! "switch it"):** the plant ([`super::Plant`]) runs this network by
//! default; the old model is the explicit ablation
//! `Mechanisms::without_network_containment()`, and the plant reads the
//! network through [`NetworkResult::to_containment_result`]. Its geometry
//! ~~is not drawn yet~~ is drawn (gh:#1110, 2026-11-02: [`ContainmentNetwork::geometry_svg`],
//! images in `verification_and_validation/bwr_mark1/geometry/`) but not yet
//! checked by a human, so results on it are tentative, and the
//! vent's depressurisation depends on the torus pool's layer count
//! (gh:#1108). [`ContainmentNetwork::step`] takes the same
//! per-step input ([`ContainmentSnapshot`]) and returns comparable outputs
//! ([`NetworkResult`]), so the two can be run side by side
//! (`tests/containment_network_vs_containment.rs`; the evidence is in
//! `verification_and_validation/bwr_mark1/containment_network.md`).
//!
//! ## What is mapped, and what is not (stated)
//!
//! | Snapshot input | Here |
//! |---|---|
//! | relief steam, enthalpy, H2 (at `srv_gas_k` with `relief_h2_enthalpy`, else the pool's temperature) | a source at the quenchers' port in the torus pool (**since gh:#1100** near the torus bottom, into the layer there, the H2 at that layer's temperature) |
//! | RPV leak steam, enthalpy, H2 at `leak_gas_k` | a source in the drywell atmosphere |
//! | `vent_ordered` | the vent valve: open once ordered and the rupture disk has burst (`p_ww ≥ vent_rupture_disk_pa`) |
//! | `rpv_wall_w` | heat into the drywell atmosphere |
//! | `spray_kg_s`, `spray_k`, `spray_exchange` (gh:#1109) | the steam the droplets condense (`containment::spray_condensation`) as a drywell atmosphere steam sink, and the spray with its condensate into the drywell sump |
//! | `pedestal_inflow_kg_s`, `vessel_drain_kg_s` | water into the drywell sump (**since gh:#1100** it starts dry and may boil dry again, as the old pedestal) |
//! | ~~`ex_vessel_w` … the heat into it while it holds more than its initial fill, else into the drywell structures (no cavity, no MCCI, no concrete gases)~~ | **CORRECTED 2026-11-01 (gh:#1100):** see the next rows |
//! | `ex_vessel_w`, `cav_uo2_kg`, `cav_zr_kg`, `cav_debris_k`, `cavity_phases` | sembawang's own [`super::cavity::Cavity`] (MELCOR CAV RM §3, reused, not rewritten): its upward heat into the sump while water lies over the debris (~~the sump beyond its initial fill~~ **CORRECTED 2026-11-01:** while the sump is not dry, [`ControlVolume::is_pool_dry`]), else radiated to the drywell structures; its concrete gases (steam, H2, CO, CO2) into the drywell atmosphere at the ablation temperature ([`NetworkMechanisms::cavity`]) |
//! | `vessel_p_pa` and the debris arriving through water | sembawang's FDI fall (`containment::lpme_fall` (crate-private), MELCOR FDI RM §2.2): its heat into the sump ([`NetworkMechanisms::fdi`]) |
//! | `dw_wall_outer` | sembawang's drywell slab (`containment::dw_slab_step` (crate-private)) behind the lumped structures ([`NetworkMechanisms::dw_slab`]) |
//! | `dw_wall_hs`, `ww_shell` | the drywell structures and the torus shell as lumped walls on the atmospheres, by the same MELCOR surface ([`pool_surface::structure_surface`]) |
//! | `xe_*`, `cs_*`, `i_*`, `uo2_*`, `rn_relief`, `rn_leak` and the scrubbing switches | **since gh:#1100** sembawang's own fission products ([`super::containment_fp::FpHold`], moved out of `Containment` with unchanged arithmetic): the relief flow's scrubbed by SPARC-90's condensation at the quenchers' port, the leaks' into the drywell; each step they ride the gas each path moved (the downcomers' scrubbed by condensation, impaction and the swarm rise; the vacuum breakers', the vent's and the flange's unscrubbed), and every 10 s they deposit on the drywell's floor and structures and on the pool's surface ([`NetworkMechanisms::fission_products`]) |
//! | `q_noble_w`, `q_alkali_w`, `q_halogen_w`, `q_chalcogen_w` | **since gh:#1100** the decay heat of what the network holds, by location as in the old model: airborne in the drywell by `set_atmosphere_heat`, on its structures into the lumped wall, in the wetwell gas, on the pool's surface and scrubbed by `set_pool_heat` on the torus ([`NetworkMechanisms::fp_decay_heat`]) |
//! | `pool_layers` | **since gh:#1100** `tampines`' layered pool on the torus ([`NetworkMechanisms::pool_layers`], [`derived_pool_layers`] layers): the quenchers' steam joins the layer near the bottom, the downcomers' the layer at their exits, the surface exchange and flashing act on ~~the top layer~~ **CORRECTED 2026-11-01:** `tampines`' surface mixed block ([`NetworkMechanisms::surface_mixed_block`]; the top layer alone made the exchange depend on the layer count), an unstable column mixes (the old model's `layer_pool` rule, generalised), a stable one conducts ([`NetworkMechanisms::pool_layer_conduction`]); `t_pool_k` stays the energy mean and `t_pool_top_k` is the surface layer. **Differs from the old model (stated):** the old two layers split once at the downcomers' exits and the pool's share of the fission products' decay heat goes into its top layer (`pool_energy`); here the layers are Lagrangian and ~~`set_pool_heat` shares that heat by mass (`tampines` has no pool heat at an elevation)~~ **CORRECTED 2026-11-01:** that heat goes into the water above the downcomers' exits by `set_pool_heat_at`, as the old model's top layer ([`NetworkMechanisms::fp_pool_heat_above_exits`]) |
//! | `bur_ignition`, `burn_dynamics` (the building's burns) | **since gh:#1100** ([`NetworkMechanisms::burns`]) the building burns by the old model's own BUR code (`containment::bur_ignites`, `bur_step`) on its start-of-step gas, steam and CO2 the diluent; since gh:#1103 the chemistry is sembawang's ([`super::reaction`], moved from `tampines`): the burn becomes `tampines`' species sinks (H2, CO, O2) and sources (H2O, CO2) by `set_atmosphere_species` and the heat of reaction by `set_atmosphere_heat`, once. Always BUR's dynamics (the old ablation's heat-free "explosion" is not expressible). The drywell and torus are inerted and get no oxygen |
//! | ~~burns,~~ (**CORRECTED 2026-11-01**: mapped, row above) ~~`pool_layers`,~~ ~~fission products, aerosols, iodine, decay heat of fission products (`q_*_w`), MCCI, FDI, `dw_wall_outer`~~ | **not mapped**: sembawang's, and not yet on the network (MCCI, FDI and the slab **CORRECTED 2026-11-01**: mapped, rows above; the fission products and their decay heat likewise, **CORRECTED 2026-11-01**) |
//!
//! **How the fission products ride the paths (stated).** A path's mean gas
//! mass over the step (tampines' `FlowTransfer`, positive `from → to`) moves
//! the fraction `m / (M + Σ out)` of its donor's gas, `M` the donor's gas at
//! the end of the step and `Σ out` all the gas it sent out (donor cell, the
//! end-of-step mixture; each donor's paths taken in turn from what is left).
//! The downcomers' composition (for SPARC's non-condensable share) is the
//! drywell's end-of-step mixture, CO, CO2 and O2 counted with the nitrogen as
//! the old model counts them. Liquid crossing a path carries no fission
//! products (the scrubbed inventory stays in the torus). The drywell sump
//! holds none (as the old pedestal). The torus gas's aerosol volume follows
//! the network's atmosphere volume (the pool displaces it); the pool's
//! surface for deposition is the V(z) table's area at the level (711 m²
//! against the old model's 750 m²). Wall condensation counts toward the
//! drywell's diffusiophoresis; the rain-out inside the control volume does
//! not (it is not reported; the old model counts both).
//!
//! ## Geometry: from `params`, with what had to be assumed marked \[A\]
//!
//! One plant datum, the torus bottom at 0 m. Each volume's V(z) table is in
//! that datum, so a path's gravity term `ρ g Δz` is the real one.
//!
//! - **Torus:** a torus of major radius [`NetworkConfig::torus_major_radius_m`]
//!   (15 m \[A\], the same assumed torus as `WW_SHELL_AREA_M2`), its tube
//!   radius chosen so that the vessel holds `wetwell_gas_m3` plus the pool
//!   (1750 m³ at 995 kg/m³, sembawang's mass) at IF97's density: 3.84 m. Its
//!   horizontal section at height `y` is `4πR √(r² − (y − r)²)`
//!   (Pappus), tabulated at 64 points. At the initial level that is 711 m²
//!   against `ww_pool_surface_m2` = 750 m² (sembawang's prismatic pool);
//!   [`TorusShape::Prismatic`] reproduces the latter.
//! - **Downcomer exits and quenchers:** `vent_submergence_pa` below the
//!   initial level at 995 kg/m³ and 9.81 m/s² (sembawang's depth), the
//!   opening the downcomer's diameter (0.515 m). ~~The quenchers' port is
//!   put at the same depth, as sembawang does (\[A\], their geometry is
//!   not held).~~ **CORRECTED 2026-11-01 (gh:#1100):** sembawang does not
//!   put them there: its layered pool sends the relief flow into the deep
//!   layer, "the relief valves' quenchers, near the pool's bottom"
//!   (`Containment::step`; `Mechanisms::pool_layers`), the deep layer being
//!   all the water below the downcomers' exits, heated as one. The
//!   quenchers' port is therefore at the torus bottom
//!   ([`NetworkConfig::quencher_elevation_m`] = 0 m, \[A\]: their geometry
//!   is still not held), so that their heat, rising, mixes the whole column
//!   below the warmer water above, as the old deep layer is mixed; a port
//!   higher up would leave the water beneath it unheated (`tampines`' L2).
//!   The SPARC scrubbing of the relief flow keeps the downcomers' depth
//!   (`FpHold::arrive`, unchanged, as in the old model).
//! - **Torus pool layers:** [`derived_pool_layers`], four on average across
//!   the submergence: 11 for Unit 1 (the initial pool 3.237 m deep, 0.29 m
//!   a layer on average, the bottom layer 0.0–0.57 m where the torus is
//!   narrow).
//! - **Drywell:** a floor of `dw_floor_m2` (200 m² \[A\]) carrying a
//!   prismatic volume of `drywell_volume_m3` (\[A\] the light bulb's shape is
//!   not held) above a sump pit of 2 m² × 1.5 m ~~holding 1 m³ \[A\] (a
//!   control volume needs a pool)~~ (**CORRECTED 2026-11-01:** dry, as the
//!   old pedestal starts, since `tampines`' pools may be empty); the floor at the torus top \[A\]
//!   ([`NetworkConfig::drywell_floor_m`]), the downcomers' entry 2 m above
//!   it \[A\], the head flange at the top.
//! - **Reactor building:** `reactor_building_m3` on a 650 m² floor \[A\]
//!   (40 m tall, so it spans the drywell head) above a 2 m² sump pit,
//!   ~~with 1 m³ \[A\]~~ dry (**CORRECTED 2026-11-01**), dry air at one atmosphere and 303.15 K (sembawang's
//!   `T_RB`), open to the environment through 1 m² \[A\] near its top
//!   (sembawang holds its building at one atmosphere exactly).
//! - **Paths:** sembawang's orifice `C_d A √(2ρΔp)` is the steady state of a
//!   path of area `A`, discharge coefficient `C_d` and form loss `K = 1`,
//!   so each path takes `params`' area and coefficient with `K = 1`; the
//!   inertial lengths are \[A\]: downcomers 8 m, vacuum breakers 1 m, vent
//!   60 m, flange 0.5 m, building openings 1 m. The vent is a check valve
//!   (sembawang's vent never flows back); the vacuum breakers open at
//!   `vacuum_breaker_pa`.

use tampines::control_volume::{AtmosphereSpeciesRates, ControlVolume, ElevatedPoolHeat, GasMixture, NonCondensable, Port, Source, VolumeAltitudeTable};
use tampines::cv_network::{CvNetwork, NetworkBoundary, NetworkControl, NetworkEnd, NetworkPath};
use tampines::flow_path::{FlowPath, FlowPathGeometry, FlowPathKind};
use tampines::TampinesError;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use tampines_steam_tables::region_2_vapour::h_tp_2;
use tampines_steam_tables::region_5_steam_at_800_plus_degc::intensive_properties::h_tp_5;
use uom::si::amount_of_substance::mole;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AmountOfSubstance, Area, AvailableEnergy, Length, Mass, MassRate, Power, Pressure, ThermodynamicTemperature, Time, Volume};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

mod drawing;
pub use drawing::VolumeGeometry;

use super::cavity::Cavity;
use super::containment::{bur_ignites, bur_step, dw_slab_step, lpme_fall, Burn, ContainmentResult, ContainmentSnapshot, DW_SLAB_CELLS};
use super::containment_fp::{DowncomerFlow, FpDecayHeat, FpHold, FpReport, PoolSide, RnContext};
use super::params::*;
use super::pool_surface;
use super::steam::p_sat_pa;

/// sembawang's building temperature (`containment::T_RB`).
const T_RB: f64 = 303.15;
/// sembawang's constant liquid `c_p` (`containment::CP_WATER`), for the
/// structures' condensate.
const CP_WATER: f64 = 4186.0;

/// The torus's V(z).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TorusShape {
    /// A torus (the default).
    Torus,
    /// sembawang's prismatic pool: area `ww_pool_surface_m2` throughout.
    Prismatic,
}

/// What a unit's containment network needs beyond [`Params`]: the shapes
/// and elevations `params` does not hold. Every field is \[A\] for Unit 1
/// (see the module docs); ~~Units 2 and 3 would differ here and in `Params`
/// (the V&V doc lists the data that would be needed).~~ **CORRECTED
/// 2026-11-01 (gh:#1101):** Units 2 and 3 are [`NetworkConfig::unit2`] and
/// [`NetworkConfig::unit3`] with [`Params::unit2`] and [`Params::unit3`];
/// of this struct only the torus's major radius is sourced and the
/// building's floor derived, the rest borrowed from Unit 1 (the V&V doc's
/// table lists each field's mark and what is still missing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkConfig {
    /// The torus's shape.
    pub torus_shape: TorusShape,
    /// Torus major radius, m.
    pub torus_major_radius_m: f64,
    /// Drywell floor above the torus bottom, m (`None`: at the torus top).
    pub drywell_floor_m: Option<f64>,
    /// The downcomers' entry above the drywell floor, m.
    pub downcomer_entry_m: f64,
    /// Sump pits (drywell, building): area, m², depth, m, initial water, m³.
    /// ~~1 m³ \[A\] (a control volume needs a pool)~~ **CORRECTED
    /// 2026-11-01 (gh:#1100):** 0 for Unit 1, both pits start dry, as the
    /// old model's pedestal does (`Containment::pedestal_kg` starts at 0);
    /// `tampines`' pools may now be empty. A positive value is the gh:#1097
    /// configuration, in which the water "over the debris" is what the
    /// drywell sump holds beyond this fill.
    pub sump_area_m2: f64,
    pub sump_depth_m: f64,
    pub sump_water_m3: f64,
    /// The torus pool's layer count (gh:#1100; used with
    /// [`NetworkMechanisms::pool_layers`]): `None` derives it from the
    /// geometry, [`derived_pool_layers`]; `Some(n)` sets it (the resolution
    /// study runs `n` and `2n`).
    pub pool_layers: Option<usize>,
    /// The relief valves' quenchers' port above the torus bottom, m
    /// (gh:#1100): `Some(0.0)` for Unit 1, at the bottom \[A\] (see the
    /// module docs); `None` puts them at the downcomers' exit depth, the
    /// gh:#1097 placement.
    pub quencher_elevation_m: Option<f64>,
    /// Reactor building floor area, m², and its openings to the
    /// environment, m².
    pub building_floor_m2: f64,
    pub building_openings_m2: f64,
    /// Inertial lengths, m: downcomers, vacuum breakers, vent, flange,
    /// building openings.
    pub downcomer_length_m: f64,
    pub vacuum_breaker_length_m: f64,
    pub vent_length_m: f64,
    pub flange_length_m: f64,
    pub building_length_m: f64,
}

impl NetworkConfig {
    /// Unit 1, every value \[A\] (module docs).
    pub fn unit1() -> Self {
        Self {
            torus_shape: TorusShape::Torus,
            torus_major_radius_m: 15.0,
            drywell_floor_m: None,
            downcomer_entry_m: 2.0,
            sump_area_m2: 2.0,
            sump_depth_m: 1.5,
            sump_water_m3: 0.0,
            pool_layers: None,
            quencher_elevation_m: Some(0.0),
            building_floor_m2: 650.0,
            building_openings_m2: 1.0,
            downcomer_length_m: 8.0,
            vacuum_breaker_length_m: 1.0,
            vent_length_m: 60.0,
            flange_length_m: 0.5,
            building_length_m: 1.0,
        }
    }
}

impl NetworkConfig {
    /// The configuration the gh:#1097 and first gh:#1100 pins were measured
    /// on, kept for those pins (run it with
    /// [`NetworkMechanisms::without_pool_layers`]): 1 m³ in each sump pit
    /// and the quenchers at the downcomers' exit depth.
    pub fn unit1_gh1097() -> Self {
        Self { sump_water_m3: 1.0, quencher_elevation_m: None, ..Self::unit1() }
    }

    /// Fukushima Daiichi Unit 2 (gh:#1101), with [`Params::unit2`]. Marks
    /// as in `verification_and_validation/bwr_mark1/units23_open_data.md`
    /// (whose source keys are used here): \[S\] a search excerpt of a named
    /// public document, the primary not opened; \[D\] derived from \[S\]
    /// values; \[A\] assumed.
    ///
    /// ~~No accident sequence comes with it: this crate has no RCIC or HPCI
    /// model, so a Unit 2 network can be driven only by a snapshot the
    /// caller builds, never compared with the accident.~~ **CORRECTED
    /// 2026-11-02 (gh:#1109):** `Plant::for_unit(Unit::Two, ..)` drives it
    /// with Unit 2's cited sequence and the RCIC model (`super::units23`,
    /// `super::turbine_pump`).
    pub fn unit2() -> Self {
        Self {
            // [A] the modelling choice, as Unit 1.
            torus_shape: TorusShape::Torus,
            // [S] SI2, MDPI23: the torus's major diameter 33.5 m. The tube
            // radius stays derived from the sourced volumes (4.309 m), not
            // the sourced 4.45 m: the free volume is 0.938 of the geometric
            // torus, the rest being internals the network does not draw.
            torus_major_radius_m: 16.75,
            // [A] borrowed from Unit 1: the drywell floor at the torus top
            // (Unit 2's drywell elevation over the torus is not found).
            drywell_floor_m: None,
            // [A] borrowed from Unit 1.
            downcomer_entry_m: 2.0,
            // [A] borrowed from Unit 1 (numerical pits, dry).
            sump_area_m2: 2.0,
            sump_depth_m: 1.5,
            sump_water_m3: 0.0,
            // [A] borrowed from Unit 1: derived from the geometry.
            pool_layers: None,
            // [A] borrowed from Unit 1: the quenchers at the torus bottom.
            quencher_elevation_m: Some(0.0),
            // [D] the building's free volume (38 000 m^3, [A] from Unit 3)
            // spread from the torus bottom to the roof: the roof 46 m above
            // grade (SI2, IRID), grade taken at the R/B first floor, OP
            // 10.2 m (TEPCO16), and the torus bottom 4.45 m below its
            // equator at OP 1.9 m (TEPCO16), so 58.75 m: 646.8 m^2.
            building_floor_m2: 38_000.0 / (46.0 + 10.2 + (4.45 - 1.9)),
            // [A] borrowed from Unit 1 (the blow-out panel's size is not
            // found; it opened on 12 March, which is sequence, not
            // configuration).
            building_openings_m2: 1.0,
            // [A] all borrowed from Unit 1.
            downcomer_length_m: 8.0,
            vacuum_breaker_length_m: 1.0,
            vent_length_m: 60.0,
            flange_length_m: 0.5,
            building_length_m: 1.0,
        }
    }

    /// Fukushima Daiichi Unit 3 (gh:#1101), with [`Params::unit3`]: Unit
    /// 2's configuration. The torus is \[S\] for Unit 3 itself (SI3: 33.5 m
    /// and 8.9 m); the building's floor is \[D\] from Unit 3's own volume
    /// (KTH12) with Unit 2's elevations (TEPCO16), \[A\] for Unit 3, whose
    /// elevations are not found. Every other field is \[A\] borrowed from
    /// Unit 1, as for Unit 2.
    pub fn unit3() -> Self {
        Self::unit2()
    }
}

/// The torus pool's layer count from its geometry (gh:#1100): four layers
/// on average across the downcomers' submergence, `⌈4 D / s⌉`, `D` the
/// initial pool depth and `s` the submergence (`vent_submergence_pa` as a
/// depth), so that the water above the downcomers' exits, the old model's
/// top layer, is resolved by several layers, each a few tenths of a metre.
/// Unit 1: `D` = 3.237 m, `s` = 1.209 m, so 11 (mean 0.29 m). A resolution,
/// not a physical coefficient: the V&V doc runs 11 and 22.
pub fn derived_pool_layers(depth_m: f64, submergence_m: f64) -> usize {
    if !(depth_m > 0.0 && submergence_m > 0.0) {
        return 1;
    }
    (4.0 * depth_m / submergence_m).ceil().max(1.0) as usize
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self::unit1()
    }
}

/// The network's own mechanisms, all on by default; ~~each `without_*` on
/// [`ContainmentNetwork`] is an ablation~~ (**CORRECTED 2026-11-01**: there
/// were no such methods; each field cleared, or the `without_*` builders
/// below, is an ablation) for attributing a difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkMechanisms {
    /// The pool's compression work on the atmosphere (each volume).
    pub pressure_work: bool,
    /// IF97 steam in the atmospheres (else sembawang's ideal steam).
    pub real_steam: bool,
    /// IF97 liquid (else sembawang's constant `c_p`).
    pub real_liquid: bool,
    /// The swollen level decides what a port draws.
    pub level_swell: bool,
    /// MELCOR's bubble efficiency at submerged ports.
    pub bubble_pass_through: bool,
    /// Rain-out to saturation (else sembawang's four passes).
    pub equilibrium_rain_out: bool,
    /// Choking on every path.
    pub choking: bool,
    /// sembawang's cavity (MCCI) on the drywell floor (gh:#1100); without
    /// it, the debris's heat boils the sump while it holds water (beyond
    /// any configured fill; none by default since the sumps start dry),
    /// else goes into the structures (the gh:#1097 mapping).
    pub cavity: bool,
    /// sembawang's FDI fall of arriving debris through the water over the
    /// floor (needs `cavity`).
    pub fdi: bool,
    /// sembawang's concrete slab behind the drywell structures.
    pub dw_slab: bool,
    /// sembawang's fission products (`FpHold`) carried on the paths, with
    /// pool scrubbing and deposition (gh:#1100).
    pub fission_products: bool,
    /// Their decay heat in the volumes (needs `fission_products`).
    pub fp_decay_heat: bool,
    /// The torus pool in layers (`ControlVolume::with_pool_layers`,
    /// gh:#1100): what enters at a port joins the layer at its elevation,
    /// the surface sees ~~the top layer~~ the surface mixed block
    /// (**CORRECTED 2026-11-01**, [`Self::surface_mixed_block`]), an unstable column mixes. Without it,
    /// one well-mixed pool.
    pub pool_layers: bool,
    /// A layered pool's stable layers conduct (`tampines`' default; needs
    /// `pool_layers`).
    pub pool_layer_conduction: bool,
    /// The layered pool's surface exchange sees `tampines`' surface mixed
    /// block (`tampines`' default since gh:#1100's fix; needs
    /// `pool_layers`); without it, the top layer alone, which made the
    /// exchange depend on the layer count (V&V doc).
    pub surface_mixed_block: bool,
    /// The decay heat of the fission products in the torus (its gas, its
    /// pool's surface, scrubbed in the pool) goes into the water above the
    /// downcomers' exits, shared by mass (`CvNetwork::set_pool_heat_at`),
    /// where the old model puts it: into its top layer, which is the water
    /// above the exits (`Containment::step` step 0, `pool_energy()` with
    /// `layer_pool`'s split at the submergence). Without it, shared over
    /// the whole pool by mass (`set_pool_heat`). One layer: the same, bit
    /// for bit. Needs `fp_decay_heat`.
    pub fp_pool_heat_above_exits: bool,
    /// Every volume's pool flashes to the state consistent with the
    /// pressure its own vapour raises (`tampines`' default since gh:#1107,
    /// after MELCOR CVH/FL RM §4.2.1, §4.3, Eq 5-30); without it, the
    /// explicit flash, which after the vent flashed the torus's saturated
    /// surface block in one step (post-vent `p_ww` swings, V&V doc).
    pub implicit_flash: bool,
    /// Hydrogen and CO burns in the building by MELCOR's BUR criteria and
    /// rates, sembawang's (`containment::bur_ignites`, `bur_step`, as the
    /// old model's building), the stoichiometry and heat by
    /// [`super::reaction::burn_inputs`] (moved from `tampines`, gh:#1103) as
    /// `tampines`' species sources and sinks and one atmosphere heat.
    pub burns: bool,
    /// The wall surfaces see each non-condensable with its own transport
    /// properties (tampines' `structure_surface_mixture`, gh:#1114, #1120);
    /// without it, every non-condensable but H2 counted as N2.
    pub ncg_wall_properties: bool,
}

impl NetworkMechanisms {
    /// Ablation: the wall surfaces count every non-condensable but H2 as
    /// N2 (the three-gas `structure_surface`, as before gh:#1120).
    pub fn without_ncg_wall_properties(self) -> Self {
        Self { ncg_wall_properties: false, ..self }
    }
    /// Ablation: no cavity (nor FDI).
    pub fn without_cavity(self) -> Self {
        Self { cavity: false, ..self }
    }
    /// Ablation: no FDI fall.
    pub fn without_fdi(self) -> Self {
        Self { fdi: false, ..self }
    }
    /// Ablation: no drywell slab.
    pub fn without_dw_slab(self) -> Self {
        Self { dw_slab: false, ..self }
    }
    /// Ablation: no fission products (nor their decay heat).
    pub fn without_fission_products(self) -> Self {
        Self { fission_products: false, ..self }
    }
    /// Ablation: the fission products carried, their decay heat not
    /// deposited in the volumes.
    pub fn without_fp_decay_heat(self) -> Self {
        Self { fp_decay_heat: false, ..self }
    }
    /// Ablation: one well-mixed torus pool.
    pub fn without_pool_layers(self) -> Self {
        Self { pool_layers: false, ..self }
    }
    /// Ablation: a layered pool's stable layers exchange nothing.
    pub fn without_pool_layer_conduction(self) -> Self {
        Self { pool_layer_conduction: false, ..self }
    }
    /// Ablation: the layered pool's surface sees its top layer alone.
    pub fn without_surface_mixed_block(self) -> Self {
        Self { surface_mixed_block: false, ..self }
    }
    /// Ablation: the torus's fission-product decay heat shared over the
    /// whole pool by mass.
    pub fn without_fp_pool_heat_above_exits(self) -> Self {
        Self { fp_pool_heat_above_exits: false, ..self }
    }
    /// Ablation: nothing burns (the ignition criterion still reported).
    pub fn without_burns(self) -> Self {
        Self { burns: false, ..self }
    }
    /// Ablation: the explicit flash (`ControlVolume::without_implicit_flash`)
    /// in every volume, as before gh:#1107.
    pub fn without_implicit_flash(self) -> Self {
        Self { implicit_flash: false, ..self }
    }
    /// Ablation: no pool compression work.
    pub fn without_pressure_work(self) -> Self {
        Self { pressure_work: false, ..self }
    }
    /// Ablation: sembawang's ideal steam.
    pub fn without_real_steam(self) -> Self {
        Self { real_steam: false, ..self }
    }
    /// Ablation: sembawang's constant-`c_p` liquid.
    pub fn without_real_liquid(self) -> Self {
        Self { real_liquid: false, ..self }
    }
    /// Ablation: the collapsed level decides what a port draws.
    pub fn without_level_swell(self) -> Self {
        Self { level_swell: false, ..self }
    }
    /// Ablation: `ε = 1` at submerged ports.
    pub fn without_bubble_pass_through(self) -> Self {
        Self { bubble_pass_through: false, ..self }
    }
    /// Ablation: sembawang's four rain-out passes.
    pub fn without_equilibrium_rain_out(self) -> Self {
        Self { equilibrium_rain_out: false, ..self }
    }
    /// Ablation: no choking.
    pub fn without_choking(self) -> Self {
        Self { choking: false, ..self }
    }
}

impl Default for NetworkMechanisms {
    fn default() -> Self {
        Self {
            pressure_work: true,
            real_steam: true,
            real_liquid: true,
            level_swell: true,
            bubble_pass_through: true,
            equilibrium_rain_out: true,
            choking: true,
            cavity: true,
            fdi: true,
            dw_slab: true,
            fission_products: true,
            fp_decay_heat: true,
            pool_layers: true,
            pool_layer_conduction: true,
            surface_mixed_block: true,
            fp_pool_heat_above_exits: true,
            burns: true,
            implicit_flash: true,
            ncg_wall_properties: true,
        }
    }
}

/// MELCOR's heat-structure surface facing a volume's atmosphere `a` at
/// `t_g` (K) and `p` (Pa), on a wall at `t_wall` (K) of height `height`
/// (m): the convective coefficient, W/(m^2 K), and the condensation flux,
/// kg/(m^2 s). With `ncg_properties`, each non-condensable keeps its own
/// transport properties (tampines' `structure_surface_mixture`, gh:#1114,
/// #1120). Without it, the three-gas `structure_surface` counts every
/// non-condensable but H2 as N2, bit for bit as before gh:#1120.
pub fn wall_surface(a: &GasMixture, t_g: f64, p: f64, t_wall: f64, height: f64, ncg_properties: bool) -> (f64, f64) {
    if ncg_properties {
        let atm = tampines::components::wall_condensation::GasMixtureAtmosphere { t_k: t_g, p_pa: p, n_steam: a.steam_kg / M_H2O, n_ncg: a.ncg_moles };
        tampines::components::wall_condensation::structure_surface_mixture(&atm, t_wall, height)
    } else {
        let n_h2 = a.ncg_moles[NonCondensable::H2.index()];
        let atm = pool_surface::Atmosphere { t_k: t_g, p_pa: p, n_steam: a.steam_kg / M_H2O, n_n2: a.ncg_total_moles() - n_h2, n_h2 };
        pool_surface::structure_surface(&atm, t_wall, height)
    }
}

/// One step's outputs, comparable with [`super::containment::ContainmentResult`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkResult {
    pub p_dw_pa: f64,
    pub p_ww_pa: f64,
    pub p_rb_pa: f64,
    pub t_dw_k: f64,
    pub t_ww_gas_k: f64,
    /// The torus pool's temperature, K: its energy mean over the layers.
    pub t_pool_k: f64,
    /// The pool's surface layer, K (gh:#1100; as
    /// `ContainmentResult::t_pool_top_k`); the pool's for one layer.
    pub t_pool_top_k: f64,
    /// The number of layers the torus pool holds.
    pub pool_layers: usize,
    /// Torus pool: mass, kg, collapsed and swollen level, m (plant datum).
    pub pool_kg: f64,
    pub pool_level_m: f64,
    pub pool_swollen_level_m: f64,
    /// Water in the drywell sump, kg, and whether it is dry (gh:#1100).
    pub dw_sump_kg: f64,
    pub dw_sump_dry: bool,
    pub dw_steam_kg: f64,
    pub ww_steam_kg: f64,
    pub dw_h2_kg: f64,
    pub ww_h2_kg: f64,
    /// Hydrogen's mole fraction in the building's gas (steam included).
    pub rb_h2_fraction: f64,
    /// The building's Le Chatelier combustible fraction `X_H2 + X_CO
    /// L_H2 / L_CO` (BUR RM Eq 2-3) and whether it meets MELCOR's ignition
    /// criteria (`containment::bur_ignites`, sembawang's; steam and CO2 as
    /// the diluent). ~~A diagnostic only: the network does not burn (no
    /// non-condensable sink in `tampines`; V&V doc).~~ **CORRECTED
    /// 2026-11-01:** with [`NetworkMechanisms::burns`] (default) the
    /// building burns when the criteria are met, at the start of a step.
    pub rb_burn_y: f64,
    pub rb_ignites: bool,
    /// A burn is running in the building (`burns`), and what `tampines`
    /// burned there since the start, kg of H2 and CO, with the heat of
    /// reaction it released, J (gh:#1100).
    pub rb_burning: bool,
    /// A burn has started in the building since the run began (gh:#1099;
    /// the old model's `rb_exploded` with `burn_dynamics`).
    pub rb_burn_started: bool,
    pub rb_h2_burned_kg: f64,
    pub rb_co_burned_kg: f64,
    pub rb_burn_heat_j: f64,
    /// Pool heat each volume (drywell, torus, building) put into its
    /// atmosphere because its pool was dry or boiled dry within a step,
    /// since the start, J (`tampines`' `pool_heat_to_atmosphere`, gh:#1100).
    pub pool_heat_to_atmosphere_j: [f64; 3],
    pub vent_flowing: bool,
    pub vented_kg_s: f64,
    pub vented_h2_kg: f64,
    pub flange_leaking: bool,
    pub flange_kg_s: f64,
    /// Gas through the downcomers (positive into the torus) and the vacuum
    /// breakers (into the drywell), kg/s.
    pub downcomer_kg_s: f64,
    pub vacuum_breaker_kg_s: f64,
    /// The cavity (gh:#1100): concrete ablated, kg; H2 and CO made there,
    /// kg; the debris temperature, K; steam raised by the FDI fall, kg.
    pub cav_ablated_kg: f64,
    pub cav_h2_kg: f64,
    pub cav_co_kg: f64,
    pub cav_debris_k: f64,
    pub fdi_steam_kg: f64,
    /// The cavity debris's metallic Zr, kg, and whether an FDI fall was
    /// fast enough for MELCOR's high-pressure model (flagged, still treated
    /// as low-pressure; `lpme_fall`'s flag, as the old model reports it).
    /// Added 2026-11-02 (gh:#1099) so that [`NetworkResult::to_containment_result`]
    /// fills every field the plant reads.
    pub cav_zr_kg: f64,
    pub fdi_hpme: bool,
    /// The building's CO mole fraction (steam included) and the largest
    /// burn's adiabatic pressure bound, Pa (`Burn::dp_max_pa`; a diagnostic,
    /// as in the old model: the network's own pressure rise is `p_rb_pa`).
    /// Added 2026-11-02 (gh:#1099).
    pub rb_co_fraction: f64,
    pub rb_burn_dp_pa: f64,
    /// CO and CO2 in the drywell atmosphere, kg.
    pub dw_co_kg: f64,
    pub dw_co2_kg: f64,
    /// The drywell structures' temperature and the slab's inner cell, K.
    pub dw_wall_k: f64,
    pub dw_slab_inner_k: f64,
    /// Where the fission products are (gh:#1100; zero with
    /// `without_fission_products`), and the decay heat they deposited this
    /// step, W, by location.
    pub fp: FpReport,
    pub fp_heat: FpDecayHeat,
    /// ~~Picard iterations~~ **CORRECTED 2026-11-03 (gh:#1124):** the
    /// network's iterations this step (summed
    /// over sub-steps; Newton's, or Picard's where Newton fell back, as
    /// `tampines`' `NetworkStepReport::iterations`), the largest residual,
    /// and the sub-steps taken so far in the run.
    pub iterations: usize,
    pub residual: f64,
    pub substeps: usize,
    /// `tampines`' Newton step this step (gh:#1124), summed over sub-steps:
    /// its iterates, the failed ones of a fallback included (so the step
    /// cost `newton_iterations` plus the fallbacks' Picard iterations), and
    /// the sub-steps on which it fell back to Picard. `newton_fell_back` is
    /// `newton_fallbacks > 0`.
    pub newton_iterations: usize,
    pub newton_fallbacks: usize,
    pub newton_fell_back: bool,
}

impl NetworkResult {
    /// The plant's view of this step (gh:#1099, 2026-11-02): the old
    /// model's [`ContainmentResult`], field by field, so that
    /// [`super::Plant`] reads the network exactly as it read
    /// [`super::containment::Containment`]. Mapped as:
    ///
    /// - pressures, temperatures, hydrogen, the vent and the flange, the
    ///   FDI steam, the cavity and the fission products: the same
    ///   quantities, directly;
    /// - `rb_flammable`: the building's H2 fraction at or above
    ///   `prm.h2_flammable`, as the old model computes it;
    /// - `rb_exploded`: a burn has started ([`NetworkResult::rb_burn_started`];
    ///   the old model sets it when `bur_step` starts a burn);
    /// - `rb_burn_dp_pa`: the same adiabatic bound (`Burn::dp_max_pa`),
    ///   although the network's building has its own pressure (`p_rb_pa`,
    ///   not carried: the old result has no field for it).
    ///
    /// The old model's building fractions are of its gas at one atmosphere;
    /// the network's are of its building volume's gas, steam included.
    pub fn to_containment_result(&self, prm: &Params) -> ContainmentResult {
        let fp = &self.fp;
        ContainmentResult {
            p_dw_pa: self.p_dw_pa,
            p_ww_pa: self.p_ww_pa,
            t_dw_k: self.t_dw_k,
            t_pool_k: self.t_pool_k,
            t_pool_top_k: self.t_pool_top_k,
            dw_h2_kg: self.dw_h2_kg,
            ww_h2_kg: self.ww_h2_kg,
            rb_h2_fraction: self.rb_h2_fraction,
            vent_flowing: self.vent_flowing,
            flange_leaking: self.flange_leaking,
            rb_flammable: self.rb_h2_fraction >= prm.h2_flammable,
            rb_exploded: self.rb_burn_started,
            rb_burn_y: self.rb_burn_y,
            rb_co_fraction: self.rb_co_fraction,
            rb_burning: self.rb_burning,
            rb_burn_dp_pa: self.rb_burn_dp_pa,
            fdi_steam_kg: self.fdi_steam_kg,
            fdi_hpme: self.fdi_hpme,
            vented_h2_kg: self.vented_h2_kg,
            vented_kg_s: self.vented_kg_s,
            xe_dw: fp.xe_dw,
            xe_ww: fp.xe_ww,
            xe_vented: fp.xe_vented,
            xe_building: fp.xe_building,
            cs_dw_structures_kg: fp.cs_dw_structures_kg,
            cs_ww_pool_kg: fp.cs_ww_pool_kg,
            cs_airborne_kg: fp.cs_airborne_kg,
            cs_vented_kg: fp.cs_vented_kg,
            cs_building_kg: fp.cs_building_kg,
            i_dw_structures_kg: fp.i_dw_structures_kg,
            i_ww_pool_kg: fp.i_ww_pool_kg,
            i_airborne_kg: fp.i_airborne_kg,
            i_vented_kg: fp.i_vented_kg,
            i_building_kg: fp.i_building_kg,
            te_kg: fp.te_kg,
            cav_ablated_kg: self.cav_ablated_kg,
            cav_h2_kg: self.cav_h2_kg,
            cav_co_kg: self.cav_co_kg,
            cav_debris_k: self.cav_debris_k,
            cav_zr_kg: self.cav_zr_kg,
        }
    }
}

/// Deepest halving of a step that does not converge (a 1 s step down to
/// 1/64 s).
const MAX_HALVINGS: usize = 6;

/// What each volume reported over one [`advance`], summed over its
/// sub-steps, by volume: pool heat into the atmosphere, J; H2 and CO taken
/// by its species sinks, mol; (for the building's running total) the burn's
/// stored-energy release `Q`, J, which sembawang computes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct VolumeSums {
    pool_heat_to_atm_j: f64,
    h2_mol: f64,
    co_mol: f64,
    rx_heat_j: f64,
}

/// One network step of `dt`; if the Picard iteration does not converge,
/// two of `dt / 2`, recursively, up to [`MAX_HALVINGS`] (as MELCOR's
/// executive cuts its step when a package fails to converge, EXEC RM,
/// cited). The network commits nothing on a failed step, and the held
/// sources and heats are rates, so the halves see the same drive. Writes
/// each path's mean mass rate over `dt` into `rates`, `[both phases, gas]`;
/// returns the iterations, the largest residual, the extra sub-steps taken
/// and the Newton step's count ([`Advanced`]).
fn advance(net: &mut CvNetwork, dt: f64, depth: usize, rates: &mut Vec<[f64; 2]>, sums: &mut Vec<VolumeSums>) -> Result<Advanced, TampinesError> {
    match net.step(Time::new::<second>(dt)) {
        Ok(r) => {
            sums.resize(r.volumes.len(), VolumeSums::default());
            for (s, v) in sums.iter_mut().zip(&r.volumes) {
                s.pool_heat_to_atm_j += v.pool_heat_to_atmosphere.get::<uom::si::energy::joule>();
                // What the species sinks took (gh:#1103: the burn's H2 and
                // CO leave by `tampines`' species sinks).
                s.h2_mol -= v.species_ncg[NonCondensable::H2.index()].get::<kilogram>() / NonCondensable::H2.molar_mass();
                s.co_mol -= v.species_ncg[NonCondensable::CO.index()].get::<kilogram>() / NonCondensable::CO.molar_mass();
            }
            *rates = r
                .transfers
                .iter()
                .map(|t| {
                    let g = t.gas_mass_rate.get::<kilogram_per_second>();
                    [g + t.liquid_mass_rate.get::<kilogram_per_second>(), g]
                })
                .collect();
            Ok(Advanced {
                iterations: r.iterations,
                residual: r.residual,
                substeps: 0,
                newton_iterations: r.newton_iterations,
                newton_fallbacks: usize::from(r.newton_fell_back),
            })
        }
        Err(TampinesError::Numerical(_)) if depth < MAX_HALVINGS => {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            let x = advance(net, 0.5 * dt, depth + 1, &mut a, sums)?;
            let y = advance(net, 0.5 * dt, depth + 1, &mut b, sums)?;
            *rates = a.iter().zip(&b).map(|(x, y)| [0.5 * (x[0] + y[0]), 0.5 * (x[1] + y[1])]).collect();
            Ok(Advanced {
                iterations: x.iterations + y.iterations,
                residual: x.residual.max(y.residual),
                substeps: 1 + x.substeps + y.substeps,
                newton_iterations: x.newton_iterations + y.newton_iterations,
                newton_fallbacks: x.newton_fallbacks + y.newton_fallbacks,
            })
        }
        Err(e) => Err(e),
    }
}

/// What [`advance`] reports: the iterations, the largest residual, the
/// extra sub-steps, and `tampines`' Newton iterates (failed ones included)
/// and fallbacks, each summed over the sub-steps (gh:#1124).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Advanced {
    iterations: usize,
    residual: f64,
    substeps: usize,
    newton_iterations: usize,
    newton_fallbacks: usize,
}

/// Lumped wall on an atmosphere (the drywell structures, the torus shell).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wall {
    t_k: f64,
}

/// The Mark I containment as a control-volume network. See the module docs.
#[derive(Clone, Debug)]
pub struct ContainmentNetwork {
    net: CvNetwork,
    prm: Params,
    cfg: NetworkConfig,
    dw: usize,
    ww: usize,
    rb: usize,
    downcomer: usize,
    vacuum_breaker: usize,
    vent: usize,
    flange: usize,
    dw_wall: Wall,
    ww_shell: Wall,
    disk_burst: bool,
    flange_open: bool,
    vented_h2_kg: f64,
    substeps: usize,
    sump_initial_kg: f64,
    /// The downcomers' exits and the quenchers' port, m (plant datum).
    z_exit: f64,
    z_quencher: f64,
    mech: NetworkMechanisms,
    cavity: Cavity,
    fdi_steam_kg: f64,
    fdi_hpme: bool,
    dw_slab: [f64; DW_SLAB_CELLS],
    fp: FpHold,
    /// The torus's top, m (the band of the placed decay heat ends there).
    ww_top: f64,
    /// The building's burn (sembawang's BUR state) and what `tampines`
    /// burned and released there, and each volume's pool heat into its
    /// atmosphere, since the start (gh:#1100).
    rb_burn: Burn,
    /// A burn has started in the building (gh:#1099: the old model's
    /// `rb_exploded`, which with `burn_dynamics` is set when a burn starts).
    rb_burn_started: bool,
    rb_burned: VolumeSums,
    pool_heat_to_atm_j: [f64; 3],
}

/// Ports, by volume.
const DW_DOWNCOMER: usize = 0;
const DW_VB: usize = 1;
const DW_FLANGE: usize = 2;
const DW_LEAK: usize = 3;
const DW_SUMP: usize = 4;
const DW_CAVITY: usize = 5;
const WW_DOWNCOMER: usize = 0;
const WW_VB: usize = 1;
const WW_VENT: usize = 2;
const WW_QUENCHER: usize = 3;
const WW_DRAIN: usize = 4;
const RB_FLANGE: usize = 0;
const RB_OPENING: usize = 1;

fn len(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn vol(x: f64) -> Volume {
    Volume::new::<cubic_meter>(x)
}
fn kel(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}
fn kgs(x: f64) -> MassRate {
    MassRate::new::<kilogram_per_second>(x)
}
fn mol(x: f64) -> AmountOfSubstance {
    AmountOfSubstance::new::<mole>(x)
}
fn rho_liquid(t: f64, p: f64) -> f64 {
    1.0 / v_tp_1(kel(t), Pressure::new::<pascal>(p)).get::<cubic_meter_per_kilogram>()
}

/// A torus's volume below `y` (from its bottom) at major radius `big_r` and
/// tube radius `r`, tabulated: `(y, V)` at `n + 1` points, by the midpoint
/// rule on `A(y) = 4πR √(r² − (y − r)²)` then scaled to the exact total
/// `2π² R r²` (so the table is a torus of the right volume).
fn torus_table(big_r: f64, r: f64, n: usize) -> Vec<(f64, f64)> {
    let pi = std::f64::consts::PI;
    let dy = 2.0 * r / n as f64;
    let mut pts = vec![(0.0, 0.0)];
    let mut v = 0.0;
    for k in 0..n {
        let y = (k as f64 + 0.5) * dy - r;
        v += 4.0 * pi * big_r * (r * r - y * y).max(0.0).sqrt() * dy;
        pts.push(((k + 1) as f64 * dy, v));
    }
    let total = 2.0 * pi * pi * big_r * r * r;
    let scale = total / v;
    for p in &mut pts {
        p.1 *= scale;
    }
    pts
}

fn table(points: &[(f64, f64)]) -> Result<VolumeAltitudeTable, TampinesError> {
    let pts: Vec<(Length, Volume)> = points.iter().map(|(z, v)| (len(*z), vol(*v))).collect();
    VolumeAltitudeTable::new(&pts)
}

/// A pit of `a` m² × `d` m at `z0`, then `area` m² up to `total` m³.
fn pit_then_prism(z0: f64, a: f64, d: f64, area: f64, total: f64) -> Vec<(f64, f64)> {
    let v_pit = a * d;
    vec![(z0, 0.0), (z0 + d, v_pit), (z0 + d + (total - v_pit) / area, total)]
}

fn path(area: f64, cd: f64, length: f64, z_from: f64, z_to: f64, opening: f64, kind: FlowPathKind, choking: bool) -> Result<FlowPath, TampinesError> {
    let p = FlowPath::new(
        FlowPathGeometry {
            area: Area::new::<square_meter>(area),
            inertial_length: len(length),
            form_loss: 1.0,
            discharge_coefficient: cd,
            from_elevation: len(z_from),
            to_elevation: len(z_to),
            opening_height: len(opening),
        },
        kind,
    )?;
    Ok(if choking { p } else { p.without_choking() })
}

/// The torus pool in `n` layers (after the other ablations, so each layer
/// takes the volume's liquid model), and conduction between them ablated
/// when `mech` says so; one layer leaves the volume as it was.
fn layer_pool(cv: ControlVolume, m: &NetworkMechanisms, n: usize) -> Result<ControlVolume, TampinesError> {
    if !m.pool_layers || n <= 1 {
        return Ok(cv);
    }
    let cv = cv.with_pool_layers(n)?;
    let cv = if m.pool_layer_conduction { cv } else { cv.without_pool_layer_conduction() };
    Ok(if m.surface_mixed_block { cv } else { cv.without_surface_mixed_block() })
}

fn ablate(cv: ControlVolume, m: &NetworkMechanisms) -> Result<ControlVolume, TampinesError> {
    let mut cv = cv;
    if !m.pressure_work {
        cv = cv.without_pressure_work();
    }
    if !m.level_swell {
        cv = cv.without_level_swell();
    }
    if !m.bubble_pass_through {
        cv = cv.without_bubble_pass_through();
    }
    if !m.equilibrium_rain_out {
        cv = cv.without_equilibrium_rain_out();
    }
    if !m.real_steam {
        cv = cv.without_real_steam()?;
    }
    if !m.real_liquid {
        cv = cv.without_real_liquid()?;
    }
    if !m.implicit_flash {
        cv = cv.without_implicit_flash();
    }
    Ok(cv)
}

impl ContainmentNetwork {
    /// Unit 1 from `prm`, [`NetworkConfig::unit1`], every mechanism on.
    ///
    /// # Errors
    ///
    /// The volumes' and paths' construction errors.
    pub fn new(prm: &Params) -> Result<Self, TampinesError> {
        Self::with_config(prm, NetworkConfig::unit1(), NetworkMechanisms::default())
    }

    /// Any unit's network: `prm` for what `params` holds, `cfg` for the
    /// shapes and elevations, `mech` for the ablations.
    ///
    /// # Errors
    ///
    /// The volumes' and paths' construction errors.
    pub fn with_config(prm: &Params, cfg: NetworkConfig, mech: NetworkMechanisms) -> Result<Self, TampinesError> {
        let p0 = prm.initial_containment_pa;
        let g = 9.81;
        // Torus.
        let t_pool = prm.initial_pool_k;
        let pool_kg = prm.pool_water_m3 * 995.0;
        let v_torus = prm.wetwell_gas_m3 + pool_kg / rho_liquid(t_pool, p0);
        let ww_pts = match cfg.torus_shape {
            TorusShape::Torus => {
                let big_r = cfg.torus_major_radius_m;
                let r = (v_torus / (2.0 * std::f64::consts::PI * std::f64::consts::PI * big_r)).sqrt();
                torus_table(big_r, r, 64)
            }
            TorusShape::Prismatic => vec![(0.0, 0.0), (v_torus / prm.ww_pool_surface_m2, v_torus)],
        };
        let ww_table = table(&ww_pts)?;
        let ww_top = ww_pts.last().map_or(0.0, |p| p.0);
        let level0 = ww_table.height_of_volume(vol(pool_kg / rho_liquid(t_pool, p0))).get::<meter>();
        let exit = level0 - prm.vent_submergence_pa / (995.0 * g);
        let d_dc = (4.0 * prm.downcomer_area_m2 / (prm.downcomer_count * std::f64::consts::PI)).sqrt();
        let z_ww_gas = ww_top - 0.5;
        let z_quencher = cfg.quencher_elevation_m.unwrap_or(exit);
        let ww_ports = vec![
            Port { elevation: len(exit), opening_height: len(d_dc) },
            Port { elevation: len(z_ww_gas), opening_height: len(0.5) },
            Port { elevation: len(z_ww_gas), opening_height: len(0.3) },
            Port { elevation: len(z_quencher), opening_height: len(d_dc) },
            Port { elevation: len(level0 + 0.3), opening_height: len(0.05) },
        ];
        let p_s = p_sat_pa(t_pool);
        let n_ww = p0 * prm.wetwell_gas_m3 / (R_GAS * t_pool);
        let ww_gas = GasMixture::new(Mass::new::<kilogram>(p_s / p0 * n_ww * M_H2O), &[(NonCondensable::N2, mol(n_ww * (1.0 - p_s / p0)))]);
        let ww = ControlVolume::new(ww_table, ww_ports, Mass::new::<kilogram>(pool_kg), kel(t_pool), ww_gas, kel(t_pool))?;
        // Drywell.
        let t_dw = prm.initial_drywell_k;
        let floor = cfg.drywell_floor_m.unwrap_or(ww_top);
        let sump_kg = cfg.sump_water_m3 * rho_liquid(t_dw, p0);
        let dw_total = prm.drywell_volume_m3 + sump_kg / rho_liquid(t_dw, p0);
        let dw_pts = pit_then_prism(floor - cfg.sump_depth_m, cfg.sump_area_m2, cfg.sump_depth_m, prm.dw_floor_m2, dw_total);
        let dw_top = dw_pts[2].0;
        let z_dc = floor + cfg.downcomer_entry_m;
        let dw_ports = vec![
            Port { elevation: len(z_dc), opening_height: len(1.0) },
            Port { elevation: len(z_dc), opening_height: len(0.5) },
            Port { elevation: len(dw_top - 0.2), opening_height: len(0.05) },
            Port { elevation: len(0.5 * (z_dc + dw_top)), opening_height: len(0.1) },
            Port { elevation: len(floor - 0.1), opening_height: len(0.1) },
            // The cavity's gases enter the atmosphere (sembawang's), at the
            // leak's height, so above any water on the floor.
            Port { elevation: len(0.5 * (z_dc + dw_top)), opening_height: len(0.1) },
        ];
        let n_dw = p0 * prm.drywell_volume_m3 / (R_GAS * t_dw);
        let dw_gas = GasMixture::new(Mass::new::<kilogram>(0.0), &[(NonCondensable::N2, mol(n_dw))]);
        let dw = ControlVolume::new(table(&dw_pts)?, dw_ports, Mass::new::<kilogram>(sump_kg), kel(t_dw), dw_gas, kel(t_dw))?;
        // Reactor building.
        let rb_sump = cfg.sump_water_m3 * rho_liquid(T_RB, P_ATM);
        let rb_pts = pit_then_prism(-cfg.sump_depth_m, cfg.sump_area_m2, cfg.sump_depth_m, cfg.building_floor_m2, prm.reactor_building_m3 + cfg.sump_water_m3);
        let rb_top = rb_pts[2].0;
        if !(rb_top > dw_top) {
            return Err(TampinesError::InvalidInput(format!("the building ({rb_top} m) must reach above the drywell head ({dw_top} m)")));
        }
        let rb_ports = vec![
            Port { elevation: len(dw_top - 0.2), opening_height: len(0.05) },
            Port { elevation: len(rb_top - 1.0), opening_height: len(1.0) },
        ];
        let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
        let rb_gas = GasMixture::new(Mass::new::<kilogram>(0.0), &[(NonCondensable::N2, mol(0.79 * n_rb)), (NonCondensable::O2, mol(0.21 * n_rb))]);
        let rb = ControlVolume::new(table(&rb_pts)?, rb_ports, Mass::new::<kilogram>(rb_sump), kel(T_RB), rb_gas, kel(T_RB))?;
        let mut net = CvNetwork::new(NetworkControl::default());
        let dw = net.add_volume(ablate(dw, &mech)?);
        let n_layers = if mech.pool_layers { cfg.pool_layers.unwrap_or_else(|| derived_pool_layers(level0, prm.vent_submergence_pa / (995.0 * g))) } else { 1 };
        let ww = net.add_volume(layer_pool(ablate(ww, &mech)?, &mech, n_layers)?);
        let rb = net.add_volume(ablate(rb, &mech)?);
        let cd = prm.discharge_coefficient;
        let env = NetworkBoundary::dry_air(Pressure::new::<pascal>(P_ATM), kel(300.0));
        let at = |volume, port| NetworkEnd::Volume { volume, port };
        let downcomer = net.add_path(NetworkPath {
            path: path(prm.downcomer_area_m2, cd, cfg.downcomer_length_m, z_dc, exit, d_dc, FlowPathKind::Open, mech.choking)?,
            from: at(dw, DW_DOWNCOMER),
            to: at(ww, WW_DOWNCOMER),
        })?;
        let vacuum_breaker = net.add_path(NetworkPath {
            path: path(
                prm.vacuum_breaker_m2,
                cd,
                cfg.vacuum_breaker_length_m,
                z_ww_gas,
                z_dc,
                0.0,
                FlowPathKind::VacuumBreaker { opening_pressure_difference: Pressure::new::<pascal>(prm.vacuum_breaker_pa) },
                mech.choking,
            )?,
            from: at(ww, WW_VB),
            to: at(dw, DW_VB),
        })?;
        let vent = net.add_path(NetworkPath {
            path: path(prm.vent_area_m2, cd, cfg.vent_length_m, z_ww_gas, z_ww_gas, 0.0, FlowPathKind::CheckValve, mech.choking)?,
            from: at(ww, WW_VENT),
            to: NetworkEnd::Boundary(env),
        })?;
        let flange = net.add_path(NetworkPath {
            path: path(prm.flange_leak_m2, cd, cfg.flange_length_m, dw_top - 0.2, dw_top - 0.2, 0.0, FlowPathKind::CheckValve, mech.choking)?,
            from: at(dw, DW_FLANGE),
            to: at(rb, RB_FLANGE),
        })?;
        net.add_path(NetworkPath {
            path: path(cfg.building_openings_m2, 1.0, cfg.building_length_m, rb_top - 1.0, rb_top - 1.0, 0.0, FlowPathKind::Open, mech.choking)?,
            from: at(rb, RB_OPENING),
            to: NetworkEnd::Boundary(env),
        })?;
        net.set_path_open_fraction(vent, 0.0)?;
        net.set_path_open_fraction(flange, 0.0)?;
        Ok(Self {
            net,
            prm: *prm,
            cfg,
            dw,
            ww,
            rb,
            downcomer,
            vacuum_breaker,
            vent,
            flange,
            dw_wall: Wall { t_k: t_dw },
            ww_shell: Wall { t_k: t_pool },
            disk_burst: false,
            flange_open: false,
            vented_h2_kg: 0.0,
            substeps: 0,
            sump_initial_kg: sump_kg,
            z_exit: exit,
            z_quencher,
            mech,
            cavity: Cavity::default(),
            fdi_steam_kg: 0.0,
            fdi_hpme: false,
            dw_slab: [t_dw; DW_SLAB_CELLS],
            fp: FpHold::new(prm.drywell_volume_m3, prm.wetwell_gas_m3),
            ww_top,
            rb_burn: Burn::default(),
            rb_burn_started: false,
            rb_burned: VolumeSums::default(),
            pool_heat_to_atm_j: [0.0; 3],
        })
    }

    /// The network (read-only).
    pub fn network(&self) -> &CvNetwork {
        &self.net
    }

    /// The configuration it was built with.
    pub fn config(&self) -> NetworkConfig {
        self.cfg
    }

    /// The mechanisms it was built with.
    pub fn mechanisms(&self) -> NetworkMechanisms {
        self.mech
    }

    /// The debris on the drywell floor (sembawang's cavity).
    pub fn cavity(&self) -> &Cavity {
        &self.cavity
    }

    /// The fission products the network holds (sembawang's `FpHold`).
    pub fn fission_products(&self) -> &FpHold {
        &self.fp
    }

    /// The drywell structures' and the torus shell's temperatures, K.
    pub fn wall_temperatures(&self) -> (f64, f64) {
        (self.dw_wall.t_k, self.ww_shell.t_k)
    }

    /// MELCOR's surface on a lumped wall, from volume `i`'s state: the heat
    /// into the atmosphere, W, the steam condensed, kg/s, and the heat into
    /// the wall, W (explicit; sembawang's `dw_wall_surface` and
    /// `ww_shell_step` with the same correlation, at most 90 % of the steam
    /// a step).
    fn wall_exchange(&self, i: usize, wall: Wall, area: f64, height: f64, fixed_h: Option<f64>, dt: f64) -> (f64, f64, f64) {
        let Some(cv) = self.net.volume(i) else { return (0.0, 0.0, 0.0) };
        let a = cv.atmosphere();
        let t_g = cv.atmosphere_temperature().get::<kelvin>();
        let p = cv.atmosphere_pressure().get::<pascal>();
        let (h, m) = match fixed_h {
            Some(h) => (h, 0.0),
            None => {
                let (h, flux) = wall_surface(&a, t_g, p, wall.t_k, height, self.mech.ncg_wall_properties);
                (h, (flux * area * dt).min(0.9 * a.steam_kg) / dt)
            }
        };
        let q_conv = h * area * (t_g - wall.t_k);
        let latent = m * (pool_surface::steam_h(t_g) - CP_WATER * (wall.t_k - 273.15));
        (-q_conv, m, q_conv + latent)
    }

    /// Advance one step with sembawang's snapshot (see the module docs for
    /// what is mapped).
    ///
    /// # Errors
    ///
    /// The network's errors (a Picard step that does not converge, a state
    /// outside IF97).
    pub fn step(&mut self, s: &ContainmentSnapshot) -> Result<NetworkResult, TampinesError> {
        let prm = self.prm;
        let dt = s.dt_s;
        let (dw, ww, rb) = (self.dw, self.ww, self.rb);
        // The water at the quenchers (the pool's for one layer).
        let t_pool = self.liquid_t_at(self.z_quencher);
        // Sources.
        let t_h2 = if s.relief_h2_enthalpy { s.srv_gas_k } else { t_pool };
        let relief = Source::water(kgs(s.srv_steam_kg_s), AvailableEnergy::new::<joule_per_kilogram>(s.srv_h))
            .with_gas(&[(NonCondensable::H2, kgs(s.srv_h2_kg_s))], kel(t_h2.max(1.0)));
        self.net.set_source(ww, WW_QUENCHER, relief)?;
        let leak = Source::water(kgs(s.leak_steam_kg_s), AvailableEnergy::new::<joule_per_kilogram>(s.leak_h))
            .with_gas(&[(NonCondensable::H2, kgs(s.leak_h2_kg_s))], kel(s.leak_gas_k.max(1.0)));
        self.net.set_source(dw, DW_LEAK, leak)?;
        // Fission products (gh:#1100): the aerosol volumes follow the
        // atmospheres; the decay heat of what is held, from the
        // start-of-step inventory (as the old model's step 0); then what
        // arrives.
        let fp_on = self.mech.fission_products;
        let mut fp_heat = FpDecayHeat::default();
        if fp_on {
            self.fp.begin_step(s);
            if let (Some(a), Some(b)) = (self.net.volume(dw), self.net.volume(ww)) {
                self.fp.dw_rn.set_volume(a.atmosphere_volume().get::<cubic_meter>());
                self.fp.ww_rn.set_volume(b.atmosphere_volume().get::<cubic_meter>());
            }
            if self.mech.fp_decay_heat {
                fp_heat = self.fp.decay_heat(&prm, s).unwrap_or_default();
            }
            self.dw_wall.t_k += fp_heat.dw_wall_w * dt / prm.dw_wall_c;
            let side = self.pool_side(Some(self.z_quencher));
            self.fp.arrive(&prm, s, side);
        }
        // The torus's share: into the water above the downcomers' exits
        // (the old model's top layer, `fp_pool_heat_above_exits`), else by
        // mass over the pool.
        if self.mech.fp_pool_heat_above_exits {
            self.net.set_pool_heat(ww, Power::new::<watt>(0.0))?;
            self.net.set_pool_heat_at(ww, &[ElevatedPoolHeat::between(len(self.z_exit), len(self.ww_top), Power::new::<watt>(fp_heat.pool_w))])?;
        } else {
            self.net.set_pool_heat(ww, Power::new::<watt>(fp_heat.pool_w))?;
        }
        // Structures (explicit, from the start-of-step state).
        let (q_dw, m_dw, q_wall) = self.wall_exchange(dw, self.dw_wall, prm.dw_wall_area_m2, prm.dw_wall_height_m, (!s.dw_wall_hs).then_some(prm.dw_wall_h), dt);
        let (q_ww, m_ww, q_shell) = if s.ww_shell { self.wall_exchange(ww, self.ww_shell, prm.ww_shell_area_m2, prm.ww_shell_height_m, None, dt) } else { (0.0, 0.0, 0.0) };
        self.dw_wall.t_k += q_wall * dt / prm.dw_wall_c;
        self.ww_shell.t_k += q_shell * dt / prm.ww_shell_c;
        self.net.set_atmosphere_heat(dw, Power::new::<watt>(q_dw + s.rpv_wall_w + fp_heat.dw_gas_w))?;
        if fp_on {
            self.fp.dw_condensed_kg += m_dw * dt;
        }
        // The drywell spray (gh:#1109): the steam its droplets condense
        // (`containment::spray_condensation`, from the start-of-step
        // atmosphere less the structures' share), and its water with the
        // condensate onto the floor below. Not counted as wall condensation
        // for the fission products' diffusiophoresis (stated).
        let (m_spray, t_spray) = if s.spray_kg_s > 0.0 && s.spray_exchange {
            self.net.volume(dw).map_or((0.0, s.spray_k), |cv| {
                let a = cv.atmosphere();
                let t = cv.atmosphere_temperature().get::<kelvin>();
                let v = cv.atmosphere_volume().get::<cubic_meter>().max(1e-9);
                let p_s = a.steam_kg / M_H2O * R_GAS * t / v;
                super::containment::spray_condensation(s.spray_kg_s, s.spray_k, t, p_s, (a.steam_kg - m_dw * dt).max(0.0), dt)
            })
        } else {
            (0.0, s.spray_k)
        };
        self.net.set_atmosphere_steam_sink(dw, kgs(m_dw + m_spray))?;
        self.net.set_atmosphere_heat(ww, Power::new::<watt>(q_ww))?;
        self.net.set_atmosphere_steam_sink(ww, kgs(m_ww))?;
        // Water into the drywell sump: the structures' condensate at the
        // wall's temperature, the pedestal's inflow, the failed vessel's
        // drain (one stream, mass-weighted enthalpy, sembawang's c_p).
        let streams = [(m_dw, self.dw_wall.t_k), (s.pedestal_inflow_kg_s, prm.injection_temperature_k), (s.vessel_drain_kg_s, s.vessel_drain_k), (s.spray_kg_s + m_spray, t_spray)];
        let m_sump: f64 = streams.iter().map(|x| x.0).sum();
        let h_sump = if m_sump > 0.0 { streams.iter().map(|(m, t)| m * CP_WATER * (t - 273.15)).sum::<f64>() / m_sump } else { 0.0 };
        self.net.set_source(dw, DW_SUMP, Source::water(kgs(m_sump), AvailableEnergy::new::<joule_per_kilogram>(h_sump)))?;
        // Debris on the floor (after the lower head fails). The water over
        // it is the drywell sump's (gh:#1100: it starts dry and may boil
        // dry, as the old model's pedestal), beyond any initial fill the
        // configuration gives (the gh:#1097 configuration's 1 m³; none by
        // default). Dry, the debris's heat goes into the structures, as the
        // old model's does (`boil_pedestal` only while the pedestal holds
        // water, else `dw_wall_t`), not into the drywell atmosphere where
        // `tampines` would put heat into a dry pool.
        let (sump_kg, sump_t, sump_dry) =
            self.net.volume(dw).map_or((0.0, t_pool, true), |v| (v.pool_mass().get::<kilogram>(), v.pool_temperature().get::<kelvin>(), v.is_pool_dry()));
        let over_kg = if sump_dry { 0.0 } else { (sump_kg - self.sump_initial_kg).max(0.0) };
        let p_dw0 = self.net.volume(dw).map_or(prm.initial_containment_pa, |v| v.atmosphere_pressure().get::<pascal>());
        let mut q_sump = 0.0;
        let mut cav_gas = Source::none();
        if self.mech.cavity {
            let (q, gas) = self.debris(s, over_kg, sump_t, p_dw0);
            q_sump = q;
            cav_gas = gas;
        } else if over_kg > 0.0 {
            // The gh:#1097 mapping (the ablation): the heat boils the water
            // on the floor, or, with none, goes into the structures.
            q_sump = s.ex_vessel_w;
        } else {
            self.dw_wall.t_k += s.ex_vessel_w * dt / prm.dw_wall_c;
        }
        if self.mech.cavity {
            // The heat into the sump is capped at what heats it to
            // saturation and boils the water over the debris (the old model
            // caps its pedestal's boiling at its water, `boil_pedestal`);
            // the rest is radiated to the structures, as from a dry floor.
            let t_s = pool_surface::t_sat(p_dw0);
            let h_fg = pool_surface::steam_h(t_s) - CP_WATER * (t_s - 273.15);
            let cap = (sump_kg * CP_WATER * (t_s - sump_t)).max(0.0) / dt + over_kg * h_fg / dt;
            if q_sump > cap {
                self.dw_wall.t_k += (q_sump - cap) * dt / prm.dw_wall_c;
                q_sump = cap;
            }
        }
        self.net.set_pool_heat(dw, Power::new::<watt>(q_sump))?;
        self.net.set_source(dw, DW_CAVITY, cav_gas)?;
        // The structures' outer heat path (sembawang's slab).
        if self.mech.dw_slab && s.dw_wall_outer {
            dw_slab_step(&prm, &mut self.dw_wall.t_k, &mut self.dw_slab, dt);
        }
        // The torus shell's condensate drains to the pool at the shell's
        // temperature (sembawang's), through a port just above the level.
        let h_drain = CP_WATER * (self.ww_shell.t_k - 273.15);
        self.net.set_source(ww, WW_DRAIN, Source::water(kgs(m_ww), AvailableEnergy::new::<joule_per_kilogram>(h_drain)))?;
        // Valves.
        let p_ww = self.net.volume(ww).map_or(0.0, |v| v.atmosphere_pressure().get::<pascal>());
        let p_dw = self.net.volume(dw).map_or(0.0, |v| v.atmosphere_pressure().get::<pascal>());
        if s.vent_ordered && p_ww >= prm.vent_rupture_disk_pa {
            self.disk_burst = true;
        }
        let vent_open = s.vent_ordered && self.disk_burst;
        self.net.set_path_open_fraction(self.vent, if vent_open { 1.0 } else { 0.0 })?;
        if p_dw >= prm.flange_leak_pa {
            self.flange_open = true;
        }
        self.net.set_path_open_fraction(self.flange, if self.flange_open { 1.0 } else { 0.0 })?;
        // Burns in the building (gh:#1100): sembawang's BUR criteria and
        // rate on the start-of-step building, `tampines` applying them and
        // closing the heat. The criterion: `bur_ignites` (or the old
        // threshold with `bur_ignition` ablated in the snapshot), steam and
        // CO2 the diluent. The rate: always BUR's dynamics (the old
        // model's `burn_dynamics`, its default); its instant "explosion"
        // without them removes the fuel with no heat, which a control
        // volume cannot express. The drywell and torus are inerted (N2,
        // and no path brings them oxygen), so their criterion cannot be met.
        if self.mech.burns {
            let (rx, heat_w, q) = self.building_burn(&prm, s.bur_ignition, dt);
            self.net.set_atmosphere_species(rb, rx)?;
            self.net.set_atmosphere_heat(rb, Power::new::<watt>(heat_w))?;
            self.rb_burned.rx_heat_j += q;
        }
        let mut rates = Vec::new();
        let mut sums = Vec::new();
        let adv = advance(&mut self.net, dt, 0, &mut rates, &mut sums)?;
        self.substeps += adv.substeps;
        for (i, v) in sums.iter().enumerate().take(3) {
            self.pool_heat_to_atm_j[i] += v.pool_heat_to_atm_j;
        }
        if let Some(v) = sums.get(rb) {
            self.rb_burned.h2_mol += v.h2_mol;
            self.rb_burned.co_mol += v.co_mol;
        }
        let rate = |i: usize| rates.get(i).map_or(0.0, |r| r[0]);
        if fp_on {
            let gas: Vec<f64> = rates.iter().map(|r| r[1]).collect();
            self.move_fission_products(s, &gas);
        }
        let vented = rate(self.vent);
        let v_ww = self.net.volume(ww).ok_or_else(|| TampinesError::InvalidInput("no wetwell".into()))?;
        let v_dw = self.net.volume(dw).ok_or_else(|| TampinesError::InvalidInput("no drywell".into()))?;
        let v_rb = self.net.volume(rb).ok_or_else(|| TampinesError::InvalidInput("no building".into()))?;
        let h2 = NonCondensable::H2.index();
        let ww_gas = v_ww.atmosphere();
        let x_h2_ww = ww_gas.ncg_moles[h2] * M_H2 / ww_gas.total_mass().get::<kilogram>();
        self.vented_h2_kg += vented.max(0.0) * x_h2_ww * dt;
        let rb_gas = v_rb.atmosphere();
        let n_rb = rb_gas.steam_kg / M_H2O + rb_gas.ncg_total_moles();
        Ok(NetworkResult {
            p_dw_pa: v_dw.atmosphere_pressure().get::<pascal>(),
            p_ww_pa: v_ww.atmosphere_pressure().get::<pascal>(),
            p_rb_pa: v_rb.atmosphere_pressure().get::<pascal>(),
            t_dw_k: v_dw.atmosphere_temperature().get::<kelvin>(),
            t_ww_gas_k: v_ww.atmosphere_temperature().get::<kelvin>(),
            t_pool_k: v_ww.pool_temperature().get::<kelvin>(),
            t_pool_top_k: v_ww.pool_surface_temperature().get::<kelvin>(),
            pool_layers: v_ww.pool_layer_count(),
            pool_kg: v_ww.pool_mass().get::<kilogram>(),
            pool_level_m: v_ww.pool_level().get::<meter>(),
            pool_swollen_level_m: v_ww.swollen_level().get::<meter>(),
            dw_sump_kg: v_dw.pool_mass().get::<kilogram>(),
            dw_sump_dry: v_dw.is_pool_dry(),
            dw_steam_kg: v_dw.atmosphere().steam_kg,
            ww_steam_kg: ww_gas.steam_kg,
            dw_h2_kg: v_dw.atmosphere().ncg_moles[h2] * M_H2,
            ww_h2_kg: ww_gas.ncg_moles[h2] * M_H2,
            rb_h2_fraction: rb_gas.ncg_moles[h2] / n_rb,
            rb_burn_y: (rb_gas.ncg_moles[h2] + rb_gas.ncg_moles[NonCondensable::CO.index()] * prm.bur_xh2ign / prm.bur_xcoign) / n_rb,
            rb_ignites: bur_ignites(
                &prm,
                rb_gas.ncg_moles[h2] / n_rb,
                rb_gas.ncg_moles[NonCondensable::CO.index()] / n_rb,
                rb_gas.ncg_moles[NonCondensable::O2.index()] / n_rb,
                (rb_gas.steam_kg / M_H2O + rb_gas.ncg_moles[NonCondensable::CO2.index()]) / n_rb,
            ),
            rb_burning: self.rb_burn.is_burning(),
            rb_burn_started: self.rb_burn_started,
            rb_h2_burned_kg: self.rb_burned.h2_mol * M_H2,
            rb_co_burned_kg: self.rb_burned.co_mol * NonCondensable::CO.molar_mass(),
            rb_burn_heat_j: self.rb_burned.rx_heat_j,
            pool_heat_to_atmosphere_j: self.pool_heat_to_atm_j,
            vent_flowing: vent_open && vented > 0.0,
            vented_kg_s: vented,
            vented_h2_kg: self.vented_h2_kg,
            flange_leaking: self.flange_open,
            flange_kg_s: rate(self.flange),
            downcomer_kg_s: rate(self.downcomer),
            vacuum_breaker_kg_s: rate(self.vacuum_breaker),
            cav_ablated_kg: self.cavity.ablated_kg,
            cav_h2_kg: self.cavity.h2_kg,
            cav_co_kg: self.cavity.co_kg,
            cav_debris_k: self.cavity.t_k,
            fdi_steam_kg: self.fdi_steam_kg,
            cav_zr_kg: self.cavity.zr_kg,
            fdi_hpme: self.fdi_hpme,
            rb_co_fraction: rb_gas.ncg_moles[NonCondensable::CO.index()] / n_rb,
            rb_burn_dp_pa: self.rb_burn.dp_max_pa(),
            dw_co_kg: v_dw.atmosphere().ncg_moles[NonCondensable::CO.index()] * NonCondensable::CO.molar_mass(),
            dw_co2_kg: v_dw.atmosphere().ncg_moles[NonCondensable::CO2.index()] * NonCondensable::CO2.molar_mass(),
            dw_wall_k: self.dw_wall.t_k,
            dw_slab_inner_k: self.dw_slab[0],
            fp: self.fp.report(),
            fp_heat,
            iterations: adv.iterations,
            residual: adv.residual,
            substeps: self.substeps,
            newton_iterations: adv.newton_iterations,
            newton_fallbacks: adv.newton_fallbacks,
            newton_fell_back: adv.newton_fallbacks > 0,
        })
    }

    /// The building's burn for this step (gh:#1100): sembawang's BUR
    /// ([`bur_ignites`], [`bur_step`], the old model's code) on the
    /// building's start-of-step gas, steam and CO2 the diluent, turned into
    /// `tampines`' species sources and sinks and the heat of reaction by
    /// [`super::reaction::burn_inputs`] (gh:#1103: the chemistry is
    /// sembawang's). Returns the species rates, the heat, W, and the
    /// stored-energy release `Q`, J.
    fn building_burn(&mut self, prm: &Params, bur_ignition: bool, dt: f64) -> (AtmosphereSpeciesRates, f64, f64) {
        let Some(v) = self.net.volume(self.rb) else { return (AtmosphereSpeciesRates::none(), 0.0, 0.0) };
        let g = v.atmosphere();
        let n = g.steam_kg / M_H2O + g.ncg_total_moles();
        let (ih, ic, io, ic2) = (NonCondensable::H2.index(), NonCondensable::CO.index(), NonCondensable::O2.index(), NonCondensable::CO2.index());
        let (h2, co, o2) = (g.ncg_moles[ih], g.ncg_moles[ic], g.ncg_moles[io]);
        let x_d = (g.steam_kg / M_H2O + g.ncg_moles[ic2]) / n;
        let ignites = if bur_ignition { bur_ignites(prm, h2 / n, co / n, o2 / n, x_d) } else { h2 / n >= prm.h2_explosion };
        let (d_h2, d_co, started) = bur_step(prm, &mut self.rb_burn, ignites, h2, co, n, x_d, o2 / n, prm.reactor_building_m3, dt);
        self.rb_burn_started |= started;
        if d_h2 == 0.0 && d_co == 0.0 {
            return (AtmosphereSpeciesRates::none(), 0.0, 0.0);
        }
        match super::reaction::burn_inputs(v, d_h2, d_co, dt) {
            Ok((rates, heat_w, _, q)) => (rates, heat_w, q),
            // `bur_step`'s amounts are finite and non-negative, so this is
            // unreachable in practice; nothing burns rather than a panic.
            Err(_) => (AtmosphereSpeciesRates::none(), 0.0, 0.0),
        }
    }

    /// The building's burn state (sembawang's BUR; read-only).
    pub fn building_burn_state(&self) -> Burn {
        self.rb_burn
    }

    /// The torus pool's liquid temperature at elevation `z`, K: the layer
    /// there (gh:#1100), the pool's for one layer.
    fn liquid_t_at(&self, z: f64) -> f64 {
        self.net.volume(self.ww).map_or(self.prm.initial_pool_k, |v| v.phase_states_at(len(z)).0.temperature.get::<kelvin>())
    }

    /// The torus pool as the scrubbing sees it: the temperature of the
    /// water at elevation `z` (the quenchers', the downcomers' exits) or,
    /// with `None`, of the surface layer (the aerosol step's pool surface);
    /// the torus pressure and its water. One layer: the pool's temperature
    /// in every case. (The old model takes its top layer for all three.)
    fn pool_side(&self, z: Option<f64>) -> PoolSide {
        self.net.volume(self.ww).map_or(PoolSide { t_k: self.prm.initial_pool_k, p_ww_pa: self.prm.initial_containment_pa, water_kg: 0.0 }, |v| PoolSide {
            t_k: match z {
                Some(z) => self.liquid_t_at(z),
                None => v.pool_surface_temperature().get::<kelvin>(),
            },
            p_ww_pa: v.atmosphere_pressure().get::<pascal>(),
            water_kg: v.pool_mass().get::<kilogram>(),
        })
    }

    /// The torus pool's layer count (1 without `pool_layers`).
    pub fn pool_layer_count(&self) -> usize {
        self.net.volume(self.ww).map_or(1, |v| v.pool_layer_count())
    }

    /// The downcomers' exits and the quenchers' port above the torus
    /// bottom, m.
    pub fn port_elevations(&self) -> (f64, f64) {
        (self.z_exit, self.z_quencher)
    }

    /// The fission products ride the gas each path moved this step
    /// (`gas`, each path's mean gas mass rate, kg/s, positive `from → to`;
    /// the module docs state the donor-cell split), then, every 10 s, the
    /// aerosol step deposits them (`FpHold::step_rn`, the old model's) with
    /// the network's end-of-step volumes.
    fn move_fission_products(&mut self, s: &ContainmentSnapshot, gas: &[f64]) {
        let prm = self.prm;
        let dt = s.dt_s;
        let g = |i: usize| gas.get(i).copied().unwrap_or(0.0) * dt;
        let (dc, vb, vent, flange) = (g(self.downcomer), g(self.vacuum_breaker), g(self.vent), g(self.flange));
        let mass = |i: usize| self.net.volume(i).map_or(0.0, |v| v.atmosphere().total_mass().get::<kilogram>());
        // Each donor's outflows (the vacuum breakers and the vent never
        // flow back; a downcomer's backflow would leave the torus gas).
        let dw_out = [dc.max(0.0), flange.max(0.0)];
        let ww_out = [vb.max(0.0), vent.max(0.0), (-dc).max(0.0)];
        let dw_avail = mass(self.dw) + dw_out.iter().sum::<f64>();
        let ww_avail = mass(self.ww) + ww_out.iter().sum::<f64>();
        let share = |m: f64, avail: f64, left: &mut f64| -> f64 {
            if m <= 0.0 || avail <= 0.0 || *left <= 0.0 {
                return 0.0;
            }
            let f = m / avail;
            let of_left = (f / *left).min(1.0);
            *left -= f;
            of_left
        };
        // The drywell's: the downcomers (scrubbed), the flange.
        let mut left = 1.0;
        let f_dc = share(dw_out[0], dw_avail, &mut left);
        if f_dc > 0.0 {
            if let Some(v) = self.net.volume(self.dw) {
                let a = v.atmosphere();
                let n_h2 = a.ncg_moles[NonCondensable::H2.index()];
                let n_nc = a.ncg_total_moles();
                let scale = dw_out[0] / a.total_mass().get::<kilogram>();
                let (dn2, dh2, dsteam) = (scale * (n_nc - n_h2), scale * n_h2, scale * a.steam_kg);
                let flow = DowncomerFlow {
                    f: f_dc,
                    n: dn2 + dh2 + dsteam / M_H2O,
                    dn2,
                    dh2,
                    dsteam_kg: dsteam,
                    t_dw_k: v.atmosphere_temperature().get::<kelvin>(),
                    p_dw_pa: v.atmosphere_pressure().get::<pascal>(),
                };
                let side = self.pool_side(Some(self.z_exit));
                self.fp.downcomer(&prm, s, flow, side);
            }
        }
        let f_fl = share(dw_out[1], dw_avail, &mut left);
        if f_fl > 0.0 {
            self.fp.flange(f_fl);
        }
        // The torus gas's: the vacuum breakers, the vent, any backflow.
        let mut left = 1.0;
        let f_vb = share(ww_out[0], ww_avail, &mut left);
        if f_vb > 0.0 {
            self.fp.ww_to_dw(f_vb);
        }
        let f_vent = share(ww_out[1], ww_avail, &mut left);
        if f_vent > 0.0 {
            self.fp.vent(f_vent, f_vent);
        }
        let f_back = share(ww_out[2], ww_avail, &mut left);
        if f_back > 0.0 {
            self.fp.ww_to_dw(f_back);
        }
        // The aerosol step, every 10 s.
        if let Some(h) = self.fp.rn_due(dt) {
            if let Some(c) = self.rn_context(s) {
                self.fp.step_rn(&prm, &c, h, s.phoresis);
            }
        }
    }

    /// The aerosol step's view of the network (end of step).
    fn rn_context(&self, s: &ContainmentSnapshot) -> Option<RnContext> {
        let prm = self.prm;
        let (vd, vw) = (self.net.volume(self.dw)?, self.net.volume(self.ww)?);
        let (a, b) = (vd.atmosphere(), vw.atmosphere());
        let h2 = NonCondensable::H2.index();
        let t_dw = vd.atmosphere_temperature().get::<kelvin>();
        let p_dw = vd.atmosphere_pressure().get::<pascal>();
        let h_wall = if s.dw_wall_hs {
            wall_surface(&a, t_dw, p_dw, self.dw_wall.t_k, prm.dw_wall_height_m, self.mech.ncg_wall_properties).0
        } else {
            prm.dw_wall_h
        };
        let n_ww = b.steam_kg / M_H2O + b.ncg_total_moles();
        Some(RnContext {
            dw_steam_kg: a.steam_kg,
            dw_n2: a.ncg_total_moles() - a.ncg_moles[h2],
            dw_h2: a.ncg_moles[h2],
            dw_t_k: t_dw,
            p_dw_pa: p_dw,
            dw_gas_m3: vd.atmosphere_volume().get::<cubic_meter>(),
            h_wall,
            dw_wall_t_k: self.dw_wall.t_k,
            pool: self.pool_side(None),
            ww_x_steam: if n_ww > 0.0 { b.steam_kg / M_H2O / n_ww } else { 0.0 },
            ww_t_k: vw.atmosphere_temperature().get::<kelvin>(),
            ww_n2: b.ncg_total_moles() - b.ncg_moles[h2],
            ww_h2: b.ncg_moles[h2],
            pool_area_m2: vw.table().area_at(vw.pool_level()).get::<square_meter>(),
        })
    }

    /// One step of sembawang's cavity under the drywell (gh:#1100), from
    /// the start-of-step state: `over_kg` of water over the debris at
    /// `sump_t` K, the drywell at `p_dw` Pa. Debris newly arrived falls
    /// through that water first (sembawang's FDI fall, `lpme_fall`), its
    /// heat into the water; then the cavity steps (`Cavity::step`, as
    /// `Containment::step` calls it). Returns the heat into the sump, W
    /// (the fall's and, wet, the debris top's), and the concrete gases as a
    /// source on the drywell atmosphere; dry, the top's heat goes into the
    /// structures here.
    fn debris(&mut self, s: &ContainmentSnapshot, over_kg: f64, sump_t: f64, p_dw: f64) -> (f64, Source) {
        let prm = self.prm;
        let dt = s.dt_s;
        if s.ex_vessel_w <= 0.0 {
            return (0.0, Source::none());
        }
        let mut q_sump = 0.0;
        let du = (s.cav_uo2_kg - self.cavity.seen_uo2_kg).max(0.0);
        let dz = (s.cav_zr_kg - self.cavity.seen_zr_kg).max(0.0);
        let t_in = if self.mech.fdi && s.fdi_lpme && over_kg > 0.0 && du + dz > 0.0 {
            let f = lpme_fall(&prm, s, du, dz, p_dw, over_kg, sump_t);
            self.fdi_steam_kg += f.steam_kg;
            self.fdi_hpme |= f.hpme;
            q_sump += f.steam_kg * f.dh_j_per_kg / dt;
            f.t_bottom_k
        } else {
            s.cav_debris_k
        };
        self.cavity.arrive(&prm, s.cav_uo2_kg, s.cav_zr_kg, t_in);
        let water = (over_kg > 0.0).then(|| (pool_surface::t_sat(p_dw), p_dw));
        let f = self.cavity.step(&prm, s.ex_vessel_w, self.dw_wall.t_k, water, dt, s.cavity_phases);
        if water.is_some() {
            q_sump += f.q_up_w;
        } else {
            self.dw_wall.t_k += f.q_up_w * dt / prm.dw_wall_c;
        }
        let t = f.gas_k.max(1.0);
        let n_tot = f.steam_kg / M_H2O + f.h2_mol + f.co_mol + f.co2_mol;
        let p_steam = if n_tot > 0.0 { p_dw * f.steam_kg / M_H2O / n_tot } else { p_dw };
        let h_steam = steam_h_if97(t, p_steam.max(1.0));
        let gas = Source::water(kgs(f.steam_kg / dt), AvailableEnergy::new::<joule_per_kilogram>(h_steam)).with_gas(
            &[
                (NonCondensable::H2, kgs(f.h2_mol * NonCondensable::H2.molar_mass() / dt)),
                (NonCondensable::CO, kgs(f.co_mol * NonCondensable::CO.molar_mass() / dt)),
                (NonCondensable::CO2, kgs(f.co2_mol * NonCondensable::CO2.molar_mass() / dt)),
            ],
            kel(t),
        );
        (q_sump, gas)
    }
}

/// IF97 steam's specific enthalpy at `t` K and `p` Pa: region 2 to
/// 1073.15 K, region 5 above (the concrete's gases leave at
/// `concrete_ablation_k`, 1450 K, beyond region 2).
fn steam_h_if97(t: f64, p: f64) -> f64 {
    let (tt, pp) = (kel(t), Pressure::new::<pascal>(p));
    if t <= 1073.15 { h_tp_2(tt, pp) } else { h_tp_5(tt, pp) }.get::<joule_per_kilogram>()
}
