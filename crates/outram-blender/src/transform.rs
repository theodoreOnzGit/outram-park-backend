//! Affine transforms over mesh vertices — the **CPU reference path**.
//!
//! An [`Affine3`] is a `3x3` linear map plus a translation, i.e. the standard
//! rigid/affine transform used to place, rotate, and scale mesh geometry
//! (Blender's `Object.matrix_world` is exactly this class of operation). It is
//! the "hello world" of an *embarrassingly parallel* mesh kernel: every vertex
//! is transformed independently, with no cross-vertex dependency, so the same
//! math maps cleanly onto a GPU compute shader.
//!
//! ## Why this lives in the always-compiled part of the crate
//!
//! This module is compiled in **every** build, `gpu` feature or not. It is the
//! **trusted, deterministic reference**: the GPU path in [`crate::gpu`] exists
//! only to *accelerate* the exact same computation, and its result is checked
//! against [`Affine3::transform_points`] here. Per the workspace design rules,
//! the CPU path is what V&V and downstream solvers rely on; GPU output (f32,
//! non-deterministic reduction order across hardware) is never the source of
//! truth.
//!
//! Precision: this CPU path is `f64`. The GPU compute shader is `f32` (the
//! portable storage type for WGSL). Agreement between the two is therefore an
//! *approximate* match within an `f32`-scale tolerance, not bit-exact — see the
//! GPU test in [`crate::gpu`].

use crate::math::Vec3;

/// A 3D affine transform: a `3x3` linear map `M` followed by a translation `t`,
/// acting on a point `p` as `M p + t`.
///
/// The linear part [`Affine3::linear`] is stored **row-major** as three rows of
/// three `f64` components; `linear[i]` is row `i`, so the transformed
/// coordinate `i` is `dot(linear[i], p) + translation[i]`. All components are
/// dimensionless model-space quantities (see [`crate::math`]).
///
/// `Affine3` is `Copy` and holds no borrows, in line with the workspace
/// no-lifetimes rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine3 {
    /// Row-major `3x3` linear part. `linear[i]` is the `i`-th output row; the
    /// `i`-th transformed coordinate is `linear[i] . p + translation[i]`.
    pub linear: [[f64; 3]; 3],
    /// Translation added after the linear map.
    pub translation: Vec3,
}

impl Affine3 {
    /// The identity transform: maps every point to itself (`M = I`, `t = 0`).
    pub const IDENTITY: Affine3 = Affine3 {
        linear: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: Vec3::ZERO,
    };

    /// A pure translation by `t` (identity linear part).
    pub const fn translation(t: Vec3) -> Affine3 {
        Affine3 { linear: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], translation: t }
    }

    /// A non-uniform scale about the origin by `(sx, sy, sz)`, no translation.
    pub const fn scale(sx: f64, sy: f64, sz: f64) -> Affine3 {
        Affine3 {
            linear: [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, sz]],
            translation: Vec3::ZERO,
        }
    }

    /// Build from an explicit row-major `3x3` linear part and a translation.
    pub const fn from_rows(linear: [[f64; 3]; 3], translation: Vec3) -> Affine3 {
        Affine3 { linear, translation }
    }

    /// Transform a single point `p` by this affine map, returning `M p + t`.
    ///
    /// This is the elementwise kernel; [`Affine3::transform_points`] is just
    /// this applied to a slice, and the GPU compute shader is a WGSL
    /// transcription of this same arithmetic.
    pub fn transform_point(self, p: Vec3) -> Vec3 {
        let m = self.linear;
        Vec3::new(
            m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + self.translation.x,
            m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + self.translation.y,
            m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + self.translation.z,
        )
    }

    /// Transform a whole slice of vertex positions on the CPU (the reference
    /// implementation). Returns a new `Vec` in the same order as the input.
    ///
    /// This is the trusted path: it is always available (no cargo feature), it
    /// is `f64`, and it is what the GPU result in [`crate::gpu`] is validated
    /// against.
    pub fn transform_points(self, positions: &[Vec3]) -> Vec<Vec3> {
        positions.iter().map(|&p| self.transform_point(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_a_noop() {
        let p = Vec3::new(1.0, -2.0, 3.5);
        assert_eq!(Affine3::IDENTITY.transform_point(p), p);
    }

    #[test]
    fn translation_then_scale_compose_by_hand() {
        // Scale by (2,3,4) about origin, then the translation is added after.
        let t = Affine3::from_rows(
            [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]],
            Vec3::new(10.0, 20.0, 30.0),
        );
        let out = t.transform_point(Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(out, Vec3::new(12.0, 23.0, 34.0));
    }

    #[test]
    fn batch_matches_pointwise() {
        let t = Affine3::from_rows(
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], // 90 deg about z
            Vec3::new(5.0, 0.0, -1.0),
        );
        let pts = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(2.0, 3.0, 4.0),
        ];
        let batch = t.transform_points(&pts);
        for (i, &p) in pts.iter().enumerate() {
            assert_eq!(batch[i], t.transform_point(p));
        }
        // Spot-check the rotation: (1,0,0) -> (0,1,0), then +translation.
        assert_eq!(batch[0], Vec3::new(5.0, 1.0, -1.0));
    }
}
