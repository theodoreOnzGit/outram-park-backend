/* romberg_reference_driver.c -- compiled-GSL reference for
 * petir::integration::romberg (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_romberg (GSL 2.8, commit cf180cd7,
 * integration/romberg.c) and prints
 *   "ROMBERG <name> <status> <result> <neval> <counted>"
 * at 17 significant digits; <neval> is GSL's *neval, <counted> the integrand's
 * own call count (the two differ only on the argument errors, where *neval is
 * left untouched and printed as 0).
 *
 * Build: reference-data/gsl/build_gsl_family.sh romberg_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t counted;

static double sq(double x, void *p) { (void) p; counted++; return x * x; }
static double expx(double x, void *p) { (void) p; counted++; return exp(x); }
static double sinx(double x, void *p) { (void) p; counted++; return sin(x); }
static double gauss(double x, void *p) { (void) p; counted++; return exp(-x * x); }
static double kink(double x, void *p) { (void) p; counted++; return fabs(x - 0.3); }
static double sqrtx(double x, void *p) { (void) p; counted++; return sqrt(x); }
static double runge(double x, void *p) { (void) p; counted++; return 1.0 / (1.0 + 25.0 * x * x); }
static double poly8(double x, void *p) { (void) p; counted++; return pow(x, 8.0) - x; }
static double osc(double x, void *p) { (void) p; counted++; return cos(30.0 * x); }

typedef double (*fn_t)(double, void *);

static void run(const char *name, fn_t fn, double a, double b, double epsabs, double epsrel,
                size_t n)
{
  gsl_integration_romberg_workspace *w = gsl_integration_romberg_alloc(n);
  gsl_function F;
  double r = 0;
  size_t neval = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  counted = 0;
  status = gsl_integration_romberg(&F, a, b, epsabs, epsrel, &r, &neval, w);
  printf("ROMBERG %s %d %.17e %zu %zu\n", name, status, r, neval, counted);
  gsl_integration_romberg_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();
  run("sq", sq, 0.0, 3.0, 0.0, 1e-12, 10);
  run("exp", expx, 0.0, 1.0, 0.0, 1e-12, 20);
  run("exp_abs", expx, 0.0, 1.0, 1e-8, 0.0, 20);
  run("sin", sinx, 0.0, 3.14159265358979323846, 0.0, 1e-14, 20);
  run("gauss", gauss, -3.0, 3.0, 0.0, 1e-13, 25);
  run("runge", runge, -1.0, 1.0, 0.0, 1e-10, 30);
  run("runge_n6", runge, -1.0, 1.0, 0.0, 1e-10, 6);
  run("kink_n6", kink, 0.0, 1.0, 0.0, 1e-14, 6);
  run("kink_n12", kink, 0.0, 1.0, 0.0, 1e-8, 12);
  run("sqrt_n14", sqrtx, 0.0, 1.0, 0.0, 1e-9, 14);
  run("poly8", poly8, -1.0, 2.0, 0.0, 1e-13, 12);
  run("osc", osc, 0.0, 5.0, 0.0, 1e-10, 16);
  run("reversed", expx, 1.0, 0.0, 0.0, 1e-12, 20);
  run("zero_width", expx, 0.5, 0.5, 1e-12, 0.0, 10);
  run("n1", sq, 0.0, 1.0, 0.0, 1e-8, 1);
  run("n2", sq, 0.0, 1.0, 0.0, 1e-8, 2);
  run("n31_converges", expx, 0.0, 1.0, 0.0, 1e-12, 31);
  run("eps_zero_n8", expx, 0.0, 1.0, 0.0, 0.0, 8);
  run("neg_epsabs", sq, 0.0, 1.0, -1.0, 0.0, 5);
  run("neg_epsrel", sq, 0.0, 1.0, 0.0, -1.0, 5);
  return 0;
}
