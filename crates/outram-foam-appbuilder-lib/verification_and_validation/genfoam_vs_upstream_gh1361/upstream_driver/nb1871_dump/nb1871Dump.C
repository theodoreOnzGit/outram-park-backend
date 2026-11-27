/*---------------------------------------------------------------------------*\
  gh:#1871 round 51 driver. NOT part of GeN-Foam and NOT the upstream class:
  NusseltThermalBaffle1D needs a mapped baffle pair, a `fluid` and an `FSPair`
  (a solver) and is not constructible here. This program is a LITERAL
  TRANSCRIPTION of the arithmetic of NusseltThermalBaffle1DFvPatchScalarField.C
  (GeN-Foam 652b3da): the constructor's hw_ lines, calcH, the two branches of
  updateCoeffs() and evaluate(), written with the same OpenFOAM scalarField
  expressions (field operators, max(field, scalar), pow(field, scalar)), on
  closed-form inputs. It checks the Rust translation, not the upstream class.
  Lines: "N <variant> <step> <name> v..." at %.17g.   Usage: nb1871Dump <out.txt>
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "scalarField.H"
#include <cstdio>

using namespace Foam;

static double fr(long a, long b, long i, long m)
{
    return double((a*1000003L + b*i) % m)/double(m);
}

static void pr(FILE* f, const char* var, int step, const char* name, const scalarField& v)
{
    std::fprintf(f, "N %s %d %s", var, step, name);
    forAll(v, i) std::fprintf(f, " %.17g", v[i]);
    std::fprintf(f, "\n");
}

static scalarField mkf(int n, long a, long b, long step, long m, double lo, double span)
{
    scalarField r(n);
    forAll(r, i) r[i] = lo + span*fr(a, b, i + 7*step, m);
    return r;
}

int main(int argc, char* argv[])
{
    FILE* f = std::fopen(argv[1], "w");
    const int n = 8;
    // correlation coefficients of the two sides (const, coeff, expRe, expPr)
    const scalar cnst = 0.0, coeff = 0.023, expRe = 0.8, expPr = 0.4;
    const scalar cnst2 = 4.36, coeff2 = 0.01, expRe2 = 0.7, expPr2 = 0.33;
    const char* variants[3] = {"onePhase", "twoPhase", "thin"};
    for (int v = 0; v < 3; ++v)
    {
        scalar kw_ = 15.0, tw_ = (v == 2) ? 0.0 : 0.002;
        scalar hw_ = max(kw_/max(tw_, 1e-69), 1e-69);
        hw_ = max(hw_, 1e-69);
        std::fprintf(f, "N %s 0 hw %.17g\n", variants[v], hw_);
        for (int s = 1; s <= 12; ++s)
        {
            scalarField Re1 = mkf(n, 1, 3, s, 97, 5000.0, 20000.0), Re2 = mkf(n, 2, 5, s, 89, 5000.0, 20000.0);
            scalarField Re3 = mkf(n, 3, 7, s, 83, 5000.0, 20000.0), Re4 = mkf(n, 4, 9, s, 79, 5000.0, 20000.0);
            scalarField Pr1 = mkf(n, 5, 2, s, 73, 0.6, 0.5), Pr2 = mkf(n, 6, 4, s, 71, 0.6, 0.5);
            scalarField k1 = mkf(n, 7, 6, s, 67, 0.4, 1.0), k2 = mkf(n, 8, 8, s, 61, 0.4, 1.0);
            scalarField Dh = mkf(n, 9, 1, s, 59, 0.008, 0.01);
            scalarField fF = mkf(n, 10, 3, s, 53, 0.5, 0.5);
            scalarField Ti = mkf(n, 11, 5, s, 101, 400.0, 300.0), nbrTi = mkf(n, 12, 7, s, 103, 400.0, 300.0);
            scalarField oTi = mkf(n, 13, 9, s, 107, 400.0, 300.0), oNbrTi = mkf(n, 14, 11, s, 109, 400.0, 300.0);
            scalarField kfEff = mkf(n, 15, 2, s, 113, 0.5, 2.0), nbrKfEff = mkf(n, 16, 4, s, 127, 0.5, 2.0);
            scalarField oKfEff = mkf(n, 17, 6, s, 131, 0.5, 2.0), oNbrKfEff = mkf(n, 18, 8, s, 137, 0.5, 2.0);
            scalarField dc = mkf(n, 19, 3, s, 139, 5.0, 50.0);
            pr(f, variants[v], s, "Re1", Re1); pr(f, variants[v], s, "Pr1", Pr1); pr(f, variants[v], s, "k1", k1);
            pr(f, variants[v], s, "Dh", Dh); pr(f, variants[v], s, "fF", fF);

            // calcH, without and with the f factor (onePhase / twoPhase pair)
            scalarField hf((cnst + coeff*pow(Re1, expRe)*pow(Pr1, expPr))*k1/Dh);
            scalarField nbrHf((cnst2 + coeff2*pow(Re2, expRe2)*pow(Pr2, expPr2))*k2/Dh);
            scalarField hfF((cnst + coeff*pow(Re1, expRe)*pow(Pr1, expPr))*fF*k1/Dh);
            pr(f, variants[v], s, "hf", hf); pr(f, variants[v], s, "nbrHf", nbrHf); pr(f, variants[v], s, "hfF", hfF);

            if (v == 0 || v == 2)
            {
                scalarField A(1.0+hf/hw_);
                scalarField B((hf/hw_)*Ti);
                scalarField nbrA(1.0+nbrHf/hw_);
                scalarField nbrB((nbrHf/hw_)*nbrTi);
                scalarField oneByAnbrAminOne(max(A*nbrA-1.0, 1e-69));
                scalarField Tw((B*nbrA+nbrB)/oneByAnbrAminOne);
                scalarField nbrTw((nbrB*A+B)/oneByAnbrAminOne);
                scalarField refGrad = (hf/kfEff)*(Tw-Ti);
                scalarField nbrRefGrad = (nbrHf/nbrKfEff)*(nbrTw-nbrTi);
                pr(f, variants[v], s, "Tw", Tw); pr(f, variants[v], s, "nbrTw", nbrTw);
                pr(f, variants[v], s, "refGrad", refGrad); pr(f, variants[v], s, "nbrRefGrad", nbrRefGrad);
                scalarField valueFraction(n, 0.0);
                scalarField refValue(mkf(n, 20, 5, s, 149, 300.0, 100.0));
                scalarField val
                (
                    valueFraction*refValue
                  + (1.0 - valueFraction)*(Ti + refGrad/max(dc, 1e-69))
                );
                pr(f, variants[v], s, "value", val);
            }
            else
            {
                scalarField oHf(hfF), oNbrHf(nbrHf*fF);
                scalarField hm(hf+oHf);
                scalarField nbrHm(nbrHf+oNbrHf);
                scalarField A(1.0+hm/hw_);
                scalarField B((hf*Ti+oHf*oTi)/hw_);
                scalarField nbrA(1.0+nbrHf/hw_);
                scalarField nbrB((nbrHf*nbrTi+oNbrHf*oNbrTi)/hw_);
                scalarField oneByAnbrAminOne(max(A*nbrA-1.0, 1e-69));
                scalarField Tw((B*nbrA+nbrB)/oneByAnbrAminOne);
                scalarField nbrTw((nbrB*A+B)/oneByAnbrAminOne);
                scalarField refGrad = (hf/kfEff)*(Tw-Ti);
                scalarField nbrRefGrad = (nbrHf/nbrKfEff)*(nbrTw-nbrTi);
                scalarField oRefGrad = (oHf/oKfEff)*(Tw-oTi);
                scalarField oNbrRefGrad = (oNbrHf/oNbrKfEff)*(nbrTw-oNbrTi);
                pr(f, variants[v], s, "oHf", oHf); pr(f, variants[v], s, "oNbrHf", oNbrHf);
                pr(f, variants[v], s, "Tw", Tw); pr(f, variants[v], s, "nbrTw", nbrTw);
                pr(f, variants[v], s, "refGrad", refGrad); pr(f, variants[v], s, "nbrRefGrad", nbrRefGrad);
                pr(f, variants[v], s, "oRefGrad", oRefGrad); pr(f, variants[v], s, "oNbrRefGrad", oNbrRefGrad);
            }
            pr(f, variants[v], s, "Ti", Ti); pr(f, variants[v], s, "nbrTi", nbrTi);
            pr(f, variants[v], s, "oTi", oTi); pr(f, variants[v], s, "oNbrTi", oNbrTi);
            pr(f, variants[v], s, "kfEff", kfEff); pr(f, variants[v], s, "nbrKfEff", nbrKfEff);
            pr(f, variants[v], s, "oKfEff", oKfEff); pr(f, variants[v], s, "oNbrKfEff", oNbrKfEff);
            pr(f, variants[v], s, "dc", dc);
            pr(f, variants[v], s, "Re2", Re2); pr(f, variants[v], s, "Pr2", Pr2); pr(f, variants[v], s, "k2", k2);
        }
    }
    std::fclose(f);
    return 0;
}
