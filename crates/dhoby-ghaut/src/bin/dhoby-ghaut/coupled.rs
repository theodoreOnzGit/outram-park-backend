//! Step 10's engine thread: runs [`crate::porous_core::solve`] off the UI
//! thread and streams every coupling iteration (a fourth engine, like
//! `dem.rs`: a run takes seconds, and slices and the 3D view stay live).

use std::sync::{Arc, RwLock};

use dhoby_ghaut::web_demo::link::NativeEngine;
use dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup;

use crate::porous_core::{Fields, IterationReport, Summary};

/// Set to `true` to stop at the next iteration.
pub type StopFlag = Arc<RwLock<bool>>;

pub enum MpReq {
    Run {
        setup: MultiphysicsSetup,
        stop: StopFlag,
    },
}

pub enum MpEv {
    Started {
        delta_note: String,
    },
    Iteration {
        report: IterationReport,
        fields: Fields,
        seconds: f64,
    },
    Done {
        summary: Summary,
        fields: Fields,
        stopped: bool,
        seconds: f64,
    },
    Error(String),
}

#[derive(Default)]
pub struct MpEngine;

impl NativeEngine for MpEngine {
    type Req = MpReq;
    type Ev = MpEv;

    fn handle(&mut self, req: MpReq, post: &mut impl FnMut(MpEv)) {
        let MpReq::Run { setup, stop } = req;
        let t0 = std::time::Instant::now();
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match crate::porous_core::PorousCore::new(&setup) {
                Ok(c) => post(MpEv::Started {
                    delta_note: if c.delta_m().is_finite() {
                        format!(
                            "power shape: extrapolation length {:.1} cm",
                            c.delta_m() * 100.0
                        )
                    } else {
                        "power shape: uniform".into()
                    },
                }),
                Err(e) => {
                    post(MpEv::Error(format!("set-up refused: {e}")));
                    return;
                }
            }
            let stopped = || stop.read().map_or(false, |s| *s);
            let res = crate::porous_core::solve(&setup, &stopped, |r, f| {
                post(MpEv::Iteration {
                    report: r.clone(),
                    fields: f.clone(),
                    seconds: t0.elapsed().as_secs_f64(),
                });
            });
            match res {
                Ok((summary, fields)) => {
                    let stopped = stopped() && !summary.converged;
                    post(MpEv::Done {
                        summary,
                        fields,
                        stopped,
                        seconds: t0.elapsed().as_secs_f64(),
                    });
                }
                Err(e) => post(MpEv::Error(format!("coupled run failed: {e}"))),
            }
        }));
        if let Err(e) = run {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            post(MpEv::Error(format!("coupled run panicked: {msg}")));
        }
    }
}
