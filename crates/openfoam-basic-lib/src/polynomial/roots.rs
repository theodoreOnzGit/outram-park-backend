/// Root classification tag, matching `Foam::roots::type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum RootType {
    Real    = 0,
    Complex = 1,
    PosInf  = 2,
    NegInf  = 3,
    Nan     = 4,
}

/// Tagged root container for N roots.
/// Types are packed 3 bits per root into a u64, identical to C++ `Roots<N>`.
/// For complex conjugate pairs, slot i holds the real part and slot i+1 holds
/// the imaginary part; both slots are tagged `Complex`.
#[derive(Debug, Clone, Copy)]
pub struct Roots<const N: usize> {
    pub(crate) values: [f64; N],
    types: u64,
}

impl<const N: usize> Roots<N> {
    /// Value stored at slot `i`.
    #[inline]
    pub fn get(&self, i: usize) -> f64 {
        self.values[i]
    }

    /// Root type at slot `i`.
    #[inline]
    pub fn root_type(&self, i: usize) -> RootType {
        match (self.types >> (3 * i)) & 7 {
            0 => RootType::Real,
            1 => RootType::Complex,
            2 => RootType::PosInf,
            3 => RootType::NegInf,
            _ => RootType::Nan,
        }
    }

    /// Overwrite the type at slot `i`.
    #[inline]
    pub fn set_type(&mut self, i: usize, t: RootType) {
        let shift = 3 * i;
        self.types = (self.types & !(7u64 << shift)) | ((t as u64) << shift);
    }
}

impl<const N: usize> std::ops::Index<usize> for Roots<N> {
    type Output = f64;
    #[inline]
    fn index(&self, i: usize) -> &f64 {
        &self.values[i]
    }
}

// ---------------------------------------------------------------------------
// Constructors for each concrete arity.
// The C++ template constructors (concatenation by prepend/append) are exposed
// as named static methods on the concrete impl blocks.
// ---------------------------------------------------------------------------

impl Roots<1> {
    /// Single root with the given type and value.
    #[inline]
    pub fn new(t: RootType, x: f64) -> Self {
        Self { values: [x], types: t as u64 }
    }
}

impl Roots<2> {
    /// Concatenate two single roots.  C++ `Roots<2>(Roots<1>, Roots<1>)`.
    #[inline]
    pub fn from_pair(a: Roots<1>, b: Roots<1>) -> Self {
        Self {
            values: [a.values[0], b.values[0]],
            types: a.types | (b.types << 3),
        }
    }

    /// `Roots<1>` followed by one additional root.
    /// C++ `Roots<2>(Roots<1>, type, x)`.
    #[inline]
    pub fn with_tail(head: Roots<1>, t: RootType, x: f64) -> Self {
        Self {
            values: [head.values[0], x],
            types: head.types | ((t as u64) << 3),
        }
    }

    /// Duplicate a single root into both slots.
    /// C++ `Roots<2>(r, r)`.
    #[inline]
    pub fn both(r: Roots<1>) -> Self {
        Self::from_pair(r, r)
    }
}

impl Roots<3> {
    /// All three slots get the same type and value.
    /// C++ `Roots<3>(type, x)`.
    #[inline]
    pub fn uniform(t: RootType, x: f64) -> Self {
        let ti = t as u64;
        Self {
            values: [x, x, x],
            types: ti | (ti << 3) | (ti << 6),
        }
    }

    /// Concatenate `Roots<1>` then `Roots<2>`.
    /// C++ `Roots<3>(Roots<1>, Roots<2>)`.
    #[inline]
    pub fn concat_1_2(a: Roots<1>, b: Roots<2>) -> Self {
        Self {
            values: [a.values[0], b.values[0], b.values[1]],
            types: a.types | (b.types << 3),
        }
    }

    /// Concatenate `Roots<2>` then `Roots<1>`.
    /// C++ `Roots<3>(Roots<2>, Roots<1>)`.
    #[inline]
    pub fn concat_2_1(a: Roots<2>, b: Roots<1>) -> Self {
        Self {
            values: [a.values[0], a.values[1], b.values[0]],
            types: a.types | (b.types << 6),
        }
    }

    /// `Roots<2>` followed by one additional root.
    /// C++ `Roots<3>(Roots<2>, type, x)`.
    #[inline]
    pub fn with_tail(head: Roots<2>, t: RootType, x: f64) -> Self {
        Self::concat_2_1(head, Roots::<1>::new(t, x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trip() {
        let r = Roots::<1>::new(RootType::Real, 3.14);
        assert_eq!(r.root_type(0), RootType::Real);
        assert_eq!(r.get(0), 3.14);
    }

    #[test]
    fn concat_2() {
        let a = Roots::<1>::new(RootType::Real, 1.0);
        let b = Roots::<1>::new(RootType::Nan, 0.0);
        let r = Roots::<2>::from_pair(a, b);
        assert_eq!(r.root_type(0), RootType::Real);
        assert_eq!(r.root_type(1), RootType::Nan);
        assert_eq!(r[0], 1.0);
        assert_eq!(r[1], 0.0);
    }

    #[test]
    fn concat_3() {
        let a = Roots::<1>::new(RootType::Real, 1.0);
        let b = Roots::<2>::from_pair(
            Roots::<1>::new(RootType::Real, 2.0),
            Roots::<1>::new(RootType::Nan, 0.0),
        );
        let r = Roots::<3>::concat_1_2(a, b);
        assert_eq!(r.root_type(0), RootType::Real);
        assert_eq!(r.root_type(1), RootType::Real);
        assert_eq!(r.root_type(2), RootType::Nan);
        assert_eq!(r[0], 1.0);
        assert_eq!(r[1], 2.0);
    }

    #[test]
    fn uniform_3() {
        let r = Roots::<3>::uniform(RootType::Complex, -5.0);
        for i in 0..3 {
            assert_eq!(r.root_type(i), RootType::Complex);
            assert_eq!(r[i], -5.0);
        }
    }
}
