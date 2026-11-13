/* qagp_reference_driver.c -- compiled-GSL reference for
 * petir::integration::qagp (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qagp (GSL 2.8, commit cf180cd7, integration/qagp.c)
 * with a workspace of exactly `limit` entries (as petir allocates) and prints
 *   "QAGP <name> <status> <result> <abserr> <size> <neval>"
 * at 17 significant digits. Integrands use sqrt, exp, log, cos, pow only.
 *
 * Build: reference-data/gsl/build_gsl_family.sh qagp_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double absk(double x, void *p) { (void) p; neval++; return fabs(x - 1.0); }
static double lnabs(double x, void *p) { (void) p; neval++; return log(fabs(x - 1.0)); }
static double invsq(double x, void *p) { (void) p; neval++; return 1.0 / sqrt(fabs(x - 1.0)); }
static double step(double x, void *p) { (void) p; neval++; return x < 1.0 ? 1.0 : 2.0; }
static double peaks(double x, void *p)
{ (void) p; neval++; return 1.0 / (1e-4 + (x - 0.3) * (x - 0.3)) + 1.0 / (1e-4 + (x - 0.7) * (x - 0.7)); }
static double expm(double x, void *p) { (void) p; neval++; return exp(-x); }
static double wavy(double x, void *p) { (void) p; neval++; return cos(40.0 * x) / sqrt(fabs(x - 5.5) + 1e-3); }
static double pw09(double x, void *p) { (void) p; neval++; return pow(fabs(x - 0.5), -0.9); }
static double inv(double x, void *p) { (void) p; neval++; return 1.0 / x; }
static double sqabs(double x, void *p) { (void) p; neval++; return sqrt(fabs(x - 1.0)); }
static double cos50(double x, void *p) { (void) p; neval++; return cos(50.0 * x); }
static double invsqx(double x, void *p) { (void) p; neval++; return 1.0 / sqrt(x); }
static double sharp(double x, void *p)
{ (void) p; neval++; return exp(-1e4 * (x - 6.0) * (x - 6.0)) + exp(-1e4 * (x - 9.0) * (x - 9.0)); }

typedef double (*fn_t)(double, void *);

static void run(const char *name, fn_t fn, double *pts, size_t npts, double epsabs,
                double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  status = gsl_integration_qagp(&F, pts, npts, epsabs, epsrel, limit, w, &r, &e);
  printf("QAGP %s %d %.17e %.17e %zu %zu\n", name, status, r, e, w->size, neval);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  double p3[3] = {0.0, 1.0, 3.0};
  double p3b[3] = {0.0, 1.0, 2.0};
  double p4[4] = {0.0, 0.3, 0.7, 1.0};
  double p2[2] = {0.0, 1.0};
  double p12[12] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11};
  double p3c[3] = {0.0, 0.5, 1.0};
  double p3d[3] = {-1.0, 0.0, 1.0};
  double pbad[3] = {0.0, 2.0, 1.0};
  double pzero[3] = {0.0, 0.0, 1.0};
  double p12b[12] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11.5};
  double p2b[2] = {0.0, 2.0};
  gsl_set_error_handler_off();
  run("abs_kink", absk, p3, 3, 0.0, 1e-10, 50);
  run("ln_abs", lnabs, p3b, 3, 0.0, 1e-10, 100);
  run("inv_sqrt_abs", invsq, p3, 3, 0.0, 1e-10, 100);
  run("step", step, p3b, 3, 0.0, 1e-10, 50);
  run("two_peaks", peaks, p4, 4, 0.0, 1e-8, 200);
  run("exp_12pts", expm, p12, 12, 0.0, 1e-13, 100);
  run("wavy_12pts", wavy, p12b, 12, 0.0, 1e-9, 400);
  run("sharp_12pts", sharp, p12, 12, 1e-12, 1e-12, 400);
  run("pow09_limit4", pw09, p3c, 3, 0.0, 1e-10, 4);
  run("pow09", pw09, p3c, 3, 0.0, 1e-8, 200);
  run("pole_diverge", inv, p3d, 3, 0.0, 1e-8, 100);
  run("sqrt_abs", sqabs, p3b, 3, 0.0, 1e-10, 100);
  run("cos50", cos50, p3b, 3, 0.0, 1e-10, 100);
  run("cos50_limit5", cos50, p3b, 3, 0.0, 1e-13, 5);
  run("inv_sqrt_x", invsqx, p2, 2, 0.0, 1e-10, 100);
  run("inv_sqrt_x_tight", invsqx, p2, 2, 0.0, 1e-14, 30);
  run("unordered", absk, pbad, 3, 0.0, 1e-10, 50);
  run("npts_gt_limit", absk, p3, 3, 0.0, 1e-10, 2);
  run("badtol", absk, p3, 3, 0.0, 1e-16, 50);
  run("zero_width", absk, pzero, 3, 0.0, 1e-10, 50);
  run("abs_only", absk, p2b, 2, 1e-9, 0.0, 50);
  run("limit2_two_pts", pw09, p2, 2, 0.0, 1e-12, 2);
  return 0;
}
