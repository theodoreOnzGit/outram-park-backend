// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **HTR-10 temperature zones** (gh:#1138, epic #1137).
//!
//! The recorded model runs every material and every nuclide at one
//! temperature (300.15 K). This module splits the core into the eight zones
//! the maintainer set on 2026-11-03, each with a temperature of its own:
//!
//! | zone | material slots |
//! |---|---|
//! | [`Htr10Zone::FuelPebble`] | the UO₂ kernel, the four TRISO coatings, the matrix graphite and the shell ([`mat::KERNEL`] to [`mat::GRAPHITE`]) |
//! | [`Htr10Zone::ModeratorPebble`] | the graphite (dummy) balls, in the bed and the discharge tube ([`mat::MODERATOR_BALL`]; [`mat::HOMOG_DUMMY`] under its ablation) |
//! | [`Htr10Zone::Coolant`] | helium, everywhere it is ([`mat::HELIUM`]) |
//! | [`Htr10Zone::TopReflector`], [`Htr10Zone::SideReflector`], [`Htr10Zone::BottomReflector`] | the reflector composition slots, one copy per region ([`mat::regional_slot`]); each Fig. 4.10 box is placed by [`region_of_box`] |
//! | [`Htr10Zone::ControlRods`] | B₄C, steel sleeves, iron joints ([`mat::ROD_B4C`], [`mat::ROD_STEEL`], [`mat::ROD_IRON`]) |
//! | [`Htr10Zone::BoronatedBricks`] | the boronated carbon bricks ([`mat::BORONATED`]) |
//!
//! A zone's temperature sets the temperature its nuclides are processed at
//! (RECONR, BROADR, HEATR, PURR) and its bound thermal laws are generated at
//! (LEAPR), the materials' `temperature`, the cells' `temperature`, and, for
//! the coolant, the helium density (an ideal gas at the model's pressure).
//! [`split_by_temperature`] turns materials at several temperatures into one
//! nuclide array that holds each distinct (nuclide, temperature) once.
//!
//! Orchestration only: no physics is implemented here.

use outram_mc_libs::geometry::cell::CellFill;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use super::core_model::mat;
use super::data::Htr10NuclideLayout;
use super::materials::{htr10_material_set, Htr10MaterialConfig};
use super::reflector_geometry::Fig410Box;

/// One of the eight temperature zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Htr10Zone {
    /// The whole fuel pebble: kernel, TRISO coatings, matrix and shell.
    FuelPebble,
    /// The graphite (dummy) balls.
    ModeratorPebble,
    /// The helium coolant.
    Coolant,
    /// Reflector above the core cavity.
    TopReflector,
    /// Reflector beside the bed.
    SideReflector,
    /// Reflector below the bed, the conus region included.
    BottomReflector,
    /// The control rods: B₄C, steel sleeves and iron joints.
    ControlRods,
    /// The boronated carbon bricks.
    BoronatedBricks,
}

impl Htr10Zone {
    /// Every zone, in slider order.
    pub const ALL: [Self; 8] = [
        Self::FuelPebble,
        Self::ModeratorPebble,
        Self::Coolant,
        Self::TopReflector,
        Self::SideReflector,
        Self::BottomReflector,
        Self::ControlRods,
        Self::BoronatedBricks,
    ];

    /// Position in [`Self::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Name for a slider or a log line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::FuelPebble => "Fuel pebbles",
            Self::ModeratorPebble => "Moderator pebbles",
            Self::Coolant => "Helium coolant",
            Self::TopReflector => "Top reflector",
            Self::SideReflector => "Side reflector",
            Self::BottomReflector => "Bottom reflector",
            Self::ControlRods => "Control rods",
            Self::BoronatedBricks => "Boronated bricks",
        }
    }

    /// Short machine name (`fuel`, `moderator`, ...), for the CLI and URLs.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::FuelPebble => "fuel",
            Self::ModeratorPebble => "moderator",
            Self::Coolant => "coolant",
            Self::TopReflector => "top",
            Self::SideReflector => "side",
            Self::BottomReflector => "bottom",
            Self::ControlRods => "rods",
            Self::BoronatedBricks => "bricks",
        }
    }

    /// The zone whose [`Self::key`] is `k`.
    #[must_use]
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|z| z.key() == k)
    }
}

/// Reflector region, from a Fig. 4.10 box's place relative to the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectorRegion {
    /// Wholly above the core cavity's top.
    Top,
    /// Overlapping the height of the cavity and the bed.
    Side,
    /// Wholly below the bed floor.
    Bottom,
}

impl ReflectorRegion {
    /// 0 top, 1 side, 2 bottom: the index [`mat::regional_slot`] takes.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Top => 0,
            Self::Side => 1,
            Self::Bottom => 2,
        }
    }

    /// The temperature zone of this region.
    #[must_use]
    pub fn zone(self) -> Htr10Zone {
        match self {
            Self::Top => Htr10Zone::TopReflector,
            Self::Side => Htr10Zone::SideReflector,
            Self::Bottom => Htr10Zone::BottomReflector,
        }
    }
}

/// The core's height band in Fig. 4.10's `z_T` (cm, measured down from the
/// model's top): the core cavity's top (130 cm) to the bed floor at the
/// conus top (388.764 cm). The same band bounds the KLAK slots
/// ([`super::reflector_geometry::KLAK_SLOT_ZT_CM`]).
pub const CORE_BAND_ZT_CM: (f64, f64) = super::reflector_geometry::KLAK_SLOT_ZT_CM;

/// **Region of a Fig. 4.10 box**, by height alone: wholly above the core
/// band is the top reflector, wholly below it the bottom reflector, and any
/// box overlapping it the side reflector. So the corners go with the top and
/// bottom: the top corners sit by the cold-helium plenum and the bottom ones
/// by the hot-gas region, which is where their temperatures come from.
#[must_use]
pub fn region_of_box(b: &Fig410Box) -> ReflectorRegion {
    let (top, bottom) = CORE_BAND_ZT_CM;
    if b.zt_bot <= top {
        ReflectorRegion::Top
    } else if b.zt_top >= bottom {
        ReflectorRegion::Bottom
    } else {
        ReflectorRegion::Side
    }
}

/// **The temperature zone of a material slot** of a zoned design (any slot
/// below [`mat::ZONED_COUNT`]).
///
/// The base composition slots ([`mat::REFLECTOR`] and the Table 4-3 slots)
/// are not placed by a zoned design, which uses their regional copies; they
/// answer [`Htr10Zone::SideReflector`], as does the unused
/// [`mat::BORED_GRAPHITE`] (the side reflector's bored zones).
///
/// # Panics
/// For a slot at or above [`mat::ZONED_COUNT`].
#[must_use]
pub fn zone_of_slot(slot: usize) -> Htr10Zone {
    use Htr10Zone as Z;
    if let Some(r) = mat::region_of_regional(slot) {
        return [Z::TopReflector, Z::SideReflector, Z::BottomReflector][r];
    }
    match slot {
        mat::KERNEL | mat::BUFFER | mat::IPYC | mat::SIC | mat::OPYC | mat::GRAPHITE => {
            Z::FuelPebble
        }
        mat::MODERATOR_BALL | mat::HOMOG_DUMMY => Z::ModeratorPebble,
        mat::HELIUM => Z::Coolant,
        mat::BORONATED => Z::BoronatedBricks,
        mat::ROD_B4C | mat::ROD_STEEL | mat::ROD_IRON => Z::ControlRods,
        s if s < mat::COUNT => Z::SideReflector,
        s => panic!("material slot {s} is not a slot of the zoned HTR-10 model"),
    }
}

/// The lowest temperature a zone may be set to: 20 °C, the B1 benchmark
/// core's cold critical state.
#[must_use]
pub fn min_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(20.0)
}

/// The highest temperature a zone may be set to: 1800 °C (maintainer,
/// 2026-11-03: a little above the TRISO limit).
#[must_use]
pub fn max_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(1800.0)
}

/// Above this a zone is warned about: 1600 °C, the TRISO fuel limit.
#[must_use]
pub fn triso_limit() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(1600.0)
}

/// **A temperature for each of the eight zones.**
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneTemperatures {
    kelvin: [f64; 8],
}

impl ZoneTemperatures {
    /// Every zone at `t`.
    #[must_use]
    pub fn uniform(t: ThermodynamicTemperature) -> Self {
        Self {
            kelvin: [t.get::<kelvin>(); 8],
        }
    }

    /// The recorded model's: every zone at 300.15 K.
    #[must_use]
    pub fn record() -> Self {
        Self::uniform(ThermodynamicTemperature::new::<kelvin>(300.15))
    }

    /// From one temperature per zone in K, in [`Htr10Zone::ALL`] order: the
    /// wire form a browser worker receives. [`Self::check`] it before use.
    #[must_use]
    pub fn from_kelvin(k: [f64; 8]) -> Self {
        Self { kelvin: k }
    }

    /// One temperature per zone in K, in [`Htr10Zone::ALL`] order.
    #[must_use]
    pub fn to_kelvin(&self) -> [f64; 8] {
        self.kelvin
    }

    /// Zone `z`'s temperature.
    #[must_use]
    pub fn get(&self, z: Htr10Zone) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.kelvin[z.index()])
    }

    /// Zone `z`'s temperature, K.
    #[must_use]
    pub fn kelvin(&self, z: Htr10Zone) -> f64 {
        self.kelvin[z.index()]
    }

    /// Set zone `z` to `t`.
    pub fn set(&mut self, z: Htr10Zone, t: ThermodynamicTemperature) {
        self.kelvin[z.index()] = t.get::<kelvin>();
    }

    /// This, with zone `z` at `t`.
    #[must_use]
    pub fn with(mut self, z: Htr10Zone, t: ThermodynamicTemperature) -> Self {
        self.set(z, t);
        self
    }

    /// Whether every zone is at one temperature.
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        self.kelvin.iter().all(|&t| t.to_bits() == self.kelvin[0].to_bits())
    }

    /// The zones above the TRISO limit ([`triso_limit`]).
    #[must_use]
    pub fn above_triso_limit(&self) -> Vec<Htr10Zone> {
        let lim = triso_limit().get::<kelvin>();
        Htr10Zone::ALL
            .into_iter()
            .filter(|&z| self.kelvin(z) > lim)
            .collect()
    }

    /// Refuse a temperature outside [`min_temperature`]..=[`max_temperature`]
    /// or not finite.
    ///
    /// # Errors
    /// A message naming the zone and its temperature.
    pub fn check(&self) -> Result<(), String> {
        let (lo, hi) = (min_temperature().get::<kelvin>(), max_temperature().get::<kelvin>());
        for z in Htr10Zone::ALL {
            let t = self.kelvin(z);
            // A small tolerance for °C ↔ K round trips at the ends.
            if !t.is_finite() || t < lo - 1.0e-9 || t > hi + 1.0e-9 {
                return Err(format!(
                    "{}: {:.2} K is outside {:.2}..={:.2} K (20..=1800 °C)",
                    z.label(),
                    t,
                    lo,
                    hi
                ));
            }
        }
        Ok(())
    }
}

/// **The material set of a zoned design**: [`htr10_material_set`]'s slots,
/// then [`mat::MODERATOR_BALL`] and the regional copies (slot order up to
/// [`mat::ZONED_COUNT`]), each at its zone's temperature. The helium density
/// is the ideal gas at the coolant's temperature and the model's pressure.
///
/// Every component still indexes `layout`'s slots (one nuclide per slot, at
/// no particular temperature): [`split_by_temperature`] then gives each
/// (slot, temperature) pair a nuclide of its own.
#[must_use]
pub fn zoned_material_set(
    layout: &Htr10NuclideLayout,
    cfg: Htr10MaterialConfig,
    temps: &ZoneTemperatures,
) -> Vec<Material> {
    let coolant_k = temps.kelvin(Htr10Zone::Coolant);
    let mut mats = htr10_material_set(
        layout,
        Htr10MaterialConfig {
            temperature_k: coolant_k,
            ..cfg
        },
    );
    debug_assert_eq!(mats.len(), mat::COUNT);
    let copy = |src: &Material, id: i32, what: &str| Material {
        id,
        name: format!("{} ({what})", src.name),
        components: src.components.clone(),
        temperature: src.temperature,
    };
    mats.push(copy(&mats[mat::GRAPHITE], 75, "moderator balls"));
    for (r, what) in ["top reflector", "side reflector", "bottom reflector"]
        .into_iter()
        .enumerate()
    {
        for (i, &b) in mat::REGIONAL_BASES.iter().enumerate() {
            let id = 1000 * (r as i32 + 1) + i as i32;
            mats.push(copy(&mats[b], id, what));
        }
    }
    debug_assert_eq!(mats.len(), mat::ZONED_COUNT);
    for (slot, m) in mats.iter_mut().enumerate() {
        m.temperature = temps.kelvin(zone_of_slot(slot));
    }
    mats
}

/// **One nuclide per (slot, temperature)**: for each of the `n_slots` base
/// slots, in slot order, one entry for every distinct temperature of the
/// materials that use it (ascending), and the materials' components
/// re-pointed at their own temperature's entry. A slot no material uses
/// keeps one entry, at the lowest material temperature.
///
/// Returns the entries as `(base slot, temperature K)`. With every material
/// at one temperature the entries are `(k, T)` for `k = 0..n_slots` and no
/// component moves: the single-temperature nuclide array, unchanged.
///
/// # Panics
/// If a component indexes a slot at or above `n_slots`, or `mats` is empty.
pub fn split_by_temperature(mats: &mut [Material], n_slots: usize) -> Vec<(usize, f64)> {
    let floor = mats
        .iter()
        .map(|m| m.temperature)
        .min_by(f64::total_cmp)
        .expect("at least one material");
    let mut temps: Vec<Vec<f64>> = vec![Vec::new(); n_slots];
    for m in mats.iter() {
        for c in &m.components {
            assert!(c.nuclide_idx < n_slots, "{}: slot {} out of range", m.name, c.nuclide_idx);
            let v = &mut temps[c.nuclide_idx];
            if !v.iter().any(|t: &f64| t.to_bits() == m.temperature.to_bits()) {
                v.push(m.temperature);
            }
        }
    }
    let mut entries: Vec<(usize, f64)> = Vec::new();
    let mut first: Vec<usize> = Vec::with_capacity(n_slots);
    for (k, v) in temps.iter_mut().enumerate() {
        if v.is_empty() {
            v.push(floor);
        }
        v.sort_by(f64::total_cmp);
        first.push(entries.len());
        entries.extend(v.iter().map(|&t| (k, t)));
    }
    for m in mats.iter_mut() {
        let t = m.temperature;
        for c in &mut m.components {
            let k = c.nuclide_idx;
            let j = temps[k]
                .iter()
                .position(|x| x.to_bits() == t.to_bits())
                .expect("every material temperature of a slot has an entry");
            *c = NuclideComponent {
                nuclide_idx: first[k] + j,
                atom_density: c.atom_density,
            };
        }
    }
    entries
}

/// **Give every material cell its material's temperature** (gh:#313: the
/// cells were built at a hardcoded 293.6 K, which transport never reads; the
/// material's temperature is what it reads). Fill cells are left alone.
pub fn set_cell_temperatures(geometry: &mut Geometry, mats: &[Material]) {
    for c in &mut geometry.cells {
        if let CellFill::Material(m) = c.fill {
            if let Some(mm) = mats.get(m) {
                c.temperature = mm.temperature;
            }
        }
    }
}

/// **Palette by temperature zone** for `render_material_slice` (gh:#1138's
/// drawing): every slot of a zoned design coloured by its zone, so a slice
/// shows which cells each slider moves. Helium is pale so the pebbles show.
#[must_use]
pub fn zone_palette() -> Vec<(outram_mc_libs::geometry::plot::Rgb, &'static str)> {
    use outram_mc_libs::geometry::plot::Rgb;
    let colour = |z: Htr10Zone| match z {
        Htr10Zone::FuelPebble => (Rgb::new(220, 40, 30), "FUEL PEBBLES"),
        Htr10Zone::ModeratorPebble => (Rgb::new(60, 60, 60), "MODERATOR PEBBLES"),
        Htr10Zone::Coolant => (Rgb::new(210, 235, 255), "HELIUM COOLANT"),
        Htr10Zone::TopReflector => (Rgb::new(70, 150, 230), "TOP REFLECTOR"),
        Htr10Zone::SideReflector => (Rgb::new(200, 160, 90), "SIDE REFLECTOR"),
        Htr10Zone::BottomReflector => (Rgb::new(60, 170, 90), "BOTTOM REFLECTOR"),
        Htr10Zone::ControlRods => (Rgb::new(250, 200, 0), "CONTROL RODS"),
        Htr10Zone::BoronatedBricks => (Rgb::new(150, 50, 170), "BORONATED BRICKS"),
    };
    (0..mat::ZONED_COUNT).map(|s| colour(zone_of_slot(s))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htr10_rmc::reflector_geometry::FIG_4_10_BOXES;

    /// Every slot of a zoned design has one zone, and the zones are the
    /// table in the module docs.
    #[test]
    fn every_slot_has_its_zone() {
        use Htr10Zone as Z;
        let mut count = [0usize; 8];
        for s in 0..mat::ZONED_COUNT {
            count[zone_of_slot(s).index()] += 1;
        }
        assert_eq!(zone_of_slot(mat::KERNEL), Z::FuelPebble);
        assert_eq!(zone_of_slot(mat::GRAPHITE), Z::FuelPebble);
        assert_eq!(zone_of_slot(mat::MODERATOR_BALL), Z::ModeratorPebble);
        assert_eq!(zone_of_slot(mat::HOMOG_DUMMY), Z::ModeratorPebble);
        assert_eq!(zone_of_slot(mat::HELIUM), Z::Coolant);
        assert_eq!(zone_of_slot(mat::BORONATED), Z::BoronatedBricks);
        assert_eq!(zone_of_slot(mat::ROD_STEEL), Z::ControlRods);
        let n = mat::REGIONAL_BASES.len();
        assert_eq!(count[Z::TopReflector.index()], n);
        assert_eq!(count[Z::BottomReflector.index()], n);
        assert_eq!(count[Z::FuelPebble.index()], 6);
        assert_eq!(count[Z::ControlRods.index()], 3);
        for (r, z) in [Z::TopReflector, Z::SideReflector, Z::BottomReflector]
            .into_iter()
            .enumerate()
        {
            let s = mat::regional_slot(mat::REFLECTOR, r).expect("copy");
            assert_eq!(zone_of_slot(s), z);
            assert_eq!(mat::composition_of(s), mat::REFLECTOR);
        }
        assert_eq!(mat::composition_of(mat::MODERATOR_BALL), mat::GRAPHITE);
        assert_eq!(mat::composition_of(mat::HELIUM), mat::HELIUM);
        // The drawing's palette: one entry per slot, one colour per zone.
        let pal = zone_palette();
        assert_eq!(pal.len(), mat::ZONED_COUNT);
        let names: std::collections::BTreeSet<&str> = pal.iter().map(|(_, n)| *n).collect();
        assert_eq!(names.len(), 8);
    }

    /// The Fig. 4.10 boxes by region: the top plug (zones 1-4) on top, the
    /// bottom structures (8-18) below, the side columns beside; zone 66's
    /// two boxes fall in two regions.
    #[test]
    fn the_reflector_boxes_fall_into_three_regions() {
        let region = |zone: usize| -> Vec<ReflectorRegion> {
            FIG_4_10_BOXES
                .iter()
                .filter(|b| b.zone == zone)
                .map(region_of_box)
                .collect()
        };
        for z in 1..=4 {
            assert_eq!(region(z), vec![ReflectorRegion::Top], "zone {z}");
        }
        for z in 8..=18 {
            assert_eq!(region(z), vec![ReflectorRegion::Bottom], "zone {z}");
        }
        for z in [22, 31, 49, 58] {
            assert_eq!(region(z), vec![ReflectorRegion::Side], "zone {z}");
        }
        assert_eq!(region(66), vec![ReflectorRegion::Top, ReflectorRegion::Side]);
        let n: [usize; 3] = FIG_4_10_BOXES.iter().fold([0; 3], |mut a, b| {
            a[region_of_box(b).index()] += 1;
            a
        });
        assert_eq!(n.iter().sum::<usize>(), FIG_4_10_BOXES.len());
        assert!(n.iter().all(|&k| k > 0), "{n:?}");
    }

    /// Range: 20 °C and 1800 °C accepted, a step outside refused; the
    /// TRISO warning starts above 1600 °C.
    #[test]
    fn the_temperature_range_and_the_triso_warning() {
        let c = |x: f64| ThermodynamicTemperature::new::<degree_celsius>(x);
        assert!(ZoneTemperatures::uniform(c(20.0)).check().is_ok());
        assert!(ZoneTemperatures::uniform(c(1800.0)).check().is_ok());
        assert!(ZoneTemperatures::uniform(c(19.0)).check().is_err());
        let t = ZoneTemperatures::record().with(Htr10Zone::Coolant, c(1801.0));
        assert!(t.check().unwrap_err().contains("Helium coolant"));
        let t = ZoneTemperatures::record().with(Htr10Zone::FuelPebble, c(1650.0));
        assert_eq!(t.above_triso_limit(), vec![Htr10Zone::FuelPebble]);
        assert!(ZoneTemperatures::record().above_triso_limit().is_empty());
        assert!(ZoneTemperatures::record().is_uniform() && !t.is_uniform());
        assert_eq!(ZoneTemperatures::from_kelvin(t.to_kelvin()), t);
        for z in Htr10Zone::ALL {
            assert_eq!(Htr10Zone::from_key(z.key()), Some(z));
        }
    }

    fn m(t: f64, idx: &[usize]) -> Material {
        Material {
            id: 1,
            name: "m".into(),
            components: idx
                .iter()
                .map(|&i| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: 1.0 + i as f64,
                })
                .collect(),
            temperature: t,
        }
    }

    /// One temperature: the identity. Two: a slot used at both gets two
    /// entries, in ascending temperature, and the components follow.
    #[test]
    fn the_split_is_the_identity_at_one_temperature() {
        let mut one = vec![m(300.0, &[0, 2]), m(300.0, &[1, 2])];
        let before = one.clone();
        let e = split_by_temperature(&mut one, 4);
        assert_eq!(e, vec![(0, 300.0), (1, 300.0), (2, 300.0), (3, 300.0)]);
        assert_eq!(format!("{one:?}"), format!("{before:?}"));

        let mut two = vec![m(900.0, &[0, 2]), m(300.0, &[1, 2])];
        let e = split_by_temperature(&mut two, 3);
        assert_eq!(e, vec![(0, 900.0), (1, 300.0), (2, 300.0), (2, 900.0)]);
        let idx = |m: &Material| m.components.iter().map(|c| c.nuclide_idx).collect::<Vec<_>>();
        assert_eq!(idx(&two[0]), vec![0, 3]);
        assert_eq!(idx(&two[1]), vec![1, 2]);
        assert_eq!(two[0].components[1].atom_density, 3.0);
    }
}
