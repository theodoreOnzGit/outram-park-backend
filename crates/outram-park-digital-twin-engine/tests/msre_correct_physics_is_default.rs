//! **Correct physics is the DEFAULT for the MSRE model, and this test is what
//! keeps it that way** (gh:#1330).
//!
//! Workspace hard rule (root `CLAUDE.md`, "Correct physics is the DEFAULT
//! SETTING"; pattern: `crates/outram-mc-libs/tests/correct_physics_is_default.rs`).
//! Maintainer decision 2026-11-11 on gh:#1330: "Let's use ORNL more correct
//! physics". `msre::model::MsreModel`'s circulating-fuel kinetics are
//! ORNL-TM-1626's importance-weighted line model
//! (`CirculationModel::LineTm1626`) unless a caller explicitly ablates it
//! with `MsreConfig::without_line_model()`.
//!
//! Why it matters: TM-1070's well-stirred model, the default until
//! 2026-11-11, counts the plenums' delayed neutrons as worthless. Its
//! circulation loss reads +16 % against TM-1626's measured 0.212 % (the line
//! model +8 %), and after a fuel-pump start-up it returns the precursors 3-4 s
//! later than ORNL-4233 Fig. 24 shows (gh:#1332). A default that nothing
//! asserts drifts back silently, so this test constructs the model through
//! every ordinary path and asserts which kinetics are in force.

use outram_park_digital_twin_engine::msre::circulation::CirculationModel;
use outram_park_digital_twin_engine::msre::model::{MsreConfig, MsreModel};
use uom::si::f64::Power;
use uom::si::power::megawatt;
use uom::si::ratio::ratio;

/// The ordinary constructor, `MsreConfig::tm1070`, at three powers, gives the
/// line model; the enum's own `Default` is the line model too.
#[test]
fn the_line_model_is_the_default_circulation_model() {
    assert_eq!(
        CirculationModel::default(),
        CirculationModel::LineTm1626,
        "CirculationModel::default() is not TM-1626's line model: the \
         well-stirred TM-1070 model is an ablation (gh:#1330)"
    );
    for p in [1.0e-5, 1.0, 8.0] {
        let cfg = MsreConfig::tm1070(Power::new::<megawatt>(p));
        assert_eq!(cfg.circulation_model, CirculationModel::LineTm1626, "MsreConfig::tm1070 at {p} MW");
        let m = MsreModel::new(cfg).expect("MSRE model");
        assert_eq!(
            m.circulation_model(),
            CirculationModel::LineTm1626,
            "MsreModel built through MsreConfig::tm1070 at {p} MW does not run \
             the line model: the correct-physics default has been turned off"
        );
        // The physics is present, not just the label: the loss is the line
        // model's (0.229140 %), not the well-stirred 0.2467041 %.
        let loss = 100.0 * m.circulation_loss().get::<ratio>();
        assert!((loss - 0.2291400).abs() < 1e-6, "circulation loss {loss} % at {p} MW");
    }
}

/// The ablation is an explicit, visible act, and it does what it says.
#[test]
fn the_well_stirred_model_is_an_explicit_ablation() {
    let cfg = MsreConfig::tm1070(Power::new::<megawatt>(8.0)).without_line_model();
    assert_eq!(cfg.circulation_model, CirculationModel::WellStirredTm1070);
    let m = MsreModel::new(cfg).expect("MSRE model");
    assert_eq!(m.circulation_model(), CirculationModel::WellStirredTm1070);
    let loss = 100.0 * m.circulation_loss().get::<ratio>();
    assert!((loss - 0.2467041).abs() < 1e-6, "ablation circulation loss {loss} %");
    // Everything but the circulation model is unchanged by the ablation.
    let base = MsreConfig::tm1070(Power::new::<megawatt>(8.0));
    assert_eq!(MsreConfig { circulation_model: base.circulation_model, ..cfg }, base);
}
