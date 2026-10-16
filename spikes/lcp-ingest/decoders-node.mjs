// Decoder wiring for bun/node. `lcp.mjs` stays environment-agnostic so the
// same reader runs in the browser; only this file differs per host.
//
// Browser equivalent:
//
//   const decoders = {
//     // zlib-wrapped DEFLATE, which is what TIFF compression 8 / 32946 is.
//     inflate: async (u8) => new Uint8Array(await new Response(
//       new Blob([u8]).stream().pipeThrough(new DecompressionStream('deflate'))
//     ).arrayBuffer()),
//     zstd: (u8) => zstdWasmDecompress(u8),
//   };
//
// Two caveats for the browser path:
//   1. `DecompressionStream` is async, so the reader's chunk loop would need
//      to be async too — or decode all chunks up front in a Worker. The
//      research brief's ADR-05 (decode in CPU workers) is the right shape.
//   2. `DecompressionStream('zstd')` exists in recent Chrome but is not
//      broadly available; ship a WASM zstd rather than relying on it.

import { inflateSync } from 'node:zlib';
import { decompress as zstdDecompress } from 'fzstd';

export const decoders = {
  inflate: (u8) => inflateSync(u8),
  // fzstd is pure JS and, per the research brief, the SLOW option:
  // ~0.25–0.34 GB/s versus ~0.9–1.45 GB/s for the zstddec WASM build.
  // Treat zstd timings here as a pessimistic bound.
  zstd: (u8) => zstdDecompress(u8),
};
