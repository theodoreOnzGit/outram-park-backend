// Headless driver for upstream DWSIM Reactor_Equilibrium (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::reactors::EquilibriumReactor.
// Prints KEY=value lines only; everything else goes to stderr.
//
// Reactions are created through FlowsheetBase.CreateEquilibriumReaction with an
// explicit ln K(T) expression (KOpt.Expression: K = exp(expr(T)),
// ThermodynamicsBase.vb:274-284), so both codes evaluate the identical K and the
// comparison isolates the basis and the solver. ReactionPhase is "vapor": a
// "mixture" reaction matches neither branch of FunctionValue2N
// (Equilibrium.vb:336, :350) and contributes prod = 1.
//
// EQ_TOL sets InternalLoopTolerance (default 1e-3, Equilibrium.vb:77), which
// bounds the SUM OF SQUARED ln-residuals (:1407).
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
using DWSIM.Thermodynamics.BaseClasses;
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

class Rx {
    public string Name, Base, Basis, Units, LnK;
    public Dictionary<string, double> Nu = new Dictionary<string, double>();
}

class Case {
    public string Name;
    public string[] Comps;
    public double[] Feed; // mol/s
    public double T, P;
    public List<Rx> Rxns = new List<Rx>();
}

class Driver {
    static string F(double v) { return v.ToString("R", CultureInfo.InvariantCulture); }

    // Water-gas shift CO + H2O <-> CO2 + H2 (dnu = 0).
    // ln K = 4577.8/T - 4.33 : a fixed form typed identically into both codes.
    static Rx Wgs(string basis) {
        var r = new Rx { Name = "wgs", Base = "Carbon monoxide", Basis = basis, Units = "Pa",
                         LnK = "4577.8/T - 4.33" };
        r.Nu["Carbon monoxide"] = -1; r.Nu["Water"] = -1; r.Nu["Carbon dioxide"] = 1; r.Nu["Hydrogen"] = 1;
        return r;
    }

    // Steam-methane reforming CH4 + H2O <-> CO + 3 H2 (dnu = +2).
    // ln K = 30.42 - 27106/T : a fixed form typed identically into both codes;
    // for the partial-pressure basis in Pa, + 2 ln(101325) is added so K is per Pa^2.
    static Rx Smr(string basis, string lnk) {
        var r = new Rx { Name = "smr", Base = "Methane", Basis = basis, Units = "Pa", LnK = lnk };
        r.Nu["Methane"] = -1; r.Nu["Water"] = -1; r.Nu["Carbon monoxide"] = 1; r.Nu["Hydrogen"] = 3;
        return r;
    }

    static Case Get(string name) {
        var c = new Case { Name = name };
        switch (name) {
        case "wgs_molfrac":
        case "wgs_activity": {
            c.Comps = new[] { "Carbon monoxide", "Water", "Carbon dioxide", "Hydrogen", "Nitrogen" };
            c.Feed = new[] { 1.0, 1.0, 0.0, 0.0, 1.0 };
            c.T = 1000.0; c.P = 1.0e5;
            c.Rxns.Add(Wgs(name == "wgs_molfrac" ? "molar fraction" : "activity"));
            break;
        }
        case "smr_molfrac":
        case "smr_activity":
        case "smr_fugacity":
        case "smr_pp":
        case "smr_pp_30bar": {
            c.Comps = new[] { "Methane", "Water", "Carbon monoxide", "Hydrogen" };
            c.Feed = new[] { 1.0, 3.0, 0.0, 0.0 };
            c.T = 900.0; c.P = name == "smr_pp_30bar" ? 3.0e6 : 1.0e6;
            if (name == "smr_molfrac") c.Rxns.Add(Smr("molar fraction", "30.42 - 27106/T"));
            else if (name == "smr_activity") c.Rxns.Add(Smr("activity", "30.42 - 27106/T"));
            else if (name == "smr_fugacity") c.Rxns.Add(Smr("fugacity", "30.42 - 27106/T"));
            else c.Rxns.Add(Smr("partial pressure", "30.42 - 27106/T + 23.05217690299302")); // + 2 ln(101325)
            break;
        }
        default: throw new Exception("unknown case " + name);
        }
        return c;
    }

    static void Main(string[] args) {
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;
        System.Threading.Thread.CurrentThread.CurrentCulture = CultureInfo.InvariantCulture;
        var c = Get(args[0]);

        var calc = new Calculator(); calc.Initialize();
        var pp = new PengRobinsonPropertyPackage(true);
        calc.TransferCompounds(pp);

        var fs = new HeadlessFlowsheet();
        foreach (var n in c.Comps) fs.Options.SelectedComponents.Add(n, pp._availablecomps[n]);
        fs.AddPropertyPackage(pp);

        var ins  = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 0, 0, "IN");
        var outv = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 200, 0, "OUTV");
        var outl = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 200, 100, "OUTL");
        var es   = (EnergyStream)fs.AddObject(ObjectType.EnergyStream, 100, 100, "Q");
        var r    = (Reactor_Equilibrium)fs.AddObject(ObjectType.RCT_Equilibrium, 100, 0, "EQ");
        foreach (ISimulationObject o in new ISimulationObject[] { ins, outv, outl, r }) o.PropertyPackage = pp;
        fs.ConnectObjects(ins.GraphicObject, r.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outv.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outl.GraphicObject, 1, 0);
        fs.ConnectObjects(es.GraphicObject, r.GraphicObject, 0, 1);

        var set = (ReactionSet)fs.CreateReactionSet("RS1", "");
        fs.AddReactionSet(set);
        var made = new List<IReaction>();
        foreach (var rx in c.Rxns) {
            var rxn = fs.CreateEquilibriumReaction(rx.Name, "", rx.Nu, rx.Base, "vapor", rx.Basis, rx.Units, 0.0, rx.LnK);
            fs.AddReaction(rxn);
            fs.AddReactionToSet(rxn.ID, set.ID, true, 0);
            made.Add(rxn);
        }
        r.ReactionSetID = set.ID;
        r.ReactorOperationMode = OperationMode.Isothermic;
        var tl = Environment.GetEnvironmentVariable("EQ_TOL");
        if (!string.IsNullOrEmpty(tl)) r.InternalLoopTolerance = double.Parse(tl, CultureInfo.InvariantCulture);
        Console.WriteLine("TOL=" + F(r.InternalLoopTolerance));

        double tot = c.Feed.Sum();
        ins.SetTemperature(c.T);
        ins.SetPressure(c.P);
        ins.SetOverallComposition(c.Feed.Select(f => f / tot).ToArray());
        ins.SetMolarFlow(tot);
        ins.Calculate(true, true);

        Console.WriteLine("CASE=" + c.Name);
        Console.WriteLine("VAPFRAC_IN=" + F(ins.Phases[2].Properties.molarfraction.GetValueOrDefault()));
        for (int k = 0; k < made.Count; k++)
            Console.WriteLine("K[" + k + "]=" + F(((Reaction)made[k]).EvaluateK(c.T, pp)));

        try { r.Calculate(); }
        catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }

        for (int k = 0; k < made.Count; k++)
            Console.WriteLine("EXTENT[" + k + "]=" + F(r.ReactionExtents[made[k].ID]));
        outv.Calculate(true, true);
        outl.Calculate(true, true);
        for (int i = 0; i < c.Comps.Length; i++) {
            double fv = outv.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault();
            double fl = outl.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault();
            Console.WriteLine("F_OUT[" + i + "]=" + F(fv + fl));
        }
        // PR vapour fugacity coefficients at the outlet composition, for the record.
        var z = c.Comps.Select(n => outv.Phases[0].Compounds[n].MoleFraction.GetValueOrDefault()).ToArray();
        pp.CurrentMaterialStream = outv;
        var phi = pp.DW_CalcFugCoeff(z, c.T, c.P, State.Vapor);
        for (int i = 0; i < c.Comps.Length; i++) Console.WriteLine("PHI_V[" + i + "]=" + F(phi[i]));
    }
}
