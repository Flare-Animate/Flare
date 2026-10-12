// Extract the PNG payload from flare/sources/flare/Flare.ico and decode it to
// raw RGBA for the Rust Studio to embed.
//
// Why this exists: Flare.ico is a PNG-in-ICO container, so Rust cannot read it
// without pulling in a PNG decoder. Decoding it once here and committing the
// result lets the Studio use the *exact* main-app icon with include_bytes! and
// no new crate dependencies (and no new Cargo.lock churn).
//
//   node tools/flash/webapp/../icon/extract-ico.mjs     (run from repo root)
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
// tools/icon -> tools -> repo root
const REPO = join(HERE, "..", "..");
const ICO = join(REPO, "flare", "sources", "flare", "Flare.ico");

export function pngFromIco(buf) {
  if (buf.readUInt16LE(0) !== 0 || buf.readUInt16LE(2) !== 1) {
    throw new Error("not an ICO file");
  }
  const count = buf.readUInt16LE(4);
  let best = null;
  for (let i = 0; i < count; i++) {
    const o = 6 + i * 16;
    const size = buf.readUInt32LE(o + 8);
    const off = buf.readUInt32LE(o + 12);
    const w = buf[o] === 0 ? 256 : buf[o];
    const h = buf[o + 1] === 0 ? 256 : buf[o + 1];
    const isPng = buf[off] === 0x89 && buf[off + 1] === 0x50;
    if (isPng && (!best || w > best.w)) best = { w, h, off, size };
  }
  if (!best) throw new Error("no PNG-compressed entry in ICO");
  return { png: buf.subarray(best.off, best.off + best.size), w: best.w, h: best.h };
}

/** Minimal PNG decoder: 8-bit RGB/RGBA, non-interlaced. */
export function decodePng(buf) {
  const SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  for (let i = 0; i < 8; i++) if (buf[i] !== SIG[i]) throw new Error("not a PNG");

  let off = 8;
  let width = 0, height = 0, bitDepth = 0, colorType = 0, interlace = 0;
  const idat = [];
  let palette = null, trns = null;

  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === "PLTE") palette = Buffer.from(data);
    else if (type === "tRNS") trns = Buffer.from(data);
    else if (type === "IDAT") idat.push(Buffer.from(data));
    else if (type === "IEND") break;
    off += 12 + len;
  }

  if (bitDepth !== 8) throw new Error(`unsupported bit depth ${bitDepth}`);
  if (interlace !== 0) throw new Error("interlaced PNG not supported");

  const channels = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[colorType];
  if (!channels) throw new Error(`unsupported colour type ${colorType}`);

  const raw = inflateSync(Buffer.concat(idat));
  const bpp = channels;                 // bytes per pixel at depth 8
  const stride = width * bpp;
  const out = Buffer.alloc(height * stride);

  let pos = 0;
  for (let y = 0; y < height; y++) {
    const filter = raw[pos++];
    const line = raw.subarray(pos, pos + stride);
    pos += stride;
    const cur = out.subarray(y * stride, (y + 1) * stride);
    const prev = y > 0 ? out.subarray((y - 1) * stride, y * stride) : null;

    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? cur[x - bpp] : 0;
      const b = prev ? prev[x] : 0;
      const c = prev && x >= bpp ? prev[x - bpp] : 0;
      let v = line[x];
      switch (filter) {
        case 0: break;
        case 1: v += a; break;
        case 2: v += b; break;
        case 3: v += (a + b) >> 1; break;
        case 4: {
          const p = a + b - c;
          const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
          v += (pa <= pb && pa <= pc) ? a : (pb <= pc ? b : c);
          break;
        }
        default: throw new Error(`bad filter ${filter} on row ${y}`);
      }
      cur[x] = v & 0xff;
    }
  }

  // normalise to RGBA
  const rgba = Buffer.alloc(width * height * 4);
  for (let i = 0, n = width * height; i < n; i++) {
    const s = i * bpp, d = i * 4;
    if (colorType === 6) { rgba[d] = out[s]; rgba[d + 1] = out[s + 1]; rgba[d + 2] = out[s + 2]; rgba[d + 3] = out[s + 3]; }
    else if (colorType === 2) { rgba[d] = out[s]; rgba[d + 1] = out[s + 1]; rgba[d + 2] = out[s + 2]; rgba[d + 3] = 255; }
    else if (colorType === 0) { rgba[d] = rgba[d + 1] = rgba[d + 2] = out[s]; rgba[d + 3] = 255; }
    else if (colorType === 4) { rgba[d] = rgba[d + 1] = rgba[d + 2] = out[s]; rgba[d + 3] = out[s + 1]; }
    else if (colorType === 3) {
      const idx = out[s] * 3;
      rgba[d] = palette[idx]; rgba[d + 1] = palette[idx + 1]; rgba[d + 2] = palette[idx + 2];
      rgba[d + 3] = trns && out[s] < trns.length ? trns[out[s]] : 255;
    }
  }
  return { width, height, rgba };
}

if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  const { png, w, h } = pngFromIco(readFileSync(ICO));
  const outPng = join(HERE, "..", "..", "flare", "sources", "flare", "rust", "studio", "assets", "flare-icon.png");
  const outRgba = join(HERE, "..", "..", "flare", "sources", "flare", "rust", "studio", "assets", "flare-icon.rgba");
  const outMeta = join(HERE, "flare-icon.json");
  const img = decodePng(png);
  if (img.width !== w || img.height !== h) {
    throw new Error(`size mismatch: dir says ${w}x${h}, IHDR says ${img.width}x${img.height}`);
  }
  mkdirSync(dirname(outPng), { recursive: true });
  writeFileSync(outPng, png);
  writeFileSync(outRgba, img.rgba);
  writeFileSync(outMeta, JSON.stringify({ width: img.width, height: img.height }, null, 2) + "\n");
  console.log(`flare-icon.png  ${png.length} bytes`);
  console.log(`flare-icon.rgba ${img.rgba.length} bytes (${img.width}x${img.height}x4)`);
  // sanity: a real icon must not be uniformly transparent or opaque
  let opaque = 0;
  for (let i = 3; i < img.rgba.length; i += 4) if (img.rgba[i] > 0) opaque++;
  const pct = ((opaque / (img.width * img.height)) * 100).toFixed(1);
  console.log(`non-transparent pixels: ${opaque} (${pct}%)`);
}