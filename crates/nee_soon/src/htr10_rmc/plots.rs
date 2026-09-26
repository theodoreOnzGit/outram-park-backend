// SPDX-License-Identifier: GPL-3.0

//! **HTR-10 cross-section plots, drawn with the ported `openmc.Model.plot`**
//! ([`outram_mc_libs::geometry::plot::ModelPlot`]).
//!
//! NEW WORK (helpers), on top of a verified port: every picture is
//! `Model.plot`'s own output (0 differing pixels against OpenMC 0.16.1.dev25,
//! `outram-mc-libs/verification_and_validation/python_plotting_parity/`),
//! coloured by material, with a legend that lists only the materials the slice
//! actually contains.
//!
//! **Every cut is chosen from the BUILT bed, not from constants.** The planes
//! that "cut across the pebbles" are picked from
//! [`TwoBallBed`](super::bed::TwoBallBed)'s ball centres — the same
//! description the lattice was assembled from — and each [`PlotJob`] records
//! how many ball centres lie on its plane, so a reader can check the claim:
//!
//! - [`Htr10Plotter::rz_through_pebbles`]: the `x-z` plane `y = y0`, where `y0`
//!   is the `y` shared by the most present ball centres;
//! - [`Htr10Plotter::r_theta_at`]: the `x-y` plane at the ball-centre height
//!   nearest the requested `z`, when that `z` is inside the pebble column
//!   (bed, conus or discharge tube); outside it, the requested `z` as given;
//! - [`Htr10Plotter::pebble_cross_section`]: a plane through the centre of a
//!   fuel (or dummy) ball on that `x-z` plane;
//! - [`Htr10Plotter::triso_cross_section`]: a plane through the centre of one
//!   TRISO particle of that fuel ball, found by locating a kernel.
//!
//! [`Htr10Plotter::standard_set`] is the whole-core set: R-Z through the
//! pebbles, and `x-y` at the bed bottom, middle and top, the conus, the
//! discharge tube (the defuelling chute, holding dummy balls), the top
//! reflector through the withdrawn rods, and the hot-gas duct, so that the
//! control-rod, absorber-ball (KLAK), irradiation and coolant borings all show.

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::{
    AxisUnits, ColorBy, DomainColour, ModelPlot, ModelPlotError, Pixels, PlotBasis, PlotColour, Rgb,
};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::material::Material;
use outram_mc_libs::pebble_beds::htr10::Htr10Nuclides;

use super::bed::BallId;
use super::core_model::{mat, AssembledCore, HTR10_REFLECTOR_OUTER_CM};
use super::materials::{htr10_material_set, Htr10MaterialConfig, RodMetalNuclides};
use super::reflector_geometry::HOT_GAS_DUCT_AXIS_ZT_CM;

/// Lower end of a withdrawn rod, `z_T` \[cm\] (TECDOC-1382 § 4.1.2), as
/// `reflector_geometry` places it.
const ROD_LOWER_END_ZT_CM: f64 = 119.2;

/// Material palette, indexed by [`mat`]. Chosen so the TRISO layers read as a
/// warm-to-cool sequence from the kernel out and the graphites stay grey/brown.
pub fn palette() -> Vec<(Rgb, &'static str)> {
    let mut p = vec![(Rgb::new(0, 0, 0), ""); mat::COUNT];
    p[mat::KERNEL] = (Rgb::new(220, 20, 20), "UO2 KERNEL");
    p[mat::BUFFER] = (Rgb::new(255, 150, 0), "BUFFER PYC");
    p[mat::IPYC] = (Rgb::new(250, 225, 0), "IPYC");
    p[mat::SIC] = (Rgb::new(40, 160, 40), "SIC");
    p[mat::OPYC] = (Rgb::new(0, 190, 200), "OPYC");
    p[mat::GRAPHITE] = (Rgb::new(95, 95, 95), "MATRIX / SHELL GRAPHITE");
    p[mat::HELIUM] = (Rgb::new(205, 230, 255), "HELIUM");
    p[mat::REFLECTOR] = (Rgb::new(150, 115, 80), "REFLECTOR GRAPHITE");
    p[mat::BORONATED] = (Rgb::new(130, 40, 160), "BORONATED CARBON");
    p[mat::BORED_GRAPHITE] = (Rgb::new(190, 160, 120), "BORED REFLECTOR GRAPHITE");
    p[mat::HOMOG_DUMMY] = (Rgb::new(60, 70, 150), "HOMOG. DUMMY PEBBLES");
    // The Table 4-3 zones that keep their own composition (2026-09-25): one
    // colour each, so every zone boundary of Fig. 4.10 shows in a slice.
    let zone_colours: [(Rgb, &'static str); 24] = [
        (Rgb::new(120, 90, 60), "ZONE 0 CONUS SURROUND"),
        (Rgb::new(170, 60, 190), "ZONE 1 TOP BORONATED"),
        (Rgb::new(175, 140, 100), "ZONE 2 TOP REFLECTOR"),
        (Rgb::new(255, 240, 170), "ZONE 3 COLD HE CHAMBER"),
        (Rgb::new(185, 150, 105), "ZONE 4 TOP REFLECTOR"),
        (Rgb::new(140, 105, 70), "ZONE 8 BOTTOM"),
        (Rgb::new(160, 120, 80), "ZONE 9 BOTTOM"),
        (Rgb::new(200, 90, 90), "ZONE 10 BOTTOM (B)"),
        (Rgb::new(190, 110, 90), "ZONE 11 BOTTOM (B)"),
        (Rgb::new(210, 100, 120), "ZONE 12 BOTTOM (B)"),
        (Rgb::new(150, 130, 90), "ZONE 13 BOTTOM"),
        (Rgb::new(230, 200, 120), "ZONE 14 HOT GAS CHAMBER"),
        (Rgb::new(250, 225, 150), "ZONE 15 HOT GAS CHAMBER"),
        (Rgb::new(130, 115, 85), "ZONE 16 BOTTOM"),
        (Rgb::new(95, 75, 55), "ZONE 18 CARBON BRICK"),
        (Rgb::new(150, 50, 170), "ZONE 19 BORONATED"),
        (Rgb::new(165, 130, 95), "ZONE 20"),
        (Rgb::new(220, 190, 140), "ZONE 21"),
        (Rgb::new(115, 125, 70), "ZONES 24/51/68 (B)"),
        (Rgb::new(215, 175, 120), "ZONE 29 X1.29978"),
        (Rgb::new(100, 140, 80), "ZONE 42 X1.29978"),
        (Rgb::new(225, 185, 130), "ZONE 48"),
        (Rgb::new(200, 170, 110), "ZONE 57"),
        (Rgb::new(90, 130, 90), "ZONE 60 X1.16051"),
    ];
    for (i, c) in zone_colours.into_iter().enumerate() {
        p[mat::ZONE_TABLE_FIRST + i] = c;
    }
    p[mat::ROD_B4C] = (Rgb::new(20, 20, 20), "ROD B4C");
    p[mat::ROD_STEEL] = (Rgb::new(170, 180, 195), "ROD STEEL");
    p[mat::ROD_IRON] = (Rgb::new(90, 100, 115), "ROD IRON");
    p
}

/// One plot to emit: a configured [`ModelPlot`], a file stem, and a note of
/// how its plane was chosen (recorded next to the images).
#[derive(Debug, Clone)]
pub struct PlotJob {
    /// File stem (`<name>.py`, `<name>.png`).
    pub name: String,
    /// The plot.
    pub plot: ModelPlot,
    /// How the plane was chosen, with the number of ball centres on it.
    pub note: String,
}

/// **Draws an assembled HTR-10 core.** Owns the core and the material table
/// (built with [`htr10_material_set`] and the benchmark configuration, the
/// same call the eigenvalue example makes; a plot reads only material ids and
/// names from it).
pub struct Htr10Plotter {
    /// The assembled core ([`super::core_model::assemble_explicit_triso`]).
    pub core: AssembledCore,
    /// Material table, indexed as the geometry's cells index it.
    pub materials: Vec<Material>,
}

impl Htr10Plotter {
    /// Wrap an assembled core. Panics if the core has no ball description
    /// (the one-ball [`super::core_model::assemble`] path).
    #[must_use]
    pub fn new(core: AssembledCore) -> Self {
        assert!(
            core.bed.is_some(),
            "Htr10Plotter needs assemble_explicit_triso's ball description"
        );
        let nuclides = Htr10Nuclides {
            u235: 0,
            u238: 1,
            o16: 2,
            c_free: 3,
            c_graphite: 4,
            si28: 5,
            b10: 6,
            c_sic: 7,
            si29: 8,
            si30: 9,
            b11: 10,
        };
        let materials = htr10_material_set(
            nuclides,
            RodMetalNuclides::contiguous(11),
            Htr10MaterialConfig::benchmark_default(293.6),
        );
        Self { core, materials }
    }

    /// Present balls with their centres and identity: `(centre, is_fuel)`.
    fn balls(&self) -> Vec<([f64; 3], bool)> {
        let bed = self.core.bed.as_ref().expect("checked in new");
        bed.all_balls()
            .into_iter()
            .filter(|&id| bed.is_present(id) && self.inside_column(bed.centre(id)))
            .map(|id: BallId| (bed.centre(id), bed.is_fuel(id)))
            .collect()
    }

    /// Whether a ball centre is inside the pebble column: the bed cylinder
    /// between the bed floor and top, or below the floor inside the conus
    /// frustum and the discharge tube. The lattice spans the whole cylinder,
    /// so balls outside the container are "present" in the ball list but
    /// never reach the geometry; they must not be counted.
    fn inside_column(&self, c: [f64; 3]) -> bool {
        let bed = self.core.bed.as_ref().expect("checked in new");
        let (r, z) = (c[0].hypot(c[1]), c[2]);
        if z > bed.bed_top {
            return false;
        }
        if z >= bed.bed_bottom {
            return r <= bed.bed_radius;
        }
        let Some(t) = bed.tube else {
            return false;
        };
        if z < bed.conus_floor - t.depth {
            return false;
        }
        let allowed = if z >= bed.conus_floor {
            t.radius
                + (bed.bed_radius - t.radius) * (z - bed.conus_floor)
                    / (bed.bed_bottom - bed.conus_floor)
        } else {
            t.radius
        };
        r <= allowed
    }

    /// `y0` of the `x-z` plane holding the most present ball centres (ties:
    /// smallest `|y0|`), and how many centres lie on it.
    #[must_use]
    pub fn pebble_plane_y(&self) -> (f64, usize) {
        let mut ys: Vec<(i64, f64)> = self
            .balls()
            .iter()
            .map(|(c, _)| ((c[1] * 1.0e6).round() as i64, c[1]))
            .collect();
        ys.sort_by_key(|p| p.0);
        let mut best = (0.0_f64, 0usize);
        let mut i = 0;
        while i < ys.len() {
            let mut j = i;
            while j < ys.len() && ys[j].0 == ys[i].0 {
                j += 1;
            }
            let n = j - i;
            if n > best.1 || (n == best.1 && ys[i].1.abs() < best.0.abs()) {
                best = (ys[i].1, n);
            }
            i = j;
        }
        best
    }

    /// The ball-centre height nearest `z` and how many centres sit at it, or
    /// `None` when no ball centre lies within one ball diameter of `z`.
    #[must_use]
    pub fn nearest_ball_layer(&self, z: f64) -> Option<(f64, usize)> {
        let balls = self.balls();
        let zc = balls
            .iter()
            .map(|(c, _)| c[2])
            .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()))?;
        if (zc - z).abs() > 6.0 {
            return None;
        }
        let n = balls.iter().filter(|(c, _)| (c[2] - zc).abs() < 1.0e-9).count();
        Some((zc, n))
    }

    /// A present ball of the requested kind on the pebble plane, nearest the
    /// point `(0, y0, z)`.
    #[must_use]
    pub fn ball_on_plane(&self, fuel: bool, z: f64) -> Option<[f64; 3]> {
        let (y0, _) = self.pebble_plane_y();
        self.balls()
            .into_iter()
            .filter(|(c, f)| *f == fuel && (c[1] - y0).abs() < 1.0e-9)
            .map(|(c, _)| c)
            .min_by(|a, b| {
                let d = |c: &[f64; 3]| c[0].hypot(c[2] - z);
                d(a).total_cmp(&d(b))
            })
    }

    /// Centre of one TRISO particle of the fuel ball centred at `peb`: scan a
    /// small square about the centre for a kernel and read the particle's
    /// frame origin off the located path (the leaf level is the particle
    /// universe, entered through the TRISO lattice).
    #[must_use]
    pub fn triso_centre(&self, peb: [f64; 3]) -> Option<Position> {
        let g = &self.core.geometry;
        let u = Direction::new(0.0, 0.0, 1.0);
        for j in 0..60 {
            for i in 0..60 {
                let p = Position::new(peb[0] + 0.005 * f64::from(i), peb[1] + 0.005 * f64::from(j), peb[2]);
                if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                    if path.material == Some(mat::KERNEL) {
                        return path.levels.last().map(|c| c.offset);
                    }
                }
            }
        }
        None
    }

    fn base(&self, basis: PlotBasis, origin: [f64; 3], width: [f64; 2], cm_per_px: f64) -> ModelPlot {
        let mut p = ModelPlot::new();
        p.basis = basis;
        p.origin = Some(Position::new(origin[0], origin[1], origin[2]));
        p.width = Some(width);
        p.pixels = Pixels::Exact(width.map(|w| (w / cm_per_px).round().max(1.0) as usize));
        p.color_by = ColorBy::Material;
        p.legend = true;
        p.axis_units = AxisUnits::Cm;
        p.legend_kwargs = vec![("fontsize".into(), "'small'".into())];
        p.savefig_kwargs = vec![
            ("dpi".into(), "100".into()),
            ("bbox_inches".into(), "'tight'".into()),
        ];
        p
    }

    fn job(name: &str, plot: ModelPlot, title: String, note: String) -> PlotJob {
        let mut plot = plot;
        plot.title = Some(title);
        plot.default_output = format!("{name}.png");
        PlotJob {
            name: name.into(),
            plot,
            note,
        }
    }

    /// Whole-model `x-z` (R-Z) slice on the plane through the most ball centres.
    #[must_use]
    pub fn rz_through_pebbles(&self, cm_per_px: f64) -> PlotJob {
        let (y0, n) = self.pebble_plane_y();
        let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
        let (lo, hi) = (self.core.refl_bottom - 5.0, self.core.refl_top + 5.0);
        let p = self.base(PlotBasis::Xz, [0.0, y0, 0.5 * (lo + hi)], [2.0 * half_w, hi - lo], cm_per_px);
        Self::job(
            "htr10_rz_through_pebbles",
            p,
            format!("HTR-10 R-Z (x-z) slice at y = {y0:.4} cm, through {n} ball centres"),
            format!("x-z plane y = {y0:.6} cm: the y shared by the most present ball centres ({n})"),
        )
    }

    /// Zoomed `x-z` slice of the lower bed, conus and discharge tube on the
    /// pebble plane.
    #[must_use]
    pub fn rz_lower_column(&self, cm_per_px: f64) -> PlotJob {
        let (y0, n) = self.pebble_plane_y();
        let bed = self.core.bed.as_ref().expect("checked");
        let top = bed.bed_bottom + 40.0;
        let bottom = bed.conus_floor - 60.0;
        let p = self.base(PlotBasis::Xz, [0.0, y0, 0.5 * (top + bottom)], [230.0, top - bottom], cm_per_px);
        Self::job(
            "htr10_rz_conus_and_chute",
            p,
            format!("HTR-10 x-z at y = {y0:.4} cm: bed floor, conus, defuelling chute"),
            format!("x-z plane y = {y0:.6} cm ({n} ball centres on the plane), z {bottom:.1} .. {top:.1} cm"),
        )
    }

    /// Whole-model `x-y` (r-theta) slice at height `z`, snapped to the nearest
    /// ball-centre layer when `z` is in the pebble column.
    #[must_use]
    pub fn r_theta_at(&self, name: &str, what: &str, z: f64, cm_per_px: f64) -> PlotJob {
        let in_column = z <= self.core.bed_half_height && z >= self.core.refl_bottom;
        let (zc, note) = match self.nearest_ball_layer(z).filter(|_| in_column) {
            Some((zc, n)) => (
                zc,
                format!("x-y at z = {zc:.4} cm: the ball-centre layer nearest {z:.2} cm ({n} ball centres)"),
            ),
            None => (z, format!("x-y at z = {z:.4} cm (no pebble layer within a ball diameter)")),
        };
        let w = 2.0 * (HTR10_REFLECTOR_OUTER_CM + 5.0);
        let p = self.base(PlotBasis::Xy, [0.0, 0.0, zc], [w, w], cm_per_px);
        let zt = self.core.refl_top - zc;
        Self::job(
            name,
            p,
            format!("HTR-10 x-y slice, {what}: z = {zc:.3} cm (z_T = {zt:.1} cm)"),
            note,
        )
    }

    /// `x-y` slice through the centre of a fuel (or dummy) ball on the pebble
    /// plane nearest bed mid-height.
    #[must_use]
    pub fn pebble_cross_section(&self, fuel: bool, cm_per_px: f64) -> Option<PlotJob> {
        let c = self.ball_on_plane(fuel, 0.0)?;
        let kind = if fuel { "fuel" } else { "dummy" };
        let p = self.base(PlotBasis::Xy, c, [7.0, 7.0], cm_per_px);
        Some(Self::job(
            &format!("htr10_{kind}_pebble"),
            p,
            format!("HTR-10 {kind} pebble, x-y through its centre ({:.3}, {:.3}, {:.3}) cm", c[0], c[1], c[2]),
            format!("{kind} ball centred on the pebble plane nearest (0, y0, 0): centre {c:?}"),
        ))
    }

    /// `x-y` slice through the centre of one TRISO particle, `width` cm wide.
    #[must_use]
    pub fn triso_cross_section(&self, name: &str, width: f64, cm_per_px: f64) -> Option<PlotJob> {
        let peb = self.ball_on_plane(true, 0.0)?;
        let t = self.triso_centre(peb)?;
        let p = self.base(PlotBasis::Xy, [t.x, t.y, t.z], [width, width], cm_per_px);
        Some(Self::job(
            name,
            p,
            format!("HTR-10 TRISO, x-y through a particle centre ({:.4}, {:.4}, {:.4}) cm", t.x, t.y, t.z),
            format!("TRISO particle centre {:?} (found by locating a kernel), in the fuel ball at {peb:?}", [t.x, t.y, t.z]),
        ))
    }

    /// The whole set: TRISO, pebbles, R-Z, and `x-y` at the heights that show
    /// every channel family and the defuelling chute.
    #[must_use]
    pub fn standard_set(&self) -> Vec<PlotJob> {
        let c = &self.core;
        let bed = c.bed.as_ref().expect("checked");
        let zt = |zt: f64| c.refl_top - zt;
        let mut v = Vec::new();
        v.extend(self.triso_cross_section("htr10_triso", 0.12, 0.0002));
        v.extend(self.triso_cross_section("htr10_triso_array", 0.8, 0.001));
        v.extend(self.pebble_cross_section(true, 0.005));
        v.extend(self.pebble_cross_section(false, 0.005));
        v.push(self.rz_through_pebbles(0.3));
        v.push(self.rz_lower_column(0.1));
        v.push(self.r_theta_at("htr10_xy_bed_top", "top of the pebble bed", bed.bed_top - 8.0, 0.25));
        v.push(self.r_theta_at("htr10_xy_bed_mid", "bed mid-height", 0.0, 0.25));
        v.push(self.r_theta_at("htr10_xy_bed_bottom", "bottom of the pebble bed", bed.bed_bottom + 8.0, 0.25));
        v.push(self.r_theta_at("htr10_xy_conus", "conus (dummy balls)", 0.5 * (bed.bed_bottom + bed.conus_floor), 0.25));
        v.push(self.r_theta_at("htr10_xy_defuel_chute", "defuelling chute (dummy balls)", bed.conus_floor - 40.0, 0.25));
        v.push(self.r_theta_at("htr10_xy_cavity", "empty core cavity above the bed", 0.5 * (bed.bed_top + c.cavity_top), 0.25));
        v.push(self.r_theta_at("htr10_xy_top_reflector_rods", "top reflector, through the withdrawn rods", zt(0.5 * ROD_LOWER_END_ZT_CM + 0.5 * 20.0), 0.25));
        v.push(self.r_theta_at("htr10_xy_hot_gas_duct", "hot-gas duct", zt(HOT_GAS_DUCT_AXIS_ZT_CM), 0.25));
        v
    }

    /// Emit a job's script, with the legend restricted to the materials present
    /// in the slice (palette colours; unnamed materials get their table name).
    ///
    /// # Errors
    /// As [`ModelPlot::emit`].
    pub fn script(&self, job: &PlotJob) -> Result<String, ModelPlotError> {
        let g = &self.core.geometry;
        let (cells, mats, _) = job.plot.id_map(g, &self.materials)?;
        let mut present: Vec<i32> = mats.iter().copied().filter(|&m| m >= 0).collect();
        present.sort_unstable();
        present.dedup();
        let pal = palette();
        let colours = self
            .materials
            .iter()
            .enumerate()
            .filter(|(_, m)| present.binary_search(&m.id).is_ok())
            .map(|(i, m)| {
                let (rgb, label) = pal.get(i).copied().unwrap_or((Rgb::new(0, 0, 0), ""));
                DomainColour {
                    id: m.id,
                    name: if label.is_empty() { m.name.clone() } else { label.to_string() },
                    colour: PlotColour::Rgb([rgb.r, rgb.g, rgb.b]),
                }
            })
            .collect();
        let mut plot = job.plot.clone();
        plot.colors = Some(colours);
        plot.emit_with_id_map(g, &self.materials, &cells, &mats)
    }
}
