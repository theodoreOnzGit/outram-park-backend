#include <stdio.h>
#include <math.h>
#include <gsl/gsl_sf_gamma.h>
#include <gsl/gsl_errno.h>
int main(void) {
  gsl_set_error_handler_off();
  double as[] = {1.5, 0.75, 1.0001, 2.5, 3.0, 5.0, 9.5};
  for (unsigned ai = 0; ai < sizeof(as)/sizeof(as[0]); ai++) {
    double a = as[ai];
    for (int k = 1; k <= 220; k++) {
      double x = k * 0.35;
      gsl_sf_result p, g;
      if (gsl_sf_gamma_inc_P_e(a, x, &p) == GSL_SUCCESS)
        printf("P %.17e %.17e %.17e\n", a, x, p.val);
      /* Lower incomplete gamma. NOT Gamma(a) - Gamma(a,x): for small x that
         subtracts two nearly-equal large numbers and loses ~10 digits (at
         a=9.5, x=0.35 it is wrong by 1.4e-6 against a high-precision series).
         Compose Gamma(a) * P(a,x) instead, which is cancellation-free and is
         still GSL's own functions. */
      gsl_sf_result ga;
      if (gsl_sf_gamma_inc_P_e(a, x, &p) == GSL_SUCCESS &&
          gsl_sf_gamma_e(a, &ga) == GSL_SUCCESS)
        printf("L %.17e %.17e %.17e\n", a, x, ga.val * p.val);
      (void)g;
    }
  }
  return 0;
}
