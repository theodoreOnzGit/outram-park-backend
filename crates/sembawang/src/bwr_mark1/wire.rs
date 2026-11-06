//! The flat `f64` wire format snapshots and results cross worker boundaries
//! in (a `Float64Array` in the browser). Every value crosses as its exact
//! `f64` bits, so a step computed on another worker is bit-for-bit the step
//! computed in place.

#[derive(Default)]
pub struct Writer(pub Vec<f64>);

impl Writer {
    pub fn f(&mut self, v: &[f64]) {
        self.0.extend_from_slice(v);
    }
    /// A length-prefixed vector.
    pub fn vec(&mut self, v: &[f64]) {
        self.0.push(v.len() as f64);
        self.0.extend_from_slice(v);
    }
}

pub struct Reader<'a> {
    v: &'a [f64],
    i: usize,
}

impl<'a> Reader<'a> {
    pub fn new(v: &'a [f64]) -> Self {
        Self { v, i: 0 }
    }
    /// The next number (NaN past the end: a short message reads as NaN
    /// rather than panicking a worker).
    pub fn f(&mut self) -> f64 {
        let x = self.v.get(self.i).copied().unwrap_or(f64::NAN);
        self.i += 1;
        x
    }
    pub fn vec(&mut self) -> Vec<f64> {
        let n = self.f();
        let n = if n.is_finite() && n >= 0.0 { n as usize } else { 0 };
        let end = (self.i + n).min(self.v.len());
        let out = self.v[self.i.min(end)..end].to_vec();
        self.i += n;
        out
    }
}
