/* Code-to-code reference driver: runs GSL's OWN cheb routines and dumps
   results for comparison against the PETIR port. Not distributed. */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_chebyshev.h>

static double f_sin(double x, void *p) { (void)p; return sin(x); }
static double f_exp(double x, void *p) { (void)p; return exp(x); }
static double f_runge(double x, void *p) { (void)p; return 1.0/(1.0+25.0*x*x); }

static void dump(const char *tag, size_t order, double a, double b,
                 double (*fn)(double, void *)) {
  gsl_function F; F.function = fn; F.params = 0;
  gsl_cheb_series *cs = gsl_cheb_alloc(order);
  gsl_cheb_init(cs, &F, a, b);
  gsl_cheb_series *cd = gsl_cheb_alloc(order);
  gsl_cheb_series *ci = gsl_cheb_alloc(order);
  gsl_cheb_calc_deriv(cd, cs);
  gsl_cheb_calc_integ(ci, cs);
  for (size_t j = 0; j <= order; j++)
    printf("%s coeff %zu %.17e\n", tag, j, gsl_cheb_coeffs(cs)[j]);
  for (int k = 0; k <= 200; k++) {
    double x = a + (b - a) * k / 200.0;
    double r, e;
    gsl_cheb_eval_err(cs, x, &r, &e);
    printf("%s eval %.17e %.17e %.17e %.17e %.17e %.17e\n", tag, x,
           gsl_cheb_eval(cs, x), r, e, gsl_cheb_eval(cd, x), gsl_cheb_eval(ci, x));
    printf("%s evaln %.17e %.17e\n", tag, x, gsl_cheb_eval_n(cs, 7, x));
  }
  gsl_cheb_free(ci); gsl_cheb_free(cd); gsl_cheb_free(cs);
}

int main(void) {
  dump("sin40", 40, -M_PI, M_PI, f_sin);
  dump("exp12", 12, -1.0, 2.0, f_exp);
  dump("runge24", 24, -1.0, 1.0, f_runge);
  dump("sin1", 1, -5.0, 5.0, f_sin);
  dump("sin2", 2, -5.0, 5.0, f_sin);
  return 0;
}
