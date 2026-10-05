//! **Step animations** (gh:#548): the lesson anatomy asks for an animation of
//! exactly each step's idea. Each is a mode of this demo, opened with
//! `?rung=<rung>&step=<name>`, and each lesson step links its own.
//!
//! | step | rung | what it shows | numbers from |
//! |---|---|---|---|
//! | `tracers` | 1 | tracer particles drifting with the wind and spreading across it | buangkok's `sigma_y(x)`, `sigma_z(x)` and release-height wind |
//! | `slice` | 1 | the crosswind (y-z) slice widening with `x` while the flux stays 1 | buangkok's `master_equation_single_plume`, integrated as its flux test does |
//! | `mirror` | 1 | the plume and its image source, the part below ground folded back | the same, with the unreflected Gaussian's below-ground part drawn |
//! | `day-night` | 2 | the same plume at 14:00 and 02:00, same wind | changi's `stability_class`, buangkok's plume |
//! | `rise` | 3 | the plume leaving the stack, rising and bending over, `H_e` marked | pyDOSEIA's rise formulas ([`crate::engine::plume_rise_m`], unchecked: #542) |
//! | `single-puff` | 4 | one puff drifting and growing | changi's puff kernel (the puffs rung, emitting once) |
//!
//! **The demo's own pieces, illustration only:** the tracer particles (in the
//! app). Each one random-walks across the wind with steps whose variance is
//! the increase of buangkok's `sigma^2` over the step, so a crowd of them at
//! distance `x` has exactly the plume's spread. The `z` walk reflects at the
//! ground, which is the image source of the `mirror` step. They carry no
//! concentration; the verified numbers are the lessons'.
//!
//! Research, education and V&V only; not for emergency planning or response.

use crate::engine::{plume_chi_over_q, plume_class, plume_field, plume_rise_m, PlumeParams};
use crate::rungs::Rung;
use buangkok::pydoseia::dispersion::{
    height_correction_factor, master_equation_single_plume, sigma_y, sigma_z, Receptor,
};
use uom::si::f64::{Length, Velocity};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Wind measured at this height, m (pyDOSEIA's reference height).
const MEASUREMENT_HEIGHT_M: f64 = 10.0;
/// How far downwind the side views reach, m.
pub const REACH_M: f64 = 3000.0;
/// The rise step's reach, m: a plume reaches its effective height within a
/// few hundred metres, which a 3 km view squeezes into its first pixels.
pub const RISE_REACH_M: f64 = 600.0;

/// A step animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Tracers,
    Slice,
    Mirror,
    DayNight,
    Rise,
    SinglePuff,
}

/// Every step, in lesson order: (step, URL name, rung, title).
pub const STEPS: [(Step, &str, Rung, &str); 6] = [
    (Step::Tracers, "tracers", Rung::Plume, "Tracers: carried and spread"),
    (Step::Slice, "slice", Rung::Plume, "The crosswind slice keeps its flux"),
    (Step::Mirror, "mirror", Rung::Plume, "The ground: an image source"),
    (Step::DayNight, "day-night", Rung::Sigmas, "14:00 and 02:00, same wind"),
    (Step::Rise, "rise", Rung::RiseWake, "Rising and bending over"),
    (Step::SinglePuff, "single-puff", Rung::Puffs, "One puff drifting and growing"),
];

impl Step {
    pub fn name(self) -> &'static str {
        STEPS[self as usize].1
    }
    pub fn rung(self) -> Rung {
        STEPS[self as usize].2
    }
    pub fn title(self) -> &'static str {
        STEPS[self as usize].3
    }
    pub fn parse(name: &str) -> Option<Step> {
        STEPS.iter().find(|s| s.1 == name).map(|s| s.0)
    }
    pub fn from_code(c: u8) -> Option<Step> {
        STEPS.get(c as usize).map(|s| s.0)
    }
    /// The steps of one rung.
    pub fn of(rung: Rung) -> impl Iterator<Item = Step> {
        STEPS.iter().filter(move |s| s.2 == rung).map(|s| s.0)
    }
}

/// What a step needs from the controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepParams {
    pub class: u8,
    /// Wind at 10 m, m/s.
    pub wind: f64,
    /// Release (stack) height, m.
    pub h: f64,
    /// Downwind distance of the slice, m.
    pub x: f64,
    /// Exit velocity, m/s (rise).
    pub w0: f64,
}

/// One step's answer. `values` is an `nx x ny` grid (row 0 at the top) over
/// `extent = [x0, x1, y0, y1]`, `values2` a second grid on the same layout
/// (day-night's night map, mirror's below-ground ghost); `curves` are (label,
/// xs, ys); `scalars` are documented per step in [`compute`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepFrame {
    pub step: u8,
    pub nx: u32,
    pub ny: u32,
    pub extent: [f64; 4],
    pub values: Vec<f64>,
    pub values2: Vec<f64>,
    pub curves: Vec<(String, Vec<f64>, Vec<f64>)>,
    pub scalars: Vec<f64>,
}

/// Wind at the release height, m/s: pyDOSEIA's power-law correction, the
/// `u` its plume uses.
pub fn release_wind(class: u8, wind: f64, h: f64) -> f64 {
    wind * height_correction_factor(
        plume_class(class),
        Length::new::<meter>(h),
        Length::new::<meter>(MEASUREMENT_HEIGHT_M),
    )
}

/// buangkok's `(sigma_y, sigma_z)` at `x` m, in m.
pub fn sigmas(class: u8, x: f64) -> (f64, f64) {
    let c = plume_class(class);
    let l = Length::new::<meter>(x.max(1.0));
    (sigma_y(c, l).get::<meter>(), sigma_z(c, l).get::<meter>())
}

/// `chi/Q` at `(x, y, z)` from buangkok's master equation, s/m³.
fn chi_over_q_at(class: u8, u: f64, h: f64, x: f64, y: f64, z: f64) -> f64 {
    let c = plume_class(class);
    let l = Length::new::<meter>(x.max(1.0));
    let t = master_equation_single_plume(
        sigma_y(c, l),
        sigma_z(c, l),
        u,
        Length::new::<meter>(h),
        Receptor::Offset {
            y: Length::new::<meter>(y),
            z: Length::new::<meter>(z),
        },
    );
    t.pre_expo * t.expo
}

/// `u * integral of chi/Q dy dz` over `z >= 0` at `x`: the trapezoid rule of
/// buangkok's `plume_mass_flux_conservation` test (steps of `sigma / 4`,
/// `+/- 10 sigma_y`, `[0, H + 10 sigma_z]`), which should give 1.
pub fn flux_ratio(class: u8, u: f64, h: f64, x: f64) -> f64 {
    let (sy, sz) = sigmas(class, x);
    let ny = 80;
    let ystep = sy / 4.0;
    let ztop = h + 10.0 * sz;
    let nz = (ztop / (sz / 4.0)).ceil() as usize;
    let zstep = ztop / nz as f64;
    let w = |i: usize, n: usize| if i == 0 || i == n { 0.5 } else { 1.0 };
    let mut total = 0.0;
    for j in 0..=ny {
        let y = -10.0 * sy + j as f64 * ystep;
        for k in 0..=nz {
            total += w(j, ny) * w(k, nz) * chi_over_q_at(class, u, h, x, y, k as f64 * zstep);
        }
    }
    u * total * ystep * zstep
}

/// Compute one step.
pub fn compute(step: Step, p: StepParams) -> StepFrame {
    let u = release_wind(p.class, p.wind, p.h);
    let reach = if step == Step::Rise { RISE_REACH_M } else { REACH_M };
    let xs: Vec<f64> = (0..=240)
        .map(|i| 1.0 + (reach - 1.0) * i as f64 / 240.0)
        .collect();
    let curve = |label: &str, f: &dyn Fn(f64) -> f64| {
        (
            label.to_string(),
            xs.clone(),
            xs.iter().map(|x| f(*x)).collect::<Vec<f64>>(),
        )
    };
    let mut out = StepFrame {
        step: step as u8,
        ..Default::default()
    };
    match step {
        Step::Tracers | Step::SinglePuff => {
            // Scalars: release-height wind (m/s), release height (m).
            out.curves = vec![
                curve("sigma_y", &|x| sigmas(p.class, x).0),
                curve("sigma_z", &|x| sigmas(p.class, x).1),
            ];
            out.scalars = vec![u, p.h];
        }
        Step::Rise => {
            // Scalars: release-height wind, stack height, effective height at
            // the far edge, W0/U (above 1.5 the neutral formula's downwash
            // term adds height: the open question of #542).
            let rise = |x: f64| plume_rise_m(p.class, p.w0, u, x).max(0.0);
            out.curves = vec![
                curve("sigma_y", &|x| sigmas(p.class, x).0),
                curve("sigma_z", &|x| sigmas(p.class, x).1),
                curve("centreline", &|x| p.h + rise(x)),
            ];
            out.scalars = vec![u, p.h, p.h + rise(RISE_REACH_M), p.w0 / u];
        }
        Step::Slice => {
            // A y-z slice at x, on a frame fixed by the far edge's spread so
            // the widening shows. Scalars: x, sigma_y, sigma_z, flux ratio,
            // the colour scale (the slice's peak at 200 m).
            let (sy_far, sz_far) = sigmas(p.class, REACH_M);
            let half_y = 3.0 * sy_far;
            let top = p.h + 3.0 * sz_far;
            let (nx, ny) = (72u32, 48u32);
            let mut v = Vec::with_capacity((nx * ny) as usize);
            for row in 0..ny {
                let z = top * (1.0 - (row as f64 + 0.5) / ny as f64);
                for col in 0..nx {
                    let y = -half_y + 2.0 * half_y * (col as f64 + 0.5) / nx as f64;
                    v.push(chi_over_q_at(p.class, u, p.h, p.x, y, z));
                }
            }
            let (sy, sz) = sigmas(p.class, p.x);
            let scale = chi_over_q_at(p.class, u, p.h, 200.0, 0.0, p.h);
            out.nx = nx;
            out.ny = ny;
            out.extent = [-half_y, half_y, 0.0, top];
            out.values = v;
            out.scalars = vec![p.x, sy, sz, flux_ratio(p.class, u, p.h, p.x), scale];
        }
        Step::Mirror => {
            // Side view x-z on the centreline, z from -top to top: the
            // plume as coded (with its image) above ground; below ground,
            // the part of the unreflected Gaussian that would have gone
            // there (the formula without the image term, buangkok's
            // sigmas). Scalars: release height, release-height wind.
            let sz_far = sigmas(p.class, REACH_M).1;
            let top = p.h + 3.0 * sz_far;
            let (nx, ny) = (96u32, 64u32);
            let (mut v, mut ghost) = (Vec::new(), Vec::new());
            for row in 0..ny {
                let z = top - 2.0 * top * (row as f64 + 0.5) / ny as f64;
                for col in 0..nx {
                    let x = 10.0 + (REACH_M - 10.0) * (col as f64 + 0.5) / nx as f64;
                    if z >= 0.0 {
                        v.push(chi_over_q_at(p.class, u, p.h, x, 0.0, z));
                        ghost.push(0.0);
                    } else {
                        let (sy, sz) = sigmas(p.class, x);
                        v.push(0.0);
                        ghost.push(
                            (-(z - p.h).powi(2) / (2.0 * sz * sz)).exp()
                                / (2.0 * std::f64::consts::PI * sy * sz * u),
                        );
                    }
                }
            }
            out.nx = nx;
            out.ny = ny;
            out.extent = [0.0, REACH_M, -top, top];
            out.values = v;
            out.values2 = ghost;
            out.scalars = vec![p.h, u];
        }
        Step::DayNight => {
            // changi's Pasquill lookup at 14:00 and 02:00; buangkok's plume
            // for the first class of each. Scalars: day class, day's second
            // class (-1 if none), night class, night's second (-1).
            use changi::puff::stability::{stability_class, StabilitySet};
            let classes = |hour: u32| match stability_class(
                Some(Velocity::new::<meter_per_second>(p.wind)),
                hour,
            ) {
                StabilitySet::One(a) => (a as u8, -1.0),
                StabilitySet::Two(a, b) => (a as u8, b as u8 as f64),
            };
            let (day, day2) = classes(14);
            let (night, night2) = classes(2);
            let pp = PlumeParams {
                class: 0,
                wind: p.wind,
                h: p.h,
                dir_deg: 90.0,
                cells: 64,
                half_width: REACH_M / 2.0,
            };
            let map = |class: u8| {
                plume_field(&pp, |x, y| plume_chi_over_q(class, p.wind, p.h, x, y))
            };
            out.nx = 64;
            out.ny = 64;
            out.extent = [-REACH_M / 2.0, REACH_M / 2.0, -REACH_M / 2.0, REACH_M / 2.0];
            out.values = map(day);
            out.values2 = map(night);
            out.scalars = vec![day as f64, day2, night as f64, night2];
        }
    }
    out
}

impl StepFrame {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        let mut d = vec![
            self.step as f64,
            self.nx as f64,
            self.ny as f64,
            self.values.len() as f64,
            self.values2.len() as f64,
            self.scalars.len() as f64,
            self.curves.len() as f64,
        ];
        d.extend_from_slice(&self.extent);
        d.extend_from_slice(&self.values);
        d.extend_from_slice(&self.values2);
        d.extend_from_slice(&self.scalars);
        for (_, xs, ys) in &self.curves {
            d.push(xs.len() as f64);
            d.extend_from_slice(xs);
            d.extend_from_slice(ys);
        }
        let labels = self
            .curves
            .iter()
            .map(|c| c.0.as_str())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        (d, labels)
    }

    pub fn from_wire(d: &[f64], text: &str) -> Result<StepFrame, String> {
        let mut i = 0;
        let mut take = |n: usize| -> Result<Vec<f64>, String> {
            if d.len() < i + n {
                return Err("step frame truncated".into());
            }
            let v = d[i..i + n].to_vec();
            i += n;
            Ok(v)
        };
        let h = take(7)?;
        let e = take(4)?;
        let values = take(h[3] as usize)?;
        let values2 = take(h[4] as usize)?;
        let scalars = take(h[5] as usize)?;
        let mut labels = text.split('\u{1f}');
        let mut curves = Vec::new();
        for _ in 0..h[6] as usize {
            let n = take(1)?[0] as usize;
            let xs = take(n)?;
            let ys = take(n)?;
            curves.push((labels.next().unwrap_or("").to_string(), xs, ys));
        }
        Ok(StepFrame {
            step: h[0] as u8,
            nx: h[1] as u32,
            ny: h[2] as u32,
            extent: [e[0], e[1], e[2], e[3]],
            values,
            values2,
            curves,
            scalars,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> StepParams {
        StepParams {
            class: 3,
            wind: 2.0,
            h: 30.0,
            x: 1000.0,
            w0: 10.0,
        }
    }

    /// Every step computes, survives the wire, and the table agrees with
    /// itself.
    #[test]
    fn every_step_computes_and_round_trips() {
        for (i, s) in STEPS.iter().enumerate() {
            assert_eq!(s.0 as usize, i);
            assert_eq!(Step::parse(s.1), Some(s.0));
            let f = compute(s.0, params());
            let (d, t) = f.to_wire();
            assert_eq!(StepFrame::from_wire(&d, &t).unwrap(), f);
        }
    }

    /// The slice step's on-screen flux is buangkok's conservation check: 1 to
    /// 1e-12 at every distance it animates (that test's own criterion).
    #[test]
    fn the_slice_carries_the_whole_release() {
        for x in [100.0, 500.0, 1000.0, 3000.0] {
            let f = compute(Step::Slice, StepParams { x, ..params() });
            assert!((f.scalars[3] - 1.0).abs() < 1e-12, "x = {x}: {}", f.scalars[3]);
        }
    }

    /// The mirror step's ghost is exactly the image the code adds: above
    /// ground the coded plume is the direct term plus the direct term
    /// reflected through z = 0.
    #[test]
    fn the_ghost_is_the_image() {
        let p = params();
        let u = release_wind(p.class, p.wind, p.h);
        let (x, z) = (800.0, 12.0);
        let (sy, sz) = sigmas(p.class, x);
        let direct = |z: f64| {
            (-(z - p.h).powi(2) / (2.0 * sz * sz)).exp()
                / (2.0 * std::f64::consts::PI * sy * sz * u)
        };
        let coded = chi_over_q_at(p.class, u, p.h, x, 0.0, z);
        assert!((coded - direct(z) - direct(-z)).abs() < 1e-12 * coded);
    }

    /// Day and night differ at 2 m/s: changi's table gives B at 14:00 and
    /// E or F at 02:00.
    #[test]
    fn day_and_night_classes_follow_changi() {
        let f = compute(Step::DayNight, params());
        assert_eq!(f.scalars[0], 1.0);
        assert_eq!(f.scalars[2], 4.0);
        assert_eq!(f.scalars[3], 5.0);
    }
}
