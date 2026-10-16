import { readFileSync, readdirSync } from 'node:fs';
import { load, packCellsDirect } from './lcp.mjs';
import { decoders } from './decoders-node.mjs';
const dir = process.argv[2];
const ab = (p) => { const b = readFileSync(p); return b.buffer.slice(b.byteOffset, b.byteOffset+b.byteLength); };
const files = readdirSync(dir);
// Group by the WxH embedded in the filename so each compressed variant is
// compared against the uncompressed baseline of the SAME grid.
const sizes = [...new Set(files.map(f => f.match(/(\d+x\d+)/)?.[1]).filter(Boolean))];
let fail = 0;
for (const sz of sizes.sort()) {
  const refName = `synth_${sz}_tile.tif`;
  if (!files.includes(refName)) continue;
  const ref = load(ab(`${dir}/${refName}`), { decoders });
  const refPack = packCellsDirect(ref, ref.width, ref.height).packed;
  console.log(`\n${sz}  reference=${refName}  (${ref.width}x${ref.height}, ${ref.numBands} bands)`);
  for (const f of files.filter(f => f.includes(sz) && f !== refName && f.endsWith('.tif')).sort()) {
    const img = load(ab(`${dir}/${f}`), { decoders });
    if (img.width !== ref.width || img.height !== ref.height) { console.log(`  ${f} SKIP (dims)`); continue; }
    let d = 0, first = -1;
    for (let i = 0; i < ref.interleaved.length; i++)
      if (img.interleaved[i] !== ref.interleaved[i]) { d++; if (first<0) first=i; }
    const pk = packCellsDirect(img, img.width, img.height).packed;
    let pd = 0; for (let i=0;i<refPack.length;i++) if (pk[i]!==refPack[i]) pd++;
    if (d || pd) fail++;
    console.log(`  ${f.padEnd(34)} codec=${(img.meta.compressionName+'/p'+img.meta.predictor).padEnd(12)} ` +
                `samples:${d===0?'IDENTICAL':`${d} diff @${first}`}  packed:${pd===0?'IDENTICAL':`${pd} diff`}`);
  }
}
console.log(`\n${fail === 0 ? 'ALL VARIANTS BYTE-IDENTICAL' : `${fail} FAILING VARIANTS`}`);
process.exit(fail ? 1 : 0);
