/* Reference driver: GSL 2.8 Cholesky decomposition, solve and inverse.
 *
 * Routines: gsl_linalg_cholesky_decomp1, gsl_linalg_cholesky_solve,
 * gsl_linalg_cholesky_invert -- what petir::linalg::cholesky ports.
 *
 * Output format (one value per line, %.17e so every double round-trips):
 *   CASE <name> <n>
 *   A <i> <j> <v>      the input matrix, exactly as factorised
 *   B <i> <v>          the right-hand side
 *   STATUS <code>      return of decomp1 (0 = success, GSL_EDOM = 1)
 *   LLT <i> <j> <v>    the packed factor (L below + diagonal, A above)
 *   X <i> <v>          the solve
 *   INV <i> <j> <v>    the inverse
 *   END <name>
 * The replaying test rebuilds its inputs from the A/B lines, so nothing has
 * to be kept in step by hand.
 *
 * Build: see README.md in this directory (same static GSL as the QR driver).
 */
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_linalg.h>

static unsigned long lcg_state = 12345UL;
/* Deterministic values in [-0.5, 0.5); the exact doubles are printed anyway. */
static double next_val(void)
{
  lcg_state = (1103515245UL * lcg_state + 12345UL) % 2147483648UL;
  return (double) lcg_state / 2147483648.0 - 0.5;
}

static void emit(const char *name, gsl_matrix *A0, const gsl_vector *b)
{
  const size_t n = A0->size1;
  gsl_matrix *A = gsl_matrix_alloc(n, n);
  gsl_vector *x = gsl_vector_alloc(n);
  size_t i, j;
  int status;

  gsl_matrix_memcpy(A, A0);
  printf("CASE %s %zu\n", name, n);
  for (i = 0; i < n; i++)
    for (j = 0; j < n; j++)
      printf("A %zu %zu %.17e\n", i, j, gsl_matrix_get(A0, i, j));
  for (i = 0; i < n; i++)
    printf("B %zu %.17e\n", i, gsl_vector_get(b, i));

  status = gsl_linalg_cholesky_decomp1(A);
  printf("STATUS %d\n", status);
  if (status == GSL_SUCCESS) {
    for (i = 0; i < n; i++)
      for (j = 0; j < n; j++)
        printf("LLT %zu %zu %.17e\n", i, j, gsl_matrix_get(A, i, j));
    gsl_linalg_cholesky_solve(A, b, x);
    for (i = 0; i < n; i++)
      printf("X %zu %.17e\n", i, gsl_vector_get(x, i));
    gsl_linalg_cholesky_invert(A);
    for (i = 0; i < n; i++)
      for (j = 0; j < n; j++)
        printf("INV %zu %zu %.17e\n", i, j, gsl_matrix_get(A, i, j));
  }
  printf("END %s\n", name);
  gsl_matrix_free(A);
  gsl_vector_free(x);
}

/* B^T B + shift * I for a pseudo-random n x n B: symmetric positive definite. */
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

static gsl_vector *rhs(size_t n)
{
  gsl_vector *b = gsl_vector_alloc(n);
  size_t i;
  for (i = 0; i < n; i++)
    gsl_vector_set(b, i, next_val());
  return b;
}

int main(void)
{
  gsl_set_error_handler_off();

  /* 1. Textbook 3x3 with the exact integer factor [[2],[6,1],[-8,5,3]]. */
  {
    double a[] = {4, 12, -16, 12, 37, -43, -16, -43, 98};
    gsl_matrix_view A = gsl_matrix_view_array(a, 3, 3);
    gsl_vector *b = rhs(3);
    emit("textbook3", &A.matrix, b);
    gsl_vector_free(b);
  }
  /* 2. Hilbert 6x6: SPD and badly conditioned (cond ~ 1.5e7). */
  {
    gsl_matrix *A = gsl_matrix_alloc(6, 6);
    size_t i, j;
    gsl_vector *b = rhs(6);
    for (i = 0; i < 6; i++)
      for (j = 0; j < 6; j++)
        gsl_matrix_set(A, i, j, 1.0 / (double) (i + j + 1));
    emit("hilbert6", A, b);
    gsl_matrix_free(A);
    gsl_vector_free(b);
  }
  /* 3. A pseudo-random 8x8 Gram matrix. */
  {
    gsl_matrix *A = gram(8, 0.1);
    gsl_vector *b = rhs(8);
    emit("gram8", A, b);
    gsl_matrix_free(A);
    gsl_vector_free(b);
  }
  /* 4. 24x24: the largest size GSL still factors with its Level-2 code. */
  {
    gsl_matrix *A = gram(24, 1.0);
    gsl_vector *b = rhs(24);
    emit("gram24", A, b);
    gsl_matrix_free(A);
    gsl_vector_free(b);
  }
  /* 5. 30x30: ABOVE CROSSOVER_CHOLESKY = 24, where GSL switches to its
   *    recursive Level-3 blocking and petir does not. */
  {
    gsl_matrix *A = gram(30, 1.0);
    gsl_vector *b = rhs(30);
    emit("gram30", A, b);
    gsl_matrix_free(A);
    gsl_vector_free(b);
  }
  /* 6. Symmetric but indefinite: decomp1 must refuse with GSL_EDOM. */
  {
    double a[] = {1, 2, 2, 1};
    gsl_matrix_view A = gsl_matrix_view_array(a, 2, 2);
    gsl_vector *b = rhs(2);
    emit("indefinite2", &A.matrix, b);
    gsl_vector_free(b);
  }
  return 0;
}
