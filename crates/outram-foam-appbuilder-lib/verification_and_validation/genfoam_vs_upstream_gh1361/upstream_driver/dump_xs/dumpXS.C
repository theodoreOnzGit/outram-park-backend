// dumpXS: GeN-Foam 652b3da cross-section probe for gh:#1361 round 6. Research, education and V&V only.
// Written for this workspace. It CALLS GeN-Foam's own `XS` class (libNeutronicsSolvers, GPL-3.0, EPFL);
// no upstream code is copied. Builds the `XS` of region `neutroRegion` at the time given by -time (the
// `TFuel` and `rhoCool` fields are read from that time directory), calls `xs.correct()` exactly as
// `solveNeutronics.H` does, and writes into <case>/xsdump/ one %.17g value per cell per line:
// D<g>, nuSigmaEff<g>, sigmaPow<g>, sigmaRemoval<g>, chiPrompt<g>, chiDelayed<g>, IV<g>,
// sigmaFromTo_<j>_<i> (P0), BetaTot, discFactor<g>.
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "volFields.H"
#include "OSspecific.H"
#include "XS.H"
#include <cstdio>

using namespace Foam;

static void dumpS(const fileName& d, const word& n, const scalarField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g\n", f[i]);
    fclose(fp);
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    fvMesh mesh
    (
        IOobject(word("neutroRegion"), runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    XS xs(mesh);
    xs.correct();
    const fileName d = runTime.path()/"xsdump";
    mkDir(d);
    const label G = xs.energyGroups();
    for (label g = 0; g < G; ++g)
    {
        dumpS(d, "D" + name(g), xs.D()[g].primitiveField());
        dumpS(d, "nuSigmaEff" + name(g), xs.nuSigmaEff()[g].primitiveField());
        dumpS(d, "sigmaPow" + name(g), xs.sigmaPow()[g].primitiveField());
        dumpS(d, "sigmaRemoval" + name(g), xs.sigmaRemoval()[g].primitiveField());
        dumpS(d, "chiPrompt" + name(g), xs.chiPrompt()[g].primitiveField());
        dumpS(d, "chiDelayed" + name(g), xs.chiDelayed()[g].primitiveField());
        dumpS(d, "IV" + name(g), xs.IV()[g].primitiveField());
        dumpS(d, "discFactor" + name(g), xs.discFactor()[g].primitiveField());
    }
    const List<List<PtrList<volScalarField>>> sft = xs.sigmaFromTo();
    for (label j = 0; j < G; ++j)
    {
        for (label i = 0; i < G; ++i)
        {
            dumpS(d, "sigmaFromTo_" + name(j) + "_" + name(i), sft[0][j][i].primitiveField());
        }
    }
    dumpS(d, "BetaTot", xs.BetaTot().primitiveField());
    Info<< "wrote " << d << endl;
    return 0;
}
