// Headless driver for upstream DWSIM Reactor_Gibbs (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::reactors::GibbsReactor.
// Prints KEY=value lines only; everything else goes to stderr.
//
// usage: mono gibbs_driver.exe <case> <method>
//   case   : smr1 (1100 K, 1 bar) | smr20 (1100 K, 20 bar)
//   method : lagrange — AlternateSolvingMethod = True -> Calculate_Lagrange
//                       (element potentials + Newton; Gibbs.vb:1743), the
//                       formulation this port's RAND minimiser implements;
//            bfgs     — default Calculate_GibbsMin (Gibbs.vb:1059) with
//                       UseIPOPTSolver = False (managed BFGS-B; no Linux IPOPT).
// Also prints, for feeding the port: DELGF[i] = AUX_DELGF_T(298.15, T, i)*MW_i,
// the dimensionless g°_i(T)/RT both upstream methods use (Gibbs.vb:1371, 1998),
// and LNPHI[i], upstream's PR vapour ln(phi_i) at the outlet composition.
using System;
using System.Linq;
using System.Globalization;
using System.Collections.Generic;
using DWSIM.Interfaces;
using DWSIM.Interfaces.Enums;
using DWSIM.Interfaces.Enums.GraphicObjects;
using DWSIM.Thermodynamics.CalculatorInterface;
using DWSIM.Thermodynamics.PropertyPackages;
using DWSIM.Thermodynamics.Streams;
using DWSIM.UnitOperations.Reactors;
using DWSIM.UnitOperations.Streams;

class HeadlessFlowsheet : DWSIM.FlowsheetBase.FlowsheetBase {
    public override void DisplayForm(object form) {}
    public override IFlowsheet GetNewInstance() { return new HeadlessFlowsheet(); }
    public override void ShowDebugInfo(string text, int level) {}
    public override void ShowMessage(string text, IFlowsheet.MessageType mtype, string exceptionID = "") {
        Console.Error.WriteLine("[dwsim msg] " + text);
    }
    public override void UpdateOpenEditForms() {}
    public override void CloseOpenEditForms() {}
    public override void RunCodeOnUIThread(Action act) { act(); }
    public override void SetMessageListener(Action<string, IFlowsheet.MessageType> act) {}
    public override void UpdateInformation() {}
    public override void UpdateInterface() {}
    public override object GetApplicationObject() { return null; }
    public override bool SupressMessages { get; set; }
}

class Driver {
    static string F(double v) { return v.ToString("R", CultureInfo.InvariantCulture); }

    static void Main(string[] args) {
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;
        System.Threading.Thread.CurrentThread.CurrentCulture = CultureInfo.InvariantCulture;
        string cs = args[0], method = args[1];

        var comps = new[] { "Methane", "Water", "Carbon monoxide", "Carbon dioxide", "Hydrogen" };
        var feed = new[] { 1.0, 3.0, 0.0, 0.0, 0.0 };
        double T = 1100.0, P;
        if (cs == "smr1") P = 1.0e5;
        else if (cs == "smr20") P = 2.0e6;
        else if (cs == "smrp") P = double.Parse(Environment.GetEnvironmentVariable("GIBBS_P"), CultureInfo.InvariantCulture);
        else throw new Exception("unknown case " + cs);

        var calc = new Calculator(); calc.Initialize();
        var pp = new PengRobinsonPropertyPackage(true);
        calc.TransferCompounds(pp);

        var fs = new HeadlessFlowsheet();
        foreach (var n in comps) fs.Options.SelectedComponents.Add(n, pp._availablecomps[n]);
        fs.AddPropertyPackage(pp);

        var ins  = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 0, 0, "IN");
        var outv = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 200, 0, "OUTV");
        var outl = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 200, 100, "OUTL");
        var es   = (EnergyStream)fs.AddObject(ObjectType.EnergyStream, 100, 100, "Q");
        var r    = (Reactor_Gibbs)fs.AddObject(ObjectType.RCT_Gibbs, 100, 0, "GIBBS");
        foreach (ISimulationObject o in new ISimulationObject[] { ins, outv, outl, r }) o.PropertyPackage = pp;
        fs.ConnectObjects(ins.GraphicObject, r.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outv.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outl.GraphicObject, 1, 0);
        fs.ConnectObjects(es.GraphicObject, r.GraphicObject, 0, 1);

        double tot = feed.Sum();
        ins.SetTemperature(T);
        ins.SetPressure(P);
        ins.SetOverallComposition(feed.Select(f => f / tot).ToArray());
        ins.SetMolarFlow(tot);
        ins.Calculate(true, true);

        r.ComponentIDs = comps.ToList();
        r.CreateElementMatrix();
        for (int i = 0; i < comps.Length; i++) {
            var cp = pp._availablecomps[comps[i]];
            Console.WriteLine("DB[" + i + "]=" + cp.OriginalDB + " FORMULA=" + cp.Formula +
                " NELEMENTS=" + (cp.Elements == null ? 0 : cp.Elements.Count));
        }
        if (r.Elements.Length == 0) {
            // The compounds loaded by Calculator.Initialize carry no <elements>
            // block, so CreateElementMatrix (Gibbs.vb:726-769) finds nothing.
            // Set the formula matrix by hand from the molecular formulas:
            // pure stoichiometry, not property data.
            Console.WriteLine("ELEMENTS_SOURCE=manual");
            r.Elements = new[] { "C", "H", "O" };
            r.ElementMatrix = new double[,] {
                { 1, 0, 1, 1, 0 },   // C: CH4 H2O CO CO2 H2
                { 4, 2, 0, 0, 2 },   // H
                { 0, 1, 1, 2, 0 } }; // O
            r.TotalElements = new double[3];
        } else {
            Console.WriteLine("ELEMENTS_SOURCE=database");
        }
        r.ReactorOperationMode = OperationMode.Isothermic;
        r.InitializeFromPreviousSolution = false;
        r.ReactivePhaseBehavior = Reactor_Gibbs.ReactivePhaseType.Vapor;
        if (method == "lagrange") { r.AlternateSolvingMethod = true; }
        else if (method == "bfgs") { r.AlternateSolvingMethod = false; r.UseIPOPTSolver = false; }
        else throw new Exception("unknown method " + method);
        var tl = Environment.GetEnvironmentVariable("GIBBS_TOL");
        if (!string.IsNullOrEmpty(tl)) r.InternalTolerance = double.Parse(tl, CultureInfo.InvariantCulture);
        var mi = Environment.GetEnvironmentVariable("GIBBS_MAXIT");
        if (!string.IsNullOrEmpty(mi)) r.MaximumInternalIterations = int.Parse(mi);

        Console.WriteLine("CASE=" + cs); Console.WriteLine("METHOD=" + method);
        Console.WriteLine("T=" + F(T)); Console.WriteLine("P=" + F(P));
        Console.WriteLine("TOL=" + F(r.InternalTolerance)); Console.WriteLine("MAXIT=" + r.MaximumInternalIterations);
        Console.WriteLine("ELEMENTS=" + string.Join(",", r.Elements));
        for (int k = 0; k < r.Elements.Length; k++)
            Console.WriteLine("EMAT[" + k + "]=" + string.Join(",", Enumerable.Range(0, comps.Length).Select(j => F(r.ElementMatrix[k, j]))));

        pp.CurrentMaterialStream = ins;
        for (int i = 0; i < comps.Length; i++) {
            var cp = pp._availablecomps[comps[i]];
            Console.WriteLine("NAME[" + i + "]=" + comps[i]);
            Console.WriteLine("DELGF[" + i + "]=" + F(pp.AUX_DELGF_T(298.15, T, comps[i], false) * cp.Molar_Weight));
            Console.WriteLine("DGF25[" + i + "]=" + F(cp.IG_Gibbs_Energy_of_Formation_25C * cp.Molar_Weight));
            Console.WriteLine("DHF25[" + i + "]=" + F(cp.IG_Enthalpy_of_Formation_25C * cp.Molar_Weight));
            Console.WriteLine("F_IN[" + i + "]=" + F(feed[i]));
        }

        var t0 = DateTime.UtcNow;
        try { r.Calculate(); }
        catch (Exception e) { Console.WriteLine("ERROR=" + e.ToString().Replace('\n', ' ')); return; }
        Console.WriteLine("WALL_MS=" + F((DateTime.UtcNow - t0).TotalMilliseconds));

        // Raw values exactly as the reactor wrote them (before any stream calc).
        Console.WriteLine("MASSFLOW_V_RAW=" + F(outv.Phases[0].Properties.massflow.GetValueOrDefault()));
        Console.WriteLine("MASSFLOW_L_RAW=" + F(outl.Phases[0].Properties.massflow.GetValueOrDefault()));
        for (int i = 0; i < comps.Length; i++)
            Console.WriteLine("Y_OUT_RAW[" + i + "]=" + F(outv.Phases[0].Compounds[comps[i]].MoleFraction.GetValueOrDefault()));
        outv.Calculate(true, true);
        outl.Calculate(true, true);
        var fout = new double[comps.Length];
        for (int i = 0; i < comps.Length; i++) {
            double fv = outv.Phases[0].Compounds[comps[i]].MolarFlow.GetValueOrDefault();
            double fl = outl.Phases[0].Compounds[comps[i]].MolarFlow.GetValueOrDefault();
            // An all-vapour outlet leaves a round-off mass flow (~1e-17 kg/s) on
            // the liquid outlet, whose stream calculation can return NaN.
            if (double.IsNaN(fl) || double.IsInfinity(fl)) { Console.WriteLine("FL_NONFINITE[" + i + "]=1"); fl = 0.0; }
            fout[i] = fv + fl;
            Console.WriteLine("F_OUT[" + i + "]=" + F(fout[i]));
        }
        Console.WriteLine("ELBAL=" + F(r.ElementBalance));
        Console.WriteLine("G_INITIAL=" + F(r.InitialGibbsEnergy));
        Console.WriteLine("G_FINAL=" + F(r.FinalGibbsEnergy));

        // Upstream's own vapour fugacity coefficients at its outlet composition.
        double s = fout.Sum();
        var y = fout.Select(v => v / s).ToArray();
        pp.CurrentMaterialStream = outv;
        var phi = pp.DW_CalcFugCoeff(y, T, P, State.Vapor);
        for (int i = 0; i < comps.Length; i++) Console.WriteLine("LNPHI[" + i + "]=" + F(Math.Log(phi[i])));
    }
}
