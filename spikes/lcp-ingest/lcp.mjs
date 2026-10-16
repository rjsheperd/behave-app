// LCP / LCP-as-GeoTIFF reader — no dependencies, runs in browser or bun/node.
//
// Plan C spike (FLAMMAP_WEBGPU_PLANS.org §5): determine the cost of loading
// FlamMap landscape data without GDAL. Two input paths:
//
//   1. Raw .lcp  — 7316-byte `headdata` header, then int16 pixel-interleaved.
//      Header size verified by summing firemodels/lib/flammap/include/flammap3.h:190-269.
//   2. LCP-as-GeoTIFF — what LANDFIRE and SpatialFOFEM actually ship.
//
// Both yield the same thing: per-band Int16Array planes plus geo metadata.

// ---------------------------------------------------------------- raw .lcp

export const LCP_HEADER_BYTES = 7316;

// Field-by-field, NOT a struct memcpy. The on-disk header is unpadded, but the
// C struct would be 7320 with natural alignment (a double at offset 12 forces
// 4 bytes of padding after the three leading longs). Reading it as a packed
// struct is the obvious way to get this wrong.
export function parseLcpHeader(buf) {
  const dv = new DataView(buf);
  let p = 0;
  const i32 = () => { const v = dv.getInt32(p, true); p += 4; return v; };
  const f64 = () => { const v = dv.getFloat64(p, true); p += 8; return v; };
  const i16 = () => { const v = dv.getInt16(p, true); p += 2; return v; };
  const str = (n) => {
    const b = new Uint8Array(buf, p, n); p += n;
    const z = b.indexOf(0);
    return new TextDecoder().decode(z < 0 ? b : b.subarray(0, z));
  };
  // A "theme" is lo/hi/num + a 100-long category table = 412 bytes.
  const theme = () => {
    const lo = i32(), hi = i32(), num = i32();
    const cats = new Int32Array(25); // only first 25 kept; table is 100 longs
    for (let i = 0; i < 100; i++) { const v = i32(); if (i < 25) cats[i] = v; }
    return { lo, hi, num, cats };
  };

  const h = {};
  h.crownFuels = i32();   // 20 = absent, 21 = present
  h.groundFuels = i32();  // 20 = absent, 21 = present
  h.latitude = i32();
  h.loeast = f64(); h.hieast = f64(); h.lonorth = f64(); h.hinorth = f64();
  h.themes = {};
  for (const k of ['elev', 'slope', 'aspect', 'fuel', 'cover',
                   'height', 'base', 'density', 'duff', 'woody']) {
    h.themes[k] = theme();
  }
  h.numeast = i32(); h.numnorth = i32();
  h.eastUtm = f64(); h.westUtm = f64(); h.northUtm = f64(); h.southUtm = f64();
  h.gridUnits = i32();                 // 0 metric, 1 english
  h.xResol = f64(); h.yResol = f64();
  h.units = {};
  for (const k of ['e', 's', 'a', 'fOptions', 'c', 'h', 'b', 'p', 'd', 'wOptions']) {
    h.units[k] = i16();
  }
  h.files = {};
  for (const k of ['elev', 'slope', 'aspect', 'fuel', 'cover',
                   'height', 'base', 'density', 'duff', 'woody']) {
    h.files[k] = str(256);
  }
  h.description = str(512);

  if (p !== LCP_HEADER_BYTES) {
    throw new Error(`LCP header parsed ${p} bytes, expected ${LCP_HEADER_BYTES}`);
  }
  // Band count is driven by the header flags, not assumed.
  h.numBands = 5 + (h.crownFuels === 21 ? 3 : 0) + (h.groundFuels === 21 ? 2 : 0);
  return h;
}

export function readRawLcp(buf) {
  const header = parseLcpHeader(buf.slice(0, LCP_HEADER_BYTES));
  const { numeast: width, numnorth: height, numBands } = header;
  const interleaved = new Int16Array(buf, LCP_HEADER_BYTES,
                                     width * height * numBands);
  return { width, height, numBands, header, interleaved, source: 'raw-lcp' };
}

// ------------------------------------------------------------ TIFF / GeoTIFF

const TIFF_TYPE_BYTES = { 1:1, 2:1, 3:2, 4:4, 5:8, 6:1, 7:1, 8:2, 9:4, 10:8, 11:4, 12:8, 16:8, 17:8, 18:8 };

function readTiffIFD(dv, buf, off, le, big) {
  const entries = new Map();
  const count = big ? Number(dv.getBigUint64(off, le)) : dv.getUint16(off, le);
  const entrySize = big ? 20 : 12;
  let p = off + (big ? 8 : 2);

  for (let i = 0; i < count; i++) {
    const e = p + i * entrySize;
    const tag = dv.getUint16(e, le);
    const type = dv.getUint16(e + 2, le);
    const n = big ? Number(dv.getBigUint64(e + 4, le)) : dv.getUint32(e + 4, le);
    const valField = e + (big ? 12 : 8);
    const inlineBytes = big ? 8 : 4;
    const total = (TIFF_TYPE_BYTES[type] ?? 1) * n;
    const base = total <= inlineBytes
      ? valField
      : (big ? Number(dv.getBigUint64(valField, le)) : dv.getUint32(valField, le));

    const vals = [];
    for (let k = 0; k < n; k++) {
      const o = base + k * (TIFF_TYPE_BYTES[type] ?? 1);
      switch (type) {
        case 1: case 7: vals.push(dv.getUint8(o)); break;
        case 3: vals.push(dv.getUint16(o, le)); break;
        case 4: vals.push(dv.getUint32(o, le)); break;
        case 8: vals.push(dv.getInt16(o, le)); break;
        case 9: vals.push(dv.getInt32(o, le)); break;
        case 11: vals.push(dv.getFloat32(o, le)); break;
        case 12: vals.push(dv.getFloat64(o, le)); break;
        case 16: vals.push(Number(dv.getBigUint64(o, le))); break;
        default: vals.push(0);
      }
    }
    entries.set(tag, vals);
  }
  const nextOff = p + count * entrySize;
  const next = big ? Number(dv.getBigUint64(nextOff, le)) : dv.getUint32(nextOff, le);
  return { entries, next };
}

export function readLcpGeoTiff(buf) {
  const dv = new DataView(buf);
  const le = dv.getUint16(0, true) === 0x4949;
  const magic = dv.getUint16(2, le);
  const big = magic === 43;
  if (magic !== 42 && magic !== 43) throw new Error('not a TIFF');
  const firstIFD = big ? Number(dv.getBigUint64(8, le)) : dv.getUint32(4, le);
  const { entries: t } = readTiffIFD(dv, buf, firstIFD, le, big);

  const get = (tag, dflt) => (t.has(tag) ? t.get(tag) : dflt);
  const width = get(256)[0];
  const height = get(257)[0];
  const numBands = get(277, [1])[0];
  const bits = get(258, [16]);
  const compression = get(259, [1])[0];
  const planar = get(284, [1])[0];
  const sampleFormat = get(339, [2])[0];

  if (compression !== 1) {
    throw new Error(`compression ${compression} unsupported in this spike ` +
                    `(only uncompressed; DEFLATE/ZSTD would need a decoder)`);
  }
  if (bits.some((b) => b !== 16)) throw new Error(`expected 16-bit bands, got ${bits}`);
  if (planar !== 1) throw new Error(`expected pixel-interleaved (planar=1), got ${planar}`);

  // Gather sample data from strips or tiles into one interleaved plane.
  //
  // IMPORTANT: strip/tile offsets are arbitrary byte offsets and are routinely
  // ODD (this sample's first strip is at 5049). `new Int16Array(buf, odd)`
  // throws "Byte offset is not aligned", so zero-copy int16 views over TIFF
  // sample data are not generally possible. We copy bytes into an aligned
  // destination instead — one unavoidable memcpy per strip/tile.
  const interleaved = new Int16Array(width * height * numBands);
  const dest8 = new Uint8Array(interleaved.buffer);
  const rowBytes = width * numBands * 2;
  let bytesCopied = 0;

  if (t.has(324)) {
    // Tiled.
    const tw = get(322)[0], th = get(323)[0];
    const offs = get(324), counts = get(325);
    const tilesAcross = Math.ceil(width / tw);
    const tileRowBytes = tw * numBands * 2;
    for (let i = 0; i < offs.length; i++) {
      const tx = (i % tilesAcross) * tw, ty = Math.floor(i / tilesAcross) * th;
      for (let r = 0; r < th; r++) {
        const y = ty + r; if (y >= height) break;
        const nBytes = Math.min(tw, width - tx) * numBands * 2;
        const src = new Uint8Array(buf, offs[i] + r * tileRowBytes, nBytes);
        dest8.set(src, y * rowBytes + tx * numBands * 2);
      }
      bytesCopied += counts[i];
    }
  } else {
    // Stripped — the common LCP case (RowsPerStrip is typically 1).
    const rps = get(278, [height])[0];
    const offs = get(273), counts = get(279);
    for (let i = 0; i < offs.length; i++) {
      const src = new Uint8Array(buf, offs[i], counts[i]);
      dest8.set(src, i * rps * rowBytes);
      bytesCopied += counts[i];
    }
  }

  // Big-endian files need a byte swap; TIFF sample data follows the file's order.
  if (!le) {
    for (let i = 0; i < dest8.length; i += 2) {
      const a = dest8[i]; dest8[i] = dest8[i + 1]; dest8[i + 1] = a;
    }
  }

  const scale = get(33550, [0, 0, 0]);
  const tie = get(33922, [0, 0, 0, 0, 0, 0]);

  return {
    width, height, numBands, interleaved, source: 'geotiff',
    bytesCopied,
    meta: {
      compression, planar, sampleFormat, bits: bits[0],
      tiled: t.has(324),
      rowsPerStrip: t.has(278) ? get(278)[0] : null,
      stripOrTileCount: (get(324) ?? get(273) ?? []).length,
      cellSize: scale[0], originX: tie[3], originY: tie[4],
    },
  };
}

// ------------------------------------------------------- deinterleave + pack

/** Split pixel-interleaved samples into per-band planes. */
export function deinterleave({ width, height, numBands, interleaved }) {
  const n = width * height;
  const planes = Array.from({ length: numBands }, () => new Int16Array(n));
  for (let c = 0; c < n; c++) {
    const base = c * numBands;
    for (let b = 0; b < numBands; b++) planes[b][c] = interleaved[base + b];
  }
  return planes;
}

/**
 * Pack to the 8 B/cell GPU layout from wildfire-sim-research.org / review §8.
 * Two u32 words per cell:
 *   word0: fuel(u8) | cover(u8) | height(u8) | base(u8)
 *   word1: density(u8) | elev(u16, offset-encoded) | flags(u8)
 * Slope and aspect are intentionally NOT stored — derived on the GPU.
 */
export function packCells(planes, width, height, { crownFuels = true } = {}) {
  const n = width * height;
  const out = new Uint32Array(n * 2);
  const [elev, , , fuel, cover, hgt, base, dens] = planes;
  // Offset-encode elevation so it fits u16 even with negative values.
  let minElev = 32767;
  for (let i = 0; i < n; i++) if (elev[i] > -9999 && elev[i] < minElev) minElev = elev[i];

  for (let i = 0; i < n; i++) {
    const f = fuel[i] < 0 ? 0 : Math.min(255, fuel[i]);
    const cc = cover[i] < 0 ? 0 : Math.min(100, cover[i]);
    // LANDFIRE stores canopy height and base as metres x 10.
    const ch = crownFuels && hgt ? Math.min(255, Math.max(0, Math.round(hgt[i] / 2.5))) : 0;
    const cb = crownFuels && base ? Math.min(255, Math.max(0, base[i])) : 0;
    const cd = crownFuels && dens ? Math.min(255, Math.max(0, dens[i])) : 0;
    const e = Math.min(65535, Math.max(0, elev[i] - minElev));
    const flags = (fuel[i] <= 0 || fuel[i] === 91 || fuel[i] === 98 || fuel[i] === 99) ? 1 : 0;

    out[i * 2] = f | (cc << 8) | (ch << 16) | (cb << 24);
    out[i * 2 + 1] = cd | (e << 8) | (flags << 24);
  }
  return { packed: out, minElev, bytesPerCell: 8 };
}

/**
 * Fused deinterleave + pack: write the GPU layout straight from the
 * pixel-interleaved samples, never materialising per-band planes.
 *
 * Band order is the LCP convention: elev, slope, aspect, fuel, cover,
 * [height, base, density], [duff, woody].
 *
 * This skips an entire pass over the data (and ~numBands x 2 bytes/cell of
 * intermediate allocation). Measured to roughly halve total ingest cost at
 * 4096² — see README.org.
 */
export function packCellsDirect({ interleaved, numBands }, width, height) {
  const n = width * height;
  const out = new Uint32Array(n * 2);
  const nb = numBands;

  let minElev = 32767;
  for (let c = 0; c < n; c++) {
    const e = interleaved[c * nb];
    if (e > -9999 && e < minElev) minElev = e;
  }

  const hasCrown = nb >= 8;
  for (let c = 0; c < n; c++) {
    const b = c * nb;
    const fuel = interleaved[b + 3];
    const f = fuel < 0 ? 0 : (fuel > 255 ? 255 : fuel);
    const ccRaw = interleaved[b + 4];
    const cc = ccRaw < 0 ? 0 : (ccRaw > 100 ? 100 : ccRaw);
    let ch = 0, cb = 0, cd = 0;
    if (hasCrown) {
      const hRaw = interleaved[b + 5];           // metres x 10
      // Must ROUND, not truncate, to stay bit-identical with packCells().
      ch = hRaw <= 0 ? 0 : (hRaw > 637 ? 255 : Math.round(hRaw / 2.5));
      const bRaw = interleaved[b + 6];
      cb = bRaw <= 0 ? 0 : (bRaw > 255 ? 255 : bRaw);
      const dRaw = interleaved[b + 7];
      cd = dRaw <= 0 ? 0 : (dRaw > 255 ? 255 : dRaw);
    }
    let e = interleaved[b] - minElev;
    e = e < 0 ? 0 : (e > 65535 ? 65535 : e);
    const flags = (fuel <= 0 || fuel === 91 || fuel === 98 || fuel === 99) ? 1 : 0;

    out[c * 2] = f | (cc << 8) | (ch << 16) | (cb << 24);
    out[c * 2 + 1] = cd | (e << 8) | (flags << 24);
  }
  return { packed: out, minElev, bytesPerCell: 8 };
}

export function load(buf) {
  // Discriminate by TIFF magic; anything else is treated as a raw .lcp.
  const dv = new DataView(buf);
  const b0 = dv.getUint16(0, true);
  const isTiff = (b0 === 0x4949 || b0 === 0x4d4d);
  return isTiff ? readLcpGeoTiff(buf) : readRawLcp(buf);
}
