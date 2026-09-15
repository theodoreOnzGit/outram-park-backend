/* Reference driver: the six GSL-derived petir modules that had no
 * compiled-GSL reference at the time petir was declared mature (2026-09-15):
 * roots, min, deriv, interp, integration, ode.
 *
 * DESIGN NOTE, and the reason this is worth more than a final-answer check:
 * for the ITERATIVE routines (roots, min) the driver emits the whole ITERATE
 * SEQUENCE, not just the converged result. Any correct bisection and any
 * correct Brent converge to the same root, so comparing final answers would
 * pass against a port of a different algorithm. The trajectory is what pins
 * the algorithm actually implemented.
 *
 * Build: see README.md in this directory.
 */
#include <stdio.h>
#include <math.h>
#include <gsl/gsl_math.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_roots.h>
#include <gsl/gsl_min.h>
#include <gsl/gsl_deriv.h>
#include <gsl/gsl_interp.h>
#include <gsl/gsl_integration.h>
#include <gsl/gsl_odeiv2.h>

/* ---- shared test functions ------------------------------------------- */
/* x^2 - 5: root sqrt(5), the case GSL's own roots/test.c uses. */
static double f_quad(double x, void *p) { (void) p; return x * x - 5.0; }
static double df_quad(double x, void *p) { (void) p; return 2.0 * x; }
static void fdf_quad(double x, void *p, double *y, double *dy)
{ *y = f_quad(x, p); *dy = df_quad(x, p); }

/* cos(x) + 1: minimum at pi, GSL's min/test.c case. */
static double f_cos(double x, void *p) { (void) p; return cos(x) + 1.0; }
static double f_exp(double x, void *p) { (void) p; return exp(x); }
static double f_runge(double x, void *p) { (void) p; return 1.0 / (1.0 + 25.0 * x * x); }
static double f_sin(double x, void *p) { (void) p; return sin(x); }

/* ---- roots ------------------------------------------------------------ */
static void roots_bracketing(const char *name, const gsl_root_fsolver_type *T,
                             double lo, double hi)
{
  gsl_root_fsolver *s = gsl_root_fsolver_alloc(T);
  gsl_function F; int i;
  F.function = f_quad; F.params = 0;
  gsl_root_fsolver_set(s, &F, lo, hi);
  printf("CASE roots_%s\n", name);
  for (i = 0; i < 30; i++) {
    gsl_root_fsolver_iterate(s);
    printf("IT %d %.17e %.17e %.17e\n", i,
           gsl_root_fsolver_root(s),
           gsl_root_fsolver_x_lower(s),
           gsl_root_fsolver_x_upper(s));
  }
  printf("END roots_%s\n", name);
  gsl_root_fsolver_free(s);
}

static void roots_polishing(const char *name, const gsl_root_fdfsolver_type *T,
                            double guess)
{
  gsl_root_fdfsolver *s = gsl_root_fdfsolver_alloc(T);
  gsl_function_fdf FDF; int i;
  FDF.f = f_quad; FDF.df = df_quad; FDF.fdf = fdf_quad; FDF.params = 0;
  gsl_root_fdfsolver_set(s, &FDF, guess);
  printf("CASE roots_%s\n", name);
  for (i = 0; i < 12; i++) {
    gsl_root_fdfsolver_iterate(s);
    printf("IT %d %.17e\n", i, gsl_root_fdfsolver_root(s));
  }
  printf("END roots_%s\n", name);
  gsl_root_fdfsolver_free(s);
}

/* ---- min -------------------------------------------------------------- */
static void minimise(const char *name, const gsl_min_fminimizer_type *T)
{
  gsl_min_fminimizer *s = gsl_min_fminimizer_alloc(T);
  gsl_function F; int i;
  F.function = f_cos; F.params = 0;
  gsl_min_fminimizer_set(s, &F, 3.0, 0.0, 6.0);
  printf("CASE min_%s\n", name);
  for (i = 0; i < 25; i++) {
    gsl_min_fminimizer_iterate(s);
    printf("IT %d %.17e %.17e %.17e\n", i,
           gsl_min_fminimizer_x_minimum(s),
           gsl_min_fminimizer_x_lower(s),
           gsl_min_fminimizer_x_upper(s));
  }
  printf("END min_%s\n", name);
  gsl_min_fminimizer_free(s);
}

/* ---- deriv ------------------------------------------------------------ */
static void derivatives(void)
{
  gsl_function F; int k;
  F.function = f_exp; F.params = 0;
  printf("CASE deriv_exp\n");
  for (k = 0; k <= 12; k++) {
    double x = -1.0 + 0.25 * k, r, e;
    gsl_deriv_central(&F, x, 1e-4, &r, &e);
    printf("C %d %.17e %.17e %.17e\n", k, x, r, e);
    gsl_deriv_forward(&F, x, 1e-4, &r, &e);
    printf("F %d %.17e %.17e %.17e\n", k, x, r, e);
    gsl_deriv_backward(&F, x, 1e-4, &r, &e);
    printf("B %d %.17e %.17e %.17e\n", k, x, r, e);
  }
  printf("END deriv_exp\n");
}

/* ---- interp ----------------------------------------------------------- */
static void interpolate(const char *name, const gsl_interp_type *T)
{
  enum { N = 9 };
  double xa[N], ya[N];
  gsl_interp *s = gsl_interp_alloc(T, N);
  gsl_interp_accel *acc = gsl_interp_accel_alloc();
  int i, k;
  for (i = 0; i < N; i++) {
    xa[i] = -1.0 + 2.0 * i / (double) (N - 1);
    ya[i] = 1.0 / (1.0 + 25.0 * xa[i] * xa[i]);
  }
  gsl_interp_init(s, xa, ya, N);
  printf("CASE interp_%s\n", name);
  for (k = 0; k <= 60; k++) {
    double x = -1.0 + 2.0 * k / 60.0;
    printf("P %d %.17e %.17e %.17e\n", k, x,
           gsl_interp_eval(s, xa, ya, x, acc),
           gsl_interp_eval_deriv(s, xa, ya, x, acc));
  }
  printf("END interp_%s\n", name);
  gsl_interp_accel_free(acc);
  gsl_interp_free(s);
}

/* ---- integration ------------------------------------------------------ */
static void quadrature(void)
{
  gsl_function F;
  double r, e, ra, ea;
  size_t n;
  gsl_integration_workspace *w = gsl_integration_workspace_alloc(200);

  F.function = f_runge; F.params = 0;
  printf("CASE integration\n");

  gsl_integration_qk15(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 15 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);
  gsl_integration_qk21(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 21 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);
  gsl_integration_qk31(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 31 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);
  gsl_integration_qk41(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 41 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);
  gsl_integration_qk51(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 51 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);
  gsl_integration_qk61(&F, -1.0, 1.0, &r, &e, &ra, &ea);
  printf("QK 61 %.17e %.17e %.17e %.17e\n", r, e, ra, ea);

  F.function = f_sin; F.params = 0;
  gsl_integration_qag(&F, 0.0, M_PI, 0.0, 1e-10, 200, GSL_INTEG_GAUSS21, w, &r, &e);
  printf("QAG sin %.17e %.17e\n", r, e);
  F.function = f_runge; F.params = 0;
  gsl_integration_qag(&F, -1.0, 1.0, 0.0, 1e-10, 200, GSL_INTEG_GAUSS21, w, &r, &e);
  printf("QAG runge %.17e %.17e\n", r, e);
  (void) n;
  printf("END integration\n");
  gsl_integration_workspace_free(w);
}

/* ---- ode -------------------------------------------------------------- */
static int rhs_exp(double t, const double y[], double dydt[], void *p)
{ (void) t; (void) p; dydt[0] = y[0]; return GSL_SUCCESS; }

static int rhs_osc(double t, const double y[], double dydt[], void *p)
{ (void) t; (void) p; dydt[0] = y[1]; dydt[1] = -y[0]; return GSL_SUCCESS; }

static void ode_steps(const char *name, const gsl_odeiv2_step_type *T,
                      int dim, int (*rhs)(double, const double[], double[], void *))
{
  gsl_odeiv2_step *s = gsl_odeiv2_step_alloc(T, dim);
  gsl_odeiv2_system sys = { rhs, 0, (size_t) dim, 0 };
  double y[2], yerr[2];
  double t = 0.0, h = 0.05;
  int i, j;

  y[0] = 1.0; y[1] = 0.0;
  printf("CASE ode_%s %d\n", name, dim);
  for (i = 0; i < 20; i++) {
    gsl_odeiv2_step_apply(s, t, h, y, yerr, 0, 0, &sys);
    t += h;
    printf("S %d %.17e", i, t);
    for (j = 0; j < dim; j++) printf(" %.17e %.17e", y[j], yerr[j]);
    printf("\n");
  }
  printf("END ode_%s\n", name);
  gsl_odeiv2_step_free(s);
}

int main(void)
{
  gsl_set_error_handler_off();

  roots_bracketing("bisection", gsl_root_fsolver_bisection, 0.0, 5.0);
  roots_bracketing("falsepos", gsl_root_fsolver_falsepos, 0.0, 5.0);
  roots_bracketing("brent", gsl_root_fsolver_brent, 0.0, 5.0);
  roots_polishing("newton", gsl_root_fdfsolver_newton, 5.0);
  roots_polishing("secant", gsl_root_fdfsolver_secant, 5.0);
  roots_polishing("steffenson", gsl_root_fdfsolver_steffenson, 5.0);

  minimise("goldensection", gsl_min_fminimizer_goldensection);
  minimise("brent", gsl_min_fminimizer_brent);

  derivatives();

  interpolate("linear", gsl_interp_linear);
  interpolate("cspline", gsl_interp_cspline);

  quadrature();

  ode_steps("rk4", gsl_odeiv2_step_rk4, 1, rhs_exp);
  ode_steps("rkf45", gsl_odeiv2_step_rkf45, 1, rhs_exp);
  ode_steps("rkf45osc", gsl_odeiv2_step_rkf45, 2, rhs_osc);

  return 0;
}
