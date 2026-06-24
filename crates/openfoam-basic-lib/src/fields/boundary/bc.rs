use crate::fields::field::Field;
use crate::primitives::Vector3;

/// Boundary condition variant for a single patch.
///
/// Covers the BC types required by the target solvers.  More exotic types
/// (inlet-outlet, total pressure, etc.) will be added when Layer 3 is
/// implemented.
#[derive(Debug, Clone)]
pub enum BoundaryCondition<T: Clone> {
    /// Dirichlet: fixed uniform value.
    FixedValue(T),
    /// Dirichlet: fixed per-face values.
    FixedField(Field<T>),
    /// Neumann: zero normal gradient — boundary face value = internal adjacent value.
    ZeroGradient,
    /// Symmetry plane — normal component zeroed.
    Symmetry,
    /// 2-D / wedge — zero-area faces; value has no physical meaning.
    Empty,
    /// Value computed by the solver and stored here (read-only from BC side).
    Calculated(Field<T>),
}

impl<T: Clone + Default> BoundaryCondition<T> {
    /// True if the BC imposes a value (Dirichlet-like).
    pub fn is_fixed_value(&self) -> bool {
        matches!(self, Self::FixedValue(_) | Self::FixedField(_))
    }
}

/// Boundary field for one patch: the BC type plus the current face values.
///
/// The `values` field always holds the latest face values (updated by
/// `update_coeffs` in Layer 3 operators).  For `FixedValue`/`FixedField` the
/// values are set at construction and never change.  For `ZeroGradient` and
/// `Calculated` they are written by the operator code.
#[derive(Debug, Clone)]
pub struct PatchField<T: Clone> {
    pub bc: BoundaryCondition<T>,
    /// Current face values for this patch (length == patch.size).
    pub values: Field<T>,
}

impl PatchField<f64> {
    pub fn fixed_value(size: usize, v: f64) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    pub fn zero_gradient(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zeros(size),
        }
    }

    pub fn empty() -> Self {
        Self { bc: BoundaryCondition::Empty, values: Field::new(vec![]) }
    }
}

impl PatchField<Vector3> {
    pub fn fixed_value_vec(size: usize, v: Vector3) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    pub fn zero_gradient_vec(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zero_vec(size),
        }
    }

    pub fn empty_vec() -> Self {
        Self { bc: BoundaryCondition::Empty, values: Field::new(vec![]) }
    }
}
