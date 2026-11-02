// Verification for extract-ico.mjs's hand-rolled PNG decoder.
//
// A wrong unfilter or a mis-sized stride produces high-frequency noise; a real
// icon has large flat areas. These checks catch that, and the round-trip proves
// the RGBA we are about to embed survives a re-encode unchanged.
import { readFileSync } from "node:fs";
import { deflateSync, inflateSync } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { decodePng, pngFromIco } from "./extract-ico.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, "..", "..");
const meta = JSON.parse(readFileSync(join(HERE, "flare-icon.json"), "utf8"));
const rgba = readFileSync(join(REPO, "flare", "sources", "flare", "rust", "studio", "assets", "flare-icon.rgba"));
const { width, height } = meta;

let fail = 0;
const check = (name, cond, detail) => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? " -- " + detail : ""}`);
  if (!cond) fail++;
};

// 1. size
check("rgba length is width*height*4",
  rgba.length === width * height * 4, `${rgba.length}`);

// 2. flatness: fraction of horizontally-adjacent pairs that are identical
let same = 0, total = 0;
for (let y = 0; y < height; y++) {
  for (let x = 0; x + 1 < width; x++) {
    const a = (y * width + x) * 4, b = a + 4;
    if (rgba[a] === rgba[b] && rgba[a + 1] === rgba[b + 1] &&
        rgba[a + 2] === rgba[b + 2] && rgba[a + 3] === rgba[b + 3]) same++;
    total++;
  }
}
const flat = same / total;
check("image is flat, not noise", flat > 0.5, `${(flat * 100).toFixed(1)}% identical neighbour pairs`);

// 3. round-trip: inflate the re-encoded PNG and unfilter, must equal rgba
const reenc = readFileSync(join(HERE, "flare-icon-decoded.png"));
const back = decodePng(reenc);
check("re-decode round-trips to identical RGBA",
  Buffer.compare(back.rgba, rgba) === 0,
  back.rgba.equals(rgba) ? "byte-identical" : "MISMATCH");

// 4. against the source of truth: decode the ICO payload a second time
const ico = readFileSync(join(REPO, "flare", "sources", "flare", "Flare.ico"));
const fromIco = decodePng(pngFromIco(ico).png);
check("matches a fresh decode of Flare.ico",
  Buffer.compare(fromIco.rgba, rgba) === 0);

// 5. alpha is meaningful (not all 0, not all 255)
let clear = 0, solid = 0;
for (let i = 3; i < rgba.length; i += 4) {
  if (rgba[i] === 0) clear++;
  else if (rgba[i] === 255) solid++;
}
const n = width * height;
check("has both transparent and opaque pixels",
  clear > 0 && solid > 0,
  `${((clear / n) * 100).toFixed(1)}% clear, ${((solid / n) * 100).toFixed(1)}% solid`);

process.exit(fail ? 1 : 0);