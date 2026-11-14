/* qawo_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{QawoTable, qawo, qawf} (gh:#1340, round 3).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qawo_table_alloc / _set / _set_length,
 * gsl_integration_qawo and gsl_integration_qawf (GSL 2.8, commit cf180cd7,
 * integration/qawo.c, qawf.c, qc25f.c, qmomof.c) and prints, at 17 significant
 * digits,
 *   "TBL <tag> <omega> <L> <sine> <n> <n*25 moments>"
 *   "QAWO <name> <status> <result> <abserr> <size> <neval>"
 *   "QAWF <name> <status> <result> <abserr> <size> <cycle_size> <neval>"
 *
 * The integrands deliberately call no sin/cos (only exp, log, sqrt and
 * rational functions), so the only sin/cos in the program are the weights
 * inside GSL's qmomof.c and qc25f.c: a relink against PETIR's sin/cos alone
 * (petir_sincos_shim.rs) then isolates exactly the platform-libm difference.
 *
 * Build: reference-data/gsl/build_gsl_nocfg.sh <build-dir> \
 *          reference-data/gsl/qawo_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double one(double x, void *p) { (void) p; (void) x; neval++; return 1.0; }
static double zero(double x, void *p) { (void) p; (void) x; neval++; return 0.0; }
static double lin(double x, void *p) { (void) p; neval++; return x; }
static double expmx(double x, void *p) { (void) p; neval++; return exp(-x); }
static double expx(double x, void *p) { (void) p; neval++; return exp(x); }
static double invsqrt(double x, void *p) { (void) p; neval++; return 1.0 / sqrt(x); }
static double logx(double x, void *p) { (void) p; neval++; return log(x); }
static double lorentz(double x, void *p) { (void) p; neval++; return 1.0 / (1.0 + x * x); }
static double rec1p(double x, void *p) { (void) p; neval++; return 1.0 / (1.0 + x); }
static double rec1(double x, void *p) { (void) p; neval++; return 1.0 / x; }
static double expm2x(double x, void *p) { (void) p; neval++; return exp(-(x + 2.0)); }
static double gauss(double x, void *p) { (void) p; neval++; return exp(-x * x); }
static double noise(double x, void *p)
{
  /* a rough deterministic integrand without sin/cos: fractional part of x*1e6 */
  double y = x * 1.0e6;
  (void) p;
  neval++;
  return 1.0e-3 * (y - floor(y));
}

typedef double (*fn_t)(double, void *);

static void table_dump(const char *tag, gsl_integration_qawo_table *t, size_t n)
{
  size_t i;
  printf("TBL %s %.17g %.17g %d %zu", tag, t->omega, t->L, (int) t->sine, n);
  for (i = 0; i < 25 * n; i++) printf(" %.17e", t->chebmo[i]);
  printf("\n");
}

static void table_case(const char *tag, double omega, double L, int sine, size_t n)
{
  gsl_integration_qawo_table *t = gsl_integration_qawo_table_alloc(
      omega, L, sine ? GSL_INTEG_SINE : GSL_INTEG_COSINE, n);
  table_dump(tag, t, n);
  gsl_integration_qawo_table_free(t);
}

static void run_qawo(const char *name, fn_t fn, double a, double omega, double L, int sine,
                     size_t levels, double epsabs, double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_integration_qawo_table *t = gsl_integration_qawo_table_alloc(
      omega, L, sine ? GSL_INTEG_SINE : GSL_INTEG_COSINE, levels);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  status = gsl_integration_qawo(&F, a, epsabs, epsrel, limit, w, t, &r, &e);
  printf("QAWO %s %d %.17e %.17e %zu %zu\n", name, status, r, e, w->size, neval);
  gsl_integration_qawo_table_free(t);
  gsl_integration_workspace_free(w);
}

static void run_qawf(const char *name, fn_t fn, double a, double omega, int sine,
                     double epsabs, size_t limit, size_t cycle_limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_integration_workspace *cw = gsl_integration_workspace_alloc(cycle_limit);
  gsl_integration_qawo_table *t = gsl_integration_qawo_table_alloc(
      omega, 1.0, sine ? GSL_INTEG_SINE : GSL_INTEG_COSINE, 25);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  status = gsl_integration_qawf(&F, a, epsabs, limit, w, cw, t, &r, &e);
  printf("QAWF %s %d %.17e %.17e %zu %zu %zu\n", name, status, r, e, w->size, cw->size, neval);
  gsl_integration_qawo_table_free(t);
  gsl_integration_workspace_free(cw);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();

  /* Moment tables: |par| <= 24 (boundary-value branch), > 24 (forward
   * recursion), and tables that cross 24 as the level halves par. */
  table_case("small_cos", 0.5, 1.0, 0, 3);
  table_case("mid_cos", 10.0, 1.0, 0, 4);
  table_case("mid_sin", 40.0, 1.0, 1, 4);
  table_case("big_cos", 100.0, 2.0, 0, 3);
  table_case("neg_sin", -15.0, 1.0, 1, 3);
  table_case("cross24_cos", 60.0, 1.0, 0, 4);
  table_case("cross24_sin", 60.0, 1.0, 1, 4);
  table_case("pi_cos", 3.141592653589793, 2.0, 0, 3);
  {
    gsl_integration_qawo_table *t = gsl_integration_qawo_table_alloc(3.0, 1.0, GSL_INTEG_COSINE, 3);
    table_dump("seq0", t, 3);
    gsl_integration_qawo_table_set_length(t, 1.0); /* same length: no-op */
    table_dump("seq_same_len", t, 3);
    gsl_integration_qawo_table_set_length(t, 2.5);
    table_dump("seq_len2.5", t, 3);
    gsl_integration_qawo_table_set(t, 7.0, 0.5, GSL_INTEG_SINE);
    table_dump("seq_set", t, 3);
    gsl_integration_qawo_table_free(t);
  }

  /* QAWO: (name, f, a, omega, L, sine, levels, epsabs, epsrel, limit) */
  run_qawo("exp_cos10", expmx, 0.0, 10.0, 1.0, 0, 25, 0.0, 1e-10, 100);
  run_qawo("x_sin40", lin, 0.0, 40.0, 1.0, 1, 25, 0.0, 1e-10, 100);
  run_qawo("one_cos_low", one, 0.0, 0.5, 1.0, 0, 10, 0.0, 1e-10, 100);
  run_qawo("one_sin_low", one, 0.0, 1.5, 1.0, 1, 10, 0.0, 1e-10, 100);
  run_qawo("invsqrt_sin20", invsqrt, 0.0, 20.0, 1.0, 1, 25, 0.0, 1e-8, 200);
  run_qawo("invsqrt_cos100", invsqrt, 0.0, 100.0, 2.0, 0, 30, 0.0, 1e-8, 300);
  run_qawo("log_cos100", logx, 0.0, 100.0, 6.283185307179586, 0, 30, 0.0, 1e-8, 300);
  run_qawo("expx_sin1000", expx, 0.0, 1000.0, 1.0, 1, 25, 0.0, 1e-10, 100);
  run_qawo("big_par_cos", expmx, 0.0, 100.0, 1.0, 0, 25, 0.0, 1e-10, 100);
  run_qawo("big_par_sin", expmx, 0.0, 100.0, 1.0, 1, 25, 0.0, 1e-10, 100);
  run_qawo("neg_omega", rec1p, 0.0, -15.0, 2.0, 1, 25, 0.0, 1e-10, 100);
  run_qawo("a_negative", lorentz, -2.0, 7.0, 4.0, 0, 25, 0.0, 1e-10, 100);
  run_qawo("gauss_cos3", gauss, -3.0, 3.0, 6.0, 0, 25, 1e-12, 0.0, 100);
  run_qawo("omega_zero_cos", expmx, 0.0, 0.0, 1.0, 0, 5, 0.0, 1e-10, 100);
  run_qawo("omega_zero_sin", expmx, 0.0, 0.0, 1.0, 1, 5, 0.0, 1e-10, 100);
  run_qawo("zero_f", zero, 0.0, 30.0, 1.0, 0, 5, 0.0, 1e-10, 100);
  run_qawo("noise_sin", noise, 0.0, 50.0, 1.0, 1, 25, 0.0, 1e-12, 200);
  run_qawo("hard_extrap", invsqrt, 0.0, 400.0, 1.0, 0, 40, 0.0, 1e-13, 500);
  run_qawo("limit1", invsqrt, 0.0, 20.0, 1.0, 1, 25, 0.0, 1e-12, 1);
  run_qawo("limit2", invsqrt, 0.0, 20.0, 1.0, 1, 25, 0.0, 1e-12, 2);
  run_qawo("limit4", invsqrt, 0.0, 20.0, 1.0, 1, 25, 0.0, 1e-12, 4);
  run_qawo("limit10", invsqrt, 0.0, 20.0, 1.0, 1, 25, 0.0, 1e-12, 10);
  run_qawo("table_n1", invsqrt, 0.0, 200.0, 1.0, 0, 1, 0.0, 1e-12, 100);
  run_qawo("table_n3", invsqrt, 0.0, 200.0, 1.0, 0, 3, 0.0, 1e-12, 100);
  run_qawo("table_n6", invsqrt, 0.0, 200.0, 1.0, 0, 6, 0.0, 1e-12, 100);
  run_qawo("badtol", one, 0.0, 10.0, 1.0, 0, 5, 0.0, 1e-17, 10);
  run_qawo("abs_only", expmx, 0.0, 12.0, 3.0, 1, 25, 1e-9, 0.0, 100);
  run_qawo("long_interval", rec1p, 0.0, 3.0, 40.0, 0, 30, 0.0, 1e-10, 300);

  /* QAWF: (name, f, a, omega, sine, epsabs, limit, cycle_limit) */
  run_qawf("expmx_cos1", expmx, 0.0, 1.0, 0, 1e-10, 50, 50);
  run_qawf("lorentz_cos2", lorentz, 0.0, 2.0, 0, 1e-10, 50, 50);
  run_qawf("rec1_sin1", rec1, 1.0, 1.0, 1, 1e-8, 50, 50);
  run_qawf("expmx_sin5", expmx, 0.0, 5.0, 1, 1e-10, 50, 50);
  run_qawf("omega0_cos", expmx, 0.0, 0.0, 0, 1e-10, 10, 100);
  run_qawf("omega0_sin", expmx, 0.0, 0.0, 1, 1e-10, 10, 100);
  run_qawf("badtol", expmx, 0.0, 1.0, 0, 0.0, 10, 10);
  run_qawf("limit3", rec1p, 0.0, 10.0, 1, 1e-10, 3, 20);
  run_qawf("limit8", rec1p, 0.0, 10.0, 1, 1e-10, 8, 20);
  run_qawf("cycle_lt_limit", expmx, 0.0, 1.0, 0, 1e-10, 20, 10);
  run_qawf("neg_omega", expmx, 0.0, -3.0, 1, 1e-10, 50, 50);
  run_qawf("invsqrt_cos3", invsqrt, 0.0, 3.0, 0, 1e-8, 50, 100);
  run_qawf("rec1p_sin10", rec1p, 0.0, 10.0, 1, 1e-8, 100, 100);
  run_qawf("a_offset", expm2x, -1.5, 2.0, 0, 1e-10, 50, 50);
  run_qawf("omega50", expmx, 0.0, 50.0, 0, 1e-10, 50, 50);
  run_qawf("tight", expmx, 0.0, 1.0, 1, 1e-14, 80, 80);
  run_qawf("const_cos", one, 0.0, 1.0, 0, 1e-8, 30, 30);
  run_qawf("tiny_eps", expmx, 0.0, 2.0, 0, 1e-300, 60, 60);
  return 0;
}
