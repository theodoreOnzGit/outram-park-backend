/* Reference driver: GSL 2.8 singular value decomposition (Golub-Reinsch)
 * and SVD solve -- what petir::linalg::svd ports.
 *
 * Routines: gsl_linalg_SV_decomp, gsl_linalg_SV_solve.
 *
 * Output format (%.17e):
 *   CASE <name> <m> <n>
 *   A <i> <j> <v> / B <i> <v>    inputs
 *   STATUS <code>
 *   U <i> <j> <v> / S <i> <v> / V <i> <j> <v>
 *   X <i> <v>                    gsl_linalg_SV_solve(U, V, S, b)
 *   END <name>
 *
 * Build: reference-data/gsl/build_driver.sh (see README.md).
 */
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_linalg.h>

static unsigned long lcg_state = 777UL;
static double next_val(void)
{
  lcg_state = (1103515245UL * lcg_state + 12345UL) % 2147483648UL;
  return (double) lcg_state / 2147483648.0 - 0.5;
}

static void emit(const char *name, const gsl_matrix *A0)
{
  const size_t m = A0->size1, n = A0->size2;
  gsl_matrix *U = gsl_matrix_alloc(m, n);
  gsl_matrix *V = gsl_matrix_alloc(n, n);
  gsl_vector *S = gsl_vector_alloc(n);
  gsl_vector *work = gsl_vector_alloc(n);
  gsl_vector *b = gsl_vector_alloc(m);
  gsl_vector *x = gsl_vector_alloc(n);
  size_t i, j;
  int status;

  for (i = 0; i < m; i++)
    gsl_vector_set(b, i, next_val());
  printf("CASE %s %zu %zu\n", name, m, n);
  for (i = 0; i < m; i++)
    for (j = 0; j < n; j++)
      printf("A %zu %zu %.17e\n", i, j, gsl_matrix_get(A0, i, j));
  for (i = 0; i < m; i++)
    printf("B %zu %.17e\n", i, gsl_vector_get(b, i));

  gsl_matrix_memcpy(U, A0);
  status = gsl_linalg_SV_decomp(U, V, S, work);
  printf("STATUS %d\n", status);
  for (i = 0; i < m; i++)
    for (j = 0; j < n; j++)
      printf("U %zu %zu %.17e\n", i, j, gsl_matrix_get(U, i, j));
  for (i = 0; i < n; i++)
    printf("S %zu %.17e\n", i, gsl_vector_get(S, i));
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      printf("V %zu %zu %.17e\n", i, j, gsl_matrix_get(V, i, j));
  gsl_linalg_SV_solve(U, V, S, b, x);
  for (i = 0; i < n; i++)
    printf("X %zu %.17e\n", i, gsl_vector_get(x, i));
  printf("END %s\n", name);

  gsl_matrix_free(U);
  gsl_matrix_free(V);
  gsl_vector_free(S);
  gsl_vector_free(work);
  gsl_vector_free(b);
  gsl_vector_free(x);
}

static gsl_matrix *randm(size_t m, size_t n, double scale)
{
  gsl_matrix *A = gsl_matrix_alloc(m, n);
  size_t i, j;
  for (i = 0; i < m; i++)
    for (j = 0; j < n; j++)
      gsl_matrix_set(A, i, j, scale * next_val());
  return A;
}

int main(void)
{
  gsl_set_error_handler_off();
  {
    double a[] = {3, 0, 4, 5};
    gsl_matrix_view A = gsl_matrix_view_array(a, 2, 2);
    emit("square2", &A.matrix);
  }
  {
    double a[] = {1, 2, 3, 4, 5, 6, 7, 8, 10, 2, -1, 4};
    gsl_matrix_view A = gsl_matrix_view_array(a, 4, 3);
    emit("general43", &A.matrix);
  }
  /* column vector: the N == 1 branch */
  {
    double a[] = {1, -2, 2, 4, 0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 5, 1);
    emit("column5", &A.matrix);
  }
  /* Hilbert 6: singular values spanning ~1e-8 .. 1.6 */
  {
    gsl_matrix *A = gsl_matrix_alloc(6, 6);
    size_t i, j;
    for (i = 0; i < 6; i++)
      for (j = 0; j < 6; j++)
        gsl_matrix_set(A, i, j, 1.0 / (double) (i + j + 1));
    emit("hilbert6", A);
    gsl_matrix_free(A);
  }
  /* rank one: zeros appear on the bidiagonal */
  {
    double a[] = {1, 2, 3, 2, 4, 6, 3, 6, 9};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    emit("rank1_3", &A.matrix);
  }
  /* a zero middle column: an intermediate zero on the diagonal */
  {
    double a[] = {1, 0, 2, 3, 0, 1, 4, 0, -1, 2, 0, 5};
    gsl_matrix_view A = gsl_matrix_view_array(a, 4, 3);
    emit("zerocol43", &A.matrix);
  }
  /* a zero last column: a trailing zero on the diagonal */
  {
    double a[] = {2, 1, 0, 1, 3, 0, 0, 1, 0, 4, 1, 0};
    gsl_matrix_view A = gsl_matrix_view_array(a, 4, 3);
    emit("zerolast43", &A.matrix);
  }
  /* the identity: already diagonal, nothing to do */
  {
    double a[] = {1, 0, 0, 0, 1, 0, 0, 0, 1};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    emit("identity3", &A.matrix);
  }
  /* tiny and huge entries: the block rescaling branches */
  {
    gsl_matrix *A = randm(4, 4, 1e-160);
    emit("tiny4", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = randm(4, 4, 1e160);
    emit("huge4", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = randm(8, 5, 1.0);
    emit("rand85", A);
    gsl_matrix_free(A);
  }
  {
    gsl_matrix *A = randm(30, 20, 1.0);
    emit("rand3020", A);
    gsl_matrix_free(A);
  }
  return 0;
}
