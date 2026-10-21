// Headless driver for upstream DWSIM Reactor_Conversion (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::reactors::ConversionReactor.
// Prints KEY=value lines only; everything else goes to stderr.
//
// Reactions are created through FlowsheetBase.CreateConversionReaction, so
// upstream's own CalcReactionStoichiometry sets ReactionHeat (per mol of base
// reactant, FlowsheetBase.vb:4368). Each reaction carries a set RANK: equal
// ranks form a *parallel* group (Conversion.vb:459-823), different ranks run
// in sequence.
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
    public string Name, Base, Phase = "mixture";
    public Dictionary<string, double> Nu = new Dictionary<string, double>();
    public double XPercent;
    public int Rank;
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

    // Total combustion and partial combustion of methane in excess air.
    static Rx Total(double x, int rank) {
        var r = new Rx { Name = "total", Base = "Methane", XPercent = x, Rank = rank };
        r.Nu["Methane"] = -1; r.Nu["Oxygen"] = -2; r.Nu["Carbon dioxide"] = 1; r.Nu["Water"] = 2;
        return r;
    }
    static Rx Partial(double x, int rank) {
        var r = new Rx { Name = "partial", Base = "Methane", XPercent = x, Rank = rank };
        r.Nu["Methane"] = -1; r.Nu["Oxygen"] = -1.5; r.Nu["Carbon monoxide"] = 1; r.Nu["Water"] = 2;
        return r;
    }

    static Case Get(string name) {
        var c = new Case { Name = name };
        var comb = new[] { "Methane", "Oxygen", "Carbon dioxide", "Carbon monoxide", "Water", "Nitrogen" };
        switch (name) {
        case "single": {
            // CH4 + 2 O2 -> CO2 + 2 H2O, base reactant O2 (|nu_BC| = 2), X = 60 %.
            c.Comps = comb; c.Feed = new[] { 1.0, 1.5, 0.0, 0.0, 0.0, 5.64 };
            c.T = 1000.0; c.P = 1.0e5;
            var r = Total(60.0, 0); r.Base = "Oxygen"; c.Rxns.Add(r);
            break;
        }
        case "parallel":      // same rank: one parallel group
        case "sequential": {  // ranks 0 then 1
            c.Comps = comb; c.Feed = new[] { 1.0, 4.0, 0.0, 0.0, 0.0, 15.0 };
            c.T = 1000.0; c.P = 1.0e5;
            c.Rxns.Add(Total(50.0, 0));
            c.Rxns.Add(Partial(30.0, name == "parallel" ? 0 : 1));
            break;
        }
        case "overspec": {    // parallel group whose conversions sum to 140 %
            c.Comps = comb; c.Feed = new[] { 1.0, 4.0, 0.0, 0.0, 0.0, 15.0 };
            c.T = 1000.0; c.P = 1.0e5;
            c.Rxns.Add(Total(80.0, 0));
            c.Rxns.Add(Partial(60.0, 0));
            break;
        }
        case "unchecked": {
            // Parallel group: total combustion (X = 100 %) with O2 short (1 of
            // the 2 mol/s it needs), then CO shift (X = 50 %), which has no O2.
            // Conversion.vb:599-635 resets nif per reaction, so only the LAST
            // reaction's compounds are penalised if negative: O2 is never checked.
            c.Comps = new[] { "Methane", "Oxygen", "Carbon dioxide", "Carbon monoxide", "Water", "Hydrogen" };
            c.Feed = new[] { 1.0, 1.0, 0.0, 1.0, 1.0, 0.0 };
            c.T = 1000.0; c.P = 1.0e5;
            c.Rxns.Add(Total(100.0, 0));
            var s = new Rx { Name = "shift", Base = "Carbon monoxide", XPercent = 50.0, Rank = 0 };
            s.Nu["Carbon monoxide"] = -1; s.Nu["Water"] = -1; s.Nu["Carbon dioxide"] = 1; s.Nu["Hydrogen"] = 1;
            c.Rxns.Add(s);
            break;
        }
        case "liq_two_phase": {
            // n-butane -> isobutane on the LIQUID phase only, in a two-phase feed.
            c.Comps = new[] { "Methane", "N-butane", "Isobutane" };
            c.Feed = new[] { 1.0, 1.0, 0.0 };
            c.T = 300.0; c.P = 1.0e6;
            var r = new Rx { Name = "iso", Base = "N-butane", XPercent = 50.0, Rank = 0, Phase = "liquid" };
            r.Nu["N-butane"] = -1; r.Nu["Isobutane"] = 1;
            c.Rxns.Add(r);
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
        var r    = (Reactor_Conversion)fs.AddObject(ObjectType.RCT_Conversion, 100, 0, "CONV");
        foreach (ISimulationObject o in new ISimulationObject[] { ins, outv, outl, r }) o.PropertyPackage = pp;
        fs.ConnectObjects(ins.GraphicObject, r.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outv.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outl.GraphicObject, 1, 0);
        fs.ConnectObjects(es.GraphicObject, r.GraphicObject, 0, 1);

        var set = (ReactionSet)fs.CreateReactionSet("RS1", "");
        fs.AddReactionSet(set);
        var made = new List<IReaction>();
        foreach (var rx in c.Rxns) {
            var rxn = fs.CreateConversionReaction(rx.Name, "", rx.Nu, rx.Base, rx.Phase,
                rx.XPercent.ToString("R", CultureInfo.InvariantCulture));
            fs.AddReaction(rxn);
            fs.AddReactionToSet(rxn.ID, set.ID, true, rx.Rank);
            made.Add(rxn);
        }
        r.ReactionSetID = set.ID;
        r.ReactorOperationMode = OperationMode.Isothermic;

        double tot = c.Feed.Sum();
        ins.SetTemperature(c.T);
        ins.SetPressure(c.P);
        ins.SetOverallComposition(c.Feed.Select(f => f / tot).ToArray());
        ins.SetMolarFlow(tot);
        ins.Calculate(true, true);

        Console.WriteLine("CASE=" + c.Name);
        Console.WriteLine("T=" + F(c.T)); Console.WriteLine("P=" + F(c.P));
        Console.WriteLine("VAPFRAC_IN=" + F(ins.Phases[2].Properties.molarfraction.GetValueOrDefault()));
        for (int i = 0; i < c.Comps.Length; i++) {
            Console.WriteLine("F_IN[" + i + "]=" + F(c.Feed[i]));
            Console.WriteLine("F_IN_LIQ[" + i + "]=" + F(ins.Phases[1].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault()));
        }
        for (int k = 0; k < made.Count; k++)
            Console.WriteLine("RXN_HEAT[" + k + "]=" + F(made[k].ReactionHeat));

        try { r.Calculate(); }
        catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }

        for (int k = 0; k < made.Count; k++)
            Console.WriteLine("X_FINAL[" + k + "]=" + F(r.Conversions[made[k].ID]));
        outv.Calculate(true, true);
        outl.Calculate(true, true);
        for (int i = 0; i < c.Comps.Length; i++) {
            double fv = outv.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault();
            double fl = outl.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault();
            Console.WriteLine("F_OUT[" + i + "]=" + F(fv + fl));
        }
        Console.WriteLine("DELTAQ_KW=" + F(r.DeltaQ.GetValueOrDefault()));
    }
}
