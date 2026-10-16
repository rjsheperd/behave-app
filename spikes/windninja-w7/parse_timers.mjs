// Parse WindNinja's phase-timer output into an Amdahl table.
//
//   bun run parse_timers.mjs run-*.log
//
// WindNinja prints these lines (ninja.cpp:578-595), but ONLY when built with
// OpenMP — they sit inside `#ifdef _OPENMP`:
//
//   Meshing time was %lf seconds.
//   Initialization time was %lf seconds.
//   Equation building time was %lf seconds.
//   Solver time was %lf seconds.
//   Output writing time was %lf seconds.
//   Total simulation time was %lf seconds.
//
// Plus, if accumulate-timers.patch is applied:
//   W7 cumulative equation building time was %lf seconds.
//   W7 cumulative solver time was %lf seconds over %d solves.
//   W7 matching iterations = %d

const PHASES = [
  ['mesh', /Meshing time was ([\d.eE+-]+) seconds/],
  ['init', /Initialization time was ([\d.eE+-]+) seconds/],
  ['buildEq', /Equation building time was ([\d.eE+-]+) seconds/],
  ['solve', /Solver time was ([\d.eE+-]+) seconds/],
  ['writeOut', /Output writing time was ([\d.eE+-]+) seconds/],
  ['total', /Total simulation time was ([\d.eE+-]+) seconds/],
  ['cumBuildEq', /W7 cumulative equation building time was ([\d.eE+-]+) seconds/],
  ['cumSolve', /W7 cumulative solver time was ([\d.eE+-]+) seconds over (\d+) solves/],
  ['matchIters', /W7 matching iterations = (\d+)/],
];

export function parseLog(text) {
  const out = {};
  for (const [key, re] of PHASES) {
    const m = text.match(re);
    if (m) {
      out[key] = Number(m[1]);
      if (key === 'cumSolve' && m[2]) out.solveCount = Number(m[2]);
    }
  }
  // Mesh size, if the run echoed it.
  const nodes = text.match(/NUMNP\s*=?\s*(\d+)/i);
  if (nodes) out.numnp = Number(nodes[1]);
  const res = text.match(/mesh(?:\s+|_)?resolution[^\d]*([\d.]+)/i);
  if (res) out.meshRes = Number(res[1]);
  return out;
}

function pct(a, b) { return b > 0 ? ((a / b) * 100).toFixed(1) + '%' : '—'; }

const files = process.argv.slice(2);
if (!files.length) {
  console.error('usage: bun run parse_timers.mjs <run.log> [...]');
  process.exit(1);
}

const { readFileSync } = await import('node:fs');
const rows = [];
for (const f of files) {
  const r = parseLog(readFileSync(f, 'utf8'));
  if (r.total === undefined) {
    console.error(`WARNING ${f}: no timer lines found. ` +
      `Was it built with OpenMP? (timers are inside #ifdef _OPENMP)`);
    continue;
  }
  rows.push({ file: f.split('/').pop(), ...r });
}
if (!rows.length) process.exit(1);

const hasCum = rows.some((r) => r.cumSolve !== undefined);

console.log('\nW7 — phase breakdown (seconds, and share of total)\n');
const hdr = ['run', 'NUMNP', 'mesh', 'init', 'buildEq', 'solve', 'writeOut', 'TOTAL', 'solve %'];
if (hasCum) hdr.push('cumSolve', 'solves', 'cum solve %');
console.log(hdr.map((h) => h.padStart(11)).join(''));

for (const r of rows) {
  const cells = [
    r.file.slice(0, 11),
    r.numnp ?? '—',
    (r.mesh ?? 0).toFixed(2),
    (r.init ?? 0).toFixed(2),
    (r.buildEq ?? 0).toFixed(2),
    (r.solve ?? 0).toFixed(2),
    (r.writeOut ?? 0).toFixed(2),
    r.total.toFixed(2),
    pct(r.solve ?? 0, r.total),
  ];
  if (hasCum) {
    cells.push((r.cumSolve ?? 0).toFixed(2), r.solveCount ?? '—',
               pct(r.cumSolve ?? 0, r.total));
  }
  console.log(cells.map((c) => String(c).padStart(11)).join(''));
}

console.log('\nInterpretation guide:');
console.log('  solve % < ~30%        -> the solve is NOT dominant; close Gate 4,');
console.log('                           do not pursue WebGPU (spike plan W7 decision).');
console.log('  solve % 30-60%        -> GPU could help but Amdahl caps the win;');
console.log('                           weigh against W9 iteration inflation.');
console.log('  solve % > ~60%        -> solve dominates; Gate 4 is worth opening.');
if (hasCum) {
  const bad = rows.filter((r) => (r.solveCount ?? 1) > 1);
  if (bad.length) {
    console.log('\nNOTE: runs with >1 solve had their per-iteration "Solver time"');
    console.log('      overwritten by the matching loop. Use the cumSolve column;');
    console.log('      the plain "solve" column understates those runs.');
  }
} else {
  console.log('\nNOTE: accumulate-timers.patch is NOT applied. If any run used');
  console.log('      point initialisation with station matching, its reported');
  console.log('      solve time covers only the LAST outer iteration and is');
  console.log('      an undercount. Apply the patch before trusting those rows.');
}
