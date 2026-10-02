// Headless driver for upstream DWSIM Reactor_PFR (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::reactors::Pfr.
// Prints KEY=value lines only; everything else goes to stderr.
//
// Upstream marches the volume in 1/dV segments (PFR.vb:894; dV defaults to
// 0.01), freezes the volumetric flow Q at each segment's start (PFR.vb:955-972),
// integrates the segment with a DotNumerics ODE solver at its default
// RelTol 1e-3 / AbsTol 1e-6 (PFR.vb:1108-1169, xOdeBase.cs:96,105), then
// re-flashes. The per-segment profile is printed so the port can be run on the
// same piecewise-constant Q.
//
// Env: PFR_DV (segment fraction, default upstream's 0.01),
//      PFR_SOLVER (InternalSolver 0..3; 0 = implicit RK5, upstream's default).
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
    public string Name, Base;
    public Dictionary<string, double> Nu = new Dictionary<string, double>();
    public Dictionary<string, double> Fwd = new Dictionary<string, double>();
    public Dictionary<string, double> Rev = new Dictionary<string, double>();
    public double Af, Ef, Ar, Er;
    public string Num, Den; // heterogeneous catalytic: rate = Num / Den (PFR.vb:368-425)
}

class Case {
    public string Name;
    public string[] Comps;
    public double[] Feed; // mol/s
    public double T, P, V;
    public string Phase = "mixture";
    public double Loading, Void; // CatalystLoading [kg/m3], CatalystVoidFraction
    public List<Rx> Rxns = new List<Rx>();
}

class Driver {
    static string F(double v) { return v.ToString("R", CultureInfo.InvariantCulture); }

    static Rx Iso() {
        var r = new Rx { Name = "iso", Base = "N-butane", Af = 0.01, Ef = 0.0, Ar = 0.0, Er = 0.0 };
        r.Nu["N-butane"] = -1; r.Nu["Isobutane"] = 1;
        r.Fwd["N-butane"] = 1; r.Fwd["Isobutane"] = 0;
        r.Rev["N-butane"] = 0; r.Rev["Isobutane"] = 0;
        return r;
    }

    static Case Get(string name) {
        var c = new Case { Name = name };
        switch (name) {
        case "iso_liq":
            // Same kinetics and conditions as the CSTR driver's iso_liq.
            c.Comps = new[] { "N-butane", "Isobutane" }; c.Feed = new[] { 1.0, 0.0 };
            c.T = 300.0; c.P = 1.0e6; c.V = 0.02; c.Rxns.Add(Iso());
            break;
        case "iso_gas":
            c.Comps = new[] { "N-butane", "Isobutane" }; c.Feed = new[] { 1.0, 0.0 };
            c.T = 400.0; c.P = 1.0e5; c.V = 20.0; c.Rxns.Add(Iso());
            break;
        case "hetcat_gas": {
            // n-butane -> isobutane on a packed bed, Langmuir-Hinshelwood:
            // rate = 1e-5 C_A / (1 + 0.05 C_A)^2 [mol/(kg.s)], C_A in mol/m3.
            c.Comps = new[] { "N-butane", "Isobutane" }; c.Feed = new[] { 1.0, 0.0 };
            c.T = 400.0; c.P = 1.0e5; c.V = 20.0; c.Loading = 500.0; c.Void = 0.4;
            var r = new Rx { Name = "iso_cat", Base = "N-butane", Num = "0.00001*R1", Den = "(1+0.05*R1)^2" };
            r.Nu["N-butane"] = -1; r.Nu["Isobutane"] = 1;
            c.Rxns.Add(r);
            break;
        }
        case "smr": {
            // The CSTR driver's DOVER steam-reforming deck, in a PFR of the same volume.
            c.Comps = new[] { "Methane", "Water", "Carbon monoxide", "Carbon dioxide", "Hydrogen" };
            c.Feed = new[] { 1.0, 3.0, 0.0, 0.0, 0.0 };
            c.T = 1123.15; c.P = 2.0e6; c.V = 2.0;
            var r0 = new Rx { Name = "reforming", Base = "Methane",
                Af = 1.0e7, Ef = 2.4e5, Ar = 5.314382778663273e-7, Er = 34100.0 };
            r0.Nu["Methane"] = -1; r0.Nu["Water"] = -1; r0.Nu["Carbon monoxide"] = 1; r0.Nu["Hydrogen"] = 3;
            r0.Fwd["Methane"] = 1; r0.Fwd["Water"] = 1; r0.Fwd["Carbon monoxide"] = 0; r0.Fwd["Hydrogen"] = 0;
            r0.Rev["Methane"] = 0; r0.Rev["Water"] = 0; r0.Rev["Carbon monoxide"] = 1; r0.Rev["Hydrogen"] = 3;
            var r1 = new Rx { Name = "shift", Base = "Carbon monoxide",
                Af = 100.0, Ef = 67000.0, Ar = 15628.549082752832, Er = 108200.0 };
            r1.Nu["Carbon monoxide"] = -1; r1.Nu["Water"] = -1; r1.Nu["Carbon dioxide"] = 1; r1.Nu["Hydrogen"] = 1;
            r1.Fwd["Carbon monoxide"] = 1; r1.Fwd["Water"] = 1; r1.Fwd["Carbon dioxide"] = 0; r1.Fwd["Hydrogen"] = 0;
            r1.Rev["Carbon monoxide"] = 0; r1.Rev["Water"] = 0; r1.Rev["Carbon dioxide"] = 1; r1.Rev["Hydrogen"] = 1;
            c.Rxns.Add(r0); c.Rxns.Add(r1);
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
        var outs = (MaterialStream)fs.AddObject(ObjectType.MaterialStream, 200, 0, "OUT");
        var es   = (EnergyStream)fs.AddObject(ObjectType.EnergyStream, 100, 100, "Q");
        var r    = (Reactor_PFR)fs.AddObject(ObjectType.RCT_PFR, 100, 0, "PFR");
        foreach (ISimulationObject o in new ISimulationObject[] { ins, outs, r }) o.PropertyPackage = pp;
        fs.ConnectObjects(ins.GraphicObject, r.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outs.GraphicObject, 0, 0);
        fs.ConnectObjects(es.GraphicObject, r.GraphicObject, 0, 1);

        var set = new ReactionSet { ID = "RS1", Name = "RS1" };
        int rank = 0;
        foreach (var rx in c.Rxns) {
            var rxn = rx.Num != null
                ? (Reaction)fs.CreateHetCatReaction(rx.Name, "", rx.Nu, rx.Base, c.Phase,
                    "molar concentration", "mol/m3", "mol/[kg.s]", rx.Num, rx.Den)
                : (Reaction)fs.CreateKineticReaction(rx.Name, "", rx.Nu, rx.Fwd, rx.Rev, rx.Base, c.Phase,
                    "molar concentration", "mol/m3", "mol/[m3.s]", rx.Af, rx.Ef, rx.Ar, rx.Er, "", "");
            fs.AddReaction(rxn);
            // Rank 0 for all: one parallel group, every reaction in one ODE system.
            set.Reactions.Add(rxn.ID, new ReactionSetBase(rxn.ID, 0, true));
            rank++;
        }
        fs.AddReactionSet(set);
        r.ReactionSetID = set.ID;
        r.ReactorOperationMode = OperationMode.Isothermic;
        r.ReactorSizingType = Reactor_PFR.SizingType.Length;
        r.Volume = c.V;
        r.Length = 1.0;
        r.CatalystLoading = c.Loading;
        r.CatalystVoidFraction = c.Void;
        // Isobaric comparison: with a bed, upstream applies Ergun (needs a particle
        // diameter, default 0, so dP explodes). Pin dP = 0; the port has no dP.
        r.UseUserDefinedPressureDrop = true;
        r.UserDefinedPressureDrop = 0.0;
        var dv = Environment.GetEnvironmentVariable("PFR_DV");
        if (!string.IsNullOrEmpty(dv)) r.dV = double.Parse(dv, CultureInfo.InvariantCulture);
        var sv = Environment.GetEnvironmentVariable("PFR_SOLVER");
        if (!string.IsNullOrEmpty(sv)) r.InternalSolver = int.Parse(sv);
        Console.WriteLine("DV=" + F(r.dV)); Console.WriteLine("SOLVER=" + r.InternalSolver);

        double tot = c.Feed.Sum();
        ins.SetTemperature(c.T);
        ins.SetPressure(c.P);
        ins.SetOverallComposition(c.Feed.Select(f => f / tot).ToArray());
        ins.SetMolarFlow(tot);
        ins.Calculate(true, true);

        Console.WriteLine("CASE=" + c.Name);
        Console.WriteLine("V=" + F(c.V));
        Console.WriteLine("Q_IN=" + F(ins.Phases[0].Properties.volumetric_flow.GetValueOrDefault()));
        Console.WriteLine("VAPFRAC_IN=" + F(ins.Phases[2].Properties.molarfraction.GetValueOrDefault()));
        for (int i = 0; i < c.Comps.Length; i++) Console.WriteLine("F_IN[" + i + "]=" + F(c.Feed[i]));

        var t0 = DateTime.UtcNow;
        try { r.Calculate(); }
        catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }
        Console.WriteLine("WALL_MS=" + F((DateTime.UtcNow - t0).TotalMilliseconds));

        // Profile: (position along Length, T, P, items). Q at each point is
        // MolarFlow / MolarConcentration of any compound with flow (PFR.vb:810-811).
        Console.WriteLine("NPROFILE=" + r.Profile.Count);
        for (int k = 0; k < r.Profile.Count; k++) {
            var pt = r.Profile[k];
            double q = double.NaN;
            foreach (var it in pt.Item4) if (it.MolarFlow > 0 && it.MolarConcentration > 0) { q = it.MolarFlow / it.MolarConcentration; break; }
            var flows = string.Join(",", c.Comps.Select(n => F(pt.Item4.First(it => it.Compound == n).MolarFlow)));
            Console.WriteLine("PROFILE[" + k + "]=" + F(pt.Item1) + ";" + F(q) + ";" + flows);
        }

        outs.Calculate(true, true);
        for (int i = 0; i < c.Comps.Length; i++)
            Console.WriteLine("F_OUT[" + i + "]=" + F(outs.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault()));
        Console.WriteLine("Q_OUT=" + F(outs.Phases[0].Properties.volumetric_flow.GetValueOrDefault()));
    }
}
