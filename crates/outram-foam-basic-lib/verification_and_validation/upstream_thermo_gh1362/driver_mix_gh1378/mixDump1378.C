// gh:#1378 family 3: multicomponent mixture rules on the UNMODIFIED
// OpenFOAM-v2506 (ESI) specie classes. multiComponentMixture needs a mesh, so
// the three-line loops of multiComponentMixture::cellMixture and
// cellVolMixture and the body of correctMassFractions are copied verbatim from
// multiComponentMixture.C (v2506); the operators they call (operator*,
// operator+=, in every layer of the species stack) are the real ones.
// Output per row: Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah W (hex).
#include "specie.H"
#include "perfectGas.H"
#include "rhoConst.H"
#include "hConstThermo.H"
#include "sensibleEnthalpy.H"
#include "thermo.H"
#include "constTransport.H"
#include "sutherlandTransport.H"
#include "IStringStream.H"
#include "dictionary.H"
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <string>
#include <vector>

using namespace Foam;

static const double Ts[] = {300, 600, 1200};
static const double Ps[] = {1e5, 5e6};

static void hex(const std::string& tag, int it, int ip, const double* v, int n)
{
    std::printf("%s %d %d", tag.c_str(), it, ip);
    for (int i = 0; i < n; i++)
    {
        std::uint64_t u;
        std::memcpy(&u, &v[i], 8);
        std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\n");
}

struct EosPG   { static const char* name() { return "perfectGas"; }
                 template<class S> using type = perfectGas<S>;
                 static std::string text(int) { return ""; } };
struct EosRC   { static const char* name() { return "rhoConst"; }
                 template<class S> using type = rhoConst<S>;
                 static std::string text(int i)
                 {
                     const char* rho[3] = {"800", "1000", "998"};
                     return std::string("equationOfState { rho ") + rho[i] + "; }\n";
                 } };

struct TrConst { static const char* name() { return "const"; }
                 template<class T> using type = constTransport<T>;
                 static std::string text(int i)
                 {
                     const char* mu[3] = {"1.8e-5", "1.5e-5", "1.2e-5"};
                     const char* pr[3] = {"0.7", "0.8", "0.9"};
                     return std::string("transport { mu ") + mu[i] + "; Pr " + pr[i] + "; }\n";
                 } };
struct TrSuth  { static const char* name() { return "sutherland"; }
                 template<class T> using type = sutherlandTransport<T>;
                 static std::string text(int i)
                 {
                     const char* as[3] = {"1.46e-6", "1.5e-6", "1.2e-6"};
                     const char* ts[3] = {"110", "240", "150"};
                     return std::string("transport { As ") + as[i] + "; Ts " + ts[i] + "; }\n";
                 } };

static const char* Wtxt[3] = {"28.0", "44.01", "18.015"};
static const char* Cptxt[3] = {"1040", "850", "1900"};
static const char* Hftxt[3] = {"0", "-8.94e6", "-1.34e7"};

static const double Ysets[6][3] =
{
    {0.5, 0.3, 0.2},
    {0.9, 0.05, 0.05},
    {1.0/3.0, 1.0/3.0, 1.0/3.0},
    {0.7, 0.3, 0.0},
    {0.4, 0.4, 0.4},   // un-normalised
    {0.4, 0.4, 0.4}    // same, passed through correctMassFractions
};

template<class Eos, class Tr>
static void oneCombo()
{
    typedef typename Tr::template type
    <
        species::thermo<hConstThermo<typename Eos::template type<specie>>, sensibleEnthalpy>
    > Mixture;

    std::vector<Mixture> sp;
    for (int i = 0; i < 3; i++)
    {
        const std::string text =
            std::string("specie { molWeight ") + Wtxt[i] + "; }\n"
          + Eos::text(i)
          + "thermodynamics { Cp " + Cptxt[i] + "; Hf " + Hftxt[i] + "; }\n"
          + Tr::text(i);
        IStringStream is(text);
        dictionary dict(is);
        sp.push_back(Mixture(dict));
    }

    for (int iy = 0; iy < 6; iy++)
    {
        double Y[3] = {Ysets[iy][0], Ysets[iy][1], Ysets[iy][2]};
        if (iy == 5)
        {
            // correctMassFractions body, one cell
            scalar Yt = 1.0*Y[0];
            for (int n = 1; n < 3; n++) { Yt += Y[n]; }
            for (int n = 0; n < 3; n++) { Y[n] /= Yt; }
        }

        for (int mode = 0; mode < 2; mode++)
        {
            const std::string tag = std::string(Eos::name()) + "/" + Tr::name()
              + (mode == 0 ? "/cell/" : "/vol/") + std::to_string(iy);
            for (int it = 0; it < 3; it++)
            {
                for (int ip = 0; ip < 2; ip++)
                {
                    const scalar T = Ts[it];
                    const scalar p = Ps[ip];
                    Mixture mixture(sp[0]);
                    if (mode == 0)
                    {
                        // multiComponentMixture::cellMixture
                        mixture = Y[0]*sp[0];
                        for (label n = 1; n < 3; n++)
                        {
                            mixture += Y[n]*sp[n];
                        }
                    }
                    else
                    {
                        // multiComponentMixture::cellVolMixture
                        scalar rhoInv = 0.0;
                        for (label i = 0; i < 3; i++)
                        {
                            rhoInv += Y[i]/sp[i].rho(p, T);
                        }
                        mixture = Y[0]/sp[0].rho(p, T)/rhoInv*sp[0];
                        for (label n = 1; n < 3; n++)
                        {
                            mixture += Y[n]/sp[n].rho(p, T)/rhoInv*sp[n];
                        }
                    }
                    const Mixture& m = mixture;
                    double v[13] =
                    {
                        m.Cp(p, T), m.Ha(p, T), m.Hs(p, T), m.Hc(), m.S(p, T),
                        m.rho(p, T), m.psi(p, T), m.Z(p, T), m.CpMCv(p, T),
                        m.mu(p, T), m.kappa(p, T), m.alphah(p, T), m.W()
                    };
                    hex(tag, it, ip, v, 13);
                }
            }
        }
    }
}

int main()
{
    std::printf("# mixDump1378 gh:#1378 family 3, ESI v2506; tag = eos/transport/cell|vol/Yset; rows T(3) x p(2); Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah W\n");
    oneCombo<EosPG, TrConst>();
    oneCombo<EosPG, TrSuth>();
    oneCombo<EosRC, TrConst>();
    oneCombo<EosRC, TrSuth>();
    return 0;
}
