// Plan C spike — measure the cost of loading FlamMap landscape data.
//
//   bun run bench.mjs <file.tif|file.lcp> [...]
//
// Reports per-stage timings, then projects to larger grids. Projection is
// linear in cell count, which holds for deinterleave and pack (both are
// single-pass per cell) but understates large-grid cache effects — see the
// caveat printed at the end.

import { readFileSync, statSync } from 'node:fs';
import { gzipSync, deflateSync } from 'node:zlib';
import { load, deinterleave, packCells, packCellsDirect } from './lcp.mjs';

const REPS = 7; // median of 7 to damp JIT warmup and GC noise

function time(label, fn) {
  const samples = [];
  let result;
  for (let i = 0; i < REPS; i++) {
    const t0 = performance.now();
    result = fn();
    samples.push(performance.now() - t0);
  }
  samples.sort((a, b) => a - b);
  return { label, ms: samples[Math.floor(REPS / 2)], min: samples[0], result };
}

function fmt(n, w = 9) { return n.toFixed(2).padStart(w); }

function bench(path) {
  const bytes = statSync(path).size;
  const buf = readFileSync(path);
  const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);

  const tRead = time('read bytes from disk', () => {
    const b = readFileSync(path);
    return b.byteLength;
  });

  // NB: pass `ab` directly, never `ab.slice(0)` — slicing copies the whole
  // file (320 MB at 4096²) inside the timed loop and inflates this stage by
  // several hundred ms. `load` does not mutate its input.
  const tParse = time('parse TIFF + assemble interleaved', () => load(ab));
  const img = tParse.result;
  const cells = img.width * img.height;

  const tDeint = time('deinterleave to band planes', () => deinterleave(img));
  const planes = tDeint.result;

  const tPack = time('pack to 8 B/cell GPU layout',
    () => packCells(planes, img.width, img.height));

  // Fused path: skip the band planes entirely.
  const tFused = time('FUSED deinterleave+pack (direct)',
    () => packCellsDirect(img, img.width, img.height));

  // Equivalence check — the fused path must produce identical bytes.
  const a = tPack.result.packed, b = tFused.result.packed;
  let mismatch = -1;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) { mismatch = i; break; }

  // What would compression buy on the wire? The brief assumes ~5:1 ZSTD.
  const tDeflate = time('deflate (level 6) the packed cells',
    () => deflateSync(Buffer.from(tPack.result.packed.buffer)).byteLength);
  const packedBytes = tPack.result.packed.byteLength;
  const deflatedBytes = tDeflate.result;

  console.log(`\n${'='.repeat(78)}`);
  console.log(`${path.split('/').pop()}`);
  console.log('='.repeat(78));
  console.log(`  grid           ${img.width} x ${img.height} = ${cells.toLocaleString()} cells`);
  console.log(`  bands          ${img.numBands} x int16`);
  console.log(`  file           ${(bytes / 1048576).toFixed(2)} MB  (${(bytes / cells).toFixed(1)} B/cell on disk)`);
  if (img.meta) {
    const m = img.meta;
    console.log(`  layout         ${m.tiled ? 'TILED' : 'STRIPPED'}` +
                `${m.rowsPerStrip ? ` rowsPerStrip=${m.rowsPerStrip}` : ''}` +
                `  chunks=${m.stripOrTileCount}  compression=${m.compression === 1 ? 'none' : m.compression}`);
    console.log(`  geo            ${m.cellSize} m cells, origin (${m.originX}, ${m.originY})`);
  }
  if (img.header) {
    const h = img.header;
    console.log(`  lcp header     crownFuels=${h.crownFuels} groundFuels=${h.groundFuels} ` +
                `bands=${h.numBands} res=${h.xResol}x${h.yResol}`);
  }

  console.log(`\n  stage                                    median      min   throughput`);
  for (const t of [tRead, tParse, tDeint, tPack, tFused]) {
    console.log(`  ${t.label.padEnd(38)} ${fmt(t.ms)} ${fmt(t.min, 8)}` +
                `   ${(cells / t.ms / 1000).toFixed(1)}M cells/s`);
  }
  const total = tParse.ms + tDeint.ms + tPack.ms;
  const fused = tParse.ms + tFused.ms;
  console.log(`  ${'TOTAL two-pass (parse+deint+pack)'.padEnd(38)} ${fmt(total)}`);
  console.log(`  ${'TOTAL fused  (parse+packDirect)'.padEnd(38)} ${fmt(fused)}` +
              `   ${(total / fused).toFixed(2)}x faster` +
              `   [${mismatch < 0 ? 'byte-identical' : `MISMATCH @${mismatch}`}]`);
  console.log(`\n  packed size    ${(packedBytes / 1048576).toFixed(2)} MB (8 B/cell)` +
              `   vs ${(bytes / 1048576).toFixed(2)} MB on disk` +
              `   = ${(bytes / packedBytes).toFixed(2)}x shrink`);
  console.log(`  deflated       ${(deflatedBytes / 1048576).toFixed(2)} MB` +
              `   = ${(packedBytes / deflatedBytes).toFixed(2)}:1 on packed cells` +
              `   (deflate ${fmt(tDeflate.ms, 1)} ms)`);

  return { cells, total, packedBytes, deflatedBytes, bytes,
           perCellMs: fused / cells };
}

const files = process.argv.slice(2);
if (!files.length) {
  console.error('usage: bun run bench.mjs <file.tif|file.lcp> [...]');
  process.exit(1);
}

const results = files.map(bench);

// ------------------------------------------------------------- projections

const ref = results.reduce((a, b) => (b.cells > a.cells ? b : a));
console.log(`\n${'='.repeat(78)}`);
console.log(`PROJECTIONS  (linear in cell count, from ${ref.cells.toLocaleString()} cells)`);
console.log('='.repeat(78));
console.log(`  grid                  cells      CPU ms   packed MB  @2.9:1 MB` +
            `   100Mbit s`);

const targets = [
  ['482 x 554 (sample)', 482, 554],
  ['1024²', 1024, 1024],
  ['2048²', 2048, 2048],
  ['4096² (brief target)', 4096, 4096],
  ['8192²', 8192, 8192],
  ['California bbox', 37000, 38000],
  ['LANDFIRE CONUS', 97283, 154208],
];

for (const [name, w, h] of targets) {
  const c = w * h;
  const ms = ref.perCellMs * c;
  const mb = (c * 8) / 1048576;
  const wire = mb / 2.9; // MEASURED deflate ratio on packed cells, not the assumed 5:1
  const net = (wire * 8) / 100; // seconds at 100 Mbit/s
  console.log(`  ${name.padEnd(22)} ${c.toLocaleString().padStart(13)} ` +
              `${ms > 100000 ? (ms / 1000).toFixed(0) + ' s' : ms.toFixed(0) + ' ms'}`.padStart(11) +
              `${mb.toFixed(1)}`.padStart(12) +
              `${wire.toFixed(1)}`.padStart(10) +
              `${net < 0.1 ? '<0.1' : net.toFixed(1)}`.padStart(12));
}

console.log(`\nCaveats:`);
console.log(`  - Projection is linear from the largest measured grid. Measured`);
console.log(`    scaling 1024²->4096² is ~1.3x worse than linear, so treat the`);
console.log(`    very large rows as optimistic floors.`);
console.log(`  - Deflate on packed cells measured 2.9:1 (4096²) to 4.05:1 (1024²),`);
console.log(`    NOT the 5:1 the research brief assumes. ZSTD would do somewhat`);
console.log(`    better, but 5:1 should not be used for sizing.`);
console.log(`  - Single-threaded. Deinterleave and pack are per-cell independent,`);
console.log(`    so both parallelize across Workers.`);
console.log(`  - Measured in bun on the dev machine, NOT in a browser. Browser`);
console.log(`    typed-array performance is comparable but should be confirmed.`);
