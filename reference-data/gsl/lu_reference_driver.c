/* Reference driver: GSL 2.8 LU factorisation with partial pivoting and its
 * solve (gsl_linalg_LU_decomp, gsl_linalg_LU_solve), for
 * petir::linalg::lu_decomp (gh:#992).
 *
 * Emits, for each case, the input matrix A and right-hand side b (so the
 * replay test reads its inputs from here rather than regenerating them), the
 * packed LU factor, the permutation, signum, the decomp status and -- unless
 * the matrix is singular -- the solution x.
 *
 * gsl_linalg_LU_decomp runs the Level-2 LU_decomp_L2 for N <= 24
 * (CROSSOVER_LU) and the recursive Level-3 LU_decomp_L3 above; the n30 case
 * is there to MEASURE the documented deviation (petir ports L2 only).
 *
 * Build: see README.md in this directory.
 */
#include <stdio.h>
#include <stdlib.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_permutation.h>
#include <gsl/gsl_linalg.h>

static void emit(const char *name, size_t n, const double *a, const double *b)
{
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  gsl_vector *B = gsl_vector_alloc(n);
  gsl_vector *X = gsl_vector_alloc(n);
  gsl_permutation *p = gsl_permutation_alloc(n);
  int signum = 0, status, solve_status;
  size_t i, j;

  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      gsl_matrix_set(A, i, j, a[i * n + j]);
  for (i = 0; i < n; i++)
    gsl_vector_set(B, i, b[i]);

  printf("CASE %s %zu\n", name, n);
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      printf("A %zu %zu %.17e\n", i, j, a[i * n + j]);
  for (i = 0; i < n; i++)
    printf("B %zu %.17e\n", i, b[i]);

  status = gsl_linalg_LU_decomp(A, p, &signum);
  printf("STATUS %d\n", status);
  printf("SIGNUM %d\n", signum);
  for (i = 0; i < n; i++)
    printf("P %zu %zu\n", i, gsl_permutation_get(p, i));
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      printf("LU %zu %zu %.17e\n", i, j, gsl_matrix_get(A, i, j));

  solve_status = gsl_linalg_LU_solve(A, p, B, X);
  printf("SOLVE %d\n", solve_status);
  if (solve_status == GSL_SUCCESS)
    for (i = 0; i < n; i++)
      printf("X %zu %.17e\n", i, gsl_vector_get(X, i));
  printf("END %s\n", name);

  gsl_matrix_free(A);
  gsl_vector_free(B);
  gsl_vector_free(X);
  gsl_permutation_free(p);
}

/* A deterministic, unstructured matrix built from integer arithmetic and one
 * correctly-rounded division per entry, so nothing depends on a libm. */
static void filled(size_t n, double *a, double *b)
{
  size_t i, j;
  for (i = 0; i < n; i++) {
    for (j = 0; j < n; j++) {
      long k = (long)((i * 7 + j * 13 + i * j) % 23) - 11;
      a[i * n + j] = (double)k + (double)(i + 1) / (double)(j + 3);
    }
    b[i] = (double)((long)(i % 5) - 2) + 0.25 * (double)i;
  }
}

int main(void)
{
  gsl_set_error_handler_off();

  { double a[] = {2.0, 1.0, 1.0, 3.0}; double b[] = {5.0, 10.0};
    emit("square2", 2, a, b); }

  /* Zero leading pivot: needs a row swap. */
  { double a[] = {0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0};
    double b[] = {3.0, 4.0, 5.0};
    emit("swap3", 3, a, b); }

  /* Ties in the pivot column: idamax takes the first. */
  { double a[] = {1.0, 2.0, 3.0, -1.0, 5.0, 1.0, 1.0, -2.0, 4.0};
    double b[] = {1.0, 2.0, 3.0};
    emit("tie3", 3, a, b); }

  /* Rows of very different scale: unscaled and scaled partial pivoting choose
     different rows here, which is the point of this case. */
  { double a[] = {1e-3, 1e3, 2.0, 0.0,
                  2e-3, 1.0, 0.0, 5.0,
                  1.0, 1e6, 3.0, 1.0,
                  0.5, 2.0, 7.0, 1e-4};
    double b[] = {1.0, -1.0, 2.0, 0.5};
    emit("scaled4", 4, a, b); }

  /* A pivot below DBL_MIN: GSL divides instead of multiplying by 1/Ajj. */
  { double a[] = {1e-310, 1.0, 4e-311, 3.0}; double b[] = {1.0, 1.0};
    emit("subnormal2", 2, a, b); }

  /* Exactly singular. */
  { double a[] = {1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 1.0, 0.0, 1.0};
    double b[] = {1.0, 2.0, 3.0};
    emit("singular3", 3, a, b); }

  { double a[36], b[6]; filled(6, a, b); emit("filled6", 6, a, b); }
  { double a[576], b[24]; filled(24, a, b); emit("filled24", 24, a, b); }
  { double a[900], b[30]; filled(30, a, b); emit("filled30_level3", 30, a, b); }
  return 0;
}
