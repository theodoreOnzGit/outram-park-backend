using System;
using DWSIM.Thermodynamics.CalculatorInterface;

class VV {
    static void Main() {
        var c = new Calculator();
        c.Initialize();
        Console.WriteLine("prop,compound,T_K,P_Pa,dwsim_value");
        Run(c, "Carbon dioxide", 400.0, 5e6);
        Run(c, "Carbon dioxide", 400.0, 10e6);
        Run(c, "Nitrogen",       300.0, 10e6);
        Run(c, "Nitrogen",       200.0, 5e6);
    }
    static void Run(Calculator c, string comp, double T, double P) {
        string[] comps = { comp };
        double[] z = { 1.0 };
        try {
            var r = c.CalcProp("Peng-Robinson (PR)", "density", "Mass", "Vapor", comps, T, P, z);
            Console.WriteLine($"density,{comp},{T},{P},{Convert.ToDouble(r[0]):R}");
        } catch (Exception ex) {
            Console.WriteLine($"density,{comp},{T},{P},ERROR:{ex.Message}");
        }
        try {
            var r = c.CalcProp("Peng-Robinson (PR)", "compressibilityfactor", "Mole", "Vapor", comps, T, P, z);
            Console.WriteLine($"Z,{comp},{T},{P},{Convert.ToDouble(r[0]):R}");
        } catch (Exception ex) {
            Console.WriteLine($"Z,{comp},{T},{P},ERROR:{ex.Message}");
        }
    }
}
