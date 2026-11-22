/* Reference driver: GSL 2.8 symmetric tridiagonal solver
 * (gsl_linalg_solve_symm_tridiag, linalg/tridiag.c), for
 * petir::linalg::solve_symm_tridiag.
 *
 * Only the symmetric solver is ported. GSL's nonsymmetric (solve_tridiag_nonsym)
 * and cyclic (solve_cyc_tridiag, solve_cyc_tridiag_nonsym) routines have no
 * petir counterpart and are not emitted.
 *
 * Inputs are generated here from an integer LCG and divided by a power of two,
 * so every input is exact and no libm is involved. Each case writes its inputs
 * (as hex bit patterns) so the replaying Rust test reads them from this file.
 *
 * Output, per case:
 *   CASE <name> <n>
 *   D <i> <hex> <dec>          diagonal
 *   O <i> <hex> <dec>          off-diagonal (n-1 entries)
 *   B <i> <hex> <dec>          right-hand side
 *   STATUS <gsl status>
 *   X <i> <hex> <dec>          solution (only when STATUS 0)
 *   END <name>
 *
 * Build and run: reference-data/gsl/build_driver.sh (see README.md).
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_linalg.h>

static uint64_t state = 0x9E3779B97F4A7C15ULL;

/* Uniform in [0, 1), exact: 53 random bits divided by 2^53. */
static double unif(void)
{
  state = state * 6364136223846793005ULL + 1442695040888963407ULL;
  return (double)(state >> 11) / 9007199254740992.0;
}

static void hexdec(const char *tag, size_t i, double v)
{
  uint64_t bits;
  memcpy(&bits, &v, sizeof bits);
  printf("%s %zu %016llx %.17e\n", tag, i, (unsigned long long)bits, v);
}

static void emit(const char *name, size_t n, const double *d, const double *o,
                 const double *b)
{
  gsl_vector *diag = gsl_vector_alloc(n);
  gsl_vector *off = gsl_vector_alloc(n > 1 ? n - 1 : 1);
  gsl_vector *rhs = gsl_vector_alloc(n);
  gsl_vector *x = gsl_vector_alloc(n);
  size_t i;
  int status;

  for (i = 0; i < n; i++) {
    gsl_vector_set(diag, i, d[i]);
    gsl_vector_set(rhs, i, b[i]);
  }
  for (i = 0; i + 1 < n; i++)
    gsl_vector_set(off, i, o[i]);

  printf("CASE %s %zu\n", name, n);
  for (i = 0; i < n; i++)
    hexdec("D", i, d[i]);
  for (i = 0; i + 1 < n; i++)
    hexdec("O", i, o[i]);
  for (i = 0; i < n; i++)
    hexdec("B", i, b[i]);

  status = gsl_linalg_solve_symm_tridiag(diag, off, rhs, x);
  printf("STATUS %d\n", status);
  if (status == GSL_SUCCESS)
    for (i = 0; i < n; i++)
      hexdec("X", i, gsl_vector_get(x, i));
  printf("END %s\n", name);

  gsl_vector_free(diag);
  gsl_vector_free(off);
  gsl_vector_free(rhs);
  gsl_vector_free(x);
}

/* Diagonally dominant: diag in [3, 4), off-diagonals in [-1, 1), so each row
 * satisfies |d_i| > |o_{i-1}| + |o_i| with margin at least 1. */
static void random_dominant(size_t n, double *d, double *o, double *b)
{
  size_t i;
  for (i = 0; i < n; i++) {
    d[i] = 3.0 + unif();
    b[i] = 2.0 * unif() - 1.0;
  }
  for (i = 0; i + 1 < n; i++)
    o[i] = 2.0 * unif() - 1.0;
}

int main(void)
{
  gsl_set_error_handler_off();

  { double d[3], o[2], b[3]; random_dominant(3, d, o, b); emit("rand3", 3, d, o, b); }
  { double d[5], o[4], b[5]; random_dominant(5, d, o, b); emit("rand5", 5, d, o, b); }
  { double d[10], o[9], b[10]; random_dominant(10, d, o, b); emit("rand10", 10, d, o, b); }
  { double d[50], o[49], b[50]; random_dominant(50, d, o, b); emit("rand50", 50, d, o, b); }
  { double d[200], o[199], b[200]; random_dominant(200, d, o, b); emit("rand200", 200, d, o, b); }

  /* Near-singular: the Neumann Laplacian (diag 1,2,...,2,1; off -1) is exactly
   * singular with kernel (1,...,1). Lifting the last diagonal entry by 2^-40
   * leaves a last pivot of 2^-40 (exact in binary), so the condition number is
   * about 2^40 and the solution is large but finite. */
  {
    enum { N = 10 };
    double d[N], o[N - 1], b[N];
    size_t i;
    for (i = 0; i < N; i++) d[i] = 2.0;
    d[0] = 1.0;
    d[N - 1] = 1.0 + 9.094947017729282e-13; /* 1 + 2^-40 */
    for (i = 0; i + 1 < N; i++) o[i] = -1.0;
    for (i = 0; i < N; i++) b[i] = 1.0 + 0.125 * (double)i;
    emit("near_singular10", N, d, o, b);
  }

  /* Exact zero leading pivot: GSL returns GSL_EZERODIV (the STATUS line
   * carries the numeric code). The solution is not emitted. */
  {
    double d[3] = {0.0, 2.0, 2.0};
    double o[2] = {1.0, 1.0};
    double b[3] = {1.0, 2.0, 3.0};
    emit("zero_pivot3", 3, d, o, b);
  }

  return 0;
}
