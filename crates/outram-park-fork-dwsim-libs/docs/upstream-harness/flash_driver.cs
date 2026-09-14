using System;
using DWSIM.Thermodynamics.CalculatorInterface;
using DWSIM.Thermodynamics.PropertyPackages;


class F {
    static void Main() {
        var c = new Calculator(); c.Initialize();
        // Binary methane/ethane PT flash, PR — a two-phase region case.
        string[] comps = { "Methane", "Ethane" };
        double[] z = { 0.5, 0.5 };
        foreach (var tp in new[]{
            new double[]{200.0, 2e6}, new double[]{220.0, 3e6},
            new double[]{180.0, 1e6}, new double[]{250.0, 5e6}}) {
            double T = tp[0], P = tp[1];
            try {
                var pp = new PengRobinsonPropertyPackage(true);
                var r = c.CalcEquilibrium(Calculator.FlashCalculationType.PressureTemperature, 0,
                                          P, T, pp, comps, z, null, 0.0);
                Console.WriteLine($"T={T,5} P={P/1e6,4} MPa  beta_V={r.GetVaporPhaseMoleFraction():F8}");
                var y = r.GetVaporPhaseMoleFractions(); var x = r.GetLiquidPhase1MoleFractions();
                Console.WriteLine($"    y = [{y[0]:F8}, {y[1]:F8}]   x = [{x[0]:F8}, {x[1]:F8}]");
            } catch (Exception e) {
                Console.WriteLine($"T={T} P={P}  ERR {e.GetType().Name}: {e.Message}");
            }
        }
    }
}
