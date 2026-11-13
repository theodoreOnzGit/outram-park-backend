/* interp_cspline_poly_reference_driver.c -- compiled-GSL reference for
 * petir::interp_cspline_poly (gh:#1341).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls GSL 2.8 (commit cf180cd7) gsl_interp_eval, _eval_deriv, _eval_deriv2
 * and _eval_integ for gsl_interp_cspline_periodic and gsl_interp_polynomial
 * (NO accelerator). Same row formats as interp_piecewise_reference_driver.c:
 *   "T <table> <n> <periodic> x0 y0 x1 y1 ..."
 *   "INIT <method> <table> <status>"       gsl_interp_init's return value
 *   "<method> <table> E  <x> <y>"          (also D, D2, and I <a> <b> <int>)
 * 241 evaluation points per table plus every knot; integrals over every
 * ordered pair of a 21-point sub-grid. Tables:
 *   periodic: per12 (sine, y[11] = y[0]), per3 (closed-form 2x2), per4, per5,
 *             nonper (y[0] != y[n-1], which GSL does not check), per2 (n = 2:
 *             GSL's cyclic solver refuses, INIT status != 0)
 *   polynomial: poly3, poly5, poly7 (non-uniform, cp-shaped), runge9, cubic6
 *             (data on a cubic; the polynomial reproduces it)
 *
 * Build: reference-data/gsl/build_gsl_family.sh interp_cspline_poly_reference_driver.c out.txt
 * (the polynomial integral calls pow, so it reaches glibc here; the replaying
 * test compares with PETIR's pow and counts the differences.)
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_interp.h>

static void run(const char *mname, const gsl_interp_type *T, const char *tname,
                const double *xa, const double *ya, int n)
{
  gsl_interp *in = gsl_interp_alloc(T, n);
  int i, j, np = 241, ng = 21, st;
  double x0 = xa[0], x1 = xa[n - 1];
  st = gsl_interp_init(in, xa, ya, n);
  printf("INIT %s %s %d\n", mname, tname, st);
  if (st != 0) { gsl_interp_free(in); return; }
  for (i = 0; i < np; i++) {
    double x = (i == np - 1) ? x1 : x0 + (x1 - x0) * i / (np - 1);
    printf("%s %s E %.17g %.17g\n", mname, tname, x, gsl_interp_eval(in, xa, ya, x, NULL));
    printf("%s %s D %.17g %.17g\n", mname, tname, x, gsl_interp_eval_deriv(in, xa, ya, x, NULL));
    printf("%s %s D2 %.17g %.17g\n", mname, tname, x, gsl_interp_eval_deriv2(in, xa, ya, x, NULL));
  }
  for (i = 0; i < n; i++) {
    double x = xa[i];
    printf("%s %s E %.17g %.17g\n", mname, tname, x, gsl_interp_eval(in, xa, ya, x, NULL));
    printf("%s %s D %.17g %.17g\n", mname, tname, x, gsl_interp_eval_deriv(in, xa, ya, x, NULL));
    printf("%s %s D2 %.17g %.17g\n", mname, tname, x, gsl_interp_eval_deriv2(in, xa, ya, x, NULL));
  }
  for (i = 0; i < ng; i++) {
    for (j = i; j < ng; j++) {
      double a = (i == ng - 1) ? x1 : x0 + (x1 - x0) * i / (ng - 1);
      double b = (j == ng - 1) ? x1 : x0 + (x1 - x0) * j / (ng - 1);
      printf("%s %s I %.17g %.17g %.17g\n", mname, tname, a, b,
             gsl_interp_eval_integ(in, xa, ya, a, b, NULL));
    }
  }
  gsl_interp_free(in);
}

static void table(const char *method, const gsl_interp_type *T, const char *tname,
                  const double *xa, const double *ya, int n, int periodic)
{
  int k;
  printf("T %s %d %d", tname, n, periodic);
  for (k = 0; k < n; k++) printf(" %.17g %.17g", xa[k], ya[k]);
  printf("\n");
  run(method, T, tname, xa, ya, n);
}

int main(void)
{
  double px[12], py[12], rx[9], ry[9];
  const double p3x[3] = {0.0, 1.5, 2.5}, p3y[3] = {1.0, 3.0, 1.0};
  const double p4x[4] = {0.0, 0.7, 1.9, 3.0}, p4y[4] = {2.0, -1.0, 0.5, 2.0};
  const double p5x[5] = {0.0, 1.0, 2.5, 3.0, 4.0}, p5y[5] = {0.0, 2.0, 1.0, 3.0, 0.0};
  const double nx[8] = {0, 0.5, 1.2, 2.0, 3.1, 3.9, 5.0, 6.5};
  const double ny[8] = {0.3, 1.1, 0.4, -0.7, 0.2, 1.9, 1.2, 2.2};
  const double p2x[2] = {0.0, 1.0}, p2y[2] = {0.0, 0.0};
  const double q3x[3] = {1.0, 2.0, 4.0}, q3y[3] = {2.0, 3.0, 1.5};
  const double q5x[5] = {0.0, 0.5, 1.5, 2.0, 3.5}, q5y[5] = {1.0, 0.2, -0.4, 0.9, 2.1};
  const double q7x[7] = {200, 300, 400, 500, 700, 1000, 1200};
  const double q7y[7] = {1.003, 1.005, 1.013, 1.030, 1.075, 1.142, 1.176};
  double cx[6], cy[6];
  int i;
  gsl_set_error_handler_off();
  for (i = 0; i < 12; i++) { px[i] = 2.0 * M_PI * i / 11.0; py[i] = sin(px[i]); }
  py[11] = py[0];
  for (i = 0; i < 9; i++) { rx[i] = -1.0 + 0.25 * i; ry[i] = 1.0 / (1.0 + 25.0 * rx[i] * rx[i]); }
  for (i = 0; i < 6; i++) { cx[i] = 0.5 * i; cy[i] = 1.0 - 2.0 * cx[i] + 0.5 * cx[i] * cx[i] * cx[i]; }

  table("cspline_periodic", gsl_interp_cspline_periodic, "per12", px, py, 12, 1);
  table("cspline_periodic", gsl_interp_cspline_periodic, "per3", p3x, p3y, 3, 1);
  table("cspline_periodic", gsl_interp_cspline_periodic, "per4", p4x, p4y, 4, 1);
  table("cspline_periodic", gsl_interp_cspline_periodic, "per5", p5x, p5y, 5, 1);
  table("cspline_periodic", gsl_interp_cspline_periodic, "nonper", nx, ny, 8, 0);
  table("cspline_periodic", gsl_interp_cspline_periodic, "per2", p2x, p2y, 2, 1);

  table("polynomial", gsl_interp_polynomial, "poly3", q3x, q3y, 3, 0);
  table("polynomial", gsl_interp_polynomial, "poly5", q5x, q5y, 5, 0);
  table("polynomial", gsl_interp_polynomial, "poly7", q7x, q7y, 7, 0);
  table("polynomial", gsl_interp_polynomial, "runge9", rx, ry, 9, 0);
  table("polynomial", gsl_interp_polynomial, "cubic6", cx, cy, 6, 0);
  return 0;
}
