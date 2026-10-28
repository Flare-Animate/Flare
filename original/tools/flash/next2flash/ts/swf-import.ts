// TypeScript port of Next2Flash swf-timeline-importer.js + workers/swf-parse-worker.js
// (SSF2-Mods-Official/Next2Flash, see ../vendor LICENSE). DOM-free: pure functions.
import { inflateRawSync, inflateSync } from "node:zlib";

export type Route = "n2d" | "swf" | "unsupported";
export const route = (name: string): Route => {
  const e = name.split(".").pop()!.toLowerCase();
  return e === "n2d" ? "n2d" : e === "swf" ? "swf" : "unsupported";
};

export const swfSignature = (b: Uint8Array): string =>
  String.fromCharCode(b[0], b[1], b[2]);
export const isSwf = (b: Uint8Array): boolean =>
  b.length >= 3 && ["FWS", "CWS", "ZWS"].includes(swfSignature(b));

/** Minimal ZIP reader: name -> bytes (stored/deflate). */
export function unzip(b: Uint8Array): Map<string, Uint8Array> {
  const d = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const out = new Map<string, Uint8Array>();
  let eocd = b.length - 22;
  while (eocd >= 0 && d.getUint32(eocd, true) !== 0x06054b50) eocd--;
  if (eocd < 0) throw new Error("not a zip");
  let p = d.getUint32(eocd + 16, true);
  for (let n = d.getUint16(eocd + 10, true); n > 0; n--) {
    const method = d.getUint16(p + 10, true), csize = d.getUint32(p + 20, true);
    const nl = d.getUint16(p + 28, true), el = d.getUint16(p + 30, true), cl = d.getUint16(p + 32, true);
    const lho = d.getUint32(p + 42, true);
    const name = new TextDecoder().decode(b.subarray(p + 46, p + 46 + nl));
    const ds = lho + 30 + d.getUint16(lho + 26, true) + d.getUint16(lho + 28, true);
    const raw = b.subarray(ds, ds + csize);
    out.set(name, method === 0 ? raw : new Uint8Array(inflateRawSync(raw)));
    p += 46 + nl + el + cl;
  }
  return out;
}

/** Minimal MessagePack decoder (no ext types). */
export function msgpackDecode(b: Uint8Array): unknown {
  const d = new DataView(b.buffer, b.byteOffset, b.byteLength);
  let p = 0;
  const str = (n: number) => { const s = new TextDecoder().decode(b.subarray(p, p + n)); p += n; return s; };
  const arr = (n: number) => Array.from({ length: n }, rd);
  const map = (n: number) => { const o: Record<string, unknown> = {}; for (; n > 0; n--) { const k = String(rd()); o[k] = rd(); } return o; };
  const bin = (n: number) => { const s = b.slice(p, p + n); p += n; return s; };
  function rd(): unknown {
    const t = b[p++];
    if (t < 0x80) return t;
    if (t >= 0xe0) return t - 256;
    if (t >= 0xa0 && t < 0xc0) return str(t & 31);
    if (t >= 0x90 && t < 0xa0) return arr(t & 15);
    if (t >= 0x80 && t < 0x90) return map(t & 15);
    const u = (n: 1 | 2 | 4) => { const v = n === 1 ? d.getUint8(p) : n === 2 ? d.getUint16(p) : d.getUint32(p); p += n; return v; };
    switch (t) {
      case 0xc0: return null; case 0xc2: return false; case 0xc3: return true;
      case 0xc4: return bin(u(1)); case 0xc5: return bin(u(2)); case 0xc6: return bin(u(4));
      case 0xca: { const v = d.getFloat32(p); p += 4; return v; }
      case 0xcb: { const v = d.getFloat64(p); p += 8; return v; }
      case 0xcc: return u(1); case 0xcd: return u(2); case 0xce: return u(4);
      case 0xcf: { const v = Number(d.getBigUint64(p)); p += 8; return v; }
      case 0xd0: { const v = d.getInt8(p); p += 1; return v; }
      case 0xd1: { const v = d.getInt16(p); p += 2; return v; }
      case 0xd2: { const v = d.getInt32(p); p += 4; return v; }
      case 0xd3: { const v = Number(d.getBigInt64(p)); p += 8; return v; }
      case 0xd9: return str(u(1)); case 0xda: return str(u(2)); case 0xdb: return str(u(4));
      case 0xdc: return arr(u(2)); case 0xdd: return arr(u(4));
      case 0xde: return map(u(2)); case 0xdf: return map(u(4));
    }
    throw new Error("msgpack: unsupported type 0x" + t.toString(16));
  }
  return rd();
}

export type Parsed =
  | { type: "parsed"; data: unknown; format: "msgpack" | "json" }
  | { type: "raw-blob"; buffer: Uint8Array }
  | { type: "error"; message: string };

/** Port of parseN2DBlob (worker). Sync; legacy zlib blobs inflated as a bonus. */
export function parseN2DBlob(b: Uint8Array): Parsed {
  try {
    if (b.length >= 4 && b[0] === 0x50 && b[1] === 0x4b) {
      const z = unzip(b);
      const m = z.get("project.msgpack");
      if (m) return { type: "parsed", data: msgpackDecode(m), format: "msgpack" };
      const j = z.get("project.json");
      if (j) return { type: "parsed", data: JSON.parse(new TextDecoder().decode(j)), format: "json" };
      throw new Error("No project.msgpack or project.json in ZIP");
    }
    try { return { type: "parsed", data: msgpackDecode(new Uint8Array(inflateSync(b))), format: "msgpack" }; }
    catch { return { type: "raw-blob", buffer: b }; }
  } catch (e) { return { type: "error", message: (e as Error).message }; }
}
