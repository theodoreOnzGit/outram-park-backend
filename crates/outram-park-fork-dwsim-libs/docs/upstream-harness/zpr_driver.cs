using System;
using DWSIM.Thermodynamics.PropertyPackages.Auxiliary;
class ZPR {
    static void Main() {
        var pr = new PengRobinson();
        Run(pr, "CO2 400K 5MPa",  400, 5e6,  304.21, 7.383e6, 0.223621);
        Run(pr, "CO2 400K 10MPa", 400, 10e6, 304.21, 7.383e6, 0.223621);
        Run(pr, "N2 300K 10MPa",  300, 10e6, 126.2,  3.398e6, 0.037);
        Run(pr, "N2 200K 5MPa",   200, 5e6,  126.2,  3.398e6, 0.037);
    }
    static void Run(PengRobinson pr, string n, double T, double P, double Tc, double Pc, double w) {
        double[] Vx = {1.0}, vtc = {Tc}, vpc = {Pc}, vw = {w};
        double[,] kij = new double[1,1];
        try {
            double z = pr.Z_PR(T, P, Vx, kij, vtc, vpc, vw, "V");
            Console.WriteLine($"{n,-18} Z_PR(direct) = {z:R}");
        } catch (Exception e) { Console.WriteLine($"{n,-18} ERR {e.Message}"); }
    }
}
