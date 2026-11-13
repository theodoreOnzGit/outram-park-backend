/* GSL 2.8 reference driver for petir::multiroots::fdjacobian (gh:#1338 case 2).
 *
 * Calls gsl_multiroot_fdjacobian on the problems of GSL's own
 * multiroots/test_funcs.c (linked in unmodified): Rosenbrock, Roth, Powell
 * badly scaled, Brown badly scaled, Powell singular, Wood, Helical, at the
 * test initial point scaled by 1, 10, 0.1 and by 0 (all-zero x, the
 * dx = epsrel branch), for epsrel = sqrt(DBL_EPSILON) (what GSL's solvers
 * pass) and 1e-4.  Two extra functions give a null Jacobian column (GSL_ESING).
 * Output: hex bits of x, f(x), J (row-major) and the GSL status.
 *
 * build: reference-data/gsl/build_multiroots_fdjac.sh <out.txt> [GSL-dir]
 */
#include <config.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <gsl/gsl_vector.h>
#include <gsl/gsl_matrix.h>
#include <gsl/gsl_multiroots.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_machine.h>
#include "test_funcs.h"

static void hx(const char *tag, double v)
{ uint64_t u; memcpy(&u, &v, 8); printf("%s %016llx\n", tag, (unsigned long long) u); }

/* null column: f does not depend on x[1] */
static int f_nullcol(const gsl_vector *x, void *p, gsl_vector *f)
{ gsl_vector_set(f, 0, 2.0 * gsl_vector_get(x, 0)); gsl_vector_set(f, 1, -gsl_vector_get(x, 0)); return GSL_SUCCESS; }
/* dx too small to register: 1e30 + x0 absorbs the step */
static int f_absorb(const gsl_vector *x, void *p, gsl_vector *f)
{ gsl_vector_set(f, 0, 1e30 + gsl_vector_get(x, 0)); gsl_vector_set(f, 1, gsl_vector_get(x, 1) * gsl_vector_get(x, 1)); return GSL_SUCCESS; }

static void one(const char *name, gsl_multiroot_function *F, const double *x0, int n, double scale, double epsrel)
{
  gsl_vector *x = gsl_vector_alloc(n), *f = gsl_vector_alloc(n);
  gsl_matrix *J = gsl_matrix_alloc(n, n);
  for (int i = 0; i < n; i++) gsl_vector_set(x, i, scale * x0[i]);
  GSL_MULTIROOT_FN_EVAL(F, x, f);
  gsl_matrix_set_zero(J);
  int s = gsl_multiroot_fdjacobian(F, x, f, epsrel, J);
  printf("CASE %s_s%g_e%g %d %.17g\n", name, scale, epsrel, n, epsrel);
  for (int i = 0; i < n; i++) hx("X", gsl_vector_get(x, i));
  for (int i = 0; i < n; i++) hx("F", gsl_vector_get(f, i));
  for (int i = 0; i < n; i++) for (int j = 0; j < n; j++) hx("J", gsl_matrix_get(J, i, j));
  printf("STATUS %d\n", s);
  printf("END\n");
  gsl_vector_free(x); gsl_vector_free(f); gsl_matrix_free(J);
}

#define PROB(NAME, FDF, INIT, N) do { \
  gsl_vector *v = gsl_vector_alloc(N); INIT(v); double x0[N]; \
  for (int i = 0; i < N; i++) x0[i] = gsl_vector_get(v, i); \
  gsl_multiroot_function F = { (FDF).f, N, 0 }; \
  const double sc[] = { 1.0, 10.0, 0.1, 0.0 }; const double ep[] = { GSL_SQRT_DBL_EPSILON, 1e-4 }; \
  for (int a = 0; a < 4; a++) for (int b = 0; b < 2; b++) one(NAME, &F, x0, N, sc[a], ep[b]); \
  gsl_vector_free(v); } while (0)

/* Whole-solver trajectory: GSL's dnewton (fdjacobian + LU + full Newton step)
 * iterate by iterate. petir has no dnewton; the Rust test re-composes it from
 * petir's fdjacobian and LU, so this pins the composition. */
static void traj(const char *name, gsl_multiroot_function_fdf *fdf,
                 void (*init)(gsl_vector *), int n, int maxit)
{
  gsl_multiroot_function F = { fdf->f, n, 0 };
  gsl_vector *x = gsl_vector_alloc(n);
  init(x);
  gsl_multiroot_fsolver *sv = gsl_multiroot_fsolver_alloc(gsl_multiroot_fsolver_dnewton, n);
  int s = gsl_multiroot_fsolver_set(sv, &F, x);
  printf("CASE dnewton_%s %d %d\n", name, n, maxit);
  printf("SETSTATUS %d\n", s);
  for (int k = 0; k < maxit && s == 0; k++) {
    s = gsl_multiroot_fsolver_iterate(sv);
    printf("ITSTATUS %d %d\n", k + 1, s);
    if (s) break;
    for (int i = 0; i < n; i++) hx("X", gsl_vector_get(sv->x, i));
    for (int i = 0; i < n; i++) hx("F", gsl_vector_get(sv->f, i));
    for (int i = 0; i < n; i++) hx("DX", gsl_vector_get(sv->dx, i));
  }
  printf("END\n");
  gsl_multiroot_fsolver_free(sv);
  gsl_vector_free(x);
}

int main(void)
{
  gsl_set_error_handler_off();
  PROB("rosenbrock", rosenbrock, rosenbrock_initpt, 2);
  PROB("roth", roth, roth_initpt, 2);
  PROB("powellscal", powellscal, powellscal_initpt, 2);
  PROB("brownscal", brownscal, brownscal_initpt, 2);
  PROB("powellsing", powellsing, powellsing_initpt, 4);
  PROB("wood", wood, wood_initpt, 4);
  PROB("helical", helical, helical_initpt, 3);
  traj("rosenbrock", &rosenbrock, rosenbrock_initpt, 2, 15);
  traj("roth", &roth, roth_initpt, 2, 15);
  traj("powellscal", &powellscal, powellscal_initpt, 2, 15);
  traj("powellsing", &powellsing, powellsing_initpt, 4, 15);
  traj("wood", &wood, wood_initpt, 4, 15);
  traj("helical", &helical, helical_initpt, 3, 15);
  {
    double x0[2] = { 1.0, 5.0 };
    gsl_multiroot_function F1 = { f_nullcol, 2, 0 }, F2 = { f_absorb, 2, 0 };
    one("nullcol", &F1, x0, 2, 1.0, GSL_SQRT_DBL_EPSILON);
    one("absorb", &F2, x0, 2, 1.0, GSL_SQRT_DBL_EPSILON);
  }
  return 0;
}
