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

class Col2 {
    static void Main() {
        var calc = new Calculator(); calc.Initialize();
        int nc = 2, nstages = 10, ns = nstages - 1;   // ns = last index
        var pp = new PengRobinsonPropertyPackage(true);
        calc.TransferCompounds(pp);
        var ms = new MaterialStream("", "");
        foreach (var phase in ms.Phases.Values)
            foreach (var cn in new[]{"Methane","Ethane"}) {
                phase.Compounds.Add(cn, new Compound(cn, ""));
                phase.Compounds[cn].ConstantProperties = pp._availablecomps[cn];
            }
        var fs = new HeadlessFlowsheet();
        ms.SetFlowsheet(fs); pp.CurrentMaterialStream = ms; pp.Flowsheet = fs;

        var col = new DistillationColumn();
        col.PropertyPackage = pp;
        col.Specs["C"] = new ColumnSpec { SType = ColumnSpec.SpecType.Stream_Ratio,
                                          SpecValue = 2.0, SpecUnit = "", StageNumber = 0 };
        col.Specs["R"] = new ColumnSpec { SType = ColumnSpec.SpecType.Product_Molar_Flow_Rate,
                                          SpecValue = 0.5, SpecUnit = "mol/s", StageNumber = ns };

        // EXACTLY this port's initial estimates, so the comparison is solver-to-solver.
        double[] V = {1e-10,3,3,3,3,3,3,3,3,3};
        double[] L = {2.5,3,3,3,3,4,4,4,4,1};
        double[] K = {3.2639145771, 0.0539472228};
        double[] X = {0.2947234887, 0.7052765113};
        double[] Y = {0.9619522909, 0.0380477091};

        var input = new ColumnSolverInputData {
            ColumnObject = col, NumberOfCompounds = nc, NumberOfStages = ns,
            MaximumIterations = 500,
            Tolerances = new List<double>{1e-6,1e-6,1e-6},
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
        for (int j = 0; j <= ns; j++) {
            input.StageTemperatures.Add(160.0);
            input.StagePressures.Add(5e5);
            input.StageHeats.Add(0.0);
            input.StageEfficiencies.Add(1.0);
            input.FeedFlows.Add(j == 5 ? 1.0 : 0.0);
            input.FeedCompositions.Add(j == 5 ? new double[]{0.5,0.5} : new double[]{0.0,0.0});
            input.FeedEnthalpies.Add(0.0);
            input.VaporFlows.Add(V[j]);
            input.LiquidFlows.Add(L[j]);
            input.VaporCompositions.Add(new double[]{Y[0],Y[1]});
            input.LiquidCompositions.Add(new double[]{X[0],X[1]});
            input.OverallCompositions.Add(new double[]{0.5,0.5});
            input.Kvalues.Add(new double[]{K[0],K[1]});
            input.VaporSideDraws.Add(0.0);
            input.LiquidSideDraws.Add(j == 0 ? 0.5 : 0.0);
        }
        try {
            var outp = new WangHenkeMethod().SolveColumn(input);
            Console.WriteLine($"DWSIM it={outp.IterationsTaken} err={outp.FinalError:E6}");
            Console.Write("DWSIM_T="); foreach (var v in outp.StageTemperatures) Console.Write($"{v:F4},"); Console.WriteLine();
            Console.Write("DWSIM_V="); foreach (var v in outp.VaporFlows) Console.Write($"{v:F6},"); Console.WriteLine();
            Console.Write("DWSIM_L="); foreach (var v in outp.LiquidFlows) Console.Write($"{v:F6},"); Console.WriteLine();
            var x = outp.LiquidCompositions;
            Console.WriteLine($"DWSIM_X0={x[0][0]:F6},{x[0][1]:F6}");
            Console.WriteLine($"DWSIM_X9={x[ns][0]:F6},{x[ns][1]:F6}");
        } catch (Exception e) { Console.WriteLine("ERR " + e.Message); }
    }
}
