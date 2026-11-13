//! # V&V: `tampines`'s swollen level against `sembawang`'s (gh:#1092)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! `tampines::control_volume::swollen_level` is a port of
//! `sembawang::bwr_mark1::swollen_level` (MELCOR 2.2 CVH/FL RM §5.1.3, Eqs
//! 5-26 to 5-30, cited, not quoted) with the vessel's `volume_below` and
//! `height_of_volume` passed in as closures. The comparison lives here
//! because `sembawang` depends on `tampines` (maintainer direction,
//! 2026-11-01), never the reverse.
//!
//! **Methodology.** `sembawang`'s Fukushima Unit 1 vessel (`Params::default`),
//! saturated water at about 7 MPa (`h_fg = 1.5e6 J/kg`, `v_g = 0.027 m³/kg`,
//! the inputs of `sembawang`'s own unit test), collapsed levels from 0.5 m
//! below to 6 m above the bottom of active fuel (lower plenum, core and upper
//! plenum, so all three pieces of the vessel's `volume_below`), heat from
//! 1 kW to 1 GW (from no swell to the 0.4 void cap). Each pair is compared
//! with `f64::to_bits`.
//!
//! **Prediction (written 2026-11-01 before the first run):** bit-identical
//! for every case with heat (the forms are identical, operation for
//! operation); with no heat both return the collapsed level (`tampines` in
//! its own datum, so `(level + baf) − baf`, compared to 1e-15 m, not bits).
//!
//! **Result (2026-11-01, release):** 64 of 64 heated cases bit-identical;
//! the unheated cases agree to within 1e-15 m. As predicted.

use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::{height_of_volume, swollen_level as sem_swollen, volume_below};
use tampines::control_volume::swollen_level;

#[test]
fn swollen_level_is_bit_identical_to_sembawangs() {
    let prm = Params::default();
    let (h_fg, v_g) = (1.5e6, 0.027);
    let mut n = 0;
    for level in [-0.5, 0.0, 1.0, 2.0, 3.5, 4.2, 5.0, 6.0] {
        let z = level + prm.baf_m;
        for q in [1.0e3, 1.0e5, 1.0e6, 1.0e7, 3.0e7, 1.0e8, 3.0e8, 1.0e9] {
            let ours = swollen_level(|x| volume_below(&prm, x), |v| height_of_volume(&prm, v), z, q, h_fg, v_g, prm.bubble_rise_m_s, prm.max_pool_void) - prm.baf_m;
            let theirs = sem_swollen(&prm, level, q, h_fg, v_g);
            assert_eq!(ours.to_bits(), theirs.to_bits(), "level {level} m, q {q} W: {ours} vs {theirs}");
            n += 1;
        }
        let ours = swollen_level(|x| volume_below(&prm, x), |v| height_of_volume(&prm, v), z, 0.0, h_fg, v_g, prm.bubble_rise_m_s, prm.max_pool_void) - prm.baf_m;
        assert!((ours - sem_swollen(&prm, level, 0.0, h_fg, v_g)).abs() < 1e-15 + 1e-15 * level.abs());
    }
    assert_eq!(n, 64);
}
