/* Reference driver: Reference-LAPACK DGEBAL and DGEBAK, for
 * petir::linalg::gebal (gh:#998).
 *
 * Usage (from the repository root): see README.md in this directory.
 *   gebal_reference_driver <expm-octave-reference.txt>
 *
 * Cases: the input matrices A of the Octave expm reference (so the balancing
 * expm.m performs is checked on exactly its inputs), plus generated ones:
 * isolated rows/columns, a zero matrix, entries near the dnrm2 and DGEBAL
 * safe-range thresholds, and dense random matrices with rows/columns scaled
 * over 16 decades. For every case and every JOB in N, P, S, B it writes
 * INFO, ILO, IHI, SCALE, the balanced matrix, and DGEBAK of a fixed n x 3
 * matrix VIN for SIDE = R and L. Format (shared by every petir linalg
 * reference): CASE <name> <n>, <TAG> <i> [<j>] <value> with zero-based
 * indices and %.17e, END <name>.
 */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void dgebal_(const char *job, const int *n, double *a, const int *lda, int *ilo, int *ihi,
                    double *scale, int *info, size_t joblen);
extern void dgebak_(const char *job, const char *side, const int *n, const int *ilo,
                    const int *ihi, const double *scale, const int *m, double *v, const int *ldv,
                    int *info, size_t joblen, size_t sidelen);

static unsigned long long st = 12345;
static double urand(void) { /* splitmix64 -> [0,1) */
  unsigned long long z = (st += 0x9e3779b97f4a7c15ull);
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
  z ^= z >> 31;
  return (double)(z >> 11) * 0x1p-53;
}

/* a is row-major n x n here; LAPACK gets the column-major copy */
static void run_case(const char *name, int n, const double *a) {
  printf("CASE %s %d\n", name, n);
  for (int i = 0; i < n; i++)
    for (int j = 0; j < n; j++) printf("A %d %d %.17e\n", i, j, a[i * n + j]);
  const int m = 3;
  double *vin = malloc(sizeof(double) * n * m);
  for (int i = 0; i < n; i++)
    for (int j = 0; j < m; j++) {
      vin[i * m + j] = urand() - 0.5;
      printf("VIN %d %d %.17e\n", i, j, vin[i * m + j]);
    }
  const char *jobs = "NPSB";
  for (int jb = 0; jb < 4; jb++) {
    char job = jobs[jb];
    double *cm = malloc(sizeof(double) * n * n), *scale = malloc(sizeof(double) * n);
    for (int i = 0; i < n; i++)
      for (int j = 0; j < n; j++) cm[j * n + i] = a[i * n + j];
    int ilo, ihi, info, lda = n;
    dgebal_(&job, &n, cm, &lda, &ilo, &ihi, scale, &info, 1);
    printf("INFO_%c %d\nILO_%c %d\nIHI_%c %d\n", job, info, job, ilo, job, ihi);
    for (int i = 0; i < n; i++) printf("SCALE_%c %d %.17e\n", job, i, scale[i]);
    for (int i = 0; i < n; i++)
      for (int j = 0; j < n; j++) printf("BAL_%c %d %d %.17e\n", job, i, j, cm[j * n + i]);
    for (int sd = 0; sd < 2; sd++) {
      char side = sd ? 'L' : 'R';
      double *v = malloc(sizeof(double) * n * m);
      for (int i = 0; i < n; i++)
        for (int j = 0; j < m; j++) v[j * n + i] = vin[i * m + j];
      int binfo, ldv = n;
      dgebak_(&job, &side, &n, &ilo, &ihi, scale, &m, v, &ldv, &binfo, 1, 1);
      printf("BAKINFO%c_%c %d\n", side, job, binfo);
      for (int i = 0; i < n; i++)
        for (int j = 0; j < m; j++) printf("BAK%c_%c %d %d %.17e\n", side, job, i, j, v[j * n + i]);
      free(v);
    }
    free(cm);
    free(scale);
  }
  printf("END %s\n", name);
  free(vin);
}

/* read the A blocks of the Octave expm reference */
static void octave_cases(const char *path) {
  FILE *f = fopen(path, "r");
  if (!f) { perror(path); exit(1); }
  char line[256], name[64] = "";
  int n = 0;
  double *a = NULL;
  while (fgets(line, sizeof line, f)) {
    char tag[16];
    if (sscanf(line, "CASE %63s %d", name, &n) == 2) {
      free(a);
      a = calloc((size_t)n * n, sizeof(double));
    } else if (!strncmp(line, "A ", 2)) {
      int i, j; double v;
      sscanf(line, "%15s %d %d %lf", tag, &i, &j, &v);
      a[i * n + j] = v;
    } else if (!strncmp(line, "END ", 4)) {
      char full[80];
      snprintf(full, sizeof full, "octave_%s", name);
      run_case(full, n, a);
    }
  }
  free(a);
  fclose(f);
}

int main(int argc, char **argv) {
  if (argc < 2) { fprintf(stderr, "usage: %s expm-octave-reference.txt\n", argv[0]); return 1; }
  octave_cases(argv[1]);

  /* 5x5 with an isolatable row (3) and column (1), and a zero row elsewhere */
  double iso5[25] = {2, 1, 3, 0, 4,
                     0, 5, 0, 0, 0,
                     1, 2, 3, 4, 5,
                     0, 7, 1, 8, 0,
                     0, 1, 1, 0, 9};
  run_case("isolate5", 5, iso5);
  double zero3[9] = {0};
  run_case("zero3", 3, zero3);
  double one1[1] = {3.5};
  run_case("one1", 1, one1);
  /* zero row/col inside the block: c == 0 or r == 0 */
  double holes4[16] = {1, 2, 0, 3,
                       4, 5, 0, 6,
                       7, 8, 9, 1,
                       2, 3, 0, 4};
  run_case("holes4", 4, holes4);
  /* Found by searching 0/1 patterns: the column search gives a different
     SCALE if its inner DO I = K, L used the K of the OUTER loop's entry
     instead of the current K. Pins Fortran's DO-bound semantics. */
  double loop6[36] = {1, 0, 2, 0, 0, 0,
                      0, 3, 0, 0, 0, 0,
                      4, 0, 5, 0, 0, 0,
                      0, 0, 0, 6, 7, 8,
                      0, 0, 9, 0, 10, 0,
                      0, 0, 11, 0, 0, 12};
  run_case("loopbound6", 6, loop6);
  /* dnrm2 big and small accumulators, and DGEBAL's safe-range guards */
  double ext4[16] = {1.0, 0x1p600, 0x1p-600, 3.0,
                     0x1p-700, 2.0, 0x1p520, 0x1p-520,
                     0x1p700, 0x1p-490, 5.0, 0x1p490,
                     7.0, 0x1p-1000, 0x1p1000, 1.0};
  run_case("extreme4", 4, ext4);
  double guard3[9] = {1.0, 0x1p960, 0.0,
                      0x1p-960, 1.0, 0x1p960,
                      0.0, 0x1p-960, 1.0};
  run_case("guard3", 3, guard3);
  double sub3[9] = {0x1p-1060, 0x1p-1070, 1.0,
                    0x1p-1050, 0x1p-1060, 0x1p-1072,
                    1.0, 0x1p-1066, 0x1p-1060};
  run_case("subnormal3", 3, sub3);
  /* dense random with rows/columns scaled by 10^U(-8, 8) */
  int ns[3] = {6, 10, 25};
  for (int t = 0; t < 3; t++) {
    int n = ns[t];
    double *a = malloc(sizeof(double) * n * n), *rs = malloc(sizeof(double) * n);
    for (int i = 0; i < n; i++) rs[i] = pow(10.0, 16.0 * urand() - 8.0);
    for (int i = 0; i < n; i++)
      for (int j = 0; j < n; j++) a[i * n + j] = (urand() - 0.5) * rs[i] / rs[j];
    char name[32];
    snprintf(name, sizeof name, "scaledrand%d", n);
    run_case(name, n, a);
    /* the same with a few exact zeros, so permutation and scaling interact */
    for (int i = 0; i < n; i++) a[(n - 1) * n + i] = (i == n - 1) ? a[(n - 1) * n + i] : 0.0;
    for (int i = 0; i < n; i++) a[i * n + 0] = (i == 0) ? a[0] : 0.0;
    snprintf(name, sizeof name, "scaledrandiso%d", n);
    run_case(name, n, a);
    free(a);
    free(rs);
  }
  /* sparse random patterns (about 70 % zeros), n = 4..9: many orders in
     which rows and columns become isolatable, so the permutation search's
     loop-bound semantics are exercised, not just its common path */
  for (int t = 0; t < 40; t++) {
    int n = 4 + t % 6;
    double *a = malloc(sizeof(double) * n * n);
    for (int i = 0; i < n * n; i++) a[i] = (urand() < 0.3) ? ldexp(urand() + 0.5, (int)(40 * urand()) - 20) : 0.0;
    char name[32];
    snprintf(name, sizeof name, "sparse%02d_%d", t, n);
    run_case(name, n, a);
    free(a);
  }
  return 0;
}
