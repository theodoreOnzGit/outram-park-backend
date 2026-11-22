/* GSL 2.8 gsl_sf_sin_e / gsl_sf_cos_e reference driver for PETIR (gh:#1421).
 *
 * One line per call:  <sin|cos> <arg bits> <gsl status> <value bits>
 * (64-bit IEEE patterns, 16 hex digits).  gsl_sf_sin_e/cos_e (specfunc/trig.c)
 * call no libm function, so this reference does not depend on the platform
 * libm: PETIR's `sin_gsl`/`cos_gsl` are expected to match it bit for bit.
 *
 * Grid: dense k/4 on [-160, 160]; the octant boundaries m*pi/4 for
 * m = -64..64 and their floating-point neighbours; a fixed LCG
 * (state' = state*6364136223846793005 + 1442695040888963407, seed 12345)
 * sampling [-1e3, 1e3], [-1e6, 1e6] and log-uniform magnitudes up to 1e14;
 * the round numbers 1e4 .. 1e15, 1e20, 1e100, 1e300, DBL_MAX; the
 * small-argument cut (DBL_EPSILON^(1/4) and neighbours); +-0, +-DBL_MIN,
 * +-4.9e-324.
 *
 * Build and run (from the repository root; GSL 2.8 built under $G):
 *   gcc -O2 -I$G -o drv reference-data/gsl/trig_sincos_reference_driver.c \
 *       $G/.libs/libgsl.a $G/cblas/.libs/libgslcblas.a -lm
 *   ./drv > reference-data/gsl/trig-sincos-gsl-2.8-reference.txt
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <float.h>
#include <math.h>
#include <gsl/gsl_sf_trig.h>
#include <gsl/gsl_errno.h>

static uint64_t bits(double x) { uint64_t u; memcpy(&u, &x, 8); return u; }
#define B(x) ((unsigned long long)bits(x))

static void emit(double x) {
  gsl_sf_result r;
  int s = gsl_sf_sin_e(x, &r);
  printf("sin %016llx %d %016llx\n", B(x), s, B(r.val));
  s = gsl_sf_cos_e(x, &r);
  printf("cos %016llx %d %016llx\n", B(x), s, B(r.val));
}
static void emit3(double x) {
  emit(x); emit(nextafter(x, INFINITY)); emit(nextafter(x, -INFINITY));
}
static uint64_t st = 12345;
static double u01(void) {
  st = st * 6364136223846793005ULL + 1442695040888963407ULL;
  return (double)(st >> 11) / 9007199254740992.0;
}

int main(void) {
  gsl_set_error_handler_off();
  for (int k = -640; k <= 640; k++) emit(k * 0.25);
  for (int m = -64; m <= 64; m++) emit3(m * (M_PI / 4.0));
  for (int i = 0; i < 6000; i++) emit((2.0 * u01() - 1.0) * 1e3);
  for (int i = 0; i < 6000; i++) emit((2.0 * u01() - 1.0) * 1e6);
  for (int i = 0; i < 6000; i++) emit((u01() < 0.5 ? -1.0 : 1.0) * pow(10.0, 14.0 * u01()));
  double big[] = {1e4, 1e5, 1e6, 1e8, 1e9, 1e10, 1e12, 1e15, 1e16, 1e20, 1e100, 1e300, DBL_MAX};
  for (int i = 0; i < 13; i++) { emit(big[i]); emit(-big[i]); }
  double cut = sqrt(sqrt(DBL_EPSILON));
  emit3(cut); emit3(-cut);
  emit(0.0); emit(-0.0); emit(DBL_MIN); emit(-DBL_MIN); emit(4.9e-324); emit(-4.9e-324);
  return 0;
}
