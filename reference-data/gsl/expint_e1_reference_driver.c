#include <stdio.h>
#include <math.h>
#include <gsl/gsl_sf_expint.h>
#include <gsl/gsl_errno.h>
int main(void) {
  gsl_set_error_handler_off();
  double xs[400]; int n = 0;
  for (int k = -60; k <= 60; k++) { double x = k * 0.5; if (x != 0.0) xs[n++] = x; }
  for (int k = 1; k <= 40; k++) { xs[n++] = k * 0.021734; xs[n++] = -k * 0.021734; }
  for (int k = 1; k <= 20; k++) { xs[n++] = 100.0 + k * 7.3; xs[n++] = -80.0 - k * 3.1; }
  for (int i = 0; i < n; i++) {
    gsl_sf_result r;
    int st = gsl_sf_expint_E1_e(xs[i], &r);
    if (st == GSL_SUCCESS) printf("e1 %.17e %.17e %.17e\n", xs[i], r.val, r.err);
    gsl_sf_result rs;
    int sts = gsl_sf_expint_E1_scaled_e(xs[i], &rs);
    if (sts == GSL_SUCCESS) printf("e1s %.17e %.17e %.17e\n", xs[i], rs.val, rs.err);
  }
  return 0;
}
