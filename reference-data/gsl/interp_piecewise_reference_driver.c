/* interp_piecewise_reference_driver.c -- compiled-GSL reference for
 * petir::interp_piecewise (gh:#1341).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls GSL 2.8 (commit cf180cd7) gsl_interp_eval, _eval_deriv, _eval_deriv2
 * and _eval_integ for gsl_interp_steffen, gsl_interp_akima and
 * gsl_interp_akima_periodic on five tables (NO accelerator, so the plain
 * bsearch the port implements):
 *
 *   runge     9 points, x in [-1, 1], y = 1/(1+25x^2)
 *   thermo    11 non-uniform knots, 200..1200, cp-shaped
 *   step      7 points, flat-step-flat (monotonicity; zero slopes)
 *   periodic  12 points, y = sin(x) on [0, 2 pi] with y[n-1] = y[0] forced
 *   plateau   8 points with equal consecutive slopes (Akima NE == 0 branch)
 *
 * Row formats (17 significant digits; one row per evaluation point). Each
 * table is first written as "T <table> <n> <periodic> x0 y0 x1 y1 ..." so the
 * replaying test uses the inputs the C library formed.
 *   "<method> <table> E  <x> <y>"
 *   "<method> <table> D  <x> <dy>"
 *   "<method> <table> D2 <x> <d2y>"
 *   "<method> <table> I  <a> <b> <integral>"
 * 241 evaluation points per table over [x0, x_{n-1}] (knots, mid-points and
 * a dense grid); integrals over every ordered pair of a 21-point sub-grid.
 * GSL's error handler is switched off.
 *
 * Build (GSL 2.8 built in a scratch directory, see README.md):
 *   gcc -O2 -I$G -o d interp_piecewise_reference_driver.c \
 *       $G/.libs/libgsl.a $G/cblas/.libs/libgslcblas.a -lm
 *   ./d > interp-piecewise-gsl-2.8-reference.txt
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_interp.h>

static void run(const char *mname, const gsl_interp_type *T, const char *tname,
                const double *xa, const double *ya, int n)
{
  gsl_interp *in = gsl_interp_alloc(T, n);
  int i, j, np = 241, ng = 21;
  double x0 = xa[0], x1 = xa[n - 1];
  gsl_interp_init(in, xa, ya, n);
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

static void table(const char *tname, const double *xa, const double *ya, int n, int periodic)
{
  int k;
  printf("T %s %d %d", tname, n, periodic);
  for (k = 0; k < n; k++) printf(" %.17g %.17g", xa[k], ya[k]);
  printf("\n");
  run("steffen", gsl_interp_steffen, tname, xa, ya, n);
  if (n >= 5) {
    run("akima", gsl_interp_akima, tname, xa, ya, n);
    if (periodic) run("akima_periodic", gsl_interp_akima_periodic, tname, xa, ya, n);
  }
}

int main(void)
{
  double rx[9], ry[9], px[12], py[12];
  const double tx[11] = {200, 250, 300, 400, 500, 650, 700, 850, 1000, 1100, 1200};
  const double ty[11] = {1.003, 1.004, 1.005, 1.013, 1.030, 1.062, 1.075, 1.110, 1.142, 1.160, 1.176};
  const double sx[7] = {0, 1, 2, 3, 4, 5, 6};
  const double sy[7] = {0, 0, 0, 1, 1, 1, 1};
  const double lx[8] = {0, 1, 2, 3, 4, 5, 6, 7};
  const double ly[8] = {0, 1, 2, 3, 5, 7, 9, 11};
  int i;
  gsl_set_error_handler_off();
  for (i = 0; i < 9; i++) { rx[i] = -1.0 + 0.25 * i; ry[i] = 1.0 / (1.0 + 25.0 * rx[i] * rx[i]); }
  for (i = 0; i < 12; i++) { px[i] = 2.0 * M_PI * i / 11.0; py[i] = sin(px[i]); }
  py[11] = py[0];
  table("runge", rx, ry, 9, 0);
  table("thermo", tx, ty, 11, 0);
  table("step", sx, sy, 7, 0);
  table("plateau", lx, ly, 8, 0);
  table("periodic", px, py, 12, 1);
  return 0;
}
