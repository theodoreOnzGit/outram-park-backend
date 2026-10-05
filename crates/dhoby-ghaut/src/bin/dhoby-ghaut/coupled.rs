//! Step 10's engine thread: runs the coupled case off the UI thread and
//! streams every coupling iteration (a fourth engine, like `dem.rs`: a run
//! takes seconds to minutes, and slices and the 3D view stay live).
//!
//! With the solved power shape (the default, gh:#591) it runs
//! [`crate::spatial::solve`] on the Step 7/8 hand-off and, at the end, draws
//! the computed power on the neutronics mesh; with a prescribed shape (an
//! ablation) [`crate::porous_core::solve`].

use std::sync::{Arc, RwLock};

use dhoby_ghaut::web_demo::link::NativeEngine;
use dhoby_ghaut::workbench::multiphysics::{MultiphysicsInputs, MultiphysicsSetup};
use outram_blender::csg::plot::{ImageData, PlotBasis};
use outram_blender::unstructured::UnstructuredMesh;

use crate::porous_core::{Fields, IterationReport, Summary};

/// Set to `true` to stop at the next iteration.
pub type StopFlag = Arc<RwLock<bool>>;

/// What the solved shape needs from Steps 7 and 8.
pub struct SpatialJob {
    /// The hand-off.
    pub inputs: Arc<MultiphysicsInputs>,
    /// The neutronics mesh in memory, to draw the computed power on.
    pub n_mesh: Arc<UnstructuredMesh>,
    /// The TH mesh in memory (point location for the ring-grid transfer).
    pub th_mesh: Arc<UnstructuredMesh>,
}

pub enum MpReq {
    Run {
        setup: MultiphysicsSetup,
        stop: StopFlag,
        /// Needed (and only used) when the setup's shape is solved.
        spatial: Option<SpatialJob>,
    },
}

pub enum MpEv {
    Started {
        notes: Vec<String>,
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
        /// The computed power drawn on the neutronics mesh (X-Z), solved
        /// shape only.
        mesh_power: Option<ImageData>,
    },
    Error(String),
}

#[derive(Default)]
pub struct MpEngine;

impl NativeEngine for MpEngine {
    type Req = MpReq;
    type Ev = MpEv;

    fn handle(&mut self, req: MpReq, post: &mut impl FnMut(MpEv)) {
        let MpReq::Run {
            setup,
            stop,
            spatial,
        } = req;
        let t0 = std::time::Instant::now();
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let stopped = || stop.read().map_or(false, |s| *s);
            if setup.neutronics.shape.is_solved() {
                let Some(job) = spatial else {
                    post(MpEv::Error(
                        "the solved power shape needs Step 7's meshes and Step 8's cross sections: \
                         run Steps 7 and 8 first, or choose a prescribed shape (an ablation) in Step 9"
                            .into(),
                    ));
                    return;
                };
                // Events are posted from inside the solver's callbacks.
                let post_cell = std::cell::RefCell::new(&mut *post);
                let res = crate::spatial::solve(
                    &setup,
                    &job.inputs,
                    &job.th_mesh,
                    crate::spatial::NeutronBoundary::MarshakFace,
                    stopped,
                    |notes| {
                        (*post_cell.borrow_mut())(MpEv::Started {
                            notes: notes.to_vec(),
                        })
                    },
                    |r, f| {
                        (*post_cell.borrow_mut())(MpEv::Iteration {
                            report: r.clone(),
                            fields: f.clone(),
                            seconds: t0.elapsed().as_secs_f64(),
                        });
                    },
                );
                let post = post_cell.into_inner();
                match res {
                    Ok((summary, fields, sf)) => {
                        let q: Vec<f64> = sf.q_n_w_m3.iter().map(|q| q * 1e-6).collect();
                        let mesh_power = crate::spatial_draw::draw_field(
                            &job.n_mesh,
                            &q,
                            PlotBasis::Xz,
                            [0.0; 3],
                            "STEP 10 COMPUTED POWER DENSITY ON THE NEUTRONICS MESH, X-Z",
                            "W/CM3",
                            1e-9,
                            "NO FISSION POWER",
                            700,
                        )
                        .ok();
                        let stopped = stopped() && !summary.converged;
                        post(MpEv::Done {
                            summary,
                            fields,
                            stopped,
                            seconds: t0.elapsed().as_secs_f64(),
                            mesh_power,
                        });
                    }
                    Err(e) => post(MpEv::Error(format!("coupled run failed: {e}"))),
                }
                return;
            }
            match crate::porous_core::PorousCore::new(&setup) {
                Ok(c) => post(MpEv::Started {
                    notes: vec![if c.delta_m().is_finite() {
                        format!(
                            "ABLATION, prescribed power shape: extrapolation length {:.1} cm",
                            c.delta_m() * 100.0
                        )
                    } else {
                        "ABLATION, prescribed power shape: uniform".into()
                    }],
                }),
                Err(e) => {
                    post(MpEv::Error(format!("set-up refused: {e}")));
                    return;
                }
            }
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
                        mesh_power: None,
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
