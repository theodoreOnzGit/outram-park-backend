/* interp2d_reference_driver.c -- compiled-GSL reference for
 * petir::interp2d (gh:#1341).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls GSL 2.8 (commit cf180cd7) gsl_interp2d_eval_e, _eval_deriv_x_e, _y_e,
 * _xx_e, _yy_e, _xy_e and _eval_extrap_e for gsl_interp2d_bilinear and
 * gsl_interp2d_bicubic (NO accelerators) on four grids:
 *   g2x3   2 x 3 (bilinear only; bicubic's alloc refuses: min_size 4)
 *   g4x4   4 x 4, the smallest bicubic grid
 *   g5x6   5 x 6, non-uniform
 *   g8x7   8 x 7, non-uniform, bumpy
 * Row formats (17 significant digits):
 *   "G <name> <nx> <ny> x... y... z..."          the grid (z[j*nx+i])
 *   "ALLOC <method> <name> <ok>"                 alloc succeeded (1) or not (0)
 *   "<method> <name> <op> <x> <y> <value>"       op in E DX DY DXX DYY DXY X
 *                                                (X = eval_extrap, 12 points
 *                                                outside the grid)
 *   "OOR <method> <name> <x> <y> <status>"       eval_e status outside the grid
 * Evaluation points: a 17 x 17 grid over the domain (corners and edges
 * included) plus every node.
 *
 * Build: reference-data/gsl/build_gsl_family.sh interp2d_reference_driver.c out.txt
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_interp2d.h>

typedef int (*ev_t)(const gsl_interp2d *, const double *, const double *, const double *,
                    double, double, gsl_interp_accel *, gsl_interp_accel *, double *);

static void point(const char *m, const char *name, const gsl_interp2d *in, const double *xa,
                  const double *ya, const double *za, double x, double y)
{
  static const char *ops[6] = {"E", "DX", "DY", "DXX", "DYY", "DXY"};
  ev_t evs[6] = {gsl_interp2d_eval_e, gsl_interp2d_eval_deriv_x_e, gsl_interp2d_eval_deriv_y_e,
                 gsl_interp2d_eval_deriv_xx_e, gsl_interp2d_eval_deriv_yy_e,
                 gsl_interp2d_eval_deriv_xy_e};
  int k;
  for (k = 0; k < 6; k++) {
    double v = 0;
    int st = evs[k](in, xa, ya, za, x, y, NULL, NULL, &v);
    if (st != 0) printf("# unexpected status %d for %s %s %s\n", st, m, name, ops[k]);
    printf("%s %s %s %.17g %.17g %.17g\n", m, name, ops[k], x, y, v);
  }
}

static void run(const char *m, const gsl_interp2d_type *T, const char *name, const double *xa,
                const double *ya, const double *za, size_t nx, size_t ny)
{
  gsl_interp2d *in = gsl_interp2d_alloc(T, nx, ny);
  size_t i, j;
  double x0 = xa[0], x1 = xa[nx - 1], y0 = ya[0], y1 = ya[ny - 1];
  printf("ALLOC %s %s %d\n", m, name, in != NULL);
  if (in == NULL) return;
  if (gsl_interp2d_init(in, xa, ya, za, nx, ny) != 0) { printf("# init failed\n"); return; }
  for (j = 0; j < 17; j++)
    for (i = 0; i < 17; i++) {
      double x = (i == 16) ? x1 : x0 + (x1 - x0) * i / 16.0;
      double y = (j == 16) ? y1 : y0 + (y1 - y0) * j / 16.0;
      point(m, name, in, xa, ya, za, x, y);
    }
  for (j = 0; j < ny; j++)
    for (i = 0; i < nx; i++) point(m, name, in, xa, ya, za, xa[i], ya[j]);
  {
    double ox[12] = {x0 - 0.5, x1 + 0.5, x0 - 0.1, x1 + 0.1, 0.5 * (x0 + x1), 0.5 * (x0 + x1),
                     x0 - 1.0, x1 + 1.0, x0, x1, x0 - 0.25, x1 + 0.25};
    double oy[12] = {0.5 * (y0 + y1), 0.5 * (y0 + y1), y0 - 0.1, y1 + 0.1, y0 - 0.5, y1 + 0.5,
                     y0 - 1.0, y1 + 1.0, y0 - 0.3, y1 + 0.3, y1 + 0.25, y0 - 0.25};
    int k;
    for (k = 0; k < 12; k++) {
      double v = 0, w = 0;
      int st = gsl_interp2d_eval_e(in, xa, ya, za, ox[k], oy[k], NULL, NULL, &w);
      int ok = gsl_interp2d_eval_extrap_e(in, xa, ya, za, ox[k], oy[k], NULL, NULL, &v);
      if (ok != 0) printf("# unexpected extrap status %d\n", ok);
      printf("OOR %s %s %.17g %.17g %d\n", m, name, ox[k], oy[k], st);
      printf("%s %s X %.17g %.17g %.17g\n", m, name, ox[k], oy[k], v);
    }
  }
  gsl_interp2d_free(in);
}

static void grid(const char *name, const double *xa, const double *ya, const double *za,
                 size_t nx, size_t ny)
{
  size_t i;
  printf("G %s %zu %zu", name, nx, ny);
  for (i = 0; i < nx; i++) printf(" %.17g", xa[i]);
  for (i = 0; i < ny; i++) printf(" %.17g", ya[i]);
  for (i = 0; i < nx * ny; i++) printf(" %.17g", za[i]);
  printf("\n");
  run("bilinear", gsl_interp2d_bilinear, name, xa, ya, za, nx, ny);
  run("bicubic", gsl_interp2d_bicubic, name, xa, ya, za, nx, ny);
}

int main(void)
{
  const double a2x[2] = {0.0, 1.5}, a3y[3] = {-1.0, 0.0, 2.0};
  double a23[6] = {1.0, 3.0, 0.5, 2.0, 4.0, 1.0};
  const double b4x[4] = {0.0, 1.0, 2.0, 3.0}, b4y[4] = {0.0, 1.0, 2.0, 3.0};
  double b44[16];
  const double c5x[5] = {0.0, 0.4, 1.3, 2.0, 3.5}, c6y[6] = {-2.0, -1.5, 0.0, 0.7, 1.9, 3.0};
  double c56[30];
  double d8x[8], d7y[7], d87[56];
  size_t i, j;
  gsl_set_error_handler_off();
  for (j = 0; j < 4; j++) for (i = 0; i < 4; i++) b44[j * 4 + i] = sin(b4x[i]) * cos(b4y[j]) + 0.1 * i * j;
  for (j = 0; j < 6; j++) for (i = 0; i < 5; i++) c56[j * 5 + i] = exp(-0.3 * c5x[i]) * (1.0 + c6y[j] * c6y[j]) + 0.2 * sin(3.0 * c5x[i] * c6y[j]);
  for (i = 0; i < 8; i++) d8x[i] = 0.3 * i * i + 0.2 * i;
  for (j = 0; j < 7; j++) d7y[j] = 0.5 * j + 0.1 * j * j * j * 0.1;
  for (j = 0; j < 7; j++) for (i = 0; i < 8; i++) d87[j * 8 + i] = cos(0.9 * d8x[i]) * sin(0.7 * d7y[j]) + ((i * 7 + j * 3) % 5) * 0.05;
  grid("g2x3", a2x, a3y, a23, 2, 3);
  grid("g4x4", b4x, b4y, b44, 4, 4);
  grid("g5x6", c5x, c6y, c56, 5, 6);
  grid("g8x7", d8x, d7y, d87, 8, 7);
  return 0;
}
