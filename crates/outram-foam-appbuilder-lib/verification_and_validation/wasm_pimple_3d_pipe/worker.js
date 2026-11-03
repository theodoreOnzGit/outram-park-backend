// Web Worker: instantiates the pipe wasm with NO imports, runs one case and
// posts its timings back. The page (run_worker.mjs serves it) starts one
// Worker per case, so each case's memory figure is its own peak.
self.onmessage = async (ev) => {
  const c = ev.data;
  const bytes = await (await fetch(c.wasm)).arrayBuffer();
  const module = await WebAssembly.compile(bytes);
  const imports = WebAssembly.Module.imports(module);
  const { exports: ex } = await WebAssembly.instantiate(module, {});
  const t = () => performance.now() / 1000;
  const out = { name: c.name, imports, wasm_bytes: bytes.byteLength };
  let t0 = t();
  out.cells = c.route === 'ogrid'
    ? ex.pipe_mesh_ogrid(c.a, c.b, c.c)
    : ex.pipe_mesh_cfmesh(c.a, c.b, c.c);
  out.mesh_s = t() - t0;
  t0 = t();
  ex.pipe_to_fv();
  out.to_fv_s = t() - t0;
  out.non_ortho_max = ex.pipe_quality(0);
  out.neg = ex.pipe_quality(2);
  ex.pipe_case_new(c.dt);
  const hu = [], hp = [];
  let total = 0, best = Infinity, k = 0, r = Infinity;
  while (k < c.max_steps && !(r < 1e-3)) {
    t0 = t();
    r = ex.pipe_steps(1);
    const w = t() - t0;
    total += w; best = Math.min(best, w); k++;
    hu.push([k * c.dt, ex.pipe_diag(0)]);
    hp.push([k * c.dt, ex.pipe_diag(5)]);
  }
  // Same definition as foam_wasm_pipe::settling_time.
  const settle = (xs) => {
    const last = xs[xs.length - 1][1];
    for (let i = xs.length - 1; i >= 0; i--) if (Math.abs(xs[i][1] - last) > 0.01 * Math.abs(last)) return xs[i][0];
    return 0;
  };
  Object.assign(out, {
    steps: k, t_phys_s: k * c.dt, solve_wall_s: total, best_step_s: best, mean_step_s: total / k,
    settle1pct_t_s: Math.max(settle(hu), settle(hp)),
    uc15: ex.pipe_diag(0), uc17: ex.pipe_diag(4), dpdz_over_hp: ex.pipe_diag(1),
    qout_over_qin: ex.pipe_diag(2), dp_total: ex.pipe_diag(5),
    memory_bytes: ex.memory.buffer.byteLength,
  });
  self.postMessage(out);
};
