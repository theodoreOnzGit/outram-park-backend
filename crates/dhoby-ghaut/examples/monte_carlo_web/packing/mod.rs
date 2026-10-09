//! The `packing` rung: **the demo of the lesson's step 6 code walk**
//! (`DhUniverse::pebble` → `pack_in_ball` → `PackingConfig::generate` →
//! `pack_spheres`, gh:#785). Random sequential addition placing the FHR
//! pebble's TRISO particles one at a time, the packing fraction as placement
//! goes, and the cut-particle pitfall (0.30 asked, about 0.289 kept) with
//! `pack_in_ball`'s fix.
//!
//! **The real packer, in the worker.** `pack_in_ball` runs once to get its
//! attempt record (`BallPacking::attempts`); the attempt the reader picks is
//! then replayed through `pack_spheres_observed`, which IS `pack_spheres` with
//! an observer (pinned bit for bit in `sphere_packing`'s tests), and the page
//! animates the placements in order. Nothing is re-implemented here; the
//! replay is checked against the attempt record before it is sent.
//!
//! The parameters are `dh_keff_vv`'s: `TrisoSpec::FHR_HALEU_UCO` (particle
//! radius 0.0425 cm, packing fraction 0.30), a 1.9 cm fuel zone, the cube of
//! half-width 1.9 + 0.0425 cm that `pack_in_ball` packs, and the record's seed
//! (`PebbleParams::fhr_unit_cell().seed | 1`). No nuclear data.

pub mod ui;

use crate::anim::Spectrum;
use crate::engine::Tier;
use crate::history::History;
use crate::keff::{Generation, KeffConfig};
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use crate::walkdemo::WalkKind;
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::view::View;
use egui::Rect;
use outram_mc_libs::dh_universe::{is_whole_in_ball, pack_in_ball, BallPackingAttempt, PebbleParams};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::pebble_beds::sphere_packing::pack_spheres_observed;

/// The marker type `rung_table!` names.
pub struct Packing;

/// Message codes of [`LoadedRung::walk`] for this rung.
pub const MSG_PLAN: f64 = 0.0;
pub const MSG_REPLAY: f64 = 1.0;
/// Values per placement in a replay answer: x, y, z, trials, whole (0/1).
pub const PER_PLACEMENT: usize = 5;

/// Particle radius (to the OPyC), fuel-zone radius, requested fraction, seed.
pub fn params() -> (f64, f64, f64, u64) {
    let p = PebbleParams::fhr_unit_cell();
    (
        p.spec.opyc,
        p.fuel_zone_radius,
        p.spec.packing_fraction,
        p.seed | 1,
    )
}

impl McRung for Packing {
    const INFO: RungInfo = RungInfo {
        name: "packing",
        title: "TRISO step 6: random packing and cut particles",
        lesson: "tutorials/monte-carlo/triso.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = Builder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &[]
    }
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    fn job_weights(_tier: Tier) -> Vec<f64> {
        Vec::new()
    }
    fn half_extent() -> f64 {
        2.0
    }
    fn draw(_painter: &egui::Painter, _rect: Rect, _view: &View) {}
    fn notes() -> &'static [&'static str] {
        &[
            "The real packer: pack_in_ball runs in this tab's worker, then the attempt shown is replayed through pack_spheres_observed (pack_spheres with an observer, bit for bit the same list) and animated in placement order. The FHR pebble of dh_keff_vv.rs: particle radius 0.0425 cm, a 1.9 cm fuel zone, 0.30 requested, the record's seed.",
            "The picture is the plane z = 0: a particle shows only if it crosses that plane (about 2 % of them), at its cross-section there. The counts and fractions are over all particles, not the slice.",
            "Whole means |centre| + r <= R (dh_universe::is_whole_in_ball, pack_in_ball's keep rule). Cut means the particle straddles the fuel-zone surface. The cube's corners hold particles outside the zone entirely.",
            "RSA cannot pass about 0.38 (MAX_PF_RSA): the trials per placement rise as the cube fills. Real fuel zones sit at 0.05 to 0.30.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn has_tracks() -> bool {
        false
    }
    fn walk_demo() -> Option<WalkKind> {
        Some(WalkKind::Packing)
    }
}

/// Nothing to process.
pub struct Builder;

impl RungBuilder for Builder {
    type Loaded = Loaded;
    fn new(_tier: Tier) -> Self {
        Builder
    }
    fn step(&mut self, _bytes: &[u8], _store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        Err("the packing rung processes no tapes".into())
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded::default())
    }
}

/// The worker side: `pack_in_ball`'s attempt record, once computed.
#[derive(Default)]
pub struct Loaded {
    plan: Option<(Vec<BallPackingAttempt>, usize)>,
}

impl Loaded {
    /// Run `pack_in_ball` (once) and return its attempts and the index of the
    /// one it kept, with the seconds it took (0 if already known).
    pub fn plan(&mut self) -> Result<(&[BallPackingAttempt], usize, f64), String> {
        let mut secs = 0.0;
        if self.plan.is_none() {
            let (r_p, big_r, pf, seed) = params();
            let t = now_s();
            let b = pack_in_ball(r_p, big_r, pf, seed).map_err(|e| e.to_string())?;
            secs = now_s() - t;
            let best = b
                .attempts
                .iter()
                .position(|a| a.realised == b.realised)
                .ok_or("pack_in_ball: best attempt not in its record")?;
            self.plan = Some((b.attempts, best));
        }
        let (a, best) = self.plan.as_ref().ok_or("no plan")?;
        Ok((a, *best, secs))
    }

    /// Replay attempt `i` through the packer with an observer: per placement
    /// x, y, z, trials, whole. Checked against the attempt record.
    pub fn replay(&mut self, i: usize) -> Result<Vec<f64>, String> {
        let (attempts, _, _) = self.plan()?;
        let a = *attempts.get(i).ok_or_else(|| format!("no attempt {i}"))?;
        let (r_p, big_r, _, seed) = params();
        let mut out = Vec::with_capacity(PER_PLACEMENT * a.generated);
        let mut kept = 0usize;
        pack_spheres_observed(r_p, big_r + r_p, a.request, seed, |p| {
            let whole = is_whole_in_ball(p.center, r_p, big_r);
            kept += whole as usize;
            out.extend([
                p.center.x,
                p.center.y,
                p.center.z,
                p.trials as f64,
                whole as u8 as f64,
            ]);
        })
        .map_err(|e| e.to_string())?;
        if out.len() != PER_PLACEMENT * a.generated || kept != a.kept {
            return Err(format!(
                "replay of attempt {i} placed {} and kept {kept}; pack_in_ball recorded {} and {}",
                out.len() / PER_PLACEMENT,
                a.generated,
                a.kept
            ));
        }
        Ok(out)
    }
}

impl LoadedRung for Loaded {
    /// No tracks on this rung (`has_tracks` is false); never asked for.
    fn run_next(&mut self) -> History {
        History::from_track(0, Position::ZERO, 0.0, false, Default::default())
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the packing rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    /// `[MSG_PLAN]` → `[MSG_PLAN, secs, best, n, (request, generated, kept,
    /// realised) × n]`; `[MSG_REPLAY, i]` → `[MSG_REPLAY, i, secs, then
    /// PER_PLACEMENT values per placement]`.
    fn walk(&mut self, msg: &[f64]) -> Result<Vec<f64>, String> {
        match msg {
            [c] if *c == MSG_PLAN => {
                let (attempts, best, secs) = self.plan()?;
                let mut v = vec![MSG_PLAN, secs, best as f64, attempts.len() as f64];
                for a in attempts {
                    v.extend([a.request, a.generated as f64, a.kept as f64, a.realised]);
                }
                Ok(v)
            }
            [c, i] if *c == MSG_REPLAY => {
                let t = now_s();
                let body = self.replay(i.max(0.0) as usize)?;
                let mut v = Vec::with_capacity(3 + body.len());
                v.extend([MSG_REPLAY, *i, now_s() - t]);
                v.extend(body);
                Ok(v)
            }
            _ => Err(format!("packing: unknown message {msg:?}")),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The worker's plan is `pack_in_ball`'s: the first attempt asks for the
    /// target and keeps less (the cut particles), the kept one is within
    /// 0.2 %; every attempt replays to the recorded counts. Prints the numbers
    /// quoted on gh:#785.
    #[test]
    fn the_plan_and_its_replays_are_pack_in_ball() {
        let mut l = Builder::new(Tier::Loose).finish().unwrap();
        let t = std::time::Instant::now();
        let p = l.walk(&[MSG_PLAN]).unwrap();
        let plan_s = t.elapsed().as_secs_f64();
        let (best, n) = (p[2] as usize, p[3] as usize);
        assert_eq!(p.len(), 4 + 4 * n);
        let at = |i: usize| &p[4 + 4 * i..8 + 4 * i];
        assert_eq!(at(0)[0], 0.30);
        assert!(at(0)[3] < 0.30, "first attempt realised {}", at(0)[3]);
        assert!((at(best)[3] / 0.30 - 1.0).abs() <= 2.0e-3);
        for i in 0..n {
            let a = at(i);
            eprintln!("  attempt {i}: request {:.5} -> {} placed in the cube, {} whole in the ball, realised {:.5}", a[0], a[1], a[2], a[3]);
            let t = std::time::Instant::now();
            let r = l.walk(&[MSG_REPLAY, i as f64]).unwrap();
            assert_eq!((r.len() - 3) / PER_PLACEMENT, a[1] as usize);
            let whole = r[3..]
                .chunks_exact(PER_PLACEMENT)
                .filter(|c| c[4] == 1.0)
                .count();
            assert_eq!(whole, a[2] as usize);
            let trials: Vec<f64> = r[3..].chunks_exact(PER_PLACEMENT).map(|c| c[3]).collect();
            let tenth = trials.len() / 10;
            let mean = |w: &[f64]| w.iter().sum::<f64>() / w.len() as f64;
            eprintln!(
                "    replay {:.2} s natively; trials per placement: first tenth {:.2}, last tenth {:.2}, max {}",
                t.elapsed().as_secs_f64(), mean(&trials[..tenth]), mean(&trials[trials.len() - tenth..]),
                trials.iter().cloned().fold(0.0, f64::max)
            );
        }
        eprintln!("  pack_in_ball: {plan_s:.2} s natively, kept attempt {best}");
        assert!(l.walk(&[MSG_REPLAY, 99.0]).is_err());
    }
}
