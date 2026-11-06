/*
 * SPDX-License-Identifier: LGPL-3.0-or-later
 *
 * gh:#1192 reference driver: runs OpenDA's own algorithms, on OpenDA's own
 * test models, on fixed inputs, and writes every number the Rust port is
 * compared against. It is compiled against OpenDA commit
 * e51565c2c8def7d35f8b378a9ac118edbd98a94d (OpenDA-Association/OpenDA,
 * 2026-10-23) and lives in OpenDA's package so it can read the protected
 * ensemble; it adds no algorithm. Because it links OpenDA (LGPL-3.0) it is
 * offered under the same licence.
 *
 * What it does that OpenDA's tests do not:
 *  - replaces the java.util.Random behind StochVector, ParticleFilter and
 *    AutoLocalizationZhang2011 with a recording subclass (same seed, same
 *    stream), so every draw OpenDA consumed is written out and can be
 *    injected into the port;
 *  - captures the forecast ensemble, the gain before and after localisation,
 *    and the analysis ensemble at every analysis time (subclasses that call
 *    super; no arithmetic is changed);
 *  - records every cost-function evaluation of the optimisers, in order.
 *
 * Generating command: see ../generate_references.sh.
 */
package org.openda.algorithms.kalmanFilter;

import org.openda.algorithms.*;
import org.openda.interfaces.*;
import org.openda.models.lorenz.LorenzStochModelFactory;
import org.openda.models.oscillator.OscillatorStochModelFactory;
import org.openda.models.oscillator.OscillatorStochModelInstance;
import org.openda.observers.CsvStochObserver;
import org.openda.utils.Matrix;
import org.openda.utils.StochVector;
import org.openda.utils.Vector;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

public class Gh1192Driver {

	// ------------------------------------------------------------------
	// A java.util.Random that records what it hands out. Same algorithm,
	// same seed handling (it IS java.util.Random), only logging added.
	// ------------------------------------------------------------------
	public static class RecordingRandom extends Random {
		public final List<Double> gaussians = new ArrayList<>();
		public final List<Double> doubles = new ArrayList<>();
		public final List<Integer> ints = new ArrayList<>();
		public RecordingRandom(long seed) { super(seed); }
		@Override public synchronized double nextGaussian() { double g = super.nextGaussian(); gaussians.add(g); return g; }
		@Override public double nextDouble() { double d = super.nextDouble(); doubles.add(d); return d; }
		@Override public int nextInt(int bound) { int i = super.nextInt(bound); ints.add(i); return i; }
	}

	static void setStatic(Class<?> c, String name, Object value) throws Exception {
		Field f = c.getDeclaredField(name);
		f.setAccessible(true);
		f.set(null, value);
	}
	static void setField(Object o, Class<?> c, String name, Object value) throws Exception {
		Field f = c.getDeclaredField(name);
		f.setAccessible(true);
		f.set(o, value);
	}
	static Object getField(Object o, Class<?> c, String name) throws Exception {
		Field f = c.getDeclaredField(name);
		f.setAccessible(true);
		return f.get(o);
	}

	// ------------------------------------------------------------------
	// Minimal JSON writer. Doubles via Double.toString, which round-trips
	// exactly (shortest repr, JDK >= 19).
	// ------------------------------------------------------------------
	static class J {
		final StringBuilder sb = new StringBuilder();
		boolean first = true;
		J open() { sb.append('{'); first = true; return this; }
		J close() { sb.append('}'); first = false; return this; }
		J key(String k) { if (!first) sb.append(','); first = false; sb.append('"').append(k).append("\":"); return this; }
		J num(double v) {
			if (Double.isNaN(v) || Double.isInfinite(v)) sb.append('"').append(v).append('"');
			else sb.append(Double.toString(v));
			return this;
		}
		J str(String s) { sb.append('"').append(s).append('"'); return this; }
		J raw(String s) { sb.append(s); return this; }
		J arr(double[] v) { sb.append('['); for (int i = 0; i < v.length; i++) { if (i > 0) sb.append(','); num(v[i]); } sb.append(']'); return this; }
		J arr(List<Double> v) { double[] a = new double[v.size()]; for (int i = 0; i < a.length; i++) a[i] = v.get(i); return arr(a); }
		J iarr(List<Integer> v) { sb.append('['); for (int i = 0; i < v.size(); i++) { if (i > 0) sb.append(','); sb.append(v.get(i)); } sb.append(']'); return this; }
		J mat(double[][] m) { sb.append('['); for (int i = 0; i < m.length; i++) { if (i > 0) sb.append(','); arr(m[i]); } sb.append(']'); return this; }
		J vecs(IVector[] v) { double[][] m = new double[v.length][]; for (int i = 0; i < v.length; i++) m[i] = v[i].getValues(); return mat(m); }
		J vecList(List<double[]> v) { double[][] m = v.toArray(new double[0][]); return mat(m); }
		J matList(List<double[][]> v) { sb.append('['); for (int i = 0; i < v.size(); i++) { if (i > 0) sb.append(','); mat(v.get(i)); } sb.append(']'); return this; }
	}

	static void write(File dir, String name, String content) throws IOException {
		try (FileWriter w = new FileWriter(new File(dir, name))) { w.write(content); w.write("\n"); }
	}

	// ------------------------------------------------------------------
	// Capture of one analysis.
	// ------------------------------------------------------------------
	static class Step {
		double time;
		double[][] forecast, analysis, gainPre, gainPost;
		double[] obs, obsStd, mainAfter;
	}

	static double[][] states(IStochModelInstance[] ens) {
		double[][] r = new double[ens.length][];
		for (int i = 0; i < ens.length; i++) r[i] = ens[i].getState().getValues();
		return r;
	}

	/** EnKF that records; every override calls super unchanged. */
	public static class CapturingEnKF extends EnKF {
		public final List<Step> steps = new ArrayList<>();
		Step cur;
		@Override
		protected IVector[] computeGainMatrix(IStochObserver obs, EnsembleVectors p, EnsembleVectors x, boolean lin, boolean out) {
			IVector[] k = super.computeGainMatrix(obs, p, x, lin, out);
			// Only the top-level call (out == true); Zhang's bootstrap calls with out == write_output too,
			// so record the first call of each analysis only.
			if (cur != null && cur.gainPre == null) {
				double[][] g = new double[k.length][];
				for (int i = 0; i < k.length; i++) g[i] = k[i].getValues();
				cur.gainPre = g;
			}
			return k;
		}
		@Override
		protected void applyLocalizationToGain(IStochObserver obs, IVector[] k, EnsembleVectors p, EnsembleVectors x) {
			super.applyLocalizationToGain(obs, k, p, x);
			double[][] g = new double[k.length][];
			for (int i = 0; i < k.length; i++) g[i] = k[i].getValues();
			cur.gainPost = g;
		}
		@Override
		public void analysis(IStochObserver obs, IVector obsValues, IVector pred, IStochModelInstance main, ITime t) {
			cur = new Step();
			cur.time = t.getMJD();
			cur.forecast = states(this.ensemble);
			cur.obs = obs.getExpectations().getValues();
			cur.obsStd = obs.getStandardDeviations().getValues();
			super.analysis(obs, obsValues, pred, main, t);
			cur.analysis = states(this.ensemble);
			cur.mainAfter = main.getState().getValues();
			steps.add(cur);
			cur = null;
		}
	}

	public static class CapturingEnSR extends EnSR {
		public final List<Step> steps = new ArrayList<>();
		@Override
		public void analysis(IStochObserver obs, IVector obsValues, IVector pred, IStochModelInstance main, ITime t) {
			Step s = new Step();
			s.time = t.getMJD();
			s.forecast = states(this.ensemble);
			s.obs = obs.getExpectations().getValues();
			s.obsStd = obs.getStandardDeviations().getValues();
			super.analysis(obs, obsValues, pred, main, t);
			s.analysis = states(this.ensemble);
			s.mainAfter = main.getState().getValues();
			steps.add(s);
		}
	}

	public static class CapturingPF extends ParticleFilter {
		public final List<Step> steps = new ArrayList<>();
		@Override
		public void analysis(IStochObserver obs, IVector obsValues, IVector pred, IStochModelInstance main, ITime t) {
			Step s = new Step();
			s.time = t.getMJD();
			s.forecast = states(this.ensemble);
			s.obs = obs.getExpectations().getValues();
			s.obsStd = obs.getStandardDeviations().getValues();
			super.analysis(obs, obsValues, pred, main, t);
			s.analysis = states(this.ensemble);
			s.mainAfter = main.getState().getValues();
			steps.add(s);
		}
	}

	public static class CapturingSteady extends SteadyStateFilter {
		public final List<Step> steps = new ArrayList<>();
		@Override
		public void analysis(IStochObserver obs, IVector obsValues, IVector pred, IStochModelInstance main, ITime t) {
			Step s = new Step();
			s.time = t.getMJD();
			s.forecast = new double[][]{ main.getState().getValues() };
			s.obs = obs.getExpectations().getValues();
			s.obsStd = obs.getStandardDeviations().getValues();
			super.analysis(obs, obsValues, pred, main, t);
			s.analysis = new double[][]{ main.getState().getValues() };
			s.mainAfter = main.getState().getValues();
			steps.add(s);
		}
	}

	/** Oscillator whose Hamill localisation mask is a fixed, non-trivial rho (the stock one is all ones). */
	public static class LocalisedOscillatorFactory extends OscillatorStochModelFactory {
		static double[] RHO = {1.0, 0.4};
		@Override
		public IStochModelInstance getInstance(OutputLevel outputLevel) {
			OscillatorStochModelInstance r = new OscillatorStochModelInstance(this.workingDir, this.arguments[0]) {
				@Override
				public IVector[] getObservedLocalization(IObservationDescriptions d, double distance) {
					IVector[] out = new IVector[d.getObservationCount()];
					for (int i = 0; i < out.length; i++) out[i] = new Vector(RHO);
					return out;
				}
			};
			r.outputLevel = outputLevel;
			return r;
		}
	}

	static void stepsJson(J j, List<Step> steps, boolean gains) {
		j.raw("[");
		for (int i = 0; i < steps.size(); i++) {
			Step s = steps.get(i);
			if (i > 0) j.raw(",");
			j.open();
			j.key("time").num(s.time);
			j.key("obs").arr(s.obs);
			j.key("obs_std").arr(s.obsStd);
			j.key("forecast").mat(s.forecast);
			j.key("analysis").mat(s.analysis);
			j.key("main_after").arr(s.mainAfter);
			if (gains && s.gainPre != null) j.key("gain_pre").mat(s.gainPre);
			if (gains && s.gainPost != null) j.key("gain_post").mat(s.gainPost);
			j.close();
		}
		j.raw("]");
	}

	// Oscillator config of OpenDA's KalmanFilterTest (testEnKF / testEnSR / testParticleFilter).
	static final String OSC_CONFIG = "<oscillatorConfig><simulationTimespan>[0.0,0.05,10.0]</simulationTimespan><parameters names=\"t_damp,omega\">[8.0,1.5708]</parameters><parameterUncertainty names=\"t_damp,omega\">[1.0,0.1257]</parameterUncertainty><systemNoise>{[0.0,0.0],[0.3,0.3]}</systemNoise><initialState>[0.0,0.0]</initialState><initialStateUncertainty>[0.8,0.8]</initialStateUncertainty></oscillatorConfig>";
	static final long INIT_SEED = 1234567890L; // KalmanFilterTest.initSeed

	/** OpenDA KalmanFilterTest.generateObservations, in memory (no CSV round trip). */
	static CsvStochObserver oscillatorObservations(double[][] colsOut) {
		IStochModelFactory fact = new OscillatorStochModelFactory();
		fact.initialize(null, new String[]{""});
		IStochModelInstance model = fact.getInstance(IStochModelFactory.OutputLevel.Suppress);
		model.setAutomaticNoiseGeneration(false);
		IVector time = Vector.range(0.0, 10.0, 1.0);
		int n = time.getSize();
		IVector values = new Vector(n);
		IVector stdVal = new Vector(n); stdVal.setConstant(0.1);
		IVector indVal = new Vector(n); indVal.setConstant(0.0);
		IVector[] columns = {time, indVal, values, stdVal};
		String[] keys = {"time", "index", "value", "std"};
		IStochObserver obs = new CsvStochObserver(columns, keys);
		IObservationDescriptions descr = obs.getObservationDescriptions();
		model.announceObservedValues(descr);
		model.compute(model.getTimeHorizon().getEndTime());
		IVector prd = model.getObservationOperator().getObservedValues(descr);
		columns[2] = prd;
		colsOut[0] = time.getValues(); colsOut[1] = indVal.getValues(); colsOut[2] = prd.getValues(); colsOut[3] = stdVal.getValues();
		return new CsvStochObserver(columns, keys);
	}

	static RecordingRandom installStochVectorRecorder(long seed) throws Exception {
		RecordingRandom r = new RecordingRandom(seed);
		setStatic(StochVector.class, "generator", r);
		return r;
	}

	// ------------------------------------------------------------------
	// Sequential cases
	// ------------------------------------------------------------------
	static void sequential(File out) throws Exception {
		double[][] cols = new double[4][];
		J j = new J().open();
		j.key("openda_commit").str("e51565c2c8def7d35f8b378a9ac118edbd98a94d");
		j.key("oscillator_config").str(OSC_CONFIG.replace("\"", "\\\""));
		CsvStochObserver obs = oscillatorObservations(cols);
		j.key("oscillator_obs").open().key("time").arr(cols[0]).key("index").arr(cols[1]).key("value").arr(cols[2]).key("std").arr(cols[3]).close();

		String[][] enkfCases = {
			{"enkf_none", "<algorithm><ensembleSize>10</ensembleSize><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"},
			{"enkf_hamill", "<algorithm><ensembleSize>10</ensembleSize><localization>hamill</localization><distance>1.0</distance><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"},
			{"enkf_zhang", "<algorithm><ensembleSize>10</ensembleSize><localization>autoZhang</localization><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"},
			{"enkf_stochpar", "<algorithm><ensembleSize>10</ensembleSize><ensembleModel stochParameter=\"true\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"},
		};
		for (String[] c : enkfCases) {
			RecordingRandom rec = installStochVectorRecorder(INIT_SEED);
			RecordingRandom zrec = new RecordingRandom(20100816L);
			setStatic(AutoLocalizationZhang2011.class, "generator", zrec);
			IStochModelFactory factory = c[0].equals("enkf_hamill") ? new LocalisedOscillatorFactory() : new OscillatorStochModelFactory();
			factory.initialize(null, new String[]{OSC_CONFIG});
			CapturingEnKF a = new CapturingEnKF();
			a.initialize(null, new String[]{c[1]});
			a.setStochComponents(obs, factory);
			a.prepare();
			a.run();
			j.key(c[0]).open();
			j.key("config").str(c[1].replace("\"", "\\\""));
			j.key("final_state").arr(a.getCurrentState().getValues());
			j.key("gaussians").arr(rec.gaussians);
			j.key("bootstrap_ints").iarr(zrec.ints);
			j.key("steps"); stepsJson(j, a.steps, true);
			j.close();
		}

		{ // EnSR
			RecordingRandom rec = installStochVectorRecorder(INIT_SEED);
			IStochModelFactory factory = new OscillatorStochModelFactory();
			factory.initialize(null, new String[]{OSC_CONFIG});
			CapturingEnSR a = new CapturingEnSR();
			String cfg = "<algorithm><ensembleSize>10</ensembleSize></algorithm>";
			a.initialize(null, new String[]{cfg});
			a.setStochComponents(obs, factory);
			a.prepare();
			a.run();
			j.key("ensr").open().key("config").str(cfg).key("final_state").arr(a.getCurrentState().getValues());
			j.key("gaussians").arr(rec.gaussians).key("steps"); stepsJson(j, a.steps, false); j.close();
		}

		{ // Particle filter (residual resampling)
			RecordingRandom rec = installStochVectorRecorder(INIT_SEED);
			IStochModelFactory factory = new OscillatorStochModelFactory();
			factory.initialize(null, new String[]{OSC_CONFIG});
			CapturingPF a = new CapturingPF();
			String cfg = "<algorithm><ensembleSize>20</ensembleSize></algorithm>";
			a.initialize(null, new String[]{cfg});
			RecordingRandom urec = new RecordingRandom(1234567890L); // ParticleFilter.fixedSeed
			setField(a, ParticleFilter.class, "randomGenerator", urec);
			a.setStochComponents(obs, factory);
			a.prepare();
			a.run();
			j.key("particle_filter").open().key("config").str(cfg).key("final_state").arr(a.getCurrentState().getValues());
			j.key("gaussians").arr(rec.gaussians).key("uniforms").arr(urec.doubles).key("steps"); stepsJson(j, a.steps, false); j.close();
		}

		{ // Steady-state filter: gain saved by an EnKF at t = 5, then applied with no ensemble
			File wd = new File(out, "steady_tmp");
			wd.mkdirs();
			installStochVectorRecorder(INIT_SEED);
			IStochModelFactory factory = new OscillatorStochModelFactory();
			factory.initialize(wd, new String[]{OSC_CONFIG});
			EnKF e = new EnKF();
			e.initialize(wd, new String[]{"<algorithm><ensembleSize>10</ensembleSize><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /><saveGain><times type=\"fixed\" timeFormat=\"mjd\">5.0</times><file dirPrefix=\"kg_\" fileName=\"kalmanGainStorage.xml\" /></saveGain></algorithm>"});
			e.setStochComponents(obs, factory);
			e.prepare();
			e.run();
			installStochVectorRecorder(INIT_SEED);
			IStochModelFactory factory2 = new OscillatorStochModelFactory();
			factory2.initialize(wd, new String[]{OSC_CONFIG});
			CapturingSteady s = new CapturingSteady();
			s.initialize(wd, new String[]{"<algorithm><readGain><dirPrefix>kg_</dirPrefix><time timeFormat=\"mjd\">5.0</time><file>kalmanGainStorage.xml</file></readGain></algorithm>"});
			s.setStochComponents(obs, factory2);
			s.prepare();
			@SuppressWarnings("unchecked")
			java.util.HashMap<String, IVector> gv = (java.util.HashMap<String, IVector>) getField(s, AbstractSequentialAlgorithm.class, "gainVectors");
			s.run();
			j.key("steady_state").open().key("final_state").arr(s.getCurrentState().getValues());
			j.key("gain_ids").raw("[");
			int q = 0;
			for (String k : gv.keySet()) { if (q++ > 0) j.raw(","); j.str(k); }
			j.raw("]").key("gain_columns").raw("[");
			q = 0;
			for (String k : gv.keySet()) { if (q++ > 0) j.raw(","); j.arr(gv.get(k).getValues()); }
			j.raw("]").key("steps"); stepsJson(j, s.steps, false); j.close();
		}

		{ // Lorenz-63 EnKF: noise-free truth, x, y, z observed every 0.5 time units
			IStochModelFactory tf = new LorenzStochModelFactory();
			String lcfg = "<lorenzConfig><simulationTimespan>[0.0,0.025,5.0]</simulationTimespan><systemNoise>{[0.0,0.0,0.0],[0.1,0.1,0.1]}</systemNoise><initialState>[1.508870,-1.531271,25.46091]</initialState><initialStateUncertainty>[0.5,0.5,0.5]</initialStateUncertainty></lorenzConfig>";
			tf.initialize(null, new String[]{lcfg});
			IStochModelInstance truth = tf.getInstance(IStochModelFactory.OutputLevel.Suppress);
			truth.setAutomaticNoiseGeneration(false);
			int nt = 10, no = 3 * nt;
			IVector time = new Vector(no), idx = new Vector(no), val = new Vector(no), std = new Vector(no);
			for (int k = 0; k < nt; k++) for (int c = 0; c < 3; c++) {
				time.setValue(3 * k + c, 0.5 * (k + 1)); idx.setValue(3 * k + c, c); std.setValue(3 * k + c, 1.0);
			}
			IVector[] columns = {time, idx, val, std};
			String[] keys = {"time", "index", "value", "std"};
			IStochObserver o0 = new CsvStochObserver(columns, keys);
			truth.announceObservedValues(o0.getObservationDescriptions());
			truth.compute(truth.getTimeHorizon().getEndTime());
			columns[2] = truth.getObservationOperator().getObservedValues(o0.getObservationDescriptions());
			CsvStochObserver lobs = new CsvStochObserver(columns, keys);
			RecordingRandom rec = installStochVectorRecorder(INIT_SEED);
			IStochModelFactory lf = new LorenzStochModelFactory();
			lf.initialize(null, new String[]{lcfg});
			CapturingEnKF a = new CapturingEnKF();
			String cfg = "<algorithm><ensembleSize>10</ensembleSize><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>";
			a.initialize(null, new String[]{cfg});
			a.setStochComponents(lobs, lf);
			a.prepare();
			a.run();
			j.key("lorenz_enkf").open().key("config").str(lcfg.replace("\"", "\\\"")).key("final_state").arr(a.getCurrentState().getValues());
			j.key("obs").open().key("time").arr(time.getValues()).key("index").arr(idx.getValues()).key("value").arr(columns[2].getValues()).key("std").arr(std.getValues()).close();
			j.key("truth_final").arr(truth.getState().getValues());
			j.key("gaussians").arr(rec.gaussians).key("steps"); stepsJson(j, a.steps, true); j.close();
		}

		{ // Statistical comparison, upstream side: 200 replicates each of EnKF, EnSR (N 10) and the PF (N 20)
		  // on the oscillator, Java's own streams at seeds 1..200; final analysis state of each replicate.
			int reps = 200;
			String[] names = {"enkf", "ensr", "particle_filter"};
			j.key("replicates").open().key("seeds").str("1..200");
			for (String nm : names) {
				double[][] fin = new double[reps][];
				for (int r = 0; r < reps; r++) {
					installStochVectorRecorder(r + 1);
					IStochModelFactory factory = new OscillatorStochModelFactory();
					factory.initialize(null, new String[]{OSC_CONFIG});
					AbstractSequentialEnsembleAlgorithm a;
					if (nm.equals("enkf")) {
						a = new EnKF();
						a.initialize(null, new String[]{"<algorithm><ensembleSize>10</ensembleSize><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"});
					} else if (nm.equals("ensr")) {
						a = new EnSR();
						a.initialize(null, new String[]{"<algorithm><ensembleSize>10</ensembleSize></algorithm>"});
					} else {
						a = new ParticleFilter();
						a.initialize(null, new String[]{"<algorithm><ensembleSize>20</ensembleSize></algorithm>"});
						setField(a, ParticleFilter.class, "randomGenerator", new Random(1000 + r));
					}
					a.setStochComponents(obs, factory);
					a.prepare();
					a.run();
					fin[r] = a.getCurrentState().getValues();
				}
				j.key(nm).mat(fin);
			}
			j.close();
		}

		{ // Upstream's own assertion, reproduced (N = 100, the CSV round trip of the test): provenance check only.
			File wd = new File(out, "csv_tmp");
			wd.mkdirs();
			oscillatorObservations(new double[4][]).toFile(wd, "observations_oscillator_generated.csv");
			installStochVectorRecorder(INIT_SEED);
			IStochModelFactory factory = new OscillatorStochModelFactory();
			factory.initialize(null, new String[]{OSC_CONFIG});
			IStochObserver og = new CsvStochObserver();
			og.initialize(wd, new String[]{"observations_oscillator_generated.csv"});
			EnKF e = new EnKF();
			e.initialize(null, new String[]{"<algorithm><ensembleModel stochParameter=\"false\" stochForcing=\"true\" stochInit=\"true\" /></algorithm>"});
			e.setStochComponents(og, factory);
			e.prepare();
			e.run();
			j.key("upstream_test_enkf_n100").open().key("final_state").arr(e.getCurrentState().getValues())
				.key("upstream_asserted").arr(new double[]{-0.2315739613743379, 0.028205645374715847}).close();
		}
		j.close();
		write(out, "openda_sequential.json", j.sb.toString());
	}

	// ------------------------------------------------------------------
	// Calibration cases
	// ------------------------------------------------------------------
	@SuppressWarnings("unchecked")
	static void history(J j, Object f) throws Exception {
		Class<?> c = f.getClass();
		java.util.Vector<IVector> pars = (java.util.Vector<IVector>) getField(f, c, "allPars");
		java.util.Vector<Double> costs = (java.util.Vector<Double>) getField(f, c, "allCosts");
		IVector[] pv = pars.toArray(new IVector[0]);
		j.key("evaluated_parameters").vecs(pv);
		j.key("evaluated_costs").arr(costs);
	}

	static void calibration(File out) throws Exception {
		J j = new J().open();
		j.key("openda_commit").str("e51565c2c8def7d35f8b378a9ac118edbd98a94d");

		{ SimpleCostFunction f = new SimpleCostFunction();
		  SimplexCoreOptimizer s = new SimplexCoreOptimizer(f);
		  s.initialize(new Vector("[0.0,0.0]"), 1.5);
		  setField(s, SimplexCoreOptimizer.class, "absTolSimplex", 0.001);
		  setField(s, SimplexCoreOptimizer.class, "relTolSimplex", 0.00001);
		  s.optimize();
		  j.key("simplex_simple").open().key("optimal").arr(s.getOptimalValue().getValues()).key("optimal_cost").num(s.getOptimalCost());
		  history(j, f); j.close(); }

		{ RosenbrockCostFunction f = new RosenbrockCostFunction();
		  SimplexCoreOptimizer s = new SimplexCoreOptimizer(f);
		  s.initialize(new Vector("[0.0,0.0]"), 1.5);
		  setField(s, SimplexCoreOptimizer.class, "absTolSimplex", 0.001);
		  setField(s, SimplexCoreOptimizer.class, "relTolSimplex", 0.0001);
		  s.optimize();
		  j.key("simplex_rosenbrock").open().key("optimal").arr(s.getOptimalValue().getValues()).key("optimal_cost").num(s.getOptimalCost());
		  history(j, f); j.close(); }

		{ SimpleCostFunction f = new SimpleCostFunction();
		  PowellCoreOptimizer p = new PowellCoreOptimizer(f);
		  p.initialize(new Vector("[0.1,0.1]"), 0.5);
		  p.absTolPowell = 0.001; p.relTolPowell = 0.0001;
		  p.optimize();
		  j.key("powell_simple").open().key("optimal").arr(p.getOptimalValue().getValues()).key("optimal_cost").num(p.getOptimalCost());
		  history(j, f); j.close(); }

		{ RosenbrockCostFunction f = new RosenbrockCostFunction();
		  PowellCoreOptimizer p = new PowellCoreOptimizer(f);
		  p.initialize(new Vector("[0.0,0.0]"), 0.5);
		  p.absTolPowell = 0.001; p.relTolPowell = 0.0001;
		  p.optimize();
		  j.key("powell_rosenbrock").open().key("optimal").arr(p.getOptimalValue().getValues()).key("optimal_cost").num(p.getOptimalCost());
		  history(j, f); j.close(); }

		{ SimpleCostFunction f = new SimpleCostFunction();
		  PowellCoreOptimizer p = new PowellCoreOptimizer(f);
		  p.initialize(new Vector("[0.0,0.0]"), new IVector[]{new Vector("[2.0,0.0]"), new Vector("[0.0,1.0]")});
		  p.optimize();
		  j.key("powell_simple_searchinit").open().key("optimal").arr(p.getOptimalValue().getValues()).key("optimal_cost").num(p.getOptimalCost());
		  history(j, f); j.close(); }

		for (boolean bg : new boolean[]{false, true}) {
		  SimpleLeastSquaresCostFunction f = new SimpleLeastSquaresCostFunction();
		  f.setBackgroundTerm(bg);
		  DudCoreOptimizer d = new DudCoreOptimizer(f);
		  d.initialize(new Vector("[0.0,0.0]"));
		  d.absTol = 0.001; d.relTol = 0.001;
		  d.optimize();
		  j.key(bg ? "dud_simple_ls_background" : "dud_simple_ls").open().key("optimal").arr(d.getOptimalValue().getValues()).key("optimal_cost").num(d.getOptimalCost());
		  j.key("std").arr(d.getStdValues());
		  history(j, f); j.close(); }

		{ SimpleLeastSquaresCostFunction f = new SimpleLeastSquaresCostFunction();
		  SimplexCoreOptimizer s = new SimplexCoreOptimizer(f);
		  s.initialize(new Vector("[0.0,0.0]"), 1.0);
		  s.optimize();
		  j.key("simplex_simple_ls").open().key("optimal").arr(s.getOptimalValue().getValues()).key("optimal_cost").num(s.getOptimalCost());
		  j.key("std").arr(s.getStdValues());
		  history(j, f); j.close(); }

		// Oscillator calibration of (t_damp, omega) with the weighted cost function on OpenDA's own
		// fixture oscillatorCalibrationObservations.csv (copied verbatim beside this driver).
		File obsDir = new File(System.getProperty("gh1192.fixtures"));
		String[] algs = {"dud", "simplex", "powell"};
		for (String alg : algs) {
			for (boolean bg : new boolean[]{false, true}) {
				IStochModelFactory fac = new OscillatorStochModelFactory();
				fac.initialize(null, new String[]{""});
				CsvStochObserver o = new CsvStochObserver();
				o.initialize(obsDir, new String[]{"oscillatorCalibrationObservations.csv"});
				SimulationKwadraticCostFunction f = new SimulationKwadraticCostFunction(fac, o);
				f.addBackgroundTerm = bg;
				IVector p0 = new Vector("[8.0,1.5708]");
				String key = "oscillator_" + alg + (bg ? "_background" : "");
				j.key(key).open();
				if (alg.equals("dud")) {
					DudCoreOptimizer d = new DudCoreOptimizer(f);
					d.initialize(p0);
					d.maxit = 10;
					d.optimize();
					j.key("optimal").arr(d.getOptimalValue().getValues()).key("optimal_cost").num(d.getOptimalCost());
					j.key("std").arr(d.getStdValues());
				} else if (alg.equals("simplex")) {
					SimplexCoreOptimizer s = new SimplexCoreOptimizer(f);
					s.initialize(p0, 0.5);
					s.optimize();
					j.key("optimal").arr(s.getOptimalValue().getValues()).key("optimal_cost").num(s.getOptimalCost());
				} else {
					PowellCoreOptimizer p = new PowellCoreOptimizer(f);
					p.initialize(p0, 0.5);
					p.optimize();
					j.key("optimal").arr(p.getOptimalValue().getValues()).key("optimal_cost").num(p.getOptimalCost());
				}
				history(j, f);
				@SuppressWarnings("unchecked")
				java.util.Vector<IVector> preds = (java.util.Vector<IVector>) getField(f, SimulationKwadraticCostFunction.class, "allPreds");
				j.key("evaluated_predictions").vecs(preds.toArray(new IVector[0]));
				j.close();
			}
		}
		j.close();
		write(out, "openda_calibration.json", j.sb.toString());
	}

	// ------------------------------------------------------------------
	// Linear algebra: OpenDA Matrix (and through it JAMA's SVD) on fixed
	// matrices; plus the two java.lang.Math facts the port relies on.
	// ------------------------------------------------------------------
	static void linalg(File out) {
		J j = new J().open();
		double[][][] mats = {
			{{4.0, 1.0}, {1.0, 3.0}},
			{{2.0, -1.0, 0.0}, {-1.0, 2.0, -1.0}, {0.0, -1.0, 2.0}},
			{{1.0, 2.0, 3.0}, {4.0, 5.0, 6.0}, {7.0, 8.0, 10.0}},
			{{0.3, 0.1, 0.0, 0.2}, {0.1, 0.9, 0.4, 0.0}, {0.0, 0.4, 1.7, 0.6}, {0.2, 0.0, 0.6, 2.2}},
			{{1.0, 2.0}, {3.0, 4.0}, {5.0, 6.5}},
			{{1e-3, 2.0, 0.5}, {3.0, 1e-9, 4.0}, {0.25, 7.0, 1.0}, {2.0, 2.0, 2.0}, {9.0, -1.0, 0.0}},
		};
		j.key("cases").raw("[");
		for (int c = 0; c < mats.length; c++) {
			if (c > 0) j.raw(",");
			Matrix a = new Matrix(mats[c]);
			Matrix[] svd = a.svd();
			Jama.SingularValueDecomposition js = new Jama.Matrix(mats[c]).svd();
			j.open().key("a").mat(mats[c]);
			j.key("jama_u").mat(js.getU().getArray()).key("jama_s").arr(js.getSingularValues()).key("jama_v").mat(js.getV().getArray());
			j.key("oda_svd_u").mat(svd[0].asArray()).key("oda_svd_v").mat(svd[2].asArray());
			if (mats[c].length == mats[c][0].length) {
				j.key("oda_inverse").mat(a.inverse().asArray());
				j.key("oda_sqrt").mat(a.sqrt().asArray());
			}
			Vector rhs = new Vector(mats[c].length);
			for (int i = 0; i < mats[c].length; i++) rhs.setValue(i, 0.5 + i * 0.75);
			Vector x = new Vector(mats[c][0].length);
			a.rightSolve(rhs, x);
			j.key("rhs").arr(rhs.getValues()).key("oda_right_solve").arr(x.getValues());
			Matrix p = new Matrix(mats[c].length, mats[c].length);
			p.multiply(1.0, a, a, 0.0, false, true);
			j.key("oda_a_at").mat(p.asArray());
			j.close();
		}
		j.raw("]");
		// Math.pow(x, 2.0) == x*x and Math.exp == StrictMath.exp on a fixed sample (both used by the port).
		Random r = new Random(7);
		int powDiff = 0, expDiff = 0, n = 200000;
		for (int i = 0; i < n; i++) {
			double x = (r.nextDouble() - 0.5) * Math.pow(10.0, r.nextInt(20) - 10);
			if (Double.doubleToRawLongBits(Math.pow(x, 2.0)) != Double.doubleToRawLongBits(x * x)) powDiff++;
			double y = -30.0 * r.nextDouble();
			if (Double.doubleToRawLongBits(Math.exp(y)) != Double.doubleToRawLongBits(StrictMath.exp(y))) expDiff++;
		}
		j.key("pow2_vs_square_differences").num(powDiff).key("exp_vs_strictexp_differences").num(expDiff).key("samples").num(n);
		// Math.exp on the PF's arguments' range, written so the port can check its own exp against it.
		double[] xs = new double[2000]; double[] ex = new double[2000];
		for (int i = 0; i < xs.length; i++) { xs[i] = -0.5 * (r.nextDouble() * 40.0); ex[i] = Math.exp(xs[i]); }
		j.key("exp_args").arr(xs).key("exp_values").arr(ex);
		j.close();
		try { write(out, "openda_linalg.json", j.sb.toString()); } catch (IOException e) { throw new RuntimeException(e); }
	}

	public static void main(String[] args) throws Exception {
		File out = new File(args[0]);
		out.mkdirs();
		linalg(out);
		calibration(out);
		sequential(out);
		System.out.println("java.version=" + System.getProperty("java.version") + " vm=" + System.getProperty("java.vm.name"));
	}
}
