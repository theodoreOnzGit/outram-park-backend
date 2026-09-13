using System;
using System.Collections.Generic;
using DWSIM.Thermodynamics.CalculatorInterface;
using DWSIM.Thermodynamics.PropertyPackages;
using DWSIM.Thermodynamics.Streams;
using DWSIM.Thermodynamics.BaseClasses;
using DWSIM.UnitOperations.UnitOperations;
using DWSIM.UnitOperations.UnitOperations.Auxiliary.SepOps;
using DWSIM.UnitOperations.UnitOperations.Auxiliary.SepOps.SolvingMethods;
using DWSIM.Interfaces;

/// Minimal headless flowsheet: the 12 abstract members of FlowsheetBase are all
/// UI-facing, so they are no-ops here. Nothing numerical lives in them.
class HeadlessFlowsheet : DWSIM.FlowsheetBase.FlowsheetBase {
    public override void DisplayForm(object form) {}
    public override IFlowsheet GetNewInstance() { return new HeadlessFlowsheet(); }
    public override void ShowDebugInfo(string text, int level) {}
    public override void ShowMessage(string text, IFlowsheet.MessageType mtype, string exceptionID = "") {}
    public override void UpdateOpenEditForms() {}
    public override void CloseOpenEditForms() {}
    public override void RunCodeOnUIThread(Action act) { act(); }
    public override void SetMessageListener(Action<string, IFlowsheet.MessageType> act) {}
    public override void UpdateInformation() {}
    public override void UpdateInterface() {}
    public override object GetApplicationObject() { return null; }
    public override bool SupressMessages { get; set; }
}

class Col {
    static void Main() {
        var calc = new Calculator(); calc.Initialize();
        int nc = 2, ns = 5;                       // 5 stages, condenser..reboiler
        var pp = new PengRobinsonPropertyPackage(true);
        calc.TransferCompounds(pp);

        // Upstream's package is stateful: it needs a CurrentMaterialStream
        // with the compound slate attached before any enthalpy/K call works.
        var ms = new MaterialStream("", "");
        foreach (var phase in ms.Phases.Values)
            foreach (var cn in new[]{"Methane","Ethane"}) {
                phase.Compounds.Add(cn, new Compound(cn, ""));
                phase.Compounds[cn].ConstantProperties = pp._availablecomps[cn];
            }
        var fs = new HeadlessFlowsheet();
        ms.SetFlowsheet(fs);
        pp.CurrentMaterialStream = ms;
        pp.Flowsheet = fs;

        var col = new DistillationColumn();
        col.PropertyPackage = pp;
        col.Specs["C"] = new ColumnSpec { SType = ColumnSpec.SpecType.Stream_Ratio,
                                          SpecValue = 2.0, SpecUnit = "", StageNumber = 0 };
        col.Specs["R"] = new ColumnSpec { SType = ColumnSpec.SpecType.Product_Molar_Flow_Rate,
                                          SpecValue = 0.5, SpecUnit = "mol/s", StageNumber = ns };

        var input = new ColumnSolverInputData {
            ColumnObject = col, NumberOfCompounds = nc, NumberOfStages = ns,   // DWSIM: ns is the LAST stage index; arrays are ns+1 long
            MaximumIterations = 500,
            Tolerances = new List<double>{1e-6, 1e-6, 1e-6},
            ColumnType = col.ColumnType, CondenserType = Column.condtype.Total_Condenser,
            StageTemperatures = new List<double>(), StagePressures = new List<double>(),
            StageHeats = new List<double>(), StageEfficiencies = new List<double>(),
            FeedFlows = new List<double>(), FeedCompositions = new List<double[]>(),
            FeedEnthalpies = new List<double>(),
            VaporFlows = new List<double>(), VaporCompositions = new List<double[]>(),
            LiquidFlows = new List<double>(), LiquidCompositions = new List<double[]>(),
            VaporSideDraws = new List<double>(), LiquidSideDraws = new List<double>(),
            Kvalues = new List<double[]>(), OverallCompositions = new List<double[]>(),
        };
        // Feed enthalpy from the package itself, not a hardcoded zero.
        double Tf = 210.0, Pf = 2e6;
        double[] zf = {0.5, 0.5};
        double hf = 0.0;
        try { hf = pp.DW_CalcEnthalpy(zf, Tf, Pf, DWSIM.Thermodynamics.PropertyPackages.State.Liquid); }
        catch (Exception ex) { Console.WriteLine("enthalpy err: " + ex.Message); }
        Console.WriteLine($"feed molar enthalpy = {hf:F4}");
        // Wilson K estimates at each stage temperature.
        double[] Tc = {190.56, 305.32}, Pc = {4.599e6, 4.872e6}, w = {0.011, 0.099};

        for (int j = 0; j <= ns; j++) {
            double Tj = 200.0 + 8.0*j;
            input.StageTemperatures.Add(Tj);
            input.StagePressures.Add(2e6);
            input.StageHeats.Add(0.0);
            input.StageEfficiencies.Add(1.0);
            input.FeedFlows.Add(j == 2 ? 1.0 : 0.0);
            input.FeedCompositions.Add(j == 2 ? new double[]{0.5,0.5} : new double[]{0.0,0.0});
            input.FeedEnthalpies.Add(j == 2 ? hf : 0.0);
            input.VaporFlows.Add(0.5);
            input.LiquidFlows.Add(0.5);
            input.VaporCompositions.Add(new double[]{0.7,0.3});
            input.LiquidCompositions.Add(new double[]{0.3,0.7});
            input.OverallCompositions.Add(new double[]{0.5,0.5});
            var kj = new double[2];
            for (int i = 0; i < 2; i++)
                kj[i] = Pc[i]/2e6 * Math.Exp(5.373*(1.0+w[i])*(1.0 - Tc[i]/Tj));
            input.Kvalues.Add(kj);
            input.VaporSideDraws.Add(0.0);
            input.LiquidSideDraws.Add(j == 0 ? 0.5 : 0.0);
        }
        try {
            var solver = new WangHenkeMethod();
            var outp = solver.SolveColumn(input);
            Console.WriteLine($"iterations = {outp.IterationsTaken}, final error = {outp.FinalError:E6}");
            Console.WriteLine("stage temperatures:");
            foreach (var t in outp.StageTemperatures) Console.Write($" {t:F4}");
            Console.WriteLine();
        } catch (Exception e) {
            Console.WriteLine(e.ToString());
        }
    }
}
