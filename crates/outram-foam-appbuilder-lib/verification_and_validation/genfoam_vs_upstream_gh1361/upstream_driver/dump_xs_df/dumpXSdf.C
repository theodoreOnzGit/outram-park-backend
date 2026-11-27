// dumpXSdf: GeN-Foam 652b3da XS-class probe for gh:#1785 / gh:#1474 (README round 39). Research, education and V&V only.
// Written for this workspace. It DERIVES from GeN-Foam's own `XS` class (libNeutronicsSolvers, GPL-3.0, EPFL) to read its
// protected members (`CRposition_`, `discFactorList_`); no upstream code is copied.
//
// For each time t_i of `-times "t1 t2 ..."` (time index i = 1, 2, ...: `runTime.setTime(t_i, i)`, so `correct()` takes
// the `timeIndex - startTimeIndex > 0` branch exactly as inside a solver run):
//   1. xs.correct()                       (driveline position + zone remap + the per-cell discFactor from discFactorList_)
//   2. dump into <case>/xsdump/<i>/: CRposition (one value per rod), D<g>, nuSigmaEff<g>, sigmaPow<g>,
//      sigmaRemoval<g>, chiPrompt<g>, chiDelayed<g>, IV<g>, discFactor<g>, sigmaFromTo_<j>_<i>, BetaTot (per cell, %.17g)
//   3. xs.adjustDiscFactors(fluxStar_i)   with the synthetic fluxStar of group g, cell c, step i:
//          1 + double((7*c + 13*g + 17*i) % 101) / 101        (integer arithmetic, then one division)
//      and dump discFactorList_ ([zone][group], %.17g) as dfList
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "volFields.H"
#include "OSspecific.H"
#include "XS.H"
#include <cstdio>
#include <sstream>

using namespace Foam;

class XSProbe : public XS
{
public:
    XSProbe(const fvMesh& mesh) : XS(mesh) {}
    scalar crPosition(label r) const { return CRposition_[r]; }
    label crCount() const { return CRposition_.size(); }
    const PtrList<scalarField>& dfList() const { return discFactorList_; }
};

static void dumpS(const fileName& d, const word& n, const scalarField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g\n", f[i]);
    fclose(fp);
}

int main(int argc, char* argv[])
{
    argList::addOption("times", "list", "times of the probe, e.g. \"0 5 15\"");
    #include "setRootCase.H"
    #include "createTime.H"
    string timesStr;
    args.readIfPresent("times", timesStr);
    std::istringstream is(timesStr);
    std::vector<double> times;
    double tv;
    while (is >> tv) times.push_back(tv);

    fvMesh mesh
    (
        IOobject(word("neutroRegion"), runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    XSProbe xs(mesh);
    const label G = xs.energyGroups();
    const fileName root = runTime.path()/"xsdump";
    mkDir(root);

    for (size_t k = 0; k < times.size(); ++k)
    {
        const label idx = label(k) + 1;
        runTime.setTime(times[k], idx);
        xs.correct();

        const fileName d = root/name(idx);
        mkDir(d);
        scalarField crp(xs.crCount());
        forAll(crp, r) crp[r] = xs.crPosition(r);
        dumpS(d, "CRposition", crp);
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
            for (label i = 0; i < G; ++i)
                dumpS(d, "sigmaFromTo_" + name(j) + "_" + name(i), sft[0][j][i].primitiveField());
        dumpS(d, "BetaTot", xs.BetaTot().primitiveField());

        PtrList<volScalarField> fluxStar(G);
        for (label g = 0; g < G; ++g)
        {
            fluxStar.set
            (
                g,
                new volScalarField
                (
                    IOobject("fluxStar" + name(g), runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
                    mesh,
                    dimensionedScalar("", dimless, 1.0),
                    zeroGradientFvPatchScalarField::typeName
                )
            );
            scalarField& f = fluxStar[g].primitiveFieldRef();
            forAll(f, c)
            {
                const long m = (7L*c + 13L*g + 17L*idx) % 101L;
                f[c] = 1.0 + double(m)/101.0;
            }
        }
        xs.adjustDiscFactors(fluxStar);
        FILE* fp = fopen((d/"dfList").c_str(), "w");
        forAll(xs.dfList(), z) forAll(xs.dfList()[z], g) fprintf(fp, "%.17g\n", xs.dfList()[z][g]);
        fclose(fp);
    }
    Info<< "wrote " << root << endl;
    return 0;
}
