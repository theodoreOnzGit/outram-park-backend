/* interp_integ_reference_driver.c -- compiled-GSL reference for
 * petir::interp::Interp::eval_integ (gh:#989).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_interp_eval_integ (GSL 2.8, commit cf180cd7, interpolation/interp.c)
 * for gsl_interp_linear and gsl_interp_cspline on two tables:
 *
 *   runge   -- the 9-point Runge table numerics_reference_driver.c already
 *              uses for eval/eval_deriv, integrated over every ordered pair
 *              (a, b) of a 21-point grid on [-1, 1] (knots, mid-segment
 *              points, a == b, single-segment and multi-segment spans);
 *   thermo  -- a non-uniform 11-knot table shaped like the specific-heat
 *              tables tuas_boussinesq_solver integrates for enthalpy
 *              (x in kelvin, 200..1200), integrated from 273.15 K to every
 *              T on a 41-point grid at or above 273.15 K, and from the
 *              first knot to every knot.
 *
 * Plus the refusals: a > b and a limit outside [x0, x_{n-1}]. GSL's error
 * handler is switched off so those return NaN; the replaying test expects
 * PetirError::Domain for each ERR row.
 *
 * Every row: "I <a> <b> <integral>" at 17 significant digits.
 *
 * Build (see README.md, "interp-integ-gsl-2.8-reference.txt"):
 *   G=crates/petir/upstream_source/GSL
 *   mkdir -p /tmp/gslinteg/gsl && cd /tmp/gslinteg
 *   for h in $G/*.h $G/interpolation/*.h $G/linalg/*.h $G/vector/*.h \
 *            $G/block/*.h $G/err/*.h $G/sys/*.h; do
 *     ln -sf "$h" gsl/$(basename $h); done
 *   printf '#define HAVE_INLINE 1\n#define RETURN_IF_NULL(x) if (!x) { return ; }\n' > config.h
 *   gcc -O2 -I. -I./gsl -I$G -I$G/interpolation -o integdriver \
 *     <this dir>/interp_integ_reference_driver.c \
 *     $G/interpolation/{interp,linear,cspline,accel,bsearch}.c \
 *     $G/linalg/tridiag.c $G/vector/{init,view}.c $G/block/init.c \
 *     $G/err/{error,stream}.c -lm
 *   ./integdriver > interp-integ-gsl-2.8-reference.txt
 * (the exact object list that linked is recorded in README.md)
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_interp.h>

#define NR 9
#define NT 11

static void runge(const char *name, const gsl_interp_type *T)
{
  double xa[NR], ya[NR];
  gsl_interp *s = gsl_interp_alloc(T, NR);
  int i, k, m;
  for (i = 0; i < NR; i++) {
    xa[i] = -1.0 + 2.0 * i / (double) (NR - 1);
    ya[i] = 1.0 / (1.0 + 25.0 * xa[i] * xa[i]);
  }
  gsl_interp_init(s, xa, ya, NR);
  printf("CASE integ_runge_%s\n", name);
  for (k = 0; k <= 20; k++) {
    for (m = k; m <= 20; m++) {
      double a = -1.0 + 2.0 * k / 20.0;
      double b = -1.0 + 2.0 * m / 20.0;
      printf("I %.17e %.17e %.17e\n", a, b,
             gsl_interp_eval_integ(s, xa, ya, a, b, NULL));
    }
  }
  /* refusals */
  printf("ERR %.17e %.17e %.17e\n", 0.5, -0.5,
         gsl_interp_eval_integ(s, xa, ya, 0.5, -0.5, NULL));
  printf("ERR %.17e %.17e %.17e\n", -1.5, 0.0,
         gsl_interp_eval_integ(s, xa, ya, -1.5, 0.0, NULL));
  printf("ERR %.17e %.17e %.17e\n", 0.0, 1.0000000000000002,
         gsl_interp_eval_integ(s, xa, ya, 0.0, 1.0000000000000002, NULL));
  printf("END integ_runge_%s\n", name);
  gsl_interp_free(s);
}

static void thermo(const char *name, const gsl_interp_type *T)
{
  /* Knots in kelvin, deliberately non-uniform. Ordinates are a smooth
   * cp-like shape (J/kg/K): rising, then flattening. Synthetic, not data. */
  static const double xa[NT] = {200.0, 250.0, 273.15, 300.0, 350.0, 400.0,
                                500.0, 600.0, 800.0, 1000.0, 1200.0};
  double ya[NT];
  gsl_interp *s = gsl_interp_alloc(T, NT);
  int i, k;
  for (i = 0; i < NT; i++)
    ya[i] = 500.0 + 0.4 * xa[i] - 1.5e-4 * xa[i] * xa[i];
  gsl_interp_init(s, xa, ya, NT);
  printf("CASE integ_thermo_%s\n", name);
  for (k = 0; k <= 40; k++) {
    double t = 273.15 + (1200.0 - 273.15) * k / 40.0;
    printf("I %.17e %.17e %.17e\n", 273.15, t,
           gsl_interp_eval_integ(s, xa, ya, 273.15, t, NULL));
  }
  for (k = 0; k < NT; k++)
    printf("I %.17e %.17e %.17e\n", xa[0], xa[k],
           gsl_interp_eval_integ(s, xa, ya, xa[0], xa[k], NULL));
  printf("ERR %.17e %.17e %.17e\n", 273.15, 150.0,
         gsl_interp_eval_integ(s, xa, ya, 273.15, 150.0, NULL));
  printf("ERR %.17e %.17e %.17e\n", 273.15, 1300.0,
         gsl_interp_eval_integ(s, xa, ya, 273.15, 1300.0, NULL));
  printf("END integ_thermo_%s\n", name);
  gsl_interp_free(s);
}

int main(void)
{
  gsl_set_error_handler_off();
  runge("linear", gsl_interp_linear);
  runge("cspline", gsl_interp_cspline);
  thermo("linear", gsl_interp_linear);
  thermo("cspline", gsl_interp_cspline);
  return 0;
}
