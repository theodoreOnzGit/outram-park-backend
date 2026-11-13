/* qng_reference_driver.c -- compiled-GSL reference for
 * petir::integration::qng (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qng (GSL 2.8, commit cf180cd7, integration/qng.c) and
 * prints, per case, the status, result, error estimate and number of
 * integrand evaluations (GSL's own *neval, and a count made by the integrand)
 * at 17 significant digits:
 *   "QNG <name> <status> <result> <abserr> <neval> <counted>"
 *
 * Build: see build_gsl_family.sh in this directory.
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t counted;

static double cubic(double x, void *p) { (void) p; counted++; return x * x * x; }
static double expx(double x, void *p) { (void) p; counted++; return exp(x); }
static double cos10(double x, void *p) { (void) p; counted++; return cos(10.0 * x); }
static double sqrtx(double x, void *p) { (void) p; counted++; return sqrt(x); }
static double invsqrt(double x, void *p) { (void) p; counted++; return 1.0 / sqrt(x); }
static double gauss(double x, void *p) { (void) p; counted++; return exp(-x * x); }
static double runge(double x, void *p) { (void) p; counted++; return 1.0 / (1.0 + 25.0 * x * x); }
static double kink(double x, void *p) { (void) p; counted++; return fabs(x - 0.3); }
static double logx(double x, void *p) { (void) p; counted++; return log(x); }
static double one(double x, void *p) { (void) p; (void) x; counted++; return 1.0; }
static double sin5(double x, void *p) { (void) p; counted++; return sin(5.0 * x) * x; }

typedef double (*fn_t)(double, void *);

static void run(const char *name, fn_t fn, double a, double b, double epsabs, double epsrel)
{
  gsl_function F;
  double r = 0, e = 0;
  size_t neval = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  counted = 0;
  status = gsl_integration_qng(&F, a, b, epsabs, epsrel, &r, &e, &neval);
  printf("QNG %s %d %.17e %.17e %zu %zu\n", name, status, r, e, neval, counted);
}

int main(void)
{
  gsl_set_error_handler_off();
  run("cubic", cubic, 0.0, 2.0, 0.0, 1e-10);
  run("exp_1e10", expx, 0.0, 1.0, 0.0, 1e-10);
  run("exp_1e13", expx, 0.0, 1.0, 0.0, 1e-13);
  run("exp_1e15", expx, 0.0, 1.0, 0.0, 1e-15);
  run("exp_abs", expx, 0.0, 1.0, 1e-9, 0.0);
  run("cos10_1e8", cos10, 0.0, 1.0, 0.0, 1e-8);
  run("cos10_1e12", cos10, 0.0, 1.0, 0.0, 1e-12);
  run("cos10_1e6", cos10, 0.0, 1.0, 0.0, 1e-6);
  run("sqrt_1e14", sqrtx, 0.0, 1.0, 0.0, 1e-14);
  run("sqrt_1e4", sqrtx, 0.0, 1.0, 0.0, 1e-4);
  run("invsqrt_1e8", invsqrt, 0.0, 1.0, 0.0, 1e-8);
  run("gauss_m3_3", gauss, -3.0, 3.0, 0.0, 1e-9);
  run("gauss_m3_3_1e12", gauss, -3.0, 3.0, 1e-12, 1e-12);
  run("runge", runge, -1.0, 1.0, 0.0, 1e-8);
  run("runge_1e3", runge, -1.0, 1.0, 0.0, 1e-3);
  run("kink", kink, 0.0, 1.0, 0.0, 1e-8);
  run("log_0_1", logx, 0.0, 1.0, 0.0, 1e-6);
  run("log_1_2", logx, 1.0, 2.0, 0.0, 1e-12);
  run("reversed", expx, 1.0, 0.0, 0.0, 1e-10);
  run("zero_length", expx, 0.5, 0.5, 0.0, 1e-10);
  run("const_one", one, 0.0, 3.0, 0.0, 1e-10);
  run("xsin5", sin5, 0.0, 4.0, 0.0, 1e-10);
  run("xsin5_wide", sin5, -10.0, 10.0, 1e-3, 0.0);
  run("badtol_zero", cubic, 0.0, 1.0, 0.0, 0.0);
  run("badtol_tiny", cubic, 0.0, 1.0, 0.0, 1e-15);
  run("badtol_neg", cubic, 0.0, 1.0, -1.0, 1e-16);
  run("tol_abs_only", cubic, 0.0, 1.0, 1e-8, 0.0);
  return 0;
}
