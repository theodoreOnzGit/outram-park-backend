/* fixed_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{FixedType, FixedWorkspace} (gh:#1340, round 3).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_fixed_alloc and gsl_integration_fixed (GSL 2.8,
 * commit cf180cd7, integration/fixed.c and the nine type files) for every
 * weight type over several parameter sets and orders, and prints
 *   "FX <tag> <kind> <n> <a> <b> <alpha> <beta> <ok> [I1 I2 I3 I4 x0 w0 x1 w1 ...]"
 * at 17 significant digits; ok = 0 means the allocation returned NULL (and
 * nothing follows). The four integrals are sum w_i f(x_i) for
 *   f1 = 1, f2 = x*x, f3 = x*x*x*x*x, f4 = 1 / (1 + x*x),
 * chosen to call no libm function, so the only libm calls in the program are
 * GSL's own: pow in fixed_compute and jacobi.c, and what gsl_sf_gamma calls.
 *
 * Build: reference-data/gsl/build_gsl_nocfg.sh <build-dir> \
 *          reference-data/gsl/fixed_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static double f1(double x, void *p) { (void) p; (void) x; return 1.0; }
static double f2(double x, void *p) { (void) p; return x * x; }
static double f3(double x, void *p) { (void) p; return x * x * x * x * x; }
static double f4(double x, void *p) { (void) p; return 1.0 / (1.0 + x * x); }

static void row(const char *tag, const char *kind, const gsl_integration_fixed_type *type,
                size_t n, double a, double b, double alpha, double beta)
{
  gsl_integration_fixed_workspace *w = gsl_integration_fixed_alloc(type, n, a, b, alpha, beta);
  printf("FX %s %s %zu %.17g %.17g %.17g %.17g %d", tag, kind, n, a, b, alpha, beta, w != NULL);
  if (w != NULL) {
    gsl_function F;
    double r[4];
    size_t i;
    F.params = NULL;
    F.function = f1; gsl_integration_fixed(&F, &r[0], w);
    F.function = f2; gsl_integration_fixed(&F, &r[1], w);
    F.function = f3; gsl_integration_fixed(&F, &r[2], w);
    F.function = f4; gsl_integration_fixed(&F, &r[3], w);
    for (i = 0; i < 4; i++) printf(" %.17e", r[i]);
    for (i = 0; i < n; i++) printf(" %.17e %.17e", w->x[i], w->weights[i]);
    gsl_integration_fixed_free(w);
  }
  printf("\n");
}

int main(void)
{
  static const size_t ns[] = { 1, 2, 3, 4, 5, 8, 10, 16, 25, 40 };
  const size_t nn = sizeof ns / sizeof ns[0];
  size_t k;
  gsl_set_error_handler_off();

  for (k = 0; k < nn; k++) {
    size_t n = ns[k];
    /* Legendre */
    row("m1_1", "legendre", gsl_integration_fixed_legendre, n, -1.0, 1.0, 0.0, 0.0);
    row("0_3", "legendre", gsl_integration_fixed_legendre, n, 0.0, 3.0, 0.0, 0.0);
    row("rev", "legendre", gsl_integration_fixed_legendre, n, 2.0, -1.0, 0.0, 0.0);
    /* Chebyshev, Chebyshev2 */
    row("m1_1", "chebyshev", gsl_integration_fixed_chebyshev, n, -1.0, 1.0, 0.0, 0.0);
    row("h_4", "chebyshev", gsl_integration_fixed_chebyshev, n, 0.5, 4.0, 0.0, 0.0);
    row("m1_1", "chebyshev2", gsl_integration_fixed_chebyshev2, n, -1.0, 1.0, 0.0, 0.0);
    row("0_2", "chebyshev2", gsl_integration_fixed_chebyshev2, n, 0.0, 2.0, 0.0, 0.0);
    /* Gegenbauer */
    row("a0", "gegenbauer", gsl_integration_fixed_gegenbauer, n, -1.0, 1.0, 0.0, 0.0);
    row("a.5", "gegenbauer", gsl_integration_fixed_gegenbauer, n, -1.0, 1.0, 0.5, 0.0);
    row("a-.5", "gegenbauer", gsl_integration_fixed_gegenbauer, n, 0.0, 2.0, -0.5, 0.0);
    row("a2.5", "gegenbauer", gsl_integration_fixed_gegenbauer, n, -2.0, 3.0, 2.5, 0.0);
    /* Jacobi */
    row("0_0", "jacobi", gsl_integration_fixed_jacobi, n, -1.0, 1.0, 0.0, 0.0);
    row("1_2", "jacobi", gsl_integration_fixed_jacobi, n, -1.0, 1.0, 1.0, 2.0);
    row("-.5_.5", "jacobi", gsl_integration_fixed_jacobi, n, 0.5, 3.0, -0.5, 0.5);
    row("3.5_-.25", "jacobi", gsl_integration_fixed_jacobi, n, -1.0, 1.0, 3.5, -0.25);
    /* Laguerre */
    row("a0", "laguerre", gsl_integration_fixed_laguerre, n, 0.0, 1.0, 0.0, 0.0);
    row("a1.5", "laguerre", gsl_integration_fixed_laguerre, n, 1.0, 2.5, 1.5, 0.0);
    row("a-.5", "laguerre", gsl_integration_fixed_laguerre, n, 0.0, 1.0, -0.5, 0.0);
    /* Hermite */
    row("a0", "hermite", gsl_integration_fixed_hermite, n, 0.0, 1.0, 0.0, 0.0);
    row("a1", "hermite", gsl_integration_fixed_hermite, n, -1.0, 4.0, 1.0, 0.0);
    row("a2.5", "hermite", gsl_integration_fixed_hermite, n, 0.0, 1.0, 2.5, 0.0);
    /* Exponential */
    row("a0", "exponential", gsl_integration_fixed_exponential, n, 0.0, 1.0, 0.0, 0.0);
    row("a1.5", "exponential", gsl_integration_fixed_exponential, n, -2.0, 3.0, 1.5, 0.0);
    row("a-.5", "exponential", gsl_integration_fixed_exponential, n, 0.0, 1.0, -0.5, 0.0);
    /* Rational: needs beta < alpha + beta + 2n < 0 */
    row("a0", "rational", gsl_integration_fixed_rational, n, 1.0, 2.0, 0.0, -(2.0 * n) - 2.5);
    row("a1", "rational", gsl_integration_fixed_rational, n, 0.5, 1.5, 1.0, -(2.0 * n + 1.0) - 3.5);
    row("a0_close", "rational", gsl_integration_fixed_rational, n, 1.0, 2.0, 0.0, -(2.0 * n) - 1.0e-3);
  }

  /* refusals */
  row("n0", "legendre", gsl_integration_fixed_legendre, 0, -1.0, 1.0, 0.0, 0.0);
  row("narrow", "legendre", gsl_integration_fixed_legendre, 4, 1.0, 1.0 + 1e-16, 0.0, 0.0);
  row("reversed", "chebyshev", gsl_integration_fixed_chebyshev, 4, 1.0, -1.0, 0.0, 0.0);
  row("reversed", "chebyshev2", gsl_integration_fixed_chebyshev2, 4, 1.0, -1.0, 0.0, 0.0);
  row("alpha-1", "gegenbauer", gsl_integration_fixed_gegenbauer, 4, -1.0, 1.0, -1.0, 0.0);
  row("reversed", "gegenbauer", gsl_integration_fixed_gegenbauer, 4, 1.0, -1.0, 0.0, 0.0);
  row("alpha-1", "jacobi", gsl_integration_fixed_jacobi, 4, -1.0, 1.0, -1.0, 0.0);
  row("beta-1", "jacobi", gsl_integration_fixed_jacobi, 4, -1.0, 1.0, 0.0, -1.0);
  row("b0", "laguerre", gsl_integration_fixed_laguerre, 4, 0.0, 0.0, 0.0, 0.0);
  row("alpha-1", "laguerre", gsl_integration_fixed_laguerre, 4, 0.0, 1.0, -1.0, 0.0);
  row("b-1", "hermite", gsl_integration_fixed_hermite, 4, 0.0, -1.0, 0.0, 0.0);
  row("alpha-1", "hermite", gsl_integration_fixed_hermite, 4, 0.0, 1.0, -1.0, 0.0);
  row("reversed", "exponential", gsl_integration_fixed_exponential, 4, 1.0, 0.0, 0.0, 0.0);
  row("alpha-1", "exponential", gsl_integration_fixed_exponential, 4, 0.0, 1.0, -1.0, 0.0);
  row("beta_pos", "rational", gsl_integration_fixed_rational, 4, 1.0, 2.0, 0.0, 1.0);
  row("abn_pos", "rational", gsl_integration_fixed_rational, 4, 1.0, 2.0, 0.0, -3.0);
  row("neg_ab", "rational", gsl_integration_fixed_rational, 4, -1.0, 0.5, 0.0, -20.0);
  row("alpha-1", "rational", gsl_integration_fixed_rational, 4, 1.0, 2.0, -1.0, -20.0);
  return 0;
}
