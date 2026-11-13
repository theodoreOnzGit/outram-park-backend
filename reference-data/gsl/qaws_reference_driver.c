/* qaws_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{QawsTable, qaws} (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Calls gsl_integration_qaws_table_alloc and gsl_integration_qaws (GSL 2.8,
 * commit cf180cd7, integration/qaws.c, qc25s.c, qmomo.c) with a workspace of
 * exactly `limit` entries and prints
 *   "TBL <alpha> <beta> <mu> <nu> ri[0..25) rj[0..25) rg[0..25) rh[0..25)"
 *   "TERR <alpha> <beta> <mu> <nu> <ok>"          alloc accepted (1) or not (0)
 *   "QAWS <name> <status> <result> <abserr> <size> <neval>"
 * at 17 significant digits. The weights call pow and log, which reach glibc
 * here and PETIR's in the port; the replaying test counts the differences.
 *
 * Build: reference-data/gsl/build_gsl_family.sh qaws_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static size_t neval;

static double one(double x, void *p) { (void) p; (void) x; neval++; return 1.0; }
static double cosx(double x, void *p) { (void) p; neval++; return cos(x); }
static double expx(double x, void *p) { (void) p; neval++; return exp(x); }
static double rec1p(double x, void *p) { (void) p; neval++; return 1.0 / (1.0 + x); }
static double lin(double x, void *p) { (void) p; neval++; return x; }
static double sin5(double x, void *p) { (void) p; neval++; return sin(5.0 * x); }
static double expmx(double x, void *p) { (void) p; neval++; return exp(-x); }
static double quad(double x, void *p) { (void) p; neval++; return 1.0 + x * x; }
static double cos10(double x, void *p) { (void) p; neval++; return cos(10.0 * x); }

typedef double (*fn_t)(double, void *);

static void table_dump(double alpha, double beta, int mu, int nu)
{
  gsl_integration_qaws_table *t = gsl_integration_qaws_table_alloc(alpha, beta, mu, nu);
  int i;
  printf("TERR %.17g %.17g %d %d %d\n", alpha, beta, mu, nu, t != NULL);
  if (t == NULL) return;
  printf("TBL %.17g %.17g %d %d", alpha, beta, mu, nu);
  for (i = 0; i < 25; i++) printf(" %.17g", t->ri[i]);
  for (i = 0; i < 25; i++) printf(" %.17g", t->rj[i]);
  for (i = 0; i < 25; i++) printf(" %.17g", t->rg[i]);
  for (i = 0; i < 25; i++) printf(" %.17g", t->rh[i]);
  printf("\n");
  gsl_integration_qaws_table_free(t);
}

static void run(const char *name, fn_t fn, double a, double b, double alpha, double beta,
                int mu, int nu, double epsabs, double epsrel, size_t limit)
{
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(limit);
  gsl_integration_qaws_table *t = gsl_integration_qaws_table_alloc(alpha, beta, mu, nu);
  gsl_function F;
  double r = 0, e = 0;
  int status;
  F.function = fn;
  F.params = NULL;
  neval = 0;
  status = gsl_integration_qaws(&F, a, b, t, epsabs, epsrel, limit, w, &r, &e);
  printf("QAWS %s %d %.17e %.17e %zu %zu\n", name, status, r, e, w->size, neval);
  gsl_integration_qaws_table_free(t);
  gsl_integration_workspace_free(w);
}

int main(void)
{
  gsl_set_error_handler_off();
  table_dump(0.0, 0.0, 1, 0);
  table_dump(-0.5, -0.5, 0, 0);
  table_dump(-0.5, 0.0, 0, 0);
  table_dump(0.0, -0.7, 0, 1);
  table_dump(0.5, 0.5, 1, 1);
  table_dump(1.5, -0.3, 1, 0);
  table_dump(-0.9, -0.9, 0, 0);
  table_dump(2.0, 3.0, 1, 1);
  table_dump(-1.5, 0.0, 0, 0);
  table_dump(0.0, -1.5, 0, 0);
  table_dump(0.0, 0.0, 2, 0);
  table_dump(0.0, 0.0, 0, -1);

  run("log_only", one, 0.0, 1.0, 0.0, 0.0, 1, 0, 0.0, 1e-10, 100);
  run("sqrt_ends", one, 0.0, 1.0, -0.5, -0.5, 0, 0, 0.0, 1e-9, 100);
  run("alg_left", cosx, 0.0, 2.0, -0.5, 0.0, 0, 0, 0.0, 1e-10, 100);
  run("alg_right", expx, 0.0, 1.0, 0.0, -0.7, 0, 0, 0.0, 1e-10, 100);
  run("both_log", rec1p, 0.0, 3.0, 0.5, 0.5, 1, 1, 0.0, 1e-10, 100);
  run("nu_only", lin, 0.5, 2.0, 0.0, 0.0, 0, 1, 0.0, 1e-10, 100);
  run("plain", sin5, 0.0, 2.0, 0.0, 0.0, 0, 0, 0.0, 1e-10, 100);
  run("mix", expmx, -1.0, 1.0, 1.5, -0.3, 1, 0, 0.0, 1e-10, 100);
  run("hard", quad, 0.0, 1.0, -0.9, -0.9, 0, 0, 0.0, 1e-12, 200);
  /* limit 2 is omitted on purpose: after the two initial intervals the first
     bisection appends a third, writing past a workspace of two (GSL bug, seen
     as size 3 and status 5 with limit 2); PETIR refuses it as Invalid. */
  run("limit3", cos10, 0.0, 1.0, -0.5, -0.5, 0, 0, 0.0, 1e-12, 3);
  run("limit4", cos10, 0.0, 1.0, -0.5, -0.5, 0, 0, 0.0, 1e-12, 4);
  run("limit5", cos10, 0.0, 1.0, -0.5, -0.5, 0, 0, 0.0, 1e-12, 5);
  run("abs_only", expx, 0.0, 1.0, -0.5, 0.0, 0, 0, 1e-8, 0.0, 100);
  run("b_lt_a", one, 1.0, 0.0, 0.0, 0.0, 0, 0, 0.0, 1e-8, 10);
  run("b_eq_a", one, 1.0, 1.0, 0.0, 0.0, 0, 0, 0.0, 1e-8, 10);
  run("badtol", one, 0.0, 1.0, 0.0, 0.0, 0, 0, 0.0, 1e-17, 10);
  run("both_sing_hard", cos10, 0.0, 3.0, -0.95, -0.95, 1, 1, 0.0, 1e-13, 300);
  return 0;
}
