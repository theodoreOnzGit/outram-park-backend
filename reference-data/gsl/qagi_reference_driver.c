/* qagi_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{qagi, qagiu, qagil} (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qagi, _qagiu and _qagil (GSL 2.8, commit cf180cd7,
 * integration/qags.c) and prints, per case, the status, result, error
 * estimate, workspace size and number of integrand evaluations at 17
 * significant digits:
 *   "QAGI <name> <kind> <status> <result> <abserr> <size> <neval>"
 * with kind in {i, u, l}.
 *
 * Integrands are rational or use sqrt, exp, pow only; the replaying test is a
 * std build, whose f64 methods reach the same glibc as this driver.
 *
 * Build (GSL 2.8 built from ampl/gsl@cf180cd7, see README.md):
 *   gcc -O2 -I$G -o d qagi_reference_driver.c $G/.libs/libgsl.a \
 *       $G/cblas/.libs/libgslcblas.a -lm
 *   ./d > qagi-gsl-2.8-reference.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double gauss(double x, void *p) { (void) p; neval++; return exp(-x * x); }
static double cauchy(double x, void *p) { (void) p; neval++; return 1.0 / (1.0 + x * x); }
static double quartic(double x, void *p) { (void) p; neval++; return 1.0 / (1.0 + x * x * x * x); }
static double peak30(double x, void *p)
{ (void) p; neval++; return 1.0 / (1.0 + (x - 30.0) * (x - 30.0) * 100.0); }
static double inv_sqrt1(double x, void *p) { (void) p; neval++; return 1.0 / sqrt(1.0 + x * x); }
static double inv_x2(double x, void *p) { (void) p; neval++; return 1.0 / (x * x); }
static double x_m15(double x, void *p) { (void) p; neval++; return 1.0 / (x * sqrt(x)); }
static double x_m11(double x, void *p) { (void) p; neval++; return pow(x, -1.1); }
static double expm(double x, void *p) { (void) p; neval++; return exp(-x); }
static double expp(double x, void *p) { (void) p; neval++; return exp(x); }
static double inv_x(double x, void *p) { (void) p; neval++; return 1.0 / x; }
static double ln_x2(double x, void *p) { (void) p; neval++; return log(x) / (x * x); }

typedef double (*fn_t)(double, void *);

static void run(const char *name, char kind, fn_t fn, double lim, double epsabs,
                double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_function F;
  double r = 0, e = 0;
  int status = 0;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  if (kind == 'i') status = gsl_integration_qagi(&F, epsabs, epsrel, limit, w, &r, &e);
  if (kind == 'u') status = gsl_integration_qagiu(&F, lim, epsabs, epsrel, limit, w, &r, &e);
  if (kind == 'l') status = gsl_integration_qagil(&F, lim, epsabs, epsrel, limit, w, &r, &e);
  printf("QAGI %s %c %d %.17e %.17e %zu %zu\n", name, kind, status, r, e, w->size, neval);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();
  run("gauss", 'i', gauss, 0, 0.0, 1e-10, 200);
  run("cauchy", 'i', cauchy, 0, 0.0, 1e-10, 200);
  run("quartic", 'i', quartic, 0, 0.0, 1e-12, 200);
  run("peak30", 'i', peak30, 0, 0.0, 1e-8, 500);
  run("peak30_limit3", 'i', peak30, 0, 0.0, 1e-12, 3);
  run("inv_sqrt1_diverge", 'i', inv_sqrt1, 0, 0.0, 1e-8, 100);
  run("cauchy_badtol", 'i', cauchy, 0, 0.0, 1e-14, 50);
  run("inv_x2_from1", 'u', inv_x2, 1.0, 0.0, 1e-10, 200);
  run("x_m15_from1", 'u', x_m15, 1.0, 0.0, 1e-10, 200);
  run("x_m11_from1", 'u', x_m11, 1.0, 0.0, 1e-6, 500);
  run("exp_from2", 'u', expm, 2.0, 0.0, 1e-10, 200);
  run("cauchy_from0", 'u', cauchy, 0.0, 1e-12, 0.0, 200);
  run("inv_x_from1_diverge", 'u', inv_x, 1.0, 0.0, 1e-8, 100);
  run("ln_x2_from1", 'u', ln_x2, 1.0, 0.0, 1e-10, 200);
  run("exp_to_m1", 'l', expp, -1.0, 0.0, 1e-10, 200);
  run("cauchy_to0", 'l', cauchy, 0.0, 0.0, 1e-10, 200);
  run("inv_x2_to_m1", 'l', inv_x2, -1.0, 0.0, 1e-10, 200);
  run("gauss_to_m3", 'l', gauss, -3.0, 1e-9, 1e-9, 100);
  return 0;
}
