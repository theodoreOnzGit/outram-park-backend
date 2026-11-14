// gh:#1378 family 2: reaction-rate classes of the UNMODIFIED OpenFOAM-v2506
// (ESI) specie layer. One line per (tag, T index, c index): the hex bit
// pattern of rate(p = 1e5, T, c).
#include "specie.H"
#include "speciesTable.H"
#include "ArrheniusReactionRate.H"
#include "thirdBodyArrheniusReactionRate.H"
#include "LandauTellerReactionRate.H"
#include "powerSeriesReactionRate.H"
#include "JanevReactionRate.H"
#include "LangmuirHinshelwoodReactionRate.H"
#include "infiniteReactionRate.H"
#include "thirdBodyEfficiencies.H"
#include "LindemannFallOffFunction.H"
#include "TroeFallOffFunction.H"
#include "SRIFallOffFunction.H"
#include "FallOffReactionRate.H"
#include "ChemicallyActivatedReactionRate.H"
#include "speciesTable.H"
#include "IStringStream.H"
#include "dictionary.H"
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <string>

using namespace Foam;

static const double Ts[] = {250, 300, 500, 800, 1000, 1500, 2000, 3000};
static const double Cs[4][3] =
{
    {1e-2, 2e-2, 5e-3},
    {1e-4, 3e-4, 2e-4},
    {1.0, 0.5, 0.25},
    {1e-8, 1e-8, 1e-8}
};

template<class Rate>
static void dump(const char* tag, const Rate& r)
{
    for (int it = 0; it < 8; it++)
    {
        for (int ic = 0; ic < 4; ic++)
        {
            scalarField c(3);
            for (int k = 0; k < 3; k++) { c[k] = Cs[ic][k]; }
            const double v = r(1e5, Ts[it], c);
            std::uint64_t u;
            std::memcpy(&u, &v, 8);
            std::printf("%s %d %d %016llx\n", tag, it, ic, (unsigned long long)u);
        }
    }
}

int main()
{
    speciesTable species(wordList{"CO", "C3H6", "NO"});
    const thirdBodyEfficiencies tbes(species, scalarList{1.0, 2.5, 0.4});

    std::printf("# rateDump1378 gh:#1378 family 2, ESI v2506; tag iT ic bits; T = 250 300 500 800 1000 1500 2000 3000 K\n");

    dump("Arrhenius/full", ArrheniusReactionRate(3.547e15, -0.406, 8353.0));
    dump("Arrhenius/noBeta", ArrheniusReactionRate(1.0e10, 0.0, 5000.0));
    dump("Arrhenius/noTa", ArrheniusReactionRate(1.475e12, 0.6, 0.0));
    dump("Arrhenius/plain", ArrheniusReactionRate(2.0, 0.0, 0.0));
    dump("thirdBodyArrhenius", thirdBodyArrheniusReactionRate(2.65e16, -0.67, 17041.0, tbes));
    dump("LandauTeller/full", LandauTellerReactionRate(1.0e9, 0.3, 1200.0, 35.0, -80.0));
    dump("LandauTeller/noTa", LandauTellerReactionRate(1.0e9, 0.0, 0.0, 35.0, 0.0));
    dump("LandauTeller/plain", LandauTellerReactionRate(5.0, 0.0, 0.0, 0.0, 0.0));
    dump("powerSeries", powerSeriesReactionRate(1.0e3, 0.5, 99.0, FixedList<scalar, 4>({-9000.0, 2.0e6, -3.0e8, 1.0e10})));
    dump("Janev", JanevReactionRate(1.0e-8, 0.4, 3000.0, FixedList<scalar, 9>({-1.0, 0.8, -0.05, 0.003, -1.0e-4, 5.0e-6, -2.0e-7, 1.0e-8, -3.0e-10})));
    dump("infinite", infiniteReactionRate());

    {
        dictionary dict;
        IStringStream is("coeffs ((1.0e3 100.0) (2.0e1 500.0) (3.0e1 700.0) (4.0e4 900.0) (5.0e2 300.0));\n");
        dict.read(is);
        dump("LangmuirHinshelwood", LangmuirHinshelwoodReactionRate(species, dict));
    }

    const ArrheniusReactionRate k0(6.366e20, -1.72, 524.8);
    const ArrheniusReactionRate kInf(1.475e12, 0.6, 0.0);

    dump("FallOff/Lindemann", FallOffReactionRate<ArrheniusReactionRate, LindemannFallOffFunction>(k0, kInf, LindemannFallOffFunction(), tbes));
    dump("FallOff/Troe", FallOffReactionRate<ArrheniusReactionRate, TroeFallOffFunction>(k0, kInf, TroeFallOffFunction(0.5, 1e-30, 1e30, 1e100), tbes));
    dump("FallOff/Troe/finite", FallOffReactionRate<ArrheniusReactionRate, TroeFallOffFunction>(k0, kInf, TroeFallOffFunction(0.7346, 94.0, 1756.0, 5182.0), tbes));
    dump("FallOff/Troe/small", FallOffReactionRate<ArrheniusReactionRate, TroeFallOffFunction>(k0, kInf, TroeFallOffFunction(1.0, 1e-30, 1e-30, 1e100), tbes));
    dump("FallOff/SRI", FallOffReactionRate<ArrheniusReactionRate, SRIFallOffFunction>(k0, kInf, SRIFallOffFunction(0.45, 797.0, 979.0, 1.0, 0.0), tbes));
    dump("FallOff/SRI/e", FallOffReactionRate<ArrheniusReactionRate, SRIFallOffFunction>(k0, kInf, SRIFallOffFunction(0.5, 300.0, 1000.0, 1.2, 0.1), tbes));
    dump("ChemicallyActivated/Lindemann", ChemicallyActivatedReactionRate<ArrheniusReactionRate, LindemannFallOffFunction>(k0, kInf, LindemannFallOffFunction(), tbes));
    dump("ChemicallyActivated/Troe", ChemicallyActivatedReactionRate<ArrheniusReactionRate, TroeFallOffFunction>(k0, kInf, TroeFallOffFunction(0.7346, 94.0, 1756.0, 5182.0), tbes));
    dump("ChemicallyActivated/SRI", ChemicallyActivatedReactionRate<ArrheniusReactionRate, SRIFallOffFunction>(k0, kInf, SRIFallOffFunction(0.45, 797.0, 979.0, 1.0, 0.0), tbes));
    return 0;
}
