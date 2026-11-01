//! The Mark I containment on `tampines`'s control-volume network (gh:#1097,
//! epic gh:#1091): drywell, wetwell (torus) and reactor building as
//! [`ControlVolume`]s, the downcomers, vacuum breakers, wetwell vent and
//! drywell head-flange leak as [`FlowPath`]s, the stack and the environment
//! as boundaries. After MELCOR 2.2's CVH/FL packages (Reference Manual §2,
//! §5, §6; cited by section, not quoted; no MELCOR source used).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **Built alongside [`super::containment::Containment`], not replacing
//! it.** The plant ([`super::Plant`]) still runs the old model and its
//! fixtures are unchanged. [`ContainmentNetwork::step`] takes the same
//! per-step input ([`ContainmentSnapshot`]) and returns comparable outputs
//! ([`NetworkResult`]), so the two can be run side by side
//! (`tests/containment_network_vs_containment.rs`; the evidence is in
//! `verification_and_validation/bwr_mark1/containment_network.md`).
//!
//! ## What is mapped, and what is not (stated)
//!
//! | Snapshot input | Here |
//! |---|---|
//! | relief steam, enthalpy, H2 (at `srv_gas_k` with `relief_h2_enthalpy`, else the pool's temperature) | a source at the quenchers' port in the torus pool |
//! | RPV leak steam, enthalpy, H2 at `leak_gas_k` | a source in the drywell atmosphere |
//! | `vent_ordered` | the vent valve: open once ordered and the rupture disk has burst (`p_ww ≥ vent_rupture_disk_pa`) |
//! | `rpv_wall_w` | heat into the drywell atmosphere |
//! | `pedestal_inflow_kg_s`, `vessel_drain_kg_s` | water into the drywell sump |
//! | ~~`ex_vessel_w` … the heat into it while it holds more than its initial fill, else into the drywell structures (no cavity, no MCCI, no concrete gases)~~ | **CORRECTED 2026-11-01 (gh:#1100):** see the next rows |
//! | `ex_vessel_w`, `cav_uo2_kg`, `cav_zr_kg`, `cav_debris_k`, `cavity_phases` | sembawang's own [`super::cavity::Cavity`] (MELCOR CAV RM §3, reused, not rewritten): its upward heat into the sump while water lies over the debris (the sump beyond its initial fill), else radiated to the drywell structures; its concrete gases (steam, H2, CO, CO2) into the drywell atmosphere at the ablation temperature ([`NetworkMechanisms::cavity`]) |
//! | `vessel_p_pa` and the debris arriving through water | sembawang's FDI fall (`containment::lpme_fall` (crate-private), MELCOR FDI RM §2.2): its heat into the sump ([`NetworkMechanisms::fdi`]) |
//! | `dw_wall_outer` | sembawang's drywell slab (`containment::dw_slab_step` (crate-private)) behind the lumped structures ([`NetworkMechanisms::dw_slab`]) |
//! | `dw_wall_hs`, `ww_shell` | the drywell structures and the torus shell as lumped walls on the atmospheres, by the same MELCOR surface ([`pool_surface::structure_surface`]) |
//! | fission products, aerosols, iodine, decay heat of fission products (`q_*_w`), burns, ~~MCCI, FDI,~~ `pool_layers`~~, `dw_wall_outer`~~ | **not mapped**: sembawang's, and not yet on the network (MCCI, FDI and the slab **CORRECTED 2026-11-01**: mapped, rows above) |
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
//!   opening the downcomer's diameter (0.515 m). The quenchers' port is
//!   put at the same depth, as sembawang does (\[A\], their geometry is
//!   not held).
//! - **Drywell:** a floor of `dw_floor_m2` (200 m² \[A\]) carrying a
//!   prismatic volume of `drywell_volume_m3` (\[A\] the light bulb's shape is
//!   not held) above a sump pit of 2 m² × 1.5 m holding 1 m³ \[A\] (a
//!   control volume needs a pool); the floor at the torus top \[A\]
//!   ([`NetworkConfig::drywell_floor_m`]), the downcomers' entry 2 m above
//!   it \[A\], the head flange at the top.
//! - **Reactor building:** `reactor_building_m3` on a 650 m² floor \[A\]
//!   (40 m tall, so it spans the drywell head) above a 2 m² sump pit with
//!   1 m³ \[A\], dry air at one atmosphere and 303.15 K (sembawang's
//!   `T_RB`), open to the environment through 1 m² \[A\] near its top
//!   (sembawang holds its building at one atmosphere exactly).
//! - **Paths:** sembawang's orifice `C_d A √(2ρΔp)` is the steady state of a
//!   path of area `A`, discharge coefficient `C_d` and form loss `K = 1`,
//!   so each path takes `params`' area and coefficient with `K = 1`; the
//!   inertial lengths are \[A\]: downcomers 8 m, vacuum breakers 1 m, vent
//!   60 m, flange 0.5 m, building openings 1 m. The vent is a check valve
//!   (sembawang's vent never flows back); the vacuum breakers open at
//!   `vacuum_breaker_pa`.

use tampines::control_volume::{ControlVolume, GasMixture, NonCondensable, Port, Source, VolumeAltitudeTable};
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

use super::cavity::Cavity;
use super::containment::{dw_slab_step, lpme_fall, ContainmentSnapshot, DW_SLAB_CELLS};
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
/// (see the module docs); Units 2 and 3 would differ here and in `Params`
/// (the V&V doc lists the data that would be needed).
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
    pub sump_area_m2: f64,
    pub sump_depth_m: f64,
    pub sump_water_m3: f64,
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
            sump_water_m3: 1.0,
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
    /// it, the debris's heat boils the sump while it holds more than its
    /// fill, else goes into the structures (the gh:#1097 mapping).
    pub cavity: bool,
    /// sembawang's FDI fall of arriving debris through the water over the
    /// floor (needs `cavity`).
    pub fdi: bool,
    /// sembawang's concrete slab behind the drywell structures.
    pub dw_slab: bool,
}

impl NetworkMechanisms {
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
        }
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
    pub t_pool_k: f64,
    /// Torus pool: mass, kg, collapsed and swollen level, m (plant datum).
    pub pool_kg: f64,
    pub pool_level_m: f64,
    pub pool_swollen_level_m: f64,
    /// Water in the drywell sump, kg.
    pub dw_sump_kg: f64,
    pub dw_steam_kg: f64,
    pub ww_steam_kg: f64,
    pub dw_h2_kg: f64,
    pub ww_h2_kg: f64,
    /// Hydrogen's mole fraction in the building's gas (steam included).
    pub rb_h2_fraction: f64,
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
    /// CO and CO2 in the drywell atmosphere, kg.
    pub dw_co_kg: f64,
    pub dw_co2_kg: f64,
    /// The drywell structures' temperature and the slab's inner cell, K.
    pub dw_wall_k: f64,
    pub dw_slab_inner_k: f64,
    /// Picard iterations this step (summed over sub-steps), the largest
    /// residual, and the sub-steps taken so far in the run.
    pub iterations: usize,
    pub residual: f64,
    pub substeps: usize,
}

/// Deepest halving of a step that does not converge (a 1 s step down to
/// 1/64 s).
const MAX_HALVINGS: usize = 6;

/// One network step of `dt`; if the Picard iteration does not converge,
/// two of `dt / 2`, recursively, up to [`MAX_HALVINGS`] (as MELCOR's
/// executive cuts its step when a package fails to converge, EXEC RM,
/// cited). The network commits nothing on a failed step, and the held
/// sources and heats are rates, so the halves see the same drive. Writes
/// each path's mean mass rate over `dt` into `rates`; returns the Picard
/// iterations, the largest residual and the extra sub-steps taken.
fn advance(net: &mut CvNetwork, dt: f64, depth: usize, rates: &mut Vec<f64>) -> Result<(usize, f64, usize), TampinesError> {
    match net.step(Time::new::<second>(dt)) {
        Ok(r) => {
            *rates = r.transfers.iter().map(|t| t.gas_mass_rate.get::<kilogram_per_second>() + t.liquid_mass_rate.get::<kilogram_per_second>()).collect();
            Ok((r.iterations, r.residual, 0))
        }
        Err(TampinesError::Numerical(_)) if depth < MAX_HALVINGS => {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            let (i1, r1, s1) = advance(net, 0.5 * dt, depth + 1, &mut a)?;
            let (i2, r2, s2) = advance(net, 0.5 * dt, depth + 1, &mut b)?;
            *rates = a.iter().zip(&b).map(|(x, y)| 0.5 * (x + y)).collect();
            Ok((i1 + i2, r1.max(r2), 1 + s1 + s2))
        }
        Err(e) => Err(e),
    }
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
    mech: NetworkMechanisms,
    cavity: Cavity,
    fdi_steam_kg: f64,
    dw_slab: [f64; DW_SLAB_CELLS],
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
        let ww_ports = vec![
            Port { elevation: len(exit), opening_height: len(d_dc) },
            Port { elevation: len(z_ww_gas), opening_height: len(0.5) },
            Port { elevation: len(z_ww_gas), opening_height: len(0.3) },
            Port { elevation: len(exit), opening_height: len(d_dc) },
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
        let ww = net.add_volume(ablate(ww, &mech)?);
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
            mech,
            cavity: Cavity::default(),
            fdi_steam_kg: 0.0,
            dw_slab: [t_dw; DW_SLAB_CELLS],
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
        let n_h2 = a.ncg_moles[NonCondensable::H2.index()];
        let atm = pool_surface::Atmosphere {
            t_k: t_g,
            p_pa: cv.atmosphere_pressure().get::<pascal>(),
            n_steam: a.steam_kg / M_H2O,
            n_n2: a.ncg_total_moles() - n_h2,
            n_h2,
        };
        let (h, m) = match fixed_h {
            Some(h) => (h, 0.0),
            None => {
                let (h, flux) = pool_surface::structure_surface(&atm, wall.t_k, height);
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
        let t_pool = self.net.volume(ww).map_or(prm.initial_pool_k, |v| v.pool_temperature().get::<kelvin>());
        // Sources.
        let t_h2 = if s.relief_h2_enthalpy { s.srv_gas_k } else { t_pool };
        let relief = Source::water(kgs(s.srv_steam_kg_s), AvailableEnergy::new::<joule_per_kilogram>(s.srv_h))
            .with_gas(&[(NonCondensable::H2, kgs(s.srv_h2_kg_s))], kel(t_h2.max(1.0)));
        self.net.set_source(ww, WW_QUENCHER, relief)?;
        let leak = Source::water(kgs(s.leak_steam_kg_s), AvailableEnergy::new::<joule_per_kilogram>(s.leak_h))
            .with_gas(&[(NonCondensable::H2, kgs(s.leak_h2_kg_s))], kel(s.leak_gas_k.max(1.0)));
        self.net.set_source(dw, DW_LEAK, leak)?;
        // Structures (explicit, from the start-of-step state).
        let (q_dw, m_dw, q_wall) = self.wall_exchange(dw, self.dw_wall, prm.dw_wall_area_m2, prm.dw_wall_height_m, (!s.dw_wall_hs).then_some(prm.dw_wall_h), dt);
        let (q_ww, m_ww, q_shell) = if s.ww_shell { self.wall_exchange(ww, self.ww_shell, prm.ww_shell_area_m2, prm.ww_shell_height_m, None, dt) } else { (0.0, 0.0, 0.0) };
        self.dw_wall.t_k += q_wall * dt / prm.dw_wall_c;
        self.ww_shell.t_k += q_shell * dt / prm.ww_shell_c;
        self.net.set_atmosphere_heat(dw, Power::new::<watt>(q_dw + s.rpv_wall_w))?;
        self.net.set_atmosphere_steam_sink(dw, kgs(m_dw))?;
        self.net.set_atmosphere_heat(ww, Power::new::<watt>(q_ww))?;
        self.net.set_atmosphere_steam_sink(ww, kgs(m_ww))?;
        // Water into the drywell sump: the structures' condensate at the
        // wall's temperature, the pedestal's inflow, the failed vessel's
        // drain (one stream, mass-weighted enthalpy, sembawang's c_p).
        let streams = [(m_dw, self.dw_wall.t_k), (s.pedestal_inflow_kg_s, prm.injection_temperature_k), (s.vessel_drain_kg_s, s.vessel_drain_k)];
        let m_sump: f64 = streams.iter().map(|x| x.0).sum();
        let h_sump = if m_sump > 0.0 { streams.iter().map(|(m, t)| m * CP_WATER * (t - 273.15)).sum::<f64>() / m_sump } else { 0.0 };
        self.net.set_source(dw, DW_SUMP, Source::water(kgs(m_sump), AvailableEnergy::new::<joule_per_kilogram>(h_sump)))?;
        // Debris on the floor (after the lower head fails). Water "on the
        // floor" is what the sump holds beyond its initial fill [A] (a
        // control volume needs a pool, so the sump never dries).
        let (sump_kg, sump_t) = self.net.volume(dw).map_or((0.0, t_pool), |v| (v.pool_mass().get::<kilogram>(), v.pool_temperature().get::<kelvin>()));
        let over_kg = (sump_kg - self.sump_initial_kg).max(0.0);
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
            // saturation and boils the water over the fill (the old model
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
        let mut rates = Vec::new();
        let (iterations, residual, substeps) = advance(&mut self.net, dt, 0, &mut rates)?;
        self.substeps += substeps;
        let rate = |i: usize| rates.get(i).copied().unwrap_or(0.0);
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
            pool_kg: v_ww.pool_mass().get::<kilogram>(),
            pool_level_m: v_ww.pool_level().get::<meter>(),
            pool_swollen_level_m: v_ww.swollen_level().get::<meter>(),
            dw_sump_kg: v_dw.pool_mass().get::<kilogram>(),
            dw_steam_kg: v_dw.atmosphere().steam_kg,
            ww_steam_kg: ww_gas.steam_kg,
            dw_h2_kg: v_dw.atmosphere().ncg_moles[h2] * M_H2,
            ww_h2_kg: ww_gas.ncg_moles[h2] * M_H2,
            rb_h2_fraction: rb_gas.ncg_moles[h2] / n_rb,
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
            dw_co_kg: v_dw.atmosphere().ncg_moles[NonCondensable::CO.index()] * NonCondensable::CO.molar_mass(),
            dw_co2_kg: v_dw.atmosphere().ncg_moles[NonCondensable::CO2.index()] * NonCondensable::CO2.molar_mass(),
            dw_wall_k: self.dw_wall.t_k,
            dw_slab_inner_k: self.dw_slab[0],
            iterations,
            residual,
            substeps: self.substeps,
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
