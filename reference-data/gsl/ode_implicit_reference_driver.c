/* GSL 2.8 reference driver for petir::ode::implicit (gh:#1338 case 5).
 *
 * #includes ode-initval2/rk1imp.c so the file-static state (y_onestep, the
 * single backward-Euler step petir ports) is readable; GSL returns the
 * two-half-step answer from rk1imp_apply, which petir deliberately does not.
 * BDF2 has no GSL counterpart; its Newton solve IS modnewton1 with
 * A = [[1/coefft]], so the driver runs modnewton1 directly with that A.
 *
 * Problems (ode-initval2/test.c): stiff linear (rhs_stiff), Van der Pol
 * mu = 10, Robertson.  Each march is FIXED step; the next state is the
 * single-step result so that every step is compared from its own start.
 * Output: hex bits.
 *
 * build:  reference-data/gsl/build_ode_implicit.sh <out.txt>
 */
#include <config.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "rk1imp.c"

static void pr(const char *tag, int k, int n, const double *v)
{
  printf("%s %d", tag, k);
  for (int i = 0; i < n; i++) {
    uint64_t u; memcpy(&u, &v[i], 8);
    printf(" %016llx", (unsigned long long) u);
  }
  printf("\n");
}

static int rhs_stiff(double t, const double y[], double f[], void *p)
{ f[0] = 998.0 * y[0] + 1998.0 * y[1]; f[1] = -999.0 * y[0] - 1999.0 * y[1]; return GSL_SUCCESS; }
static int jac_stiff(double t, const double y[], double *d, double dt[], void *p)
{ d[0] = 998.0; d[1] = 1998.0; d[2] = -999.0; d[3] = -1999.0; dt[0] = 0; dt[1] = 0; return GSL_SUCCESS; }

static int rhs_vdp(double t, const double y[], double f[], void *p)
{ const double mu = 10.0; f[0] = y[1]; f[1] = -y[0] + mu * y[1] * (1.0 - y[0] * y[0]); return GSL_SUCCESS; }
static int jac_vdp(double t, const double y[], double *d, double dt[], void *p)
{ const double mu = 10.0; d[0] = 0.0; d[1] = 1.0; d[2] = -2.0 * mu * y[0] * y[1] - 1.0;
  d[3] = mu * (1.0 - y[0] * y[0]); dt[0] = 0; dt[1] = 0; return GSL_SUCCESS; }

static int rhs_rob(double t, const double y[], double f[], void *p)
{ const double a = 0.04, b = 1.0e4, c = 3.0e7;
  f[0] = -a * y[0] + b * y[1] * y[2]; f[2] = c * y[1] * y[1]; f[1] = -f[0] - f[2]; return GSL_SUCCESS; }
static int jac_rob(double t, const double y[], double *d, double dt[], void *p)
{ const double a = 0.04, b = 1.0e4, c = 3.0e7;
  d[0] = -a; d[1] = b * y[2]; d[2] = b * y[1];
  d[3] = a; d[4] = -b * y[2] - 2 * c * y[1]; d[5] = -b * y[1];
  d[6] = 0.0; d[7] = 2 * c * y[1]; d[8] = 0.0;
  dt[0] = dt[1] = dt[2] = 0.0; return GSL_SUCCESS; }

/* the same errlev as rk1imp_apply: a_y = 1, a_dydt = 0 */
static void errlev_of(const gsl_odeiv2_driver *drv, int n, const double *y, double h, double *e)
{ for (int i = 0; i < n; i++) gsl_odeiv2_control_errlevel(drv->c, y[i], 0.0, h, i, &e[i]); }

static void run(const char *name, gsl_odeiv2_system *sys, int n, const double *y0,
                double h, int nsteps, double eabs, double erel)
{
  gsl_odeiv2_driver *drv = gsl_odeiv2_driver_alloc_y_new(sys, gsl_odeiv2_step_rk1imp, h, eabs, erel);
  /* ---- backward Euler: rk1imp_apply, read y_onestep ---- */
  rk1imp_state_t *st = rk1imp_alloc(n);
  st->driver = drv;
  double y[8], yw[8], yerr[8], t = 0.0;
  memcpy(y, y0, n * sizeof(double));
  printf("CASE be_%s %d %d %.17g %.17g %.17g\n", name, n, nsteps, h, eabs, erel);
  pr("Y", 0, n, y);
  for (int k = 0; k < nsteps; k++) {
    memcpy(yw, y, n * sizeof(double));
    int s = rk1imp_apply(st, n, t, h, yw, yerr, NULL, NULL, sys);
    if (s != GSL_SUCCESS) { printf("FAIL %d %d\n", k, s); break; }
    pr("ONE", k + 1, n, st->y_onestep);
    pr("TWO", k + 1, n, yw);
    pr("ERR", k + 1, n, yerr);
    memcpy(y, st->y_onestep, n * sizeof(double));
    t += h;
  }
  printf("END be_%s\n", name);

  /* ---- BDF2: BE first step, then modnewton1 with A = 1/coefft ---- */
  modnewton1_state_t *es = modnewton1_alloc(n, 1);
  gsl_matrix *A = gsl_matrix_alloc(1, 1);
  gsl_matrix *dfdy = gsl_matrix_alloc(n, n);
  double dfdt[8], YZ[8], fYZ[8], errlev[8], ybar[8], ynew[8], yold[8], ycur[8];
  memcpy(ycur, y0, n * sizeof(double));
  t = 0.0;
  double hold = 0.0;
  printf("CASE bdf2_%s %d %d %.17g %.17g %.17g\n", name, n, nsteps, h, eabs, erel);
  pr("Y", 0, n, ycur);
  for (int k = 0; k < nsteps; k++) {
    double a, coefft = 0, coefft00 = 0, coefft0 = 0;
    if (k == 0) { a = 1.0; memcpy(ybar, ycur, n * sizeof(double)); }
    else {
      coefft = 1.0 + h / (h + hold);
      coefft00 = h * h / (hold * (h + hold));
      coefft0 = coefft + coefft00;
      for (int i = 0; i < n; i++) ybar[i] = (coefft0 * ycur[i] - coefft00 * yold[i]) / coefft;
      a = 1.0 / coefft;
    }
    errlev_of(drv, n, ycur, h, errlev);
    GSL_ODEIV_JA_EVAL(sys, t, ycur, dfdy->data, dfdt);
    gsl_matrix_set(A, 0, 0, a);
    const double c[] = { 1.0 };
    int s = modnewton1_init(es, A, h, dfdy, sys);
    if (s == GSL_SUCCESS) s = modnewton1_solve(es, A, c, t, h, ybar, sys, YZ, errlev);
    if (s == GSL_SUCCESS) s = GSL_ODEIV_FN_EVAL(sys, t + c[0] * h, YZ, fYZ);
    if (s != GSL_SUCCESS) { printf("FAIL %d %d\n", k, s); break; }
    const double b[] = { a };
    rksubs(ynew, h, ybar, fYZ, b, 1, n);
    pr("ONE", k + 1, n, ynew);
    memcpy(yold, ycur, n * sizeof(double));
    memcpy(ycur, ynew, n * sizeof(double));
    hold = h; t += h;
  }
  printf("END bdf2_%s\n", name);
}

int main(void)
{
  gsl_set_error_handler_off();
  gsl_odeiv2_system s1 = { rhs_stiff, jac_stiff, 2, 0 };
  gsl_odeiv2_system s2 = { rhs_vdp, jac_vdp, 2, 0 };
  gsl_odeiv2_system s3 = { rhs_rob, jac_rob, 3, 0 };
  double ya[2] = { 1.0, 0.0 }, yv[2] = { 2.0, 0.0 }, yr[3] = { 1.0, 0.0, 0.0 }, yr2[3] = { 0.98, 2e-5, 0.01998 };
  run("stiff_h001", &s1, 2, ya, 0.01, 20, 1e-10, 1e-8);
  run("stiff_h05", &s1, 2, ya, 0.5, 20, 1e-10, 1e-8);
  run("stiff_tight", &s1, 2, ya, 0.01, 20, 1e-14, 1e-12);
  run("vdp_h005", &s2, 2, yv, 0.05, 40, 1e-10, 1e-8);
  run("vdp_h02", &s2, 2, yv, 0.2, 40, 1e-10, 1e-8);
  /* tight tolerance: GSL's Newton exhausts 7 iterations at step 0 -- a
     failure that petir must reproduce (Err), not a result */
  run("rob_fail", &s3, 3, yr, 0.01, 5, 1e-10, 1e-8);
  run("rob_h1e3", &s3, 3, yr2, 1e-3, 40, 1e-6, 1e-4);
  run("rob_h01", &s3, 3, yr2, 0.01, 40, 1e-6, 1e-4);
  run("rob_h1", &s3, 3, yr2, 1.0, 40, 1e-6, 1e-4);
  return 0;
}
