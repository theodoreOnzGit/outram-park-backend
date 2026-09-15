/* Reference driver: GSL 2.8 gsl_cheb_eval_mode / gsl_cheb_eval_mode_e.
 *
 * These are the two entry points of gsl_chebyshev.h that petir did not cover
 * until 2026-09-15. They select cs->order (GSL_PREC_DOUBLE) or cs->order_sp
 * (anything else).
 *
 * gsl_cheb_alloc sets order_sp = order and nothing in GSL ever changes it, so
 * the default case is indistinguishable from gsl_cheb_eval. To exercise the
 * truncated path this driver ALSO sets cs->order_sp by hand -- which the GSL
 * header explicitly invites ("Users can use it if they like").
 *
 * Build: see README.md in this directory.
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_chebyshev.h>
#include <gsl/gsl_mode.h>

static double f_exp(double x, void *p) { (void) p; return exp(x); }
static double f_runge(double x, void *p) { (void) p; return 1.0 / (1.0 + 25.0 * x * x); }

static void emit(const char *name, double (*fn)(double, void *),
                 size_t order, double a, double b, size_t order_sp)
{
  gsl_cheb_series *cs = gsl_cheb_alloc(order);
  gsl_function F;
  int k;

  F.function = fn;
  F.params = 0;
  gsl_cheb_init(cs, &F, a, b);

  printf("CASE %s %zu %.17e %.17e %zu %zu\n",
         name, order, a, b, cs->order_sp, order_sp);

  /* First: the untouched series, where order_sp == order. */
  for (k = 0; k <= 40; k++) {
    double x = a + (b - a) * k / 40.0;
    double rd, ed, rs, es;
    gsl_cheb_eval_mode_e(cs, x, GSL_PREC_DOUBLE, &rd, &ed);
    gsl_cheb_eval_mode_e(cs, x, GSL_PREC_SINGLE, &rs, &es);
    printf("DEFAULT %d %.17e %.17e %.17e %.17e %.17e %.17e\n",
           k, x, rd, ed, rs, es, gsl_cheb_eval_mode(cs, x, GSL_PREC_DOUBLE));
  }

  /* Then: order_sp reduced by hand, exercising the truncated branch. */
  cs->order_sp = order_sp;
  for (k = 0; k <= 40; k++) {
    double x = a + (b - a) * k / 40.0;
    double rs, es;
    gsl_cheb_eval_mode_e(cs, x, GSL_PREC_SINGLE, &rs, &es);
    printf("REDUCED %d %.17e %.17e %.17e %.17e\n",
           k, x, rs, es, gsl_cheb_eval_mode(cs, x, GSL_PREC_APPROX));
  }

  printf("END %s\n", name);
  gsl_cheb_free(cs);
}

int main(void)
{
  emit("exp24", f_exp, 24, -1.0, 1.0, 6);
  emit("runge30", f_runge, 30, -1.0, 1.0, 11);
  emit("exp5", f_exp, 5, -2.0, 3.0, 0);   /* order_sp = 0: the constant term alone */
  return 0;
}
