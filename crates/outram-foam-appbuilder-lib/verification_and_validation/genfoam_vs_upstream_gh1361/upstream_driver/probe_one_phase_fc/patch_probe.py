import re
p = '/home/user/upstream-build/probe1894/classes/thermalHydraulics/solvers/onePhase/include/equations/pEqnf_1p.H'
s = open(p).read()

s = '#include "../../../probe1894.H"\n' + s

s = s.replace('''while (pimple_.correct(convergedPISO))
{
''', '''while (pimple_.correct(convergedPISO))
{
    probe1894::active() = (mesh_.time().timeIndex() == 1);
    if (probe1894::active()) probe1894::corr()++;
    probe1894::dumpScalar("alpha", alpha.primitiveField());
    probe1894::dumpScalar("rho", rho.primitiveField());
    probe1894::dumpScalar("p_in", p_.primitiveField());
    probe1894::dumpScalar("T", fluid_.thermo().T().primitiveField());
    probe1894::dumpScalar("contErr", fluid_.contErr().primitiveField());
    probe1894::dumpScalar("magU", fluid_.magU().primitiveField());
    probe1894::dumpVector("U", U.primitiveField());
    probe1894::dumpVolVecB("B_U", U);
    probe1894::dumpScalar("UA", UEqn.A()().primitiveField());
    probe1894::dumpVector("UH", UEqn.H()().primitiveField());
    probe1894::dumpScalar("Udiag", UEqn.diag());
    probe1894::dumpVector("Usource", UEqn.source());
    probe1894::dumpPatchVec("Uint", UEqn.internalCoeffs());
    probe1894::dumpPatchVec("Ubnd", UEqn.boundaryCoeffs());
    probe1894::dumpFace("alphaf", alphaf);
    probe1894::dumpFace("alphaRhofByDt", alphaRhofByDt);
    probe1894::dumpFace("alphaRhof0ByDt", alphaRhof0ByDt);
    probe1894::dumpFace("rAUf1", rAUf);
    probe1894::dumpFace("alpharAUf", alpharAUf);
    probe1894::dumpFace("phi_in", fluid_.phi());
    probe1894::dumpFace("phi_old", fluid_.phi().oldTime());
    probe1894::dumpFace("alphaRhoPhi", fluid_.alphaRhoPhi());
    probe1894::dumpFaceB("B_phi_in", fluid_.phi());
    probe1894::dumpFaceB("B_alphaRhoPhi", fluid_.alphaRhoPhi());
    probe1894::dumpFaceB("B_alphaf", alphaf);
''', 1)

s = s.replace('''    #include "calcrAUfSetSnGradCalcComp_1p.H"
''', '''    #include "calcrAUfSetSnGradCalcComp_1p.H"

    probe1894::dumpScalar("p_rgh_pre", p_rgh_.primitiveField());
    probe1894::dumpScalar("gh", gh_.primitiveField());
    probe1894::dumpFace("phiSbyA", phiSbyA);
    probe1894::dumpFace("phiHbyA", phiHbyA);
    probe1894::dumpFace("rAUf2", rAUf);
    probe1894::dumpFaceB("B_phiHbyA", phiHbyA);
    probe1894::dumpFaceB("B_phiSbyA", phiSbyA);
    probe1894::dumpFaceB("B_rAUf2", rAUf);
    probe1894::dumpScalar("pEqnComp_src", pEqnComp().source());
    probe1894::dumpScalar("pEqnComp_diag", pEqnComp().diag());
''', 1)

s = s.replace('''            phi_ = phiHbyA + pEqnIncomp.flux();
''', '''            phi_ = phiHbyA + pEqnIncomp.flux();
            probe1894::dumpScalar("pdiag", pEqn.diag());
            probe1894::dumpScalar("pupper", pEqn.upper());
            probe1894::dumpScalar("psource", pEqn.source());
            probe1894::dumpPatchScalar("pint", pEqn.internalCoeffs());
            probe1894::dumpPatchScalar("pbnd", pEqn.boundaryCoeffs());
            probe1894::dumpScalar("p_rgh_post", p_rgh_.primitiveField());
            probe1894::dumpVolB("B_p_rgh_post", p_rgh_);
            probe1894::dumpFace("phi_post", phi_);
            probe1894::dumpFaceB("B_phi_post", phi_);
''', 1)

s = s.replace('''    #include "updatePP_rghRho_1p.H"
''', '''    #include "updatePP_rghRho_1p.H"
    probe1894::dumpScalar("p_out", p_.primitiveField());
    probe1894::dumpScalar("p_rgh_out", p_rgh_.primitiveField());
    probe1894::dumpScalar("rho_out", rho.primitiveField());
''', 1)
open(p, 'w').write(s)
print(s.count('probe1894'))
