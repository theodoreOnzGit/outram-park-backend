/* Probe for gh:#1338 case 1: why do PETIR's airy (x < -1) and Ci (x > 4)
 * differ from GSL at large |x|?
 *
 * GSL computes the oscillating factor with its OWN gsl_sf_cos_e / gsl_sf_sin_e
 * (specfunc/trig.c: octant reduction with a three-part pi/4 and Chebyshev
 * series); PETIR calls libm's cos / sin.  This prints, for a few arguments,
 * GSL's cos/sin, libm's cos/sin, and the long-double reference, so the
 * difference between the two is attributable to the trig routine alone.
 *
 *   reference-data/gsl/build_driver.sh reference-data/gsl/trig_vs_libm_probe.c \
 *       reference-data/gsl/trig-vs-libm-probe-gsl-2.8.txt
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_sf_trig.h>
#include <gsl/gsl_errno.h>

int main(void) {
  gsl_set_error_handler_off();
  double xs[] = {1.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6, 1e8, 1e10, 6.6666667e11, 1e12, 1e15};
  printf("# x  gsl_cos  libm_cos  ref_cos(long double)  |gsl-ref|  |libm-ref|  (same for sin)\n");
  for (unsigned i = 0; i < sizeof xs / sizeof *xs; i++) {
    double x = xs[i];
    gsl_sf_result c, s;
    gsl_sf_cos_e(x, &c);
    gsl_sf_sin_e(x, &s);
    long double rc = cosl((long double)x), rs = sinl((long double)x);
    printf("%.17g  cos %.17g %.17g %.17Lg  %.3e %.3e   sin %.17g %.17g %.17Lg  %.3e %.3e\n", x,
           c.val, cos(x), rc, fabs((double)(c.val - rc)), fabs((double)(cos(x) - rc)),
           s.val, sin(x), rs, fabs((double)(s.val - rs)), fabs((double)(sin(x) - rs)));
  }
  return 0;
}
