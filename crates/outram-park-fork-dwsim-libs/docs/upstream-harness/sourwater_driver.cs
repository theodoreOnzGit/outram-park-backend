// Headless driver for upstream DWSIM sour water (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::thermo::sour_water.
// Prints KEY=value lines only; everything else goes to stderr.
//
// Modes:
//   spec  <T K> <m_NH3> <m_H2S> <m_CO2> <m>
//       Calls the liquid-phase kernel FlashAlgorithms/SourWater.vb
//       CalculateEquilibriumConcentrations (:384-559) directly on total
//       molalities [mol/kg]. <m> is its first argument, which the PT flash
//       passes as totalkg = L * MMM(Vxl) / 1000 (:271, :295); it enters only
//       the carbon closure m0C = conc0(CO2) * m * 44.01 / 1000 (:407).
//   flash <T K> <P Pa> <n_H2O> <n_NH3> <n_H2S> <n_CO2> <ions:1|0>
//       Full SourWaterPropertyPackage PT flash (Flash_PT, :173-382) on the
//       given molar amounts (normalised), with the ionic species in the
//       compound list (ions=1) or without them (ions=0, the :329 branch).
using System;
using System.Linq;
using System.Globalization;
using System.Collections.Generic;
using DWSIM.Interfaces;
using DWSIM.Thermodynamics.CalculatorInterface;
using DWSIM.Thermodynamics.PropertyPackages;
using DWSIM.Thermodynamics.Streams;
using DWSIM.Thermodynamics.BaseClasses;
using SW = DWSIM.Thermodynamics.PropertyPackages.Auxiliary.FlashAlgorithms.SourWater;

class Driver {
    static string F(double v) { return v.ToString("R", CultureInfo.InvariantCulture); }
    static double D(string s) { return double.Parse(s, CultureInfo.InvariantCulture); }

    static readonly string[] Molecular = { "Water", "Ammonia", "Hydrogen sulfide", "Carbon dioxide" };
    static readonly string[] Ions = { "Hydron", "Hydroxide", "Ammonium (ion)", "Bicarbonate (ion)",
        "Carbonate (ion)", "Carbamate (ion)", "Bisulfide (ion)", "Sulfide (ion)" };

    static SourWaterPropertyPackage Package(Calculator calc, string[] comps, out MaterialStream ms) {
        var pp = new SourWaterPropertyPackage(true);
        calc.TransferCompounds(pp);
        ms = new MaterialStream("", "");
        foreach (var phase in ms.Phases.Values)
            foreach (var cn in comps) {
                phase.Compounds.Add(cn, new Compound(cn, ""));
                phase.Compounds[cn].ConstantProperties = pp._availablecomps[cn];
            }
        ms.PropertyPackage = pp;
        pp.CurrentMaterialStream = ms;
        return pp;
    }

    static void Main(string[] args) {
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;
        System.Threading.Thread.CurrentThread.CurrentCulture = CultureInfo.InvariantCulture;
        var calc = new Calculator(); calc.Initialize();

        if (args[0] == "spec") {
            double T = D(args[1]), mN = D(args[2]), mS = D(args[3]), mC = D(args[4]), m = D(args[5]);
            var comps = Molecular.Concat(Ions).ToArray();
            MaterialStream ms;
            var pp = Package(calc, comps, out ms);
            var sw = new SW();
            sw.CompoundProperties = comps.Select(c => (ICompoundConstantProperties)pp._availablecomps[c]).ToList();
            var conc = new Dictionary<string, double>();
            var conc0 = new Dictionary<string, double>();
            var id = new Dictionary<string, int>();
            sw.Setup(conc, conc0, id);
            conc0["H2O"] = 1000.0 / 18.015;
            conc0["NH3"] = mN; conc0["H2S"] = mS; conc0["CO2"] = mC;
            var t0 = DateTime.UtcNow;
            try { sw.CalculateEquilibriumConcentrations(m, T, pp, conc, conc0, id); }
            catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }
            Console.WriteLine("WALL_MS=" + F((DateTime.UtcNow - t0).TotalMilliseconds));
            int k = 0;
            foreach (var r in sw.Reactions) Console.WriteLine("K[" + (k++) + "]=" + F(r.EvaluateK(T, pp)));
            foreach (var kv in conc) Console.WriteLine("C[" + kv.Key + "]=" + F(kv.Value));
            Console.WriteLine("PH=" + F(-Math.Log10(conc["H+"])));
            Console.WriteLine("CARBON_TOTAL=" + F(conc["CO2"] + conc["HCO3-"] + conc["CO3-2"] + conc["H2NCOO-"]));
            Console.WriteLine("NITROGEN_TOTAL=" + F(conc["NH3"] + conc["NH4+"] + conc["H2NCOO-"]));
            Console.WriteLine("SULFUR_TOTAL=" + F(conc["H2S"] + conc["HS-"] + conc["S-2"]));
            return;
        }

        if (args[0] == "flash") {
            double T = D(args[1]), P = D(args[2]);
            double[] n = { D(args[3]), D(args[4]), D(args[5]), D(args[6]) };
            bool ions = args[7] == "1";
            var comps = ions ? Molecular.Concat(Ions).ToArray() : Molecular;
            MaterialStream ms;
            var pp = Package(calc, comps, out ms);
            double tot = n.Sum();
            var z = new double[comps.Length];
            for (int i = 0; i < 4; i++) z[i] = n[i] / tot;
            object[] res;
            try { res = (object[])pp.FlashBase.Flash_PT(z, P, T, pp); }
            catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }
            double L = (double)res[0], V = (double)res[1];
            var x = (double[])res[2]; var y = (double[])res[3];
            Console.WriteLine("L=" + F(L)); Console.WriteLine("V=" + F(V));
            Console.WriteLine("ITER=" + res[4]);
            for (int i = 0; i < comps.Length; i++) {
                Console.WriteLine("Z[" + comps[i] + "]=" + F(z[i]));
                Console.WriteLine("X[" + comps[i] + "]=" + F(x[i]));
                Console.WriteLine("Y[" + comps[i] + "]=" + F(y[i]));
                if (x[i] > 0) Console.WriteLine("K[" + comps[i] + "]=" + F(y[i] / x[i]));
            }
            // Henry volatilities upstream uses on that liquid (psia per mol/kg):
            // AUX_PVAPi_SW = value * conc / 0.000145038 / Vx with conc = Vx / (MMM/1000).
            double kg = pp.AUX_MMM(x) / 1000.0;
            for (int i = 1; i < 4; i++) {
                double pv = pp.AUX_PVAPi_SW(i, T, x);
                Console.WriteLine("HENRY_PSIA_PER_MOLAL[" + comps[i] + "]=" + F(pv * 0.000145038 * kg));
            }
            Console.WriteLine("LIQ_MOLALITY_NH3=" + F(x[1] / kg));
            Console.WriteLine("LIQ_MOLALITY_H2S=" + F(x[2] / kg));
            Console.WriteLine("LIQ_MOLALITY_CO2=" + F(x[3] / kg));
            return;
        }
        if (args[0] == "prflash") {
            // Sour natural gas on Peng-Robinson: Calculator.CalcEquilibrium PT flash
            // (the flash_driver.cs path), plus the compound constants and the
            // k_ij upstream holds for every pair (PengRobinson.vb:70-110).
            double T = D(args[1]), P = D(args[2]);
            string[] comps = { "Methane", "Carbon dioxide", "Hydrogen sulfide", "Water" };
            double[] z = { D(args[3]), D(args[4]), D(args[5]), D(args[6]) };
            double tot = z.Sum();
            for (int i = 0; i < 4; i++) z[i] /= tot;
            var pp = new PengRobinsonPropertyPackage(true);
            calc.TransferCompounds(pp);
            foreach (var c in comps) {
                var cp = pp._availablecomps[c];
                Console.WriteLine("CONST[" + c + "]=MW " + F(cp.Molar_Weight) + " TC " + F(cp.Critical_Temperature)
                    + " PC " + F(cp.Critical_Pressure) + " W " + F(cp.Acentric_Factor) + " TB " + F(cp.Normal_Boiling_Point)
                    + " VC " + F(cp.Critical_Volume) + " DB " + cp.OriginalDB);
            }
            var ip = pp.m_pr.InteractionParameters;
            for (int i = 0; i < 4; i++)
                for (int j = i + 1; j < 4; j++) {
                    double k = 0.0; string src = "absent";
                    if (ip.ContainsKey(comps[i]) && ip[comps[i]].ContainsKey(comps[j])) { k = ip[comps[i]][comps[j]].kij; src = "ij"; }
                    else if (ip.ContainsKey(comps[j]) && ip[comps[j]].ContainsKey(comps[i])) { k = ip[comps[j]][comps[i]].kij; src = "ji"; }
                    Console.WriteLine("KIJ[" + comps[i] + "|" + comps[j] + "]=" + F(k) + " " + src);
                }
            var r = calc.CalcEquilibrium(Calculator.FlashCalculationType.PressureTemperature, 0, P, T,
                new PengRobinsonPropertyPackage(true), comps, z, null, 0.0);
            Console.WriteLine("BETA_V=" + F(r.GetVaporPhaseMoleFraction()));
            Console.WriteLine("BETA_L1=" + F(r.GetLiquidPhase1MoleFraction()));
            Console.WriteLine("BETA_L2=" + F(r.GetLiquidPhase2MoleFraction()));
            var y = r.GetVaporPhaseMoleFractions(); var x = r.GetLiquidPhase1MoleFractions();
            for (int i = 0; i < 4; i++) {
                Console.WriteLine("Y[" + comps[i] + "]=" + F(y[i]));
                Console.WriteLine("X[" + comps[i] + "]=" + F(x[i]));
                if (x[i] > 0) Console.WriteLine("K[" + comps[i] + "]=" + F(y[i] / x[i]));
            }
            return;
        }
        if (args[0] == "prphi") {
            // Upstream PR fugacity coefficients at a GIVEN composition (no flash):
            // prphi <T> <P> <liquid|vapor> z_CH4 z_CO2 z_H2S z_H2O
            double T = D(args[1]), P = D(args[2]);
            string[] comps = { "Methane", "Carbon dioxide", "Hydrogen sulfide", "Water" };
            double[] z = { D(args[4]), D(args[5]), D(args[6]), D(args[7]) };
            var pp = new PengRobinsonPropertyPackage(true);
            calc.TransferCompounds(pp);
            var ms = new MaterialStream("", "");
            foreach (var phase in ms.Phases.Values)
                foreach (var cn in comps) {
                    phase.Compounds.Add(cn, new Compound(cn, ""));
                    phase.Compounds[cn].ConstantProperties = pp._availablecomps[cn];
                }
            ms.PropertyPackage = pp; pp.CurrentMaterialStream = ms;
            var st = args[3] == "liquid" ? State.Liquid : State.Vapor;
            var vk = pp.RET_VKij(); var vtc = pp.RET_VTC(); var vpc = pp.RET_VPC(); var vw = pp.RET_VW();
            var vn = pp.RET_VNAMES();
            for (int i = 0; i < 4; i++) {
                Console.WriteLine("ARG[" + vn[i] + "]=TC " + F(vtc[i]) + " PC " + F(vpc[i]) + " W " + F(vw[i]));
                for (int j = 0; j < 4; j++) Console.WriteLine("VKIJ[" + i + "," + j + "]=" + F(vk[i, j]));
            }
            {
                // Recompute upstream's A, B exactly as CalcLnFugCPU does
                // (PengRobinson.vb:1213-1241) and print its liquid/vapour Z roots
                // with the cubic residual, to separate the root from the formula.
                double R = 8.314, aml = 0, bml = 0;
                var ai = new double[4]; var bi = new double[4];
                for (int i = 0; i < 4; i++) {
                    double al = Math.Pow(1 + (0.37464 + 1.54226 * vw[i] - 0.26992 * vw[i] * vw[i]) * (1 - Math.Sqrt(T / vtc[i])), 2);
                    ai[i] = 0.45724 * al * Math.Pow(R * vtc[i], 2) / vpc[i];
                    bi[i] = 0.0778 * R * vtc[i] / vpc[i];
                    bml += z[i] * bi[i];
                }
                for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++)
                    aml += z[i] * z[j] * Math.Sqrt(ai[i] * ai[j]) * (1 - vk[i, j]);
                double AG = aml * P / Math.Pow(R * T, 2), BG = bml * P / (R * T);
                var zs = DWSIM.Thermodynamics.PropertyPackages.ThermoPlugs.PR.CalcZ2(AG, BG);
                Console.WriteLine("AG=" + F(AG)); Console.WriteLine("BG=" + F(BG));
                foreach (var zz in zs) {
                    double res = zz * zz * zz + (BG - 1) * zz * zz + (AG - 3 * BG * BG - 2 * BG) * zz + (-AG * BG + BG * BG + BG * BG * BG);
                    Console.WriteLine("ZROOT=" + F(zz) + " RESID " + F(res));
                }
            }
            var phi = pp.DW_CalcFugCoeff(z, T, P, st);
            for (int i = 0; i < 4; i++) Console.WriteLine("LNPHI[" + comps[i] + "]=" + F(Math.Log(phi[i])));
            return;
        }
        throw new Exception("unknown mode " + args[0]);
    }
}
