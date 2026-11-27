// pkRun: gh:#1864 (GeN-Foam port, round 36). Subclasses GeN-Foam 652b3da's
// solvers::pointKineticNeutronics (its members are protected) and drives the REAL
// correctPhysics() step by step on a real fvMesh and Time, with per-cell feedback fields
// set from integer formulas (so the port reproduces them exactly), printing everything at
// 17 significant digits. Before each step (from the second) it also runs upstream's own
// computeFeedbackFieldValues.H + correctReactivity.H verbatim to print the weighted
// averages and every reactivity term (those are locals inside the step).
// Original OUTRAM PARK verification code; links, does not modify, upstream.
#include "fvCFD.H"
#include "pointKineticNeutronics.H"
#include <cstdio>
#include <cmath>
#include <cstdlib>

namespace Foam {

static double frac(long i, long k, long a, long b, long m)
{
    return double((a*i + b*k) % m)/double(m);
}

class probe : public solvers::pointKineticNeutronics
{
public:
    probe(fvMesh& m) : solvers::pointKineticNeutronics(m) {}

    static scalar sumField(const volScalarField& f)
    {
        scalar s = 0;
        forAll(f, i) s += f[i];
        return s;
    }

    void printConfig() const
    {
        printf("CFG pgt %.17g\n", promptGenerationTime_);
        printf("CFG nbeta %d\n", int(betas_.size()));
        forAll(betas_, i) printf("CFG beta %d %.17g\nCFG lambda %d %.17g\n", int(i), betas_[i], int(i), lambdas_[i]);
        printf("CFG coeff %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g\n",
               coeffDoppler_, coeffTFuel_, coeffTClad_, coeffTCool_, coeffRhoCool_,
               coeffTStruct_, coeffTStructMech_, coeffDrivelineExp_);
        printf("CFG fast %d\n", int(fastNeutrons_));
        printf("CFG liquid %d extsrc %d\n", int(liquidFuel_), int(externalSourceNeutronics_));
        printf("CFG pTarget %.17g powerTarget %.17g\n", pTarget_, powerTarget_);
        printf("CFG subcritical %.17g ksrc %.17g\n", subcriticalIndex_, ksrc_);
        printf("CFG found ksrc %d subcriticalIndex %d precursorPowers %d\n",
               int(reactorState_.found("ksrc")), int(reactorState_.found("subcriticalIndex")),
               int(reactorState_.found("precursorPowers")));
        printf("CFG nuFission %.17g energyPerFission %.17g nuSource %.17g beamEnergy %.17g\n",
               nuFission_, energyPerFission_, externalSourceNeutronics_ ? nuSource_ : 0.0,
               externalSourceNeutronics_ ? beamEnergy_ : 0.0);
        printf("CFG gain %.17g srcRef %.17g srcPower %.17g srcBeamPower %.17g srcBeamI %.17g\n",
               intrinsicGain_, externalSourcePowerRef_, externalSourcePower_,
               externalSourceBeamPower_, externalSourceBeamIntensity_);
        printf("CFG modFactor %.17g modDelay %.17g mode %s\n", modulationFactor_,
               powerModulationDelay_, externalSourceMode_.empty() ? "-" : externalSourceMode_.c_str());
        printf("CFG power0 %.17g fission0 %.17g decay0 %.17g\n", power_, fissionPower_, decayPower_);
        printf("CFG prec0");
        forAll(precursorPowers_, i) printf(" %.17g", precursorPowers_[i]);
        printf("\n");
        printf("CFG nrods %d\n", int(controlRodReactivityMap_.size()));
        forAll(controlRodReactivityMap_, i)
            printf("CFG rod %d %.17g %.17g\n", int(i), controlRodReactivityMap_[i].first(),
                   controlRodReactivityMap_[i].second());
        printf("CFG ngem %d\n", int(GEMReactivityMap_.size()));
        forAll(GEMReactivityMap_, i)
            printf("CFG gemmap %d %.17g %.17g\n", int(i), GEMReactivityMap_[i].first(),
                   GEMReactivityMap_[i].second());
        printf("CFG energyGroups %d beta %.17g\n", int(energyGroups_), beta_);
        printf("CFG sumFlux0 %.17g sumFluxGroup0 %.17g\n", sumField(oneGroupFlux_),
               energyGroups_ > 0 ? sumField(fluxes_[0]) : 0.0);
        printf("CFG sumFuelCells %.17g sumCoolCells %.17g sumStructCells %.17g sumStructMechCells %.17g sumDriveCells %.17g\n",
               sumField(fuelFeedbackCellField_), sumField(coolFeedbackCellField_),
               sumField(structFeedbackCellField_), sumField(structMechFeedbackCellField_),
               sumField(drivelineFeedbackCellField_));
        printf("CFG decayValid %d extReactValid %d boronValid %d extModValid %d\n",
               int(decayPowerTimeProfile_.valid()), int(externalReactivityTimeProfile_.valid()),
               int(boronReactivityTimeProfile_.valid()), int(externalSourceModulationTimeProfile_.valid()));
        printf("CFG nCells %d\n", int(mesh_.nCells()));
        fflush(stdout);
    }

    // Profile values at a time (the profile objects are not const-callable)
    void printProfiles(scalar t)
    {
        printf("PROF %.17g %.17g %.17g %.17g %.17g\n", t,
               decayPowerTimeProfile_.valid() ? decayPowerTimeProfile_.value(t) : -1.0,
               externalReactivityTimeProfile_.value(t),
               boronReactivityTimeProfile_.value(t),
               externalSourceModulationTimeProfile_.valid() ? externalSourceModulationTimeProfile_.value(t) : -1.0);
    }

    void setPD()
    {
        forAll(powerDensity_, i)
        {
            powerDensity_[i] = 1.0e6 + 3.0e5*frac(i, 1, 17, 1, 61);
            secondaryPowerDensity_[i] = 2.0e5 + 1.0e5*frac(i, 1, 19, 1, 59);
        }
    }

    void setFields(long k)
    {
        forAll(TFuel_, i)
        {
            TFuel_[i] = 900.0 + 150.0*frac(i, k, 3, 5, 97);
            TClad_[i] = 850.0 + 60.0*frac(i, k, 5, 3, 89);
            TCool_[i] = 800.0 + 50.0*frac(i, k, 7, 2, 83);
            rhoCool_[i] = 3300.0 - 80.0*frac(i, k, 2, 9, 79);
            TStruct_[i] = 780.0 + 30.0*frac(i, k, 11, 4, 73);
            TStructMech_[i] = 700.0 + 20.0*frac(i, k, 13, 6, 71);
        }
    }

    // Upstream's computeFeedbackFieldValues.H and correctReactivity.H, verbatim, on the
    // current fields (idempotent apart from totalReactivity_, which is recomputed equal).
    void terms(long idx)
    {
        #include "computeFeedbackFieldValues.H"
        #include "correctReactivity.H"
        printf("T %ld %.17g %.17g %.17g %.17g %.17g %.17g %.17g | %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g | %.17g %.17g\n",
               idx, TFuelValue, TCladValue, TCoolValue, rhoCoolValue, TStructValue,
               TStructMechValue, TDrivelineValue,
               DopplerReactivity, TFuelReactivity, TCladReactivity, TCoolReactivity,
               rhoCoolReactivity, TStructReactivity, TStructMechReactivity,
               drivelineExpValue, drivelineReactivity, GEMReactivity, externalReactivity,
               boronReactivity, totalReactivity_, GEMSodiumLevel);
    }

    void stepAndPrint(long idx, scalar t, scalar dt)
    {
        const scalar pOld = power_;
        const scalar fOld = fissionPower_;
        correctPhysics();
        printf("K %ld %.17g %.17g | %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g | %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g",
               idx, t, dt, totalReactivity_, fissionPower_, decayPower_, power_,
               externalSourcePower_, externalSourceModulation_, externalSourceBeamPower_,
               externalSourceBeamIntensity_, maxDeltaT(), sumField(oneGroupFlux_),
               energyGroups_ > 0 ? sumField(fluxes_[energyGroups_-1]) : 0.0,
               sumField(powerDensity_), sumField(secondaryPowerDensity_), pOld,
               TFuelRef_, TCladRef_, TCoolRef_, rhoCoolRef_, TStructRef_, TStructMechRef_,
               TDrivelineRef_, fOld);
        printf(" |");
        forAll(precursorPowers_, i) printf(" %.17g", precursorPowers_[i]);
        printf("\n");
        printf("F %ld %.17g %.17g %.17g\n", idx, oneGroupFlux_[0],
               oneGroupFlux_[mesh_.nCells()/2], oneGroupFlux_[mesh_.nCells()-1]);
        fflush(stdout);
    }
};

} // namespace Foam

using namespace Foam;

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    int nSteps = 120;
    if (const char* e = getenv("PK_STEPS")) nSteps = atoi(e);
    fvMesh mesh
    (
        IOobject("neutroRegion", runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    probe pk(mesh);
    pk.printConfig();
    pk.setPD();
    const double dts[] = {1e-3, 2e-3, 5e-3, 1e-2, 2e-2, 5e-2, 1e-1, 2e-1};
    for (int k = 0; k < nSteps; ++k)
    {
        const double dt = dts[k % 8];
        runTime.setDeltaT(dt);
        runTime++;
        const scalar t = runTime.timeOutputValue();
        pk.setFields(k);
        pk.printProfiles(t);
        if (k > 0) pk.terms(k);
        pk.stepAndPrint(k, t, dt);
    }
    return 0;
}
