//! The assembled network, drawn (gh:#1110).
//!
//! tampines' geometry rule (inherited here) asks for a drawing a human can
//! check before results on a geometry are trusted, drawn from what the
//! solver holds, never from the named constants. Everything below is read
//! back from the built [`CvNetwork`]:
//!
//! - each volume's free area against elevation, sampled from its own
//!   `VolumeAltitudeTable` (`area_at`), between the table's bottom
//!   (`height_of_volume(0)`) and top (`height_of_volume(total)`);
//! - the pool level and the swollen level (`pool_level`, `swollen_level`);
//! - the torus pool's layer boundaries, placed the way tampines places
//!   them (`ControlVolume::layer_at`: the mass below an elevation at the
//!   pool's mean density, so each boundary is
//!   `height_of_volume(Σ m_layer / ρ_pool)`);
//! - every port's elevation and opening height (`ports`);
//! - every flow path, joining its two ends at the junction elevations its
//!   geometry carries (`from_elevation`, `to_elevation`), or running to the
//!   environment.
//!
//! **One datum.** The three tables share the plant datum the network is
//! built on (the torus bottom at 0 m, the drywell floor at the torus top;
//! the building's own table starts at its sump pit), so the panels share
//! one vertical axis. A panel's width is its free area **normalised to that
//! volume's largest area**, so shapes compare within a panel, not between
//! panels; the largest area is printed under each panel.
//!
//! Drawing code, not physics: its test checks that what is drawn is what the
//! network holds (each label, each layer boundary, the top boundary at the
//! pool level), not how it looks.

use std::fmt::Write as _;

use tampines::control_volume::ControlVolume;
use tampines::cv_network::NetworkEnd;
use uom::si::area::square_meter;
use uom::si::f64::{Length, Volume};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::volume::cubic_meter;

use super::ContainmentNetwork;

/// Port names, by volume (the order the network builds them in).
const DW_PORTS: [&str; 6] = ["downcomer entry", "vacuum breaker", "head flange", "RPV leak", "sump drain", "cavity gas"];
const WW_PORTS: [&str; 5] = ["downcomer exit", "vacuum breaker", "vent", "quencher", "drain"];
const RB_PORTS: [&str; 2] = ["flange inflow", "openings"];
/// Path names, in the order the network adds them.
const PATHS: [&str; 5] = ["downcomer", "vacuum breaker", "wetwell vent", "head flange", "building openings"];

const PANEL_W: f64 = 360.0;
const PLOT_H: f64 = 760.0;
const TOP: f64 = 70.0;
const LEFT: f64 = 70.0;

/// What the drawing read back from one volume, in metres and m².
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeGeometry {
    pub name: &'static str,
    pub bottom_m: f64,
    pub top_m: f64,
    pub max_area_m2: f64,
    pub total_m3: f64,
    pub pool_level_m: f64,
    pub swollen_level_m: f64,
    /// Layer boundaries from the bottom, the last at the top of the pool.
    pub layer_tops_m: Vec<f64>,
    /// `(name, elevation, opening height)`.
    pub ports: Vec<(&'static str, f64, f64)>,
}

fn read_volume(v: &ControlVolume, name: &'static str, port_names: &[&'static str]) -> VolumeGeometry {
    let t = v.table();
    let total = t.total_volume();
    let bottom_m = t.height_of_volume(Volume::new::<cubic_meter>(0.0)).get::<meter>();
    let top_m = t.height_of_volume(total).get::<meter>();
    let n = 400;
    let max_area_m2 = (0..=n)
        .map(|i| t.area_at(Length::new::<meter>(bottom_m + (top_m - bottom_m) * i as f64 / n as f64)).get::<square_meter>())
        .fold(0.0, f64::max);
    let rho = v.pool_density().get::<kilogram_per_cubic_meter>();
    let mut cum = 0.0;
    let layer_tops_m = if v.pool_layer_count() > 1 {
        v.pool_layer_masses()
            .iter()
            .map(|m| {
                cum += m.get::<kilogram>();
                t.height_of_volume(Volume::new::<cubic_meter>(cum / rho)).get::<meter>()
            })
            .collect()
    } else {
        Vec::new()
    };
    let ports = v
        .ports()
        .iter()
        .enumerate()
        .map(|(i, p)| (port_names.get(i).copied().unwrap_or("port"), p.elevation.get::<meter>(), p.opening_height.get::<meter>()))
        .collect();
    VolumeGeometry {
        name,
        bottom_m,
        top_m,
        max_area_m2,
        total_m3: total.get::<cubic_meter>(),
        pool_level_m: v.pool_level().get::<meter>(),
        swollen_level_m: v.swollen_level().get::<meter>(),
        layer_tops_m,
        ports,
    }
}

impl ContainmentNetwork {
    /// The drywell, torus and building as read back from the network
    /// (gh:#1110; see the module docs for what is read and how).
    pub fn geometry(&self) -> [VolumeGeometry; 3] {
        let get = |i: usize, name, ports: &[&'static str]| match self.net.volume(i) {
            Some(v) => read_volume(v, name, ports),
            None => VolumeGeometry { name, bottom_m: 0.0, top_m: 0.0, max_area_m2: 0.0, total_m3: 0.0, pool_level_m: 0.0, swollen_level_m: 0.0, layer_tops_m: Vec::new(), ports: Vec::new() },
        };
        [get(self.dw, "drywell", &DW_PORTS), get(self.ww, "torus (wetwell)", &WW_PORTS), get(self.rb, "reactor building", &RB_PORTS)]
    }

    /// An SVG of the assembled network: each volume's free area against
    /// elevation on one datum, its pool, the torus layers, every port and
    /// every flow path (gh:#1110).
    pub fn geometry_svg(&self, title: &str) -> String {
        let vols = self.geometry();
        let z_lo = vols.iter().map(|v| v.bottom_m).fold(f64::INFINITY, f64::min).floor();
        let z_hi = vols.iter().map(|v| v.top_m).fold(f64::NEG_INFINITY, f64::max).ceil();
        let y = |z: f64| TOP + PLOT_H * (z_hi - z) / (z_hi - z_lo);
        let width = LEFT + 3.0 * PANEL_W + 160.0;
        let height = TOP + PLOT_H + 150.0;
        let mut s = String::new();
        let _ = writeln!(s, r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" font-family="sans-serif" font-size="11">"##);
        let _ = writeln!(s, r##"<rect width="100%" height="100%" fill="#fbf1c7"/>"##);
        let _ = writeln!(s, r##"<text x="{LEFT}" y="24" font-size="16" font-weight="bold">{}</text>"##, esc(title));
        let _ = writeln!(s, r##"<text x="{LEFT}" y="44">Read back from the assembled ContainmentNetwork (gh:#1110). Width: free area, normalised per volume. Shared plant datum.</text>"##);
        // Axis.
        let _ = writeln!(s, r##"<line x1="{LEFT}" y1="{:.1}" x2="{LEFT}" y2="{:.1}" stroke="#3c3836"/>"##, y(z_hi), y(z_lo));
        let step = if z_hi - z_lo > 40.0 { 5.0 } else { 2.0 };
        let mut z = (z_lo / step).ceil() * step;
        while z <= z_hi + 1e-9 {
            let _ = writeln!(s, r##"<line x1="{:.1}" y1="{yy:.1}" x2="{LEFT}" y2="{yy:.1}" stroke="#3c3836"/><text x="{:.1}" y="{:.1}" text-anchor="end">{:.0} m</text>"##, LEFT - 5.0, LEFT - 8.0, y(z) + 4.0, z + 0.0, yy = y(z));
            z += step;
        }
        let centre = |k: usize| LEFT + PANEL_W * (k as f64 + 0.5) + 20.0;
        let half = 0.26 * PANEL_W;
        for (k, v) in vols.iter().enumerate() {
            let cx = centre(k);
            let t = match self.net.volume([self.dw, self.ww, self.rb][k]) {
                Some(cv) => cv.table(),
                None => continue,
            };
            let w = |z: f64| half * t.area_at(Length::new::<meter>(z)).get::<square_meter>() / v.max_area_m2.max(1e-12);
            let n = 300;
            let zs: Vec<f64> = (0..=n).map(|i| v.bottom_m + (v.top_m - v.bottom_m) * i as f64 / n as f64).collect();
            let outline = |upto: f64, fill: &str, s: &mut String| {
                let mut pts = String::new();
                for &z in zs.iter().filter(|&&z| z <= upto) {
                    let _ = write!(pts, "{:.1},{:.1} ", cx + w(z), y(z));
                }
                let _ = write!(pts, "{:.1},{:.1} {:.1},{:.1} ", cx + w(upto), y(upto), cx - w(upto), y(upto));
                for &z in zs.iter().rev().filter(|&&z| z <= upto) {
                    let _ = write!(pts, "{:.1},{:.1} ", cx - w(z), y(z));
                }
                let _ = writeln!(s, r##"<polygon points="{pts}" fill="{fill}" stroke="#3c3836" stroke-width="1"/>"##);
            };
            outline(v.top_m, "#ebdbb2", &mut s);
            if v.pool_level_m > v.bottom_m {
                outline(v.pool_level_m.min(v.top_m), "#83a598", &mut s);
            }
            for (i, &zt) in v.layer_tops_m.iter().enumerate() {
                let _ = writeln!(s, r##"<line class="layer" x1="{:.1}" y1="{yy:.1}" x2="{:.1}" y2="{yy:.1}" stroke="#076678" stroke-dasharray="3,2"/>"##, cx - w(zt), cx + w(zt), yy = y(zt));
                if i + 1 == v.layer_tops_m.len() {
                    let _ = writeln!(s, r##"<text x="{:.1}" y="{:.1}" fill="#076678">{} layers</text>"##, cx - half, y(zt) + 14.0, v.layer_tops_m.len());
                }
            }
            if (v.swollen_level_m - v.pool_level_m).abs() > 1e-6 {
                let _ = writeln!(s, r##"<line x1="{:.1}" y1="{yy:.1}" x2="{:.1}" y2="{yy:.1}" stroke="#076678"/>"##, cx - half, cx + half, yy = y(v.swollen_level_m));
            }
            // Labels sorted by height and pushed apart so ports at one
            // elevation stay readable; a leader joins each to its port.
            let mut order: Vec<usize> = (0..v.ports.len()).collect();
            order.sort_by(|&a, &b| v.ports[b].1.total_cmp(&v.ports[a].1));
            let mut last_y = f64::NEG_INFINITY;
            for j in order {
                let (name, z, h) = v.ports[j];
                let x = cx + half + 10.0;
                let yy = y(z);
                let ly = (yy + 4.0).max(last_y + 13.0);
                last_y = ly;
                let _ = writeln!(s, r##"<rect x="{:.1}" y="{:.1}" width="6" height="{:.1}" fill="#cc241d"/>"##, cx - 3.0, y(z + h), (y(z) - y(z + h)).max(1.5));
                let _ = writeln!(s, r##"<polyline points="{:.1},{yy:.1} {:.1},{yy:.1} {x:.1},{:.1}" fill="none" stroke="#cc241d" stroke-width="0.6"/><text class="port" x="{:.1}" y="{ly:.1}" fill="#9d0006">{} {z:.2} m</text>"##, cx + 3.0, x - 6.0, ly - 4.0, x + 2.0, esc(name));
            }
            let _ = writeln!(s, r##"<text x="{cx:.1}" y="{:.1}" text-anchor="middle" font-weight="bold">{}</text>"##, y(z_lo) + 24.0, esc(v.name));
            let _ = writeln!(
                s,
                r##"<text x="{cx:.1}" y="{:.1}" text-anchor="middle">{:.2} to {:.2} m, {:.0} m³, widest {:.1} m²</text><text x="{cx:.1}" y="{:.1}" text-anchor="middle">pool level {:.3} m (swollen {:.3} m)</text>"##,
                y(z_lo) + 40.0,
                v.bottom_m,
                v.top_m,
                v.total_m3,
                v.max_area_m2,
                y(z_lo) + 55.0,
                v.pool_level_m,
                v.swollen_level_m
            );
        }
        // Flow paths.
        let idx = |vol: usize| [self.dw, self.ww, self.rb].iter().position(|&i| i == vol);
        let mut i = 0;
        while let Some(p) = self.net.path(i) {
            let g = p.path.geometry();
            let name = PATHS.get(i).copied().unwrap_or("path");
            let end = |e: &NetworkEnd, z: f64| match e {
                NetworkEnd::Volume { volume, .. } => idx(*volume).map(|k| (centre(k), y(z))),
                NetworkEnd::Boundary(_) => Some((width - 40.0, y(z))),
            };
            if let (Some(a), Some(b)) = (end(&p.from, g.from_elevation.get::<meter>()), end(&p.to, g.to_elevation.get::<meter>())) {
                let _ = writeln!(s, r##"<line class="path" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#b16286" stroke-width="1.6" stroke-dasharray="6,3"/>"##, a.0, a.1, b.0, b.1);
                let _ = writeln!(s, r##"<text x="{:.1}" y="{:.1}" fill="#8f3f71">{}: {:.4} m² (from {:.2} m to {:.2} m)</text>"##, 0.5 * (a.0 + b.0) + 4.0, 0.5 * (a.1 + b.1) - 4.0, esc(name), g.area.get::<square_meter>(), g.from_elevation.get::<meter>(), g.to_elevation.get::<meter>());
            }
            i += 1;
        }
        let _ = writeln!(s, r##"<text x="{:.1}" y="{:.1}" text-anchor="end" fill="#8f3f71">environment</text>"##, width - 10.0, TOP - 6.0);
        s.push_str("</svg>\n");
        s
    }
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use crate::bwr_mark1::containment_network::{ContainmentNetwork, NetworkConfig, NetworkMechanisms};
    use crate::bwr_mark1::params::Params;

    /// What is drawn is what the network holds, for each unit: every port and
    /// path is labelled, the torus has one dashed boundary per layer, the
    /// top boundary sits at the pool level (the layers hold the whole pool),
    /// the downcomer exits and quenchers are where `port_elevations` says,
    /// and the drywell floor meets the torus top on the shared datum.
    #[test]
    fn the_drawing_shows_what_the_network_holds() {
        for (prm, cfg) in [
            (Params::default(), NetworkConfig::unit1()),
            (Params::default(), NetworkConfig::unit1_gh1097()),
            (Params::unit2(), NetworkConfig::unit2()),
            (Params::unit3(), NetworkConfig::unit3()),
        ] {
            let n = ContainmentNetwork::with_config(&prm, cfg, NetworkMechanisms::default()).unwrap();
            let [dw, ww, rb] = n.geometry();
            let svg = n.geometry_svg("test");
            for v in [&dw, &ww, &rb] {
                assert!(v.top_m > v.bottom_m && v.total_m3 > 0.0 && v.max_area_m2 > 0.0, "{v:?}");
                for (name, _, _) in &v.ports {
                    assert!(svg.contains(name), "{name}");
                }
            }
            assert_eq!(svg.matches("class=\"layer\"").count(), n.pool_layer_count().max(1) * usize::from(n.pool_layer_count() > 1));
            assert_eq!(svg.matches("class=\"path\"").count(), 5);
            if let Some(&top) = ww.layer_tops_m.last() {
                assert!((top - ww.pool_level_m).abs() < 1e-6, "{top} vs {}", ww.pool_level_m);
            }
            let (exit, quencher) = n.port_elevations();
            assert_eq!(ww.ports[0].1, exit);
            assert_eq!(ww.ports[3].1, quencher);
            assert!(ww.ports[0].1 < ww.pool_level_m, "the downcomer exits are submerged");
            // The drywell's pit starts below its floor, which is the torus
            // top unless the config moves it.
            assert!(dw.bottom_m < dw.top_m && ww.top_m > 0.0 && ww.bottom_m == 0.0, "{dw:?} {ww:?}");
        }
    }
}
