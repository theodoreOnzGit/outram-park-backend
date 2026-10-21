/* qags_reference_driver.c -- compiled-GSL reference for petir::integration::qags.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qags (GSL 2.8, commit cf180cd7, integration/qags.c)
 * on a set of integrands chosen to reach every exit of the routine: the
 * first-rule return, plain convergence, convergence by extrapolation, the
 * iteration limit, round-off, and the divergence / failure branches. For each
 * case it prints the status, result, error estimate, number of sub-intervals
 * and number of integrand evaluations, all to 17 significant digits.
 *
 * Build (see README.md, "qags-gsl-2.8-reference.txt"):
 *   G=crates/petir/upstream_source/GSL
 *   mkdir -p /tmp/gslqags/gsl && cd /tmp/gslqags
 *   for h in $G/*.h $G/integration/*.h $G/err/*.h $G/sys/*.h; do
 *     ln -sf "$h" gsl/$(basename $h); done
 *   printf '#define HAVE_INLINE 1\n#define RETURN_IF_NULL(x) if (!x) { return ; }\n' > config.h
 *   gcc -O2 -I. -I./gsl -I$G/integration -o qagsdriver \
 *     <this dir>/qags_reference_driver.c $G/integration/{qags,qk21,qk15,qk,workspace}.c \
 *     $G/err/{error,stream}.c -lm
 *   ./qagsdriver > qags-gsl-2.8-reference.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double f1_alpha26(double x, void *p)
{ (void) p; neval++; return pow(x, 2.6) * log(1 / x); }
static double f11_alpha2(double x, void *p)
{ (void) p; neval++; return pow(log(1 / x), 2.0 - 1.0); }
static double inv_sqrt(double x, void *p)
{ (void) p; neval++; return 1.0 / sqrt(x); }
static double log_x(double x, void *p)
{ (void) p; neval++; return log(x); }
static double x_pow_m09(double x, void *p)
{ (void) p; neval++; return pow(x, -0.9); }
static double runge(double x, void *p)
{ (void) p; neval++; return 1.0 / (1.0 + 25.0 * x * x); }
static double peak(double x, void *p)
{ (void) p; neval++; return 1.0 / ((x - 0.3) * (x - 0.3) + 1e-8); }
static double inv_x(double x, void *p)
{ (void) p; neval++; return 1.0 / x; }
static double sin_x(double x, void *p)
{ (void) p; neval++; return sin(x); }
static double osc(double x, void *p)
{ (void) p; neval++; return cos(100.0 * x) * log(x); }
/* A plume-shine-like point kernel along x: build-up (1 + k mu r), 1/(4 pi r^2)
 * and exp(-mu r) about a receptor 0.1 m off the line. */
static double kernel(double x, void *p)
{
  (void) p; neval++;
  double r2 = (x - 500.0) * (x - 500.0) + 0.01;
  double r = sqrt(r2);
  return (1.0 + 1.2 * 0.01 * r) / (4.0 * M_PI * r2) * exp(-0.01 * r);
}

static double inv_x2(double x, void *p)
{ (void) p; neval++; return 1.0 / (x * x); }
static double sin_inv_x_over_x(double x, void *p)
{ (void) p; neval++; return sin(1.0 / x) / x; }
static double interior_sqrt(double x, void *p)
{ (void) p; neval++; return x == 0 ? 0 : 1.0 / sqrt(fabs(x - 0.33333)); }
static double pole(double x, void *p)
{ (void) p; neval++; return 1.0 / (x - 0.5); }
static double log_x_over_x(double x, void *p)
{ (void) p; neval++; return log(x) / x; }

static void run(const char *name, double (*fn)(double, void *), double a, double b,
                double epsabs, double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn; F.params = 0;
  neval = 0;
  status = gsl_integration_qags(&F, a, b, epsabs, epsrel, limit, w, &r, &e);
  printf("QAGS %s %d %.17e %.17e %zu %zu\n", name, status, r, e, w->size, neval);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();
  printf("CASE qags\n");
  run("f1", f1_alpha26, 0.0, 1.0, 0.0, 1e-10, 1000);
  run("f1_reverse", f1_alpha26, 1.0, 0.0, 0.0, 1e-10, 1000);
  run("f11", f11_alpha2, 1.0, 1000.0, 1e-7, 0.0, 1000);
  run("inv_sqrt", inv_sqrt, 0.0, 1.0, 0.0, 1e-10, 200);
  run("log_x", log_x, 0.0, 1.0, 0.0, 1e-12, 200);
  run("x_pow_m09", x_pow_m09, 0.0, 1.0, 0.0, 1e-10, 200);
  run("runge", runge, -1.0, 1.0, 0.0, 1e-10, 200);
  run("sin_first_rule", sin_x, 0.0, 1.0, 1e-6, 1e-6, 50);
  run("peak_limit50", peak, 0.0, 1.0, 0.0, 1e-10, 50);
  run("peak_limit500", peak, 0.0, 1.0, 0.0, 1e-10, 500);
  run("inv_x_divergent", inv_x, 0.0, 1.0, 0.0, 1e-10, 200);
  run("osc_limit3", osc, 0.0, 1.0, 0.0, 1e-10, 3);
  run("osc", osc, 0.0, 1.0, 0.0, 1e-10, 500);
  run("kernel_1e-3", kernel, 400.0, 600.0, 1.49e-3, 1.49e-3, 50);
  run("kernel_1e-8", kernel, 400.0, 600.0, 0.0, 1.49e-8, 50);
  /* the error exits (status: 22 EDIVERGE, 18 EROUND, 21 ESING, 11 EMAXITER,
     13 EBADTOL) */
  run("inv_x2_diverge", inv_x2, 0.0, 1.0, 0.0, 1e-6, 50);
  run("sin_inv_x_diverge", sin_inv_x_over_x, 0.0, 1.0, 0.0, 1e-6, 1000);
  run("interior_sqrt_round", interior_sqrt, 0.0, 1.0, 0.0, 1e-10, 50);
  run("pole_sing", pole, 0.0, 1.0, 0.0, 1e-10, 1000);
  run("log_x_over_x_maxiter", log_x_over_x, 0.0, 1.0, 0.0, 1e-10, 50);
  run("badtol", runge, 0.0, 1.0, 0.0, 1e-14, 50);
  printf("END qags\n");
  return 0;
}
