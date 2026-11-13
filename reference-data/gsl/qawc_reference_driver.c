/* qawc_reference_driver.c -- compiled-GSL reference for
 * petir::integration::qawc (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qawc (GSL 2.8, commit cf180cd7, integration/qawc.c)
 * with a workspace of exactly `limit` entries and prints
 *   "QAWC <name> <status> <result> <abserr> <size> <neval>"
 * at 17 significant digits.
 *
 * Build: reference-data/gsl/build_gsl_family.sh qawc_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double one(double x, void *p) { (void) p; (void) x; neval++; return 1.0; }
static double lin(double x, void *p) { (void) p; neval++; return x; }
static double expx(double x, void *p) { (void) p; neval++; return exp(x); }
static double cosx(double x, void *p) { (void) p; neval++; return cos(3.0 * x); }
static double sqabs(double x, void *p) { (void) p; neval++; return sqrt(fabs(x)); }
static double gauss(double x, void *p) { (void) p; neval++; return exp(-x * x); }
static double peak(double x, void *p) { (void) p; neval++; return 1.0 / (1e-3 + (x - 0.4) * (x - 0.4)); }
static double kink(double x, void *p) { (void) p; neval++; return fabs(x - 0.25); }
static double rough(double x, void *p) { (void) p; neval++; return sin(200.0 * x) + 2.0; }

typedef double (*fn_t)(double, void *);

static void run(const char *name, fn_t fn, double a, double b, double c, double epsabs,
                double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  status = gsl_integration_qawc(&F, a, b, c, epsabs, epsrel, limit, w, &r, &e);
  printf("QAWC %s %d %.17e %.17e %zu %zu\n", name, status, r, e, w->size, neval);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();
  run("one_centre", one, -1.0, 2.0, 0.0, 0.0, 1e-10, 100);
  run("one_reversed", one, 2.0, -1.0, 0.0, 0.0, 1e-10, 100);
  run("one_outside", one, 0.0, 1.0, 5.0, 0.0, 1e-10, 100);
  run("one_near", one, 0.0, 1.0, 1.05, 0.0, 1e-10, 100);
  run("lin_centre", lin, -1.0, 1.0, 0.3, 0.0, 1e-10, 100);
  run("exp_centre", expx, 0.0, 2.0, 0.7, 0.0, 1e-10, 100);
  run("exp_off_centre", expx, 0.0, 2.0, 1.99, 0.0, 1e-10, 100);
  run("cos3", cosx, -2.0, 3.0, 0.1, 0.0, 1e-8, 100);
  run("sqabs_cusp", sqabs, -1.0, 1.0, 0.0, 0.0, 1e-8, 100);
  run("sqabs_off", sqabs, -1.0, 1.0, 0.3, 0.0, 1e-12, 100);
  run("gauss_wide", gauss, -5.0, 5.0, 1.0, 0.0, 1e-10, 100);
  run("peak_near_c", peak, 0.0, 1.0, 0.45, 0.0, 1e-8, 200);
  run("kink_at_c", kink, 0.0, 1.0, 0.25, 0.0, 1e-9, 100);
  run("kink_off_c", kink, 0.0, 1.0, 0.6, 0.0, 1e-12, 100);
  run("rough_limit3", rough, 0.0, 2.0, 0.7, 0.0, 1e-12, 3);
  run("rough", rough, 0.0, 2.0, 0.7, 0.0, 1e-9, 400);
  run("limit1", expx, 0.0, 2.0, 0.7, 0.0, 1e-14, 1);
  run("abs_only", expx, 0.0, 2.0, 0.7, 1e-8, 0.0, 100);
  run("c_eq_a", expx, 0.0, 2.0, 0.0, 0.0, 1e-8, 100);
  run("c_eq_b", expx, 0.0, 2.0, 2.0, 0.0, 1e-8, 100);
  run("badtol", expx, 0.0, 2.0, 0.7, 0.0, 1e-17, 100);
  run("tight_roundoff", expx, 0.0, 2.0, 0.7, 0.0, 1e-15, 200);
  return 0;
}
