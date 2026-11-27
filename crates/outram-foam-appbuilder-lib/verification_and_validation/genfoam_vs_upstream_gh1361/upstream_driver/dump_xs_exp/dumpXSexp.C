// dumpXSexp: GeN-Foam 652b3da XS-class probe for gh:#1898 (README round 47). Research, education and V&V only.
// Written for this workspace. It DERIVES from GeN-Foam's own `XS` class (libNeutronicsSolvers, GPL-3.0, EPFL) to read its
// protected members (`disp_`, `xsVariableNames_`, `xsVariableFields_`); no upstream code is copied.
//
// Reads the case (mesh region neutroRegion, `<time>/neutroRegion/disp` if present), calls xs.correct() once at the time
// index startTimeIndex + 1 and dumps into <case>/xsexp/ (one value per line, %.17g):
//   dispX dispY dispZ        the displacement as XS read it (the inputs of the port)
//   <variable>               each xsVariables field (axExp, radExp) as computed by setNeutronicsVariables.H
//   D<g> nuSigmaEff<g> sigmaRemoval<g> discFactor<g> sigmaFromTo_<j>_<i>   per cell
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "volFields.H"
#include "OSspecific.H"
#include "XS.H"
#include <cstdio>

using namespace Foam;

class XSProbe : public XS
{
public:
    XSProbe(const fvMesh& mesh) : XS(mesh) {}
    const volVectorField& dispField() const { return disp_; }
    label nVars() const { return xsVariableNames_.size(); }
    const word& varName(label i) const { return xsVariableNames_[i]; }
    const volScalarField& varField(label i) const { return xsVariableFields_[i]; }
};

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
    XSProbe xs(mesh);
    const label G = xs.energyGroups();
    const fileName d = runTime.path()/"xsexp";
    mkDir(d);

    runTime.setTime(runTime.value(), runTime.startTimeIndex() + 1);
    xs.correct();

    const vectorField& u = xs.dispField().primitiveField();
    scalarField ux(u.size()), uy(u.size()), uz(u.size());
    forAll(u, c) { ux[c] = u[c].x(); uy[c] = u[c].y(); uz[c] = u[c].z(); }
    dumpS(d, "dispX", ux);
    dumpS(d, "dispY", uy);
    dumpS(d, "dispZ", uz);
    for (label i = 0; i < xs.nVars(); ++i)
    {
        dumpS(d, xs.varName(i), xs.varField(i).primitiveField());
    }
    for (label g = 0; g < G; ++g)
    {
        dumpS(d, "D" + name(g), xs.D()[g].primitiveField());
        dumpS(d, "nuSigmaEff" + name(g), xs.nuSigmaEff()[g].primitiveField());
        dumpS(d, "sigmaRemoval" + name(g), xs.sigmaRemoval()[g].primitiveField());
        dumpS(d, "discFactor" + name(g), xs.discFactor()[g].primitiveField());
    }
    const List<List<PtrList<volScalarField>>> sft = xs.sigmaFromTo();
    for (label j = 0; j < G; ++j)
        for (label i = 0; i < G; ++i)
            dumpS(d, "sigmaFromTo_" + name(j) + "_" + name(i), sft[0][j][i].primitiveField());
    Info<< "wrote " << d << endl;
    return 0;
}
