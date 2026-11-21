// Differential driver for estar.cpp:estarCalculation (gh:#1696). Includes the
// upstream source directly with the Fortran-binding macros stubbed; the
// egsFatal/egsInformation stubs are in egs_functions_stub.h (renamed
// egs_functions.h on the include path). Not committed with any upstream code.
#define F77_OBJ_(a, b) a
#define __extc__
#define ESTAR_EXPORT
#include "estar.cpp"
#include "estar_formulaCalcs.cpp"
#include "estar_dataParser.cpp"
#include "estar_dataTables.cpp"

struct Case { int iscomp; float rho; float ipot; int n; const char* e[4]; double w[4]; float a[4]; };

int main() {
  Case cs[] = {
    {0, 8.375e-5f, -1, 1, {"H"}, {1}, {1}},
    {0, 0.0708f, -1, 1, {"H"}, {1}, {1}},
    {0, 2.699f, -1, 1, {"Al"}, {1}, {1}},       // conductor
    {0, 2.265f, -1, 1, {"C"}, {1}, {1}},        // elemental carbon special case
    {0, 11.35f, -1, 1, {"Pb"}, {1}, {1}},
    {0, 8.96f, -1, 1, {"Cu"}, {1}, {1}},
    {0, 19.32f, -1, 1, {"Au"}, {1}, {1}},
    {0, 0.001205f, -1, 1, {"N"}, {1}, {1}},     // gas
    {1, 1.0f, -1, 2, {"H", "O"}, {0, 0}, {2, 1}},           // water
    {1, 1.0f, 75.0f, 2, {"H", "O"}, {0, 0}, {2, 1}},        // water, user I-value
    {1, 3.67f, -1, 2, {"Na", "I"}, {0, 0}, {1, 1}},         // NaI
    {1, 2.32f, -1, 2, {"Si", "O"}, {0, 0}, {1, 2}},         // SiO2
    {1, 0.000667f, -1, 2, {"C", "H"}, {0, 0}, {1, 4}},      // methane gas
    {1, 1.0f, -1, 3, {"H", "O", "H"}, {0, 0, 0}, {1, 1, 1}},// repeated element merge
    {2, 1.205e-3f, -1, 2, {"N2", "O2"}, {0.78, 0.22}, {0, 0}},
    {2, 1.0f, -1, 2, {"H2O", "C1H4"}, {0.25, 0.75}, {0, 0}},
    {2, 2.0f, -1, 3, {"Si1O2", "Al2O3", "Fe2O3"}, {60, 15, 7}, {0, 0, 0}},
  };
  for (auto& c : cs) {
    string el[4]; double w[4]; float a[4];
    for (int i = 0; i < c.n; ++i) { el[i] = c.e[i]; w[i] = c.w[i]; a[i] = c.a[i]; }
    double dc[113], en[113]; float mi; float ip = c.ipot;
    fprintf(stderr, "running %s n=%d rho=%g\n", c.e[0], c.n, c.rho); int r = estarCalculation(c.iscomp == 1, c.n, c.rho, el, w, a, dc, en, &mi, &ip, 1, "");
    printf("case %d %d %.9g %.9g", c.iscomp == 1 ? 1 : 0, c.n, c.rho, c.ipot);
    for (int i = 0; i < c.n; ++i) printf(" %s:%.17g:%.9g", c.e[i], c.w[i], c.a[i]);
    printf(" | %d %.9g |", r, mi);
    for (int i = 0; i < 113; ++i) printf(" %.17g", dc[i]);
    printf("\n");
  }
}
