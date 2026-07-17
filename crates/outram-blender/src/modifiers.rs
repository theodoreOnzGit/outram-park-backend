//! Non-destructive **modifier stack** (Blender's `modifiers/intern/MOD_*`).
//!
//! > **Scaffold: honest `TODO` stubs.** The [`Modifier`] enum and the
//! > [`ModifierStack`] that evaluates a list of them are real and compile; each
//! > modifier's geometry is unimplemented and returns
//! > [`ModifierError::NotImplemented`]. This pins the API shape; the algorithms
//! > are separate `op-hzs` workstreams.
//!
//! ## Modifier stack vs. operators
//!
//! A Blender **modifier** is *non-destructive*: it sits in an ordered stack on
//! an object and recomputes derived geometry from the original mesh every time,
//! leaving the base mesh untouched. That is the distinction from
//! [`crate::ops`], whose operators destructively edit a mesh in place. The
//! stack is evaluated top-to-bottom by [`ModifierStack::evaluate`].
//!
//! ## The modifiers (Blender analogue in parentheses)
//!
//! - [`Modifier::Subsurf`] — Catmull-Clark subdivision surface at a view level
//!   (`MOD_subsurf`, backed by OpenSubdiv upstream).
//! - [`Modifier::Mirror`] — mirror across one or more axis planes with weld
//!   (`MOD_mirror`).
//! - [`Modifier::Array`] — repeat the mesh in a regular pattern (`MOD_array`).

use crate::mesh::Mesh;

/// Errors returned while evaluating a [`Modifier`] or a [`ModifierStack`].
#[derive(Debug, thiserror::Error)]
pub enum ModifierError {
    /// The modifier is scaffolded but its algorithm is not implemented yet.
    #[error("modifier not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// Which axis planes a [`Modifier::Mirror`] reflects across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MirrorAxes {
    /// Mirror across the YZ plane (reflect X).
    pub x: bool,
    /// Mirror across the XZ plane (reflect Y).
    pub y: bool,
    /// Mirror across the XY plane (reflect Z).
    pub z: bool,
}

/// A closed set of non-destructive modifiers.
#[derive(Debug, Clone)]
pub enum Modifier {
    /// Catmull-Clark subdivision to `levels` refinement passes.
    Subsurf {
        /// Number of subdivision levels (viewport render level).
        levels: u32,
    },
    /// Mirror the mesh across the selected axis planes and weld the seam.
    Mirror {
        /// Which axis planes to reflect across.
        axes: MirrorAxes,
    },
    /// Repeat the mesh `count` times, each copy offset by `offset` units along
    /// each axis (relative-offset array).
    Array {
        /// Number of copies including the original (`>= 1`).
        count: u32,
        /// Per-copy translation, one mesh-bound-length multiple per axis.
        offset: [f64; 3],
    },
}

impl Modifier {
    /// Evaluate this modifier against `input`, returning the derived mesh.
    ///
    /// **Not yet implemented** — returns [`ModifierError::NotImplemented`]. The
    /// input is borrowed (non-destructive: the base mesh is preserved) and a
    /// new mesh would be returned.
    pub fn evaluate(&self, _input: &Mesh) -> Result<Mesh, ModifierError> {
        match self {
            Modifier::Subsurf { .. } => Err(ModifierError::NotImplemented(
                "subsurf (MOD_subsurf / OpenSubdiv): Catmull-Clark refinement",
            )),
            Modifier::Mirror { .. } => Err(ModifierError::NotImplemented(
                "mirror (MOD_mirror): reflect + weld across axis planes",
            )),
            Modifier::Array { .. } => Err(ModifierError::NotImplemented(
                "array (MOD_array): tile the mesh with a per-copy offset",
            )),
        }
    }
}

/// An ordered, non-destructive stack of [`Modifier`]s applied to a base mesh.
///
/// Mirrors Blender's per-object modifier stack: the base mesh is kept, and
/// [`ModifierStack::evaluate`] folds each modifier in order to produce the
/// final derived mesh.
#[derive(Debug, Clone, Default)]
pub struct ModifierStack {
    /// The modifiers, evaluated first-to-last (top-to-bottom in Blender's UI).
    pub modifiers: Vec<Modifier>,
}

impl ModifierStack {
    /// Create an empty stack (evaluates to the input mesh unchanged).
    pub fn new() -> Self {
        ModifierStack::default()
    }

    /// Append a modifier to the end of the stack (builder style).
    pub fn push(mut self, m: Modifier) -> Self {
        self.modifiers.push(m);
        self
    }

    /// Evaluate the whole stack against `base`.
    ///
    /// An empty stack clones `base` unchanged. Otherwise each [`Modifier`] is
    /// folded in order — currently the first non-empty modifier returns
    /// [`ModifierError::NotImplemented`] (no fake-green).
    pub fn evaluate(&self, base: &Mesh) -> Result<Mesh, ModifierError> {
        let mut current = base.clone();
        for m in &self.modifiers {
            current = m.evaluate(&current)?;
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn empty_stack_is_identity() {
        let base = primitives::cube(1.0);
        let out = ModifierStack::new().evaluate(&base).unwrap();
        assert_eq!(out.vertex_count(), base.vertex_count());
        assert_eq!(out.face_count(), base.face_count());
    }

    #[test]
    fn nonempty_stack_reports_not_implemented() {
        let base = primitives::cube(1.0);
        let stack = ModifierStack::new().push(Modifier::Subsurf { levels: 2 });
        assert!(matches!(
            stack.evaluate(&base),
            Err(ModifierError::NotImplemented(_))
        ));
    }
}
