//! MATLAB container semantics — the one module with no `.m` counterpart.
//!
//! # Why this module exists
//!
//! MATLAB's array language supplies things Rust does not: multi-dimensional
//! column-major arrays, a sparse triplet form that sums duplicates, `find`,
//! `nnz`. The reference leans on all of them. They live here so the translated
//! modules read as physics rather than as plumbing.
//!
//! # Indexing — 0-based
//!
//! **These containers are 0-based**, and the translated index formulas are
//! converted from the reference's 1-based arithmetic accordingly. The usual
//! shape of the change is that `(ix-1)*stride` becomes `ix*stride` and a
//! trailing `+ iz` becomes `+ iz` with `iz` already 0-based, so
//!
//! ```text
//! idx = (g-1)*es + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz     % MATLAB, 1-based
//! ```
//!
//! becomes
//!
//! ```text
//! idx =  g*es    +  ix*maxiy*maxiz    +  iy*maxiz    + iz     // Rust, 0-based
//! ```
//!
//! Storage stays **column-major**, matching MATLAB, because the reference does
//! linear indexing into multi-dimensional arrays in places and the layout is
//! therefore observable.
//!
//! # The sentinel that does not survive the conversion
//!
//! One thing is *not* a mechanical reindex, and it is worth knowing about
//! before reading `convert_grid3d`. The reference uses **`key(idx) == 0` to
//! mean "this node carries no material"**, which works only because `0` is not
//! a valid 1-based index. Going 0-based makes `0` a perfectly good index and
//! destroys the sentinel.
//!
//! Rather than invent a different magic number, the translation carries that
//! map as `Option<usize>` — `None` for absent, `Some(i)` for the compacted
//! index. It is the same information with the ambiguity removed, and the two
//! call sites that tested `== 0` test `is_none()` instead.
//!
//! # What belongs here
//!
//! Only container semantics and MATLAB built-ins. Physics belongs in the module
//! named after the `.m` file it came from.

/// Column-major 2-D array — the translation's stand-in for a MATLAB matrix.
///
/// Indices are **0-based**. Element `(i, j)` lives at linear offset
/// `j * rows + i`, which is MATLAB's layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Array2<T> {
    rows: usize,
    cols: usize,
    data: Vec<T>,
}

/// An empty 0-by-0 array, so structs holding one can derive `Default`.
///
/// Written by hand rather than derived so it does not require `T: Default`.
impl<T> Default for Array2<T> {
    fn default() -> Self {
        Self {
            rows: 0,
            cols: 0,
            data: Vec::new(),
        }
    }
}

impl<T: Copy + Default> Array2<T> {
    /// `zeros(rows, cols)`.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![T::default(); rows * cols],
        }
    }

    /// `size(a, 1)`.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// `size(a, 2)`.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Element `(i, j)`, 0-based.
    ///
    /// # Panics
    /// If either index is past the end.
    pub fn get(&self, i: usize, j: usize) -> T {
        self.data[self.linear(i, j)]
    }

    /// Set element `(i, j)`, 0-based.
    ///
    /// # Panics
    /// As [`Array2::get`]. Does not grow the array the way MATLAB would; the
    /// reference never relies on that.
    pub fn set(&mut self, i: usize, j: usize, value: T) {
        let k = self.linear(i, j);
        self.data[k] = value;
    }

    /// The backing store, column-major, for bulk operations.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// Read by **linear** index in column-major order — MATLAB's `a(k)` on a
    /// 2-D array, 0-based.
    ///
    /// For `k < rows` this lands on element `(k, 0)`, i.e. the first column.
    /// That is not a general-purpose accessor; it exists because
    /// [`crate::makesigmadfxyz`] reproduces a place where the reference indexes
    /// a 2-D array linearly, and doing so silently reads the first column. See
    /// that module for why the behaviour is preserved rather than corrected.
    ///
    /// # Panics
    /// If `k` is past the end of the backing store.
    pub fn get_linear_column_major(&self, k: usize) -> T {
        assert!(
            k < self.data.len(),
            "linear index {k} out of range 0..{}",
            self.data.len()
        );
        self.data[k]
    }

    fn linear(&self, i: usize, j: usize) -> usize {
        assert!(i < self.rows, "row index {i} out of range 0..{}", self.rows);
        assert!(
            j < self.cols,
            "column index {j} out of range 0..{}",
            self.cols
        );
        j * self.rows + i
    }
}

/// Column-major 3-D array — the `(ix, iy, iz)` grids the reference uses for
/// `whichsigma`, diffusion coefficients and the like. 0-based.
#[derive(Clone, Debug, PartialEq)]
pub struct Array3<T> {
    rows: usize,
    cols: usize,
    pages: usize,
    data: Vec<T>,
}

/// An empty 0-by-0-by-0 array; see [`Array2`]'s `Default`.
impl<T> Default for Array3<T> {
    fn default() -> Self {
        Self {
            rows: 0,
            cols: 0,
            pages: 0,
            data: Vec::new(),
        }
    }
}

impl<T: Copy + Default> Array3<T> {
    /// `zeros(rows, cols, pages)`.
    pub fn zeros(rows: usize, cols: usize, pages: usize) -> Self {
        Self {
            rows,
            cols,
            pages,
            data: vec![T::default(); rows * cols * pages],
        }
    }

    /// `size(a, 1)`.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// `size(a, 2)`.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// `size(a, 3)`.
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Element `(i, j, k)`, 0-based.
    pub fn get(&self, i: usize, j: usize, k: usize) -> T {
        self.data[self.linear(i, j, k)]
    }

    /// The backing store, column-major, for bulk operations such as a whole-
    /// array `NaN` sweep.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// Set element `(i, j, k)`, 0-based.
    pub fn set(&mut self, i: usize, j: usize, k: usize, value: T) {
        let n = self.linear(i, j, k);
        self.data[n] = value;
    }

    fn linear(&self, i: usize, j: usize, k: usize) -> usize {
        assert!(
            i < self.rows,
            "dim-1 index {i} out of range 0..{}",
            self.rows
        );
        assert!(
            j < self.cols,
            "dim-2 index {j} out of range 0..{}",
            self.cols
        );
        assert!(
            k < self.pages,
            "dim-3 index {k} out of range 0..{}",
            self.pages
        );
        k * self.rows * self.cols + j * self.rows + i
    }
}

/// Column-major 4-D array — the `(ix, iy, iz, g)` group-wise quantities such
/// as `diffvalues`. 0-based.
#[derive(Clone, Debug, PartialEq)]
pub struct Array4<T> {
    d1: usize,
    d2: usize,
    d3: usize,
    d4: usize,
    data: Vec<T>,
}

/// An empty array; see [`Array2`]'s `Default`.
impl<T> Default for Array4<T> {
    fn default() -> Self {
        Self {
            d1: 0,
            d2: 0,
            d3: 0,
            d4: 0,
            data: Vec::new(),
        }
    }
}

impl<T: Copy + Default> Array4<T> {
    /// `zeros(d1, d2, d3, d4)`.
    pub fn zeros(d1: usize, d2: usize, d3: usize, d4: usize) -> Self {
        Self {
            d1,
            d2,
            d3,
            d4,
            data: vec![T::default(); d1 * d2 * d3 * d4],
        }
    }

    /// Element `(i, j, k, l)`, 0-based.
    pub fn get(&self, i: usize, j: usize, k: usize, l: usize) -> T {
        self.data[self.linear(i, j, k, l)]
    }

    /// Set element `(i, j, k, l)`, 0-based.
    pub fn set(&mut self, i: usize, j: usize, k: usize, l: usize, value: T) {
        let n = self.linear(i, j, k, l);
        self.data[n] = value;
    }

    /// `size(a, 4)` — the group count, in every current use.
    pub fn groups(&self) -> usize {
        self.d4
    }

    fn linear(&self, i: usize, j: usize, k: usize, l: usize) -> usize {
        assert!(i < self.d1, "dim-1 index {i} out of range 0..{}", self.d1);
        assert!(j < self.d2, "dim-2 index {j} out of range 0..{}", self.d2);
        assert!(k < self.d3, "dim-3 index {k} out of range 0..{}", self.d3);
        assert!(l < self.d4, "dim-4 index {l} out of range 0..{}", self.d4);
        (l * self.d3 * self.d2 * self.d1) + (k * self.d2 * self.d1) + (j * self.d1) + i
    }
}

/// One structural non-zero of a sparse matrix.
///
/// Row and column indices are **0-based**, unlike MATLAB's `[i, j, v] =
/// find(mat)` which returns them 1-based.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triplet {
    /// 0-based row index.
    pub i: usize,
    /// 0-based column index.
    pub j: usize,
    /// The stored value.
    pub v: f64,
}

/// Sparse matrix in triplet (coordinate) form, mirroring MATLAB's `sparse`.
///
/// # Semantics carried over from MATLAB
///
/// - `sparse(i, j, v, m, n)` **sums duplicate `(i, j)` pairs** rather than
///   overwriting. [`SparseMatrix::assemble`] reproduces that.
/// - Explicitly stored zeros are dropped, so `nnz` counts structural
///   non-zeros only.
/// - [`SparseMatrix::find`] returns entries in **column-major order**, the
///   order MATLAB's `find` produces on a sparse matrix.
#[derive(Clone, Debug, Default)]
pub struct SparseMatrix {
    rows: usize,
    cols: usize,
    entries: Vec<Triplet>,
}

impl SparseMatrix {
    /// An all-zero `rows`-by-`cols` sparse matrix — `sparse(m, n)`.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            entries: Vec::new(),
        }
    }

    /// `sparse(i, j, v, rows, cols)` with **0-based** index slices, summing
    /// duplicates and dropping exact zeros.
    ///
    /// # Panics
    /// If the three slices differ in length, or an index falls outside the
    /// declared shape.
    pub fn assemble(i: &[usize], j: &[usize], v: &[f64], rows: usize, cols: usize) -> Self {
        assert_eq!(i.len(), j.len(), "row and column index counts differ");
        assert_eq!(i.len(), v.len(), "index and value counts differ");
        let mut m = Self::zeros(rows, cols);
        for n in 0..i.len() {
            m.add(i[n], j[n], v[n]);
        }
        m.compress();
        m
    }

    /// `size(a, 1)`.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// `size(a, 2)`.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Accumulate `value` into entry `(i, j)`, 0-based.
    ///
    /// Duplicates are merged the next time the matrix is compressed, which
    /// every reading method ([`SparseMatrix::find`], [`SparseMatrix::nnz`],
    /// [`SparseMatrix::mul_vec`] and friends) does first.
    pub fn add(&mut self, i: usize, j: usize, value: f64) {
        assert!(i < self.rows, "row index {i} out of range 0..{}", self.rows);
        assert!(
            j < self.cols,
            "column index {j} out of range 0..{}",
            self.cols
        );
        self.entries.push(Triplet { i, j, v: value });
    }

    /// Overwrite entry `(i, j)`, 0-based — `mat(i, j) = value`.
    pub fn set(&mut self, i: usize, j: usize, value: f64) {
        self.compress();
        match self.entries.iter_mut().find(|t| t.i == i && t.j == j) {
            Some(t) => t.v = value,
            None => self.add(i, j, value),
        }
        self.compress();
    }

    /// `nnz(mat)` — the count of structural non-zeros.
    pub fn nnz(&mut self) -> usize {
        self.compress();
        self.entries.len()
    }

    /// `[i, j, v] = find(mat)` — non-zeros in column-major order, 0-based.
    pub fn find(&mut self) -> Vec<Triplet> {
        self.compress();
        self.entries.clone()
    }

    /// `diag(mat)` — the leading diagonal as a dense vector, `min(rows, cols)`
    /// long.
    ///
    /// Structural zeros read back as `0.0`, as MATLAB's `diag` on a sparse
    /// matrix does.
    pub fn diagonal(&mut self) -> Vec<f64> {
        let n = self.rows.min(self.cols);
        let mut out = vec![0.0; n];
        for t in self.find() {
            if t.i == t.j && t.i < n {
                out[t.i] = t.v;
            }
        }
        out
    }

    /// `sum(mat)` — the **column** sums, `cols()` long.
    ///
    /// MATLAB's `sum` on a matrix reduces down the first dimension, giving a
    /// row vector of per-column totals. The two solvers use it only to form
    /// `sum(mat) - diag(mat)`, the off-diagonal column mass, for their symmetry
    /// diagnostics.
    pub fn column_sums(&mut self) -> Vec<f64> {
        let mut out = vec![0.0; self.cols];
        for t in self.find() {
            out[t.j] += t.v;
        }
        out
    }

    /// A linear combination `sum_k factor_k * term_k` — the `gradD + nodal +
    /// sigma.tot - sigma.s` assembly both flux solvers build their `LHS` with.
    ///
    /// Duplicate coordinates across terms are summed, and entries that cancel
    /// to exactly zero are dropped, so this is exactly MATLAB's sparse `+`/`-`.
    ///
    /// # Panics
    /// If `terms` is empty, or the terms do not all have the same shape.
    pub fn combine(terms: &[(&SparseMatrix, f64)]) -> Self {
        let (first, _) = terms.first().expect("combine needs at least one term");
        let (rows, cols) = (first.rows, first.cols);
        let mut out = Self::zeros(rows, cols);
        for (term, factor) in terms {
            assert_eq!(
                (term.rows, term.cols),
                (rows, cols),
                "cannot combine a {}x{} matrix with a {rows}x{cols} one",
                term.rows,
                term.cols
            );
            for t in &term.entries {
                out.entries.push(Triplet {
                    i: t.i,
                    j: t.j,
                    v: factor * t.v,
                });
            }
        }
        out.compress();
        out
    }

    /// `mat * x` — sparse matrix times dense vector.
    ///
    /// # Panics
    /// If `x` is not `cols()` long.
    pub fn mul_vec(&mut self, x: &[f64]) -> Vec<f64> {
        assert_eq!(
            x.len(),
            self.cols,
            "vector length {} does not match matrix columns {}",
            x.len(),
            self.cols
        );
        let mut out = vec![0.0; self.rows];
        for t in self.find() {
            out[t.i] += t.v * x[t.j];
        }
        out
    }

    /// Merge duplicate coordinates by summation, drop exact zeros, and sort
    /// into column-major order.
    fn compress(&mut self) {
        // Column-major: column first, then row within the column.
        self.entries.sort_by_key(|t| (t.j, t.i));
        let mut merged: Vec<Triplet> = Vec::with_capacity(self.entries.len());
        for t in self.entries.drain(..) {
            match merged.last_mut() {
                Some(last) if last.i == t.i && last.j == t.j => last.v += t.v,
                _ => merged.push(t),
            }
        }
        merged.retain(|t| t.v != 0.0);
        self.entries = merged;
    }
}

/// `A \ b` for a small **dense** system — Gaussian elimination with partial
/// pivoting.
///
/// The reference uses MATLAB's `mldivide` on the tiny per-node systems the
/// nodal expansion produces: `G`-by-`G` at a boundary face and `2G`-by-`2G` at
/// an interior one, with `G` typically 2. At that size a hand-written
/// elimination is clearer than a linear-algebra dependency, and the pivoting
/// makes it as stable as `mldivide` is on the same input.
///
/// This is **not** for the large sparse systems elsewhere in the solver; those
/// want a real sparse factorisation.
///
/// # Arguments
///
/// - `a` — the `n`-by-`n` matrix, **row-major**: element `(i, j)` at
///   `a[i * n + j]`.
/// - `b` — the right-hand side, `n` long.
/// - `n` — the system size.
///
/// # Returns
///
/// The solution vector. A **singular** matrix yields `NaN` entries rather than
/// an error, which is the closest match to MATLAB's behaviour — it warns and
/// returns non-finite values rather than aborting. Callers that care must check.
///
/// # Panics
///
/// If `a` is not `n * n` long or `b` is not `n` long.
pub fn solve_dense(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    assert_eq!(a.len(), n * n, "matrix is not {n}x{n}");
    assert_eq!(b.len(), n, "right-hand side is not {n} long");

    let mut m = a.to_vec();
    let mut rhs = b.to_vec();

    for k in 0..n {
        // Partial pivot: largest magnitude in column k at or below row k.
        let mut pivot = k;
        for i in (k + 1)..n {
            if m[i * n + k].abs() > m[pivot * n + k].abs() {
                pivot = i;
            }
        }
        if m[pivot * n + k] == 0.0 {
            // Singular: MATLAB warns and propagates non-finite values.
            return vec![f64::NAN; n];
        }
        if pivot != k {
            for j in 0..n {
                m.swap(k * n + j, pivot * n + j);
            }
            rhs.swap(k, pivot);
        }

        for i in (k + 1)..n {
            let factor = m[i * n + k] / m[k * n + k];
            if factor == 0.0 {
                continue;
            }
            for j in k..n {
                m[i * n + j] -= factor * m[k * n + j];
            }
            rhs[i] -= factor * rhs[k];
        }
    }

    // Back substitution.
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut acc = rhs[i];
        for j in (i + 1)..n {
            acc -= m[i * n + j] * x[j];
        }
        x[i] = acc / m[i * n + i];
    }
    x
}

/// `norm(v, 1)` — the sum of absolute values.
///
/// The two flux solvers use this for the fission-source integral that drives
/// the `keff` update. Note [`crate::sanodaldiffusion_solverxyz`] uses a plain
/// `sum` in two of the three places where [`crate::diffusion_solverxyz`] uses
/// this; the two differ whenever the fission source has a negative entry, which
/// a diverging solve can produce. That inconsistency is the reference's — see
/// defect N10.
pub fn norm1(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).sum()
}

/// `norm(v)` — the Euclidean (2-)norm.
pub fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// `decomposition(A)` — a reusable sparse factorisation, MATLAB's own idiom for
/// "factorise once, then apply `\` many times".
///
/// Both flux solvers factorise their `LHS` outside the power iteration and
/// solve against it every pass; [`crate::sanodaldiffusion_solverxyz`]
/// refactorises whenever the nodal correction is updated. That reuse is the
/// whole reason the reference calls `decomposition` rather than `A\b`.
///
/// The factorisation is an LU with partial pivoting, matching what MATLAB
/// selects for a general (unsymmetric, non-triangular) sparse matrix — the
/// nodal-diffusion `LHS` is unsymmetric, so no Cholesky path applies.
///
/// # A singular matrix yields `NaN`, it does not error
///
/// MATLAB's `decomposition` **warns** on a singular matrix and its `\` then
/// propagates `Inf`/`NaN`; it does not abort. That behaviour is load-bearing
/// here, because both solvers are written to catch it downstream —
/// `sanodaldiffusion_solverxyz` runs the result through
/// [`crate::fixinfnan::fixinfnan`], and both break their power iteration on
/// `isnan(k_eff)`. So a failed factorisation is represented as
/// [`Decomposition::Singular`], whose [`Decomposition::solve`] returns all
/// `NaN`, rather than as an error that would skip those guards.
// The two variants differ in size by the whole factorisation, which clippy
// flags. Boxing is the usual remedy and the workspace forbids `Box<T>`; the
// asymmetry is harmless here because a `Decomposition` is created once per
// factorisation and held by value in one local, never in a collection.
#[allow(clippy::large_enum_variant)]
pub enum Decomposition {
    /// A successful LU factorisation.
    Factorised(faer::sparse::linalg::solvers::Lu<usize, f64>),
    /// The factorisation failed — see the type-level note on why this is not an
    /// error.
    Singular,
}

impl Decomposition {
    /// Factorise a square sparse matrix.
    ///
    /// # Panics
    /// If `a` is not square.
    pub fn new(a: &mut SparseMatrix) -> Self {
        assert_eq!(
            a.rows(),
            a.cols(),
            "decomposition needs a square matrix, got {}x{}",
            a.rows(),
            a.cols()
        );
        let n = a.rows();
        let triplets: Vec<faer::sparse::Triplet<usize, usize, f64>> = a
            .find()
            .into_iter()
            .map(|t| faer::sparse::Triplet::new(t.i, t.j, t.v))
            .collect();

        match faer::sparse::SparseColMat::try_new_from_triplets(n, n, &triplets) {
            Ok(m) => match m.sp_lu() {
                Ok(lu) => Self::Factorised(lu),
                Err(_) => Self::Singular,
            },
            Err(_) => Self::Singular,
        }
    }

    /// `dA \ b`.
    ///
    /// # Panics
    /// If `rhs` is not the matrix's dimension long.
    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        use faer::prelude::Solve;

        match self {
            Self::Singular => vec![f64::NAN; rhs.len()],
            Self::Factorised(lu) => {
                let mut b = faer::Mat::<f64>::zeros(rhs.len(), 1);
                for (i, &v) in rhs.iter().enumerate() {
                    b[(i, 0)] = v;
                }
                lu.solve_in_place(&mut b);
                (0..rhs.len()).map(|i| b[(i, 0)]).collect()
            }
        }
    }
}

/// `min(abs(v))` over the entries that have a finite magnitude.
///
/// MATLAB's `min` skips `NaN`, and `abs(Inf)` is `Inf`, which can only be the
/// minimum when every entry is non-finite. This reproduces both behaviours and
/// returns `None` for that degenerate case, where MATLAB would return `Inf`.
/// The one caller, `fixinfnan`, only reaches it when at least one entry is
/// non-finite.
pub fn min_abs_finite(v: &[f64]) -> Option<f64> {
    v.iter().filter(|x| x.is_finite()).map(|x| x.abs()).fold(
        None,
        |acc: Option<f64>, x| match acc {
            Some(m) if m <= x => Some(m),
            _ => Some(x),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array2_is_column_major() {
        let mut a = Array2::<f64>::zeros(2, 3);
        a.set(0, 0, 1.0);
        a.set(1, 0, 2.0);
        a.set(0, 1, 3.0);
        // MATLAB: a(:)' == [1 2 3 0 0 0]
        assert_eq!(a.as_slice(), &[1.0, 2.0, 3.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn array3_linear_layout_matches_matlab() {
        let mut a = Array3::<f64>::zeros(2, 2, 2);
        a.set(0, 0, 1, 7.0);
        assert_eq!(a.get(0, 0, 1), 7.0);
    }

    #[test]
    fn sparse_sums_duplicates_and_drops_zeros() {
        let mut m = SparseMatrix::assemble(&[0, 0, 1], &[0, 0, 1], &[1.5, -1.5, 4.0], 2, 2);
        // (0,0) cancels to zero and is dropped; only (1,1) survives.
        assert_eq!(m.nnz(), 1);
        let found = m.find();
        assert_eq!(found[0].i, 1);
        assert_eq!(found[0].j, 1);
        assert_eq!(found[0].v, 4.0);
    }

    #[test]
    fn min_abs_ignores_non_finite() {
        let v = [f64::NAN, -3.0, f64::INFINITY, 5.0];
        assert_eq!(min_abs_finite(&v), Some(3.0));
        assert_eq!(min_abs_finite(&[f64::NAN]), None);
    }
}
