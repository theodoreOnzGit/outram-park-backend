//! The `dhshort` rung: **the demo of the lesson's step 5 code walk**
//! (`dh_keff_vv.rs::main` → `DhUniverse::keff` → `run_keff_delta_in`, gh:#785).
//! The same FHR unit cell under each `DhTreatment`: what geometry and material
//! each one answers `material_at` with (a live slice, drawn in the worker from
//! the universe `DhUniverse::pebble` builds), beside the recorded k bias and
//! speed-up of each, with dates and re-measurement status.
//!
//! **No k is computed here.** The recorded numbers are quoted from their
//! records ([`record`]). A live low-statistics run was considered and its
//! feasibility predicted on gh:#785; see the rung's notes for what was decided.
//! The picture needs no nuclear data, so the rung loads none.

pub mod model;
pub mod record;
pub mod ui;

use crate::anim::Spectrum;
use crate::engine::Tier;
use crate::history::History;
use crate::keff::{Generation, KeffConfig};
use crate::raster::RasterReq;
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use crate::walkdemo::WalkKind;
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::view::View;
use egui::Rect;
use outram_mc_libs::dh_universe::DhUniverse;
use outram_mc_libs::geometry::position::Position;

/// The marker type `rung_table!` names.
pub struct DhShort;

impl McRung for DhShort {
    const INFO: RungInfo = RungInfo {
        name: "dhshort",
        title: "TRISO step 5: the DH shortcuts, side by side",
        lesson: "tutorials/monte-carlo/triso.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = Builder;

    /// No tapes: drawing `material_at` needs compositions only.
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
        3.1
    }
    /// The picture is the live slice ([`ui`]); nothing static.
    fn draw(_painter: &egui::Painter, _rect: Rect, _view: &View) {}
    fn notes() -> &'static [&'static str] {
        &[
            "Geometry: dh_keff_vv.rs's FHR unit cell (1.9 cm fuel zone at a particle packing fraction of 0.30, 2.0 cm pebble, FLiBe to a reflective sphere at 3.0 cm), built by DhUniverse::pebble under each treatment in this tab's worker, with the record's material table and packing seed. Every pixel is what DhUniverse::material_at answers there: the same call the delta-tracked power iteration makes.",
            "CLS and SCLS store no geometry. Their material_at SAMPLES, so their picture is drawn as one flight per pixel row (left to right) and a redraw gives a different picture. It is what one neutron would meet, not a map of the fuel.",
            "Recorded results only: no k is computed in this tab. Every record predates a change that could move it (URR and DBRC on by default from 2026-09-20; SCLS's wiring fixed on 2026-09-14), so all are marked re-measurement pending (#582). Quote ratios of speed, not seconds: the records ran on different hosts.",
            "A bias that is not resolved means the run could not measure it, not that it is zero.",
            "No live k here: SCLS costs about 24 times delta tracking (the #582 pilot), and the other arms' live run was not built in this change; the predicted browser cost is on #785.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn has_tracks() -> bool {
        false
    }
    fn walk_demo() -> Option<WalkKind> {
        Some(WalkKind::Shortcuts)
    }
}

/// Nothing to process.
pub struct Builder;

impl RungBuilder for Builder {
    type Loaded = Loaded;
    fn new(_tier: Tier) -> Self {
        Builder
    }
    fn step(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Err("the dhshort rung processes no tapes".into())
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded::default())
    }
}

/// The worker side: each treatment's universe, built on first use, and how
/// long it took.
#[derive(Default)]
pub struct Loaded {
    universes: Vec<Option<(DhUniverse, f64)>>,
}

/// Message codes of [`LoadedRung::walk`] for this rung.
pub const MSG_DESCRIBE: f64 = 0.0;

impl Loaded {
    /// Treatment `i`'s universe and its build time, s (built if needed).
    pub fn universe(&mut self, i: usize) -> Result<(&DhUniverse, f64), String> {
        if self.universes.len() < model::TREATMENTS.len() {
            self.universes.resize_with(model::TREATMENTS.len(), || None);
        }
        let slot = self
            .universes
            .get_mut(i)
            .ok_or_else(|| format!("no treatment {i}"))?;
        if slot.is_none() {
            let t = now_s();
            let u = model::build(i)?;
            *slot = Some((u, now_s() - t));
        }
        let (u, s) = slot.as_ref().ok_or("not built")?;
        Ok((u, *s))
    }
}

impl LoadedRung for Loaded {
    /// No tracks on this rung (`has_tracks` is false); never asked for.
    fn run_next(&mut self) -> History {
        History::from_track(0, Position::ZERO, 0.0, false, Default::default())
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the dhshort rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    /// `param`'s integer part is the treatment; its fraction counts redraws.
    fn raster(&mut self, req: &RasterReq) -> Result<Vec<u8>, String> {
        let (u, _) = self.universe(req.param.max(0.0).floor() as usize)?;
        Ok(model::raster(u, req))
    }
    /// `[MSG_DESCRIBE, i]` → `[MSG_DESCRIBE, i, build s, particles stored,
    /// packing fraction modelled, materials in the table]`.
    fn walk(&mut self, msg: &[f64]) -> Result<Vec<f64>, String> {
        match msg {
            [c, i] if *c == MSG_DESCRIBE => {
                let i = i.max(0.0) as usize;
                let (u, secs) = self.universe(i)?;
                Ok(vec![
                    MSG_DESCRIBE,
                    i as f64,
                    secs,
                    u.particle_count() as f64,
                    u.packing_fraction(),
                    u.materials().len() as f64,
                ])
            }
            _ => Err(format!("dhshort: unknown message {msg:?}")),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::raster::Basis;

    /// The worker answers a describe and a raster for every treatment, as the
    /// page asks for them; the explicit arm's realised packing fraction is
    /// `pack_in_ball`'s, within 0.2 % of the requested 0.30.
    #[test]
    fn the_worker_serves_every_treatment() {
        let mut l = Builder::new(Tier::Loose).finish().unwrap();
        for i in 0..model::TREATMENTS.len() {
            let d = l.walk(&[MSG_DESCRIBE, i as f64]).unwrap();
            assert_eq!(d.len(), 6);
            assert_eq!(d[1] as usize, i);
            if i == 0 {
                assert!(d[3] > 20_000.0, "particles stored: {}", d[3]);
                assert!((d[4] / 0.30 - 1.0).abs() <= 2.0e-3, "pf {}", d[4]);
            } else {
                assert_eq!(d[3], 0.0);
            }
            let req = RasterReq {
                id: 2,
                basis: Basis::Xy,
                centre: [0.0, 0.0],
                depth: 0.0,
                width: [6.2, 6.2],
                px: [32, 24],
                param: i as f64 + 0.003,
            };
            assert_eq!(l.raster(&req).unwrap().len(), 32 * 24);
        }
        assert!(l.walk(&[9.0]).is_err());
    }
}
