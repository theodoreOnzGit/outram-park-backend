// Headless driver for upstream DWSIM Reactor_CSTR (pinned 1abf72d1).
// Compared code-to-code against outram-park-fork-dwsim-libs::reactors::Cstr.
// Prints KEY=value lines only; everything else goes to stderr.
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

// One kinetic reaction: stoichiometry (compound -> nu), forward/reverse orders,
// Arrhenius pairs, base reactant. MolarConc basis, mol/m3 and mol/[m3.s].
class Rx {
    public string Name, Base;
    public Dictionary<string, double> Nu = new Dictionary<string, double>();
    public Dictionary<string, double> Fwd = new Dictionary<string, double>();
    public Dictionary<string, double> Rev = new Dictionary<string, double>();
    public double Af, Ef, Ar, Er;
}

class Case {
    public string Name;
    public string[] Comps;
    public double[] Feed;          // mol/s
    public double T, P, V, Headspace;
    public ReactionPhase Phase;
    public List<Rx> Rxns = new List<Rx>();
}

class Driver {
    static string F(double v) { return v.ToString("R", CultureInfo.InvariantCulture); }

    static Case Get(string name) {
        var c = new Case { Name = name };
        switch (name) {
        case "iso_liq":
        case "iso_gas":
        case "iso_gas_mix": {
            c.Comps = new[] { "N-butane", "Isobutane" };
            c.Feed = new[] { 1.0, 0.0 };
            if (name == "iso_liq") { c.T = 300.0; c.P = 1.0e6; c.V = 0.02; c.Phase = ReactionPhase.Liquid; }
            else if (name == "iso_gas") { c.T = 400.0; c.P = 1.0e5; c.V = 20.0; c.Headspace = 20.0; c.Phase = ReactionPhase.Vapor; }
            else                   { c.T = 400.0; c.P = 1.0e5; c.V = 20.0; c.Headspace = 0.0; c.Phase = ReactionPhase.Mixture; }
            var r = new Rx { Name = "iso", Base = "N-butane", Af = 0.01, Ef = 0.0, Ar = 0.0, Er = 0.0 };
            r.Nu["N-butane"] = -1; r.Nu["Isobutane"] = 1;
            r.Fwd["N-butane"] = 1; r.Fwd["Isobutane"] = 0;
            r.Rev["N-butane"] = 0; r.Rev["Isobutane"] = 0;
            c.Rxns.Add(r);
            break;
        }
        case "smr": {
            // DOVER's base deck (crates/dover/decks/smr_cstr.toml), reverse rates from
            // dover::smr::consistent_reverse at 1123.15 K, printed at full precision.
            c.Comps = new[] { "Methane", "Water", "Carbon monoxide", "Carbon dioxide", "Hydrogen" };
            c.Feed = new[] { 1.0, 3.0, 0.0, 0.0, 0.0 };
            c.T = 1123.15; c.P = 2.0e6; c.V = 2.0; c.Headspace = 0.0; c.Phase = ReactionPhase.Mixture;
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
        var r    = (Reactor_CSTR)fs.AddObject(ObjectType.RCT_CSTR, 100, 0, "CSTR");
        foreach (ISimulationObject o in new ISimulationObject[] { ins, outs, r }) o.PropertyPackage = pp;
        fs.ConnectObjects(ins.GraphicObject, r.GraphicObject, 0, 0);
        fs.ConnectObjects(r.GraphicObject, outs.GraphicObject, 0, 0);
        fs.ConnectObjects(es.GraphicObject, r.GraphicObject, 0, 1);

        // Reactions and set.
        var set = new ReactionSet { ID = "RS1", Name = "RS1" };
        int rank = 0;
        foreach (var rx in c.Rxns) {
            var rxn = new Reaction(rx.Name, rx.Name, "");
            rxn.ReactionType = ReactionType.Kinetic;
            rxn.ReactionBasis = ReactionBasis.MolarConc;
            rxn.ReactionPhase = c.Phase;
            rxn.BaseReactant = rx.Base;
            rxn.ConcUnit = "mol/m3";
            rxn.VelUnit = "mol/[m3.s]";
            rxn.A_Forward = rx.Af; rxn.E_Forward = rx.Ef;
            rxn.A_Reverse = rx.Ar; rxn.E_Reverse = rx.Er;
            foreach (var kv in rx.Nu)
                rxn.Components.Add(kv.Key, new ReactionStoichBase(kv.Key, kv.Value, kv.Key == rx.Base, rx.Fwd[kv.Key], rx.Rev[kv.Key]));
            fs.AddReaction(rxn);
            set.Reactions.Add(rxn.ID, new ReactionSetBase(rxn.ID, rank++, true));
        }
        fs.AddReactionSet(set);
        r.ReactionSetID = set.ID;
        r.ReactorOperationMode = OperationMode.Isothermic;
        r.Volume = c.V;
        r.Headspace = c.Headspace;
        var mi = Environment.GetEnvironmentVariable("CSTR_MAXIT");
        if (!string.IsNullOrEmpty(mi)) r.MaxIterations = int.Parse(mi);
        var tl = Environment.GetEnvironmentVariable("CSTR_TOL");
        if (!string.IsNullOrEmpty(tl)) r.Tolerance = double.Parse(tl, CultureInfo.InvariantCulture);
        Console.WriteLine("MAXIT=" + r.MaxIterations); Console.WriteLine("TOL=" + F(r.Tolerance));

        // Inlet.
        double tot = c.Feed.Sum();
        ins.SetTemperature(c.T);
        ins.SetPressure(c.P);
        ins.SetOverallComposition(c.Feed.Select(f => f / tot).ToArray());
        ins.SetMolarFlow(tot);
        ins.Calculate(true, true);

        Console.WriteLine("CASE=" + c.Name);
        Console.WriteLine("T=" + F(c.T)); Console.WriteLine("P=" + F(c.P)); Console.WriteLine("V=" + F(c.V)); Console.WriteLine("HEADSPACE=" + F(c.Headspace));
        Console.WriteLine("Q_IN=" + F(ins.Phases[0].Properties.volumetric_flow.GetValueOrDefault()));
        Console.WriteLine("QL_IN=" + F(ins.Phases[1].Properties.volumetric_flow.GetValueOrDefault()));
        Console.WriteLine("QV_IN=" + F(ins.Phases[2].Properties.volumetric_flow.GetValueOrDefault()));
        Console.WriteLine("VAPFRAC_IN=" + F(ins.Phases[2].Properties.molarfraction.GetValueOrDefault()));
        for (int i = 0; i < c.Comps.Length; i++) Console.WriteLine("F_IN[" + i + "]=" + F(c.Feed[i]));

        var t0 = DateTime.UtcNow;
        try { r.Calculate(); }
        catch (Exception e) { Console.WriteLine("ERROR=" + e.Message.Replace('\n', ' ')); return; }
        Console.WriteLine("WALL_MS=" + F((DateTime.UtcNow - t0).TotalMilliseconds));

        // Raw values exactly as the reactor wrote them (before any stream calc).
        Console.WriteLine("MASSFLOW_OUT_RAW=" + F(outs.Phases[0].Properties.massflow.GetValueOrDefault()));
        for (int i = 0; i < c.Comps.Length; i++)
            Console.WriteLine("X_OUT_RAW[" + i + "]=" + F(outs.Phases[0].Compounds[c.Comps[i]].MoleFraction.GetValueOrDefault()));
        // What the flowsheet solver does next: calculate the outlet stream.
        outs.Calculate(true, true);
        for (int i = 0; i < c.Comps.Length; i++)
            Console.WriteLine("F_OUT[" + i + "]=" + F(outs.Phases[0].Compounds[c.Comps[i]].MolarFlow.GetValueOrDefault()));
        Console.WriteLine("MOLARFLOW_OUT=" + F(outs.Phases[0].Properties.molarflow.GetValueOrDefault()));
        Console.WriteLine("Q_OUT=" + F(outs.Phases[0].Properties.volumetric_flow.GetValueOrDefault()));
        Console.WriteLine("TAU_L=" + F(r.ResidenceTimeL));
        Console.WriteLine("DELTAQ=" + F(r.DeltaQ.GetValueOrDefault()));
    }
}
