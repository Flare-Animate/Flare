// Checks the page's .flare reader and writer against files the desktop app's code made, under Node
// (the same two libraries the page loads). Usage:
//   node scripts/test_flare_file.mjs NATIVE_FILE.flare OUT_DIR
// NATIVE_FILE is a project saved by flare-cli. The script reads it, writes it back with this
// module's writer into OUT_DIR/rewritten.flare, and checks what it can; the Python test around it
// then has flare-cli open the rewritten file.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";
import sqlite3InitModule from "@sqlite.org/sqlite-wasm";
import * as zstd from "@bokuweb/zstd-wasm";
import { createFlareFile } from "../src/lib/flare-file.js";

const [nativePath, outDir] = process.argv.slice(2);
const sqlite3 = await sqlite3InitModule({ print: () => {}, printErr: () => {} });
await zstd.init();
const file = createFlareFile({ sqlite3, zstd });

// 1. A file the desktop code wrote can be read.
const native = new Uint8Array(readFileSync(nativePath));
const parts = file.read(native);
assert.match(parts.projectId, /^[0-9a-f-]{36}$/);
assert.ok(parts.settings.includes("frame_rate"));
assert.ok(parts.records.length >= 1);
assert.ok(parts.records.some((r) => r.kind === "scene"));
for (const r of parts.records) {
  assert.ok(r.digest instanceof Uint8Array && r.digest.length === 32, "a 32 byte digest");
  assert.ok(r.payload instanceof Uint8Array && r.payload.length > 0);
  assert.ok(["cbor", "cbor+zstd"].includes(r.codec));
  assert.equal(typeof r.size, "number");
}

// 2. Written back, it is a database the desktop code opens, with 1 KiB pages and the same rows.
const rewritten = file.write(parts);
writeFileSync(join(outDir, "rewritten.flare"), rewritten);
const again = file.read(rewritten);
assert.deepEqual(again.records.map((r) => [r.id, r.kind, r.ordinal, r.codec, r.size]),
  parts.records.map((r) => [r.id, r.kind, r.ordinal, r.codec, r.size]));
assert.equal(again.projectId, parts.projectId);
assert.equal(again.settings, parts.settings);
for (let i = 0; i < parts.records.length; i++) {
  assert.deepEqual(again.records[i].payload, parts.records[i].payload);
  assert.deepEqual(again.records[i].digest, parts.records[i].digest);
}
assert.equal((rewritten[16] << 8) | rewritten[17], 1024, "1 KiB pages");

// 3. The browser's zstd round-trips, and what it makes is a single zstd frame.
const raw = Uint8Array.from({ length: 70000 }, (_, i) => (i * 7) % 251);
const frame = file.zstdCompress(raw);
assert.ok(frame.length < raw.length);
assert.deepEqual(file.zstdDecompress(frame), raw);
assert.deepEqual([...frame.slice(0, 4)], [0x28, 0xb5, 0x2f, 0xfd]);

// 4. What is not a project is refused in words, never half-read.
assert.throws(() => file.read(new Uint8Array([1, 2, 3, 4, 5])), /not a Flare|SQLite/);
assert.throws(() => file.read(new TextEncoder().encode("this is plainly not a database, just text".repeat(40))),
  /not a Flare|SQLite|malformed/);
const empty = new sqlite3.oo1.DB(":memory:", "c");
empty.exec("CREATE TABLE x(a)");
const stranger = sqlite3.capi.sqlite3_js_db_export(empty.pointer);
empty.close();
assert.throws(() => file.read(stranger), /not a Flare/);

console.log(`ok: ${parts.records.length} records read, written and read again`);
