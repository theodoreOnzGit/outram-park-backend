/* Reference driver: GSL 2.8 Householder QR and least-squares solve.
 *
 * Emits, for each case, the packed QR factor, tau, the least-squares
 * solution and the residual, so that petir::linalg::qr can be compared
 * against what GSL's compiled code actually computes rather than against
 * what its documentation says it should.
 *
 * Uses gsl_linalg_QR_decomp_old -- the CLASSICAL Householder sweep, which is
 * what petir ports. gsl_linalg_QR_decomp is the blocked Level-3 variant and
 * would be a different (equally valid) factorisation.
 *
 * Build: see README.md in this directory.
 */
#include <stdio.h>
#include <stdlib.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_linalg.h>

static void emit(const char *name, size_t m, size_t n,
                 const double *a, const double *b)
{
  gsl_matrix *A = gsl_matrix_alloc(m, n);
  gsl_vector *tau = gsl_vector_alloc(m < n ? m : n);
  gsl_vector *B = gsl_vector_alloc(m);
  size_t i, j;

  for (i = 0; i < m; i++)
    for (j = 0; j < n; j++)
      gsl_matrix_set(A, i, j, a[i * n + j]);
  for (i = 0; i < m; i++)
    gsl_vector_set(B, i, b[i]);

  gsl_linalg_QR_decomp_old(A, tau);

  printf("CASE %s %zu %zu\n", name, m, n);
  for (i = 0; i < m; i++) {
    for (j = 0; j < n; j++)
      printf("QR %zu %zu %.17e\n", i, j, gsl_matrix_get(A, i, j));
  }
  for (i = 0; i < tau->size; i++)
    printf("TAU %zu %.17e\n", i, gsl_vector_get(tau, i));

  if (m >= n) {
    gsl_vector *x = gsl_vector_alloc(n);
    gsl_vector *r = gsl_vector_alloc(m);
    gsl_linalg_QR_lssolve(A, tau, B, x, r);
    for (i = 0; i < n; i++)
      printf("X %zu %.17e\n", i, gsl_vector_get(x, i));
    for (i = 0; i < m; i++)
      printf("R %zu %.17e\n", i, gsl_vector_get(r, i));
    gsl_vector_free(x);
    gsl_vector_free(r);
  }
  printf("END %s\n", name);

  gsl_matrix_free(A);
  gsl_vector_free(tau);
  gsl_vector_free(B);
}

int main(void)
{
  /* 1. A small square system. */
  {
    double a[] = {2.0, 1.0, 1.0, 3.0};
    double b[] = {5.0, 10.0};
    emit("square2", 2, 2, a, b);
  }
  /* 2. A general 4x3, no structure. */
  {
    double a[] = {1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 2.0, -1.0, 4.0};
    double b[] = {1.0, 2.0, 3.0, 4.0};
    emit("general43", 4, 3, a, b);
  }
  /* 3. An overdetermined Vandermonde -- the shape a polynomial fit makes. */
  {
    double a[15], b[5];
    int i, j;
    for (i = 0; i < 5; i++) {
      double x = -1.0 + 0.5 * i;
      for (j = 0; j < 3; j++)
        a[i * 3 + j] = (j == 0) ? 1.0 : ((j == 1) ? x : x * x);
      b[i] = 1.0 + 2.0 * x - 0.5 * x * x + ((i % 2) ? 0.01 : -0.01);
    }
    emit("vander53", 5, 3, a, b);
  }
  /* 4. A Chebyshev design matrix at non-node points -- petir's fit path. */
  {
    double a[32], b[8];
    int i, j;
    for (i = 0; i < 8; i++) {
      double t = -1.0 + 2.0 * i / 7.0;
      double tk[4];
      tk[0] = 1.0; tk[1] = t;
      for (j = 2; j < 4; j++) tk[j] = 2.0 * t * tk[j-1] - tk[j-2];
      for (j = 0; j < 4; j++) a[i * 4 + j] = tk[j];
      b[i] = 1.0 / (1.0 + 25.0 * t * t);
    }
    emit("cheb84", 8, 4, a, b);
  }
  /* 5. A column scaled far from the others, to exercise the DBL_MIN branch
     of the Householder transform less trivially. */
  {
    double a[] = {1e8, 1.0, 1e8, 2.0, 1e8, 3.0, 1e8, 4.0};
    double b[] = {1.0, 2.0, 3.0, 4.0};
    emit("scaled42", 4, 2, a, b);
  }
  return 0;
}
