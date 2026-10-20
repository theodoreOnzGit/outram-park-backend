/* Reference driver: GSL 2.8 nonsymmetric eigenvalues (balancing, Hessenberg
 * reduction, Francis double-shift QR) -- what petir::linalg::eigen_nonsymm
 * ports.
 *
 * Routines: gsl_linalg_hessenberg_decomp, gsl_linalg_balance_matrix,
 * gsl_eigen_nonsymm (eigenvalues only; compute_t = 0, as after
 * gsl_eigen_nonsymm_alloc), with gsl_eigen_nonsymm_params(0, balance, w)
 * for balance = 0 and 1.
 *
 * Output format (%.17e):
 *   CASE <name> <n>
 *   A <i> <j> <v>               input
 *   HESS <i> <j> <v>            hessenberg_decomp(A), packed
 *   HTAU <i> <v>                its tau[0 .. n-3]
 *   BAL <i> <j> <v> / BALD <i> <v>   balance_matrix(A) and D
 *   STATUS0 <code> / EV0R <k> <re> / EV0I <k> <im>   unbalanced eigenvalues
 *   STATUS1 <code> / EV1R <k> <re> / EV1I <k> <im>   balanced eigenvalues
 *   END <name>
 *
 * Build: reference-data/gsl/build_driver.sh (see README.md).
 */
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_complex.h>
#include <gsl/gsl_linalg.h>
#include <gsl/gsl_eigen.h>

static unsigned long lcg_state = 31337UL;
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

static void eig(const gsl_matrix *A0, int balance, const char *st,
                const char *re, const char *im)
{
  const size_t n = A0->size1;
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  gsl_vector_complex *ev = gsl_vector_complex_alloc(n);
  gsl_eigen_nonsymm_workspace *w = gsl_eigen_nonsymm_alloc(n);
  size_t k;
  int status;
  gsl_matrix_memcpy(A, A0);
  gsl_eigen_nonsymm_params(0, balance, w);
  status = gsl_eigen_nonsymm(A, ev, w);
  printf("%s %d\n", st, status);
  for (k = 0; k < n; k++) {
    gsl_complex z = gsl_vector_complex_get(ev, k);
    printf("%s %zu %.17e\n", re, k, GSL_REAL(z));
    printf("%s %zu %.17e\n", im, k, GSL_IMAG(z));
  }
  gsl_eigen_nonsymm_free(w);
  gsl_vector_complex_free(ev);
  gsl_matrix_free(A);
}

static void emit(const char *name, const gsl_matrix *A0)
{
  const size_t n = A0->size1;
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  gsl_vector *tau = gsl_vector_alloc(n);
  size_t i;

  printf("CASE %s %zu\n", name, n);
  pmat("A", A0);

  gsl_matrix_memcpy(A, A0);
  gsl_linalg_hessenberg_decomp(A, tau);
  pmat("HESS", A);
  for (i = 0; i + 2 < n; i++)
    printf("HTAU %zu %.17e\n", i, gsl_vector_get(tau, i));

  gsl_matrix_memcpy(A, A0);
  gsl_linalg_balance_matrix(A, tau);
  pmat("BAL", A);
  for (i = 0; i < n; i++)
    printf("BALD %zu %.17e\n", i, gsl_vector_get(tau, i));

  eig(A0, 0, "STATUS0", "EV0R", "EV0I");
  eig(A0, 1, "STATUS1", "EV1R", "EV1I");
  printf("END %s\n", name);
  gsl_matrix_free(A);
  gsl_vector_free(tau);
}

static gsl_matrix *randm(size_t n)
{
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  size_t i, j;
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      gsl_matrix_set(A, i, j, next_val());
  return A;
}

int main(void)
{
  gsl_set_error_handler_off();
  {
    double a[] = {4.0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 1, 1);
    emit("one1", &A.matrix);
  }
  /* rotation generator: +-i */
  {
    double a[] = {0, -1, 1, 0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 2, 2);
    emit("rot2", &A.matrix);
  }
  /* real 2x2 with distinct real eigenvalues */
  {
    double a[] = {1, 2, 3, 4};
    gsl_matrix_view A = gsl_matrix_view_array(a, 2, 2);
    emit("real2", &A.matrix);
  }
  /* cyclic permutation: cube roots of unity, a known stall for plain
   * Wilkinson shifts, so it reaches the exceptional shift */
  {
    double a[] = {0, 1, 0, 0, 0, 1, 1, 0, 0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    emit("cyclic3", &A.matrix);
  }
  /* 4-cycle */
  {
    double a[16] = {0};
    a[0 * 4 + 1] = 1; a[1 * 4 + 2] = 1; a[2 * 4 + 3] = 1; a[3 * 4 + 0] = 1;
    {
      gsl_matrix_view A = gsl_matrix_view_array(a, 4, 4);
      emit("cyclic4", &A.matrix);
    }
  }
  /* companion of (x-1)(x-2)(x-3)(x^2+1) */
  {
    double c[] = {-6, 11, -12, 12, -6};
    double a[25] = {0};
    size_t i;
    for (i = 1; i < 5; i++)
      a[i * 5 + i - 1] = 1;
    for (i = 0; i < 5; i++)
      a[i * 5 + 4] = -c[i];
    {
      gsl_matrix_view A = gsl_matrix_view_array(a, 5, 5);
      emit("companion5", &A.matrix);
    }
  }
  /* upper triangular: already Schur */
  {
    double a[] = {1, 2, 3, 0, 4, 5, 0, 0, 6};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    emit("triu3", &A.matrix);
  }
  /* Frank matrix 8: ill-conditioned small eigenvalues */
  {
    gsl_matrix *A = gsl_matrix_alloc(8, 8);
    size_t i, j;
    for (i = 0; i < 8; i++)
      for (j = 0; j < 8; j++) {
        double v = 0.0;
        if (j + 1 >= i)
          v = (double) (8 - (i > j ? i : j));
        gsl_matrix_set(A, i, j, v);
      }
    emit("frank8", A);
    gsl_matrix_free(A);
  }
  /* badly scaled: balancing changes the arithmetic */
  {
    double a[] = {1, 1e6, 1e-3, 1e-6, 2, 1e2, 1e4, 1e-4, 3};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    emit("scaled3", &A.matrix);
  }
  /* block diagonal: deflation splits in the middle */
  {
    double a[36] = {0};
    double b1[9] = {1, 2, 0, -2, 1, 1, 0, 1, 3};
    double b2[9] = {0, 1, 2, -1, 0, 1, 1, 1, 1};
    size_t i, j;
    for (i = 0; i < 3; i++)
      for (j = 0; j < 3; j++) {
        a[i * 6 + j] = b1[i * 3 + j];
        a[(i + 3) * 6 + (j + 3)] = b2[i * 3 + j];
      }
    {
      gsl_matrix_view A = gsl_matrix_view_array(a, 6, 6);
      emit("blocks6", &A.matrix);
    }
  }
  {
    gsl_matrix *A = randm(6);
    emit("rand6", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = randm(12);
    emit("rand12", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = randm(25);
    emit("rand25", A);
    gsl_matrix_free(A);
  }
  return 0;
}
