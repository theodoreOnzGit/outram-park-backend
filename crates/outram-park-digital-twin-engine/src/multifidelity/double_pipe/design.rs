//! The design grid of gh:#1178 and its held-out split, **fixed before any
//! run** (V&V record, "Design"). Held-out points are never used to build a
//! closure; [`crate::multifidelity::double_pipe::closure::ClosureTable::build`]
//! refuses them.
use super::DoublePipeInput;

/// Whether a point may build closures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    /// A node of the closure table.
    Training,
    /// A check: never used to build a closure.
    HeldOut,
}

impl Split {
    /// `"training"` / `"held_out"` (the dataset's spelling).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Split::Training => "training",
            Split::HeldOut => "held_out",
        }
    }

    /// Inverse of [`Self::as_str`].
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "training" => Some(Split::Training),
            "held_out" => Some(Split::HeldOut),
            _ => None,
        }
    }
}

/// One design point.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignPoint {
    /// `T01`…`T36`, `H1`…`H8`.
    pub id: String,
    /// Training or held-out.
    pub split: Split,
    /// x.
    pub input: DoublePipeInput,
}

/// Training levels: Re_hot.
pub const RE_HOT_LEVELS: [f64; 3] = [10.0, 30.0, 90.0];
/// Training levels: Re_cold.
pub const RE_COLD_LEVELS: [f64; 3] = [5.0, 15.0, 45.0];
/// Training levels: k_wall \[W/(m K)\].
pub const K_WALL_LEVELS: [f64; 2] = [1.0, 100.0];
/// Training levels: L \[m\].
pub const LENGTH_LEVELS: [f64; 2] = [0.1, 0.3];

/// The held-out points, `(id, Re_hot, Re_cold, k_wall, L)`: geometric
/// midpoints (√300, √2700, √75, √675) and two edge cases (V&V record).
#[must_use]
pub fn held_out() -> Vec<(&'static str, [f64; 4])> {
    let (a, b) = (300.0_f64.sqrt(), 2700.0_f64.sqrt());
    let (c, d) = (75.0_f64.sqrt(), 675.0_f64.sqrt());
    vec![
        ("H1", [a, c, 10.0, 0.20]),
        ("H2", [a, d, 10.0, 0.20]),
        ("H3", [b, c, 10.0, 0.20]),
        ("H4", [b, d, 10.0, 0.20]),
        ("H5", [a, d, 1.0, 0.15]),
        ("H6", [b, c, 100.0, 0.25]),
        ("H7", [30.0, 15.0, 10.0, 0.20]),
        ("H8", [90.0, 45.0, 10.0, 0.15]),
    ]
}

/// The whole design: the 36 training points (Re_hot slowest, L fastest),
/// then the 8 held-out points.
#[must_use]
pub fn design_grid() -> Vec<DesignPoint> {
    let mut out = Vec::new();
    let mut i = 0;
    for rh in RE_HOT_LEVELS {
        for rc in RE_COLD_LEVELS {
            for k in K_WALL_LEVELS {
                for l in LENGTH_LEVELS {
                    i += 1;
                    out.push(DesignPoint { id: format!("T{i:02}"), split: Split::Training, input: DoublePipeInput::si(rh, rc, k, l) });
                }
            }
        }
    }
    for (id, [rh, rc, k, l]) in held_out() {
        out.push(DesignPoint { id: id.to_string(), split: Split::HeldOut, input: DoublePipeInput::si(rh, rc, k, l) });
    }
    out
}

/// The mesh-check point (V&V record, "Mesh"): the most demanding corner.
#[must_use]
pub fn mesh_check_point() -> DoublePipeInput {
    DoublePipeInput::si(90.0, 45.0, 100.0, 0.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_is_44_points_with_8_held_out_inside_the_training_hull() {
        let g = design_grid();
        assert_eq!(g.len(), 44);
        let held: Vec<_> = g.iter().filter(|p| p.split == Split::HeldOut).collect();
        assert_eq!(held.len(), 8);
        let mut ids: Vec<&str> = g.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 44, "ids unique");
        for p in &held {
            let [rh, rc, k, l] = p.input.to_si();
            assert!((10.0..=90.0).contains(&rh) && (5.0..=45.0).contains(&rc) && (1.0..=100.0).contains(&k) && (0.1..=0.3).contains(&l), "{p:?}");
            // Not a training node.
            assert!(!g.iter().any(|t| t.split == Split::Training && t.input == p.input));
        }
        assert_eq!(Split::parse(Split::HeldOut.as_str()), Some(Split::HeldOut));
        assert_eq!(Split::parse(Split::Training.as_str()), Some(Split::Training));
        assert_eq!(Split::parse("x"), None);
        assert_eq!(mesh_check_point().to_si(), [90.0, 45.0, 100.0, 0.1]);
    }
}
