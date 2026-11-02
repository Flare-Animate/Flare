import assert from "node:assert/strict";
import { deflateRawSync } from "node:zlib";
import { route, isSwf, parseN2DBlob, msgpackDecode, unzip } from "./swf-import.ts";

assert.equal(route("a.SWF"), "swf"); assert.equal(route("a.n2d"), "n2d"); assert.equal(route("a.fla"), "unsupported");
assert.ok(isSwf(Uint8Array.from([0x43, 0x57, 0x53, 9]))); assert.ok(!isSwf(Uint8Array.from([1, 2, 3])));
// msgpack: {"a":[1,-2,"hi",true,null,1.5]}
const mp = Uint8Array.from([0x81, 0xa1, 0x61, 0x96, 1, 0xfe, 0xa2, 0x68, 0x69, 0xc3, 0xc0, 0xcb, 0x3f, 0xf8, 0, 0, 0, 0, 0, 0]);
const want = { a: [1, -2, "hi", true, null, 1.5] };
assert.deepEqual(msgpackDecode(mp), want);
function zip(name: string, data: Uint8Array, method = 8): Uint8Array {
  const raw = method ? new Uint8Array(deflateRawSync(data)) : data, nb = Buffer.from(name);
  const lh = Buffer.alloc(30); lh.writeUInt32LE(0x04034b50, 0); lh.writeUInt16LE(method, 8); lh.writeUInt32LE(raw.length, 18); lh.writeUInt32LE(data.length, 22); lh.writeUInt16LE(nb.length, 26);
  const cd = Buffer.alloc(46); cd.writeUInt32LE(0x02014b50, 0); cd.writeUInt16LE(method, 10); cd.writeUInt32LE(raw.length, 20); cd.writeUInt32LE(data.length, 24); cd.writeUInt16LE(nb.length, 28); cd.writeUInt32LE(0, 42);
  const off = 30 + nb.length + raw.length, e = Buffer.alloc(22);
  e.writeUInt32LE(0x06054b50, 0); e.writeUInt16LE(1, 10); e.writeUInt32LE(46 + nb.length, 12); e.writeUInt32LE(off, 16);
  return Buffer.concat([lh, nb, raw, cd, nb, e]);
}
assert.deepEqual([...unzip(zip("x", Uint8Array.from([7, 7]))).get("x")!], [7, 7]);
assert.deepEqual(parseN2DBlob(zip("project.msgpack", mp)), { type: "parsed", data: want, format: "msgpack" });
assert.deepEqual(parseN2DBlob(zip("project.json", Buffer.from('{"k":1}'), 0)), { type: "parsed", data: { k: 1 }, format: "json" });
assert.equal(parseN2DBlob(zip("other", Uint8Array.from([1]))).type, "error");
assert.equal(parseN2DBlob(Uint8Array.from([1, 2, 3, 4])).type, "raw-blob");
console.log("ts ok");
