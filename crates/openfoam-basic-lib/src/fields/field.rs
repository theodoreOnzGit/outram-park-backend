use std::ops::{Add, Sub, Mul, Div, Neg, AddAssign, SubAssign};

use crate::primitives::{Vector3, Tensor, SymmTensor, SphericalTensor};

/// A flat array over all cells or faces, with element-wise arithmetic.
///
/// Mirrors `Foam::Field<Type>` from `src/OpenFOAM/fields/Fields/Field/Field.H`.
/// The raw storage is `Vec<T>` with no dimension bookkeeping — that lives in
/// the wrapping `VolField`/`SurfaceField`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field<T> {
    data: Vec<T>,
}

// ── Construction ─────────────────────────────────────────────────────────────

impl<T: Clone> Field<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self { data }
    }

    pub fn uniform(n: usize, value: T) -> Self {
        Self { data: vec![value; n] }
    }

    pub fn from_fn(n: usize, f: impl Fn(usize) -> T) -> Self {
        Self { data: (0..n).map(f).collect() }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn into_vec(self) -> Vec<T> {
        self.data
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    pub fn map<U: Clone>(&self, f: impl Fn(&T) -> U) -> Field<U> {
        Field { data: self.data.iter().map(f).collect() }
    }
}

impl Field<f64> {
    pub fn zeros(n: usize) -> Self {
        Self::uniform(n, 0.0)
    }

    pub fn ones(n: usize) -> Self {
        Self::uniform(n, 1.0)
    }

    pub fn sum(&self) -> f64 {
        self.data.iter().copied().sum()
    }

    pub fn mean(&self) -> f64 {
        if self.data.is_empty() { 0.0 } else { self.sum() / self.data.len() as f64 }
    }

    pub fn min(&self) -> f64 {
        self.data.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn l2_norm(&self) -> f64 {
        self.data.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Element-wise absolute value.
    pub fn abs(&self) -> Self {
        self.map(|x| x.abs())
    }

    /// Element-wise clamp.
    pub fn clamp(&self, lo: f64, hi: f64) -> Self {
        self.map(|x| x.clamp(lo, hi))
    }

    /// Element-wise product of two scalar fields.
    pub fn pointwise_mul(&self, rhs: &Self) -> Self {
        assert_eq!(self.len(), rhs.len(), "Field length mismatch in pointwise_mul");
        Field::from_fn(self.len(), |i| self.data[i] * rhs.data[i])
    }

    /// Element-wise division of two scalar fields.
    pub fn pointwise_div(&self, rhs: &Self) -> Self {
        assert_eq!(self.len(), rhs.len(), "Field length mismatch in pointwise_div");
        Field::from_fn(self.len(), |i| self.data[i] / rhs.data[i])
    }

    /// Weighted sum: sum(w[i] * x[i]).
    pub fn weighted_sum(&self, weights: &Field<f64>) -> f64 {
        assert_eq!(self.len(), weights.len());
        self.data.iter().zip(weights.data.iter()).map(|(x, w)| x * w).sum()
    }
}

impl Field<Vector3> {
    pub fn zero_vec(n: usize) -> Self {
        Self::uniform(n, Vector3::ZERO)
    }

    /// Element-wise dot product → scalar field.
    pub fn dot_field(&self, rhs: &Field<Vector3>) -> Field<f64> {
        assert_eq!(self.len(), rhs.len());
        Field::from_fn(self.len(), |i| self.data[i].dot(rhs.data[i]))
    }

    /// Scale each element by the corresponding scalar field entry.
    pub fn scale(&self, s: &Field<f64>) -> Self {
        assert_eq!(self.len(), s.len());
        Field::from_fn(self.len(), |i| self.data[i] * s[i])
    }
}

// ── Index ─────────────────────────────────────────────────────────────────────

impl<T> std::ops::Index<usize> for Field<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.data[i]
    }
}

impl<T> std::ops::IndexMut<usize> for Field<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.data[i]
    }
}

impl<T> AsRef<[T]> for Field<T> {
    fn as_ref(&self) -> &[T] { &self.data }
}

impl<T> IntoIterator for Field<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Field<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<T: Clone> From<Vec<T>> for Field<T> {
    fn from(v: Vec<T>) -> Self { Self::new(v) }
}

// ── Arithmetic — Field<T> op Field<T> ────────────────────────────────────────

impl<T: Add<Output=T> + Clone> Add for Field<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        assert_eq!(self.len(), rhs.len(), "Field length mismatch in add");
        Field::from_fn(self.len(), |i| self.data[i].clone() + rhs.data[i].clone())
    }
}

impl<T: Sub<Output=T> + Clone> Sub for Field<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        assert_eq!(self.len(), rhs.len(), "Field length mismatch in sub");
        Field::from_fn(self.len(), |i| self.data[i].clone() - rhs.data[i].clone())
    }
}

impl<T: Neg<Output=T> + Clone> Neg for Field<T> {
    type Output = Self;
    fn neg(self) -> Self {
        self.map(|x| -x.clone())
    }
}

impl<T: Add<Output=T> + Clone> AddAssign for Field<T> {
    fn add_assign(&mut self, rhs: Self) {
        assert_eq!(self.len(), rhs.len());
        for (a, b) in self.data.iter_mut().zip(rhs.data) {
            *a = a.clone() + b;
        }
    }
}

impl<T: Sub<Output=T> + Clone> SubAssign for Field<T> {
    fn sub_assign(&mut self, rhs: Self) {
        assert_eq!(self.len(), rhs.len());
        for (a, b) in self.data.iter_mut().zip(rhs.data) {
            *a = a.clone() - b;
        }
    }
}

// ── Arithmetic — Field<T> op f64 ─────────────────────────────────────────────

impl<T: Mul<f64, Output=T> + Clone> Mul<f64> for Field<T> {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        self.map(|x| x.clone() * rhs)
    }
}

// f64 * Field<T>  (commutativity helper)
impl Mul<Field<f64>> for f64 {
    type Output = Field<f64>;
    fn mul(self, rhs: Field<f64>) -> Field<f64> {
        rhs.map(|x| self * x)
    }
}

impl Mul<Field<Vector3>> for f64 {
    type Output = Field<Vector3>;
    fn mul(self, rhs: Field<Vector3>) -> Field<Vector3> {
        rhs.map(|v| v.clone() * self)
    }
}

impl<T: Mul<f64, Output=T> + Clone> Div<f64> for Field<T> {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        let inv = 1.0 / rhs;
        self.map(|x| x.clone() * inv)
    }
}

// ── Field<f64> specific binary ops ───────────────────────────────────────────

impl Mul for Field<f64> {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.pointwise_mul(&rhs)
    }
}

impl Div for Field<f64> {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        self.pointwise_div(&rhs)
    }
}

// Field<Vector3> scaled by Field<f64>
impl Mul<Field<f64>> for Field<Vector3> {
    type Output = Field<Vector3>;
    fn mul(self, rhs: Field<f64>) -> Field<Vector3> {
        self.scale(&rhs)
    }
}

impl Mul<Field<Vector3>> for Field<f64> {
    type Output = Field<Vector3>;
    fn mul(self, rhs: Field<Vector3>) -> Field<Vector3> {
        rhs.scale(&self)
    }
}

// ── Default ───────────────────────────────────────────────────────────────────

impl Default for Field<f64> {
    fn default() -> Self { Self::new(vec![]) }
}

impl Default for Field<Vector3> {
    fn default() -> Self { Self::new(vec![]) }
}

impl Default for Field<Tensor> {
    fn default() -> Self { Self::new(vec![]) }
}

impl Default for Field<SymmTensor> {
    fn default() -> Self { Self::new(vec![]) }
}

impl Default for Field<SphericalTensor> {
    fn default() -> Self { Self::new(vec![]) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_f64_fields() {
        let a = Field::new(vec![1.0, 2.0, 3.0]);
        let b = Field::new(vec![4.0, 5.0, 6.0]);
        let c = a + b;
        assert_eq!(c.as_slice(), &[5.0, 7.0, 9.0]);
    }

    #[test]
    fn mul_scalar() {
        let a = Field::new(vec![1.0, 2.0, 3.0]);
        let b = a * 2.0;
        assert_eq!(b.as_slice(), &[2.0, 4.0, 6.0]);
    }

    #[test]
    fn div_scalar() {
        let a = Field::new(vec![2.0, 4.0, 6.0]);
        let b = a / 2.0;
        assert_eq!(b.as_slice(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn neg_field() {
        let a = Field::new(vec![1.0, -2.0, 3.0]);
        let b = -a;
        assert_eq!(b.as_slice(), &[-1.0, 2.0, -3.0]);
    }

    #[test]
    fn sum_mean() {
        let a = Field::new(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(a.sum(), 10.0);
        assert_eq!(a.mean(), 2.5);
    }

    #[test]
    fn pointwise_mul_fields() {
        let a = Field::new(vec![2.0, 3.0]);
        let b = Field::new(vec![4.0, 5.0]);
        let c = a.pointwise_mul(&b);
        assert_eq!(c.as_slice(), &[8.0, 15.0]);
    }

    #[test]
    fn vector3_add_fields() {
        let a = Field::new(vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)]);
        let b = Field::new(vec![Vector3::new(0.0, 0.0, 1.0), Vector3::new(1.0, 0.0, 0.0)]);
        let c = a + b;
        assert_eq!(c[0], Vector3::new(1.0, 0.0, 1.0));
        assert_eq!(c[1], Vector3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn vector3_scale_by_scalar_field() {
        let v = Field::new(vec![Vector3::new(1.0, 2.0, 3.0), Vector3::new(4.0, 5.0, 6.0)]);
        let s = Field::new(vec![2.0, 0.5]);
        let result = v.scale(&s);
        assert_eq!(result[0], Vector3::new(2.0, 4.0, 6.0));
        assert_eq!(result[1], Vector3::new(2.0, 2.5, 3.0));
    }

    #[test]
    fn uniform_and_len() {
        let f: Field<f64> = Field::uniform(5, 3.14);
        assert_eq!(f.len(), 5);
        assert!((f[0] - 3.14).abs() < 1e-15);
    }
}
