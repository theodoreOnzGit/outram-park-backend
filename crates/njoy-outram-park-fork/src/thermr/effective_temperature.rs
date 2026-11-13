// Translated from NJOY2016.80 `src/thermr.f90`: `gateff` (:594-701) and
// `gatef2` (:703-746). NJOY2016 is under a modified BSD 3-Clause (LANL/DOE)
// licence, GPL-compatible; this derivative file is distributed under
// AGPL-3.0-only. Modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! **Default effective temperatures for ENDF-4/5 thermal evaluations**
//! (`gateff`, `gatef2`): the General Atomic report's `T_eff(T)` for graphite,
//! beryllium, water, ZrH, polyethylene, ..., which pre-ENDF-6 MF=7 files do
//! not carry. THERMR's `calcem` uses it in the short-collision-time branch.

/// `gateff`'s table: `(MAT, T [K], T_eff [K])`.
const GATEFF: [(f64, f64, f64); 67] = [
    (1002., 296.0, 1396.8),
    (1002., 350.0, 1411.6),
    (1002., 400.0, 1427.4),
    (1002., 450.0, 1444.9),
    (1002., 500.0, 1464.1),
    (1002., 600.0, 1506.8),
    (1002., 800.0, 1605.8),
    (1002., 1000., 1719.8),
    (1004., 296.0, 940.91),
    (1004., 350.0, 961.62),
    (1004., 400.0, 982.93),
    (1004., 450.0, 1006.1),
    (1004., 500.0, 1030.9),
    (1004., 600.0, 1085.1),
    (1004., 800.0, 1209.0),
    (1004., 1000., 1350.0),
    (1064., 296.0, 405.64),
    (1064., 400., 484.22),
    (1064., 500.0, 568.53),
    (1064., 600., 657.66),
    (1064., 700., 749.69),
    (1064., 800., 843.63),
    (1064., 1000., 1035.),
    (1064., 1200., 1229.3),
    (1065., 296.0, 713.39),
    (1065., 400.0, 754.68),
    (1065., 500.0, 806.67),
    (1065., 600.0, 868.38),
    (1065., 700.0, 937.64),
    (1065., 800.0, 1012.7),
    (1065., 1000., 1174.9),
    (1065., 1200., 1348.2),
    (1065., 1600., 1712.9),
    (1065., 2000., 2091.0),
    (1095., 296.0, 1165.9),
    (1095., 350.0, 1177.8),
    (1095., 400.0, 1191.4),
    (1095., 450.0, 1207.7),
    (1095., 500.0, 1226.0),
    (1095., 600.0, 1268.7),
    (1095., 800.0, 1373.4),
    (1095., 1000., 1497.7),
    (1096., 296.0, 317.27),
    (1096., 400.0, 416.29),
    (1096., 500.0, 513.22),
    (1096., 600.0, 611.12),
    (1096., 700.0, 709.60),
    (1096., 800.0, 808.43),
    (1096., 1000., 1006.8),
    (1096., 1200., 1205.7),
    (1097., 296.0, 806.79),
    (1097., 400.0, 829.98),
    (1097., 500.0, 868.44),
    (1097., 600.0, 920.08),
    (1097., 700.0, 981.82),
    (1097., 800.0, 1051.1),
    (1097., 1000., 1205.4),
    (1097., 1200., 1373.4),
    (1099., 296.0, 596.4),
    (1099., 400.0, 643.9),
    (1099., 500.0, 704.6),
    (1099., 600.0, 775.3),
    (1099., 800.0, 935.4),
    (1099., 1000., 1109.8),
    (1099., 1200., 1292.3),
    (1114., 296.0, 1222.0),
    (1114., 350.0, 1239.0),
];

/// `gatef2`'s table (second atom of BeO and benzene): `(MAT, T, T_eff)`.
const GATEF2: [(f64, f64, f64); 16] = [
    (1095., 296.0, 685.54),
    (1095., 350., 712.02),
    (1095., 400., 738.97),
    (1095., 450., 768.10),
    (1095., 500., 799.22),
    (1095., 600., 866.63),
    (1095., 800., 1017.3),
    (1095., 1000., 1182.3),
    (1099., 296.0, 427.8),
    (1099., 400., 502.8),
    (1099., 500., 584.3),
    (1099., 600., 671.3),
    (1099., 700., 761.6),
    (1099., 800., 854.2),
    (1099., 1000., 1043.7),
    (1099., 1200., 1236.6),
];

fn lookup(table: &[(f64, f64, f64)], temp: f64, mat: i32) -> f64 {
    let mut eff = 0.0;
    for &(m, t, e) in table {
        // `jmat = nint(tabl(1,j))`; `abs(tabl(2,j) - temp) <= 5`.
        if m.round() as i32 == mat && (t - temp).abs() <= 5.0 {
            eff = e;
        }
    }
    if eff == 0.0 {
        temp
    } else {
        eff
    }
}

/// `gateff` for one temperature: the tabulated `T_eff` of material `mat` when
/// a table temperature lies within 5 K of `temp` (the last such row wins, as
/// in the Fortran loop), else `temp` itself.
pub fn gateff(temp: f64, mat: i32) -> f64 {
    lookup(&GATEFF, temp, mat)
}

/// `gatef2` for one temperature: `T_eff` of the second atom (BeO, benzene).
pub fn gatef2(temp: f64, mat: i32) -> f64 {
    lookup(&GATEF2, temp, mat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_296_k_is_the_general_atomic_value() {
        assert_eq!(gateff(296.0, 1065), 713.39);
        assert_eq!(gateff(298.0, 1065), 713.39); // within 5 K
        assert_eq!(gateff(1600.0, 1065), 1712.9);
    }

    #[test]
    fn an_untabulated_material_or_temperature_keeps_the_physical_temperature() {
        assert_eq!(gateff(296.0, 9999), 296.0);
        assert_eq!(gateff(450.0, 1065), 450.0);
    }

    #[test]
    fn the_second_atom_tables_are_read() {
        assert_eq!(gatef2(296.0, 1099), 427.8);
        assert_eq!(gatef2(1000.0, 1095), 1182.3);
        assert_eq!(gatef2(296.0, 1065), 296.0);
    }
}
