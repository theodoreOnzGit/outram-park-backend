/* cquad_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{cquad, cquad_with_status} (gh:#1340, round 3).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_cquad (GSL 2.8, commit cf180cd7, integration/cquad.c,
 * cquad_const.c) with the 25 test integrands of GSL's own integration/test.c
 * (Gonnet's set, tests.c cqf1..cqf25) and further cases: looser tolerances,
 * small workspaces (intervals are dropped), integrands that are NaN or
 * infinite on part of the range (the downdating path), divergent integrals
 * (GSL_EDIVERGE), refused tolerances, reversed and degenerate intervals.
 * Prints, at 17 significant digits,
 *   "CQUAD <name> <status> <result> <abserr> <nevals> <calls>"
 *   "WSALLOC <n> <ok>"
 * Unwritten outputs (EBADTOL, EDIVERGE's abserr) print as the initial 0.
 *
 * cquad.c itself calls only sqrt, so unlike the weights of QAWO there is no
 * platform-libm difference in the routine; the integrands call glibc here and
 * in the Rust test alike.
 *
 * Build: reference-data/gsl/build_gsl_nocfg.sh <build-dir> \
 *          reference-data/gsl/cquad_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t calls;

static double cqf1(double x, void *p) { (void) p; calls++; return exp(x); }
static double cqf2(double x, void *p) { (void) p; calls++; return x >= 0.3; }
static double cqf3(double x, void *p) { (void) p; calls++; return sqrt(x); }
static double cqf4(double x, void *p) { (void) p; calls++; return (23.0 / 25) * cosh(x) - cos(x); }
static double cqf5(double x, void *p)
{
  double x2 = x * x;
  (void) p; calls++;
  return 1.0 / (x2 * (x2 + 1) + 0.9);
}
static double cqf6(double x, void *p) { (void) p; calls++; return x * sqrt(x); }
static double cqf7(double x, void *p) { (void) p; calls++; return 1.0 / sqrt(x); }
static double cqf8(double x, void *p)
{
  double x2 = x * x;
  (void) p; calls++;
  return 1.0 / (1 + x2 * x2);
}
static double cqf9(double x, void *p) { (void) p; calls++; return 2.0 / (2 + sin(10 * M_PI * x)); }
static double cqf10(double x, void *p) { (void) p; calls++; return 1.0 / (1 + x); }
static double cqf11(double x, void *p) { (void) p; calls++; return 1.0 / (1 + exp(x)); }
static double cqf12(double x, void *p) { (void) p; calls++; return x / (exp(x) - 1.0); }
static double cqf13(double x, void *p) { (void) p; calls++; return sin(100 * M_PI * x) / (M_PI * x); }
static double cqf14(double x, void *p) { (void) p; calls++; return sqrt(50.0) * exp(-50 * M_PI * x * x); }
static double cqf15(double x, void *p) { (void) p; calls++; return 25.0 * exp(-25 * x); }
static double cqf16(double x, void *p) { (void) p; calls++; return 50 / M_PI * (2500 * x * x + 1); }
static double cqf17(double x, void *p)
{
  double t1 = 50 * M_PI * x, t2;
  (void) p; calls++;
  t2 = sin(t1) / t1;
  return 50 * t2 * t2;
}
static double cqf18(double x, void *p)
{
  (void) p; calls++;
  return cos(cos(x) + 3 * sin(x) + 2 * cos(2 * x) + 3 * sin(2 * x) + 3 * cos(3 * x));
}
static double cqf19(double x, void *p) { (void) p; calls++; return log(x); }
static double cqf20(double x, void *p) { (void) p; calls++; return 1 / (x * x + 1.005); }
static double cqf21(double x, void *p)
{
  (void) p; calls++;
  return 1 / cosh(10 * (x - 0.2) * 2) + 1 / cosh(100 * (x - 0.4) * 4) + 1 / cosh(1000 * (x - 0.6) * 8);
}
static double cqf22(double x, void *p)
{
  (void) p; calls++;
  return 4 * M_PI * M_PI * x * sin(20 * M_PI * x) * cos(2 * M_PI * x);
}
static double cqf23(double x, void *p)
{
  double t = 230 * x - 30;
  (void) p; calls++;
  return 1 / (1 + t * t);
}
static double cqf24(double x, void *p) { (void) p; calls++; return floor(exp(x)); }
static double cqf25(double x, void *p)
{
  (void) p; calls++;
  return (x < 1) * (x + 1) + (1 <= x && x <= 3) * (3 - x) + (x > 3) * 2;
}
/* non-finite on part of the range */
static double recip(double x, void *p) { (void) p; calls++; return 1.0 / x; }
static double recip2(double x, void *p) { (void) p; calls++; return 1.0 / (x * x); }
static double inf_at_1(double x, void *p) { (void) p; calls++; return 1.0 / (1.0 - x); }
static double sqrt_signed(double x, void *p) { (void) p; calls++; return sqrt(x); }
static double log_signed(double x, void *p) { (void) p; calls++; return log(x); }
static double nan_band(double x, void *p) { (void) p; calls++; return (x >= 0.3 && x <= 0.35) ? NAN : exp(x); }
static double nan_mid(double x, void *p) { (void) p; calls++; return fabs(x - 0.5) < 0.2 ? NAN : exp(x); }
static double poly(double x, void *p) { (void) p; calls++; return x * x * x - x; }
static double cosh_peak(double x, void *p) { (void) p; calls++; return 1.0 / cosh(200.0 * (x - 0.123)); }

typedef double (*fn_t)(double, void *);

static void run(const char *name, fn_t fn, double a, double b, double epsabs, double epsrel, size_t size)
{
  gsl_integration_cquad_workspace *ws = gsl_integration_cquad_workspace_alloc(size);
  gsl_function F;
  double r = 0, e = 0;
  size_t ne = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  calls = 0;
  if (ws == NULL) {
    printf("WSALLOC %zu 0\n", size);
    return;
  }
  status = gsl_integration_cquad(&F, a, b, epsabs, epsrel, ws, &r, &e, &ne);
  printf("CQUAD %s %d %.17e %.17e %zu %zu\n", name, status, r, e, ne, calls);
  gsl_integration_cquad_workspace_free(ws);
}

int main(void)
{
  const fn_t funs[25] = { cqf1, cqf2, cqf3, cqf4, cqf5, cqf6, cqf7, cqf8, cqf9, cqf10, cqf11, cqf12, cqf13,
                          cqf14, cqf15, cqf16, cqf17, cqf18, cqf19, cqf20, cqf21, cqf22, cqf23, cqf24, cqf25 };
  const double ranges[50] = { 0, 1, 0, 1, 0, 1, -1, 1, -1, 1, 0, 1, 0, 1, 0, 1, 0, 1,
                              0, 1, 0, 1, 0, 1, 0, 1, 0, 10, 0, 10, 0, 10, 0, 1, 0, M_PI,
                              0, 1, -1, 1, 0, 1, 0, 1, 0, 1, 0, 3, 0, 5 };
  char name[32];
  int i;
  gsl_set_error_handler_off();

  for (i = 0; i < 25; i++) {
    sprintf(name, "f%d", i + 1);
    run(name, funs[i], ranges[2 * i], ranges[2 * i + 1], 0.0, 1.0e-12, 200);
  }

  /* looser and tighter tolerances */
  run("f1_1e-6", cqf1, 0, 1, 0.0, 1e-6, 200);
  run("f18_1e-8", cqf18, 0, M_PI, 0.0, 1e-8, 200);
  run("f13_1e-3", cqf13, 0, 1, 0.0, 1e-3, 200);
  run("f21_1e-14", cqf21, 0, 1, 0.0, 1e-14, 200);
  run("f15_abs", cqf15, 0, 10, 1e-10, 0.0, 200);
  run("f15_abs_rel", cqf15, 0, 10, 1e-10, 1e-10, 200);
  run("poly_exact", poly, -1, 2, 0.0, 1e-10, 50);

  /* small workspaces: intervals are dropped */
  run("f21_ws3", cqf21, 0, 1, 0.0, 1e-12, 3);
  run("f21_ws4", cqf21, 0, 1, 0.0, 1e-12, 4);
  run("f21_ws5", cqf21, 0, 1, 0.0, 1e-12, 5);
  run("f21_ws8", cqf21, 0, 1, 0.0, 1e-12, 8);
  run("f13_ws5", cqf13, 0, 1, 0.0, 1e-12, 5);
  run("f7_ws3", cqf7, 0, 1, 0.0, 1e-12, 3);
  run("f23_ws10", cqf23, 0, 1, 0.0, 1e-12, 10);
  run("peak_ws6", cosh_peak, 0, 1, 0.0, 1e-12, 6);

  /* non-finite values: the downdating path */
  run("sqrt_signed", sqrt_signed, -1, 1, 0.0, 1e-10, 200);
  run("log_signed", log_signed, -1, 1, 0.0, 1e-10, 200);
  run("nan_band", nan_band, 0, 1, 0.0, 1e-10, 200);
  run("nan_mid", nan_mid, 0, 1, 0.0, 1e-8, 200);
  run("inf_at_1", inf_at_1, 0, 1, 0.0, 1e-8, 200);
  run("recip_0_1", recip, 0, 1, 0.0, 1e-9, 200);
  run("recip2_0_1", recip2, 0, 1, 0.0, 1e-9, 200);
  run("recip_sym", recip, -1, 1, 0.0, 1e-9, 200);
  run("recip_pos", recip, 1, 10, 0.0, 1e-12, 200);

  /* refusals and degenerate intervals */
  run("badtol_neg_abs", cqf1, 0, 1, -1.0, 1e-9, 20);
  run("badtol_neg_rel", cqf1, 0, 1, 0.0, -1e-9, 20);
  run("badtol_zero", cqf1, 0, 1, 0.0, 0.0, 20);
  run("badtol_tiny_rel", cqf1, 0, 1, 0.0, 1e-17, 20);
  run("ws2", cqf1, 0, 1, 0.0, 1e-9, 2);
  run("ws0", cqf1, 0, 1, 0.0, 1e-9, 0);
  run("reversed", cqf1, 1, 0, 0.0, 1e-12, 200);
  run("zero_width", cqf1, 0.5, 0.5, 0.0, 1e-12, 200);
  run("narrow", cqf1, 1.0, 1.0 + 1e-15, 0.0, 1e-12, 200);
  run("huge", cqf1, 0, 1e6, 0.0, 1e-8, 200);
  return 0;
}
