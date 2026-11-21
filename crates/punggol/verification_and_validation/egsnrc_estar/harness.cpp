#include <cstdio>
#include <string>
#include "estar_formulaCalcs.h"
using namespace std;
int main() {
  struct C { const char* f[4]; double w[4]; int n; };
  C cs[] = {
    {{"H2O"}, {1.0}, 1}, {{"H2O","C1H4"}, {0.25,0.75}, 2}, {{"Na1Cl1","H2O","O2"}, {1.0,2.0,0.5}, 3},
    {{"Si1O2","Al2O3","Fe2O3","Ca1O1"}, {60.0,15.0,7.0,3.0}, 4}, {{"U1O2","H2O"}, {0.9,0.1}, 2},
  };
  for (auto& c : cs) {
    string f[4]; double w[4];
    for (int i = 0; i < c.n; ++i) { f[i] = c.f[i]; w[i] = c.w[i]; }
    formula_calc fc = mixtureCalculation(1.0, f, w, c.n);
    printf("case %d", c.n);
    for (int i = 0; i < c.n; ++i) printf(" %s:%.17g", c.f[i], c.w[i]);
    printf(" | %d", fc.mmax);
    for (int i = 0; i < fc.mmax; ++i) printf(" %d %.17g", fc.jz[i], fc.wt[i]);
    printf("\n");
  }
}
