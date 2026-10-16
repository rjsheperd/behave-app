// Generate synthetic LCP test files so projections can be replaced by
// measurements, and so stripped-vs-tiled and raw-vs-GeoTIFF can be compared.
//
//   bun run gen.mjs <outdir> <width> <height> [--tiled] [--raw]
//
// Values are plausible rather than physical: fuel models drawn from FBFM40,
// a smooth elevation field, and correlated canopy. Enough to exercise the
// reader and to be representative for compression ratio.

import { writeFileSync, mkdirSync } from 'node:fs';
import { LCP_HEADER_BYTES } from './lcp.mjs';

// 10 bands = 5 basic + 3 crown + 2 ground, matching CrownFuels=21 and
// GroundFuels=21 in the header. The band count in a raw .lcp is implied by
// those flags, so data and flags must agree or the reader cannot stride.
const BANDS = ['elev', 'slope', 'aspect', 'fuel', 'cover', 'height', 'base', 'density', 'duff', 'woody'];
const FBFM40 = [101, 102, 121, 122, 123, 141, 142, 143, 161, 162, 165, 181, 182, 183, 186, 188, 201, 202];

function synth(width, height) {
  const n = width * height;
  const nb = BANDS.length;
  const px = new Int16Array(n * nb);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * nb;
      // Smooth terrain: two sinusoids, so deltas compress like real DEMs.
      const e = 1200 + 400 * Math.sin(x / 90) + 250 * Math.cos(y / 70);
      const sl = Math.abs(18 * Math.cos(x / 90)) | 0;
      const asp = ((Math.atan2(Math.cos(y / 70), Math.sin(x / 90)) * 180 / Math.PI) + 360) % 360 | 0;
      // Fuel in contiguous patches (run-length friendly, like real fuel maps).
      const f = FBFM40[((x / 40) | 0) * 7 + ((y / 40) | 0) * 3 & 15];
      const cc = Math.max(0, Math.min(100, 40 + 35 * Math.sin(x / 55 + y / 65)) | 0);
      px[i + 0] = e | 0;
      px[i + 1] = sl;
      px[i + 2] = asp;
      px[i + 3] = f;
      px[i + 4] = cc;
      px[i + 5] = cc > 20 ? 150 + (cc % 60) : 0;  // canopy height, m x 10
      px[i + 6] = cc > 20 ? 15 + (cc % 25) : 0;   // crown base
      px[i + 7] = cc > 20 ? 8 + (cc % 12) : 0;    // bulk density
      px[i + 8] = 10 + (f % 7);                   // duff
      px[i + 9] = f > 160 ? 1 + (f % 4) : 0;      // coarse woody
    }
  }
  return { px, nb };
}

function writeRawLcp(path, width, height, px, nb) {
  const head = new ArrayBuffer(LCP_HEADER_BYTES);
  const dv = new DataView(head);
  let p = 0;
  const i32 = (v) => { dv.setInt32(p, v, true); p += 4; };
  const f64 = (v) => { dv.setFloat64(p, v, true); p += 8; };
  const i16 = (v) => { dv.setInt16(p, v, true); p += 2; };
  const skip = (n) => { p += n; };

  i32(21);            // CrownFuels present
  i32(nb >= 9 ? 21 : 20); // GroundFuels
  i32(45);            // latitude
  f64(0); f64(width * 30); f64(0); f64(height * 30);
  for (let t = 0; t < 10; t++) { i32(0); i32(1000); i32(-1); skip(400); }
  i32(width); i32(height);
  f64(width * 30); f64(0); f64(height * 30); f64(0);
  i32(0);             // metric
  f64(30); f64(30);
  for (let u = 0; u < 10; u++) i16(0);
  skip(256 * 10);
  skip(512);
  if (p !== LCP_HEADER_BYTES) throw new Error(`wrote ${p}, expected ${LCP_HEADER_BYTES}`);

  const out = Buffer.concat([Buffer.from(head), Buffer.from(px.buffer, px.byteOffset, px.byteLength)]);
  writeFileSync(path, out);
  return out.byteLength;
}

function writeTiff(path, width, height, px, nb, { tiled = false, tileSize = 256 } = {}) {
  const LE = true;
  const tags = [];
  const addTag = (tag, type, vals) => tags.push({ tag, type, vals });

  const rowBytes = width * nb * 2;
  let chunkOffsets = [], chunkCounts = [], chunkData = [];

  if (tiled) {
    const across = Math.ceil(width / tileSize), down = Math.ceil(height / tileSize);
    for (let ty = 0; ty < down; ty++) {
      for (let tx = 0; tx < across; tx++) {
        const buf = Buffer.alloc(tileSize * tileSize * nb * 2); // padded, per spec
        for (let r = 0; r < tileSize; r++) {
          const y = ty * tileSize + r; if (y >= height) break;
          const cols = Math.min(tileSize, width - tx * tileSize);
          const src = Buffer.from(px.buffer, px.byteOffset + y * rowBytes + tx * tileSize * nb * 2, cols * nb * 2);
          src.copy(buf, r * tileSize * nb * 2);
        }
        chunkData.push(buf); chunkCounts.push(buf.byteLength);
      }
    }
  } else {
    for (let y = 0; y < height; y++) {
      const buf = Buffer.from(Buffer.from(px.buffer, px.byteOffset + y * rowBytes, rowBytes));
      chunkData.push(buf); chunkCounts.push(rowBytes);
    }
  }

  addTag(256, 3, [width]);
  addTag(257, 3, [height]);
  addTag(258, 3, Array(nb).fill(16));
  addTag(259, 3, [1]);
  addTag(262, 3, [1]);
  addTag(277, 3, [nb]);
  addTag(284, 3, [1]);
  addTag(339, 3, Array(nb).fill(2));
  if (tiled) {
    addTag(322, 3, [tileSize]); addTag(323, 3, [tileSize]);
    addTag(324, 4, chunkOffsets); addTag(325, 4, chunkCounts);
  } else {
    addTag(278, 3, [1]);
    addTag(273, 4, chunkOffsets); addTag(279, 4, chunkCounts);
  }
  addTag(33550, 12, [30, 30, 0]);
  addTag(33922, 12, [0, 0, 0, -1390485, 2788095, 0]);
  tags.sort((a, b) => a.tag - b.tag);

  // Offsets are not known until the IFD is sized, but the IFD size depends on
  // how MANY offsets there are. Reserve the slots now, fill the values below.
  chunkOffsets.length = chunkCounts.length;
  chunkOffsets.fill(0);

  const TB = { 1:1, 3:2, 4:4, 12:8 };
  const ifdBytes = 2 + tags.length * 12 + 4;
  let extra = 0;
  for (const t of tags) { const b = TB[t.type] * t.vals.length; if (b > 4) extra += b; }
  // Deliberately start sample data at an ODD offset to reproduce the
  // alignment hazard found in the real BlueMountain file.
  const dataStart = 8 + ifdBytes + extra + 1;
  let o = dataStart;
  for (let i = 0; i < chunkCounts.length; i++) { chunkOffsets[i] = o; o += chunkCounts[i]; }

  const header = Buffer.alloc(dataStart);
  header.write('II', 0, 'ascii');
  header.writeUInt16LE(42, 2);
  header.writeUInt32LE(8, 4);
  let ip = 8;
  header.writeUInt16LE(tags.length, ip); ip += 2;
  let ep = 8 + ifdBytes;
  for (const t of tags) {
    header.writeUInt16LE(t.tag, ip);
    header.writeUInt16LE(t.type, ip + 2);
    header.writeUInt32LE(t.vals.length, ip + 4);
    const bytes = TB[t.type] * t.vals.length;
    const put = (buf, at, v) => {
      if (t.type === 3) buf.writeUInt16LE(v, at);
      else if (t.type === 4) buf.writeUInt32LE(v, at);
      else if (t.type === 12) buf.writeDoubleLE(v, at);
      else buf.writeUInt8(v, at);
    };
    if (bytes <= 4) {
      t.vals.forEach((v, k) => put(header, ip + 8 + k * TB[t.type], v));
    } else {
      header.writeUInt32LE(ep, ip + 8);
      t.vals.forEach((v, k) => put(header, ep + k * TB[t.type], v));
      ep += bytes;
    }
    ip += 12;
  }
  header.writeUInt32LE(0, ip);

  const out = Buffer.concat([header, ...chunkData]);
  writeFileSync(path, out);
  return out.byteLength;
}

const [outdir, wArg, hArg, ...flags] = process.argv.slice(2);
if (!outdir || !wArg) {
  console.error('usage: bun run gen.mjs <outdir> <width> <height> [--tiled] [--raw]');
  process.exit(1);
}
mkdirSync(outdir, { recursive: true });
const width = +wArg, height = +(hArg ?? wArg);
const { px, nb } = synth(width, height);
const tag = `${width}x${height}`;

if (flags.includes('--raw')) {
  const n = writeRawLcp(`${outdir}/synth_${tag}.lcp`, width, height, px, nb);
  console.log(`wrote ${outdir}/synth_${tag}.lcp  ${(n / 1048576).toFixed(1)} MB`);
}
const n1 = writeTiff(`${outdir}/synth_${tag}_strip.tif`, width, height, px, nb, { tiled: false });
console.log(`wrote ${outdir}/synth_${tag}_strip.tif  ${(n1 / 1048576).toFixed(1)} MB`);
if (flags.includes('--tiled')) {
  const n2 = writeTiff(`${outdir}/synth_${tag}_tile.tif`, width, height, px, nb, { tiled: true });
  console.log(`wrote ${outdir}/synth_${tag}_tile.tif  ${(n2 / 1048576).toFixed(1)} MB`);
}
