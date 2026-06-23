/// Row-major n×n dense matrix of `f64`. Maps to `Foam::scalarSquareMatrix`.
///
/// LU decomposition uses Crout's algorithm with scaled partial pivoting,
/// matching `Foam::LUDecompose(scalarSquareMatrix&, labelList&)`.
#[derive(Debug, Clone)]
pub struct SquareMatrix {
    n: usize,
    data: Vec<f64>,
}

impl SquareMatrix {
    pub fn new(n: usize) -> Self {
        Self { n, data: vec![0.0; n * n] }
    }

    pub fn n(&self) -> usize { self.n }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.n + j]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.n + j] = v;
    }

    #[inline]
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.n + j] += v;
    }

    pub fn fill_zero(&mut self) {
        self.data.iter_mut().for_each(|x| *x = 0.0);
    }

    /// In-place LU decomposition with scaled partial pivoting.
    ///
    /// After the call, `self` holds the combined LU factors (lower strictly
    /// triangular, upper including diagonal). Returns the pivot-row indices.
    pub fn lu_decompose(&mut self) -> Vec<usize> {
        let n = self.n;
        let mut pivot = vec![0usize; n];

        // Row scaling factors: 1 / max(|row|)
        let mut vv: Vec<f64> = (0..n)
            .map(|i| {
                let mx = self.data[i * n..(i + 1) * n]
                    .iter()
                    .map(|x| x.abs())
                    .fold(0.0_f64, f64::max);
                if mx > 0.0 { 1.0 / mx } else { 0.0 }
            })
            .collect();

        for j in 0..n {
            // Update upper triangle elements above diagonal (rows i < j)
            for i in 0..j {
                let mut sum = self.data[i * n + j];
                for k in 0..i {
                    sum -= self.data[i * n + k] * self.data[k * n + j];
                }
                self.data[i * n + j] = sum;
            }

            // Update diagonal and lower: find pivot simultaneously
            let mut i_max = j;
            let mut largest = 0.0_f64;
            for i in j..n {
                let mut sum = self.data[i * n + j];
                for k in 0..j {
                    sum -= self.data[i * n + k] * self.data[k * n + j];
                }
                self.data[i * n + j] = sum;
                let tmp = vv[i] * sum.abs();
                if tmp >= largest {
                    largest = tmp;
                    i_max = i;
                }
            }

            // Swap rows j ↔ i_max
            pivot[j] = i_max;
            if j != i_max {
                for k in 0..n {
                    self.data.swap(j * n + k, i_max * n + k);
                }
                vv[i_max] = vv[j];
            }

            // Guard against exact singularity
            if self.data[j * n + j] == 0.0 {
                self.data[j * n + j] = f64::EPSILON;
            }

            // Scale column below diagonal (store L multipliers)
            if j < n - 1 {
                let r = 1.0 / self.data[j * n + j];
                for i in (j + 1)..n {
                    self.data[i * n + j] *= r;
                }
            }
        }

        pivot
    }

    /// Solve `LU·x = b` in-place (`b` is overwritten with the solution).
    ///
    /// Must be called after `lu_decompose`. Matches
    /// `Foam::LUBacksubstitute(scalarSquareMatrix&, labelList&, List<scalar>&)`.
    pub fn lu_back_substitute(&self, pivot: &[usize], b: &mut Vec<f64>) {
        let n = self.n;

        // Forward substitution with pivoting (lazy first-nonzero optimisation)
        let mut first_nz: Option<usize> = None;
        for i in 0..n {
            let ip = pivot[i];
            let mut sum = b[ip];
            b[ip] = b[i];
            if let Some(start) = first_nz {
                for j in start..i {
                    sum -= self.data[i * n + j] * b[j];
                }
            } else if sum != 0.0 {
                first_nz = Some(i);
            }
            b[i] = sum;
        }

        // Back substitution
        for i in (0..n).rev() {
            let mut sum = b[i];
            for j in (i + 1)..n {
                sum -= self.data[i * n + j] * b[j];
            }
            b[i] = sum / self.data[i * n + i];
        }
    }

    /// Convenience: decompose a copy and solve `A·x = b`.
    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        let mut a = self.clone();
        let pivot = a.lu_decompose();
        let mut b = rhs.to_vec();
        a.lu_back_substitute(&pivot, &mut b);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_solve_2x2() {
        // 2x + y = 5,  x + 3y = 10  →  x = 1, y = 3
        let mut m = SquareMatrix::new(2);
        m.set(0, 0, 2.0); m.set(0, 1, 1.0);
        m.set(1, 0, 1.0); m.set(1, 1, 3.0);
        let x = m.solve(&[5.0, 10.0]);
        assert!((x[0] - 1.0).abs() < 1e-12, "x={}", x[0]);
        assert!((x[1] - 3.0).abs() < 1e-12, "y={}", x[1]);
    }

    #[test]
    fn lu_solve_3x3() {
        // A = [2 1 1; 1 3 1; 1 1 4], RHS chosen so solution = (1,2,3)
        // verify: 2+2+3=7, 1+6+3=10, 1+2+12=15
        let mut m = SquareMatrix::new(3);
        m.set(0, 0, 2.0); m.set(0, 1, 1.0); m.set(0, 2, 1.0);
        m.set(1, 0, 1.0); m.set(1, 1, 3.0); m.set(1, 2, 1.0);
        m.set(2, 0, 1.0); m.set(2, 1, 1.0); m.set(2, 2, 4.0);
        let x = m.solve(&[7.0, 10.0, 15.0]);
        assert!((x[0] - 1.0).abs() < 1e-12, "x={}", x[0]);
        assert!((x[1] - 2.0).abs() < 1e-12, "y={}", x[1]);
        assert!((x[2] - 3.0).abs() < 1e-12, "z={}", x[2]);
    }

    #[test]
    fn lu_solve_needs_pivoting() {
        // First row has a zero on the diagonal — requires pivoting
        // 0·x + 1·y = 3,  2·x + 0·y = 4  →  x=2, y=3
        let mut m = SquareMatrix::new(2);
        m.set(0, 0, 0.0); m.set(0, 1, 1.0);
        m.set(1, 0, 2.0); m.set(1, 1, 0.0);
        let x = m.solve(&[3.0, 4.0]);
        assert!((x[0] - 2.0).abs() < 1e-12, "x={}", x[0]);
        assert!((x[1] - 3.0).abs() < 1e-12, "y={}", x[1]);
    }

    #[test]
    fn lu_identity() {
        let mut m = SquareMatrix::new(3);
        m.set(0, 0, 1.0); m.set(1, 1, 1.0); m.set(2, 2, 1.0);
        let x = m.solve(&[7.0, -2.0, 5.0]);
        assert!((x[0] - 7.0).abs() < 1e-12);
        assert!((x[1] + 2.0).abs() < 1e-12);
        assert!((x[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn lu_multiple_rhs() {
        // A = [3 1; 1 2]. Solve two RHS against the same LU factorisation.
        // RHS (5,5) → sol (1,2): 3+2=5 ✓, 1+4=5 ✓
        // RHS (13,11) → sol (3,4): 9+4=13 ✓, 3+8=11 ✓
        let mut m = SquareMatrix::new(2);
        m.set(0, 0, 3.0); m.set(0, 1, 1.0);
        m.set(1, 0, 1.0); m.set(1, 1, 2.0);
        let pivot = m.lu_decompose();

        let mut b1 = vec![5.0, 5.0];
        m.lu_back_substitute(&pivot, &mut b1);
        assert!((b1[0] - 1.0).abs() < 1e-12, "b1[0]={}", b1[0]);
        assert!((b1[1] - 2.0).abs() < 1e-12, "b1[1]={}", b1[1]);

        let mut b2 = vec![13.0, 11.0];
        m.lu_back_substitute(&pivot, &mut b2);
        assert!((b2[0] - 3.0).abs() < 1e-12, "b2[0]={}", b2[0]);
        assert!((b2[1] - 4.0).abs() < 1e-12, "b2[1]={}", b2[1]);
    }
}
