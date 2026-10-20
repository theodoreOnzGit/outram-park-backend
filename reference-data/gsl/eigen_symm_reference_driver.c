/* Reference driver: GSL 2.8 symmetric tridiagonal decomposition and
 * symmetric eigensolvers.
 *
 * Routines: gsl_linalg_symmtd_decomp, gsl_linalg_symmtd_unpack,
 * gsl_eigen_symm, gsl_eigen_symmv, gsl_eigen_symmv_sort -- what
 * petir::linalg::{symmtd, eigen_symm} port.
 *
 * Output format (%.17e, so every double round-trips):
 *   CASE <name> <n>
 *   A <i> <j> <v>      the input
 *   TD <i> <j> <v>     packed decomposition, lower triangle (j <= i)
 *   TAU <i> <v>        tau[0 .. n-3] (tau[n-2] is GSL workspace, not output)
 *   Q <i> <j> <v>      unpacked Q
 *   D <i> <v> / SD <i> <v>   tridiagonal T
 *   EVAL <i> <v>       gsl_eigen_symm, unordered
 *   VVAL <i> <v>       gsl_eigen_symmv values, unordered
 *   VVEC <i> <j> <v>   gsl_eigen_symmv vectors (columns), unordered
 *   SVAL / SVEC        after gsl_eigen_symmv_sort(VAL_ASC)
 *   END <name>
 *
 * Build: see README.md in this directory (same static GSL as the QR driver).
 */
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_linalg.h>
#include <gsl/gsl_eigen.h>

static unsigned long lcg_state = 4242UL;
static double next_val(void)
{
  lcg_state = (1103515245UL * lcg_state + 12345UL) % 2147483648UL;
  return (double) lcg_state / 2147483648.0 - 0.5;
}

static void pmat(const char *tag, const gsl_matrix *M)
{
  size_t i, j;
  for (i = 0; i < M->size1; i++)
    for (j = 0; j < M->size2; j++)
      printf("%s %zu %zu %.17e\n", tag, i, j, gsl_matrix_get(M, i, j));
}

static void pvec(const char *tag, const gsl_vector *v)
{
  size_t i;
  for (i = 0; i < v->size; i++)
    printf("%s %zu %.17e\n", tag, i, gsl_vector_get(v, i));
}

static void emit(const char *name, const gsl_matrix *A0)
{
  const size_t n = A0->size1;
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  gsl_vector *eval = gsl_vector_alloc(n);
  gsl_matrix *evec = gsl_matrix_alloc(n, n);
  size_t i, j;

  printf("CASE %s %zu\n", name, n);
  pmat("A", A0);

  if (n >= 2) {
    gsl_vector *tau = gsl_vector_alloc(n - 1);
    gsl_matrix *Q = gsl_matrix_alloc(n, n);
    gsl_vector *d = gsl_vector_alloc(n);
    gsl_vector *sd = gsl_vector_alloc(n - 1);
    gsl_matrix_memcpy(A, A0);
    gsl_linalg_symmtd_decomp(A, tau);
    for (i = 0; i < n; i++)
      for (j = 0; j <= i; j++)
        printf("TD %zu %zu %.17e\n", i, j, gsl_matrix_get(A, i, j));
    for (i = 0; i + 2 < n; i++)
      printf("TAU %zu %.17e\n", i, gsl_vector_get(tau, i));
    gsl_linalg_symmtd_unpack(A, tau, Q, d, sd);
    pmat("Q", Q);
    pvec("D", d);
    pvec("SD", sd);
    gsl_vector_free(tau);
    gsl_matrix_free(Q);
    gsl_vector_free(d);
    gsl_vector_free(sd);
  }

  {
    gsl_eigen_symm_workspace *w = gsl_eigen_symm_alloc(n);
    gsl_matrix_memcpy(A, A0);
    gsl_eigen_symm(A, eval, w);
    pvec("EVAL", eval);
    gsl_eigen_symm_free(w);
  }
  {
    gsl_eigen_symmv_workspace *w = gsl_eigen_symmv_alloc(n);
    gsl_matrix_memcpy(A, A0);
    gsl_eigen_symmv(A, eval, evec, w);
    pvec("VVAL", eval);
    pmat("VVEC", evec);
    gsl_eigen_symmv_sort(eval, evec, GSL_EIGEN_SORT_VAL_ASC);
    pvec("SVAL", eval);
    pmat("SVEC", evec);
    gsl_eigen_symmv_free(w);
  }
  printf("END %s\n", name);
  gsl_matrix_free(A);
  gsl_vector_free(eval);
  gsl_matrix_free(evec);
}

static gsl_matrix *gram(size_t n, double shift)
{
  gsl_matrix *B = gsl_matrix_alloc(n, n);
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  size_t i, j, k;
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      gsl_matrix_set(B, i, j, next_val());
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++) {
      double s = 0.0;
      for (k = 0; k < n; k++)
        s += gsl_matrix_get(B, k, i) * gsl_matrix_get(B, k, j);
      gsl_matrix_set(A, i, j, s + (i == j ? shift : 0.0));
    }
  gsl_matrix_free(B);
  return A;
}

/* symmetric with entries of both signs: (B + B^T) / 2 */
static gsl_matrix *symrand(size_t n)
{
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  size_t i, j;
  for (i = 0; i < n; i++)
    for (j = 0; j <= i; j++) {
      double v = next_val();
      gsl_matrix_set(A, i, j, v);
      gsl_matrix_set(A, j, i, v);
    }
  return A;
}

int main(void)
{
  gsl_set_error_handler_off();

  {
    double a[] = {7.0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 1, 1);
    emit("one1", &A.matrix);
  }
  {
    double a[] = {2, 1, 1, 2};
    gsl_matrix_view A = gsl_matrix_view_array(a, 2, 2);
    emit("two2", &A.matrix);
  }
  {
    double a[] = {4, 1, -2, 2, 1, 2, 0, 1, -2, 0, 3, -2, 2, 1, -2, -1};
    gsl_matrix_view A = gsl_matrix_view_array(a, 4, 4);
    emit("sym4", &A.matrix);
  }
  /* Hilbert 8: eigenvalues spanning ~1e-10 .. 1.7 */
  {
    gsl_matrix *A = gsl_matrix_alloc(8, 8);
    size_t i, j;
    for (i = 0; i < 8; i++)
      for (j = 0; j < 8; j++)
        gsl_matrix_set(A, i, j, 1.0 / (double) (i + j + 1));
    emit("hilbert8", A);
    gsl_matrix_free(A);
  }
  /* Wilkinson W21+: tridiagonal, eigenvalues in nearly-equal pairs */
  {
    gsl_matrix *A = gsl_matrix_calloc(21, 21);
    size_t i;
    for (i = 0; i < 21; i++) {
      gsl_matrix_set(A, i, i, (double) (i > 10 ? i - 10 : 10 - i));
      if (i + 1 < 21) {
        gsl_matrix_set(A, i, i + 1, 1.0);
        gsl_matrix_set(A, i + 1, i, 1.0);
      }
    }
    emit("wilkinson21", A);
    gsl_matrix_free(A);
  }
  /* Block diagonal: two decoupled 3x3 blocks, so deflation splits early */
  {
    double a[36] = {0};
    double blk[9] = {2, -1, 0, -1, 2, -1, 0, -1, 2};
    size_t i, j;
    for (i = 0; i < 3; i++)
      for (j = 0; j < 3; j++) {
        a[i * 6 + j] = blk[i * 3 + j];
        a[(i + 3) * 6 + (j + 3)] = 3.0 * blk[i * 3 + j] - 1.0 * (i == j);
      }
    {
      gsl_matrix_view A = gsl_matrix_view_array(a, 6, 6);
      emit("blocks6", &A.matrix);
    }
  }
  /* Repeated eigenvalue: 5 I + rank-one */
  {
    gsl_matrix *A = gsl_matrix_alloc(5, 5);
    size_t i, j;
    for (i = 0; i < 5; i++)
      for (j = 0; j < 5; j++)
        gsl_matrix_set(A, i, j, (i == j ? 5.0 : 0.0) + 1.0);
    emit("rankone5", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = symrand(10);
    emit("symrand10", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = gram(30, 0.5);
    emit("gram30", A);
    gsl_matrix_free(A);
  }
  return 0;
}
